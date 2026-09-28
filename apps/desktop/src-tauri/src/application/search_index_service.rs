//! SRC-004 — Search index maintenance service.
//!
//! Implements the Phase-3 "Search projection" requirement: rebuild/verify/optimize
//! commands keep canonical data usable even if the derived search indexes break.
//! `search_documents` is a pure projection of `items` (+ source_contexts, tags);
//! the FTS5 tables are maintained by triggers, so a rebuild never loses canonical data.

use crate::infrastructure::database::pool::AppDatabase;
use thiserror::Error;

#[derive(Debug, Error)]
pub enum SearchIndexError {
    #[error("database error: {0}")]
    Database(#[from] sqlx::Error),
}

/// Health report for the search projection.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct SearchIndexHealth {
    /// Non-deleted items that should be searchable.
    pub live_items: i64,
    /// Rows currently present in search_documents.
    pub indexed_docs: i64,
    /// Live items missing a search document.
    pub missing_docs: i64,
    /// Search documents whose item was deleted or no longer exists.
    pub orphan_docs: i64,
    /// FTS5 unicode61 index passes its internal integrity check.
    pub fts_unicode_ok: bool,
    /// FTS5 trigram index passes its internal integrity check.
    pub fts_trigram_ok: bool,
}

impl SearchIndexHealth {
    pub fn is_healthy(&self) -> bool {
        self.missing_docs == 0
            && self.orphan_docs == 0
            && self.fts_unicode_ok
            && self.fts_trigram_ok
    }
}

/// Canonical row data used to (re)derive a search document for an item.
struct RebuildRow {
    item_id: String,
    title: String,
    plain_text: Option<String>,
    normalized_url: Option<String>,
    source_app: String,
}

pub struct SearchIndexService {
    db: AppDatabase,
}

impl SearchIndexService {
    pub fn new(db: AppDatabase) -> Self {
        Self { db }
    }

    /// Verifies the search projection against canonical data and runs the
    /// FTS5 integrity checks. Never mutates data.
    pub async fn verify(&self) -> Result<SearchIndexHealth, SearchIndexError> {
        let pool = self.db.pool();

        let (live_items,): (i64,) =
            sqlx::query_as("SELECT COUNT(*) FROM items WHERE deleted_at IS NULL")
                .fetch_one(pool)
                .await?;
        let (indexed_docs,): (i64,) = sqlx::query_as("SELECT COUNT(*) FROM search_documents")
            .fetch_one(pool)
            .await?;
        let (missing_docs,): (i64,) = sqlx::query_as(
            "SELECT COUNT(*) FROM items i
             WHERE i.deleted_at IS NULL
               AND NOT EXISTS (SELECT 1 FROM search_documents sd WHERE sd.item_id = i.id)",
        )
        .fetch_one(pool)
        .await?;
        let (orphan_docs,): (i64,) = sqlx::query_as(
            "SELECT COUNT(*) FROM search_documents sd
             WHERE NOT EXISTS (
               SELECT 1 FROM items i WHERE i.id = sd.item_id AND i.deleted_at IS NULL)",
        )
        .fetch_one(pool)
        .await?;

        let fts_unicode_ok = fts_integrity_check(pool, "search_fts_unicode").await;
        let fts_trigram_ok = fts_integrity_check(pool, "search_fts_trigram").await;

        Ok(SearchIndexHealth {
            live_items,
            indexed_docs,
            missing_docs,
            orphan_docs,
            fts_unicode_ok,
            fts_trigram_ok,
        })
    }

    /// Rebuilds the search projection from canonical data:
    /// inserts missing documents and removes orphaned ones. FTS5 tables follow
    /// automatically via the search_documents triggers. Returns (rebuilt, removed).
    pub async fn rebuild(&self) -> Result<(i64, i64), SearchIndexError> {
        let pool = self.db.pool();

        // Read phase: derive every rebuilt document fully BEFORE opening the
        // write transaction (in-memory test pools have a single connection).
        let rows: Vec<RebuildRow> = sqlx::query_as(
            "SELECT i.id, COALESCE(i.title, ''), i.plain_text, i.normalized_url,
                    COALESCE(sc.application_name, '')
             FROM items i
             LEFT JOIN source_contexts sc ON sc.id = i.source_context_id
             WHERE i.deleted_at IS NULL
               AND NOT EXISTS (SELECT 1 FROM search_documents sd WHERE sd.item_id = i.id)",
        )
        .fetch_all(pool)
        .await?
        .into_iter()
        .map(
            |(item_id, title, plain_text, normalized_url, source_app)| RebuildRow {
                item_id,
                title,
                plain_text,
                normalized_url,
                source_app,
            },
        )
        .collect();

        // Resolve all tags_text in one grouped query (no per-item round trip).
        let tag_rows: Vec<(String, String)> = sqlx::query_as(
            "SELECT item_id, GROUP_CONCAT(name, ' ') AS tags_text FROM (
                 SELECT it.item_id AS item_id, t.name AS name
                 FROM tags t
                 JOIN item_tags it ON it.tag_id = t.id
                 WHERE t.deleted_at IS NULL
                 ORDER BY it.item_id, t.name)
             GROUP BY item_id",
        )
        .fetch_all(pool)
        .await?;
        let tags_by_item: std::collections::HashMap<String, String> =
            tag_rows.into_iter().collect();
        let tags_texts: Vec<String> = rows
            .iter()
            .map(|r| tags_by_item.get(&r.item_id).cloned().unwrap_or_default())
            .collect();

