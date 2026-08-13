//! CAP-007 Explorer file capture integration tests.
//! Verifies real files are captured into the asset store with correct types.

#[cfg(test)]
mod tests {
    use pastecanvas_desktop_lib::application::board_service::BoardService;
    use pastecanvas_desktop_lib::application::explorer_service::ExplorerCaptureService;
    use pastecanvas_desktop_lib::domain::value_objects::EntityId;
    use pastecanvas_desktop_lib::infrastructure::database::migrator;
    use pastecanvas_desktop_lib::infrastructure::database::pool::AppDatabase;
    use std::io::Write;

    async fn setup() -> (AppDatabase, tempfile::TempDir) {
        let db = AppDatabase::open_in_memory().await.expect("open db");
        migrator::run_migrations(db.pool())
            .await
            .expect("migrations");
        let data_dir = tempfile::TempDir::new().expect("temp dir");
        (db, data_dir)
    }

    fn write_temp(dir: &tempfile::TempDir, name: &str, bytes: &[u8]) -> String {
        let path = dir.path().join(name);
        let mut f = std::fs::File::create(&path).expect("create");
        f.write_all(bytes).expect("write");
        path.to_string_lossy().to_string()
    }

    fn svc(db: &AppDatabase, data_dir: &tempfile::TempDir) -> ExplorerCaptureService {
        ExplorerCaptureService::new(db.clone(), EntityId::new(), data_dir.path().to_path_buf())
    }

    #[tokio::test]
    async fn capture_png_file_detected_as_image() {
        let (db, dir) = setup().await;
        // Minimal PNG magic + IHDR (enough for magic detection)
        let png = [0x89, 0x50, 0x4E, 0x47, 0x0D, 0x0A, 0x1A, 0x0A, 0, 0, 0, 13];
        let path = write_temp(&dir, "pic.png", &png);

        let item_id = svc(&db, &dir).capture_file(&path).await.expect("capture");

        let row: (String, String) =
            sqlx::query_as("SELECT item_type, title FROM items WHERE id = ?")
                .bind(item_id.to_string())
                .fetch_one(db.pool())
                .await
                .expect("item");
        assert_eq!(row.0, "image");
        assert_eq!(row.1, "pic"); // title = filename without extension
    }

    #[tokio::test]
    async fn capture_pdf_file_detected_as_pdf() {
        let (db, dir) = setup().await;
        let path = write_temp(&dir, "doc.pdf", b"%PDF-1.4 fake content");

        let item_id = svc(&db, &dir).capture_file(&path).await.expect("capture");

        let (item_type,): (String,) = sqlx::query_as("SELECT item_type FROM items WHERE id = ?")
            .bind(item_id.to_string())
            .fetch_one(db.pool())
            .await
            .expect("item");
        assert_eq!(item_type, "pdf");
    }

    #[tokio::test]
    async fn capture_generic_file_detected_as_file() {
        let (db, dir) = setup().await;
        let path = write_temp(&dir, "notes.txt", b"just some text");

        let item_id = svc(&db, &dir).capture_file(&path).await.expect("capture");

        let (item_type,): (String,) = sqlx::query_as("SELECT item_type FROM items WHERE id = ?")
            .bind(item_id.to_string())
            .fetch_one(db.pool())
            .await
            .expect("item");
        assert_eq!(item_type, "file");
    }

    #[tokio::test]
    async fn capture_creates_search_document_with_filename() {
        let (db, dir) = setup().await;
        let path = write_temp(&dir, "report.pdf", b"%PDF-1.7 data");

        let item_id = svc(&db, &dir).capture_file(&path).await.expect("capture");

        let (filename,): (String,) =
            sqlx::query_as("SELECT filename FROM search_documents WHERE item_id = ?")
                .bind(item_id.to_string())
                .fetch_one(db.pool())
                .await
                .expect("search doc");
        assert_eq!(filename, "report.pdf");
    }

    #[tokio::test]
    async fn capture_missing_file_errors() {
        let (db, dir) = setup().await;
        let result = svc(&db, &dir).capture_file("/nonexistent/file.xyz").await;
        assert!(result.is_err());
    }

