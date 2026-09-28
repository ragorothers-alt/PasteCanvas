//! Backup and export service.
//!
//! Backup: consistent SQLite backup + referenced assets + checksums.
//! Export: user-readable JSON/Markdown + original assets.
//!
//! Uses SQLite backup API for consistency (not raw file copy of active WAL).

use crate::domain::value_objects::UnixMs;
use crate::infrastructure::database::pool::AppDatabase;
use serde::Serialize;
use std::path::{Path, PathBuf};
use thiserror::Error;

#[derive(Debug, Error)]
pub enum BackupError {
    #[error("database error: {0}")]
    Database(#[from] sqlx::Error),

    #[error("I/O error: {0}")]
    Io(#[from] std::io::Error),

    #[error("serialization error: {0}")]
    Json(#[from] serde_json::Error),

    #[error("backup directory not found")]
    DirNotFound,

    #[error("backup database not found")]
    BackupDbMissing,

    #[error("backup checksum mismatch")]
    ChecksumMismatch,
}

#[derive(Debug, Clone, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct BackupResultDto {
    pub backup_path: String,
    pub item_count: i64,
    pub asset_count: i64,
    pub created_at: i64,
}

#[derive(Debug, Clone, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct ExportResultDto {
    pub export_path: String,
    pub item_count: i64,
    pub board_count: i64,
    pub created_at: i64,
}

/// Parsed backup manifest (DAT-002).
#[derive(Debug, Clone, serde::Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct BackupManifest {
    #[serde(default)]
    pub version: i64,
    #[serde(default)]
    pub schema_version: i64,
    #[serde(default)]
    pub item_count: i64,
    #[serde(default)]
    pub asset_count: i64,
    #[serde(default)]
    pub db_sha256: Option<String>,
}

#[derive(Debug, Clone, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct RestoreValidationDto {
    pub valid: bool,
    pub schema_version: i64,
    pub item_count: i64,
    pub asset_count: i64,
    pub reason: Option<String>,
}

pub struct BackupService {
    db: AppDatabase,
    data_dir: PathBuf,
}

impl BackupService {
    pub fn new(db: AppDatabase, data_dir: PathBuf) -> Self {
        Self { db, data_dir }
    }

    /// Creates a consistent backup of the database and assets.
    pub async fn create_backup(&self) -> Result<BackupResultDto, BackupError> {
        let now = UnixMs::now();
        let timestamp = chrono::Utc::now().format("%Y%m%d_%H%M%S");
        let backup_dir = self
            .data_dir
            .join("backups")
            .join(format!("backup_{}", timestamp));
        std::fs::create_dir_all(&backup_dir)?;

        // Use VACUUM INTO for a consistent backup (safe with WAL).
        // Quote the path so apostrophes in the user profile cannot break the SQL.
        let db_backup_path = backup_dir.join("pastecanvas.sqlite3");
        let vacuum_sql = vacuum_into_sql(&db_backup_path);
        sqlx::query(&vacuum_sql).execute(self.db.pool()).await?;

        // Heavy file work (DB hash + recursive asset copy, potentially
        // hundreds of MB) runs on the blocking pool.
        let assets_src = self.data_dir.join("assets");
        let db_hash_path = db_backup_path.clone();
        let backup_dir_for_copy = backup_dir.clone();
        let (db_sha256, asset_count) =
            tokio::task::spawn_blocking(move || -> Result<(String, u64), BackupError> {
                let db_sha256 = sha256_file(&db_hash_path)?;
                let assets_dst = backup_dir_for_copy.join("assets");
                let asset_count = if assets_src.exists() {
                    copy_dir_recursive(&assets_src, &assets_dst)?
                } else {
                    0
                };
                Ok((db_sha256, asset_count))
            })
            .await
            .map_err(|e| std::io::Error::other(format!("backup task aborted: {e}")))??;

        // Count items
        let item_count: (i64,) =
            sqlx::query_as("SELECT COUNT(*) FROM items WHERE deleted_at IS NULL")
                .fetch_one(self.db.pool())
                .await?;

        // Write manifest
        let manifest = serde_json::json!({
            "version": 1,
            "createdAt": now.as_millis(),
            "itemCount": item_count.0,
            "assetCount": asset_count,
            "schemaVersion": 1,
            "dbSha256": db_sha256,
        });
        std::fs::write(
            backup_dir.join("manifest.json"),
            serde_json::to_string_pretty(&manifest)?,
        )?;

        tracing::info!(
            event = "backup_created",
            path = %backup_dir.display(),
            item_count = item_count.0,
            asset_count = asset_count,
        );

        prune_old_backups(&self.data_dir.join("backups"), 10);

        Ok(BackupResultDto {
            backup_path: backup_dir.display().to_string(),
            item_count: item_count.0,
            asset_count: asset_count as i64,
            created_at: now.as_millis(),
        })
    }

