use crate::domain::entities::item::{CaptureMethod, InboxStatus, Item};
use crate::domain::value_objects::{EntityId, ItemType, UnixMs};
use crate::infrastructure::database::pool::AppDatabase;
use sha2::{Digest, Sha256};
use thiserror::Error;

#[derive(Debug, Error)]
pub enum CaptureError {
    #[error("clipboard is empty or unsupported")]
    EmptyClipboard,

    #[error("content too large: {size} bytes")]
    ContentTooLarge { size: usize },

    #[error("database error: {0}")]
    Database(#[from] sqlx::Error),

    #[error("asset store error: {0}")]
    Asset(String),

    #[error("domain validation: {0}")]
    Domain(String),
}

/// Request to capture text content from clipboard or manual input.
#[derive(Debug, Clone)]
pub struct CaptureTextRequest {
    pub text: String,
    pub capture_method: CaptureMethod,
    pub source_app: Option<String>,
    pub window_title: Option<String>,
}

/// Result of a successful capture.
#[derive(Debug, Clone)]
pub struct CaptureResult {
    pub item_id: EntityId,
    pub committed_at: UnixMs,
    pub is_duplicate: bool,
    /// ID of the earliest existing item matching by hash or normalized URL (INB-004).
    /// Per PRD: capture still succeeds; this is an advisory notice.
    pub duplicate_of: Option<EntityId>,
}

const MAX_TEXT_SIZE: usize = 1_000_000; // 1MB text limit

/// CaptureService coordinates the capture pipeline:
/// validation → normalization → dedup check → DB transaction → event.
pub struct CaptureService {
    db: AppDatabase,
    device_id: EntityId,
}

impl CaptureService {
    pub fn new(db: AppDatabase, device_id: EntityId) -> Self {
        Self { db, device_id }
    }

    /// Captures plain text content and persists it durably.
    /// Returns success ONLY after the transaction commits.
    pub async fn capture_text(
        &self,
        req: CaptureTextRequest,
    ) -> Result<CaptureResult, CaptureError> {
        self.capture_text_with_session(req, None).await
    }

