use crate::domain::value_objects::{EntityId, ItemType, UnixMs};

/// Core Item entity representing captured content.
#[derive(Debug, Clone)]
pub struct Item {
    pub id: EntityId,
    pub item_type: ItemType,
    pub title: Option<String>,
    pub plain_text: Option<String>,
    pub normalized_url: Option<String>,
    pub display_url: Option<String>,
    pub content_hash: Option<String>,
    pub inbox_status: InboxStatus,
    pub is_favorite: bool,
    pub is_pinned: bool,
    pub capture_method: CaptureMethod,
    pub capture_session_id: Option<EntityId>,
    pub source_context_id: Option<EntityId>,
    pub created_at: UnixMs,
    pub updated_at: UnixMs,
    pub revision: i64,
    pub origin_device_id: EntityId,
    pub last_modified_device_id: EntityId,
    pub deleted_at: Option<UnixMs>,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum InboxStatus {
    New,
    Triaged,
    Archived,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum CaptureMethod {
    ManualHotkey,
    QuickCapture,
    CaptureSession,
    CanvasPaste,
    DragDrop,
    Screenshot,
    Explorer,
    Browser,
    Import,
    ManualCreate,
}

impl CaptureMethod {
    /// Stable persistence string for the capture method.
    pub fn as_str(&self) -> &'static str {
        use CaptureMethod::*;
        match self {
            ManualHotkey => "manual_hotkey",
            QuickCapture => "quick_capture",
            CaptureSession => "capture_session",
            CanvasPaste => "canvas_paste",
            DragDrop => "drag_drop",
            Screenshot => "screenshot",
            Explorer => "explorer",
            Browser => "browser",
            Import => "import",
            ManualCreate => "manual_create",
        }
    }
}

impl Item {
    /// Validates domain invariants for a new Item.
    pub fn validate(&self) -> Result<(), crate::domain::error::DomainError> {
        if self.revision < 1 {
            return Err(crate::domain::error::DomainError::InvalidRevision(
                self.revision,
            ));
        }
        if let Some(ref title) = self.title {
            if title.len() > 2000 {
                return Err(crate::domain::error::DomainError::TitleTooLong(title.len()));
            }
        }
        Ok(())
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::domain::value_objects::{EntityId, ItemType, UnixMs};

    fn make_test_item() -> Item {
        Item {
            id: EntityId::new(),
            item_type: ItemType::PlainText,
            title: Some("Test".to_string()),
            plain_text: Some("Hello world".to_string()),
            normalized_url: None,
            display_url: None,
            content_hash: None,
            inbox_status: InboxStatus::New,
            is_favorite: false,
            is_pinned: false,
            capture_method: CaptureMethod::ManualHotkey,
            capture_session_id: None,
            source_context_id: None,
            created_at: UnixMs::now(),
            updated_at: UnixMs::now(),
            revision: 1,
            origin_device_id: EntityId::new(),
            last_modified_device_id: EntityId::new(),
            deleted_at: None,
        }
    }

    #[test]
    fn valid_item_passes_validation() {
        let item = make_test_item();
        assert!(item.validate().is_ok());
    }

    #[test]
    fn item_with_zero_revision_fails() {
        let mut item = make_test_item();
        item.revision = 0;
        assert!(item.validate().is_err());
    }

    #[test]
    fn item_with_long_title_fails() {
        let mut item = make_test_item();
        item.title = Some("x".repeat(2001));
        assert!(item.validate().is_err());
    }

    #[test]
    fn capture_method_persistence_strings_are_stable() {
        // These strings are persisted in the DB — they must never change.
        assert_eq!(CaptureMethod::ManualHotkey.as_str(), "manual_hotkey");
        assert_eq!(CaptureMethod::QuickCapture.as_str(), "quick_capture");
        assert_eq!(CaptureMethod::CaptureSession.as_str(), "capture_session");
        assert_eq!(CaptureMethod::CanvasPaste.as_str(), "canvas_paste");
        assert_eq!(CaptureMethod::DragDrop.as_str(), "drag_drop");
        assert_eq!(CaptureMethod::Screenshot.as_str(), "screenshot");
        assert_eq!(CaptureMethod::Explorer.as_str(), "explorer");
        assert_eq!(CaptureMethod::Browser.as_str(), "browser");
        assert_eq!(CaptureMethod::Import.as_str(), "import");
        assert_eq!(CaptureMethod::ManualCreate.as_str(), "manual_create");
    }
}
