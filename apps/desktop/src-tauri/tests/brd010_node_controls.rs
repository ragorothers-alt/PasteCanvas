//! BRD-010 node controls integration tests.
//! Phase-2 Workstream C ("drag and resize", "z-order and lock") and
//! Workstream D ("group title, collapse and lock").

#[cfg(test)]
mod tests {
    use pastecanvas_desktop_lib::application::board_service::{BoardService, BoardServiceError};
    use pastecanvas_desktop_lib::application::capture_service::{
        CaptureService, CaptureTextRequest,
    };
    use pastecanvas_desktop_lib::domain::entities::item::CaptureMethod;
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

    async fn capture(db: &AppDatabase, text: &str) -> String {
        let svc = CaptureService::new(db.clone(), EntityId::new());
        svc.capture_text(CaptureTextRequest {
            text: text.to_string(),
            capture_method: CaptureMethod::ManualHotkey,
            source_app: None,
            window_title: None,
        })
        .await
        .expect("capture")
        .item_id
        .to_string()
    }

    /// Creates a board with `count` item nodes placed left-to-right.
    async fn board_with_nodes(
        db: &AppDatabase,
        svc: &BoardService,
        count: usize,
    ) -> (String, Vec<String>) {
        let board = svc.create_board("B", None).await.expect("board");
        let mut node_ids = Vec::new();
        for i in 0..count {
            let item = capture(db, &format!("node {i}")).await;
            let node_id = svc
                .place_item_on_board(&board.id, &item, (i * 300) as f64, 0.0)
                .await
                .expect("place");
            node_ids.push(node_id);
        }
        (board.id, node_ids)
    }

    #[tokio::test]
    async fn resize_persists_new_dimensions() {
        let (db, svc) = setup().await;
        let (board_id, nodes) = board_with_nodes(&db, &svc, 1).await;

        svc.resize_nodes(&board_id, &[(nodes[0].clone(), 500.0, 320.0)])
            .await
            .expect("resize");

        let snap = svc.get_board_snapshot(&board_id).await.expect("snap");
        assert_eq!(snap.nodes[0].width, 500.0);
        assert_eq!(snap.nodes[0].height, 320.0);
    }

    #[tokio::test]
    async fn resize_floors_below_minimum() {
        let (db, svc) = setup().await;
        let (board_id, nodes) = board_with_nodes(&db, &svc, 1).await;

        // Values below the usable floor (40px) are clamped, not rejected.
        svc.resize_nodes(&board_id, &[(nodes[0].clone(), 1.0, 10.0)])
            .await
            .expect("resize");

        let snap = svc.get_board_snapshot(&board_id).await.expect("snap");
        assert_eq!(snap.nodes[0].width, 40.0);
        assert_eq!(snap.nodes[0].height, 40.0);
    }

    #[tokio::test]
    async fn resize_rejects_non_finite_dimensions() {
        let (db, svc) = setup().await;
        let (board_id, nodes) = board_with_nodes(&db, &svc, 1).await;

        let err = svc
            .resize_nodes(&board_id, &[(nodes[0].clone(), f64::NAN, 100.0)])
            .await
            .expect_err("nan must be rejected");
        assert!(matches!(err, BoardServiceError::NotFound(_)));
    }

    #[tokio::test]
    async fn lock_roundtrip_and_not_found() {
        let (db, svc) = setup().await;
        let (board_id, nodes) = board_with_nodes(&db, &svc, 1).await;

        svc.set_node_locked(&board_id, &nodes[0], true)
            .await
            .expect("lock");
        let snap = svc.get_board_snapshot(&board_id).await.expect("snap");
        assert!(snap.nodes[0].locked);

        svc.set_node_locked(&board_id, &nodes[0], false)
            .await
            .expect("unlock");
        let snap = svc.get_board_snapshot(&board_id).await.expect("snap");
        assert!(!snap.nodes[0].locked);

        let err = svc
            .set_node_locked(&board_id, "missing-node", true)
            .await
            .expect_err("unknown node");
        assert!(matches!(err, BoardServiceError::NotFound(_)));
    }

    #[tokio::test]
    async fn collapse_roundtrip() {
        let (db, svc) = setup().await;
        let (board_id, nodes) = board_with_nodes(&db, &svc, 1).await;

        svc.set_node_collapsed(&board_id, &nodes[0], true)
            .await
            .expect("collapse");
        let snap = svc.get_board_snapshot(&board_id).await.expect("snap");
        assert!(snap.nodes[0].collapsed);

        svc.set_node_collapsed(&board_id, &nodes[0], false)
            .await
            .expect("expand");
        let snap = svc.get_board_snapshot(&board_id).await.expect("snap");
        assert!(!snap.nodes[0].collapsed);
    }

    #[tokio::test]
    async fn bring_to_front_raises_above_all_siblings() {
        let (db, svc) = setup().await;
        let (board_id, nodes) = board_with_nodes(&db, &svc, 3).await;

        // Placement order gives z = 1, 2, 3. Raise the first node.
        svc.bring_node_to_front(&board_id, &nodes[0])
            .await
            .expect("bring to front");

        let snap = svc.get_board_snapshot(&board_id).await.expect("snap");
        let first = snap.nodes.iter().find(|n| n.id == nodes[0]).expect("n0");
        let others_max = snap
            .nodes
            .iter()
            .filter(|n| n.id != nodes[0])
            .map(|n| n.z_index)
            .max()
            .expect("others");
        assert!(first.z_index > others_max);
    }

    #[tokio::test]
    async fn bring_to_front_missing_node_errors() {
        let (db, svc) = setup().await;
        let (board_id, _nodes) = board_with_nodes(&db, &svc, 1).await;

        let err = svc
            .bring_node_to_front(&board_id, "missing-node")
            .await
            .expect_err("unknown node");
        assert!(matches!(err, BoardServiceError::NotFound(_)));
    }
}
