//! Asset store — content-addressed binary storage with atomic write protocol.
//!
//! Write protocol:
//! 1. Write source data to temp file under staging/
//! 2. Flush and close
//! 3. Compute SHA-256 and detect MIME
//! 4. Determine content-addressed destination: assets/ab/cd/<sha256>.<ext>
//! 5. Atomic rename (same volume)
//! 6. Return asset metadata for DB transaction
//!
//! On duplicate hash: delete redundant temp, return existing asset ID.

use sha2::{Digest, Sha256};
use std::path::{Path, PathBuf};
use thiserror::Error;

#[derive(Debug, Error)]
pub enum AssetStoreError {
    #[error("I/O error: {0}")]
    Io(#[from] std::io::Error),

    #[error("content too large: {size} bytes exceeds {limit}")]
    TooLarge { size: u64, limit: u64 },

    #[error("unsupported media type")]
    UnsupportedType,

    #[error("unsafe asset path")]
    UnsafePath,
}

/// Result of staging an asset file.
#[derive(Debug, Clone)]
pub struct StagedAsset {
    pub sha256: String,
    pub byte_size: u64,
    pub media_kind: MediaKind,
    pub mime_type: String,
    pub relative_path: String,
    pub is_duplicate: bool,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum MediaKind {
    Image,
    Pdf,
    Font,
    Svg,
    Generic,
}

impl MediaKind {
    pub fn as_str(&self) -> &'static str {
        match self {
            Self::Image => "image",
            Self::Pdf => "pdf",
            Self::Font => "font",
            Self::Svg => "svg",
            Self::Generic => "generic",
        }
    }
}

const MAX_ASSET_SIZE: u64 = 100 * 1024 * 1024; // 100MB

/// Manages content-addressed asset storage.
pub struct AssetStore {
    root: PathBuf,
}

impl AssetStore {
    pub fn new(root: PathBuf) -> Self {
        Self { root }
    }

    /// Ensures the directory structure exists.
    pub fn initialize(&self) -> Result<(), AssetStoreError> {
        std::fs::create_dir_all(self.root.join("assets"))?;
        std::fs::create_dir_all(self.root.join("staging"))?;
        std::fs::create_dir_all(self.root.join("variants"))?;
        Ok(())
    }

    /// Stages binary data: writes to temp, hashes, moves to content-addressed path.
    /// Returns metadata for the DB transaction.
    pub fn stage_bytes(
        &self,
        data: &[u8],
        original_filename: Option<&str>,
    ) -> Result<StagedAsset, AssetStoreError> {
        let size = data.len() as u64;
        if size > MAX_ASSET_SIZE {
            return Err(AssetStoreError::TooLarge {
                size,
                limit: MAX_ASSET_SIZE,
            });
        }

        // SVG is untrusted markup — sanitize before hashing/storing so the
        // content-addressed object is the cleaned payload, not the original.
        let (kind_hint, _, _) = detect_media(data, original_filename);
        let sanitized_svg = if kind_hint == MediaKind::Svg {
            let text = std::str::from_utf8(data).map_err(|_| AssetStoreError::UnsupportedType)?;
            let sanitized = crate::domain::svg_sanitize::sanitize_svg(text)
                .filter(|s| crate::domain::svg_sanitize::is_svg_safe(s))
                .ok_or(AssetStoreError::UnsupportedType)?;
            Some(sanitized.into_bytes())
        } else {
            None
        };
        let data = sanitized_svg.as_deref().unwrap_or(data);
        let size = data.len() as u64;

        // Compute SHA-256
        let mut hasher = Sha256::new();
        hasher.update(data);
        let sha256 = format!("{:x}", hasher.finalize());

        // Detect media kind and MIME
        let (media_kind, mime_type, ext) = detect_media(data, original_filename);

        // Content-addressed relative path: assets/ab/cd/<sha256>.<ext>
        let relative_path = format!(
            "assets/{}/{}/{}.{}",
            &sha256[..2],
            &sha256[2..4],
            sha256,
            ext
        );

        let dest_path = self.root.join(&relative_path);

        // Check if already exists (dedup)
        if dest_path.exists() {
            return Ok(StagedAsset {
                sha256,
                byte_size: size,
                media_kind,
                mime_type,
                relative_path,
                is_duplicate: true,
            });
        }

        // Ensure store layout exists (idempotent; normally created by
        // initialize(), but staging must never fail on a fresh data dir).
        let staging_dir = self.root.join("staging");
        std::fs::create_dir_all(&staging_dir)?;

        // Write to staging temp file
        let temp_path = staging_dir.join(format!("{}.tmp", uuid::Uuid::now_v7()));
        std::fs::write(&temp_path, data)?;

        // Create destination directory and atomic rename
        if let Some(parent) = dest_path.parent() {
            std::fs::create_dir_all(parent)?;
        }
        std::fs::rename(&temp_path, &dest_path)?;

        Ok(StagedAsset {
            sha256,
            byte_size: size,
            media_kind,
            mime_type,
            relative_path,
            is_duplicate: false,
        })
    }

