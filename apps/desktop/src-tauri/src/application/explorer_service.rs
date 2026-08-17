//! Explorer file capture service (CAP-007).
//!
//! Captures files from Windows Explorer context-menu invocations into the Inbox:
//! copies the file into the content-addressed asset store (original file may be
//! moved/deleted afterwards — DAT-001), detects the media type, and creates the
//! Item + search document in one transaction. Never blocks the caller.

use crate::domain::entities::item::{CaptureMethod, InboxStatus, Item};
use crate::domain::value_objects::{EntityId, ItemType, UnixMs};
use crate::infrastructure::asset_store::{AssetStore, MediaKind};
use crate::infrastructure::database::pool::AppDatabase;
use thiserror::Error;

#[derive(Debug, Error)]
pub enum ExplorerCaptureError {
    #[error("I/O error: {0}")]
    Io(#[from] std::io::Error),

    #[error("database error: {0}")]
    Database(#[from] sqlx::Error),

    #[error("asset store error: {0}")]
    Asset(String),

    #[error("file not found: {0}")]
    NotFound(String),

    #[error("board error: {0}")]
    Board(String),

    #[error("file too large: {0} bytes")]
    TooLarge(u64),
}

/// Maps asset media kind to item type.
fn item_type_for_media(kind: MediaKind) -> ItemType {
    match kind {
        MediaKind::Image => ItemType::Image,
        MediaKind::Pdf => ItemType::Pdf,
        MediaKind::Font => ItemType::Font,
        MediaKind::Svg => ItemType::Svg,
        MediaKind::Generic => ItemType::File,
    }
}

/// Derives a display title from a filename (strips extension).
fn title_from_filename(filename: &str) -> String {
    let name = filename.rsplit(['/', '\\']).next().unwrap_or(filename);
    match name.rfind('.') {
        Some(idx) if idx > 0 => name[..idx].to_string(),
        _ => name.to_string(),
    }
}

pub struct ExplorerCaptureService {
    db: AppDatabase,
    device_id: EntityId,
    data_dir: std::path::PathBuf,
}

impl ExplorerCaptureService {
    pub fn new(db: AppDatabase, device_id: EntityId, data_dir: std::path::PathBuf) -> Self {
        Self {
            db,
            device_id,
            data_dir,
        }
    }

    /// Captures a single file into the Inbox. Returns the new item ID.
    /// Per DAT-001: the file is copied into the managed asset library, so the
    /// original may be moved or deleted without losing the captured content.
    pub async fn capture_file(&self, path: &str) -> Result<EntityId, ExplorerCaptureError> {
        let file_path = std::path::Path::new(path);
        if !file_path.exists() {
            return Err(ExplorerCaptureError::NotFound(path.to_string()));
        }

        const MAX_CAPTURE_BYTES: u64 = 50 * 1024 * 1024;
        let meta = std::fs::metadata(file_path)?;
        if meta.len() > MAX_CAPTURE_BYTES {
            return Err(ExplorerCaptureError::TooLarge(meta.len()));
        }

        let filename = file_path
            .file_name()
            .map(|f| f.to_string_lossy().to_string())
            .unwrap_or_else(|| "file".to_string());

        // Read and stage into the content-addressed store (atomic, dedup)
        let bytes = std::fs::read(file_path)?;
        let asset_store = AssetStore::new(self.data_dir.clone());
        // Ensure the store directory structure exists (idempotent)
        asset_store
            .initialize()
            .map_err(|e| ExplorerCaptureError::Asset(e.to_string()))?;
        let staged = asset_store
            .stage_bytes(&bytes, Some(&filename))
            .map_err(|e| ExplorerCaptureError::Asset(e.to_string()))?;

        let now = UnixMs::now();
        let item_id = EntityId::new();
        let item_type = item_type_for_media(staged.media_kind);
        let title = title_from_filename(&filename);

        let item = Item {
            id: item_id.clone(),
            item_type,
            title: Some(title.clone()),
            plain_text: None,
            normalized_url: None,
            display_url: None,
            content_hash: Some(staged.sha256.clone()),
            inbox_status: InboxStatus::New,
            is_favorite: false,
            is_pinned: false,
            capture_method: CaptureMethod::Explorer,
            capture_session_id: None,
            source_context_id: None,
            created_at: now,
            updated_at: now,
            revision: 1,
            origin_device_id: self.device_id.clone(),
            last_modified_device_id: self.device_id.clone(),
            deleted_at: None,
        };

        // Transaction: item + asset + link + search document
        self.db
            .insert_captured_asset_item(
                item.id.as_str(),
                item.item_type.as_str(),
                &title,
                "explorer",
                self.device_id.as_str(),
                now.as_millis(),
                &staged.sha256,
                staged.media_kind.as_str(),
                &staged.mime_type,
                staged.byte_size as i64,
                &staged.relative_path,
                "explorer",
                &filename,
            )
            .await?;

        tracing::info!(
            event = "explorer_file_captured",
            item_id = %item_id,
            item_type = item.item_type.as_str(),
            byte_count = staged.byte_size,
        );

        Ok(item_id)
    }

    /// Finds the most recently used board (last updated), if any.
    pub async fn last_used_board_id(&self) -> Result<Option<String>, ExplorerCaptureError> {
        let row: Option<(String,)> = sqlx::query_as(
            "SELECT id FROM boards WHERE deleted_at IS NULL ORDER BY updated_at DESC LIMIT 1",
        )
        .fetch_optional(self.db.pool())
        .await?;
        Ok(row.map(|(id,)| id))
    }

    /// Places an already-captured item on a board at a cascaded position.
    pub async fn place_on_board(
        &self,
        board_id: &str,
        item_id: &EntityId,
        index: usize,
    ) -> Result<(), ExplorerCaptureError> {
        let offset = (index as f64) * 40.0;
        let svc = crate::application::board_service::BoardService::new(
            self.db.clone(),
            self.device_id.clone(),
        );
        svc.place_item_on_board(board_id, item_id.as_str(), 100.0 + offset, 100.0 + offset)
            .await
            .map_err(|e| ExplorerCaptureError::Board(e.to_string()))?;
        Ok(())
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn maps_media_kind_to_item_type() {
        assert_eq!(item_type_for_media(MediaKind::Image), ItemType::Image);
        assert_eq!(item_type_for_media(MediaKind::Pdf), ItemType::Pdf);
        assert_eq!(item_type_for_media(MediaKind::Font), ItemType::Font);
        assert_eq!(item_type_for_media(MediaKind::Svg), ItemType::Svg);
        assert_eq!(item_type_for_media(MediaKind::Generic), ItemType::File);
    }

    #[test]
    fn title_strips_extension() {
        assert_eq!(title_from_filename("report.pdf"), "report");
        assert_eq!(title_from_filename("photo.tar.gz"), "photo.tar");
        assert_eq!(title_from_filename("noext"), "noext");
    }

    #[test]
    fn title_strips_directory_path() {
        assert_eq!(title_from_filename(r"C:\Users\me\doc.txt"), "doc");
        assert_eq!(title_from_filename("/home/me/image.png"), "image");
    }

    #[test]
    fn title_handles_hidden_files() {
        // ".gitignore" — dot at position 0 should keep the whole name
        assert_eq!(title_from_filename(".gitignore"), ".gitignore");
    }
}
