use crate::domain::entities::item::InboxStatus;
use crate::infrastructure::database::pool::AppDatabase;
use serde::Serialize;
use thiserror::Error;

#[derive(Debug, Error)]
pub enum ItemServiceError {
    #[error("database error: {0}")]
    Database(#[from] sqlx::Error),

    #[error("item not found: {0}")]
    NotFound(String),
}

/// Summary DTO for Inbox list display.
#[derive(Debug, Clone, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct ItemSummaryDto {
    pub id: String,
    pub item_type: String,
    pub title: Option<String>,
    pub text_preview: Option<String>,
    pub display_url: Option<String>,
    pub inbox_status: String,
    pub is_favorite: bool,
    pub is_pinned: bool,
    pub capture_method: String,
    pub source_app: Option<String>,
    /// Website domain for INB-002 smart grouping.
    pub domain: Option<String>,
    /// Capture session ID for INB-002 smart grouping.
    pub capture_session_id: Option<String>,
    pub created_at: i64,
    /// Optimistic-concurrency token used by item edits.
    pub revision: i64,
    /// Primary asset size when the item has one (fonts/PDFs/images).
    pub asset_byte_size: Option<i64>,
}

#[cfg(test)]
mod tests {
    use super::ItemSummaryDto;

    #[test]
    fn summary_serializes_revision_as_camel_case() {
        let summary = ItemSummaryDto {
            id: "item-1".to_string(),
            item_type: "plain_text".to_string(),
            title: Some("A title".to_string()),
            text_preview: Some("Preview".to_string()),
            display_url: None,
            inbox_status: "new".to_string(),
            is_favorite: false,
            is_pinned: false,
            capture_method: "manual_hotkey".to_string(),
            source_app: None,
            domain: None,
            capture_session_id: None,
            created_at: 1,
            revision: 7,
            asset_byte_size: None,
        };

        let json = serde_json::to_value(summary).expect("summary serializes");
        assert_eq!(
            json.get("revision").and_then(|value| value.as_i64()),
            Some(7)
        );
        assert!(json.get("revision_").is_none());
    }
}

/// Paginated Inbox query parameters.
#[derive(Debug, Clone)]
pub struct InboxQuery {
    pub limit: i64,
    pub offset: i64,
    pub status_filter: Option<InboxStatus>,
}

impl Default for InboxQuery {
    fn default() -> Self {
        Self {
            limit: 50,
            offset: 0,
            status_filter: None,
        }
    }
}

/// ItemService handles queries and mutations on Items for the Inbox.
pub struct ItemService {
    db: AppDatabase,
}

impl ItemService {
    pub fn new(db: AppDatabase) -> Self {
        Self { db }
    }

    /// Queries the Inbox with pagination and optional status filter.
    pub async fn query_inbox(
        &self,
        query: InboxQuery,
    ) -> Result<Vec<ItemSummaryDto>, ItemServiceError> {
        let items = self.db.query_inbox_items(&query).await?;
        Ok(items)
    }

    /// Toggles favorite status on an item.
    pub async fn toggle_favorite(&self, item_id: &str) -> Result<bool, ItemServiceError> {
        self.db
            .toggle_item_favorite(item_id)
            .await
            .map_err(ItemServiceError::Database)
    }

    /// Toggles the pinned flag (INB-003).
    pub async fn toggle_pin(&self, item_id: &str) -> Result<bool, ItemServiceError> {
        self.db
            .toggle_item_pin(item_id)
            .await
            .map_err(ItemServiceError::Database)
    }

    /// Soft-deletes an item (moves to trash).
    pub async fn trash_item(&self, item_id: &str) -> Result<(), ItemServiceError> {
        self.db
            .soft_delete_item(item_id)
            .await
            .map_err(ItemServiceError::Database)
    }

    /// DAT-003: trash impact stats (item count + total asset bytes).
    pub async fn trash_stats(&self) -> Result<(i64, i64), ItemServiceError> {
        self.db
            .get_trash_stats()
            .await
            .map_err(ItemServiceError::Database)
    }

    /// DAT-003: lists soft-deleted items.
    pub async fn query_trash(&self, limit: i64) -> Result<Vec<ItemSummaryDto>, ItemServiceError> {
        self.db
            .query_trash_items(limit)
            .await
            .map_err(ItemServiceError::Database)
    }

    /// DAT-003: permanently deletes trashed items + orphaned assets.
    /// Returns (deleted item count, relative paths of removed asset files).
    pub async fn empty_trash(&self) -> Result<(i64, Vec<String>), ItemServiceError> {
        self.db
            .empty_trash()
            .await
            .map_err(ItemServiceError::Database)
    }
}
