//! End-to-end integration test: capture → search → find.
//! Verifies the full pipeline works correctly.

#[cfg(test)]
mod tests {
    use pastecanvas_desktop_lib::application::capture_service::{
        CaptureService, CaptureTextRequest,
    };
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

    #[tokio::test]
    async fn capture_then_search_finds_item() {
        let db = setup().await;
        let svc = CaptureService::new(db.clone(), EntityId::new());

        // Capture a text item
        let result = svc
            .capture_text(CaptureTextRequest {
                text: "Rust programming language is great for systems".to_string(),
                capture_method: CaptureMethod::ManualHotkey,
                source_app: Some("vscode.exe".to_string()),
                window_title: None,
            })
            .await
            .expect("capture");

        // Search for it using FTS5
        let search_results: Vec<(String, String)> = sqlx::query_as(
            "SELECT sd.item_id, sd.body FROM search_fts_unicode
             JOIN search_documents sd ON sd.rowid = search_fts_unicode.rowid
             WHERE search_fts_unicode MATCH '\"Rust\"'",
        )
        .fetch_all(db.pool())
        .await
        .expect("search");

        assert_eq!(search_results.len(), 1);
        assert_eq!(search_results[0].0, result.item_id.to_string());
        assert!(search_results[0].1.contains("Rust programming"));
    }

    #[tokio::test]
    async fn capture_url_then_search_by_domain() {
        let db = setup().await;
        let svc = CaptureService::new(db.clone(), EntityId::new());

        // Capture a URL
        svc.capture_text(CaptureTextRequest {
            text: "https://github.com/tokio-rs/tokio".to_string(),
            capture_method: CaptureMethod::ManualHotkey,
            source_app: Some("chrome.exe".to_string()),
            window_title: None,
        })
        .await
        .expect("capture");

        // Search by domain
        let results: Vec<(String,)> = sqlx::query_as(
            "SELECT sd.item_id FROM search_documents sd WHERE sd.domain = 'github.com'",
        )
        .fetch_all(db.pool())
        .await
        .expect("search");

        assert_eq!(results.len(), 1);
    }

    #[tokio::test]
    async fn capture_color_then_verify_type() {
        let db = setup().await;
        let svc = CaptureService::new(db.clone(), EntityId::new());

        // Capture a color
        let result = svc
            .capture_text(CaptureTextRequest {
                text: "#FF5733".to_string(),
                capture_method: CaptureMethod::ManualHotkey,
                source_app: None,
                window_title: None,
            })
            .await
            .expect("capture");

        // Verify it was classified as color
        let item: (String,) = sqlx::query_as("SELECT item_type FROM items WHERE id = ?")
            .bind(result.item_id.as_str())
            .fetch_one(db.pool())
            .await
            .expect("query");

        assert_eq!(item.0, "color");
    }

    #[tokio::test]
    async fn capture_multiple_then_search_filters() {
        let db = setup().await;
        let svc = CaptureService::new(db.clone(), EntityId::new());

        // Capture different types
        svc.capture_text(CaptureTextRequest {
            text: "Python data science tutorial".to_string(),
            capture_method: CaptureMethod::ManualHotkey,
            source_app: Some("firefox.exe".to_string()),
            window_title: None,
        })
        .await
        .expect("capture 1");

        svc.capture_text(CaptureTextRequest {
            text: "https://rust-lang.org".to_string(),
            capture_method: CaptureMethod::ManualHotkey,
            source_app: Some("chrome.exe".to_string()),
            window_title: None,
        })
        .await
        .expect("capture 2");

        svc.capture_text(CaptureTextRequest {
            text: "#00FF00".to_string(),
            capture_method: CaptureMethod::ManualHotkey,
            source_app: None,
            window_title: None,
        })
        .await
        .expect("capture 3");

        // Search all items
        let all: Vec<(String,)> = sqlx::query_as("SELECT id FROM items WHERE deleted_at IS NULL")
            .fetch_all(db.pool())
            .await
            .expect("all");
        assert_eq!(all.len(), 3);

        // Filter by type
        let links: Vec<(String,)> =
            sqlx::query_as("SELECT id FROM items WHERE item_type = 'link' AND deleted_at IS NULL")
                .fetch_all(db.pool())
                .await
                .expect("links");
        assert_eq!(links.len(), 1);

        let colors: Vec<(String,)> =
            sqlx::query_as("SELECT id FROM items WHERE item_type = 'color' AND deleted_at IS NULL")
                .fetch_all(db.pool())
                .await
                .expect("colors");
        assert_eq!(colors.len(), 1);

        // Filter by source app
        let from_chrome: Vec<(String,)> = sqlx::query_as(
            "SELECT i.id FROM items i
             JOIN source_contexts sc ON sc.id = i.source_context_id
             WHERE sc.application_name = 'chrome.exe' AND i.deleted_at IS NULL",
        )
        .fetch_all(db.pool())
        .await
        .expect("chrome");
        assert_eq!(from_chrome.len(), 1);
    }