        // Write phase: single atomic transaction.
        let mut tx = pool.begin().await?;

        // Remove documents whose item is deleted or gone (trigger cleans FTS).
        let removed = sqlx::query(
            "DELETE FROM search_documents
             WHERE NOT EXISTS (
               SELECT 1 FROM items i WHERE i.id = search_documents.item_id AND i.deleted_at IS NULL)",
        )
        .execute(&mut *tx)
        .await?
        .rows_affected() as i64;

        let now = chrono::Utc::now().timestamp_millis();
        let mut rebuilt: i64 = 0;
        for (row, tags_text) in rows.iter().zip(tags_texts.iter()) {
            // Same derivation rules as insert_item_with_search_doc (pool.rs).
            let body_preview: String = row
                .plain_text
                .as_deref()
                .unwrap_or("")
                .chars()
                .take(5000)
                .collect();
            let domain = row
                .normalized_url
                .as_deref()
                .and_then(|u| url::Url::parse(u).ok())
                .and_then(|u| u.host_str().map(String::from))
                .unwrap_or_default();

            sqlx::query(
                "INSERT INTO search_documents (item_id, title, body, url, domain, source_app, tags_text, filename, updated_at)
                 VALUES (?, ?, ?, ?, ?, ?, ?, '', ?)",
            )
            .bind(&row.item_id)
            .bind(&row.title)
            .bind(&body_preview)
            .bind(row.normalized_url.as_deref().unwrap_or(""))
            .bind(&domain)
            .bind(&row.source_app)
            .bind(tags_text)
            .bind(now)
            .execute(&mut *tx)
            .await?;
            rebuilt += 1;
        }

        tx.commit().await?;

        tracing::info!(
            event = "search_index_rebuilt",
            rebuilt = rebuilt,
            removed_orphans = removed,
        );

        Ok((rebuilt, removed))
    }

    /// Runs the FTS5 `optimize` command on both indexes (merges b-trees so
    /// queries stay fast after many incremental updates).
    pub async fn optimize(&self) -> Result<(), SearchIndexError> {
        let pool = self.db.pool();
        sqlx::query("INSERT INTO search_fts_unicode(search_fts_unicode) VALUES ('optimize')")
            .execute(pool)
            .await?;
        sqlx::query("INSERT INTO search_fts_trigram(search_fts_trigram) VALUES ('optimize')")
            .execute(pool)
            .await?;
        Ok(())
    }
}