    /// CAP-003: capture linked to an active capture session (automatic
    /// clipboard monitoring path). `session_id` is stored on the Item so the
    /// Inbox can group session content together.
    pub async fn capture_text_with_session(
        &self,
        req: CaptureTextRequest,
        capture_session_id: Option<EntityId>,
    ) -> Result<CaptureResult, CaptureError> {
        // Validate
        if req.text.is_empty() {
            return Err(CaptureError::EmptyClipboard);
        }
        if req.text.len() > MAX_TEXT_SIZE {
            return Err(CaptureError::ContentTooLarge {
                size: req.text.len(),
            });
        }

        let now = UnixMs::now();
        let item_id = EntityId::new();

        // Classify: is this a URL?
        let (item_type, normalized_url, display_url) = classify_text(&req.text);

        // Compute content hash for dedup
        let content_hash = compute_text_hash(&req.text);

        // Check for duplicates (advisory only — user may intentionally save again).
        // Per INB-004: dedup matches on hash OR normalized URL; we still save,
        // but return the existing item ID so the UI can offer "open existing".
        let existing_by_hash = self.db.find_item_by_content_hash(&content_hash).await?;
        let existing_by_url = match &normalized_url {
            Some(url) => self.db.find_item_by_normalized_url(url).await?,
            None => None,
        };
        let duplicate_of = existing_by_hash
            .clone()
            .or(existing_by_url)
            .map(EntityId::from_string);
        let is_duplicate = duplicate_of.is_some();

        // Build source context
        let source_context_id = if req.source_app.is_some() || req.window_title.is_some() {
            let sc_id = EntityId::new();
            self.db
                .insert_source_context(
                    &sc_id,
                    req.source_app.as_deref(),
                    req.window_title.as_deref(),
                    None,
                    None,
                    now,
                )
                .await?;
            Some(sc_id)
        } else {
            None
        };

        // Build Item entity
        let item = Item {
            id: item_id.clone(),
            item_type,
            title: derive_title(&req.text, item_type),
            plain_text: Some(req.text.clone()),
            normalized_url: normalized_url.clone(),
            display_url: display_url.clone(),
            content_hash: Some(content_hash.clone()),
            inbox_status: InboxStatus::New,
            is_favorite: false,
            is_pinned: false,
            capture_method: req.capture_method,
            capture_session_id: capture_session_id.clone(),
            source_context_id: source_context_id.clone(),
            created_at: now,
            updated_at: now,
            revision: 1,
            origin_device_id: self.device_id.clone(),
            last_modified_device_id: self.device_id.clone(),
            deleted_at: None,
        };

        item.validate()
            .map_err(|e| CaptureError::Domain(e.to_string()))?;

        // Transaction: Item + SearchDocument + OperationLog + EventOutbox
        self.db
            .insert_item_with_search_doc(
                &item,
                source_context_id.as_ref(),
                req.source_app.as_deref(),
                now,
                None,
            )
            .await?;

        if let Some(ref session_id) = capture_session_id {
            sqlx::query(
                "UPDATE capture_sessions SET updated_at = ? WHERE id = ? AND state = 'active'",
            )
            .bind(now.as_millis())
            .bind(session_id.as_str())
            .execute(self.db.pool())
            .await?;
        }

        tracing::info!(
            event = "item_captured",
            item_id = %item_id,
            item_type = item_type.as_str(),
            byte_count = req.text.len(),
            is_duplicate = is_duplicate,
        );

        // Queue metadata enrichment for URLs (non-blocking, after commit)
        if item_type == ItemType::Link {
            if let Some(ref url) = normalized_url {
                let _ = crate::jobs::job_runner::queue_link_metadata_job(
                    &self.db,
                    item_id.as_str(),
                    url,
                )
                .await;
            }
        }

        Ok(CaptureResult {
            item_id,
            committed_at: now,
            is_duplicate,
            duplicate_of,
        })
    }

    /// CAP-001: captures a PNG image (clipboard image / hotkey fallback).
    /// Stages the bytes into the content-addressed store, then writes
    /// item + asset + search document in one transaction; success is
    /// reported only after commit.
    pub async fn capture_png(
        &self,
        data_dir: &std::path::Path,
        png_bytes: Vec<u8>,
        source_app: Option<String>,
        capture_method: CaptureMethod,
    ) -> Result<CaptureResult, CaptureError> {
        if png_bytes.is_empty() {
            return Err(CaptureError::EmptyClipboard);
        }

        // Stage asset (content-addressed, atomic rename). Ensure store dirs
        // exist (idempotent; matches the Explorer capture path).
        let asset_store =
            crate::infrastructure::asset_store::AssetStore::new(data_dir.to_path_buf());
        asset_store
            .initialize()
            .map_err(|e| CaptureError::Asset(e.to_string()))?;
        let staged = asset_store
            .stage_bytes(&png_bytes, Some("clipboard_image.png"))
            .map_err(|e| CaptureError::Asset(e.to_string()))?;

        let now = UnixMs::now();
        let item_id = EntityId::new();

        // Advisory dedup (INB-004): capture still succeeds.
        let duplicate_of = self
            .db
            .find_item_by_content_hash(&staged.sha256)
            .await?
            .map(EntityId::from_string);
        let is_duplicate = duplicate_of.is_some();

        self.db
            .insert_captured_asset_item(
                item_id.as_str(),
                "image",
                "Clipboard Image",
                capture_method.as_str(),
                self.device_id.as_str(),
                now.as_millis(),
                &staged.sha256,
                staged.media_kind.as_str(),
                &staged.mime_type,
                staged.byte_size as i64,
                &staged.relative_path,
                source_app.as_deref().unwrap_or(""),
                "clipboard_image.png",
            )
            .await?;

        tracing::info!(
            event = "image_captured",
            item_id = %item_id,
            byte_count = png_bytes.len(),
            is_duplicate = staged.is_duplicate,
        );

        Ok(CaptureResult {
            item_id,
            committed_at: now,
            is_duplicate,
            duplicate_of,
        })
    }

