//! DAT-002 backup restore validation + pending-restore apply tests.

#[cfg(test)]
mod tests {
    use pastecanvas_desktop_lib::application::backup_service::BackupService;
    use pastecanvas_desktop_lib::infrastructure::database::migrator;
    use pastecanvas_desktop_lib::infrastructure::database::pool::AppDatabase;

    async fn setup() -> AppDatabase {
        let db = AppDatabase::open_in_memory().await.expect("open db");
        migrator::run_migrations(db.pool())
            .await
            .expect("migrations");
        db
    }

    fn make_backup(dir: &tempfile::TempDir, schema_version: i64) -> std::path::PathBuf {
        let backup = dir.path().join("backup_test");
        std::fs::create_dir_all(&backup).expect("backup dir");
        // Dummy DB file
        std::fs::write(backup.join("pastecanvas.sqlite3"), b"dummy-db").expect("db file");
        // Manifest
        let manifest = format!(
            r#"{{"version":1,"schemaVersion":{schema_version},"itemCount":3,"assetCount":2}}"#
        );
        std::fs::write(backup.join("manifest.json"), manifest).expect("manifest");
        backup
    }

    #[tokio::test]
    async fn validate_backup_accepts_compatible_schema() {
        let db = setup().await;
        let dir = tempfile::TempDir::new().expect("temp");
        let svc = BackupService::new(db, dir.path().to_path_buf());
        let backup = make_backup(&dir, 1);

        let v = svc.validate_backup(&backup);
        assert!(v.valid);
        assert_eq!(v.schema_version, 1);
        assert_eq!(v.item_count, 3);
        assert_eq!(v.asset_count, 2);
    }

    #[tokio::test]
    async fn validate_backup_rejects_incompatible_schema() {
        let db = setup().await;
        let dir = tempfile::TempDir::new().expect("temp");
        let svc = BackupService::new(db, dir.path().to_path_buf());
        let backup = make_backup(&dir, 99);

        let v = svc.validate_backup(&backup);
        assert!(!v.valid);
        assert!(v.reason.is_some());
    }

    #[tokio::test]
    async fn validate_backup_rejects_missing_manifest() {
        let db = setup().await;
        let dir = tempfile::TempDir::new().expect("temp");
        let svc = BackupService::new(db, dir.path().to_path_buf());
        let backup = dir.path().join("no_manifest");
        std::fs::create_dir_all(&backup).expect("dir");
        std::fs::write(backup.join("pastecanvas.sqlite3"), b"x").expect("db");

        let v = svc.validate_backup(&backup);
        assert!(!v.valid);
    }

    #[tokio::test]
    async fn prepare_restore_writes_marker_and_apply_restores() {
        let db = setup().await;
        let dir = tempfile::TempDir::new().expect("temp");
        let svc = BackupService::new(db, dir.path().to_path_buf());
        let backup = make_backup(&dir, 1);

        // Prepare restore writes the pending marker
        let v = svc.prepare_restore(&backup).expect("prepare");
        assert!(v.valid);
        assert!(dir.path().join("restore_pending.json").exists());

        // Apply pending restore copies backup DB over main DB + merges assets
        let applied = BackupService::apply_pending_restore(dir.path()).expect("apply");
        assert!(applied);
        // Marker removed after apply
        assert!(!dir.path().join("restore_pending.json").exists());
        // Main DB replaced with backup content
        let main_db = std::fs::read(dir.path().join("pastecanvas.sqlite3")).expect("read main");
        assert_eq!(main_db, b"dummy-db");
    }

    #[tokio::test]
    async fn apply_without_marker_is_noop() {
        let dir = tempfile::TempDir::new().expect("temp");
        let applied = BackupService::apply_pending_restore(dir.path()).expect("apply");
        assert!(!applied);
    }

    #[tokio::test]
    async fn validate_backup_rejects_checksum_mismatch() {
        let db = setup().await;
        let dir = tempfile::TempDir::new().expect("temp");
        let svc = BackupService::new(db, dir.path().to_path_buf());
        let backup = dir.path().join("backup_bad_hash");
        std::fs::create_dir_all(&backup).expect("dir");
        std::fs::write(backup.join("pastecanvas.sqlite3"), b"dummy-db").expect("db");
        std::fs::write(
            backup.join("manifest.json"),
            r#"{"version":1,"schemaVersion":1,"itemCount":1,"assetCount":0,"dbSha256":"deadbeef"}"#,
        )
        .expect("manifest");

        let v = svc.validate_backup(&backup);
        assert!(!v.valid);
        assert!(
            v.reason
                .as_deref()
                .unwrap_or("")
                .contains("checksum mismatch"),
            "reason = {:?}",
            v.reason
        );
    }

    #[tokio::test]
    async fn apply_missing_backup_db_fails_and_keeps_marker() {
        let dir = tempfile::TempDir::new().expect("temp");
        let marker = serde_json::json!({
            "backupPath": dir.path().join("does-not-exist").display().to_string()
        });
        std::fs::write(
            dir.path().join("restore_pending.json"),
            serde_json::to_string(&marker).unwrap(),
        )
        .expect("marker");

        let err = BackupService::apply_pending_restore(dir.path()).expect_err("must fail");
        assert!(err.to_string().contains("not found"));
        assert!(
            dir.path().join("restore_pending.json").exists(),
            "marker must remain so the user can retry"
        );
    }
}
