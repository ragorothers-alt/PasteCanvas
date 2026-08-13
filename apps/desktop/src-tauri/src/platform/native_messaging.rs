//! Chrome/Edge Native Messaging protocol (CAP-008).
//!
//! Per the Native Messaging spec, each message over stdin/stdout is framed as:
//!   [4-byte little-endian length][UTF-8 JSON payload]
//!
//! This module provides framing primitives + the message schemas exchanged
//! between the browser extension and the PasteCanvas native messaging host.
//! The framing is platform-independent and fully unit-testable.

use serde::{Deserialize, Serialize};
use std::io::{self, Read, Write};
use thiserror::Error;

/// Maximum message size accepted (1 MB) — Native Messaging hard limit is 1 MB/host msg.
pub const MAX_MESSAGE_BYTES: usize = 1024 * 1024;

/// Current local IPC protocol version (WIN-006). The host rejects requests
/// carrying a higher version so a future protocol change cannot be silently
/// misinterpreted; version 1 is also accepted when the field is absent
/// (older extensions).
pub const PROTOCOL_VERSION: u32 = 1;

/// Capture kinds accepted by the host (strict whitelist — WIN-006).
pub const CAPTURE_KINDS: &[&str] = &["page", "selection", "link", "image", "video"];

/// Validates a capture request against the protocol contract (WIN-006).
/// Rejects unknown protocol versions and kinds not on the whitelist.
pub fn validate_capture_request(req: &BrowserCaptureRequest) -> Result<(), String> {
    if req.protocol_version > PROTOCOL_VERSION {
        return Err(format!(
            "unsupported protocol version {} (host supports up to {PROTOCOL_VERSION})",
            req.protocol_version
        ));
    }
    if !CAPTURE_KINDS.contains(&req.kind.as_str()) {
        return Err(format!("unknown capture kind: {}", req.kind));
    }
    Ok(())
}

#[derive(Debug, Error)]
pub enum NativeMessagingError {
    #[error("I/O error: {0}")]
    Io(#[from] io::Error),

    #[error("message too large: {size} bytes")]
    TooLarge { size: usize },

    #[error("invalid JSON: {0}")]
    Json(#[from] serde_json::Error),
}

/// Reads one length-prefixed message from a reader. Returns None on clean EOF.
pub fn read_message<R: Read>(reader: &mut R) -> Result<Option<Vec<u8>>, NativeMessagingError> {
    // Read the 4-byte little-endian length prefix
    let mut len_buf = [0u8; 4];
    match reader.read_exact(&mut len_buf) {
        Ok(()) => {}
        Err(e) if e.kind() == io::ErrorKind::UnexpectedEof => return Ok(None),
        Err(e) => return Err(e.into()),
    }

    let len = u32::from_le_bytes(len_buf) as usize;
    if len > MAX_MESSAGE_BYTES {
        return Err(NativeMessagingError::TooLarge { size: len });
    }

    let mut buf = vec![0u8; len];
    reader.read_exact(&mut buf)?;
    Ok(Some(buf))
}

/// Writes one length-prefixed message to a writer.
pub fn write_message<W: Write>(writer: &mut W, payload: &[u8]) -> Result<(), NativeMessagingError> {
    if payload.len() > MAX_MESSAGE_BYTES {
        return Err(NativeMessagingError::TooLarge {
            size: payload.len(),
        });
    }
    writer.write_all(&(payload.len() as u32).to_le_bytes())?;
    writer.write_all(payload)?;
    writer.flush()?;
    Ok(())
}

/// A capture request sent from the browser extension to the host (CAP-008).
/// Supports: save page, save selection + source URL, save link, save image, save video.
#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct BrowserCaptureRequest {
    /// Protocol version (WIN-006). Defaults to 1 for older extensions.
    #[serde(default = "default_protocol_version")]
    pub protocol_version: u32,
    /// Client-generated correlation id, echoed back in the response (WIN-006).
    #[serde(default)]
    pub request_id: Option<String>,
    /// One of: "page", "selection", "link", "image", "video".
    pub kind: String,
    /// Selected text or page title.
    #[serde(default)]
    pub text: Option<String>,
    /// The URL being captured (page URL, link href, image src, video src).
    #[serde(default)]
    pub url: Option<String>,
    /// Source page URL (for selection/link/image context).
    #[serde(default)]
    pub source_url: Option<String>,
    /// Page title if available.
    #[serde(default)]
    pub title: Option<String>,
}

fn default_protocol_version() -> u32 {
    1
}

/// Response sent back to the browser extension.
#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct BrowserCaptureResponse {
    pub ok: bool,
    #[serde(default)]
    pub item_id: Option<String>,
    #[serde(default)]
    pub error: Option<String>,
    /// Echoed request correlation id (WIN-006).
    #[serde(default)]
    pub request_id: Option<String>,
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::io::Cursor;