    /// Unified persist for a clipboard snapshot (hotkey / tray / session).
    pub async fn capture_clipboard_payload(
        &self,
        payload: crate::platform::clipboard::ClipboardPayload,
        data_dir: &std::path::Path,
        capture_method: CaptureMethod,
        source_app: Option<String>,
        session_id: Option<EntityId>,
    ) -> Result<Vec<CaptureResult>, CaptureError> {
        use crate::platform::clipboard::ClipboardPayload;
        match payload {
            ClipboardPayload::Empty => Err(CaptureError::EmptyClipboard),
            ClipboardPayload::Text(text) => {
                let r = self
                    .capture_text_with_session(
                        CaptureTextRequest {
                            text,
                            capture_method,
                            source_app,
                            window_title: None,
                        },
                        session_id,
                    )
                    .await?;
                Ok(vec![r])
            }
            ClipboardPayload::Html { html, plain } => {
                let r = self
                    .capture_rich_html(html, plain, capture_method, source_app, session_id)
                    .await?;
                Ok(vec![r])
            }
            ClipboardPayload::ImagePng(png) => {
                let r = self
                    .capture_png(data_dir, png, source_app, capture_method)
                    .await?;
                if let Some(ref sid) = session_id {
                    self.attach_session(r.item_id.as_str(), sid).await?;
                }
                Ok(vec![r])
            }
            ClipboardPayload::Files(paths) => {
                let mut out = Vec::new();
                for path in paths {
                    out.push(
                        self.capture_file_path(
                            data_dir,
                            &path,
                            capture_method,
                            source_app.clone(),
                            session_id.clone(),
                        )
                        .await?,
                    );
                }
                if out.is_empty() {
                    Err(CaptureError::EmptyClipboard)
                } else {
                    Ok(out)
                }
            }
        }
    }

    async fn capture_rich_html(
        &self,
        html: String,
        plain: Option<String>,
        capture_method: CaptureMethod,
        source_app: Option<String>,
        session_id: Option<EntityId>,
    ) -> Result<CaptureResult, CaptureError> {
        let sanitized = crate::domain::html_sanitize::sanitize_html(&html);
        if sanitized.trim().is_empty() {
            if let Some(text) = plain {
                return self
                    .capture_text_with_session(
                        CaptureTextRequest {
                            text,
                            capture_method,
                            source_app,
                            window_title: None,
                        },
                        session_id,
                    )
                    .await;
            }
            return Err(CaptureError::EmptyClipboard);
        }
        let plain_text = plain
            .filter(|t| !t.trim().is_empty())
            .unwrap_or_else(|| crate::domain::html_sanitize::html_to_plain_text(&sanitized));
        if plain_text.len() > MAX_TEXT_SIZE || sanitized.len() > MAX_TEXT_SIZE {
            return Err(CaptureError::ContentTooLarge {
                size: plain_text.len().max(sanitized.len()),
            });
        }

        let now = UnixMs::now();
        let item_id = EntityId::new();
        let content_hash = compute_text_hash(&sanitized);
        let duplicate_of = self
            .db
            .find_item_by_content_hash(&content_hash)
            .await?
            .map(EntityId::from_string);
        let is_duplicate = duplicate_of.is_some();
        let title = derive_title(&plain_text, ItemType::RichText);

        let item = Item {
            id: item_id.clone(),
            item_type: ItemType::RichText,
            title,
            plain_text: Some(plain_text),
            normalized_url: None,
            display_url: None,
            content_hash: Some(content_hash),
            inbox_status: InboxStatus::New,
            is_favorite: false,
            is_pinned: false,
            capture_method,
            capture_session_id: session_id.clone(),
            source_context_id: None,
            created_at: now,
            updated_at: now,
            revision: 1,
            origin_device_id: self.device_id.clone(),
            last_modified_device_id: self.device_id.clone(),
            deleted_at: None,
        };
        item.validate()
            .map_err(|e| CaptureError::Domain(e.to_string()))?;

        self.db
            .insert_item_with_search_doc(&item, None, source_app.as_deref(), now, Some(&sanitized))
            .await?;

        if let Some(ref sid) = session_id {
            self.touch_session(sid).await?;
        }

        Ok(CaptureResult {
            item_id,
            committed_at: now,
            is_duplicate,
            duplicate_of,
        })
    }

