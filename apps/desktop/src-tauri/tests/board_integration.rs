//! Integration tests for BoardService.

#[cfg(test)]
mod tests {
    use pastecanvas_desktop_lib::application::board_service::BoardService;
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
    async fn create_and_list_boards() {
        let (_, svc) = setup().await;

        let board = svc.create_board("Test Board", None).await.expect("create");
        assert_eq!(board.name, "Test Board");
        assert_eq!(board.node_count, 0);

        let boards = svc.list_boards().await.expect("list");
        assert_eq!(boards.len(), 1);
        assert_eq!(boards[0].name, "Test Board");
    }

    #[tokio::test]
    async fn create_board_empty_name_fails() {
        let (_, svc) = setup().await;
        let result = svc.create_board("", None).await;
        assert!(result.is_err());
    }

    #[tokio::test]
    async fn rename_board() {
        let (_, svc) = setup().await;
        let board = svc.create_board("Original", None).await.expect("create");
        svc.rename_board(&board.id, "Renamed")
            .await
            .expect("rename");

        let boards = svc.list_boards().await.expect("list");
        assert_eq!(boards[0].name, "Renamed");
    }

    #[tokio::test]
    async fn trash_board() {
        let (_, svc) = setup().await;
        let board = svc.create_board("To Trash", None).await.expect("create");
        svc.trash_board(&board.id).await.expect("trash");

        let boards = svc.list_boards().await.expect("list");
        assert_eq!(boards.len(), 0);
    }

    #[tokio::test]
    async fn place_item_and_get_snapshot() {
        let (db, svc) = setup().await;

        // Create a board
        let board = svc.create_board("Canvas", None).await.expect("create");

        // Insert an item directly
        let item_id = EntityId::new();
        let now = chrono::Utc::now().timestamp_millis();
        sqlx::query(
            "INSERT INTO items (id, item_type, title, inbox_status, is_favorite, is_pinned,
              capture_method, created_at, updated_at, revision, origin_device_id, last_modified_device_id, sync_state)
             VALUES (?, 'plain_text', 'Test Item', 'new', 0, 0, 'manual_create', ?, ?, 1, 'dev', 'dev', 'local_only')",
        )
        .bind(item_id.as_str())
        .bind(now)
        .bind(now)
        .execute(db.pool())
        .await
        .expect("insert item");

        // Place item on board
        let node_id = svc
            .place_item_on_board(&board.id, item_id.as_str(), 100.0, 200.0)
            .await
            .expect("place");

