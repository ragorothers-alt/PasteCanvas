//! Window management primitives (ARC-001).
//!
//! Pure Tauri-window operations over `AppHandle`. The commands layer wraps
//! these for the frontend, while platform adapters (tray, global hotkeys)
//! call them directly. Keeping this logic out of `commands` preserves layer
//! directionality: platform may use application services but must never
//! depend on the commands layer.

use tauri::{Manager, WebviewUrl, WebviewWindowBuilder};

const QUICK_CAPTURE_WIDTH_LOGICAL: f64 = 460.0;
const QUICK_CAPTURE_HEIGHT_LOGICAL: f64 = 280.0;
const QUICK_CAPTURE_CURSOR_OFFSET_PHYSICAL: i32 = 16;

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
struct MonitorBounds {
    x: i32,
    y: i32,
    width: i32,
    height: i32,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
struct WindowSize {
    width: i32,
    height: i32,
}

/// Calculates a cursor-adjacent top-left position in physical screen pixels.
///
/// Tauri exposes monitor position/size here, but this version does not expose
/// a work-area/taskbar rectangle. We therefore clamp to the monitor's physical
/// bounds; this is explicit rather than pretending those bounds exclude the
/// taskbar.
fn quick_capture_position(
    cursor_x: i32,
    cursor_y: i32,
    monitor: MonitorBounds,
    window: WindowSize,
    offset: i32,
) -> (i32, i32) {
    fn position_on_axis(
        cursor: i32,
        origin: i32,
        monitor_size: i32,
        window_size: i32,
        offset: i32,
    ) -> i32 {
        // A window that fills or exceeds an axis cannot be placed within its
        // monitor. Anchor it at the origin rather than passing an inverted
        // range to `clamp` (which panics). Non-positive monitor dimensions are
        // degenerate, so use the same safe origin fallback.
        if monitor_size <= 0 || window_size >= monitor_size {
            return origin;
        }

        let max = origin
            .saturating_add(monitor_size)
            .saturating_sub(window_size);
        let preferred = cursor.saturating_add(offset);
        let flipped = cursor.saturating_sub(window_size).saturating_sub(offset);
        let position = if preferred <= max { preferred } else { flipped };

        position.clamp(origin, max)
    }

    (
        position_on_axis(cursor_x, monitor.x, monitor.width, window.width, offset),
        position_on_axis(cursor_y, monitor.y, monitor.height, window.height, offset),
    )
}

fn quick_capture_physical_position(app: &tauri::AppHandle) -> Result<(i32, i32), String> {
    let cursor = app.cursor_position().map_err(|e| e.to_string())?;
    let monitors = app.available_monitors().map_err(|e| e.to_string())?;
    let target = monitors
        .iter()
        .find(|monitor| {
            let position = monitor.position();
            let size = monitor.size();
            cursor.x >= position.x as f64
                && cursor.x < position.x as f64 + size.width as f64
                && cursor.y >= position.y as f64
                && cursor.y < position.y as f64 + size.height as f64
        })
        .or_else(|| monitors.first())
        .ok_or_else(|| "no monitors available".to_string())?;
    let scale = target.scale_factor();
    let monitor_position = target.position();
    let monitor_size = target.size();
    let monitor_bounds = MonitorBounds {
        x: monitor_position.x,
        y: monitor_position.y,
        width: i32::try_from(monitor_size.width).map_err(|_| "monitor width overflow")?,
        height: i32::try_from(monitor_size.height).map_err(|_| "monitor height overflow")?,
    };
    let window_size = WindowSize {
        width: (QUICK_CAPTURE_WIDTH_LOGICAL * scale).round() as i32,
        height: (QUICK_CAPTURE_HEIGHT_LOGICAL * scale).round() as i32,
    };
    Ok(quick_capture_position(
        cursor.x.round() as i32,
        cursor.y.round() as i32,
        monitor_bounds,
        window_size,
        QUICK_CAPTURE_CURSOR_OFFSET_PHYSICAL,
    ))
}

/// Window label for the Quick Capture floating window.
pub const QUICK_CAPTURE_LABEL: &str = "quick-capture";

/// Window label for the region-selection overlay (CAP-006).
pub const REGION_OVERLAY_LABEL: &str = "region-overlay";

/// Toggles always-on-top for the main window. Returns the new state.
pub fn toggle_always_on_top(app: &tauri::AppHandle) -> Result<bool, String> {
    let window = app
        .get_webview_window("main")
        .ok_or("main window not found")?;

    let current = window.is_always_on_top().map_err(|e| e.to_string())?;
    let new_state = !current;
    window
        .set_always_on_top(new_state)
        .map_err(|e| e.to_string())?;

    tracing::info!(event = "always_on_top_toggled", enabled = new_state);
    Ok(new_state)
}

/// Gets the current always-on-top state of the main window.
pub fn get_always_on_top(app: &tauri::AppHandle) -> Result<bool, String> {
    let window = app
        .get_webview_window("main")
        .ok_or("main window not found")?;
    window.is_always_on_top().map_err(|e| e.to_string())
}

/// Shows and focuses the main window.
pub fn show_main_window(app: &tauri::AppHandle) -> Result<(), String> {
    if let Some(window) = app.get_webview_window("main") {
        window.show().map_err(|e| e.to_string())?;
        window.set_focus().map_err(|e| e.to_string())?;
    }
    Ok(())
}

/// Hides the main window to tray.
pub fn hide_main_window(app: &tauri::AppHandle) -> Result<(), String> {
    if let Some(window) = app.get_webview_window("main") {
        window.hide().map_err(|e| e.to_string())?;
    }
    Ok(())
}

/// Toggles main window visibility.
pub fn toggle_main_window(app: &tauri::AppHandle) -> Result<(), String> {
    if let Some(window) = app.get_webview_window("main") {
        if window.is_visible().map_err(|e| e.to_string())? {
            window.hide().map_err(|e| e.to_string())?;
        } else {
            window.show().map_err(|e| e.to_string())?;
            window.set_focus().map_err(|e| e.to_string())?;
        }
    }
    Ok(())
}

/// Shows or creates the Quick Capture floating window.
/// Small, always-on-top; reused when already open.
/// CAP-002: records the user's current foreground app first so focus can be
/// returned when the window hides.
pub fn show_quick_capture(app: &tauri::AppHandle) -> Result<(), String> {
    let physical_position = quick_capture_physical_position(app)?;

    if let Some(window) = app.get_webview_window(QUICK_CAPTURE_LABEL) {
        crate::platform::focus::record_previous_focus();
        window.show().map_err(|e| e.to_string())?;
        window
            .set_position(tauri::Position::Physical(tauri::PhysicalPosition {
                x: physical_position.0,
                y: physical_position.1,
            }))
            .map_err(|e| e.to_string())?;
        window.set_focus().map_err(|e| e.to_string())?;
        return Ok(());
    }

    crate::platform::focus::record_previous_focus();

    let window = WebviewWindowBuilder::new(
        app,
        QUICK_CAPTURE_LABEL,
        WebviewUrl::App("index.html".into()),
    )
    .title("Quick Capture")
    .inner_size(QUICK_CAPTURE_WIDTH_LOGICAL, QUICK_CAPTURE_HEIGHT_LOGICAL)
    .resizable(false)
    .always_on_top(true)
    .decorations(false)
    .skip_taskbar(true)
    .visible(true)
    .focused(true)
    .build()
    .map_err(|e| e.to_string())?;

    window
        .set_position(tauri::Position::Physical(tauri::PhysicalPosition {
            x: physical_position.0,
            y: physical_position.1,
        }))
        .map_err(|e| e.to_string())?;

    tracing::info!(event = "quick_capture_window_opened");
    Ok(())
}

/// Hides the Quick Capture window. CAP-002: returns focus to the application
/// the user was in before Quick Capture opened.
pub fn hide_quick_capture(app: &tauri::AppHandle) -> Result<(), String> {
    if let Some(window) = app.get_webview_window(QUICK_CAPTURE_LABEL) {
        window.hide().map_err(|e| e.to_string())?;
        crate::platform::focus::restore_previous_focus();
    }
    Ok(())
}

/// Opens the region-selection overlay on the monitor containing the cursor
/// (CAP-006, multi-monitor aware). Idempotent: a second call is a no-op.
pub fn start_region_overlay(app: &tauri::AppHandle) -> Result<(), String> {
    if app.get_webview_window(REGION_OVERLAY_LABEL).is_some() {
        return Ok(()); // Already active
    }

    // Find the monitor containing the current cursor position (multi-monitor, CAP-006)
    let cursor = app.cursor_position().map_err(|e| e.to_string())?;
    let monitors = app.available_monitors().map_err(|e| e.to_string())?;
    let target = monitors
        .iter()
        .find(|m| {
            let pos = m.position();
            let size = m.size();
            cursor.x >= pos.x as f64
                && cursor.x < (pos.x + size.width as i32) as f64
                && cursor.y >= pos.y as f64
                && cursor.y < (pos.y + size.height as i32) as f64
        })
        .or_else(|| monitors.first())
        .ok_or_else(|| "no monitors available".to_string())?;

    let scale = target.scale_factor();
    let pos = target.position();
    let size = target.size();

    let window = WebviewWindowBuilder::new(
        app,
        REGION_OVERLAY_LABEL,
        WebviewUrl::App("index.html".into()),
    )
    .title("Region Capture")
    .position(pos.x as f64 / scale, pos.y as f64 / scale)
    .inner_size(size.width as f64 / scale, size.height as f64 / scale)
    .decorations(false)
    .transparent(true)
    .always_on_top(true)
    .skip_taskbar(true)
    .resizable(false)
    .visible(true)
    .focused(true)
    .build()
    .map_err(|e| e.to_string())?;

    let _ = window.set_cursor_grab(true);
    tracing::info!(
        event = "region_capture_started",
        monitor_x = pos.x,
        monitor_y = pos.y
    );
    Ok(())
}

#[cfg(test)]
mod tests {
    use super::{MonitorBounds, WindowSize, quick_capture_position};

