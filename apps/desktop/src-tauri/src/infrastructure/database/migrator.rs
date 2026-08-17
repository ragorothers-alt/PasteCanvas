use sqlx::SqlitePool;
use thiserror::Error;

/// Errors that can occur during migration.
#[derive(Debug, Error)]
pub enum MigrationError {
    #[error("database error: {0}")]
    Database(#[from] sqlx::Error),

    #[error("migration {version} failed: {reason}")]
    MigrationFailed { version: i64, reason: String },

    #[error("migration {version} already applied but content mismatch")]
    ContentMismatch { version: i64 },
}

/// Embedded migration files, ordered by version.
struct MigrationFile {
    version: i64,
    name: &'static str,
    sql: &'static str,
}

/// All migrations embedded at compile time.
/// Once released, a migration file MUST NOT be modified.
const MIGRATIONS: &[MigrationFile] = &[
    MigrationFile {
        version: 1,
        name: "001_initial_schema",
        sql: include_str!("../../../migrations/001_initial_schema.sql"),
    },
    MigrationFile {
        version: 2,
        name: "002_indexes",
        sql: include_str!("../../../migrations/002_indexes.sql"),
    },
];

/// Runs all pending migrations in order.
///
/// Guarantees:
/// - Each migration runs in its own transaction.
/// - A failed migration rolls back and leaves the database in prior state.
/// - Never creates a silent blank database — if migration 1 fails, the app
///   must surface a recovery screen, not proceed with empty tables.
/// - Applied migrations are recorded with checksum for tamper detection.
pub async fn run_migrations(pool: &SqlitePool) -> Result<Vec<i64>, MigrationError> {
    // Ensure migration tracking table exists
    sqlx::query(
        r"
        CREATE TABLE IF NOT EXISTS _migrations (
            version INTEGER PRIMARY KEY,
            name TEXT NOT NULL,
            checksum TEXT NOT NULL,
            applied_at INTEGER NOT NULL
        )
        ",
    )
    .execute(pool)
    .await?;

    let mut applied_versions: Vec<i64> = Vec::new();

    for migration in MIGRATIONS {
        let existing: Option<(String,)> =
            sqlx::query_as("SELECT checksum FROM _migrations WHERE version = ?")
                .bind(migration.version)
                .fetch_optional(pool)
                .await?;

        if let Some((checksum,)) = existing {
            // Verify content hasn't been tampered with
            let current_checksum = compute_checksum(migration.sql);
            if checksum != current_checksum {
                return Err(MigrationError::ContentMismatch {
                    version: migration.version,
                });
            }
            continue; // Already applied
        }

        // Apply migration in a transaction
        let mut tx = pool.begin().await?;

        // Execute migration SQL
        // Note: SQLite doesn't support transactional DDL for all operations
        // (e.g., CREATE VIRTUAL TABLE), so we execute statements individually
        // but still track success atomically via the _migrations insert.
        sqlx::raw_sql(migration.sql)
            .execute(&mut *tx)
            .await
            .map_err(|e| MigrationError::MigrationFailed {
                version: migration.version,
                reason: e.to_string(),
            })?;

        // Record successful migration
        let checksum = compute_checksum(migration.sql);
        let now = chrono::Utc::now().timestamp_millis();
        sqlx::query(
            "INSERT INTO _migrations (version, name, checksum, applied_at) VALUES (?, ?, ?, ?)",
        )
        .bind(migration.version)
        .bind(migration.name)
        .bind(&checksum)
        .bind(now)
        .execute(&mut *tx)
        .await?;

        tx.commit().await?;

        tracing::info!(
            event = "migration_applied",
            version = migration.version,
            name = migration.name,
        );

        applied_versions.push(migration.version);
    }

    Ok(applied_versions)
}

/// Simple checksum for migration content integrity.
fn compute_checksum(sql: &str) -> String {
    use sha2::{Digest, Sha256};
    let mut hasher = Sha256::new();
    hasher.update(sql.as_bytes());
    format!("{:x}", hasher.finalize())
}

/// Returns the current schema version (highest applied migration).
pub async fn current_schema_version(pool: &SqlitePool) -> Result<Option<i64>, sqlx::Error> {
    // Check if _migrations table exists
    let table_exists: Option<(i32,)> =
        sqlx::query_as("SELECT 1 FROM sqlite_master WHERE type='table' AND name='_migrations'")
            .fetch_optional(pool)
            .await?;

    if table_exists.is_none() {
        return Ok(None);
    }

    let row: Option<(i64,)> = sqlx::query_as("SELECT MAX(version) FROM _migrations")
        .fetch_optional(pool)
        .await?;

    Ok(row.map(|(v,)| v))
}

/// Runs PRAGMA integrity_check on the database.
pub async fn check_integrity(pool: &SqlitePool) -> Result<bool, sqlx::Error> {
    let result: (String,) = sqlx::query_as("PRAGMA integrity_check")
        .fetch_one(pool)
        .await?;
    Ok(result.0 == "ok")
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::infrastructure::database::pool::create_test_pool;

    #[tokio::test]
    async fn migrations_apply_successfully() {
        let pool = create_test_pool().await.expect("create pool");
        let applied = run_migrations(&pool).await.expect("run migrations");
        assert_eq!(applied, vec![1, 2]);

        // Verify schema version
        let version = current_schema_version(&pool).await.expect("version");
        assert_eq!(version, Some(2));
    }

    #[tokio::test]
    async fn migrations_are_idempotent() {
        let pool = create_test_pool().await.expect("create pool");

        let first = run_migrations(&pool).await.expect("first run");
        assert_eq!(first, vec![1, 2]);

        let second = run_migrations(&pool).await.expect("second run");
        assert!(second.is_empty()); // Nothing new to apply
    }

    #[tokio::test]
    async fn integrity_check_passes_after_migration() {
        let pool = create_test_pool().await.expect("create pool");
        run_migrations(&pool).await.expect("migrations");

        let ok = check_integrity(&pool).await.expect("integrity");
        assert!(ok);
    }

    #[tokio::test]
    async fn items_table_exists_after_migration() {
        let pool = create_test_pool().await.expect("create pool");
        run_migrations(&pool).await.expect("migrations");

        let table: Option<(String,)> =
            sqlx::query_as("SELECT name FROM sqlite_master WHERE type='table' AND name='items'")
                .fetch_optional(&pool)
                .await
                .expect("query");
        assert!(table.is_some());
    }

    #[tokio::test]
    async fn fts_tables_exist_after_migration() {
        let pool = create_test_pool().await.expect("create pool");
        run_migrations(&pool).await.expect("migrations");

        let fts: Option<(String,)> = sqlx::query_as(
            "SELECT name FROM sqlite_master WHERE type='table' AND name='search_fts_unicode'",
        )
        .fetch_optional(&pool)
        .await
        .expect("query");
        assert!(fts.is_some());
    }
}
