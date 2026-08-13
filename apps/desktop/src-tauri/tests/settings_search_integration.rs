//! Integration tests for SettingsService and search functionality.

#[cfg(test)]
mod tests {
    use pastecanvas_desktop_lib::application::capture_service::{
        CaptureService, CaptureTextRequest,
    };
    use pastecanvas_desktop_lib::application::settings_service::SettingsService;
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

    // --- Settings Tests ---

    #[tokio::test]
    async fn settings_initialize_defaults() {
        let db = setup().await;
        let svc = SettingsService::new(db.clone());

        svc.initialize_defaults().await.expect("init");

        // Verify default settings exist
        let auto_capture = svc
            .get_setting("capture.automatic_enabled")
            .await
            .expect("get");
        assert_eq!(auto_capture.as_deref(), Some("false"));

        let close_to_tray = svc.get_setting("ui.close_to_tray").await.expect("get");
        assert_eq!(close_to_tray.as_deref(), Some("true"));

        let store_paths = svc
            .get_setting("privacy.store_file_paths")
            .await
            .expect("get");
        assert_eq!(store_paths.as_deref(), Some("false"));
    }

    #[tokio::test]
    async fn settings_set_and_get() {
        let db = setup().await;
        let svc = SettingsService::new(db.clone());

        svc.set_setting("test.key", "\"hello\"").await.expect("set");
        let value = svc.get_setting("test.key").await.expect("get");
        assert_eq!(value.as_deref(), Some("\"hello\""));

        // Update existing
        svc.set_setting("test.key", "\"world\"").await.expect("set");
        let value = svc.get_setting("test.key").await.expect("get");
        assert_eq!(value.as_deref(), Some("\"world\""));
    }

    #[tokio::test]
    async fn exclusions_built_in_initialized() {
        let db = setup().await;
        let svc = SettingsService::new(db.clone());

        svc.initialize_defaults().await.expect("init");

        let exclusions = svc.list_exclusions().await.expect("list");
        // Should have 13 built-in password manager exclusions
        assert!(exclusions.len() >= 13);
        assert!(exclusions.iter().all(|e| e.source == "built_in"));
        assert!(exclusions.iter().all(|e| e.enabled));

        // Verify specific known exclusion
        let has_keepass = exclusions.iter().any(|e| e.match_value == "KeePass.exe");
        assert!(has_keepass);
    }

    #[tokio::test]
    async fn exclusions_add_and_toggle() {
        let db = setup().await;
        let svc = SettingsService::new(db.clone());

        // Add user exclusion
        let exc = svc
            .add_exclusion("process_name", "MySecretApp.exe")
            .await
            .expect("add");
        assert_eq!(exc.source, "user");
        assert!(exc.enabled);

        // Toggle it off
        let new_state = svc.toggle_exclusion(&exc.id).await.expect("toggle");
        assert!(!new_state);

        // Toggle back on
        let new_state = svc.toggle_exclusion(&exc.id).await.expect("toggle");
        assert!(new_state);
    }

    #[tokio::test]
    async fn exclusions_remove_user_only() {
        let db = setup().await;
        let svc = SettingsService::new(db.clone());
        svc.initialize_defaults().await.expect("init");

        // Add user exclusion
        let exc = svc
            .add_exclusion("process_name", "TempApp.exe")
            .await
            .expect("add");

        // Remove it
        svc.remove_exclusion(&exc.id).await.expect("remove");

        // Verify it's gone
        let exclusions = svc.list_exclusions().await.expect("list");
        assert!(!exclusions.iter().any(|e| e.match_value == "TempApp.exe"));
    }

    #[tokio::test]
    async fn process_exclusion_check() {
        let db = setup().await;
        let svc = SettingsService::new(db.clone());
        svc.initialize_defaults().await.expect("init");

        // KeePass should be excluded
        assert!(svc.is_process_excluded("KeePass.exe").await.expect("check"));

        // Random app should not be excluded
        assert!(!svc.is_process_excluded("chrome.exe").await.expect("check"));
    }

