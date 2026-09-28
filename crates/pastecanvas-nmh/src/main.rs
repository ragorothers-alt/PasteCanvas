//! PasteCanvas Native Messaging Host (CAP-008).
//!
//! A standalone binary that the browser extension talks to over Chrome/Edge
//! Native Messaging (length-prefixed JSON on stdin/stdout). It captures content
//! into the same SQLite database the desktop app uses — no cloud, fully local.
//!
//! Per PRD CAP-008: "使用 Manifest V3 + Native Messaging，不要求雲端服務."

use pastecanvas_desktop_lib::application::capture_service::{CaptureService, CaptureTextRequest};
use pastecanvas_desktop_lib::bootstrap::external;
use pastecanvas_desktop_lib::domain::entities::item::CaptureMethod;
use pastecanvas_desktop_lib::domain::value_objects::EntityId;
use pastecanvas_desktop_lib::platform::native_messaging::{
    BrowserCaptureRequest, BrowserCaptureResponse, read_message, validate_capture_request,
    write_message,
};
use std::io::{self, BufReader};

/// Maps a browser capture request to the text content to persist.
fn content_for_request(req: &BrowserCaptureRequest) -> Option<String> {
    match req.kind.as_str() {
        // Page / link / video / image: capture the URL itself
        "page" | "link" | "video" | "image" => req.url.clone(),
        // Selection: capture the selected text (falls back to source URL)
        "selection" => req
            .text
            .clone()
            .filter(|t| !t.trim().is_empty())
            .or_else(|| req.source_url.clone()),
        _ => req.url.clone().or_else(|| req.text.clone()),
    }
}

fn main() {
    let stdin = io::stdin();
    let mut reader = BufReader::new(stdin.lock());
    let stdout = io::stdout();

    // Process messages until EOF
    loop {
        let msg = match read_message(&mut reader) {
            Ok(Some(m)) => m,
            Ok(None) => break, // clean EOF
            Err(e) => {
                respond_error(&mut stdout.lock(), &format!("read message: {e}"));
                break;
            }
        };

        let response = process_message(&msg);
        let payload = serde_json::to_vec(&response).unwrap_or_default();
        let mut out = stdout.lock();
        if let Err(e) = write_message(&mut out, &payload) {
            eprintln!("nmh: write response failed: {e}");
            break;
        }
    }
}

/// Processes a single capture message, returning the response.
fn process_message(msg: &[u8]) -> BrowserCaptureResponse {
    let req: BrowserCaptureRequest = match serde_json::from_slice(msg) {
        Ok(r) => r,
        Err(e) => {
            return BrowserCaptureResponse {
                ok: false,
                item_id: None,
                error: Some(format!("invalid request: {e}")),
                request_id: None,
            };
        }
    };
    let request_id = req.request_id.clone();

    // WIN-006: enforce protocol version + kind whitelist before any work.
    if let Err(e) = validate_capture_request(&req) {
        return BrowserCaptureResponse {
            ok: false,
            item_id: None,
            error: Some(e),
            request_id,
        };
    }

    let content = match content_for_request(&req) {
        Some(c) if !c.trim().is_empty() => c,
        _ => {
            return BrowserCaptureResponse {
                ok: false,
                item_id: None,
                error: Some("nothing to capture".to_string()),
                request_id,
            };
        }
    };

    // Run the capture in a blocking tokio runtime (NMH is short-lived per invocation)
    let rt = match tokio::runtime::Builder::new_current_thread()
        .enable_all()
        .build()
    {
        Ok(rt) => rt,
        Err(e) => {
            return BrowserCaptureResponse {
                ok: false,
                item_id: None,
                error: Some(format!("runtime: {e}")),
                request_id,
            };
        }
    };

    let result: Result<String, String> = rt.block_on(async {
        let (db, _data_dir) = external::open_migrated_db().await?;
        let svc = CaptureService::new(db, EntityId::new());
        let captured = svc
            .capture_text(CaptureTextRequest {
                text: content,
                capture_method: CaptureMethod::Browser,
                source_app: Some("browser".to_string()),
                window_title: req.title.clone(),
            })
            .await
            .map_err(|e| format!("capture: {e}"))?;
        Ok(captured.item_id.to_string())
    });

    match result {
        Ok(item_id) => {
            // CAP-009: tell the running app about the new item (best effort).
            if let Ok(exe) = std::env::current_exe() {
                pastecanvas_desktop_lib::platform::notify::notify_capture_app(&exe);
            }
            BrowserCaptureResponse {
                ok: true,
                item_id: Some(item_id),
                error: None,
                request_id,
            }
        }
        Err(e) => BrowserCaptureResponse {
            ok: false,
            item_id: None,
            error: Some(e),
            request_id,
        },
    }
}

/// Writes an error response (used for fatal startup failures).
fn respond_error<W: io::Write>(out: &mut W, msg: &str) {
    let resp = BrowserCaptureResponse {
        ok: false,
        item_id: None,
        error: Some(msg.to_string()),
        request_id: None,
    };
    if let Ok(payload) = serde_json::to_vec(&resp) {
        let _ = write_message(out, &payload);
    }
}
