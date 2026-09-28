//! Windows Explorer context-menu registration (CAP-007).
//!
//! Registers the `pastecanvas_explorer_dll` COM server (IExplorerCommand) so the
//! "Send to PasteCanvas" entries appear in Explorer. Registration is written under
//! HKCU\Software\Classes (per-user, no admin required — local-first friendly).
//!
//! For the Windows 11 *modern* context menu, a sparse package must actually be
//! registered with Add-AppxPackage. Writing a script alone is not success.

use serde::Serialize;
use tauri::State;

use crate::bootstrap::app_state::AppState;

const CLSID_INBOX: &str = "{7e4a0c1a-6f2c-4a31-9c7a-1a2b3c4d5e01}";
const CLSID_BOARD: &str = "{7e4a0c1a-6f2c-4a31-9c7a-1a2b3c4d5e02}";
const DLL_NAME: &str = "pastecanvas_explorer_dll.dll";
const VERB_INBOX: &str = "PasteCanvasInbox";
const VERB_BOARD: &str = "PasteCanvasBoard";

/// Classic / "Show more options" verbs that bind an IExplorerCommand CLSID.
fn explorer_command_verbs() -> [(&'static str, &'static str, &'static str); 2] {
    [
        (CLSID_INBOX, VERB_INBOX, "Send to PasteCanvas Inbox"),
        (CLSID_BOARD, VERB_BOARD, "Send to PasteCanvas Board"),
    ]
}

#[derive(Debug, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct ExplorerMenuRegistrationDto {
    pub dll_path: String,
    pub sparse_manifest_path: Option<String>,
    /// Classic / "Show more options" IExplorerCommand verbs were written.
    pub registered: bool,
    /// Win11 compact menu sparse package was actually registered with the OS.
    pub modern_registered: bool,
    pub modern_error: Option<String>,
}

fn dll_path() -> Result<std::path::PathBuf, String> {
    let exe = std::env::current_exe().map_err(|e| e.to_string())?;
    let dir = exe
        .parent()
        .ok_or_else(|| "no exe parent dir".to_string())?;
    Ok(dir.join(DLL_NAME))
}

/// Writes a registry string value via the `reg` CLI (per-user HKCU).
#[cfg(target_os = "windows")]
fn reg_add(key: &str, value_name: Option<&str>, data: &str) -> Result<(), String> {
    match value_name {
        Some(name) => crate::platform::windows::reg::add_value(key, name, data),
        None => crate::platform::windows::reg::add_default(key, data),
    }
}

/// Registers the IExplorerCommand COM server + shell entries for files & folders.
/// All OS work (reg.exe spawns, file copies, PowerShell) runs on the blocking
/// pool — a dozen subprocess launches must not stall the async runtime.
#[tauri::command]
pub async fn register_modern_explorer_menu(
    state: State<'_, AppState>,
) -> Result<ExplorerMenuRegistrationDto, String> {
    let data_dir = state.data_dir.clone();
    tokio::task::spawn_blocking(move || -> Result<ExplorerMenuRegistrationDto, String> {
        let dll = dll_path()?;
        if !dll.exists() {
            return Err(format!("COM DLL not found at {}", dll.display()));
        }
        let dll_str = dll.display().to_string();

        #[cfg(target_os = "windows")]
        {
            for clsid in [CLSID_INBOX, CLSID_BOARD] {
                let clsid_key = format!(r"HKCU\Software\Classes\CLSID\{clsid}");
                let inproc = format!(r"{clsid_key}\InprocServer32");
                reg_add(&clsid_key, None, "PasteCanvas Explorer Command")?;
                reg_add(&inproc, None, &dll_str)?;
                reg_add(&inproc, Some("ThreadingModel"), "Apartment")?;
            }
            // IExplorerCommand is invoked only when ExplorerCommandHandler is set.
            // A bare shell key with just a display name is a dead menu item.
            for root in [
                r"HKCU\Software\Classes\*",
                r"HKCU\Software\Classes\Directory",
            ] {
                for (clsid, verb, title) in explorer_command_verbs() {
                    let shell_key = format!(r"{root}\shell\{verb}");
                    reg_add(&shell_key, None, title)?;
                    reg_add(&shell_key, Some("ExplorerCommandHandler"), clsid)?;
                }
            }
        }

        #[cfg(not(target_os = "windows"))]
        {
            return Err("modern explorer menu is only supported on Windows".to_string());
        }

        // Stage + try to register the Win11 sparse package. Failure must not look
        // like success: classic verbs still work under "Show more options".
        let (sparse_manifest, modern_registered, modern_error) =
            apply_sparse_package(&data_dir, &dll)?;

        tracing::info!(
            event = "explorer_menu_registered",
            dll = %dll_str,
            modern = modern_registered
        );
        Ok(ExplorerMenuRegistrationDto {
            dll_path: dll_str,
            sparse_manifest_path: sparse_manifest,
            registered: true,
            modern_registered,
            modern_error,
        })
    })
    .await
    .map_err(|e| format!("explorer menu registration task aborted: {e}"))?
}