    #[test]
    fn round_trip_message() {
        let payload = br#"{"kind":"page"}"#;
        let mut buf = Vec::new();
        write_message(&mut buf, payload).expect("write");

        let mut reader = Cursor::new(buf);
        let read = read_message(&mut reader).expect("read").expect("some");
        assert_eq!(read, payload);
    }

    #[test]
    fn read_empty_returns_none() {
        let mut reader = Cursor::new(Vec::<u8>::new());
        assert!(read_message(&mut reader).expect("ok").is_none());
    }

    #[test]
    fn length_prefix_is_little_endian() {
        let payload = b"abcd"; // 4 bytes
        let mut buf = Vec::new();
        write_message(&mut buf, payload).expect("write");
        // First 4 bytes = length 4 in little-endian
        assert_eq!(&buf[0..4], &[4, 0, 0, 0]);
        assert_eq!(&buf[4..], b"abcd");
    }

    #[test]
    fn rejects_oversized_message_on_write() {
        let payload = vec![0u8; MAX_MESSAGE_BYTES + 1];
        let mut buf = Vec::new();
        let result = write_message(&mut buf, &payload);
        assert!(matches!(result, Err(NativeMessagingError::TooLarge { .. })));
    }

    #[test]
    fn rejects_oversized_message_on_read() {
        // Craft a header claiming 2 MB, but provide no body
        let mut buf = Vec::new();
        buf.extend_from_slice(&((MAX_MESSAGE_BYTES as u32) + 1).to_le_bytes());
        let mut reader = Cursor::new(buf);
        let result = read_message(&mut reader);
        assert!(matches!(result, Err(NativeMessagingError::TooLarge { .. })));
    }

    #[test]
    fn multiple_messages_in_stream() {
        let mut buf = Vec::new();
        write_message(&mut buf, b"first").expect("w1");
        write_message(&mut buf, b"second").expect("w2");

        let mut reader = Cursor::new(buf);
        let m1 = read_message(&mut reader).expect("r1").expect("some");
        let m2 = read_message(&mut reader).expect("r2").expect("some");
        let m3 = read_message(&mut reader).expect("r3");
        assert_eq!(m1, b"first");
        assert_eq!(m2, b"second");
        assert!(m3.is_none());
    }

    #[test]
    fn browser_capture_request_serializes_camel_case() {
        let req = BrowserCaptureRequest {
            protocol_version: PROTOCOL_VERSION,
            request_id: None,
            kind: "selection".to_string(),
            text: Some("hello".to_string()),
            url: None,
            source_url: Some("https://example.com".to_string()),
            title: Some("Example".to_string()),
        };
        let json = serde_json::to_string(&req).expect("ser");
        assert!(json.contains("sourceUrl"));
        assert!(!json.contains("source_url"));
    }

    #[test]
    fn browser_capture_request_deserializes_with_defaults() {
        let json = r#"{"kind":"link","url":"https://example.com"}"#;
        let req: BrowserCaptureRequest = serde_json::from_str(json).expect("de");
        assert_eq!(req.kind, "link");
        assert_eq!(req.url.as_deref(), Some("https://example.com"));
        assert!(req.text.is_none());
        assert!(req.source_url.is_none());
        // WIN-006: version defaults to 1 for older extensions
        assert_eq!(req.protocol_version, 1);
        assert!(req.request_id.is_none());
    }

    #[test]
    fn validates_protocol_version() {
        // WIN-006: future versions are rejected explicitly
        let json = r#"{"protocolVersion":2,"kind":"page"}"#;
        let req: BrowserCaptureRequest = serde_json::from_str(json).expect("de");
        let err = validate_capture_request(&req).expect_err("v2");
        assert!(err.contains("protocol version"));
    }

    #[test]
    fn validates_kind_whitelist() {
        let req = BrowserCaptureRequest {
            protocol_version: 1,
            request_id: None,
            kind: "evil".to_string(),
            text: None,
            url: Some("https://example.com".to_string()),
            source_url: None,
            title: None,
        };
        let err = validate_capture_request(&req).expect_err("kind");
        assert!(err.contains("unknown capture kind"));
    }

    #[test]
    fn accepts_current_version_and_whitelisted_kinds() {
        for kind in CAPTURE_KINDS {
            let req = BrowserCaptureRequest {
                protocol_version: PROTOCOL_VERSION,
                request_id: Some("req-1".to_string()),
                kind: (*kind).to_string(),
                text: None,
                url: None,
                source_url: None,
                title: None,
            };
            assert!(validate_capture_request(&req).is_ok());
        }
    }
}