    #[tokio::test]
    async fn soft_delete_excludes_from_search() {
        let db = setup().await;
        let svc = CaptureService::new(db.clone(), EntityId::new());

        let result = svc
            .capture_text(CaptureTextRequest {
                text: "temporary note to delete".to_string(),
                capture_method: CaptureMethod::ManualHotkey,
                source_app: None,
                window_title: None,
            })
            .await
            .expect("capture");

        // Verify it's searchable
        let before: Vec<(String,)> = sqlx::query_as(
            "SELECT sd.item_id FROM search_fts_unicode
             JOIN search_documents sd ON sd.rowid = search_fts_unicode.rowid
             JOIN items i ON i.id = sd.item_id AND i.deleted_at IS NULL
             WHERE search_fts_unicode MATCH '\"temporary\"'",
        )
        .fetch_all(db.pool())
        .await
        .expect("before");
        assert_eq!(before.len(), 1);

        // Soft delete
        let now = chrono::Utc::now().timestamp_millis();
        sqlx::query("UPDATE items SET deleted_at = ? WHERE id = ?")
            .bind(now)
            .bind(result.item_id.as_str())
            .execute(db.pool())
            .await
            .expect("delete");

        // Verify it's no longer in search results (joined with items)
        let after: Vec<(String,)> = sqlx::query_as(
            "SELECT sd.item_id FROM search_fts_unicode
             JOIN search_documents sd ON sd.rowid = search_fts_unicode.rowid
             JOIN items i ON i.id = sd.item_id AND i.deleted_at IS NULL
             WHERE search_fts_unicode MATCH '\"temporary\"'",
        )
        .fetch_all(db.pool())
        .await
        .expect("after");
        assert_eq!(after.len(), 0);
    }

    #[tokio::test]
    async fn toggle_pin_flips_state() {
        use pastecanvas_desktop_lib::application::item_service::ItemService;

        let db = setup().await;
        let svc = CaptureService::new(db.clone(), EntityId::new());

        let result = svc
            .capture_text(CaptureTextRequest {
                text: "pinnable content".to_string(),
                capture_method: CaptureMethod::ManualHotkey,
                source_app: None,
                window_title: None,
            })
            .await
            .expect("capture");

        let item_svc = ItemService::new(db.clone());

        // Initially not pinned → toggle on
        let pinned = item_svc
            .toggle_pin(result.item_id.as_str())
            .await
            .expect("pin on");
        assert!(pinned);

        // Toggle off
        let unpinned = item_svc
            .toggle_pin(result.item_id.as_str())
            .await
            .expect("pin off");
        assert!(!unpinned);
    }

    #[tokio::test]
    async fn inbox_query_returns_domain_for_smart_grouping() {
        use pastecanvas_desktop_lib::application::item_service::{InboxQuery, ItemService};

        let db = setup().await;
        let svc = CaptureService::new(db.clone(), EntityId::new());

        // Capture a URL — its domain should be indexed in search_documents
        svc.capture_text(CaptureTextRequest {
            text: "https://github.com/rust-lang/rust".to_string(),
            capture_method: CaptureMethod::ManualHotkey,
            source_app: Some("chrome.exe".to_string()),
            window_title: None,
        })
        .await
        .expect("capture");

        let item_svc = ItemService::new(db.clone());
        let items = item_svc
            .query_inbox(InboxQuery {
                limit: 10,
                offset: 0,
                status_filter: None,
            })
            .await
            .expect("query inbox");

        assert_eq!(items.len(), 1);
        // INB-002: domain + source_app must be populated for smart grouping
        assert_eq!(items[0].domain.as_deref(), Some("github.com"));
        assert_eq!(items[0].source_app.as_deref(), Some("chrome.exe"));
        assert_eq!(items[0].item_type, "link");
    }