/// Removes the COM server + shell entries.
#[tauri::command]
pub async fn unregister_modern_explorer_menu() -> Result<(), String> {
    tokio::task::spawn_blocking(|| -> Result<(), String> {
        #[cfg(target_os = "windows")]
        {
            use crate::platform::windows::reg;
            let mut keys = Vec::new();
            for clsid in [CLSID_INBOX, CLSID_BOARD] {
                keys.push(format!(r"HKCU\Software\Classes\CLSID\{clsid}"));
            }
            for root in [
                r"HKCU\Software\Classes\*",
                r"HKCU\Software\Classes\Directory",
            ] {
                for (_, verb, _) in explorer_command_verbs() {
                    keys.push(format!(r"{root}\shell\{verb}"));
                }
            }
            for key in keys {
                reg::delete_key(&key)?;
            }
            let _ = remove_sparse_package();
        }
        tracing::info!(event = "explorer_menu_unregistered");
        Ok(())
    })
    .await
    .map_err(|e| format!("explorer menu unregistration task aborted: {e}"))?
}

const BRIDGE_EXE: &str = "pastecanvas-explorer.exe";
const PACKAGE_FAMILY: &str = "PasteCanvas.ExplorerMenu";

fn explorer_bridge_path() -> Result<std::path::PathBuf, String> {
    let exe = std::env::current_exe().map_err(|e| e.to_string())?;
    let dir = exe
        .parent()
        .ok_or_else(|| "no exe parent dir".to_string())?;
    Ok(dir.join(BRIDGE_EXE))
}

/// Stages the sparse package and tries to register it with the OS.
/// Never reports modern success unless `Add-AppxPackage` actually succeeded.
fn apply_sparse_package(
    data_dir: &std::path::Path,
    dll: &std::path::Path,
) -> Result<(Option<String>, bool, Option<String>), String> {
    let script = write_sparse_package_assets(data_dir, &dll.display().to_string())?;
    let package_dir = data_dir.join("explorer-sparse");
    if let Err(e) = stage_sparse_package_binaries(&package_dir, dll) {
        return Ok((script, false, Some(e)));
    }
    let manifest = package_dir.join("AppxManifest.xml");
    match register_sparse_package(&manifest) {
        Ok(()) => Ok((script, true, None)),
        Err(e) => Ok((script, false, Some(e))),
    }
}

fn stage_sparse_package_binaries(
    package_dir: &std::path::Path,
    dll: &std::path::Path,
) -> Result<(), String> {
    std::fs::create_dir_all(package_dir.join("Assets"))
        .map_err(|e| format!("create sparse package dir: {e}"))?;
    let staged_manifest = package_dir.join("AppxManifest.xml");
    let source_manifest = package_dir
        .parent()
        .unwrap_or(package_dir)
        .join("explorer-menu-appxmanifest.xml");
    if source_manifest.exists() {
        std::fs::copy(&source_manifest, &staged_manifest)
            .map_err(|e| format!("copy manifest: {e}"))?;
    }
    let logo_src = package_dir
        .parent()
        .unwrap_or(package_dir)
        .join("Assets")
        .join("logo.png");
    if logo_src.exists() {
        std::fs::copy(&logo_src, package_dir.join("Assets").join("logo.png"))
            .map_err(|e| format!("copy logo: {e}"))?;
    }
    std::fs::copy(dll, package_dir.join(DLL_NAME)).map_err(|e| format!("copy COM DLL: {e}"))?;
    let bridge = explorer_bridge_path()?;
    if !bridge.exists() {
        return Err(format!("explorer bridge not found at {}", bridge.display()));
    }
    std::fs::copy(&bridge, package_dir.join(BRIDGE_EXE))
        .map_err(|e| format!("copy explorer bridge: {e}"))?;
    Ok(())
}