    /// Exports data in open format (JSON + Markdown).
    pub async fn create_export(&self) -> Result<ExportResultDto, BackupError> {
        let now = UnixMs::now();
        let timestamp = chrono::Utc::now().format("%Y%m%d_%H%M%S");
        let export_dir = self
            .data_dir
            .join("exports")
            .join(format!("export_{}", timestamp));

        // Export items as JSON
        #[derive(sqlx::FromRow)]
        struct ExportItem {
            id: String,
            item_type: String,
            title: Option<String>,
            plain_text: Option<String>,
            created_at: i64,
        }

        let items: Vec<ExportItem> = sqlx::query_as(
            "SELECT id, item_type, title, plain_text, created_at FROM items WHERE deleted_at IS NULL ORDER BY created_at",
        )
        .fetch_all(self.db.pool())
        .await?;

        let boards: Vec<(String, String)> =
            sqlx::query_as("SELECT id, name FROM boards WHERE deleted_at IS NULL")
                .fetch_all(self.db.pool())
                .await?;

        // All file writes (one per item/board + manifest) run on the blocking
        // pool — thousands of synchronous writes must not stall the runtime.
        let created_at = now.as_millis();
        let export_dir_for_writes = export_dir.clone();
        let (item_count, board_count) =
            tokio::task::spawn_blocking(move || -> Result<(i64, i64), BackupError> {
                std::fs::create_dir_all(export_dir_for_writes.join("items"))?;
                std::fs::create_dir_all(export_dir_for_writes.join("boards"))?;

                for item in &items {
                    let filename = format!("{}.md", item.id);
                    let content = format!(
                        "# {}\n\n- Type: {}\n- Created: {}\n\n---\n\n{}\n",
                        item.title.as_deref().unwrap_or("(untitled)"),
                        item.item_type,
                        item.created_at,
                        item.plain_text.as_deref().unwrap_or(""),
                    );
                    std::fs::write(export_dir_for_writes.join("items").join(filename), content)?;
                }

                for (id, name) in &boards {
                    let filename = format!("{}.md", sanitize_filename(name));
                    let content = format!("# Board: {}\n\nID: {}\n", name, id);
                    std::fs::write(export_dir_for_writes.join("boards").join(filename), content)?;
                }

                let manifest = serde_json::json!({
                    "version": 1,
                    "format": "json+markdown",
                    "createdAt": created_at,
                    "itemCount": items.len(),
                    "boardCount": boards.len(),
                });
                std::fs::write(
                    export_dir_for_writes.join("manifest.json"),
                    serde_json::to_string_pretty(&manifest)?,
                )?;

                tracing::info!(
                    event = "export_created",
                    path = %export_dir_for_writes.display(),
                    item_count = items.len(),
                );

                Ok((items.len() as i64, boards.len() as i64))
            })
            .await
            .map_err(|e| {
                BackupError::Io(std::io::Error::other(format!("export task aborted: {e}")))
            })??;

        Ok(ExportResultDto {
            export_path: export_dir.display().to_string(),
            item_count,
            board_count,
            created_at: now.as_millis(),
        })
    }

