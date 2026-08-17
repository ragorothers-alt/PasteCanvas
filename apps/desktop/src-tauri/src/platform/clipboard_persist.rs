//! Shared persist path for clipboard snapshots (hotkey / tray / session).

use tauri::{AppHandle, Manager};

use crate::application::capture_service::CaptureService;
use crate::bootstrap::app_state::AppState;
use crate::domain::entities::item::CaptureMethod;
use crate::domain::value_objects::EntityId;
use crate::platform::clipboard::ClipboardPayload;

/// Persists a previously-read clipboard snapshot and emits capture events.
/// `fail_loud` surfaces empty/errors to the user (hotkey/tray); session
/// captures stay silent on empty clipboards.
pub async fn persist_and_emit(
    app: &AppHandle,
    payload: ClipboardPayload,
    method: CaptureMethod,
    source_app: Option<String>,
    session_id: Option<EntityId>,
    fail_loud: bool,
) {
    if matches!(payload, ClipboardPayload::Empty) {
        if fail_loud {
            crate::events::emit_capture_failed(app, "Clipboard is empty");
        }
        return;
    }

    let state = app.state::<AppState>();
    let svc = CaptureService::new(state.db.clone(), state.device_id.clone());
    let data_dir = state.data_dir.clone();

    match svc
        .capture_clipboard_payload(payload, &data_dir, method, source_app, session_id)
        .await
    {
        Ok(results) => {
            for result in &results {
                crate::events::emit_item_captured(app, result.item_id.as_str());
                tracing::info!(event = "clipboard_capture_success", item_id = %result.item_id);
            }
        }
        Err(e) => {
            tracing::warn!(event = "clipboard_capture_failed", error = %e);
            if fail_loud {
                crate::events::emit_capture_failed(app, &e.to_string());
            }
        }
    }
}
