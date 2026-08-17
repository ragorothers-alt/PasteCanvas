use sqlx::sqlite::{SqliteConnectOptions, SqlitePoolOptions};
use sqlx::{Row, SqlitePool};
use std::path::Path;
use std::str::FromStr;
use std::sync::Arc;
use std::time::Duration;

use crate::application::item_service::{InboxQuery, ItemSummaryDto};
use crate::domain::entities::item::Item;
use crate::domain::value_objects::{EntityId, UnixMs};

/// Application database handle — wraps the connection pool.
/// Cloneable via Arc for sharing across services.
#[derive(Clone)]
pub struct AppDatabase {
    pool: Arc<SqlitePool>,
}

impl AppDatabase {
    /// Creates a SQLite connection pool with production-safe settings.
    pub async fn open(db_path: &Path) -> Result<Self, sqlx::Error> {
        let url = format!("sqlite:{}", db_path.display());

        let options = SqliteConnectOptions::from_str(&url)?
            .create_if_missing(true)
            .journal_mode(sqlx::sqlite::SqliteJournalMode::Wal)
            .synchronous(sqlx::sqlite::SqliteSynchronous::Normal)
            .foreign_keys(true)
            .busy_timeout(Duration::from_millis(5000))
            .pragma("cache_size", "-8000")
            .pragma("temp_store", "MEMORY")
            .pragma("journal_size_limit", "67108864");

        let pool = SqlitePoolOptions::new()
            .max_connections(4)
            .min_connections(1)
            .acquire_timeout(Duration::from_secs(10))
            .connect_with(options)
            .await?;

        Ok(Self {
            pool: Arc::new(pool),
        })
    }

    /// Loads the persisted current device id, or creates one on first launch.
    pub async fn load_or_create_current_device(&self) -> Result<EntityId, sqlx::Error> {
        if let Some((id,)) =
            sqlx::query_as::<_, (String,)>("SELECT id FROM devices WHERE is_current = 1")
                .fetch_optional(self.pool())
                .await?
        {
            let now = UnixMs::now().as_millis();
            sqlx::query("UPDATE devices SET last_seen_at = ?, updated_at = ? WHERE id = ?")
                .bind(now)
                .bind(now)
                .bind(&id)
                .execute(self.pool())
                .await?;
            return Ok(EntityId::from_string(id));
        }

        let id = EntityId::new();
        let now = UnixMs::now().as_millis();
        sqlx::query(
            "INSERT INTO devices (id, display_name, platform, is_current, created_at, updated_at, last_seen_at)
             VALUES (?, 'This device', ?, 1, ?, ?, ?)",
        )
        .bind(id.as_str())
        .bind(std::env::consts::OS)
        .bind(now)
        .bind(now)
        .bind(now)
        .execute(self.pool())
        .await?;
        Ok(id)
    }

    /// Creates an in-memory database for testing.
    pub async fn open_in_memory() -> Result<Self, sqlx::Error> {
        let options = SqliteConnectOptions::from_str("sqlite::memory:")?
            .foreign_keys(true)
            .journal_mode(sqlx::sqlite::SqliteJournalMode::Wal)
            .synchronous(sqlx::sqlite::SqliteSynchronous::Normal);

        let pool = SqlitePoolOptions::new()
            .max_connections(1)
            .connect_with(options)
            .await?;

        Ok(Self {
            pool: Arc::new(pool),
        })
    }

    pub fn pool(&self) -> &SqlitePool {
        &self.pool
    }

    // --- Capture operations ---

    /// Checks if a content hash already exists (for dedup hints).
    pub async fn check_content_hash_exists(&self, hash: &str) -> Result<bool, sqlx::Error> {
        let row: Option<(i32,)> = sqlx::query_as(
            "SELECT 1 FROM items WHERE content_hash = ? AND deleted_at IS NULL LIMIT 1",
        )
        .bind(hash)
        .fetch_optional(&*self.pool)
        .await?;
        Ok(row.is_some())
    }

