//! Integration tests for CaptureService and SessionService.

#[cfg(test)]
mod tests {
    use pastecanvas_desktop_lib::application::capture_service::{
        CaptureService, CaptureTextRequest,
    };
    use pastecanvas_desktop_lib::application::session_service::SessionService;
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
    async fn capture_text_creates_item_and_search_doc() {
        let db = setup().await;
        let svc = CaptureService::new(db.clone(), EntityId::new());

        let result = svc
            .capture_text(CaptureTextRequest {
                text: "Hello world test".to_string(),
                capture_method: CaptureMethod::ManualHotkey,
                source_app: Some("test.exe".to_string()),
                window_title: None,
            })
            .await
            .expect("capture");

        assert!(!result.is_duplicate);

        // Verify item exists
        let item: Option<(String, String)> =
            sqlx::query_as("SELECT id, plain_text FROM items WHERE id = ?")
                .bind(result.item_id.as_str())
                .fetch_optional(db.pool())
                .await
                .expect("query");
        assert!(item.is_some());
        assert_eq!(item.unwrap().1, "Hello world test");

        // Verify search document exists
        let doc: Option<(String,)> =
            sqlx::query_as("SELECT body FROM search_documents WHERE item_id = ?")
                .bind(result.item_id.as_str())
                .fetch_optional(db.pool())
                .await
                .expect("query");
        assert!(doc.is_some());
        assert!(doc.unwrap().0.contains("Hello world"));
    }

    #[tokio::test]
    async fn capture_url_classifies_as_link() {
        let db = setup().await;
        let svc = CaptureService::new(db.clone(), EntityId::new());

        let result = svc
            .capture_text(CaptureTextRequest {
                text: "https://example.com/page".to_string(),
                capture_method: CaptureMethod::ManualHotkey,
                source_app: None,
                window_title: None,
            })
            .await
            .expect("capture");

        let item: Option<(String, Option<String>)> =
            sqlx::query_as("SELECT item_type, normalized_url FROM items WHERE id = ?")
                .bind(result.item_id.as_str())
                .fetch_optional(db.pool())
                .await
                .expect("query");

        let (item_type, url) = item.unwrap();
        assert_eq!(item_type, "link");
        assert_eq!(url.as_deref(), Some("https://example.com/page"));
    }

    #[tokio::test]
    async fn capture_empty_text_fails() {
        let db = setup().await;
        let svc = CaptureService::new(db.clone(), EntityId::new());

        let result = svc
            .capture_text(CaptureTextRequest {
                text: "".to_string(),
                capture_method: CaptureMethod::ManualHotkey,
                source_app: None,
                window_title: None,
            })
            .await;

        assert!(result.is_err());
    }

    #[tokio::test]
    async fn capture_oversized_text_fails() {
        let db = setup().await;
        let svc = CaptureService::new(db.clone(), EntityId::new());

        let result = svc
            .capture_text(CaptureTextRequest {
                text: "x".repeat(2_000_000),
                capture_method: CaptureMethod::ManualHotkey,
                source_app: None,
                window_title: None,
            })
            .await;

        assert!(result.is_err());
    }

    #[tokio::test]
    async fn duplicate_detection_advisory() {
        let db = setup().await;
        let svc = CaptureService::new(db.clone(), EntityId::new());

        let req = CaptureTextRequest {
            text: "duplicate content".to_string(),
            capture_method: CaptureMethod::ManualHotkey,
            source_app: None,
            window_title: None,
        };

        let r1 = svc.capture_text(req.clone()).await.expect("first");
        assert!(!r1.is_duplicate);

        let r2 = svc.capture_text(req).await.expect("second");
        assert!(r2.is_duplicate); // Advisory only, still saved
    }

    #[tokio::test]
    async fn session_lifecycle() {
        let db = setup().await;
        let svc = SessionService::new(db.clone(), EntityId::new());

        // No active session initially
        let current = svc.get_current_session().await.expect("get");
        assert!(current.is_none());

        // Start session
        let session = svc
            .start_session(Some("Test Session"), None)
            .await
            .expect("start");
        assert_eq!(session.state, "active");
        assert_eq!(session.name.as_deref(), Some("Test Session"));

        // Cannot start another
        let result = svc.start_session(None, None).await;
        assert!(result.is_err());

        // Pause
        let paused = svc.pause_session().await.expect("pause");
        assert_eq!(paused.state, "paused");

        // Cannot pause again
        let result = svc.pause_session().await;
        assert!(result.is_err());

        // Resume
        let resumed = svc.resume_session().await.expect("resume");
        assert_eq!(resumed.state, "active");

        // Finish
        let finished = svc.finish_session().await.expect("finish");
        assert_eq!(finished.state, "finished");

        // No active session after finish
        let current = svc.get_current_session().await.expect("get");
        assert!(current.is_none());
    }

    #[tokio::test]
    async fn capture_with_session_links_item() {
        let db = setup().await;
        let session_svc = SessionService::new(db.clone(), EntityId::new());
        let capture_svc = CaptureService::new(db.clone(), EntityId::new());

        // Start session
        let session = session_svc.start_session(None, None).await.expect("start");

        // CAP-003: session-linked capture stores capture_session_id on the item
        let result = capture_svc
            .capture_text_with_session(
                CaptureTextRequest {
                    text: "session item".to_string(),
                    capture_method: CaptureMethod::CaptureSession,
                    source_app: None,
                    window_title: None,
                },
                Some(EntityId::from_string(session.id.clone())),
            )
            .await
            .expect("capture");

        let row: Option<(String, Option<String>)> =
            sqlx::query_as("SELECT id, capture_session_id FROM items WHERE id = ?")
                .bind(result.item_id.as_str())
                .fetch_optional(db.pool())
                .await
                .expect("query");
        let (id, linked_session) = row.expect("item exists");
        assert_eq!(id, result.item_id.to_string());
        assert_eq!(
            linked_session.as_deref(),
            Some(session.id.as_str()),
            "item must link to the capture session (INB-002 session grouping)"
        );

        // Session item count reflects the linked capture
        let current = session_svc
            .get_current_session()
            .await
            .expect("get")
            .expect("session");
        assert_eq!(current.item_count, 1);

        // Finish session
        let finished = session_svc.finish_session().await.expect("finish");
        assert_eq!(finished.state, "finished");
    }