#[cfg(target_os = "windows")]
fn register_sparse_package(manifest: &std::path::Path) -> Result<(), String> {
    let output = std::process::Command::new("powershell")
        .args([
            "-NoProfile",
            "-ExecutionPolicy",
            "Bypass",
            "-Command",
            &format!(
                "Add-AppxPackage -Register -Path '{}' -ErrorAction Stop",
                manifest.display()
            ),
        ])
        .output()
        .map_err(|e| format!("failed to start PowerShell: {e}"))?;
    if output.status.success() {
        return Ok(());
    }
    let stderr = String::from_utf8_lossy(&output.stderr);
    let stdout = String::from_utf8_lossy(&output.stdout);
    let detail = [stderr.trim(), stdout.trim()]
        .into_iter()
        .find(|s| !s.is_empty())
        .unwrap_or("Add-AppxPackage failed");
    Err(detail.to_string())
}

#[cfg(not(target_os = "windows"))]
fn register_sparse_package(_manifest: &std::path::Path) -> Result<(), String> {
    Err("sparse package registration is only supported on Windows".to_string())
}

#[cfg(target_os = "windows")]
fn remove_sparse_package() -> Result<(), String> {
    let output = std::process::Command::new("powershell")
        .args([
            "-NoProfile",
            "-ExecutionPolicy",
            "Bypass",
            "-Command",
            &format!(
                "Get-AppxPackage -Name '{PACKAGE_FAMILY}' | Remove-AppxPackage -ErrorAction SilentlyContinue"
            ),
        ])
        .output()
        .map_err(|e| format!("failed to start PowerShell: {e}"))?;
    if output.status.success() {
        Ok(())
    } else {
        Err(String::from_utf8_lossy(&output.stderr).trim().to_string())
    }
}

#[cfg(not(target_os = "windows"))]
fn remove_sparse_package() -> Result<(), String> {
    Ok(())
}