        // Get snapshot
        let snapshot = svc.get_board_snapshot(&board.id).await.expect("snapshot");
        assert_eq!(snapshot.board.name, "Canvas");
        assert_eq!(snapshot.nodes.len(), 1);
        assert_eq!(snapshot.nodes[0].id, node_id);
        assert_eq!(snapshot.nodes[0].x, 100.0);
        assert_eq!(snapshot.nodes[0].y, 200.0);
        assert_eq!(snapshot.nodes[0].item_title.as_deref(), Some("Test Item"));
    }

    #[tokio::test]
    async fn move_nodes_updates_positions() {
        let (db, svc) = setup().await;
        let board = svc.create_board("Move Test", None).await.expect("create");

        let item_id = EntityId::new();
        let now = chrono::Utc::now().timestamp_millis();
        sqlx::query(
            "INSERT INTO items (id, item_type, title, inbox_status, is_favorite, is_pinned,
              capture_method, created_at, updated_at, revision, origin_device_id, last_modified_device_id, sync_state)
             VALUES (?, 'plain_text', 'Movable', 'new', 0, 0, 'manual_create', ?, ?, 1, 'dev', 'dev', 'local_only')",
        )
        .bind(item_id.as_str())
        .bind(now)
        .bind(now)
        .execute(db.pool())
        .await
        .expect("insert");

        let node_id = svc
            .place_item_on_board(&board.id, item_id.as_str(), 0.0, 0.0)
            .await
            .expect("place");

        // Move node
        let moves = vec![(node_id.clone(), 500.0, 300.0)];
        svc.move_nodes(&board.id, &moves).await.expect("move");

        // Verify
        let snapshot = svc.get_board_snapshot(&board.id).await.expect("snapshot");
        assert_eq!(snapshot.nodes[0].x, 500.0);
        assert_eq!(snapshot.nodes[0].y, 300.0);
    }

    #[tokio::test]
    async fn group_and_ungroup_preserves_positions() {
        let (db, svc) = setup().await;
        let board = svc.create_board("Group Test", None).await.expect("create");

        let now = chrono::Utc::now().timestamp_millis();
        let mut node_ids = Vec::new();

        // Create 2 items and place them
        for i in 0..2 {
            let item_id = EntityId::new();
            sqlx::query(
                "INSERT INTO items (id, item_type, title, inbox_status, is_favorite, is_pinned,
                  capture_method, created_at, updated_at, revision, origin_device_id, last_modified_device_id, sync_state)
                 VALUES (?, 'plain_text', ?, 'new', 0, 0, 'manual_create', ?, ?, 1, 'dev', 'dev', 'local_only')",
            )
            .bind(item_id.as_str())
            .bind(format!("Item {i}"))
            .bind(now)
            .bind(now)
            .execute(db.pool())
            .await
            .expect("insert");

            let x = (i as f64) * 200.0 + 50.0;
            let y = 100.0;
            let node_id = svc
                .place_item_on_board(&board.id, item_id.as_str(), x, y)
                .await
                .expect("place");
            node_ids.push(node_id);
        }

        // Group them
        let group_id = svc
            .group_nodes(&board.id, &node_ids, Some("My Group"))
            .await
            .expect("group");

        // Verify group exists
        let snapshot = svc.get_board_snapshot(&board.id).await.expect("snapshot");
        let group_node = snapshot.nodes.iter().find(|n| n.id == group_id);
        assert!(group_node.is_some());
        assert_eq!(group_node.unwrap().node_kind, "group");

        // Children should have parent_node_id set
        let children: Vec<_> = snapshot
            .nodes
            .iter()
            .filter(|n| n.parent_node_id.as_deref() == Some(&group_id))
            .collect();
        assert_eq!(children.len(), 2);

        // Ungroup
        svc.ungroup_nodes(&board.id, &group_id)
            .await
            .expect("ungroup");

        // Verify children are back to absolute positions
        let snapshot = svc.get_board_snapshot(&board.id).await.expect("snapshot");
        assert_eq!(snapshot.nodes.len(), 2); // Group node is gone
        for node in &snapshot.nodes {
            assert!(node.parent_node_id.is_none());
        }
        // First item should be back at x=50, second at x=250
        let n0 = snapshot.nodes.iter().find(|n| n.id == node_ids[0]).unwrap();
        let n1 = snapshot.nodes.iter().find(|n| n.id == node_ids[1]).unwrap();
        assert!((n0.x - 50.0).abs() < 0.01);
        assert!((n1.x - 250.0).abs() < 0.01);
    }

    #[tokio::test]
    async fn place_board_link_prevents_self_reference() {
        let (_, svc) = setup().await;
        let board = svc.create_board("Self Ref", None).await.expect("create");

        let result = svc.place_board_link(&board.id, &board.id, 0.0, 0.0).await;
        assert!(result.is_err());
    }

    #[tokio::test]
    async fn save_and_restore_viewport() {
        let (_, svc) = setup().await;
        let board = svc.create_board("Viewport", None).await.expect("create");

        svc.save_viewport(&board.id, 100.0, -50.0, 2.5)
            .await
            .expect("save");

        let snapshot = svc.get_board_snapshot(&board.id).await.expect("snapshot");
        assert_eq!(snapshot.board.viewport_x, 100.0);
        assert_eq!(snapshot.board.viewport_y, -50.0);
        assert!((snapshot.board.viewport_zoom - 2.5).abs() < 0.001);
    }
}
