//! SRC-004 search index maintenance integration tests.
//! Verifies the Phase-3 requirement: canonical data stays usable even if the
//! derived search index breaks (verify / rebuild / optimize).

#[cfg(test)]
mod tests {
    use pastecanvas_desktop_lib::application::capture_service::{
        CaptureService, CaptureTextRequest,
    };
    use pastecanvas_desktop_lib::application::search_index_service::SearchIndexService;
    use pastecanvas_desktop_lib::domain::entities::item::CaptureMethod;
    use pastecanvas_desktop_lib::domain::value_objects::EntityId;
    use pastecanvas_desktop_lib::infrastructure::database::migrator;
    use pastecanvas_desktop_lib::infrastructure::database::pool::AppDatabase;

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
            source_app: None,
            window_title: None,
        })
        .await
        .expect("capture")
        .item_id
        .to_string()
    }

    /// FTS MATCH the same way the search_items command does.
    async fn fts_find(db: &AppDatabase, term: &str) -> Vec<String> {
        sqlx::query_scalar(
            "SELECT sd.item_id FROM search_fts_unicode
             JOIN search_documents sd ON sd.rowid = search_fts_unicode.rowid
             JOIN items i ON i.id = sd.item_id AND i.deleted_at IS NULL
             WHERE search_fts_unicode MATCH ?",
        )
        .bind(term)
        .fetch_all(db.pool())
        .await
        .expect("fts")
    }

    #[tokio::test]
    async fn broken_index_is_repaired_and_searchable_again() {
        let db = setup().await;
        let id = capture(&db, "unique-keyword-phoenix").await;
        capture(&db, "another item entirely").await;

        // Simulate index corruption: the document row disappears.
        sqlx::query("DELETE FROM search_documents WHERE item_id = ?")
            .bind(&id)
            .execute(db.pool())
            .await
            .expect("corrupt");

        // Search no longer finds the item.
        assert!(fts_find(&db, "\"phoenix\"").await.is_empty());

        // Verify reports the damage.
        let svc = SearchIndexService::new(db.clone());
        let health = svc.verify().await.expect("verify");
        assert_eq!(health.missing_docs, 1);
        assert!(!health.is_healthy());

        // Rebuild repairs it from canonical data.
        let (rebuilt, removed) = svc.rebuild().await.expect("rebuild");
        assert_eq!(rebuilt, 1);
        assert_eq!(removed, 0);

        // Item is searchable again; canonical data untouched.
        assert_eq!(fts_find(&db, "\"phoenix\"").await, vec![id.clone()]);
        let health = svc.verify().await.expect("verify after");
        assert!(health.is_healthy());

        // Optimize keeps running cleanly after rebuild.
        svc.optimize().await.expect("optimize");
    }

    #[tokio::test]
    async fn rebuild_is_idempotent() {
        let db = setup().await;
        capture(&db, "first entry").await;
        capture(&db, "second entry").await;

        let svc = SearchIndexService::new(db.clone());
        let (r1, _) = svc.rebuild().await.expect("rebuild 1");
        let (r2, _) = svc.rebuild().await.expect("rebuild 2");
        assert_eq!(r1, 0); // Nothing missing to begin with.
        assert_eq!(r2, 0);
        assert!(svc.verify().await.expect("verify").is_healthy());
    }

    #[tokio::test]
    async fn rebuild_restores_tags_text() {
        let db = setup().await;
        let id = capture(&db, "tagged item content").await;
        let id2 = capture(&db, "untagged item content").await;

        // Attach two tags to the first item (same rows commands/tags.rs writes).
        let now = 1_700_000_000_000i64;
        for (tid, name) in [("tag-1", "alpha"), ("tag-2", "beta")] {
            sqlx::query(
                "INSERT INTO tags (id, name, normalized_name, created_at, updated_at)
                 VALUES (?, ?, ?, ?, ?)",
            )
            .bind(tid)
            .bind(name)
            .bind(name)
            .bind(now)
            .bind(now)
            .execute(db.pool())
            .await
            .expect("insert tag");
            sqlx::query("INSERT INTO item_tags (item_id, tag_id, created_at) VALUES (?, ?, ?)")
                .bind(&id)
                .bind(tid)
                .bind(now)
                .execute(db.pool())
                .await
                .expect("insert item_tag");
        }

        // Corrupt the index: both documents disappear.
        sqlx::query("DELETE FROM search_documents")
            .execute(db.pool())
            .await
            .expect("corrupt");

        let svc = SearchIndexService::new(db.clone());
        let (rebuilt, _) = svc.rebuild().await.expect("rebuild");
        assert_eq!(rebuilt, 2);

        // Tagged item gets its tags_text back (alphabetical, space-joined).
        let tags_text: (String,) =
            sqlx::query_as("SELECT tags_text FROM search_documents WHERE item_id = ?")
                .bind(&id)
                .fetch_one(db.pool())
                .await
                .expect("tagged doc");
        assert_eq!(tags_text.0, "alpha beta");

        // Untagged item rebuilds with an empty tags_text.
        let empty_tags: (String,) =
            sqlx::query_as("SELECT tags_text FROM search_documents WHERE item_id = ?")
                .bind(&id2)
                .fetch_one(db.pool())
                .await
                .expect("untagged doc");
        assert_eq!(empty_tags.0, "");
    }
}
