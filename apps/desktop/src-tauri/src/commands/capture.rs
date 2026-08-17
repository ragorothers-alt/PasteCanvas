use serde::{Deserialize, Serialize};
use tauri::State;

use crate::application::capture_service::{CaptureService, CaptureTextRequest};
use crate::bootstrap::app_state::AppState;
use crate::domain::entities::item::CaptureMethod;

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

/// Captures the current clipboard text content — triggered by global hotkey.
#[tauri::command]
pub async fn capture_clipboard(
    app: tauri::AppHandle,
    state: State<'_, AppState>,
    clipboard_text: String,
) -> Result<CaptureTextCommandResponse, String> {
    if clipboard_text.trim().is_empty() {
        return Err("Clipboard is empty".to_string());
    }

    let svc = CaptureService::new(state.db.clone(), state.device_id.clone());

    let result = svc
        .capture_text(CaptureTextRequest {
            text: clipboard_text,
            capture_method: CaptureMethod::ManualHotkey,
            source_app: None,
            window_title: None,
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

#[derive(Debug, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct CaptureImageRequest {
    /// Base64-encoded image bytes (PNG)
    pub data_base64: String,
    #[serde(default)]
    pub source_app: Option<String>,
}

/// Captures an image from base64 data (clipboard image or drag/drop).
#[tauri::command]
pub async fn capture_image(
    app: tauri::AppHandle,
    state: State<'_, AppState>,
    request: CaptureImageRequest,
) -> Result<CaptureTextCommandResponse, String> {
    use crate::domain::value_objects::{EntityId, UnixMs};
    use base64::Engine;

    // Decode base64
    let bytes = base64::engine::general_purpose::STANDARD
        .decode(&request.data_base64)
        .map_err(|e| format!("invalid base64: {e}"))?;

    if bytes.is_empty() {
        return Err("empty image data".to_string());
    }

    // Stage asset
    let asset_store = crate::infrastructure::asset_store::AssetStore::new(state.data_dir.clone());
    let staged = asset_store
        .stage_bytes(&bytes, Some("clipboard_image.png"))
        .map_err(|e| e.to_string())?;

    let now = UnixMs::now();
    let item_id = EntityId::new();

    state
        .db
        .insert_captured_asset_item(
            item_id.as_str(),
            "image",
            "Clipboard Image",
            "manual_hotkey",
            state.device_id.as_str(),
            now.as_millis(),
            &staged.sha256,
            staged.media_kind.as_str(),
            &staged.mime_type,
            staged.byte_size as i64,
            &staged.relative_path,
            request.source_app.as_deref().unwrap_or(""),
            "clipboard_image.png",
        )
        .await
        .map_err(|e| e.to_string())?;

    crate::events::emit_item_captured(&app, item_id.as_str());

    tracing::info!(
        event = "image_captured",
        item_id = %item_id,
        byte_count = bytes.len(),
        is_duplicate = staged.is_duplicate,
    );

    Ok(CaptureTextCommandResponse {
        item_id: item_id.to_string(),
        committed_at: now.as_millis(),
        is_duplicate: staged.is_duplicate,
        duplicate_of: None,
    })
}

/// Captures a screenshot of the entire virtual screen.
#[tauri::command]
pub async fn capture_screenshot(
    app: tauri::AppHandle,
    state: State<'_, AppState>,
) -> Result<CaptureTextCommandResponse, String> {
    use crate::domain::value_objects::{EntityId, UnixMs};

    // Capture screen (platform-specific)
    let raw_data = crate::platform::windows::screenshot::capture_screen()?;

    // Stage as asset
    let asset_store = crate::infrastructure::asset_store::AssetStore::new(state.data_dir.clone());
    let staged = asset_store
        .stage_bytes(&raw_data, Some("screenshot.png"))
        .map_err(|e| e.to_string())?;

    let now = UnixMs::now();
    let item_id = EntityId::new();

    state
        .db
        .insert_captured_asset_item(
            item_id.as_str(),
            "screenshot",
            "Screenshot",
            "screenshot",
            state.device_id.as_str(),
            now.as_millis(),
            &staged.sha256,
            "image",
            "image/png",
            staged.byte_size as i64,
            &staged.relative_path,
            "",
            "screenshot.png",
        )
        .await
        .map_err(|e| e.to_string())?;

    crate::events::emit_item_captured(&app, item_id.as_str());

    tracing::info!(
        event = "screenshot_captured",
        item_id = %item_id,
        byte_count = raw_data.len(),
    );

    Ok(CaptureTextCommandResponse {
        item_id: item_id.to_string(),
        committed_at: now.as_millis(),
        is_duplicate: staged.is_duplicate,
        duplicate_of: None,
    })
}

/// Starts region screenshot mode (CAP-006): opens a borderless fullscreen
/// overlay on the monitor containing the cursor for region selection.
/// ARC-001: window logic lives in platform::window_manager.
#[tauri::command]
pub async fn start_region_capture(app: tauri::AppHandle) -> Result<(), String> {
    crate::platform::window_manager::start_region_overlay(&app)
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

    let png_data = crate::platform::windows::screenshot::capture_region(
        request.x,
        request.y,
        request.width,
        request.height,
    )?;

    // Close overlay after capture
    if let Some(window) =
        app.get_webview_window(crate::platform::window_manager::REGION_OVERLAY_LABEL)
    {
        let _ = window.close();
    }

    // Persist as screenshot item (same pipeline as full-screen capture)
    let asset_store = crate::infrastructure::asset_store::AssetStore::new(state.data_dir.clone());
    let staged = asset_store
        .stage_bytes(&png_data, Some("region_screenshot.png"))
        .map_err(|e| e.to_string())?;

    let now = UnixMs::now();
    let item_id = EntityId::new();

    state
        .db
        .insert_captured_asset_item(
            item_id.as_str(),
            "screenshot",
            "Region Screenshot",
            "screenshot",
            state.device_id.as_str(),
            now.as_millis(),
            &staged.sha256,
            "image",
            "image/png",
            staged.byte_size as i64,
            &staged.relative_path,
            "",
            "region_screenshot.png",
        )
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
