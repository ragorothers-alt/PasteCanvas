use tauri::State;

use crate::bootstrap::app_state::AppState;
use crate::platform::window_manager as wm;

/// Toggles always-on-top for the main window.
#[tauri::command]
pub async fn toggle_always_on_top(app: tauri::AppHandle) -> Result<bool, String> {
    wm::toggle_always_on_top(&app)
}

/// Gets the current always-on-top state.
#[tauri::command]
pub async fn get_always_on_top(app: tauri::AppHandle) -> Result<bool, String> {
    wm::get_always_on_top(&app)
}

/// Shows and focuses the main window.
#[tauri::command]
pub async fn show_main_window(app: tauri::AppHandle) -> Result<(), String> {
    wm::show_main_window(&app)
}

/// Hides the main window to tray.
#[tauri::command]
pub async fn hide_main_window(app: tauri::AppHandle) -> Result<(), String> {
    wm::hide_main_window(&app)
}

/// Toggles main window visibility.
#[tauri::command]
pub async fn toggle_main_window(app: tauri::AppHandle) -> Result<(), String> {
    wm::toggle_main_window(&app)
}

/// Sets autostart enabled/disabled via Windows registry.
#[tauri::command]
pub async fn set_autostart(state: State<'_, AppState>, enabled: bool) -> Result<(), String> {
    // Apply the OS hook first so a registry failure cannot leave the setting
    // showing "enabled" while the machine will not actually auto-start.
    #[cfg(target_os = "windows")]
    {
        use crate::platform::windows::reg;
        let exe_path = std::env::current_exe().map_err(|e| e.to_string())?;
        let reg_path = r"HKCU\Software\Microsoft\Windows\CurrentVersion\Run";
        let app_name = "PasteCanvas";

        if enabled {
            reg::add_value(
                reg_path,
                app_name,
                &format!("\"{}\" --minimized", exe_path.display()),
            )?;
        } else {
            reg::delete_value(reg_path, app_name)?;
        }
    }

    #[cfg(not(target_os = "windows"))]
    {
        if enabled {
            return Err("autostart is only supported on Windows".to_string());
        }
    }

    let svc = crate::application::settings_service::SettingsService::new(state.db.clone());
    let value = if enabled { "true" } else { "false" };
    svc.set_setting("app.autostart", value)
        .await
        .map_err(|e| e.to_string())?;

    tracing::info!(event = "autostart_changed", enabled = enabled);
    Ok(())
}

/// Gets autostart state.
#[tauri::command]
pub async fn get_autostart(state: State<'_, AppState>) -> Result<bool, String> {
    let svc = crate::application::settings_service::SettingsService::new(state.db.clone());
    let value = svc
        .get_setting("app.autostart")
        .await
        .map_err(|e| e.to_string())?;
    Ok(value.as_deref() == Some("true"))
}

/// Shows or creates the Quick Capture floating window.
/// Small, always-on-top, appears near cursor position.
#[tauri::command]
pub async fn show_quick_capture(app: tauri::AppHandle) -> Result<(), String> {
    wm::show_quick_capture(&app)
}

/// Hides the Quick Capture window.
#[tauri::command]
pub async fn hide_quick_capture(app: tauri::AppHandle) -> Result<(), String> {
    wm::hide_quick_capture(&app)
}

/// CAP-002: closes Quick Capture from inside the floating window itself
/// (after save or Esc) and returns focus to the previous application.
#[tauri::command]
pub async fn close_quick_capture(app: tauri::AppHandle) -> Result<(), String> {
    wm::hide_quick_capture(&app)
}
