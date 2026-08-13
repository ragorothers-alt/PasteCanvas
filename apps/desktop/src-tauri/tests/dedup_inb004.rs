//! INB-004 dedup notice integration tests.
//! Verifies duplicate detection returns existing item ID (advisory, save still succeeds).

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

    fn req(text: &str) -> CaptureTextRequest {
        CaptureTextRequest {
            text: text.to_string(),
            capture_method: CaptureMethod::ManualHotkey,
            source_app: None,
            window_title: None,
        }
    }

    #[tokio::test]
    async fn first_capture_is_not_duplicate() {
        let db = setup().await;
        let svc = CaptureService::new(db, EntityId::new());

        let result = svc
            .capture_text(req("unique content one"))
            .await
            .expect("capture");

        assert!(!result.is_duplicate);
        assert!(result.duplicate_of.is_none());
    }

    #[tokio::test]
    async fn identical_text_second_capture_reports_duplicate_of_first() {
        let db = setup().await;
        let svc = CaptureService::new(db, EntityId::new());

        let first = svc.capture_text(req("duplicate me")).await.expect("first");
        let second = svc.capture_text(req("duplicate me")).await.expect("second");

        // Per PRD INB-004: save still succeeds (new item created)
        assert_ne!(first.item_id.to_string(), second.item_id.to_string());
        // But duplicate notice points at the original
        assert!(second.is_duplicate);
        assert_eq!(
            second.duplicate_of.as_ref().map(|id| id.to_string()),
            Some(first.item_id.to_string())
        );
    }

    #[tokio::test]
    async fn line_ending_normalization_detected_as_duplicate() {
        let db = setup().await;
        let svc = CaptureService::new(db, EntityId::new());

        let first = svc.capture_text(req("line1\nline2")).await.expect("first");
        // Same content with CRLF line endings
        let second = svc
            .capture_text(req("line1\r\nline2"))
            .await
            .expect("second");

        assert!(second.is_duplicate);
        assert_eq!(
            second.duplicate_of.as_ref().map(|id| id.to_string()),
            Some(first.item_id.to_string())
        );
    }

    #[tokio::test]
    async fn same_url_different_display_detected_via_normalized_url() {
        let db = setup().await;
        let svc = CaptureService::new(db, EntityId::new());

        // First capture without fragment
        let first = svc
            .capture_text(req("https://example.com/article"))
            .await
            .expect("first");

        // Second capture with fragment — normalizes to same URL
        let second = svc
            .capture_text(req("https://example.com/article#section-2"))
            .await
            .expect("second");

        // Hashes differ (different raw text) but normalized URL matches
        assert!(second.is_duplicate);
        assert_eq!(
            second.duplicate_of.as_ref().map(|id| id.to_string()),
            Some(first.item_id.to_string())
        );
    }

    #[tokio::test]
    async fn soft_deleted_item_not_reported_as_duplicate() {
        let db = setup().await;
        let svc = CaptureService::new(db.clone(), EntityId::new());

        let first = svc
            .capture_text(req("will be deleted"))
            .await
            .expect("first");

        // Soft delete the first item
        sqlx::query("UPDATE items SET deleted_at = 1234567890 WHERE id = ?")
            .bind(first.item_id.to_string())
            .execute(db.pool())
            .await
            .expect("soft delete");

        // Re-capture same content — should NOT be duplicate
        let second = svc
            .capture_text(req("will be deleted"))
            .await
            .expect("second");
        assert!(!second.is_duplicate);
        assert!(second.duplicate_of.is_none());
    }

    #[tokio::test]
    async fn duplicate_points_to_earliest_item() {
        let db = setup().await;
        let svc = CaptureService::new(db, EntityId::new());

        let first = svc.capture_text(req("repeat me")).await.expect("first");
        let second = svc.capture_text(req("repeat me")).await.expect("second");
        let third = svc.capture_text(req("repeat me")).await.expect("third");

        // Both second and third should point to the FIRST (earliest) item
        assert_eq!(
            second.duplicate_of.as_ref().map(|id| id.to_string()),
            Some(first.item_id.to_string())
        );
        assert_eq!(
            third.duplicate_of.as_ref().map(|id| id.to_string()),
            Some(first.item_id.to_string())
        );
    }

    #[tokio::test]
    async fn distinct_content_never_duplicate() {
        let db = setup().await;
        let svc = CaptureService::new(db, EntityId::new());

        let a = svc.capture_text(req("content alpha")).await.expect("a");
        let b = svc.capture_text(req("content beta")).await.expect("b");

        assert!(!a.is_duplicate);
        assert!(!b.is_duplicate);
        assert!(b.duplicate_of.is_none());
    }
}