    #[tokio::test]
    async fn capture_sets_explorer_method() {
        let (db, dir) = setup().await;
        let path = write_temp(&dir, "a.txt", b"hello");

        let item_id = svc(&db, &dir).capture_file(&path).await.expect("capture");

        let (method,): (String,) = sqlx::query_as("SELECT capture_method FROM items WHERE id = ?")
            .bind(item_id.to_string())
            .fetch_one(db.pool())
            .await
            .expect("item");
        assert_eq!(method, "explorer");
    }

    #[tokio::test]
    async fn duplicate_file_content_reuses_asset() {
        let (db, dir) = setup().await;
        let s = svc(&db, &dir);
        let p1 = write_temp(&dir, "dup1.txt", b"same content");
        let p2 = write_temp(&dir, "dup2.txt", b"same content");

        s.capture_file(&p1).await.expect("c1");
        s.capture_file(&p2).await.expect("c2");

        // Only one asset row for identical content (content-addressed dedup)
        let (count,): (i64,) = sqlx::query_as("SELECT COUNT(*) FROM assets")
            .fetch_one(db.pool())
            .await
            .expect("count");
        assert_eq!(count, 1);
    }

    #[tokio::test]
    async fn place_on_board_triages_item() {
        let (db, dir) = setup().await;
        let device = EntityId::new();
        let board_svc = BoardService::new(db.clone(), device);
        let board = board_svc.create_board("Target", None).await.expect("board");

        let s = svc(&db, &dir);
        let path = write_temp(&dir, "b.txt", b"board content");
        let item_id = s.capture_file(&path).await.expect("capture");

        s.place_on_board(&board.id, &item_id, 0)
            .await
            .expect("place");

        // Item should now be triaged
        let (status,): (String,) = sqlx::query_as("SELECT inbox_status FROM items WHERE id = ?")
            .bind(item_id.to_string())
            .fetch_one(db.pool())
            .await
            .expect("item");
        assert_eq!(status, "triaged");

        // A board node should exist
        let (node_count,): (i64,) = sqlx::query_as(
            "SELECT COUNT(*) FROM board_nodes WHERE board_id = ? AND deleted_at IS NULL",
        )
        .bind(&board.id)
        .fetch_one(db.pool())
        .await
        .expect("nodes");
        assert_eq!(node_count, 1);
    }

    #[tokio::test]
    async fn last_used_board_returns_most_recent() {
        let (db, dir) = setup().await;
        let device = EntityId::new();
        let board_svc = BoardService::new(db.clone(), device);

        // No boards yet
        let none = svc(&db, &dir).last_used_board_id().await.expect("q");
        assert!(none.is_none());

        let b1 = board_svc.create_board("First", None).await.expect("b1");
        let b2 = board_svc.create_board("Second", None).await.expect("b2");
        // Touch b1 so it becomes most recent
        sqlx::query("UPDATE boards SET updated_at = updated_at + 1000 WHERE id = ?")
            .bind(&b1.id)
            .execute(db.pool())
            .await
            .expect("touch");

        let last = svc(&db, &dir).last_used_board_id().await.expect("q");
        assert_eq!(last, Some(b1.id));
        let _ = b2;
    }

    #[tokio::test]
    async fn batch_capture_multiple_files_mixed_types() {
        let (db, dir) = setup().await;
        let s = svc(&db, &dir);

        // Simulate the capture_files command path: capture each dropped file
        let p_png = write_temp(
            &dir,
            "a.png",
            &[0x89, 0x50, 0x4E, 0x47, 0x0D, 0x0A, 0x1A, 0x0A],
        );
        let p_pdf = write_temp(&dir, "b.pdf", b"%PDF-1.5 x");
        let p_txt = write_temp(&dir, "c.txt", b"plain");

        let mut ids = Vec::new();
        for p in [&p_png, &p_pdf, &p_txt] {
            ids.push(s.capture_file(p).await.expect("capture"));
        }
        assert_eq!(ids.len(), 3);

        // Verify all three items exist with distinct types
        let rows: Vec<(String,)> = sqlx::query_as(
            "SELECT item_type FROM items WHERE deleted_at IS NULL ORDER BY created_at",
        )
        .fetch_all(db.pool())
        .await
        .expect("items");
        let types: Vec<&str> = rows.iter().map(|(t,)| t.as_str()).collect();
        assert_eq!(types, vec!["image", "pdf", "file"]);
    }
}
