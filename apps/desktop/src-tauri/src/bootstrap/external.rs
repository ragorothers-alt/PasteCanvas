//! Shared bootstrap for out-of-process helpers (Explorer bridge, NMH).
//!
//! Both binaries must locate the same data directory Tauri's app_data_dir
//! uses and open a migrated database. Kept in one place so the layout and
//! the open/migrate sequence are defined exactly once.

use crate::infrastructure::database::migrator;
use crate::infrastructure::database::pool::AppDatabase;
use std::path::PathBuf;

/// Tauri bundle identifier — must match tauri.conf.json so the helpers open
/// the same database as the desktop app.
const APP_IDENTIFIER: &str = "com.pastecanvas.desktop";
const DB_FILENAME: &str = "pastecanvas.sqlite3";

/// Resolves the app data directory (matches Tauri's app_data_dir layout).
pub fn resolve_data_dir() -> Option<PathBuf> {
    #[cfg(target_os = "windows")]
    {
        std::env::var("APPDATA")
            .ok()
            .map(|p| PathBuf::from(p).join(APP_IDENTIFIER))
    }
    #[cfg(not(target_os = "windows"))]
    {
        std::env::var("HOME")
            .ok()
            .map(|p| PathBuf::from(p).join(".local/share").join(APP_IDENTIFIER))
    }
}

/// Ensures the data directory exists, then opens the database with all
/// migrations applied. Returns the database handle and the data directory.
pub async fn open_migrated_db() -> Result<(AppDatabase, PathBuf), String> {
    let data_dir = resolve_data_dir().ok_or_else(|| "cannot resolve app data dir".to_string())?;
    std::fs::create_dir_all(&data_dir).map_err(|e| format!("create data dir: {e}"))?;
    let db_path = data_dir.join(DB_FILENAME);
    let db = AppDatabase::open(&db_path)
        .await
        .map_err(|e| format!("open db: {e}"))?;
    migrator::run_migrations(db.pool())
        .await
        .map_err(|e| format!("migrations: {e}"))?;
    Ok((db, data_dir))
}
