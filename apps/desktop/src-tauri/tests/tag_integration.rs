//! Integration tests for Tag management.

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

    async fn create_test_item(db: &AppDatabase) -> String {
        let svc = CaptureService::new(db.clone(), EntityId::new());
        let result = svc
            .capture_text(CaptureTextRequest {
                text: "test item for tagging".to_string(),
                capture_method: CaptureMethod::ManualHotkey,
                source_app: None,
                window_title: None,
            })
            .await
            .expect("capture");
        result.item_id.to_string()
    }

    #[tokio::test]
    async fn create_and_list_tags() {
        let db = setup().await;
        let now = chrono::Utc::now().timestamp_millis();

        // Create a tag
        sqlx::query(
            "INSERT INTO tags (id, name, normalized_name, color, created_at, updated_at) VALUES (?, ?, ?, NULL, ?, ?)",
        )
        .bind("tag-1")
        .bind("Important")
        .bind("important")
        .bind(now)
        .bind(now)
        .execute(db.pool())
        .await
        .expect("create tag");

        // List tags
        let tags: Vec<(String, String)> =
            sqlx::query_as("SELECT id, name FROM tags WHERE deleted_at IS NULL")
                .fetch_all(db.pool())
                .await
                .expect("list");

        assert_eq!(tags.len(), 1);
        assert_eq!(tags[0].1, "Important");
    }

    #[tokio::test]
    async fn add_tag_to_item_and_query() {
        let db = setup().await;
        let item_id = create_test_item(&db).await;
        let now = chrono::Utc::now().timestamp_millis();

        // Create tag
        sqlx::query(
            "INSERT INTO tags (id, name, normalized_name, color, created_at, updated_at) VALUES (?, ?, ?, NULL, ?, ?)",
        )
        .bind("tag-work")
        .bind("Work")
        .bind("work")
        .bind(now)
        .bind(now)
        .execute(db.pool())
        .await
        .expect("create tag");

        // Add tag to item
        sqlx::query("INSERT INTO item_tags (item_id, tag_id, created_at) VALUES (?, ?, ?)")
            .bind(&item_id)
            .bind("tag-work")
            .bind(now)
            .execute(db.pool())
            .await
            .expect("add tag");

        // Query item's tags
        let tags: Vec<(String,)> = sqlx::query_as(
            "SELECT t.name FROM tags t JOIN item_tags it ON it.tag_id = t.id WHERE it.item_id = ?",
        )
        .bind(&item_id)
        .fetch_all(db.pool())
        .await
        .expect("query tags");

        assert_eq!(tags.len(), 1);
        assert_eq!(tags[0].0, "Work");
    }

    #[tokio::test]
    async fn remove_tag_from_item() {
        let db = setup().await;
        let item_id = create_test_item(&db).await;
        let now = chrono::Utc::now().timestamp_millis();

        // Create and add tag
        sqlx::query(
            "INSERT INTO tags (id, name, normalized_name, color, created_at, updated_at) VALUES (?, ?, ?, NULL, ?, ?)",
        )
        .bind("tag-temp")
        .bind("Temp")
        .bind("temp")
        .bind(now)
        .bind(now)
        .execute(db.pool())
        .await
        .expect("create");

        sqlx::query("INSERT INTO item_tags (item_id, tag_id, created_at) VALUES (?, ?, ?)")
            .bind(&item_id)
            .bind("tag-temp")
            .bind(now)
            .execute(db.pool())
            .await
            .expect("add");

        // Remove tag
        sqlx::query("DELETE FROM item_tags WHERE item_id = ? AND tag_id = ?")
            .bind(&item_id)
            .bind("tag-temp")
            .execute(db.pool())
            .await
            .expect("remove");

        // Verify removed
        let count: (i64,) = sqlx::query_as("SELECT COUNT(*) FROM item_tags WHERE item_id = ?")
            .bind(&item_id)
            .fetch_one(db.pool())
            .await
            .expect("count");

        assert_eq!(count.0, 0);
    }

    #[tokio::test]
    async fn duplicate_tag_name_rejected() {
        let db = setup().await;
        let now = chrono::Utc::now().timestamp_millis();

        sqlx::query(
            "INSERT INTO tags (id, name, normalized_name, color, created_at, updated_at) VALUES (?, ?, ?, NULL, ?, ?)",
        )
        .bind("tag-dup-1")
        .bind("Unique")
        .bind("unique")
        .bind(now)
        .bind(now)
        .execute(db.pool())
        .await
        .expect("first");

        // Same normalized_name should fail
        let result = sqlx::query(
            "INSERT INTO tags (id, name, normalized_name, color, created_at, updated_at) VALUES (?, ?, ?, NULL, ?, ?)",
        )
        .bind("tag-dup-2")
        .bind("UNIQUE")
        .bind("unique")
        .bind(now)
        .bind(now)
        .execute(db.pool())
        .await;

        assert!(result.is_err());
    }

    #[tokio::test]
    async fn soft_delete_tag() {
        let db = setup().await;
        let now = chrono::Utc::now().timestamp_millis();

        sqlx::query(
            "INSERT INTO tags (id, name, normalized_name, color, created_at, updated_at) VALUES (?, ?, ?, NULL, ?, ?)",
        )
        .bind("tag-del")
        .bind("ToDelete")
        .bind("todelete")
        .bind(now)
        .bind(now)
        .execute(db.pool())
        .await
        .expect("create");

        // Soft delete
        sqlx::query("UPDATE tags SET deleted_at = ? WHERE id = ?")
            .bind(now)
            .bind("tag-del")
            .execute(db.pool())
            .await
            .expect("delete");

        // Should not appear in active list
        let count: (i64,) = sqlx::query_as("SELECT COUNT(*) FROM tags WHERE deleted_at IS NULL")
            .fetch_one(db.pool())
            .await
            .expect("count");

        assert_eq!(count.0, 0);
    }
}
