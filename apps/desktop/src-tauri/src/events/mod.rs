//! Events — in-process event bus for UI refresh and integration.
//! Events are dispatched after transaction commit.
//!
//! One naming convention: `paste-canvas://<kebab-action>`.

use tauri::{AppHandle, Emitter};

pub const ITEM_CAPTURED: &str = "paste-canvas://item-captured";
pub const CAPTURE_SUCCESS: &str = "paste-canvas://capture-success";
pub const CAPTURE_FAILED: &str = "paste-canvas://capture-failed";
pub const SESSION_SUSPENDED: &str = "paste-canvas://session-suspended";
pub const SHOW_SETTINGS: &str = "paste-canvas://show-settings";
pub const FOCUS_SEARCH: &str = "paste-canvas://focus-search";
pub const TOGGLE_CAPTURE_PAUSE: &str = "paste-canvas://toggle-capture-pause";

pub fn emit_item_captured(app: &AppHandle, item_id: &str) {
    let _ = app.emit(ITEM_CAPTURED, ());
    let _ = app.emit(CAPTURE_SUCCESS, item_id);
}

pub fn emit_capture_failed(app: &AppHandle, reason: &str) {
    let _ = app.emit(CAPTURE_FAILED, reason);
}
