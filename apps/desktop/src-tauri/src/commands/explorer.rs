//! Explorer context-menu integration commands (CAP-007).
//!
//! Registers Windows Explorer context-menu entries that invoke the
//! `pastecanvas-explorer` bridge with the selected file paths. The bridge runs
//! as a detached process so Explorer is never blocked (per PRD CAP-007).
//!
//! Also exposes `capture_files` for in-app file capture (CAP-002 drag-in).

use serde::Deserialize;
use tauri::State;

use crate::bootstrap::app_state::AppState;

const MENU_INBOX: &str = "PasteCanvas";
const MENU_BOARD: &str = "PasteCanvasBoard";

/// Resolves the path to the pastecanvas-explorer bridge executable.
fn explorer_exe_path() -> Result<std::path::PathBuf, String> {
    let exe = std::env::current_exe().map_err(|e| e.to_string())?;
    let dir = exe
        .parent()
        .ok_or_else(|| "no exe parent dir".to_string())?;
    Ok(dir.join("pastecanvas-explorer.exe"))
}

/// Registers Explorer context-menu entries ("Send to Inbox" + "Send to last Board").
#[tauri::command]
pub async fn register_explorer_integration(_state: State<'_, AppState>) -> Result<(), String> {
    let exe = explorer_exe_path()?;
    if !exe.exists() {
        return Err(format!("explorer bridge not found at {}", exe.display()));
    }
    let exe_str = exe.display().to_string();

    #[cfg(target_os = "windows")]
    {
        use crate::platform::windows::reg;

        // "Send to PasteCanvas Inbox" — all files
        let inbox_key = format!(r"HKCR\*\shell\{MENU_INBOX}");
        reg::add_default(&inbox_key, "Send to PasteCanvas Inbox")?;
        let inbox_cmd = format!(r"HKCR\*\shell\{MENU_INBOX}\command");
        let inbox_cmdline = format!("\"{exe_str}\" \"%1\"");
        reg::add_default(&inbox_cmd, &inbox_cmdline)?;

        // "Send to PasteCanvas last Board" — all files
        let board_key = format!(r"HKCR\*\shell\{MENU_BOARD}");
        reg::add_default(&board_key, "Send to PasteCanvas Board")?;
        let board_cmd = format!(r"HKCR\*\shell\{MENU_BOARD}\command");
        let board_cmdline = format!("\"{exe_str}\" --board \"%1\"");
        reg::add_default(&board_cmd, &board_cmdline)?;

        // Directory (folder) entries — capture folder contents is out of scope for v1,
        // but we register the Inbox entry on directories for future use.
        let dir_key = format!(r"HKCR\Directory\shell\{MENU_INBOX}");
        reg::add_default(&dir_key, "Send to PasteCanvas Inbox")?;
        let dir_cmd = format!(r"HKCR\Directory\shell\{MENU_INBOX}\command");
        reg::add_default(&dir_cmd, &inbox_cmdline)?;
    }

    #[cfg(not(target_os = "windows"))]
    {
        let _ = _state;
        return Err("explorer integration is only supported on Windows".to_string());
    }

    tracing::info!(event = "explorer_integration_registered", exe = %exe_str);
    Ok(())
}

/// Removes the Explorer context-menu entries.
#[tauri::command]
pub async fn unregister_explorer_integration() -> Result<(), String> {
    #[cfg(target_os = "windows")]
    {
        use crate::platform::windows::reg;
        let keys = [
            format!(r"HKCR\*\shell\{MENU_INBOX}"),
            format!(r"HKCR\*\shell\{MENU_BOARD}"),
            format!(r"HKCR\Directory\shell\{MENU_INBOX}"),
        ];
        for key in keys {
            reg::delete_key(&key)?;
        }
    }
    tracing::info!(event = "explorer_integration_unregistered");
    Ok(())
}

#[derive(Debug, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct CaptureFilesRequest {
    pub paths: Vec<String>,
    /// Optional target board (CAP-002: 選擇目標 Board，可選).
    #[serde(default)]
    pub board_id: Option<String>,
}

/// Captures one or more files into the Inbox (CAP-002 drag-in / CAP-007).
/// Optionally places each captured item on the given board.
#[tauri::command]
pub async fn capture_files(
    state: State<'_, AppState>,
    request: CaptureFilesRequest,
) -> Result<Vec<String>, String> {
    if request.paths.is_empty() {
        return Err("no files provided".to_string());
    }

    let svc = crate::application::explorer_service::ExplorerCaptureService::new(
        state.db.clone(),
        state.device_id.clone(),
        state.data_dir.clone(),
    );

    let mut item_ids = Vec::new();
    let mut last_error: Option<String> = None;

    for (index, path) in request.paths.iter().enumerate() {
        match svc.capture_file(path).await {
            Ok(item_id) => {
                if let Some(ref board_id) = request.board_id {
                    if let Err(e) = svc.place_on_board(board_id, &item_id, index).await {
                        tracing::warn!(event = "capture_file_board_place_failed", error = %e);
                    }
                }
                item_ids.push(item_id.to_string());
            }
            Err(e) => {
                last_error = Some(format!("{path}: {e}"));
            }
        }
    }

    if item_ids.is_empty() {
        return Err(last_error.unwrap_or_else(|| "all captures failed".to_string()));
    }

    Ok(item_ids)
}