    /// Finds the earliest non-deleted item with the given content hash.
    /// Used by INB-004 dedup notice to locate the existing item.
    pub async fn find_item_by_content_hash(
        &self,
        hash: &str,
    ) -> Result<Option<String>, sqlx::Error> {
        let row: Option<(String,)> = sqlx::query_as(
            "SELECT id FROM items WHERE content_hash = ? AND deleted_at IS NULL ORDER BY created_at ASC LIMIT 1",
        )
        .bind(hash)
        .fetch_optional(&*self.pool)
        .await?;
        Ok(row.map(|(id,)| id))
    }

    /// Finds the earliest non-deleted item with the given normalized URL.
    /// Used by INB-004 dedup notice for URL-based dedup.
    pub async fn find_item_by_normalized_url(
        &self,
        url: &str,
    ) -> Result<Option<String>, sqlx::Error> {
        let row: Option<(String,)> = sqlx::query_as(
            "SELECT id FROM items WHERE normalized_url = ? AND deleted_at IS NULL ORDER BY created_at ASC LIMIT 1",
        )
        .bind(url)
        .fetch_optional(&*self.pool)
        .await?;
        Ok(row.map(|(id,)| id))
    }

    /// Inserts a source context record.
    pub async fn insert_source_context(
        &self,
        id: &EntityId,
        app_name: Option<&str>,
        window_title: Option<&str>,
        source_url: Option<&str>,
        source_domain: Option<&str>,
        now: UnixMs,
    ) -> Result<(), sqlx::Error> {
        sqlx::query(
            "INSERT INTO source_contexts (id, application_name, window_title, source_url, source_domain, created_at)
             VALUES (?, ?, ?, ?, ?, ?)",
        )
        .bind(id.as_str())
        .bind(app_name)
        .bind(window_title)
        .bind(source_url)
        .bind(source_domain)
        .bind(now.as_millis())
        .execute(&*self.pool)
        .await?;
        Ok(())
    }

    /// Inserts an item + content-addressed asset + search document in one
    /// transaction. Shared by image / screenshot / explorer capture paths.
    pub async fn insert_captured_asset_item(
        &self,
        item_id: &str,
        item_type: &str,
        title: &str,
        capture_method: &str,
        device_id: &str,
        now_ms: i64,
        sha256: &str,
        media_kind: &str,
        mime_type: &str,
        byte_size: i64,
        relative_path: &str,
        source_app: &str,
        filename: &str,
    ) -> Result<(), sqlx::Error> {
        let mut tx = self.pool.begin().await?;

        sqlx::query(
            "INSERT INTO items (id, item_type, title, content_hash, inbox_status, is_favorite, is_pinned,
              capture_method, created_at, updated_at, revision, origin_device_id, last_modified_device_id, sync_state)
             VALUES (?, ?, ?, ?, 'new', 0, 0, ?, ?, ?, 1, ?, ?, 'local_only')",
        )
        .bind(item_id)
        .bind(item_type)
        .bind(title)
        .bind(sha256)
        .bind(capture_method)
        .bind(now_ms)
        .bind(now_ms)
        .bind(device_id)
        .bind(device_id)
        .execute(&mut *tx)
        .await?;

        let asset_id = EntityId::new();
        sqlx::query(
            "INSERT OR IGNORE INTO assets (id, sha256, media_kind, mime_type, byte_size, relative_path, created_at)
             VALUES (?, ?, ?, ?, ?, ?, ?)",
        )
        .bind(asset_id.as_str())
        .bind(sha256)
        .bind(media_kind)
        .bind(mime_type)
        .bind(byte_size)
        .bind(relative_path)
        .bind(now_ms)
        .execute(&mut *tx)
        .await?;

        let actual_asset: (String,) = sqlx::query_as("SELECT id FROM assets WHERE sha256 = ?")
            .bind(sha256)
            .fetch_one(&mut *tx)
            .await?;

        sqlx::query(
            "INSERT OR IGNORE INTO item_assets (item_id, asset_id, role, sort_order) VALUES (?, ?, 'primary', 0)",
        )
        .bind(item_id)
        .bind(&actual_asset.0)
        .execute(&mut *tx)
        .await?;