    /// Async entry point for capture pipelines: runs the heavy staging work
    /// (SHA-256 over the payload, SVG sanitization, disk write + atomic
    /// rename) on tokio's blocking pool so multi-megabyte captures never
    /// stall the async runtime. Consumes the store (cheap PathBuf wrapper).
    pub async fn stage_bytes_async(
        self,
        data: Vec<u8>,
        original_filename: Option<String>,
    ) -> Result<StagedAsset, AssetStoreError> {
        tokio::task::spawn_blocking(move || self.stage_bytes(&data, original_filename.as_deref()))
            .await
            .map_err(|e| {
                AssetStoreError::Io(std::io::Error::other(format!("staging task aborted: {e}")))
            })?
    }

    /// Returns the absolute path for a stored asset.
    pub fn resolve_path(&self, relative_path: &str) -> PathBuf {
        self.root.join(relative_path)
    }

    /// SEC-002: validates a stored relative path before resolving it.
    /// Stored paths come from the database; a hostile or corrupted value must
    /// never escape the store root (would otherwise allow reading/exposing
    /// arbitrary files via the reveal/preview commands). Only canonical
    /// content-addressed paths (`assets/<2>/<2>/<sha>.<ext>`) are accepted.
    pub fn resolve_path_checked(&self, relative_path: &str) -> Result<PathBuf, AssetStoreError> {
        if !is_safe_relative(relative_path) {
            return Err(AssetStoreError::UnsafePath);
        }
        Ok(self.root.join(relative_path))
    }

    /// SEC-002 variant: checked resolution for derived files under `variants/`
    /// (thumbnails). Same strict rules as asset paths, different prefix.
    pub fn resolve_variant_path_checked(
        &self,
        relative_path: &str,
    ) -> Result<PathBuf, AssetStoreError> {
        if !is_safe_relative_with_prefix(relative_path, "variants/") {
            return Err(AssetStoreError::UnsafePath);
        }
        Ok(self.root.join(relative_path))
    }