/// Writes an MSIX sparse-package manifest + PowerShell registration script so the
/// entries can surface in the Windows 11 modern context menu. Returns the script path.
fn write_sparse_package_assets(
    data_dir: &std::path::Path,
    _dll_path: &str,
) -> Result<Option<String>, String> {
    let manifest = format!(
        r#"<?xml version="1.0" encoding="utf-8"?>
<Package xmlns="http://schemas.microsoft.com/appx/manifest/foundation/windows10"
         xmlns:com="http://schemas.microsoft.com/appx/manifest/com/windows10"
         xmlns:desktop="http://schemas.microsoft.com/appx/manifest/desktop/windows10"
         xmlns:uap="http://schemas.microsoft.com/appx/manifest/uap/windows10"
         IgnorableNamespaces="uap">
  <Identity Name="PasteCanvas.ExplorerMenu" ProcessorArchitecture="x64"
            Publisher="CN=PasteCanvas" Version="1.0.0.0" />
  <Properties>
    <DisplayName>PasteCanvas Explorer Menu</DisplayName>
    <PublisherDisplayName>PasteCanvas</PublisherDisplayName>
    <Logo>Assets\logo.png</Logo>
  </Properties>
  <Resources>
    <Resource Language="en-us" />
  </Resources>
  <Dependencies>
    <TargetDeviceFamily Name="Windows.Desktop" MinVersion="10.0.17763.0" MaxVersionTested="10.0.22000.0" />
  </Dependencies>
  <Capabilities>
    <Capability Name="runFullTrust" />
  </Capabilities>
  <Applications>
    <Application Id="PasteCanvasExplorerMenu" Executable="pastecanvas-explorer.exe"
                 desktop:trustLevel="mediumIL" desktop:runtimeType="standardWin32">
      <Extensions>
        <desktop:Extension Category="windows.fileExplorerContextMenus">
          <desktop:FileExplorerContextMenus>
            <desktop:ItemType Type="*">
              <desktop:Menu Id="PasteCanvasInbox">
                <desktop:Item Id="SendInbox" Clsid="{inbox}" />
              </desktop:Menu>
            </desktop:ItemType>
            <desktop:ItemType Type="Directory">
              <desktop:Menu Id="PasteCanvasBoard">
                <desktop:Item Id="SendBoard" Clsid="{board}" />
              </desktop:Menu>
            </desktop:ItemType>
          </desktop:FileExplorerContextMenus>
        </desktop:Extension>
        <com:Extension Category="windows.comServer">
          <com:ComServer>
            <com:SurrogateServer DisplayName="PasteCanvas Explorer Command">
              <com:Class Id="{inbox}" Path="{dll}" ThreadingModel="STA" />
              <com:Class Id="{board}" Path="{dll}" ThreadingModel="STA" />
            </com:SurrogateServer>
          </com:ComServer>
        </com:Extension>
      </Extensions>
    </Application>
  </Applications>
</Package>
"#,
        inbox = CLSID_INBOX,
        board = CLSID_BOARD,
        dll = DLL_NAME,
    );

    let manifest_path = data_dir.join("explorer-menu-appxmanifest.xml");
    std::fs::write(&manifest_path, manifest).map_err(|e| format!("write manifest: {e}"))?;

    // Appx identity requires Properties/Logo; write a 1x1 PNG so the script
    // is not dead on arrival.
    let assets = data_dir.join("Assets");
    std::fs::create_dir_all(&assets).map_err(|e| format!("create Assets: {e}"))?;
    const PNG_1X1: &[u8] = &[
        0x89, 0x50, 0x4E, 0x47, 0x0D, 0x0A, 0x1A, 0x0A, 0x00, 0x00, 0x00, 0x0D, 0x49, 0x48, 0x44,
        0x52, 0x00, 0x00, 0x00, 0x01, 0x00, 0x00, 0x00, 0x01, 0x08, 0x06, 0x00, 0x00, 0x00, 0x1F,
        0x15, 0xC4, 0x89, 0x00, 0x00, 0x00, 0x0A, 0x49, 0x44, 0x41, 0x54, 0x78, 0x9C, 0x63, 0x00,
        0x01, 0x00, 0x00, 0x05, 0x00, 0x01, 0x0D, 0x0A, 0x2D, 0xB4, 0x00, 0x00, 0x00, 0x00, 0x49,
        0x45, 0x4E, 0x44, 0xAE, 0x42, 0x60, 0x82,
    ];
    std::fs::write(assets.join("logo.png"), PNG_1X1).map_err(|e| format!("write logo: {e}"))?;

    let script = format!(
        r#"# Registers the PasteCanvas sparse package so the IExplorerCommand entries
# appear in the Windows 11 modern context menu. Run in an elevated PowerShell:
#   Add-AppxPackage -Register "<this dir>\explorer-menu-appxmanifest.xml"
Add-AppxPackage -Register "{manifest}"
Write-Host "Restart Explorer (or log off/on) to see the PasteCanvas context menu."
"#,
        manifest = manifest_path.display(),
    );
    let script_path = data_dir.join("register-explorer-menu.ps1");
    std::fs::write(&script_path, script).map_err(|e| format!("write script: {e}"))?;

    Ok(Some(script_path.display().to_string()))
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn sparse_package_assets_written_with_clsids() {
        let dir = tempfile::TempDir::new().expect("temp dir");
        let result =
            write_sparse_package_assets(dir.path(), r"C:\app\pastecanvas_explorer_dll.dll")
                .expect("write assets");
        assert!(result.is_some());

        let manifest_path = dir.path().join("explorer-menu-appxmanifest.xml");
        let manifest = std::fs::read_to_string(&manifest_path).expect("read manifest");
        // Both CLSIDs must be present for the two commands
        assert!(manifest.contains(CLSID_INBOX));
        assert!(manifest.contains(CLSID_BOARD));
        // The fileExplorerContextMenus extension must be declared
        assert!(manifest.contains("windows.fileExplorerContextMenus"));
        // The COM server extension must reference the DLL
        assert!(manifest.contains("windows.comServer"));
        assert!(manifest.contains("pastecanvas_explorer_dll.dll"));

        // The registration script must reference the manifest
        let script_path = dir.path().join("register-explorer-menu.ps1");
        let script = std::fs::read_to_string(&script_path).expect("read script");
        assert!(script.contains("Add-AppxPackage"));
        assert!(script.contains("explorer-menu-appxmanifest.xml"));

        let logo = dir.path().join("Assets").join("logo.png");
        assert!(logo.exists(), "sparse package logo must exist");
    }

    #[test]
    fn apply_sparse_package_does_not_claim_modern_success_without_bridge() {
        let dir = tempfile::TempDir::new().expect("temp dir");
        let dll = dir.path().join(DLL_NAME);
        std::fs::write(&dll, b"dll").expect("fake dll");
        let (script, modern, err) =
            apply_sparse_package(dir.path(), &dll).expect("apply should not hard-fail");
        assert!(script.is_some());
        assert!(!modern, "missing bridge must not report modern success");
        assert!(err.is_some());
    }

    #[test]
    fn explorer_command_verbs_bind_handler_clsids() {
        let verbs = explorer_command_verbs();
        assert_eq!(verbs[0].1, "PasteCanvasInbox");
        assert_eq!(verbs[1].1, "PasteCanvasBoard");
        assert_eq!(verbs[0].0, CLSID_INBOX);
        assert_eq!(verbs[1].0, CLSID_BOARD);
        assert!(verbs[0].2.contains("Inbox"));
        assert!(verbs[1].2.contains("Board"));
    }
}