/// FTS5 self-check: the 'integrity-check' command fails if the index is corrupt.
async fn fts_integrity_check(pool: &sqlx::SqlitePool, table: &str) -> bool {
    // Table names are compile-time constants, never user input.
    let sql = format!("INSERT INTO {table}({table}) VALUES ('integrity-check')");
    sqlx::query(&sql).execute(pool).await.is_ok()
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::application::capture_service::{CaptureService, CaptureTextRequest};
    use crate::domain::entities::item::CaptureMethod;
    use crate::domain::value_objects::EntityId;
    use crate::infrastructure::database::migrator;

    async fn setup() -> AppDatabase {
        let db = AppDatabase::open_in_memory().await.expect("open db");
        migrator::run_migrations(db.pool())
            .await
            .expect("migrations");
        db
    }

    async fn capture(db: &AppDatabase, text: &str) -> String {
        let svc = CaptureService::new(db.clone(), EntityId::new());
        svc.capture_text(CaptureTextRequest {
            text: text.to_string(),
            capture_method: CaptureMethod::ManualHotkey,
            source_app: Some("test.exe".to_string()),
            window_title: None,
        })
        .await
        .expect("capture")
        .item_id
        .to_string()
    }

    /// Inserts an orphan document for a nonexistent item.
    /// FK enforcement is suspended briefly — orphans can only arise from
    /// restore/manual corruption in practice, but rebuild must still handle them.
    async fn insert_orphan_doc(db: &AppDatabase) {
        let pool = db.pool();
        sqlx::query("PRAGMA foreign_keys = OFF")
            .execute(pool)
            .await
            .expect("fk off");
        sqlx::query(
            "INSERT INTO search_documents (item_id, title, body, url, domain, source_app, tags_text, filename, updated_at)
             VALUES ('ghost-item', 'ghost', '', '', '', '', '', '', 0)",
        )
        .execute(pool)
        .await
        .expect("corrupt insert");
        sqlx::query("PRAGMA foreign_keys = ON")
            .execute(pool)
            .await
            .expect("fk on");
    }

    #[tokio::test]
    async fn verify_healthy_after_capture() {
        let db = setup().await;
        capture(&db, "hello world").await;
        capture(&db, "https://example.com/page").await;

        let svc = SearchIndexService::new(db.clone());
        let health = svc.verify().await.expect("verify");
        assert!(health.is_healthy());
        assert_eq!(health.live_items, 2);
        assert_eq!(health.indexed_docs, 2);
    }

    #[tokio::test]
    async fn verify_detects_missing_and_orphan_docs() {
        let db = setup().await;
        let id_a = capture(&db, "alpha content").await;
        let _id_b = capture(&db, "beta content").await;

        // Corrupt: drop one document, leave an orphan pointing at a fake item.
        sqlx::query("DELETE FROM search_documents WHERE item_id = ?")
            .bind(&id_a)
            .execute(db.pool())
            .await
            .expect("corrupt delete");
        insert_orphan_doc(&db).await;

        let svc = SearchIndexService::new(db.clone());
        let health = svc.verify().await.expect("verify");
        assert!(!health.is_healthy());
        assert_eq!(health.missing_docs, 1);
        assert_eq!(health.orphan_docs, 1);
        assert!(health.fts_unicode_ok);
    }

    #[tokio::test]
    async fn rebuild_restores_missing_document_and_removes_orphans() {
        let db = setup().await;
        let id_a = capture(&db, "rebuild me please").await;
        let _id_b = capture(&db, "https://github.com/rust-lang/rust").await;

        sqlx::query("DELETE FROM search_documents WHERE item_id = ?")
            .bind(&id_a)
            .execute(db.pool())
            .await
            .expect("corrupt");
        insert_orphan_doc(&db).await;

        let svc = SearchIndexService::new(db.clone());
        let (rebuilt, removed) = svc.rebuild().await.expect("rebuild");
        assert_eq!(rebuilt, 1);
        assert_eq!(removed, 1);

        // Health is clean again.
        let health = svc.verify().await.expect("verify");
        assert!(health.is_healthy());
        assert_eq!(health.live_items, health.indexed_docs);

        // The rebuilt document is FTS-searchable again and preserves derivation.
        let found: Vec<(String, String, String)> = sqlx::query_as(
            "SELECT sd.item_id, sd.domain, sd.source_app FROM search_fts_unicode
             JOIN search_documents sd ON sd.rowid = search_fts_unicode.rowid
             WHERE search_fts_unicode MATCH ?",
        )
        .bind("\"rebuild\"")
        .fetch_all(db.pool())
        .await
        .expect("fts search");
        assert_eq!(found.len(), 1);
        assert_eq!(found[0].0, id_a);
        assert_eq!(found[0].2, "test.exe");

        // Link item kept its domain derivation.
        let domains: Vec<(String,)> =
            sqlx::query_as("SELECT sd.domain FROM search_documents sd WHERE sd.item_id != ?")
                .bind(&id_a)
                .fetch_all(db.pool())
                .await
                .expect("domain check");
        assert_eq!(domains.len(), 1);
        assert_eq!(domains[0].0, "github.com");
    }

    #[tokio::test]
    async fn rebuild_preserves_existing_documents() {
        let db = setup().await;
        let _id = capture(&db, "untouched document").await;

        let svc = SearchIndexService::new(db.clone());
        let (rebuilt, removed) = svc.rebuild().await.expect("rebuild");
        assert_eq!(rebuilt, 0);
        assert_eq!(removed, 0);

        let health = svc.verify().await.expect("verify");
        assert!(health.is_healthy());
    }

    #[tokio::test]
    async fn optimize_runs_without_error() {
        let db = setup().await;
        capture(&db, "optimize candidate").await;
        let svc = SearchIndexService::new(db.clone());
        svc.optimize().await.expect("optimize");
    }

    #[tokio::test]
    async fn rebuild_excludes_deleted_items() {
        let db = setup().await;
        let id = capture(&db, "soon deleted").await;

        // Hard-leave trail: soft-delete the item, then wipe its doc.
        sqlx::query("UPDATE items SET deleted_at = 1 WHERE id = ?")
            .bind(&id)
            .execute(db.pool())
            .await
            .expect("soft delete");
        sqlx::query("DELETE FROM search_documents WHERE item_id = ?")
            .bind(&id)
            .execute(db.pool())
            .await
            .expect("wipe doc");

        let svc = SearchIndexService::new(db.clone());
        let (rebuilt, _) = svc.rebuild().await.expect("rebuild");
        assert_eq!(rebuilt, 0); // Deleted items are never re-indexed.
        let health = svc.verify().await.expect("verify");
        assert!(health.is_healthy());
        assert_eq!(health.live_items, 0);
    }
}