    /// DAT-002: validates a backup before restore (schema version + files present).
    pub fn validate_backup(&self, backup_path: &Path) -> RestoreValidationDto {
        let manifest_path = backup_path.join("manifest.json");
        let db_path = backup_path.join("pastecanvas.sqlite3");

        if !manifest_path.exists() {
            return RestoreValidationDto {
                valid: false,
                schema_version: 0,
                item_count: 0,
                asset_count: 0,
                reason: Some("manifest.json not found".into()),
            };
        }
        if !db_path.exists() {
            return RestoreValidationDto {
                valid: false,
                schema_version: 0,
                item_count: 0,
                asset_count: 0,
                reason: Some("backup database not found".into()),
            };
        }

        let manifest: BackupManifest = match std::fs::read_to_string(&manifest_path)
            .ok()
            .and_then(|s| serde_json::from_str(&s).ok())
        {
            Some(m) => m,
            None => {
                return RestoreValidationDto {
                    valid: false,
                    schema_version: 0,
                    item_count: 0,
                    asset_count: 0,
                    reason: Some("invalid manifest.json".into()),
                };
            }
        };

        // Only restore backups with a compatible schema version
        if manifest.schema_version != 1 {
            return RestoreValidationDto {
                valid: false,
                schema_version: manifest.schema_version,
                item_count: manifest.item_count,
                asset_count: manifest.asset_count,
                reason: Some(format!(
                    "incompatible schema version {}",
                    manifest.schema_version
                )),
            };
        }

        if let Some(ref expected) = manifest.db_sha256 {
            match sha256_file(&db_path) {
                Ok(actual) if actual.eq_ignore_ascii_case(expected) => {}
                Ok(_) => {
                    return RestoreValidationDto {
                        valid: false,
                        schema_version: manifest.schema_version,
                        item_count: manifest.item_count,
                        asset_count: manifest.asset_count,
                        reason: Some("backup checksum mismatch".into()),
                    };
                }
                Err(e) => {
                    return RestoreValidationDto {
                        valid: false,
                        schema_version: manifest.schema_version,
                        item_count: manifest.item_count,
                        asset_count: manifest.asset_count,
                        reason: Some(format!("checksum failed: {e}")),
                    };
                }
            }
        }

        RestoreValidationDto {
            valid: true,
            schema_version: manifest.schema_version,
            item_count: manifest.item_count,
            asset_count: manifest.asset_count,
            reason: None,
        }
    }

    /// DAT-002: stages a restore to be applied on next startup (safe — the live DB
    /// is never overwritten while open). Writes a pending-restore marker.
    pub fn prepare_restore(&self, backup_path: &Path) -> Result<RestoreValidationDto, BackupError> {
        let validation = self.validate_backup(backup_path);
        if !validation.valid {
            return Ok(validation);
        }
        let marker = serde_json::json!({ "backupPath": backup_path.display().to_string() });
        std::fs::write(
            self.data_dir.join("restore_pending.json"),
            serde_json::to_string_pretty(&marker)?,
        )?;
        tracing::info!(event = "restore_prepared", path = %backup_path.display());
        Ok(validation)
    }

    /// Applies a pending restore (called at startup before the DB pool opens).
    /// Copies the backup DB over the main DB and merges backup assets.
    pub fn apply_pending_restore(data_dir: &Path) -> Result<bool, BackupError> {
        let marker_path = data_dir.join("restore_pending.json");
        if !marker_path.exists() {
            return Ok(false);
        }
        let marker: serde_json::Value =
            serde_json::from_str(&std::fs::read_to_string(&marker_path)?)?;
        let backup_path = match marker.get("backupPath").and_then(|v| v.as_str()) {
            Some(p) => PathBuf::from(p),
            None => {
                let _ = std::fs::remove_file(&marker_path);
                return Ok(false);
            }
        };

        let backup_db = backup_path.join("pastecanvas.sqlite3");
        let main_db = data_dir.join("pastecanvas.sqlite3");
        if !backup_db.exists() {
            return Err(BackupError::BackupDbMissing);
        }

        let manifest_path = backup_path.join("manifest.json");
        if let Ok(raw) = std::fs::read_to_string(&manifest_path) {
            if let Ok(manifest) = serde_json::from_str::<BackupManifest>(&raw) {
                if let Some(ref expected) = manifest.db_sha256 {
                    let actual = sha256_file(&backup_db)?;
                    if !actual.eq_ignore_ascii_case(expected) {
                        return Err(BackupError::ChecksumMismatch);
                    }
                }
            }
        }

        // Copy to a sibling temp file, then swap into place so a crash cannot
        // leave a half-written main database.
        replace_file_atomically(&backup_db, &main_db)?;
        let _ = std::fs::remove_file(data_dir.join("pastecanvas.sqlite3-wal"));
        let _ = std::fs::remove_file(data_dir.join("pastecanvas.sqlite3-shm"));

        // Merge backup assets (content-addressed → safe to copy over)
        let backup_assets = backup_path.join("assets");
        if backup_assets.exists() {
            copy_dir_recursive(&backup_assets, &data_dir.join("assets"))?;
        }

        let _ = std::fs::remove_file(&marker_path);
        tracing::info!(event = "restore_applied", path = %backup_path.display());
        Ok(true)
    }
}

