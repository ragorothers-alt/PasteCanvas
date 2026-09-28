//! Browser extension integration commands (CAP-008).
//!
//! Registers the PasteCanvas Native Messaging Host so the Chrome/Edge
//! extension can capture content locally. Writes a host manifest and the
//! per-browser registry entries. Everything stays on the machine.

use serde::Deserialize;
use tauri::State;

use crate::bootstrap::app_state::AppState;

/// Native messaging host name — must match background.js NMH_NAME.
const NMH_NAME: &str = "com.pastecanvas.capture";

#[derive(Debug, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct RegisterBrowserHostRequest {
    /// chrome-extension://<id>/ origin allowed to talk to the host.
    pub extension_origin: String,
}

/// Resolves the path to the pastecanvas-nmh executable (same dir as the app exe).
fn nmh_exe_path() -> Result<std::path::PathBuf, String> {
    let exe = std::env::current_exe().map_err(|e| e.to_string())?;
    let dir = exe
        .parent()
        .ok_or_else(|| "no exe parent dir".to_string())?;
    Ok(dir.join("pastecanvas-nmh.exe"))
}

/// Registers the native messaging host for Chrome and Edge.
/// Writes the host manifest JSON into the app data dir, then creates registry
/// entries for both browsers pointing to it.
/// File write + reg.exe spawns run on the blocking pool.
#[tauri::command]
pub async fn register_browser_host(
    state: State<'_, AppState>,
    request: RegisterBrowserHostRequest,
) -> Result<(), String> {
    if !is_valid_extension_origin(&request.extension_origin) {
        return Err(
            "extension origin must be chrome-extension://<32-char-id>/ or moz-extension://<uuid>/"
                .to_string(),
        );
    }

    let data_dir = state.data_dir.clone();
    let origin = request.extension_origin.trim().to_string();
    tokio::task::spawn_blocking(move || -> Result<(), String> {
        let nmh_path = nmh_exe_path()?;
        if !nmh_path.exists() {
            return Err(format!(
                "native messaging host not found at {}",
                nmh_path.display()
            ));
        }

        // Write the NMH manifest JSON
        let manifest = serde_json::json!({
            "name": NMH_NAME,
            "description": "PasteCanvas local capture host",
            "path": nmh_path.to_string_lossy(),
            "type": "stdio",
            "allowed_origins": [origin]
        });

        let manifest_path = data_dir.join("nmh-manifest.json");
        std::fs::write(
            &manifest_path,
            serde_json::to_string_pretty(&manifest).map_err(|e| e.to_string())?,
        )
        .map_err(|e| format!("write manifest: {e}"))?;

        // Register for Chrome and Edge via the registry (Windows)
        #[cfg(target_os = "windows")]
        {
            use crate::platform::windows::reg;
            let manifest_str = manifest_path.display().to_string();
            let browsers = [
                r"HKCU\Software\Google\Chrome\NativeMessagingHosts",
                r"HKCU\Software\Microsoft\Edge\NativeMessagingHosts",
            ];
            for base in browsers {
                let key = format!("{base}\\{NMH_NAME}");
                reg::add_default(&key, &manifest_str)?;
            }
        }

        #[cfg(not(target_os = "windows"))]
        {
            return Err("browser host registration is only supported on Windows".to_string());
        }

        tracing::info!(event = "browser_host_registered", nmh = %nmh_path.display());
        Ok(())
    })
    .await
    .map_err(|e| format!("browser host registration task aborted: {e}"))?
}

/// Unregisters the native messaging host for Chrome and Edge.
#[tauri::command]
pub async fn unregister_browser_host() -> Result<(), String> {
    tokio::task::spawn_blocking(|| -> Result<(), String> {
        #[cfg(target_os = "windows")]
        {
            use crate::platform::windows::reg;
            let browsers = [
                r"HKCU\Software\Google\Chrome\NativeMessagingHosts",
                r"HKCU\Software\Microsoft\Edge\NativeMessagingHosts",
            ];
            for base in browsers {
                let key = format!("{base}\\{NMH_NAME}");
                reg::delete_key(&key)?;
            }
        }
        tracing::info!(event = "browser_host_unregistered");
        Ok(())
    })
    .await
    .map_err(|e| format!("browser host unregistration task aborted: {e}"))?
}

/// Chrome IDs are 32 chars in a–p. Firefox addon origins use a UUID.
pub fn is_valid_extension_origin(origin: &str) -> bool {
    let origin = origin.trim();
    if let Some(rest) = origin.strip_prefix("chrome-extension://") {
        let id = rest.trim_end_matches('/');
        return id.len() == 32 && id.bytes().all(|b| matches!(b, b'a'..=b'p'));
    }
    if let Some(rest) = origin.strip_prefix("moz-extension://") {
        let id = rest.trim_end_matches('/');
        return uuid::Uuid::parse_str(id).is_ok();
    }
    false
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn accepts_chrome_extension_id() {
        assert!(is_valid_extension_origin(
            "chrome-extension://abcdefghijklmnopabcdefghijklmnop/"
        ));
    }

    #[test]
    fn rejects_empty_and_http_origins() {
        assert!(!is_valid_extension_origin(""));
        assert!(!is_valid_extension_origin("https://evil.example/"));
        assert!(!is_valid_extension_origin("chrome-extension://"));
        assert!(!is_valid_extension_origin("chrome-extension://SHORT/"));
        assert!(!is_valid_extension_origin(
            "chrome-extension://zzzzzzzzzzzzzzzzzzzzzzzzzzzzzzzz/"
        ));
    }

    #[test]
    fn accepts_firefox_uuid_origin() {
        assert!(is_valid_extension_origin(
            "moz-extension://123e4567-e89b-12d3-a456-426614174000/"
        ));
    }
}
