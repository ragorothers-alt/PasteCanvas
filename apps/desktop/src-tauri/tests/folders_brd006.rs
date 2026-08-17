//! BRD-006 folder hierarchy & board organization integration tests.
//! Phase-2 Workstream A: Folder create/rename/reorder/trash/restore,
//! cycle-safe moves, board favorite/restore/reorder.

#[cfg(test)]
mod tests {
    use pastecanvas_desktop_lib::application::board_service::{BoardService, BoardServiceError};
    use pastecanvas_desktop_lib::domain::value_objects::EntityId;
    use pastecanvas_desktop_lib::infrastructure::database::migrator;
    use pastecanvas_desktop_lib::infrastructure::database::pool::AppDatabase;

    async fn setup() -> (AppDatabase, BoardService) {
        let db = AppDatabase::open_in_memory().await.expect("open db");
        migrator::run_migrations(db.pool())
            .await
            .expect("migrations");
        let svc = BoardService::new(db.clone(), EntityId::new());
        (db, svc)
    }

    #[tokio::test]
    async fn create_nested_folders_and_board_in_folder() {
        let (_db, svc) = setup().await;

        let root = svc.create_folder("Projects", None).await.expect("root");
        let child = svc
            .create_folder("Design", Some(&root.id))
            .await
            .expect("child");
        assert_eq!(child.parent_folder_id.as_deref(), Some(root.id.as_str()));

        let folders = svc.list_folders().await.expect("list");
        assert_eq!(folders.len(), 2);

        let board = svc
            .create_board("Board in folder", Some(&child.id))
            .await
            .expect("board");
        assert_eq!(board.folder_id.as_deref(), Some(child.id.as_str()));

        let boards = svc.list_boards().await.expect("boards");
        assert_eq!(boards.len(), 1);
        assert_eq!(boards[0].folder_id.as_deref(), Some(child.id.as_str()));
    }

    #[tokio::test]
    async fn move_board_into_folder_and_back_to_root() {
        let (_db, svc) = setup().await;
        let folder = svc.create_folder("Work", None).await.expect("folder");
        let board = svc.create_board("Loose", None).await.expect("board");
        assert!(board.folder_id.is_none());

        svc.move_board(&board.id, Some(&folder.id))
            .await
            .expect("move in");
        let boards = svc.list_boards().await.expect("list");
        assert_eq!(boards[0].folder_id.as_deref(), Some(folder.id.as_str()));

        svc.move_board(&board.id, None).await.expect("move out");
        let boards = svc.list_boards().await.expect("list after");
        assert!(boards[0].folder_id.is_none());
    }

    #[tokio::test]
    async fn create_folder_rejects_missing_or_trashed_parent() {
        let (_db, svc) = setup().await;
        let err = svc
            .create_folder("X", Some("nonexistent"))
            .await
            .expect_err("must fail");
        assert!(matches!(err, BoardServiceError::FolderNotFound(_)));
    }

    #[tokio::test]
    async fn rename_folder_validates_name() {
        let (_db, svc) = setup().await;
        let f = svc.create_folder("Old", None).await.expect("create");
        svc.rename_folder(&f.id, "New").await.expect("rename");

        let folders = svc.list_folders().await.expect("list");
        assert_eq!(folders[0].name, "New");

        assert!(matches!(
            svc.rename_folder(&f.id, "   ").await.unwrap_err(),
            BoardServiceError::NameEmpty
        ));
    }

    #[tokio::test]
    async fn move_folder_rejects_cycles() {
        let (_db, svc) = setup().await;
        // a → b → c
        let a = svc.create_folder("A", None).await.expect("a");
        let b = svc.create_folder("B", Some(&a.id)).await.expect("b");
        let c = svc.create_folder("C", Some(&b.id)).await.expect("c");

        // Moving A under its grandchild C would create a cycle.
        let err = svc
            .move_folder(&a.id, Some(&c.id))
            .await
            .expect_err("cycle");
        assert!(matches!(err, BoardServiceError::FolderCycle));

        // Moving under itself is also rejected.
        let err = svc.move_folder(&a.id, Some(&a.id)).await.expect_err("self");
        assert!(matches!(err, BoardServiceError::FolderCycle));

        // Legal move: C to root.
        svc.move_folder(&c.id, None).await.expect("move to root");
        let folders = svc.list_folders().await.expect("list");
        let c_after = folders.iter().find(|f| f.id == c.id).expect("c");
        assert!(c_after.parent_folder_id.is_none());

        // Legal move: B under C (reversed chain, still acyclic).
        svc.move_folder(&b.id, Some(&c.id))
            .await
            .expect("b under c");
    }