    const WINDOW: WindowSize = WindowSize {
        width: 460,
        height: 280,
    };
    const OFFSET: i32 = 16;

    #[test]
    fn places_window_below_and_right_of_cursor() {
        assert_eq!(
            quick_capture_position(
                200,
                150,
                MonitorBounds {
                    x: 0,
                    y: 0,
                    width: 1920,
                    height: 1080,
                },
                WINDOW,
                OFFSET,
            ),
            (216, 166)
        );
    }

    #[test]
    fn clamps_to_right_and_bottom_edges() {
        assert_eq!(
            quick_capture_position(
                1900,
                1080,
                MonitorBounds {
                    x: 0,
                    y: 0,
                    width: 1920,
                    height: 1080,
                },
                WINDOW,
                OFFSET,
            ),
            (1424, 784)
        );
    }

    #[test]
    fn supports_negative_monitor_coordinates() {
        assert_eq!(
            quick_capture_position(
                -1200,
                200,
                MonitorBounds {
                    x: -1280,
                    y: -200,
                    width: 1280,
                    height: 1024,
                },
                WINDOW,
                OFFSET,
            ),
            (-1184, 216)
        );
    }

    #[test]
    fn anchors_oversized_window_at_monitor_origin() {
        assert_eq!(
            quick_capture_position(
                900,
                500,
                MonitorBounds {
                    x: 100,
                    y: 200,
                    width: 800,
                    height: 600,
                },
                WindowSize {
                    width: 900,
                    height: 700,
                },
                OFFSET,
            ),
            (100, 200)
        );
    }

    #[test]
    fn anchors_at_origin_for_degenerate_monitor_dimensions() {
        assert_eq!(
            quick_capture_position(
                50,
                60,
                MonitorBounds {
                    x: -100,
                    y: 200,
                    width: 0,
                    height: -1,
                },
                WindowSize {
                    width: 460,
                    height: 280,
                },
                OFFSET,
            ),
            (-100, 200)
        );
    }

    #[test]
    fn handles_negative_window_dimensions_without_panicking() {
        assert_eq!(
            quick_capture_position(
                200,
                150,
                MonitorBounds {
                    x: 0,
                    y: 0,
                    width: 1920,
                    height: 1080,
                },
                WindowSize {
                    width: -1,
                    height: 0,
                },
                OFFSET,
            ),
            (216, 166)
        );
    }
}
