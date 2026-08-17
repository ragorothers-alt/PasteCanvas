use serde::Deserialize;
use tauri::State;

use crate::application::settings_service::{ExclusionDto, HotkeyBindingDto, SettingsService};
use crate::bootstrap::app_state::AppState;

#[tauri::command]
pub async fn get_setting(
    state: State<'_, AppState>,
    key: String,
) -> Result<Option<String>, String> {
    let svc = SettingsService::new(state.db.clone());
    svc.get_setting(&key).await.map_err(|e| e.to_string())
}

#[tauri::command]
pub async fn set_setting(
    state: State<'_, AppState>,
    key: String,
    value_json: String,
) -> Result<(), String> {
    let svc = SettingsService::new(state.db.clone());
    svc.set_setting(&key, &value_json)
        .await
        .map_err(|e| e.to_string())
}

#[tauri::command]
pub async fn list_exclusions(state: State<'_, AppState>) -> Result<Vec<ExclusionDto>, String> {
    let svc = SettingsService::new(state.db.clone());
    svc.list_exclusions().await.map_err(|e| e.to_string())
}

#[derive(Debug, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct AddExclusionRequest {
    pub match_type: String,
    pub match_value: String,
}

#[tauri::command]
pub async fn add_exclusion(
    state: State<'_, AppState>,
    request: AddExclusionRequest,
) -> Result<ExclusionDto, String> {
    let svc = SettingsService::new(state.db.clone());
    svc.add_exclusion(&request.match_type, &request.match_value)
        .await
        .map_err(|e| e.to_string())
}

#[tauri::command]
pub async fn toggle_exclusion(state: State<'_, AppState>, id: String) -> Result<bool, String> {
    let svc = SettingsService::new(state.db.clone());
    svc.toggle_exclusion(&id).await.map_err(|e| e.to_string())
}

#[tauri::command]
pub async fn remove_exclusion(state: State<'_, AppState>, id: String) -> Result<(), String> {
    let svc = SettingsService::new(state.db.clone());
    svc.remove_exclusion(&id).await.map_err(|e| e.to_string())
}

#[tauri::command]
pub async fn list_hotkeys(state: State<'_, AppState>) -> Result<Vec<HotkeyBindingDto>, String> {
    let svc = SettingsService::new(state.db.clone());
    svc.list_hotkeys().await.map_err(|e| e.to_string())
}

/// FND-006: hotkey actions that failed to register at startup (drained on
/// read). The UI surfaces these once so failures are visible and recoverable
/// — the user can rebind the conflicting shortcut in Settings.
#[tauri::command]
pub async fn get_startup_hotkey_failures(
    state: State<'_, AppState>,
) -> Result<Vec<String>, String> {
    Ok(match state.hotkey_failures.lock() {
        Ok(mut guard) => std::mem::take(&mut *guard),
        Err(_) => Vec::new(),
    })
}

#[derive(Debug, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct UpdateHotkeyRequest {
    pub action: String,
    pub accelerator: String,
}

#[tauri::command]
pub async fn update_hotkey(
    app: tauri::AppHandle,
    state: State<'_, AppState>,
    request: UpdateHotkeyRequest,
) -> Result<(), String> {
    // WIN-005: validate before persisting so an invalid accelerator cannot
    // leave the stored binding out of sync with the live registration.
    if !request.accelerator.trim().is_empty() {
        crate::platform::hotkey::validate_accelerator(&request.accelerator)?;
    }

    // Register first so a failed OS bind cannot persist a dead accelerator.
    if request.accelerator.trim().is_empty() {
        crate::platform::hotkey::unregister_action_hotkey(&app, &request.action);
    } else {
        crate::platform::hotkey::register_action_hotkey(
            &app,
            &request.action,
            &request.accelerator,
        )?;
    }

    let svc = SettingsService::new(state.db.clone());
    svc.update_hotkey(&request.action, &request.accelerator)
        .await
        .map_err(|e| e.to_string())?;
    Ok(())
}
