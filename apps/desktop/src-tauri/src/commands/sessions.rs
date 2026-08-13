use serde::Deserialize;
use std::sync::Arc;
use tauri::{Emitter, State};

use crate::application::session_service::{SessionDto, SessionService};
use crate::bootstrap::app_state::AppState;

#[derive(Debug, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct StartSessionRequest {
    #[serde(default)]
    pub name: Option<String>,
    /// CAP-003 optional idle auto-end timeout in minutes (0/None = use setting).
    #[serde(default)]
    pub idle_timeout_minutes: Option<i64>,
}

/// Resolves the effective idle auto-end timeout (ms) for a new session (CAP-003).
/// Priority: explicit request > setting `session.idle_timeout_minutes` (default 30).
async fn resolve_idle_timeout_ms(
    state: &State<'_, AppState>,
    explicit_minutes: Option<i64>,
) -> Option<i64> {
    let minutes = match explicit_minutes {
        Some(m) => m,
        None => {
            let svc = crate::application::settings_service::SettingsService::new(state.db.clone());
            match svc.get_setting("session.idle_timeout_minutes").await {
                Ok(Some(v)) => v.parse::<i64>().unwrap_or(30),
                _ => 30,
            }
        }
    };
    if minutes <= 0 {
        None // disabled
    } else {
        Some(minutes * 60_000)
    }
}

#[tauri::command]
pub async fn start_capture_session(
    app: tauri::AppHandle,
    state: State<'_, AppState>,
    request: StartSessionRequest,
) -> Result<SessionDto, String> {
    let idle_ms = resolve_idle_timeout_ms(&state, request.idle_timeout_minutes).await;
    let svc = SessionService::new(state.db.clone(), state.device_id.clone());
    let result = svc
        .start_session(request.name.as_deref(), idle_ms)
        .await
        .map_err(|e| e.to_string())?;
    // Activate clipboard listener + event-driven watcher for auto-capture
    state.clipboard_listener.start();
    start_clipboard_watcher(&app, &state);
    Ok(result)
}

#[tauri::command]
pub async fn pause_capture_session(state: State<'_, AppState>) -> Result<SessionDto, String> {
    let svc = SessionService::new(state.db.clone(), state.device_id.clone());
    let result = svc.pause_session().await.map_err(|e| e.to_string())?;
    // Deactivate clipboard listener + watcher
    state.clipboard_listener.stop();
    crate::platform::clipboard_watch::stop_clipboard_watch();
    Ok(result)
}

#[tauri::command]
pub async fn resume_capture_session(
    app: tauri::AppHandle,
    state: State<'_, AppState>,
) -> Result<SessionDto, String> {
    let svc = SessionService::new(state.db.clone(), state.device_id.clone());
    let result = svc.resume_session().await.map_err(|e| e.to_string())?;
    // Re-activate clipboard listener + watcher
    state.clipboard_listener.start();
    start_clipboard_watcher(&app, &state);
    Ok(result)
}

#[tauri::command]
pub async fn finish_capture_session(state: State<'_, AppState>) -> Result<SessionDto, String> {
    let svc = SessionService::new(state.db.clone(), state.device_id.clone());
    let result = svc.finish_session().await.map_err(|e| e.to_string())?;
    // Deactivate clipboard listener + watcher
    state.clipboard_listener.stop();
    crate::platform::clipboard_watch::stop_clipboard_watch();
    Ok(result)
}

/// CAP-003: spawns the event-driven clipboard watcher (WM_CLIPBOARDUPDATE).
/// The callback applies the privacy gates BEFORE any content is persisted:
/// sequence dedup → monitor-exclusion format → process exclusion list.
fn start_clipboard_watcher(app: &tauri::AppHandle, state: &State<'_, AppState>) {
    let app_h = app.clone();
    let db = state.db.clone();
    let device = state.device_id.clone();
    let listener_state = state.clipboard_listener.clone();

    let callback = Arc::new(move || {
        let app_h = app_h.clone();
        let db = db.clone();
        let device = device.clone();
        let listener_state = listener_state.clone();
        tauri::async_runtime::spawn(async move {
            use tauri_plugin_clipboard_manager::ClipboardExt;

            // 1. Sequence dedup / listening gate
            let seq = crate::platform::clipboard::listener::get_clipboard_sequence();
            if !listener_state.should_process(seq) {
                return;
            }

            // 2. CAP-004: Windows monitor-exclusion clipboard format — skip
            //    (anonymous reason code only; no content touched).
            if crate::platform::clipboard_watch::has_clipboard_exclusion_marker() {
                tracing::info!(
                    event = "capture_skipped",
                    reason = "monitor_exclusion_format"
                );
                return;
            }

            // 3. CAP-005/004: foreground process metadata + exclusion list
            let proc_name = crate::platform::clipboard::listener::get_foreground_process_name();
            if let Some(ref p) = proc_name {
                let settings =
                    crate::application::settings_service::SettingsService::new(db.clone());
                match settings.is_process_excluded(p).await {
                    Ok(true) => {
                        tracing::info!(event = "capture_skipped", reason = "excluded_process");
                        return;
                    }
                    Ok(false) => {}
                    Err(_) => { /* settings unavailable — capture proceeds */ }
                }
            }

            // 4. Session gate: only capture while a session is ACTIVE
            let session_svc = SessionService::new(db.clone(), device.clone());
            let session = match session_svc.get_current_session().await {
                Ok(Some(s)) if s.state == "active" => s,
                _ => return,
            };

            // 5. Read + capture (content only enters here, after all gates)
            let text = match app_h.clipboard().read_text() {
                Ok(t) => t,
                Err(_) => return,
            };
            if text.trim().is_empty() {
                return;
            }

            let capture_svc = crate::application::capture_service::CaptureService::new(db, device);
            let req = crate::application::capture_service::CaptureTextRequest {
                text,
                capture_method: crate::domain::entities::item::CaptureMethod::CaptureSession,
                source_app: proc_name,
                window_title: None,
            };
            let session_id = crate::domain::value_objects::EntityId::from_string(session.id);
            match capture_svc
                .capture_text_with_session(req, Some(session_id.clone()))
                .await
            {
                Ok(result) => {
                    let _ = app_h.emit("pastecanvas:item-captured", ());
                    tracing::info!(
                        event = "session_capture_success",
                        item_id = %result.item_id,
                        session_id = %session_id
                    );
                }
                Err(e) => {
                    tracing::warn!(event = "session_capture_failed", error = %e);
                }
            }
        });
    });

    if let Err(e) = crate::platform::clipboard_watch::start_clipboard_watch(callback) {
        tracing::error!(event = "clipboard_watcher_start_failed", error = %e);
    }
}

#[tauri::command]
pub async fn get_capture_session(state: State<'_, AppState>) -> Result<Option<SessionDto>, String> {
    let svc = SessionService::new(state.db.clone(), state.device_id.clone());
    svc.get_current_session().await.map_err(|e| e.to_string())
}