    #[tokio::test]
    async fn hotkeys_initialized() {
        let db = setup().await;
        let svc = SettingsService::new(db.clone());
        svc.initialize_defaults().await.expect("init");

        let hotkeys = svc.list_hotkeys().await.expect("list");
        assert!(hotkeys.len() >= 4);

        let capture = hotkeys.iter().find(|h| h.action == "capture_clipboard");
        assert!(capture.is_some());
        assert_eq!(capture.unwrap().accelerator, "CommandOrControl+Shift+V");
    }

    #[tokio::test]
    async fn hotkey_update() {
        let db = setup().await;
        let svc = SettingsService::new(db.clone());
        svc.initialize_defaults().await.expect("init");

        svc.update_hotkey("capture_clipboard", "Ctrl+Alt+C")
            .await
            .expect("update");

        let hotkeys = svc.list_hotkeys().await.expect("list");
        let capture = hotkeys.iter().find(|h| h.action == "capture_clipboard");
        assert_eq!(capture.unwrap().accelerator, "Ctrl+Alt+C");
    }

    // --- Search Integration Tests ---

    #[tokio::test]
    async fn search_finds_captured_items() {
        let db = setup().await;
        let capture_svc = CaptureService::new(db.clone(), EntityId::new());

        // Capture some items
        capture_svc
            .capture_text(CaptureTextRequest {
                text: "Rust programming language".to_string(),
                capture_method: CaptureMethod::ManualHotkey,
                source_app: None,
                window_title: None,
            })
            .await
            .expect("capture 1");

        capture_svc
            .capture_text(CaptureTextRequest {
                text: "TypeScript frontend development".to_string(),
                capture_method: CaptureMethod::ManualHotkey,
                source_app: None,
                window_title: None,
            })
            .await
            .expect("capture 2");

        // Search using FTS5
        let results: Vec<(String, String)> = sqlx::query_as(
            "SELECT sd.item_id, sd.body FROM search_fts_unicode
             JOIN search_documents sd ON sd.rowid = search_fts_unicode.rowid
             WHERE search_fts_unicode MATCH '\"Rust\"'",
        )
        .fetch_all(db.pool())
        .await
        .expect("search");

        assert_eq!(results.len(), 1);
        assert!(results[0].1.contains("Rust programming"));
    }

    #[tokio::test]
    async fn search_trigram_finds_substrings() {
        let db = setup().await;
        let capture_svc = CaptureService::new(db.clone(), EntityId::new());

        capture_svc
            .capture_text(CaptureTextRequest {
                text: " PasteCanvas application".to_string(),
                capture_method: CaptureMethod::ManualHotkey,
                source_app: None,
                window_title: None,
            })
            .await
            .expect("capture");

        // Trigram search for substring
        let results: Vec<(String,)> = sqlx::query_as(
            "SELECT sd.item_id FROM search_fts_trigram
             JOIN search_documents sd ON sd.rowid = search_fts_trigram.rowid
             WHERE search_fts_trigram MATCH '\"Canvas\"'",
        )
        .fetch_all(db.pool())
        .await
        .expect("trigram search");

        assert_eq!(results.len(), 1);
    }

    #[tokio::test]
    async fn search_respects_soft_delete() {
        let db = setup().await;
        let capture_svc = CaptureService::new(db.clone(), EntityId::new());

        let result = capture_svc
            .capture_text(CaptureTextRequest {
                text: "delete me later".to_string(),
                capture_method: CaptureMethod::ManualHotkey,
                source_app: None,
                window_title: None,
            })
            .await
            .expect("capture");

        // Soft delete the item
        let now = chrono::Utc::now().timestamp_millis();
        sqlx::query("UPDATE items SET deleted_at = ? WHERE id = ?")
            .bind(now)
            .bind(result.item_id.as_str())
            .execute(db.pool())
            .await
            .expect("delete");

        // Search should not find it (joined with items WHERE deleted_at IS NULL)
        let results: Vec<(String,)> = sqlx::query_as(
            "SELECT sd.item_id FROM search_fts_unicode
             JOIN search_documents sd ON sd.rowid = search_fts_unicode.rowid
             JOIN items i ON i.id = sd.item_id AND i.deleted_at IS NULL
             WHERE search_fts_unicode MATCH '\"delete\"'",
        )
        .fetch_all(db.pool())
        .await
        .expect("search");

        assert_eq!(results.len(), 0);
    }
}
