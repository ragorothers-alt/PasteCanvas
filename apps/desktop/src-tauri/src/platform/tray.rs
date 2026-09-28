use tauri::{
    AppHandle, Emitter, Manager,
    menu::{Menu, MenuItem},
    tray::TrayIconBuilder,
};

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
                let _ = crate::platform::window_manager::show_main_window(app);
                let _ = app.emit(crate::events::SHOW_SETTINGS, ());
            }
            "pause" => {
                tracing::info!(event = "capture_pause_toggled", source = "tray");
                let _ = app.emit(crate::events::TOGGLE_CAPTURE_PAUSE, ());
            }
            "quit" => {
                tracing::info!(event = "app_quit_requested", source = "tray");
                let app = app.clone();
                tauri::async_runtime::spawn(async move {
                    finish_session_on_quit(&app).await;
                    app.exit(0);
                });
            }
            _ => {}
        })
        .on_tray_icon_event(|tray, event| {
            if let tauri::tray::TrayIconEvent::DoubleClick { .. } = event {
                let _ = crate::platform::window_manager::show_main_window(tray.app_handle());
            }
        })
        .build(app)?;

    Ok(())
}

/// Ends any unfinished capture session before process exit so a restart does
/// not show an "active" session whose watcher is not running.
async fn finish_session_on_quit(app: &AppHandle) {
    let state = app.state::<crate::bootstrap::app_state::AppState>();
    let svc = crate::application::session_service::SessionService::new(
        state.db.clone(),
        state.device_id.clone(),
    );
    match svc.get_current_session().await {
        Ok(Some(_)) => {
            if let Err(e) = svc.finish_session().await {
                tracing::warn!(event = "quit_finish_session_failed", error = %e);
            }
        }
        Ok(None) => {}
        Err(e) => {
            tracing::warn!(event = "quit_session_lookup_failed", error = %e);
        }
    }
    state.clipboard_listener.stop();
    crate::platform::clipboard_watch::stop_clipboard_watch();
}

/// Reads the current clipboard and captures whatever format is present.
async fn save_clipboard_from_tray(app: AppHandle) {
    let payload = crate::platform::clipboard::snapshot_clipboard_async(app.clone()).await;
    crate::platform::clipboard_persist::persist_and_emit(
        &app,
        payload,
        crate::domain::entities::item::CaptureMethod::ManualHotkey,
        None,
        None,
        true,
    )
    .await;
}