        sqlx::query(
            "INSERT INTO search_documents (item_id, title, body, url, domain, source_app, tags_text, filename, updated_at)
             VALUES (?, ?, '', '', '', ?, '', ?, ?)",
        )
        .bind(item_id)
        .bind(title)
        .bind(source_app)
        .bind(filename)
        .bind(now_ms)
        .execute(&mut *tx)
        .await?;

        tx.commit().await?;
        Ok(())
    }
    /// Returns success ONLY after commit — durable write guarantee.
    pub async fn insert_item_with_search_doc(
        &self,
        item: &Item,
        source_context_id: Option<&EntityId>,
        source_app: Option<&str>,
        now: UnixMs,
        sanitized_html: Option<&str>,
    ) -> Result<(), sqlx::Error> {
        let mut tx = self.pool.begin().await?;

        // Insert Item
        sqlx::query(
            "INSERT INTO items (id, item_type, title, plain_text, sanitized_html, normalized_url, display_url,
              content_hash, inbox_status, is_favorite, is_pinned, capture_method,
              capture_session_id, source_context_id, created_at, updated_at, revision,
              origin_device_id, last_modified_device_id, sync_state)
             VALUES (?, ?, ?, ?, ?, ?, ?, ?, ?, ?, ?, ?, ?, ?, ?, ?, ?, ?, ?, 'local_only')",
        )
        .bind(item.id.as_str())
        .bind(item.item_type.as_str())
        .bind(&item.title)
        .bind(&item.plain_text)
        .bind(sanitized_html)
        .bind(&item.normalized_url)
        .bind(&item.display_url)
        .bind(&item.content_hash)
        .bind("new")
        .bind(item.is_favorite as i32)
        .bind(item.is_pinned as i32)
        .bind(capture_method_str(item.capture_method))
        .bind(item.capture_session_id.as_ref().map(|id| id.as_str()))
        .bind(source_context_id.map(|id| id.as_str()))
        .bind(item.created_at.as_millis())
        .bind(item.updated_at.as_millis())
        .bind(item.revision)
        .bind(item.origin_device_id.as_str())
        .bind(item.last_modified_device_id.as_str())
        .execute(&mut *tx)
        .await?;

        // Insert search document
        let body_preview: String = item
            .plain_text
            .as_deref()
            .unwrap_or("")
            .chars()
            .take(5000)
            .collect();
        let domain = item
            .normalized_url
            .as_deref()
            .and_then(|u| url::Url::parse(u).ok())
            .and_then(|u| u.host_str().map(String::from))
            .unwrap_or_default();

        sqlx::query(
            "INSERT INTO search_documents (item_id, title, body, url, domain, source_app, tags_text, filename, updated_at)
             VALUES (?, ?, ?, ?, ?, ?, '', '', ?)",
        )
        .bind(item.id.as_str())
        .bind(item.title.as_deref().unwrap_or(""))
        .bind(&body_preview)
        .bind(item.normalized_url.as_deref().unwrap_or(""))
        .bind(&domain)
        .bind(source_app.unwrap_or(""))
        .bind(now.as_millis())
        .execute(&mut *tx)
        .await?;

        // Insert operation log
        let op_id = EntityId::new();
        sqlx::query(
            "INSERT INTO operation_log (id, entity_type, entity_id, action, result_revision, payload_json, device_id, occurred_at)
             VALUES (?, 'item', ?, 'capture', 1, '{}', ?, ?)",
        )
        .bind(op_id.as_str())
        .bind(item.id.as_str())
        .bind(item.origin_device_id.as_str())
        .bind(now.as_millis())
        .execute(&mut *tx)
        .await?;

        // Insert event outbox
        let evt_id = EntityId::new();
        let payload = format!(
            r#"{{"itemIds":["{}"],"committedAt":{}}}"#,
            item.id,
            now.as_millis()
        );
        sqlx::query(
            "INSERT INTO event_outbox (id, event_type, aggregate_id, payload_json, created_at, attempts)
             VALUES (?, 'item.captured', ?, ?, ?, 0)",
        )
        .bind(evt_id.as_str())
        .bind(item.id.as_str())
        .bind(&payload)
        .bind(now.as_millis())
        .execute(&mut *tx)
        .await?;

        tx.commit().await?;
        Ok(())
    }

    // --- Inbox query operations ---

    /// Queries inbox items with pagination.
    pub async fn query_inbox_items(
        &self,
        query: &InboxQuery,
    ) -> Result<Vec<ItemSummaryDto>, sqlx::Error> {
        let sql = if query.status_filter.is_some() {
            "SELECT i.id, i.item_type, i.title, i.plain_text, i.display_url, i.inbox_status,
                    i.is_favorite, i.is_pinned, i.capture_method, i.created_at,
                    i.capture_session_id, i.revision, sd.domain,
                    sc.application_name as source_app,
                    a.byte_size as asset_byte_size
             FROM items i
             LEFT JOIN source_contexts sc ON sc.id = i.source_context_id
             LEFT JOIN search_documents sd ON sd.item_id = i.id
             LEFT JOIN item_assets ia ON ia.item_id = i.id AND ia.role = 'primary'
             LEFT JOIN assets a ON a.id = ia.asset_id
             WHERE i.deleted_at IS NULL AND i.inbox_status = ?
             ORDER BY i.is_pinned DESC, i.created_at DESC
             LIMIT ? OFFSET ?"
        } else {
            "SELECT i.id, i.item_type, i.title, i.plain_text, i.display_url, i.inbox_status,
                    i.is_favorite, i.is_pinned, i.capture_method, i.created_at,
                    i.capture_session_id, i.revision, sd.domain,
                    sc.application_name as source_app,
                    a.byte_size as asset_byte_size
             FROM items i
             LEFT JOIN source_contexts sc ON sc.id = i.source_context_id
             LEFT JOIN search_documents sd ON sd.item_id = i.id
             LEFT JOIN item_assets ia ON ia.item_id = i.id AND ia.role = 'primary'
             LEFT JOIN assets a ON a.id = ia.asset_id
             WHERE i.deleted_at IS NULL AND i.inbox_status != 'archived'
             ORDER BY i.is_pinned DESC, i.created_at DESC
             LIMIT ? OFFSET ?"
        };

        let rows = if let Some(ref status) = query.status_filter {
            let status_str = match status {
                crate::domain::entities::item::InboxStatus::New => "new",
                crate::domain::entities::item::InboxStatus::Triaged => "triaged",
                crate::domain::entities::item::InboxStatus::Archived => "archived",
            };
            sqlx::query(sql)
                .bind(status_str)
                .bind(query.limit)
                .bind(query.offset)
                .fetch_all(&*self.pool)
                .await?
        } else {
            sqlx::query(sql)
                .bind(query.limit)
                .bind(query.offset)
                .fetch_all(&*self.pool)
                .await?
        };

        let items = rows
            .iter()
            .map(|row| {
                let plain_text: Option<String> = row.get("plain_text");
                let text_preview = plain_text.map(|t| t.chars().take(200).collect());
                let domain_raw: Option<String> = row.get("domain");
                ItemSummaryDto {
                    id: row.get("id"),
                    item_type: row.get("item_type"),
                    title: row.get("title"),
                    text_preview,
                    display_url: row.get("display_url"),
                    inbox_status: row.get("inbox_status"),
                    is_favorite: row.get::<i32, _>("is_favorite") == 1,
                    is_pinned: row.get::<i32, _>("is_pinned") == 1,
                    capture_method: row.get("capture_method"),
                    source_app: row.get("source_app"),
                    // Treat empty domain as absent for cleaner grouping
                    domain: domain_raw.filter(|d| !d.is_empty()),
                    capture_session_id: row.get("capture_session_id"),
                    created_at: row.get("created_at"),
                    revision: row.get("revision"),
                    asset_byte_size: row.try_get("asset_byte_size").ok().flatten(),
                }
            })
            .collect();

        Ok(items)
    }

    /// Toggles favorite on an item, returns new state.
    pub async fn toggle_item_favorite(&self, item_id: &str) -> Result<bool, sqlx::Error> {
        let now = UnixMs::now().as_millis();
        sqlx::query(
            "UPDATE items SET is_favorite = CASE WHEN is_favorite = 0 THEN 1 ELSE 0 END,
             updated_at = ?, revision = revision + 1
             WHERE id = ? AND deleted_at IS NULL",
        )
        .bind(now)
        .bind(item_id)
        .execute(&*self.pool)
        .await?;

        let row: Option<(i32,)> = sqlx::query_as("SELECT is_favorite FROM items WHERE id = ?")
            .bind(item_id)
            .fetch_optional(&*self.pool)
            .await?;

        Ok(row.map(|(v,)| v == 1).unwrap_or(false))
    }

    /// Toggles the pinned flag on an item (INB-003). Returns the new state.
    pub async fn toggle_item_pin(&self, item_id: &str) -> Result<bool, sqlx::Error> {
        let now = UnixMs::now().as_millis();
        sqlx::query(
            "UPDATE items SET is_pinned = CASE WHEN is_pinned = 0 THEN 1 ELSE 0 END,
             updated_at = ?, revision = revision + 1
             WHERE id = ? AND deleted_at IS NULL",
        )
        .bind(now)
        .bind(item_id)
        .execute(&*self.pool)
        .await?;

        let row: Option<(i32,)> = sqlx::query_as("SELECT is_pinned FROM items WHERE id = ?")
            .bind(item_id)
            .fetch_optional(&*self.pool)
            .await?;

        Ok(row.map(|(v,)| v == 1).unwrap_or(false))
    }

    /// Soft-deletes an item.
    pub async fn soft_delete_item(&self, item_id: &str) -> Result<(), sqlx::Error> {
        let now = UnixMs::now().as_millis();
        sqlx::query(
            "UPDATE items SET deleted_at = ?, updated_at = ?, revision = revision + 1 WHERE id = ?",
        )
        .bind(now)
        .bind(now)
        .bind(item_id)
        .execute(&*self.pool)
        .await?;
        Ok(())
    }

    // --- Trash operations (DAT-003) ---

    /// Trash impact stats: item count + total bytes of their primary assets.
    pub async fn get_trash_stats(&self) -> Result<(i64, i64), sqlx::Error> {
        let row: (i64, i64) = sqlx::query_as(
            "SELECT COUNT(*), COALESCE((
                 SELECT SUM(a.byte_size) FROM assets a
                 WHERE a.id IN (
                   SELECT ia.asset_id FROM item_assets ia
                   JOIN items i2 ON i2.id = ia.item_id
                   WHERE i2.deleted_at IS NOT NULL
                 )
               ), 0)
             FROM items WHERE deleted_at IS NOT NULL",
        )
        .fetch_one(&*self.pool)
        .await?;
        Ok(row)
    }

    /// Queries soft-deleted items (the trash) for display/restore.
    pub async fn query_trash_items(&self, limit: i64) -> Result<Vec<ItemSummaryDto>, sqlx::Error> {
        let rows = sqlx::query(
            "SELECT i.id, i.item_type, i.title, i.plain_text, i.display_url, i.inbox_status,
                    i.is_favorite, i.is_pinned, i.capture_method, i.created_at,
                    i.capture_session_id, i.revision, sd.domain,
                    sc.application_name as source_app,
                    a.byte_size as asset_byte_size
             FROM items i
             LEFT JOIN source_contexts sc ON sc.id = i.source_context_id
             LEFT JOIN search_documents sd ON sd.item_id = i.id
             LEFT JOIN item_assets ia ON ia.item_id = i.id AND ia.role = 'primary'
             LEFT JOIN assets a ON a.id = ia.asset_id
             WHERE i.deleted_at IS NOT NULL
             ORDER BY i.deleted_at DESC
             LIMIT ?",
        )
        .bind(limit)
        .fetch_all(&*self.pool)
        .await?;

        let items = rows
            .iter()
            .map(|row| {
                use sqlx::Row;
                let plain_text: Option<String> = row.get("plain_text");
                let text_preview = plain_text.map(|t| t.chars().take(200).collect());
                let domain_raw: Option<String> = row.get("domain");
                ItemSummaryDto {
                    id: row.get("id"),
                    item_type: row.get("item_type"),
                    title: row.get("title"),
                    text_preview,
                    display_url: row.get("display_url"),
                    inbox_status: row.get("inbox_status"),
                    is_favorite: row.get::<i32, _>("is_favorite") == 1,
                    is_pinned: row.get::<i32, _>("is_pinned") == 1,
                    capture_method: row.get("capture_method"),
                    source_app: row.get("source_app"),
                    domain: domain_raw.filter(|d| !d.is_empty()),
                    capture_session_id: row.get("capture_session_id"),
                    created_at: row.get("created_at"),
                    revision: row.get("revision"),
                    asset_byte_size: row.try_get("asset_byte_size").ok().flatten(),
                }
            })
            .collect();

        Ok(items)
    }

    /// Permanently deletes all trashed items and their orphaned assets (DAT-003).
    ///
    /// Only assets with NO remaining references are removed from the assets
    /// table; their relative paths are returned so the caller can delete the
    /// physical files. Returns (deleted_item_count, orphan_asset_paths).
    pub async fn empty_trash(&self) -> Result<(i64, Vec<String>), sqlx::Error> {
        let mut tx = self.pool.begin().await?;

        // Collect candidate asset IDs belonging to trashed items BEFORE deletion
        let asset_ids: Vec<(String,)> = sqlx::query_as(
            "SELECT DISTINCT ia.asset_id FROM item_assets ia
             JOIN items i ON i.id = ia.item_id
             WHERE i.deleted_at IS NOT NULL",
        )
        .fetch_all(&mut *tx)
        .await?;

        // board_nodes.item_id is ON DELETE RESTRICT, so placements of trashed
        // items must be removed first (edges cascade from the node delete).
        sqlx::query(
            "DELETE FROM board_nodes WHERE item_id IN (SELECT id FROM items WHERE deleted_at IS NOT NULL)",
        )
        .execute(&mut *tx)
        .await?;

        // Hard-delete trashed items (cascades to item_assets + search_documents)
        let result = sqlx::query("DELETE FROM items WHERE deleted_at IS NOT NULL")
            .execute(&mut *tx)
            .await?;
        let deleted_items = result.rows_affected() as i64;

        // Find orphaned assets (no remaining item_assets references)
        let mut orphan_paths = Vec::new();
        for (asset_id,) in asset_ids {
            let refs: (i64,) =
                sqlx::query_as("SELECT COUNT(*) FROM item_assets WHERE asset_id = ?")
                    .bind(&asset_id)
                    .fetch_one(&mut *tx)
                    .await?;
            if refs.0 == 0 {
                let path: Option<(String,)> =
                    sqlx::query_as("SELECT relative_path FROM assets WHERE id = ?")
                        .bind(&asset_id)
                        .fetch_optional(&mut *tx)
                        .await?;
                if let Some((p,)) = path {
                    orphan_paths.push(p);
                }
                sqlx::query("DELETE FROM assets WHERE id = ?")
                    .bind(&asset_id)
                    .execute(&mut *tx)
                    .await?;
            }
        }

        tx.commit().await?;
        Ok((deleted_items, orphan_paths))
    }
}