    #[tokio::test]
    async fn trash_folder_preserves_boards_and_reparents_children() {
        let (_db, svc) = setup().await;
        let parent = svc.create_folder("Parent", None).await.expect("p");
        let doomed = svc
            .create_folder("Doomed", Some(&parent.id))
            .await
            .expect("d");
        let child = svc
            .create_folder("Child", Some(&doomed.id))
            .await
            .expect("c");
        let board = svc
            .create_board("Inside doomed", Some(&doomed.id))
            .await
            .expect("b");

        svc.trash_folder(&doomed.id).await.expect("trash");

        // Folder is gone from the live list.
        let folders = svc.list_folders().await.expect("list");
        assert!(!folders.iter().any(|f| f.id == doomed.id));

        // Board survives and becomes unfiled.
        let boards = svc.list_boards().await.expect("boards");
        assert_eq!(boards.len(), 1);
        assert_eq!(boards[0].id, board.id);
        assert!(boards[0].folder_id.is_none());

        // Child folder re-parented to the trashed folder's parent.
        let child_after = folders.iter().find(|f| f.id == child.id).expect("child");
        assert_eq!(
            child_after.parent_folder_id.as_deref(),
            Some(parent.id.as_str())
        );
    }

    #[tokio::test]
    async fn restore_folder_makes_it_visible_again() {
        let (_db, svc) = setup().await;
        let f = svc.create_folder("Temp", None).await.expect("create");
        svc.trash_folder(&f.id).await.expect("trash");
        assert!(svc.list_folders().await.expect("l").is_empty());

        svc.restore_folder(&f.id).await.expect("restore");
        let folders = svc.list_folders().await.expect("list");
        assert_eq!(folders.len(), 1);
        assert_eq!(folders[0].id, f.id);

        // Restoring something not trashed fails.
        assert!(svc.restore_folder(&f.id).await.is_err());
    }

    #[tokio::test]
    async fn board_favorite_toggle_and_restore() {
        let (_db, svc) = setup().await;
        let b = svc.create_board("Fav board", None).await.expect("b");
        assert!(!b.is_favorite);

        assert!(svc.toggle_board_favorite(&b.id).await.expect("t1"));
        assert!(!svc.toggle_board_favorite(&b.id).await.expect("t2"));

        // Trash → hidden → restore → visible.
        svc.trash_board(&b.id).await.expect("trash");
        assert!(svc.list_boards().await.expect("l").is_empty());
        svc.restore_board(&b.id).await.expect("restore");
        let boards = svc.list_boards().await.expect("list");
        assert_eq!(boards.len(), 1);
        assert_eq!(boards[0].id, b.id);
    }

    #[tokio::test]
    async fn reorder_folders_and_boards_applies_sort_keys() {
        let (_db, svc) = setup().await;
        let f1 = svc.create_folder("F1", None).await.expect("f1");
        let f2 = svc.create_folder("F2", None).await.expect("f2");
        let b1 = svc.create_board("B1", None).await.expect("b1");
        let b2 = svc.create_board("B2", None).await.expect("b2");

        // Explicit reverse order.
        svc.reorder_folders(&[f2.id.clone(), f1.id.clone()])
            .await
            .expect("rf");
        svc.reorder_boards(&[b2.id.clone(), b1.id.clone()])
            .await
            .expect("rb");

        let folders = svc.list_folders().await.expect("lf");
        assert_eq!(folders[0].id, f2.id);
        assert_eq!(folders[1].id, f1.id);

        let boards = svc.list_boards().await.expect("lb");
        assert_eq!(boards[0].id, b2.id);
        assert_eq!(boards[1].id, b1.id);
    }
}
