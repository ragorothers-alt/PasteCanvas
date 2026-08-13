use tauri::{
    AppHandle, Emitter, Manager,
    menu::{Menu, MenuItem},
    tray::TrayIconBuilder,
};
use tauri_plugin_clipboard_manager::ClipboardExt;

/// Sets up the system tray with context menu (WIN-001).
/// Menu: Open, Quick Capture, Save Current Clipboard, Pause/Resume Session,
/// Always on Top, Settings, Quit.
pub fn setup_tray(app: &AppHandle) -> Result<(), Box<dyn std::error::Error>> {
    let open_item = MenuItem::with_id(app, "open", "Open PasteCanvas", true, None::<&str>)?;
    let quick_capture_item =
        MenuItem::with_id(app, "quick_capture", "Quick Capture", true, None::<&str>)?;
    let save_clipboard_item = MenuItem::with_id(
        app,
        "save_clipboard",
        "Save Current Clipboard",
        true,
        None::<&str>,
    )?;
    let pause_item = MenuItem::with_id(app, "pause", "Pause / Resume Session", true, None::<&str>)?;
    let top_item = MenuItem::with_id(app, "always_on_top", "Always on Top", true, None::<&str>)?;
    let settings_item = MenuItem::with_id(app, "settings", "Settings", true, None::<&str>)?;
    let quit_item = MenuItem::with_id(app, "quit", "Quit", true, None::<&str>)?;

    let menu = Menu::with_items(
        app,
        &[
            &open_item,
            &quick_capture_item,
            &save_clipboard_item,
            &pause_item,
            &top_item,
            &settings_item,
            &quit_item,
        ],
    )?;

    TrayIconBuilder::with_id("main-tray")
        .icon(app.default_window_icon().cloned().expect("default icon"))
        .tooltip("PasteCanvas")
        .menu(&menu)
        .show_menu_on_left_click(false)
        .on_menu_event(|app, event| match event.id.as_ref() {
            "open" => {
                // ARC-001: window ops live in platform::window_manager
                let _ = crate::platform::window_manager::show_main_window(app);
            }
            "quick_capture" => {
                tracing::info!(event = "quick_capture_requested", source = "tray");
                let _ = crate::platform::window_manager::show_quick_capture(app);
            }
            "save_clipboard" => {
                tracing::info!(event = "save_clipboard_requested", source = "tray");
                let app = app.clone();
                tauri::async_runtime::spawn(async move {
                    save_clipboard_from_tray(app).await;
                });
            }
            "always_on_top" => {
                let _ = crate::platform::window_manager::toggle_always_on_top(app);
            }
            "settings" => {
                // Show main window and ask frontend to switch to Settings view
                if let Some(window) = app.get_webview_window("main") {
                    let _ = window.show();
                    let _ = window.set_focus();
                }
                let _ = app.emit("paste-canvas://show-settings", ());
            }
            "pause" => {
                tracing::info!(event = "capture_pause_toggled", source = "tray");
                // Emit event to frontend to toggle capture pause state
                let _ = app.emit("paste-canvas://toggle-capture-pause", ());
            }
            "quit" => {
                tracing::info!(event = "app_quit_requested", source = "tray");
                app.exit(0);
            }
            _ => {}
        })
        .on_tray_icon_event(|tray, event| {
            if let tauri::tray::TrayIconEvent::DoubleClick { .. } = event {
                let app = tray.app_handle();
                if let Some(window) = app.get_webview_window("main") {
                    let _ = window.show();
                    let _ = window.set_focus();
                }
            }
        })
        .build(app)?;

    Ok(())
}

/// Reads the current clipboard text and captures it into the Inbox (WIN-001).
async fn save_clipboard_from_tray(app: AppHandle) {
    let text = match app.clipboard().read_text() {
        Ok(t) => t,
        Err(e) => {
            tracing::warn!(event = "tray_clipboard_read_failed", error = %e);
            let _ = app.emit("paste-canvas://capture-failed", "Clipboard read failed");
            return;
        }
    };

    if text.trim().is_empty() {
        let _ = app.emit("paste-canvas://capture-failed", "Clipboard is empty");
        return;
    }

    let state = app.state::<crate::bootstrap::app_state::AppState>();
    let svc = crate::application::capture_service::CaptureService::new(
        state.db.clone(),
        state.device_id.clone(),
    );

    match svc
        .capture_text(crate::application::capture_service::CaptureTextRequest {
            text,
            capture_method: crate::domain::entities::item::CaptureMethod::ManualHotkey,
            source_app: None,
            window_title: None,
        })
        .await
    {
        Ok(result) => {
            let _ = app.emit("paste-canvas://capture-success", result.item_id.to_string());
            let _ = app.emit("pastecanvas:item-captured", ());
        }
        Err(e) => {
            tracing::error!(event = "tray_capture_failed", error = %e);
            let _ = app.emit("paste-canvas://capture-failed", e.to_string());
        }
    }
}