fn capture_method_str(method: crate::domain::entities::item::CaptureMethod) -> &'static str {
    method.as_str()
}

/// Creates an in-memory pool for testing (kept for backward compat).
pub async fn create_test_pool() -> Result<SqlitePool, sqlx::Error> {
    let options = SqliteConnectOptions::from_str("sqlite::memory:")?
        .foreign_keys(true)
        .journal_mode(sqlx::sqlite::SqliteJournalMode::Wal)
        .synchronous(sqlx::sqlite::SqliteSynchronous::Normal);

    SqlitePoolOptions::new()
        .max_connections(1)
        .connect_with(options)
        .await
}

#[cfg(test)]
mod tests {
    use super::*;

    #[tokio::test]
    async fn test_pool_enables_foreign_keys() {
        let pool = create_test_pool().await.expect("create test pool");
        let fk: (i32,) = sqlx::query_as("PRAGMA foreign_keys")
            .fetch_one(&pool)
            .await
            .expect("query pragma");
        assert_eq!(fk.0, 1);
    }

    #[tokio::test]
    async fn test_app_db_capture_and_query() {
        let db = AppDatabase::open_in_memory().await.expect("open db");

        // Run migrations
        crate::infrastructure::database::migrator::run_migrations(db.pool())
            .await
            .expect("migrations");

        // Capture a text item
        let svc =
            crate::application::capture_service::CaptureService::new(db.clone(), EntityId::new());
        let result = svc
            .capture_text(crate::application::capture_service::CaptureTextRequest {
                text: "Hello from test".to_string(),
                capture_method: crate::domain::entities::item::CaptureMethod::ManualHotkey,
                source_app: Some("test_app".to_string()),
                window_title: None,
            })
            .await
            .expect("capture");

        assert!(!result.is_duplicate);

        // Query inbox
        let items = db
            .query_inbox_items(&InboxQuery::default())
            .await
            .expect("query");
        assert_eq!(items.len(), 1);
        assert_eq!(items[0].title.as_deref(), Some("Hello from test"));
        assert_eq!(items[0].source_app.as_deref(), Some("test_app"));
        assert_eq!(items[0].revision, 1);

        sqlx::query("UPDATE items SET revision = 3 WHERE id = ?")
            .bind(result.item_id.to_string())
            .execute(db.pool())
            .await
            .expect("update revision");
        let items = db
            .query_inbox_items(&InboxQuery::default())
            .await
            .expect("query updated revision");
        assert_eq!(items[0].revision, 3);

        sqlx::query("UPDATE items SET deleted_at = ? WHERE id = ?")
            .bind(chrono::Utc::now().timestamp_millis())
            .bind(result.item_id.to_string())
            .execute(db.pool())
            .await
            .expect("trash item");
        let trashed = db.query_trash_items(10).await.expect("query trash");
        assert_eq!(trashed.len(), 1);
        assert_eq!(trashed[0].revision, 3);
    }