    /// Removes a staged temp file (cleanup on failure).
    pub fn cleanup_temp(&self, temp_path: &Path) {
        let _ = std::fs::remove_file(temp_path);
    }
}

/// SEC-002: strict allow-list for stored asset paths.
/// Valid store paths are exactly `assets/<2 hex>/<2 hex>/<sha256>.<alnum>`;
/// everything else (traversal, absolute, drive letters, ADS, control chars)
/// is rejected before any filesystem access.
fn is_safe_relative(p: &str) -> bool {
    is_safe_relative_with_prefix(p, "assets/")
}

/// Prefix-parameterized variant of the SEC-002 allow-list (assets/ or
/// variants/).
fn is_safe_relative_with_prefix(p: &str, prefix: &str) -> bool {
    if p.is_empty()
        || !p.starts_with(prefix)
        || p.contains("..")
        || p.contains(':')
        || p.contains('\\')
        || p.chars().any(|c| c.is_control())
    {
        return false;
    }
    // Every component must be a plain normal component (no roots/prefixes).
    Path::new(p)
        .components()
        .all(|c| matches!(c, std::path::Component::Normal(_)))
}

/// Detects media kind from magic bytes and filename extension.
fn detect_media(data: &[u8], filename: Option<&str>) -> (MediaKind, String, &'static str) {
    // PNG magic
    if data.starts_with(&[0x89, 0x50, 0x4E, 0x47]) {
        return (MediaKind::Image, "image/png".to_string(), "png");
    }
    // JPEG magic
    if data.starts_with(&[0xFF, 0xD8, 0xFF]) {
        return (MediaKind::Image, "image/jpeg".to_string(), "jpg");
    }
    // GIF magic
    if data.starts_with(b"GIF8") {
        return (MediaKind::Image, "image/gif".to_string(), "gif");
    }
    // WebP magic (RIFF....WEBP)
    if data.len() > 12 && data.starts_with(b"RIFF") && &data[8..12] == b"WEBP" {
        return (MediaKind::Image, "image/webp".to_string(), "webp");
    }
    // PDF magic
    if data.starts_with(b"%PDF") {
        return (MediaKind::Pdf, "application/pdf".to_string(), "pdf");
    }
    // SVG (check if starts with XML-like content containing <svg)
    if let Ok(text) = std::str::from_utf8(&data[..data.len().min(1024)]) {
        if text.contains("<svg") {
            return (MediaKind::Svg, "image/svg+xml".to_string(), "svg");
        }
    }

    // Fallback: use extension
    if let Some(name) = filename {
        let ext = name.rsplit('.').next().unwrap_or("").to_lowercase();
        match ext.as_str() {
            "png" => return (MediaKind::Image, "image/png".to_string(), "png"),
            "jpg" | "jpeg" => return (MediaKind::Image, "image/jpeg".to_string(), "jpg"),
            "gif" => return (MediaKind::Image, "image/gif".to_string(), "gif"),
            "webp" => return (MediaKind::Image, "image/webp".to_string(), "webp"),
            "pdf" => return (MediaKind::Pdf, "application/pdf".to_string(), "pdf"),
            "svg" => return (MediaKind::Svg, "image/svg+xml".to_string(), "svg"),
            "ttf" => return (MediaKind::Font, "font/ttf".to_string(), "ttf"),
            "otf" => return (MediaKind::Font, "font/otf".to_string(), "otf"),
            "woff" => return (MediaKind::Font, "font/woff".to_string(), "woff"),
            "woff2" => return (MediaKind::Font, "font/woff2".to_string(), "woff2"),
            _ => {}
        }
    }

    (
        MediaKind::Generic,
        "application/octet-stream".to_string(),
        "bin",
    )
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn detect_png() {
        let data = [0x89, 0x50, 0x4E, 0x47, 0x0D, 0x0A, 0x1A, 0x0A, 0, 0];
        let (kind, mime, ext) = detect_media(&data, None);
        assert_eq!(kind, MediaKind::Image);
        assert_eq!(mime, "image/png");
        assert_eq!(ext, "png");
    }

    #[test]
    fn detect_pdf() {
        let data = b"%PDF-1.4 some content";
        let (kind, mime, ext) = detect_media(data, None);
        assert_eq!(kind, MediaKind::Pdf);
        assert_eq!(mime, "application/pdf");
        assert_eq!(ext, "pdf");
    }

    #[test]
    fn detect_jpeg() {
        let data = [0xFF, 0xD8, 0xFF, 0xE0, 0, 0];
        let (kind, _, _) = detect_media(&data, None);
        assert_eq!(kind, MediaKind::Image);
    }

    #[test]
    fn detect_by_extension_fallback() {
        let data = b"some binary data";
        let (kind, mime, ext) = detect_media(data, Some("document.pdf"));
        assert_eq!(kind, MediaKind::Pdf);
        assert_eq!(mime, "application/pdf");
        assert_eq!(ext, "pdf");
    }

    #[test]
    fn detect_unknown_generic() {
        let data = b"random bytes";
        let (kind, mime, ext) = detect_media(data, None);
        assert_eq!(kind, MediaKind::Generic);
        assert_eq!(mime, "application/octet-stream");
        assert_eq!(ext, "bin");
    }

    #[test]
    fn stage_and_dedup() {
        let dir = std::env::temp_dir().join(format!("pastecanvas-test-{}", uuid::Uuid::now_v7()));
        let store = AssetStore::new(dir.clone());
        store.initialize().expect("init");

        let data = b"hello world asset content";
        let r1 = store.stage_bytes(data, Some("test.txt")).expect("stage 1");
        assert!(!r1.is_duplicate);
        assert_eq!(r1.byte_size, data.len() as u64);

        // Same content again → duplicate
        let r2 = store.stage_bytes(data, Some("test2.txt")).expect("stage 2");
        assert!(r2.is_duplicate);
        assert_eq!(r1.sha256, r2.sha256);

        // Verify file exists
        assert!(store.resolve_path(&r1.relative_path).exists());

        // Cleanup
        let _ = std::fs::remove_dir_all(&dir);
    }

    // --- SEC-002: checked path resolution ---

    #[test]
    fn checked_resolve_accepts_canonical_paths() {
        let store = AssetStore::new(PathBuf::from("/data/app"));
        let rel =
            "assets/ab/cd/abcdef0123456789abcdef0123456789abcdef0123456789abcdef0123456789.png";
        let abs = store.resolve_path_checked(rel).expect("valid");
        assert!(abs.ends_with(rel));
    }

    #[test]
    fn checked_resolve_rejects_traversal() {
        let store = AssetStore::new(PathBuf::from("/data/app"));
        for hostile in [
            "../secret.txt",
            "assets/../../secret.txt",
            "assets/ab/../../../etc/passwd",
            "assets/..\\windows\\system32",
        ] {
            assert!(
                matches!(
                    store.resolve_path_checked(hostile),
                    Err(AssetStoreError::UnsafePath)
                ),
                "must reject: {hostile}"
            );
        }
    }

    #[test]
    fn checked_resolve_rejects_absolute_and_drive_paths() {
        let store = AssetStore::new(PathBuf::from("/data/app"));
        for hostile in [
            "/etc/passwd",
            "C:\\Windows\\System32\\cmd.exe",
            "assets/ab:evil",
            "\\\\server\\share\\file",
            "",
        ] {
            assert!(
                store.resolve_path_checked(hostile).is_err(),
                "must reject: {hostile}"
            );
        }
    }

    #[test]
    fn checked_resolve_rejects_non_store_prefixes() {
        let store = AssetStore::new(PathBuf::from("/data/app"));
        // Even harmless-looking paths outside the assets/ tree are rejected.
        assert!(store.resolve_path_checked("staging/temp.bin").is_err());
        assert!(store.resolve_path_checked("backups/x.sqlite3").is_err());
    }

    #[test]
    fn variant_paths_accept_thumbnails_and_reject_traversal() {
        let store = AssetStore::new(PathBuf::from("/data/app"));
        let ok = store
            .resolve_variant_path_checked("variants/ab12_thumb_320.png")
            .expect("valid variant");
        assert!(ok.ends_with("variants/ab12_thumb_320.png"));
        assert!(
            store
                .resolve_variant_path_checked("variants/../../secret")
                .is_err()
        );
        assert!(
            store
                .resolve_variant_path_checked("assets/ab/cd/x.png")
                .is_err()
        ); // wrong prefix for the variant resolver
    }

    #[test]
    fn staged_paths_always_pass_the_check() {
        // Round-trip invariant: anything the store itself writes is accepted.
        let dir = std::env::temp_dir().join(format!("pastecanvas-sec-{}", uuid::Uuid::now_v7()));
        let store = AssetStore::new(dir.clone());
        store.initialize().expect("init");
        let staged = store.stage_bytes(b"sec roundtrip", None).expect("stage");
        assert!(store.resolve_path_checked(&staged.relative_path).is_ok());
        let _ = std::fs::remove_dir_all(&dir);
    }

    #[test]
    fn stage_svg_strips_script_before_store() {
        let dir = std::env::temp_dir().join(format!("pastecanvas-svg-{}", uuid::Uuid::now_v7()));
        let store = AssetStore::new(dir.clone());
        store.initialize().expect("init");

        let dirty = br#"<svg xmlns="http://www.w3.org/2000/svg"><script>alert(1)</script><rect width="10" height="10"/></svg>"#;
        let staged = store
            .stage_bytes(dirty, Some("icon.svg"))
            .expect("stage svg");
        assert_eq!(staged.media_kind, MediaKind::Svg);
        let stored = std::fs::read(store.resolve_path(&staged.relative_path)).expect("read");
        let text = String::from_utf8(stored).expect("utf8");
        assert!(!text.contains("script"));
        assert!(!text.contains("alert"));
        assert!(text.contains("rect"));

        let _ = std::fs::remove_dir_all(&dir);
    }
}
