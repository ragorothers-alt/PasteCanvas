use std::collections::HashMap;
use std::sync::{Mutex, OnceLock};

use tauri::{AppHandle, Emitter};
use tauri_plugin_global_shortcut::{Code, GlobalShortcutExt, Modifiers, Shortcut};

use crate::domain::entities::item::CaptureMethod;

/// Default capture shortcut: Ctrl+Shift+V
pub const DEFAULT_CAPTURE_ACCELERATOR: &str = "CommandOrControl+Shift+V";

/// Default region screenshot shortcut: Ctrl+Shift+S (CAP-006)
pub const DEFAULT_SCREENSHOT_ACCELERATOR: &str = "CommandOrControl+Shift+S";

/// WIN-005: tracks the accelerator currently bound for each action so
/// re-registration (settings change) can release the previous binding.
fn registry() -> &'static Mutex<HashMap<String, String>> {
    static REGISTRY: OnceLock<Mutex<HashMap<String, String>>> = OnceLock::new();
    REGISTRY.get_or_init(|| Mutex::new(HashMap::new()))
}

/// WIN-005: registers (or re-registers) the global hotkey for a named action.
/// Validates the accelerator, releases any previous binding for this action,
/// then binds the new one. Used at startup and on settings changes.
pub fn register_action_hotkey(
    app: &AppHandle,
    action: &str,
    accelerator: &str,
) -> Result<(), String> {
    let shortcut = parse_accelerator(accelerator)?;

    let mut reg = registry().lock().expect("hotkey registry");
    if reg.get(action).is_some_and(|prev| prev == accelerator) {
        return Ok(());
    }

    // Bind the new shortcut first so a registration failure cannot leave the
    // previous (working) binding unregistered.
    let action_owned = action.to_string();
    app.global_shortcut()
        .on_shortcut(shortcut, move |app, _shortcut, event| {
            if event.state != tauri_plugin_global_shortcut::ShortcutState::Pressed {
                return;
            }
            handle_action(app, &action_owned);
        })
        .map_err(|e| format!("hotkey registration failed for {action}: {e}"))?;

    if let Some(prev) = reg.insert(action.to_string(), accelerator.to_string()) {
        if let Ok(prev_shortcut) = parse_accelerator(&prev) {
            let _ = app.global_shortcut().unregister(prev_shortcut);
        }
    }
    drop(reg);

    tracing::info!(
        event = "hotkey_registered",
        action = action,
        accelerator = accelerator,
    );
    Ok(())
}

/// Dispatches a pressed global hotkey to its action handler.
fn handle_action(app: &AppHandle, action: &str) {
    match action {
        "capture_clipboard" => {
            tracing::info!(event = "capture_hotkey_pressed");
            let payload = crate::platform::clipboard::snapshot_clipboard(app);
            let app_handle = app.clone();
            tauri::async_runtime::spawn(async move {
                crate::platform::clipboard_persist::persist_and_emit(
                    &app_handle,
                    payload,
                    CaptureMethod::ManualHotkey,
                    None,
                    None,
                    true,
                )
                .await;
            });
        }
        "screenshot" => {
            tracing::info!(event = "screenshot_hotkey_pressed");
            // ARC-001: platform calls platform (window_manager), never commands.
            if let Err(e) = crate::platform::window_manager::start_region_overlay(app) {
                tracing::error!(event = "region_capture_start_failed", error = %e);
            }
        }
        "toggle_main_window" => {
            let _ = crate::platform::window_manager::toggle_main_window(app);
        }
        "quick_capture" => {
            let _ = crate::platform::window_manager::show_quick_capture(app);
        }
        "global_search" => {
            let _ = crate::platform::window_manager::show_main_window(app);
            let _ = app.emit(crate::events::FOCUS_SEARCH, ());
        }
        other => {
            tracing::warn!(event = "unknown_hotkey_action", action = other);
        }
    }
}

/// Registers the global region-screenshot hotkey with its default accelerator.
/// Startup path uses register_action_hotkey with the stored binding instead.
pub fn register_screenshot_hotkey(app: &AppHandle) -> Result<(), String> {
    register_action_hotkey(app, "screenshot", DEFAULT_SCREENSHOT_ACCELERATOR)
}

/// Registers the global capture hotkey with its default accelerator.
/// Startup path uses register_action_hotkey with the stored binding instead.
pub fn register_capture_hotkey(app: &AppHandle) -> Result<(), String> {
    register_action_hotkey(app, "capture_clipboard", DEFAULT_CAPTURE_ACCELERATOR)
}

/// Unregisters the hotkey currently bound to an action (WIN-005).
pub fn unregister_action_hotkey(app: &AppHandle, action: &str) {
    let mut reg = registry().lock().expect("hotkey registry");
    if let Some(prev) = reg.remove(action) {
        if let Ok(shortcut) = parse_accelerator(&prev) {
            let _ = app.global_shortcut().unregister(shortcut);
            tracing::info!(event = "hotkey_unregistered", action = action);
        }
    }
}

/// Unregisters the capture hotkey safely.
pub fn unregister_capture_hotkey(app: &AppHandle) {
    unregister_action_hotkey(app, "capture_clipboard");
}