    async fn capture_file_path(
        &self,
        data_dir: &std::path::Path,
        path: &str,
        capture_method: CaptureMethod,
        source_app: Option<String>,
        session_id: Option<EntityId>,
    ) -> Result<CaptureResult, CaptureError> {
        let file_path = std::path::Path::new(path);
        if !file_path.exists() {
            return Err(CaptureError::Asset(format!("file not found: {path}")));
        }
        let meta = std::fs::metadata(file_path).map_err(|e| CaptureError::Asset(e.to_string()))?;
        if meta.len() > 50 * 1024 * 1024 {
            return Err(CaptureError::ContentTooLarge {
                size: meta.len() as usize,
            });
        }
        let filename = file_path
            .file_name()
            .map(|f| f.to_string_lossy().to_string())
            .unwrap_or_else(|| "file".to_string());
        let bytes = std::fs::read(file_path).map_err(|e| CaptureError::Asset(e.to_string()))?;
        let store = crate::infrastructure::asset_store::AssetStore::new(data_dir.to_path_buf());
        store
            .initialize()
            .map_err(|e| CaptureError::Asset(e.to_string()))?;
        let staged = store
            .stage_bytes(&bytes, Some(&filename))
            .map_err(|e| CaptureError::Asset(e.to_string()))?;

        let now = UnixMs::now();
        let item_id = EntityId::new();
        let item_type = match staged.media_kind {
            crate::infrastructure::asset_store::MediaKind::Image => "image",
            crate::infrastructure::asset_store::MediaKind::Pdf => "pdf",
            crate::infrastructure::asset_store::MediaKind::Font => "font",
            crate::infrastructure::asset_store::MediaKind::Svg => "svg",
            crate::infrastructure::asset_store::MediaKind::Generic => "file",
        };
        let title = filename.rsplit(['/', '\\']).next().unwrap_or(&filename);
        let title = match title.rfind('.') {
            Some(idx) if idx > 0 => &title[..idx],
            _ => title,
        };

        let duplicate_of = self
            .db
            .find_item_by_content_hash(&staged.sha256)
            .await?
            .map(EntityId::from_string);

        self.db
            .insert_captured_asset_item(
                item_id.as_str(),
                item_type,
                title,
                capture_method.as_str(),
                self.device_id.as_str(),
                now.as_millis(),
                &staged.sha256,
                staged.media_kind.as_str(),
                &staged.mime_type,
                staged.byte_size as i64,
                &staged.relative_path,
                source_app.as_deref().unwrap_or(""),
                &filename,
            )
            .await?;

        if let Some(ref sid) = session_id {
            self.attach_session(item_id.as_str(), sid).await?;
        }

        Ok(CaptureResult {
            item_id,
            committed_at: now,
            is_duplicate: duplicate_of.is_some(),
            duplicate_of,
        })
    }

    async fn attach_session(
        &self,
        item_id: &str,
        session_id: &EntityId,
    ) -> Result<(), CaptureError> {
        sqlx::query("UPDATE items SET capture_session_id = ? WHERE id = ?")
            .bind(session_id.as_str())
            .bind(item_id)
            .execute(self.db.pool())
            .await?;
        self.touch_session(session_id).await
    }

