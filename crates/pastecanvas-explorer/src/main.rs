//! PasteCanvas Explorer context-menu bridge (CAP-007).
//!
//! Invoked by Windows Explorer context-menu entries with one or more file paths.
//! Runs as a separate detached process so it never blocks the Explorer UI
//! (per PRD CAP-007). Captures each file into the Inbox; with `--board` it also
//! places the items on the most recently used board.
//!
//! Usage:
//!   pastecanvas-explorer [--board] <file> [<file> ...]

use pastecanvas_desktop_lib::application::explorer_service::ExplorerCaptureService;
use pastecanvas_desktop_lib::bootstrap::external;
use pastecanvas_desktop_lib::domain::value_objects::EntityId;

#[tokio::main(flavor = "current_thread")]
async fn main() {
    let args: Vec<String> = std::env::args().skip(1).collect();

    // Parse the optional --board flag and collect file paths
    let place_on_board = args.iter().any(|a| a == "--board");
    let files: Vec<&String> = args.iter().filter(|a| a.as_str() != "--board").collect();

    if files.is_empty() {
        eprintln!("usage: pastecanvas-explorer [--board] <file> [<file> ...]");
        std::process::exit(1);
    }

    // Open DB + run migrations (shared bootstrap with the NMH)
    let (db, data_dir) = match external::open_migrated_db().await {
        Ok(ok) => ok,
        Err(e) => {
            eprintln!("{e}");
            std::process::exit(1);
        }
    };

    let svc = ExplorerCaptureService::new(db, EntityId::new(), data_dir.clone());

    // If placing on a board, resolve the most recently used board once
    let board_id = if place_on_board {
        match svc.last_used_board_id().await {
            Ok(Some(id)) => Some(id),
            _ => None,
        }
    } else {
        None
    };

    let mut captured = 0usize;
    let mut failed = 0usize;

    for (index, file) in files.iter().enumerate() {
        match svc.capture_file(file).await {
            Ok(item_id) => {
                captured += 1;
                // Place on the last-used board if requested and available
                if let Some(ref bid) = board_id {
                    if let Err(e) = svc.place_on_board(bid, &item_id, index).await {
                        eprintln!("place on board failed for {file}: {e}");
                    }
                }
            }
            Err(e) => {
                failed += 1;
                eprintln!("capture failed for {file}: {e}");
            }
        }
    }

    // CAP-009: tell the running app about the new items (best effort; if the
    // app is not running, this activates it so the user sees the capture).
    if captured > 0 {
        if let Ok(exe) = std::env::current_exe() {
            pastecanvas_desktop_lib::platform::notify::notify_capture_app(&exe);
        }
    }

    // Non-zero exit if every file failed; partial success exits 0.
    if captured == 0 && failed > 0 {
        std::process::exit(1);
    }
}