/// WIN-005: validates an accelerator string without registering it.
pub fn validate_accelerator(accelerator: &str) -> Result<(), String> {
    parse_accelerator(accelerator).map(|_| ())
}

/// Parses an accelerator string like "CommandOrControl+Shift+V" into a Shortcut.
fn parse_accelerator(accelerator: &str) -> Result<Shortcut, String> {
    let parts: Vec<&str> = accelerator.split('+').collect();
    if parts.is_empty() {
        return Err("empty accelerator".to_string());
    }

    let mut modifiers = Modifiers::empty();
    let mut key_code: Option<Code> = None;

    for part in &parts {
        match part.to_lowercase().as_str() {
            "commandorcontrol" | "ctrl" | "control" => modifiers |= Modifiers::CONTROL,
            "shift" => modifiers |= Modifiers::SHIFT,
            "alt" | "option" => modifiers |= Modifiers::ALT,
            "super" | "meta" | "command" => modifiers |= Modifiers::SUPER,
            _ => {
                key_code = Some(parse_key_code(part)?);
            }
        }
    }

    let code = key_code.ok_or_else(|| "no key code in accelerator".to_string())?;
    Ok(Shortcut::new(Some(modifiers), code))
}

fn parse_key_code(key: &str) -> Result<Code, String> {
    match key.to_uppercase().as_str() {
        "A" => Ok(Code::KeyA),
        "B" => Ok(Code::KeyB),
        "C" => Ok(Code::KeyC),
        "D" => Ok(Code::KeyD),
        "E" => Ok(Code::KeyE),
        "F" => Ok(Code::KeyF),
        "G" => Ok(Code::KeyG),
        "H" => Ok(Code::KeyH),
        "I" => Ok(Code::KeyI),
        "J" => Ok(Code::KeyJ),
        "K" => Ok(Code::KeyK),
        "L" => Ok(Code::KeyL),
        "M" => Ok(Code::KeyM),
        "N" => Ok(Code::KeyN),
        "O" => Ok(Code::KeyO),
        "P" => Ok(Code::KeyP),
        "Q" => Ok(Code::KeyQ),
        "R" => Ok(Code::KeyR),
        "S" => Ok(Code::KeyS),
        "T" => Ok(Code::KeyT),
        "U" => Ok(Code::KeyU),
        "V" => Ok(Code::KeyV),
        "W" => Ok(Code::KeyW),
        "X" => Ok(Code::KeyX),
        "Y" => Ok(Code::KeyY),
        "Z" => Ok(Code::KeyZ),
        "0" => Ok(Code::Digit0),
        "1" => Ok(Code::Digit1),
        "2" => Ok(Code::Digit2),
        "3" => Ok(Code::Digit3),
        "4" => Ok(Code::Digit4),
        "5" => Ok(Code::Digit5),
        "6" => Ok(Code::Digit6),
        "7" => Ok(Code::Digit7),
        "8" => Ok(Code::Digit8),
        "9" => Ok(Code::Digit9),
        "F1" => Ok(Code::F1),
        "F2" => Ok(Code::F2),
        "F3" => Ok(Code::F3),
        "F4" => Ok(Code::F4),
        "F5" => Ok(Code::F5),
        "F6" => Ok(Code::F6),
        "F7" => Ok(Code::F7),
        "F8" => Ok(Code::F8),
        "F9" => Ok(Code::F9),
        "F10" => Ok(Code::F10),
        "F11" => Ok(Code::F11),
        "F12" => Ok(Code::F12),
        "SPACE" => Ok(Code::Space),
        "ENTER" => Ok(Code::Enter),
        "ESCAPE" | "ESC" => Ok(Code::Escape),
        _ => Err(format!("unsupported key: {key}")),
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn parse_default_accelerator() {
        let result = parse_accelerator(DEFAULT_CAPTURE_ACCELERATOR);
        assert!(result.is_ok());
    }

    #[test]
    fn parse_ctrl_shift_c() {
        let result = parse_accelerator("Ctrl+Shift+C");
        assert!(result.is_ok());
    }

    #[test]
    fn parse_empty_fails() {
        let result = parse_accelerator("");
        assert!(result.is_err());
    }

    #[test]
    fn parse_modifiers_only_fails() {
        let result = parse_accelerator("Ctrl+Shift");
        assert!(result.is_err());
    }

    #[test]
    fn parse_unknown_key_fails() {
        let result = parse_accelerator("Ctrl+Unknown");
        assert!(result.is_err());
    }

    #[test]
    fn validate_accelerator_matches_parse() {
        // WIN-005: pre-persist validation used by update_hotkey
        assert!(validate_accelerator("Ctrl+Shift+C").is_ok());
        assert!(validate_accelerator("Ctrl+Shift").is_err());
        assert!(validate_accelerator("").is_err());
    }

    #[test]
    fn registry_roundtrip_is_empty_initially() {
        // No action is bound before any registration happens.
        let reg = registry().lock().expect("registry");
        assert!(reg.get("definitely-not-registered").is_none());
    }
}