    #[tokio::test]
    async fn trash_stats_query_and_empty_purge() {
        use pastecanvas_desktop_lib::application::explorer_service::ExplorerCaptureService;
        use pastecanvas_desktop_lib::application::item_service::ItemService;

        let db = setup().await;
        let dir = tempfile::TempDir::new().expect("temp dir");
        let item_svc = ItemService::new(db.clone());

        // Create two items with assets via the explorer capture path
        let exp =
            ExplorerCaptureService::new(db.clone(), EntityId::new(), dir.path().to_path_buf());
        let p1 = dir.path().join("a.txt");
        std::fs::write(&p1, b"asset one").expect("write a");
        let p2 = dir.path().join("b.txt");
        std::fs::write(&p2, b"asset two").expect("write b");
        let item_a = exp
            .capture_file(&p1.to_string_lossy())
            .await
            .expect("cap a");
        let item_b = exp
            .capture_file(&p2.to_string_lossy())
            .await
            .expect("cap b");

        // Two assets on disk
        let asset_count_before: (i64,) = sqlx::query_as("SELECT COUNT(*) FROM assets")
            .fetch_one(db.pool())
            .await
            .expect("c");
        assert_eq!(asset_count_before.0, 2);

        // Trash item_a only
        item_svc.trash_item(item_a.as_str()).await.expect("trash a");

        // Stats should show 1 trashed item with its asset bytes
        let (count, bytes) = item_svc.trash_stats().await.expect("stats");
        assert_eq!(count, 1);
        assert!(bytes > 0);

        // query_trash returns the soft-deleted item
        let trash = item_svc.query_trash(100).await.expect("query trash");
        assert_eq!(trash.len(), 1);
        assert_eq!(trash[0].id, item_a.to_string());

        // Empty trash: hard-deletes item_a and its now-orphaned asset
        let (deleted, orphans) = item_svc.empty_trash().await.expect("empty");
        assert_eq!(deleted, 1);
        assert_eq!(orphans.len(), 1);

        // item_a is gone, item_b survives
        let remaining: (i64,) = sqlx::query_as("SELECT COUNT(*) FROM items")
            .fetch_one(db.pool())
            .await
            .expect("c");
        assert_eq!(remaining.0, 1);

        // Only item_b's asset remains (orphan was purged)
        let asset_count_after: (i64,) = sqlx::query_as("SELECT COUNT(*) FROM assets")
            .fetch_one(db.pool())
            .await
            .expect("c");
        assert_eq!(asset_count_after.0, 1);
        let _ = item_b;
    }

    #[tokio::test]
    async fn batch_archive_and_trash_are_atomic() {
        let db = setup().await;
        let svc = CaptureService::new(db.clone(), EntityId::new());
        let mut ids = Vec::new();
        for text in ["one", "two", "three"] {
            let r = svc
                .capture_text(CaptureTextRequest {
                    text: text.to_string(),
                    capture_method: CaptureMethod::ManualHotkey,
                    source_app: None,
                    window_title: None,
                })
                .await
                .expect("capture");
            ids.push(r.item_id.to_string());
        }

        let archived =
            pastecanvas_desktop_lib::commands::inbox::batch_archive_in_pool(db.pool(), &ids[..2])
                .await
                .expect("archive");
        assert_eq!(archived, 2);

        let statuses: Vec<(String,)> =
            sqlx::query_as("SELECT inbox_status FROM items WHERE id IN (?, ?) ORDER BY created_at")
                .bind(&ids[0])
                .bind(&ids[1])
                .fetch_all(db.pool())
                .await
                .expect("status");
        assert!(statuses.iter().all(|(s,)| s == "archived"));

        let trashed =
            pastecanvas_desktop_lib::commands::inbox::batch_trash_in_pool(db.pool(), &ids)
                .await
                .expect("trash");
        assert_eq!(trashed, 3);

        let live: (i64,) = sqlx::query_as("SELECT COUNT(*) FROM items WHERE deleted_at IS NULL")
            .fetch_one(db.pool())
            .await
            .expect("live");
        assert_eq!(live.0, 0);
    }
}
