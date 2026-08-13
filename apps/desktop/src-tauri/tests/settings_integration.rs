//! Integration tests for Settings and Backup services.

#[cfg(test)]
mod tests {
    use pastecanvas_desktop_lib::application::settings_service::SettingsService;
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

        // Verify key defaults exist
        let auto = svc
            .get_setting("capture.automatic_enabled")
            .await
            .expect("get");
        assert_eq!(auto.as_deref(), Some("false"));

        let tray = svc.get_setting("ui.close_to_tray").await.expect("get");
        assert_eq!(tray.as_deref(), Some("true"));

        let paths = svc
            .get_setting("privacy.store_file_paths")
            .await
            .expect("get");
        assert_eq!(paths.as_deref(), Some("false"));
    }

    #[tokio::test]
    async fn settings_set_and_get() {
        let db = setup().await;
        let svc = SettingsService::new(db.clone());

        svc.set_setting("test.key", "\"hello\"").await.expect("set");
        let val = svc.get_setting("test.key").await.expect("get");
        assert_eq!(val.as_deref(), Some("\"hello\""));

        // Update existing
        svc.set_setting("test.key", "\"world\"").await.expect("set");
        let val = svc.get_setting("test.key").await.expect("get");
        assert_eq!(val.as_deref(), Some("\"world\""));
    }

    #[tokio::test]
    async fn settings_get_nonexistent_returns_none() {
        let db = setup().await;
        let svc = SettingsService::new(db.clone());

        let val = svc.get_setting("nonexistent.key").await.expect("get");
        assert!(val.is_none());
    }

    #[tokio::test]
    async fn exclusions_initialized_with_builtins() {
        let db = setup().await;
        let svc = SettingsService::new(db.clone());
        svc.initialize_defaults().await.expect("init");

        let exclusions = svc.list_exclusions().await.expect("list");
        assert!(exclusions.len() >= 13);
        assert!(exclusions.iter().all(|e| e.source == "built_in"));
        assert!(exclusions.iter().all(|e| e.enabled));

        // Verify specific known exclusions
        let has_keepass = exclusions.iter().any(|e| e.match_value == "KeePass.exe");
        let has_1password = exclusions.iter().any(|e| e.match_value == "1Password.exe");
        assert!(has_keepass);
        assert!(has_1password);
    }

    #[tokio::test]
    async fn exclusions_add_user_exclusion() {
        let db = setup().await;
        let svc = SettingsService::new(db.clone());

        let exc = svc
            .add_exclusion("process_name", "MyApp.exe")
            .await
            .expect("add");
        assert_eq!(exc.source, "user");
        assert!(exc.enabled);
        assert_eq!(exc.match_value, "MyApp.exe");
    }

    #[tokio::test]
    async fn exclusions_toggle() {
        let db = setup().await;
        let svc = SettingsService::new(db.clone());

        let exc = svc
            .add_exclusion("process_name", "Toggle.exe")
            .await
            .expect("add");
        assert!(exc.enabled);

        let new_state = svc.toggle_exclusion(&exc.id).await.expect("toggle");
        assert!(!new_state);

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
            .add_exclusion("process_name", "Temp.exe")
            .await
            .expect("add");

        // Remove it
        svc.remove_exclusion(&exc.id).await.expect("remove");

        // Verify gone
        let exclusions = svc.list_exclusions().await.expect("list");
        assert!(!exclusions.iter().any(|e| e.match_value == "Temp.exe"));

        // Built-in exclusions still exist
        assert!(exclusions.iter().any(|e| e.match_value == "KeePass.exe"));
    }

    #[tokio::test]
    async fn process_exclusion_check() {
        let db = setup().await;
        let svc = SettingsService::new(db.clone());
        svc.initialize_defaults().await.expect("init");

        assert!(svc.is_process_excluded("KeePass.exe").await.expect("check"));
        assert!(
            svc.is_process_excluded("1Password.exe")
                .await
                .expect("check")
        );
        assert!(!svc.is_process_excluded("chrome.exe").await.expect("check"));
        assert!(!svc.is_process_excluded("notepad.exe").await.expect("check"));
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

    #[tokio::test]
    async fn hotkey_update_invalid_action_fails() {
        let db = setup().await;
        let svc = SettingsService::new(db.clone());
        svc.initialize_defaults().await.expect("init");

        let result = svc.update_hotkey("nonexistent_action", "Ctrl+X").await;
        assert!(result.is_err());
    }
}
