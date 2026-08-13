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
#[tauri::command]
pub async fn register_browser_host(
    state: State<'_, AppState>,
    request: RegisterBrowserHostRequest,
) -> Result<(), String> {
    if request.extension_origin.trim().is_empty() {
        return Err("extension origin cannot be empty".to_string());
    }

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
        "allowed_origins": [request.extension_origin.trim()]
    });

    let manifest_path = state.data_dir.join("nmh-manifest.json");
    std::fs::write(
        &manifest_path,
        serde_json::to_string_pretty(&manifest).map_err(|e| e.to_string())?,
    )
    .map_err(|e| format!("write manifest: {e}"))?;

    // Register for Chrome and Edge via the registry (Windows)
    #[cfg(target_os = "windows")]
    {
        use std::process::Command;
        let manifest_str = manifest_path.display().to_string();
        let browsers = [
            r"HKCU\Software\Google\Chrome\NativeMessagingHosts",
            r"HKCU\Software\Microsoft\Edge\NativeMessagingHosts",
        ];
        for base in browsers {
            let key = format!("{base}\\{NMH_NAME}");
            let _ = Command::new("reg")
                .args([
                    "add",
                    &key,
                    "/ve",
                    "/t",
                    "REG_SZ",
                    "/d",
                    &manifest_str,
                    "/f",
                ])
                .output();
        }
    }

    tracing::info!(event = "browser_host_registered", nmh = %nmh_path.display());
    Ok(())
}

/// Unregisters the native messaging host for Chrome and Edge.
#[tauri::command]
pub async fn unregister_browser_host() -> Result<(), String> {
    #[cfg(target_os = "windows")]
    {
        use std::process::Command;
        let browsers = [
            r"HKCU\Software\Google\Chrome\NativeMessagingHosts",
            r"HKCU\Software\Microsoft\Edge\NativeMessagingHosts",
        ];
        for base in browsers {
            let key = format!("{base}\\{NMH_NAME}");
            let _ = Command::new("reg").args(["delete", &key, "/f"]).output();
        }
    }
    tracing::info!(event = "browser_host_unregistered");
    Ok(())
}