    async fn touch_session(&self, session_id: &EntityId) -> Result<(), CaptureError> {
        let now = UnixMs::now();
        sqlx::query("UPDATE capture_sessions SET updated_at = ? WHERE id = ? AND state = 'active'")
            .bind(now.as_millis())
            .bind(session_id.as_str())
            .execute(self.db.pool())
            .await?;
        Ok(())
    }
}

/// Classifies text content: URL → link, color value → color, otherwise plain text.
fn classify_text(text: &str) -> (ItemType, Option<String>, Option<String>) {
    let trimmed = text.trim();
    if (trimmed.starts_with("http://") || trimmed.starts_with("https://"))
        && !trimmed.contains(' ')
        && !trimmed.contains('\n')
    {
        // Normalize: lowercase scheme+host, remove fragment
        let normalized = normalize_url(trimmed);
        (ItemType::Link, Some(normalized), Some(trimmed.to_string()))
    } else if crate::domain::color::is_color_text(trimmed) {
        // Color value detected
        (ItemType::Color, None, None)
    } else {
        (ItemType::PlainText, None, None)
    }
}

fn normalize_url(url: &str) -> String {
    // Simple normalization: remove fragment, trailing slash on bare domain
    let url = url.split('#').next().unwrap_or(url);
    url.to_string()
}

/// Derives a title from content for display purposes.
fn derive_title(text: &str, item_type: ItemType) -> Option<String> {
    match item_type {
        ItemType::Link => Some(text.trim().to_string()),
        _ => {
            let first_line = text.lines().next().unwrap_or("").trim();
            if first_line.is_empty() {
                None
            } else {
                let mut title: String = first_line.chars().take(100).collect();
                if first_line.chars().count() > 100 {
                    title.push('…');
                }
                Some(title)
            }
        }
    }
}

/// Computes a normalized hash for text dedup.
/// Normalizes line endings before hashing.
fn compute_text_hash(text: &str) -> String {
    let normalized = text.replace("\r\n", "\n").replace('\r', "\n");
    let mut hasher = Sha256::new();
    hasher.update(normalized.as_bytes());
    format!("{:x}", hasher.finalize())
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn classify_plain_text() {
        let (t, url, _) = classify_text("Hello world");
        assert_eq!(t, ItemType::PlainText);
        assert!(url.is_none());
    }

    #[test]
    fn classify_url() {
        let (t, url, display) = classify_text("https://example.com/page?q=1");
        assert_eq!(t, ItemType::Link);
        assert!(url.is_some());
        assert!(display.is_some());
    }

    #[test]
    fn classify_url_with_fragment() {
        let (t, url, _) = classify_text("https://example.com/page#section");
        assert_eq!(t, ItemType::Link);
        assert_eq!(url.as_deref(), Some("https://example.com/page"));
    }

    #[test]
    fn multiline_text_is_not_url() {
        let (t, _, _) = classify_text("https://example.com\nsome other text");
        assert_eq!(t, ItemType::PlainText);
    }

    #[test]
    fn classify_color_hex() {
        let (t, _, _) = classify_text("#FF5733");
        assert_eq!(t, ItemType::Color);
    }

    #[test]
    fn classify_color_rgb() {
        let (t, _, _) = classify_text("rgb(255, 87, 51)");
        assert_eq!(t, ItemType::Color);
    }

    #[test]
    fn classify_color_named() {
        let (t, _, _) = classify_text("turquoise");
        assert_eq!(t, ItemType::Color);
    }

    #[test]
    fn derive_title_truncates_long_text() {
        let long = "x".repeat(200);
        let title = derive_title(&long, ItemType::PlainText);
        let t = title.unwrap();
        assert!(t.chars().count() <= 101); // 100 chars + "…"
    }

    #[test]
    fn derive_title_truncates_multibyte_without_panic() {
        let long = "你好🎉".repeat(40);
        let title = derive_title(&long, ItemType::PlainText).unwrap();
        assert!(title.chars().count() <= 101);
        assert!(title.ends_with('…'));
    }

    #[test]
    fn compute_hash_normalizes_line_endings() {
        let h1 = compute_text_hash("line1\r\nline2");
        let h2 = compute_text_hash("line1\nline2");
        assert_eq!(h1, h2);
    }
}
