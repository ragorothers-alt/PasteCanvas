//! Windows Explorer context-menu registration (CAP-007).
//!
//! Registers the `pastecanvas_explorer_dll` COM server (IExplorerCommand) so the
//! "Send to PasteCanvas" entries appear in Explorer. Registration is written under
//! HKCU\Software\Classes (per-user, no admin required — local-first friendly).
//!
//! For the Windows 11 *modern* context menu, an MSIX sparse package is additionally
//! required; `register_modern_explorer_menu` also emits the sparse-package manifest
//! + registration script under the app data dir so it can be applied with one command.

use serde::Serialize;
use tauri::State;

use crate::bootstrap::app_state::AppState;

const CLSID_INBOX: &str = "{7e4a0c1a-6f2c-4a31-9c7a-1a2b3c4d5e01}";
const CLSID_BOARD: &str = "{7e4a0c1a-6f2c-4a31-9c7a-1a2b3c4d5e02}";
const DLL_NAME: &str = "pastecanvas_explorer_dll.dll";

#[derive(Debug, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct ExplorerMenuRegistrationDto {
    pub dll_path: String,
    pub sparse_manifest_path: Option<String>,
    pub registered: bool,
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
fn reg_add(key: &str, value_name: Option<&str>, data: &str) {
    use std::process::Command;
    let mut cmd_args: Vec<String> = vec!["add".into(), key.into()];
    if let Some(v) = value_name {
        cmd_args.push("/v".into());
        cmd_args.push(v.into());
    } else {
        cmd_args.push("/ve".into());
    }
    cmd_args.push("/t".into());
    cmd_args.push("REG_SZ".into());
    cmd_args.push("/d".into());
    cmd_args.push(data.into());
    cmd_args.push("/f".into());
    let _ = Command::new("reg").args(&cmd_args).output();
}

/// Registers the IExplorerCommand COM server + shell entries for files & folders.
#[tauri::command]
pub async fn register_modern_explorer_menu(
    state: State<'_, AppState>,
) -> Result<ExplorerMenuRegistrationDto, String> {
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
            reg_add(&clsid_key, None, "PasteCanvas Explorer Command");
            reg_add(&inproc, None, &dll_str);
            reg_add(&inproc, Some("ThreadingModel"), "Apartment");
        }
        // Shell entries for all files and directories
        for root in [
            r"HKCU\Software\Classes\*",
            r"HKCU\Software\Classes\Directory",
        ] {
            for clsid in [CLSID_INBOX, CLSID_BOARD] {
                let shell_key = format!(r"{root}\shell\{clsid}");
                reg_add(&shell_key, None, "Send to PasteCanvas");
            }
        }
    }

    // Emit a sparse-package manifest + registration script for the Win11 modern menu
    let sparse_manifest = write_sparse_package_assets(&state.data_dir, &dll_str)?;

    tracing::info!(event = "explorer_menu_registered", dll = %dll_str);
    Ok(ExplorerMenuRegistrationDto {
        dll_path: dll_str,
        sparse_manifest_path: sparse_manifest,
        registered: true,
    })
}

/// Removes the COM server + shell entries.
#[tauri::command]
pub async fn unregister_modern_explorer_menu() -> Result<(), String> {
    #[cfg(target_os = "windows")]
    {
        use std::process::Command;
        let mut keys = Vec::new();
        for clsid in [CLSID_INBOX, CLSID_BOARD] {
            keys.push(format!(r"HKCU\Software\Classes\CLSID\{clsid}"));
            for root in [
                r"HKCU\Software\Classes\*",
                r"HKCU\Software\Classes\Directory",
            ] {
                keys.push(format!(r"{root}\shell\{clsid}"));
            }
        }
        for key in keys {
            let _ = Command::new("reg").args(["delete", &key, "/f"]).output();
        }
    }
    tracing::info!(event = "explorer_menu_unregistered");
    Ok(())
}

/// Writes an MSIX sparse-package manifest + PowerShell registration script so the
/// entries can surface in the Windows 11 modern context menu. Returns the script path.
fn write_sparse_package_assets(
    data_dir: &std::path::Path,
    dll_path: &str,
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
        dll = dll_path,
    );

    let manifest_path = data_dir.join("explorer-menu-appxmanifest.xml");
    std::fs::write(&manifest_path, manifest).map_err(|e| format!("write manifest: {e}"))?;

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
    }
}