    #[tokio::test]
    async fn capture_png_stores_image_item_with_asset() {
        let db = setup().await;
        let svc = CaptureService::new(db.clone(), EntityId::new());
        let data_dir = std::env::temp_dir().join(format!("pc-cap-png-{}", uuid::Uuid::now_v7()));
        // Production app_data_dir always exists; create it here for the test.
        std::fs::create_dir_all(&data_dir).expect("data dir");

        // Minimal valid 1x1 RGBA PNG (encoded via the shared thumbnail encoder)
        let png =
            pastecanvas_desktop_lib::infrastructure::thumbnail::encode_png_rgba(&[9; 4], 1, 1)
                .expect("encode");

        let result = svc
            .capture_png(&data_dir, png.clone(), None, CaptureMethod::ManualHotkey)
            .await
            .expect("capture png");

        // Item stored as image
        let row: (String, Option<String>) =
            sqlx::query_as("SELECT item_type, title FROM items WHERE id = ?")
                .bind(result.item_id.as_str())
                .fetch_one(db.pool())
                .await
                .expect("item");
        assert_eq!(row.0, "image");

        // Asset stored and linked
        let asset: (String,) = sqlx::query_as(
            "SELECT a.relative_path FROM assets a
             JOIN item_assets ia ON ia.asset_id = a.id
             WHERE ia.item_id = ?",
        )
        .bind(result.item_id.as_str())
        .fetch_one(db.pool())
        .await
        .expect("asset");
        assert!(asset.0.starts_with("assets/"));

        // Searchable
        let doc: (String,) = sqlx::query_as("SELECT title FROM search_documents WHERE item_id = ?")
            .bind(result.item_id.as_str())
            .fetch_one(db.pool())
            .await
            .expect("doc");
        assert_eq!(doc.0, "Clipboard Image");

        // Dedup: same bytes again → advisory duplicate, still saved
        let again = svc
            .capture_png(&data_dir, png, None, CaptureMethod::ManualHotkey)
            .await
            .expect("capture png again");
        assert!(again.is_duplicate);
        assert_eq!(
            again.duplicate_of.map(|e| e.to_string()),
            Some(result.item_id.to_string())
        );

        let _ = std::fs::remove_dir_all(&data_dir);
    }

    #[tokio::test]
    async fn idle_session_auto_ends_after_timeout() {
        let db = setup().await;
        let svc = SessionService::new(db.clone(), EntityId::new());

        // Start with a 1ms idle timeout
        let session = svc.start_session(None, Some(1)).await.expect("start");
        assert_eq!(session.state, "active");

        // Not idle yet: use the session's own start timestamp as reference
        // (updated_at == started_at at creation) — deterministic on slow machines.
        let ended = svc
            .auto_end_idle_sessions(session.started_at)
            .await
            .expect("auto");
        assert_eq!(ended, 0);

        // After the timeout has elapsed → auto-end
        let later = session.started_at + 1000;
        let ended = svc.auto_end_idle_sessions(later).await.expect("auto");
        assert_eq!(ended, 1);

        // Session is now finished
        let current = svc.get_current_session().await.expect("get");
        assert!(current.is_none());
    }

    #[tokio::test]
    async fn system_suspend_pauses_active_session_and_never_auto_resumes() {
        let db = setup().await;
        let svc = SessionService::new(db.clone(), EntityId::new());

        let session = svc.start_session(None, None).await.expect("start");
        assert_eq!(session.state, "active");

        // WIN-004: system suspend pauses the active session
        let paused = svc.handle_system_suspend().await.expect("suspend");
        assert_eq!(paused, 1);

        let current = svc
            .get_current_session()
            .await
            .expect("get")
            .expect("session");
        assert_eq!(current.state, "paused");

        // Second suspend is a no-op (already paused, never double-counts)
        let paused_again = svc.handle_system_suspend().await.expect("suspend2");
        assert_eq!(paused_again, 0);

        // Privacy-by-default: wake does NOT auto-resume; explicit resume required
        let still = svc
            .get_current_session()
            .await
            .expect("get")
            .expect("session");
        assert_eq!(still.state, "paused");

        let resumed = svc.resume_session().await.expect("resume");
        assert_eq!(resumed.state, "active");
    }

    #[tokio::test]
    async fn session_without_timeout_never_auto_ends() {
        let db = setup().await;
        let svc = SessionService::new(db.clone(), EntityId::new());

        // Start with no idle timeout (disabled)
        let _session = svc.start_session(None, None).await.expect("start");

        // Even far in the future, no auto-end because timeout is None
        let far = chrono::Utc::now().timestamp_millis() + 10_000_000;
        let ended = svc.auto_end_idle_sessions(far).await.expect("auto");
        assert_eq!(ended, 0);

        // Still active
        let current = svc.get_current_session().await.expect("get");
        assert!(current.is_some());
    }
}