fn copy_dir_recursive(src: &Path, dst: &Path) -> Result<u64, std::io::Error> {
    std::fs::create_dir_all(dst)?;
    let mut count = 0;
    for entry in std::fs::read_dir(src)? {
        let entry = entry?;
        let src_path = entry.path();
        let dst_path = dst.join(entry.file_name());
        if src_path.is_dir() {
            count += copy_dir_recursive(&src_path, &dst_path)?;
        } else {
            std::fs::copy(&src_path, &dst_path)?;
            count += 1;
        }
    }
    Ok(count)
}

fn sanitize_filename(name: &str) -> String {
    name.chars()
        .map(|c| match c {
            'a'..='z' | 'A'..='Z' | '0'..='9' | '-' | '_' => c,
            _ => '_',
        })
        .collect::<String>()
        .chars()
        .take(50)
        .collect()
}

/// Builds a VACUUM INTO statement with a SQL-escaped path.
pub(crate) fn vacuum_into_sql(path: &Path) -> String {
    let escaped = path.to_string_lossy().replace('\'', "''");
    format!("VACUUM INTO '{escaped}'")
}

fn sha256_file(path: &Path) -> Result<String, std::io::Error> {
    use sha2::{Digest, Sha256};
    use std::io::Read;
    let mut file = std::fs::File::open(path)?;
    let mut hasher = Sha256::new();
    let mut buf = [0u8; 64 * 1024];
    loop {
        let n = file.read(&mut buf)?;
        if n == 0 {
            break;
        }
        hasher.update(&buf[..n]);
    }
    Ok(format!("{:x}", hasher.finalize()))
}

/// Writes `src` over `dest` via a sibling temp + rename so a crash cannot leave
/// a truncated destination. On Windows, the live file is moved aside first.
fn replace_file_atomically(src: &Path, dest: &Path) -> Result<(), std::io::Error> {
    let tmp = dest.with_extension("sqlite3.restore-tmp");
    let bak = dest.with_extension("sqlite3.pre-restore");
    if tmp.exists() {
        let _ = std::fs::remove_file(&tmp);
    }
    std::fs::copy(src, &tmp)?;
    if dest.exists() {
        if bak.exists() {
            let _ = std::fs::remove_file(&bak);
        }
        std::fs::rename(dest, &bak)?;
    }
    match std::fs::rename(&tmp, dest) {
        Ok(()) => {
            let _ = std::fs::remove_file(&bak);
            Ok(())
        }
        Err(e) => {
            if bak.exists() && !dest.exists() {
                let _ = std::fs::rename(&bak, dest);
            }
            let _ = std::fs::remove_file(&tmp);
            Err(e)
        }
    }
}

fn prune_old_backups(backups_root: &Path, keep: usize) {
    let Ok(entries) = std::fs::read_dir(backups_root) else {
        return;
    };
    let mut dirs: Vec<_> = entries
        .filter_map(|e| e.ok())
        .filter(|e| e.path().is_dir())
        .collect();
    dirs.sort_by_key(|e| e.file_name());
    if dirs.len() <= keep {
        return;
    }
    let drop_count = dirs.len() - keep;
    for old in dirs.into_iter().take(drop_count) {
        let _ = std::fs::remove_dir_all(old.path());
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn vacuum_into_escapes_single_quotes() {
        let path = Path::new(r"C:\Users\O'Brien\data\pastecanvas.sqlite3");
        let sql = vacuum_into_sql(path);
        assert!(sql.contains("O''Brien"));
        assert!(!sql.contains("O'Brien"));
        assert!(sql.starts_with("VACUUM INTO '"));
        assert!(sql.ends_with("'"));
    }
}
