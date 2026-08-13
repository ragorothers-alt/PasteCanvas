//! BRD-005 Board Edges integration tests.
//! Verifies edge creation, self-loop rejection, and cascade delete on node removal.

#[cfg(test)]
mod tests {
    use pastecanvas_desktop_lib::application::board_service::BoardService;
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

    /// Creates a board with two item nodes, returning (board_id, node_a, node_b).
    async fn board_with_two_nodes(db: &AppDatabase) -> (String, String, String) {
        let device = EntityId::new();
        let board_svc = BoardService::new(db.clone(), device.clone());
        let capture_svc = CaptureService::new(db.clone(), device.clone());

        let board = board_svc
            .create_board("Edge Test Board", None)
            .await
            .expect("board");

        let item_a = capture_svc
            .capture_text(CaptureTextRequest {
                text: "node A content".to_string(),
                capture_method: CaptureMethod::ManualHotkey,
                source_app: None,
                window_title: None,
            })
            .await
            .expect("item a");
        let item_b = capture_svc
            .capture_text(CaptureTextRequest {
                text: "node B content".to_string(),
                capture_method: CaptureMethod::ManualHotkey,
                source_app: None,
                window_title: None,
            })
            .await
            .expect("item b");

        let node_a = board_svc
            .place_item_on_board(&board.id, item_a.item_id.as_str(), 0.0, 0.0)
            .await
            .expect("node a");
        let node_b = board_svc
            .place_item_on_board(&board.id, item_b.item_id.as_str(), 300.0, 0.0)
            .await
            .expect("node b");

        (board.id, node_a, node_b)
    }

    #[tokio::test]
    async fn create_edge_between_two_nodes() {
        let db = setup().await;
        let (board_id, node_a, node_b) = board_with_two_nodes(&db).await;
        let svc = BoardService::new(db, EntityId::new());

        let edge_id = svc
            .create_edge(&board_id, &node_a, &node_b, Some("connects"))
            .await
            .expect("create edge");

        assert!(!edge_id.is_empty());
    }

    #[tokio::test]
    async fn self_loop_is_rejected() {
        let db = setup().await;
        let (board_id, node_a, _node_b) = board_with_two_nodes(&db).await;
        let svc = BoardService::new(db, EntityId::new());

        let result = svc.create_edge(&board_id, &node_a, &node_a, None).await;
        assert!(result.is_err());
    }

    #[tokio::test]
    async fn edge_appears_in_snapshot() {
        let db = setup().await;
        let (board_id, node_a, node_b) = board_with_two_nodes(&db).await;
        let svc = BoardService::new(db, EntityId::new());

        svc.create_edge(&board_id, &node_a, &node_b, None)
            .await
            .expect("edge");

        let snapshot = svc.get_board_snapshot(&board_id).await.expect("snapshot");
        assert_eq!(snapshot.edges.len(), 1);
        assert_eq!(snapshot.edges[0].source_node_id, node_a);
        assert_eq!(snapshot.edges[0].target_node_id, node_b);
    }

    #[tokio::test]
    async fn delete_edge_removes_from_snapshot() {
        let db = setup().await;
        let (board_id, node_a, node_b) = board_with_two_nodes(&db).await;
        let svc = BoardService::new(db, EntityId::new());

        let edge_id = svc
            .create_edge(&board_id, &node_a, &node_b, None)
            .await
            .expect("edge");
        assert_eq!(
            svc.get_board_snapshot(&board_id)
                .await
                .expect("s")
                .edges
                .len(),
            1
        );

        svc.delete_edge(&board_id, &edge_id).await.expect("delete");
        assert_eq!(
            svc.get_board_snapshot(&board_id)
                .await
                .expect("s")
                .edges
                .len(),
            0
        );
    }

    #[tokio::test]
    async fn removing_node_cascades_edge_soft_delete() {
        let db = setup().await;
        let (board_id, node_a, node_b) = board_with_two_nodes(&db).await;
        let svc = BoardService::new(db, EntityId::new());

        svc.create_edge(&board_id, &node_a, &node_b, None)
            .await
            .expect("edge");
        assert_eq!(
            svc.get_board_snapshot(&board_id)
                .await
                .expect("s")
                .edges
                .len(),
            1
        );

        // Remove node_a → its connected edge should be soft-deleted (BRD-005)
        svc.remove_node(&board_id, &node_a)
            .await
            .expect("remove node");
        assert_eq!(
            svc.get_board_snapshot(&board_id)
                .await
                .expect("s")
                .edges
                .len(),
            0
        );
    }

    #[tokio::test]
    async fn unrelated_edges_survive_node_removal() {
        let db = setup().await;
        let device = EntityId::new();
        let board_svc = BoardService::new(db.clone(), device.clone());
        let capture_svc = CaptureService::new(db.clone(), device.clone());

        let board = board_svc.create_board("Board", None).await.expect("board");

        // Create 3 nodes
        let mut node_ids = Vec::new();
        for i in 0..3 {
            let item = capture_svc
                .capture_text(CaptureTextRequest {
                    text: format!("node {}", i),
                    capture_method: CaptureMethod::ManualHotkey,
                    source_app: None,
                    window_title: None,
                })
                .await
                .expect("item");
            let nid = board_svc
                .place_item_on_board(&board.id, item.item_id.as_str(), (i as f64) * 100.0, 0.0)
                .await
                .expect("node");
            node_ids.push(nid);
        }

        // Edge 0-1 and edge 1-2
        board_svc
            .create_edge(&board.id, &node_ids[0], &node_ids[1], None)
            .await
            .expect("e1");
        board_svc
            .create_edge(&board.id, &node_ids[1], &node_ids[2], None)
            .await
            .expect("e2");
        assert_eq!(
            board_svc
                .get_board_snapshot(&board.id)
                .await
                .expect("s")
                .edges
                .len(),
            2
        );

        // Remove node 2 → only edge 1-2 should be deleted, edge 0-1 survives
        board_svc
            .remove_node(&board.id, &node_ids[2])
            .await
            .expect("remove");
        let snapshot = board_svc.get_board_snapshot(&board.id).await.expect("s");
        assert_eq!(snapshot.edges.len(), 1);
        assert_eq!(snapshot.edges[0].source_node_id, node_ids[0]);
    }
}
