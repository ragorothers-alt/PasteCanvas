use serde::{Deserialize, Serialize};
use tauri::State;

use crate::application::capture_service::{CaptureService, CaptureTextRequest};
use crate::bootstrap::app_state::AppState;
use crate::domain::entities::item::CaptureMethod;
use crate::infrastructure::database::pool::CapturedAssetInsert;

#[derive(Debug, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct CaptureTextCommandRequest {
    pub text: String,
    #[serde(default)]
    pub source_app: Option<String>,
    #[serde(default)]
    pub window_title: Option<String>,
    /// Optional capture method override (e.g., "canvas_paste" for BRD-002).
    #[serde(default)]
    pub capture_method: Option<String>,
}

/// Parses a capture method string, defaulting to QuickCapture.
fn parse_capture_method(s: Option<&str>) -> CaptureMethod {
    match s {
        Some("canvas_paste") => CaptureMethod::CanvasPaste,
        Some("manual_hotkey") => CaptureMethod::ManualHotkey,
        Some("capture_session") => CaptureMethod::CaptureSession,
        Some("drag_drop") => CaptureMethod::DragDrop,
        Some("browser") => CaptureMethod::Browser,
        Some("explorer") => CaptureMethod::Explorer,
        _ => CaptureMethod::QuickCapture,
    }
}

#[derive(Debug, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct CaptureTextCommandResponse {
    pub item_id: String,
    pub committed_at: i64,
    pub is_duplicate: bool,
    /// ID of the existing item when duplicate detected (INB-004 dedup notice).
    pub duplicate_of: Option<String>,
}

/// Captures text content — called from frontend Quick Capture or hotkey handler.
#[tauri::command]
pub async fn capture_text(
    app: tauri::AppHandle,
    state: State<'_, AppState>,
    request: CaptureTextCommandRequest,
) -> Result<CaptureTextCommandResponse, String> {
    let svc = CaptureService::new(state.db.clone(), state.device_id.clone());

    let result = svc
        .capture_text(CaptureTextRequest {
            text: request.text,
            capture_method: parse_capture_method(request.capture_method.as_deref()),
            source_app: request.source_app,
            window_title: request.window_title,
        })
        .await
        .map_err(|e| e.to_string())?;

    crate::events::emit_item_captured(&app, result.item_id.as_str());

    Ok(CaptureTextCommandResponse {
        item_id: result.item_id.to_string(),
        committed_at: result.committed_at.as_millis(),
        is_duplicate: result.is_duplicate,
        duplicate_of: result.duplicate_of.map(|id| id.to_string()),
    })
}

/// Region capture context for coordinate conversion in the overlay frontend.
#[derive(Debug, serde::Serialize)]
#[serde(rename_all = "camelCase")]
pub struct RegionCaptureContext {
    /// Monitor physical origin (virtual screen coordinates).
    pub origin_x: i32,
    pub origin_y: i32,
    /// Monitor scale factor (CSS px → physical px multiplier).
    pub scale_factor: f64,
}

/// Returns the overlay window's monitor origin + scale factor so the frontend
/// can convert CSS-pixel selections to virtual-screen physical coordinates.
#[tauri::command]
pub async fn get_region_capture_context(
    app: tauri::AppHandle,
) -> Result<RegionCaptureContext, String> {
    use tauri::Manager;
    let window = app
        .get_webview_window(crate::platform::window_manager::REGION_OVERLAY_LABEL)
        .ok_or_else(|| "region overlay not open".to_string())?;
    let monitor = window
        .current_monitor()
        .map_err(|e| e.to_string())?
        .ok_or_else(|| "no monitor for overlay".to_string())?;
    let pos = monitor.position();
    Ok(RegionCaptureContext {
        origin_x: pos.x,
        origin_y: pos.y,
        scale_factor: monitor.scale_factor(),
    })
}

#[derive(Debug, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct CaptureRegionRequest {
    /// Virtual-screen physical coordinates (converted by overlay frontend).
    pub x: i32,
    pub y: i32,
    pub width: i32,
    pub height: i32,
}

/// Captures the selected region, saves it as a screenshot item, closes the overlay (CAP-006).
#[tauri::command]
pub async fn capture_region_screenshot(
    state: State<'_, AppState>,
    app: tauri::AppHandle,
    request: CaptureRegionRequest,
) -> Result<CaptureTextCommandResponse, String> {
    use crate::domain::value_objects::{EntityId, UnixMs};
    use tauri::Manager;

    if request.width <= 0 || request.height <= 0 {
        return Err("invalid region size".to_string());
    }

    // Hide overlay before capture so the selection UI is not in the shot
    if let Some(window) =
        app.get_webview_window(crate::platform::window_manager::REGION_OVERLAY_LABEL)
    {
        let _ = window.hide();
    }
    // Let the compositor repaint without the overlay
    tokio::time::sleep(std::time::Duration::from_millis(80)).await;

    let region = (request.x, request.y, request.width, request.height);
    let png_data = tokio::task::spawn_blocking(move || {
        crate::platform::windows::screenshot::capture_region(region.0, region.1, region.2, region.3)
    })
    .await
    .map_err(|e| e.to_string())??;

    // Close overlay after capture
    if let Some(window) =
        app.get_webview_window(crate::platform::window_manager::REGION_OVERLAY_LABEL)
    {
        let _ = window.close();
    }

    // Persist as screenshot item (same pipeline as full-screen capture)
    let asset_store = crate::infrastructure::asset_store::AssetStore::new(state.data_dir.clone());
    let staged = asset_store
        .stage_bytes_async(png_data, Some("region_screenshot.png".to_string()))
        .await
        .map_err(|e| e.to_string())?;

    let now = UnixMs::now();
    let item_id = EntityId::new();

    state
        .db
        .insert_captured_asset_item(&CapturedAssetInsert {
            item_id: item_id.as_str(),
            item_type: "screenshot",
            title: "Region Screenshot",
            capture_method: "screenshot",
            device_id: state.device_id.as_str(),
            now_ms: now.as_millis(),
            sha256: &staged.sha256,
            media_kind: "image",
            mime_type: "image/png",
            byte_size: staged.byte_size as i64,
            relative_path: &staged.relative_path,
            source_app: "",
            filename: "region_screenshot.png",
        })
        .await
        .map_err(|e| e.to_string())?;

    crate::events::emit_item_captured(&app, item_id.as_str());

    tracing::info!(
        event = "region_screenshot_captured",
        item_id = %item_id,
        width = request.width,
        height = request.height,
    );

    Ok(CaptureTextCommandResponse {
        item_id: item_id.to_string(),
        committed_at: now.as_millis(),
        is_duplicate: staged.is_duplicate,
        duplicate_of: None,
    })
}

/// Cancels region capture (CAP-006: 支援取消) — closes the overlay without saving.
#[tauri::command]
pub async fn cancel_region_capture(app: tauri::AppHandle) -> Result<(), String> {
    use tauri::Manager;
    if let Some(window) =
        app.get_webview_window(crate::platform::window_manager::REGION_OVERLAY_LABEL)
    {
        window.close().map_err(|e| e.to_string())?;
        tracing::info!(event = "region_capture_cancelled");
    }
    Ok(())
}