    #[tokio::test]
    async fn test_duplicate_detection() {
        let db = AppDatabase::open_in_memory().await.expect("open db");
        crate::infrastructure::database::migrator::run_migrations(db.pool())
            .await
            .expect("migrations");

        let svc =
            crate::application::capture_service::CaptureService::new(db.clone(), EntityId::new());

        let req = crate::application::capture_service::CaptureTextRequest {
            text: "duplicate content".to_string(),
            capture_method: crate::domain::entities::item::CaptureMethod::ManualHotkey,
            source_app: None,
            window_title: None,
        };

        let r1 = svc.capture_text(req.clone()).await.expect("first");
        assert!(!r1.is_duplicate);

        let r2 = svc.capture_text(req).await.expect("second");
        assert!(r2.is_duplicate); // Advisory hint only
    }

    #[tokio::test]
    async fn device_id_persists_across_lookups() {
        let db = AppDatabase::open_in_memory().await.expect("open db");
        crate::infrastructure::database::migrator::run_migrations(db.pool())
            .await
            .expect("migrations");

        let first = db.load_or_create_current_device().await.expect("create");
        let second = db.load_or_create_current_device().await.expect("reload");
        assert_eq!(first.as_str(), second.as_str());

        let count: (i64,) = sqlx::query_as("SELECT COUNT(*) FROM devices")
            .fetch_one(db.pool())
            .await
            .expect("count");
        assert_eq!(count.0, 1);
    }
}
