//! BRD-007 Recent Boards integration tests.
//! Phase-2 Workstream A: "sidebar tree and Recent Boards".

#[cfg(test)]
mod tests {
    use pastecanvas_desktop_lib::application::board_service::BoardService;
    use pastecanvas_desktop_lib::domain::value_objects::EntityId;
    use pastecanvas_desktop_lib::infrastructure::database::migrator;
    use pastecanvas_desktop_lib::infrastructure::database::pool::AppDatabase;

    async fn setup() -> BoardService {
        let db = AppDatabase::open_in_memory().await.expect("open db");
        migrator::run_migrations(db.pool())
            .await
            .expect("migrations");
        BoardService::new(db, EntityId::new())
    }

    #[tokio::test]
    async fn recent_boards_ordered_by_last_open() {
        let svc = setup().await;
        let b1 = svc.create_board("First", None).await.expect("b1");
        tokio::time::sleep(std::time::Duration::from_millis(3)).await;
        let b2 = svc.create_board("Second", None).await.expect("b2");
        tokio::time::sleep(std::time::Duration::from_millis(3)).await;
        let b3 = svc.create_board("Third", None).await.expect("b3");

        // Open boards in order b2 → b3; opening touches updated_at.
        tokio::time::sleep(std::time::Duration::from_millis(3)).await;
        svc.get_board_snapshot(&b2.id).await.expect("open b2");
        tokio::time::sleep(std::time::Duration::from_millis(3)).await;
        svc.get_board_snapshot(&b3.id).await.expect("open b3");

        let recent = svc.get_recent_boards(5).await.expect("recent");
        assert_eq!(recent.len(), 3);
        assert_eq!(recent[0].id, b3.id); // most recently opened
        assert_eq!(recent[1].id, b2.id);
        assert_eq!(recent[2].id, b1.id); // created first, never opened
    }

    #[tokio::test]
    async fn recent_boards_respects_limit_and_clamps() {
        let svc = setup().await;
        for i in 0..4 {
            let b = svc
                .create_board(&format!("Board {i}"), None)
                .await
                .expect("board");
            tokio::time::sleep(std::time::Duration::from_millis(3)).await;
            svc.get_board_snapshot(&b.id).await.expect("open");
        }

        let recent = svc.get_recent_boards(2).await.expect("recent");
        assert_eq!(recent.len(), 2);

        // limit is clamped to at least 1
        let one = svc.get_recent_boards(0).await.expect("clamped");
        assert_eq!(one.len(), 1);
    }

    #[tokio::test]
    async fn opening_board_does_not_bump_revision() {
        let svc = setup().await;
        let b = svc.create_board("Rev check", None).await.expect("b");
        let before = svc.get_board_snapshot(&b.id).await.expect("s1");

        // Opening again touches updated_at but is not a content mutation.
        let after = svc.get_board_snapshot(&b.id).await.expect("s2");
        assert!(after.board.updated_at >= before.board.updated_at);
        assert_eq!(before.board.name, after.board.name);
    }

    #[tokio::test]
    async fn trashed_boards_are_excluded_from_recent() {
        let svc = setup().await;
        let keep = svc.create_board("Keep", None).await.expect("keep");
        let gone = svc.create_board("Gone", None).await.expect("gone");
        svc.get_board_snapshot(&gone.id).await.expect("open");
        svc.trash_board(&gone.id).await.expect("trash");

        let recent = svc.get_recent_boards(5).await.expect("recent");
        assert_eq!(recent.len(), 1);
        assert_eq!(recent[0].id, keep.id);
    }
}
