//! CAP-009: cross-process capture notification.
//!
//! Out-of-process integrations (Explorer bridge, browser Native Messaging
//! host) write captures directly into the shared SQLite database. The running
//! desktop app must then learn about the new items to refresh its Inbox.
//!
//! Mechanism: the integration launches the main executable with a marker
//! argument. When an instance is already running, `tauri-plugin-single-instance`
//! forwards the arguments to it (the launcher exits immediately); the callback
//! emits the inbox-refresh event. When no instance is running, the launch
//! activates the app (Phase-4 checklist: "app-not-running activation").
//!
//! Notification is advisory: capture has already been committed durably to the
//! database, so a failed notification must never fail the capture.

use std::path::Path;

/// Marker argument identifying a capture-notification launch.
pub const CAPTURE_NOTIFY_ARG: &str = "--pastecanvas-capture-notify";

/// Candidate main binary names: bundled release layout and cargo build layout.
#[cfg(target_os = "windows")]
const MAIN_EXE_CANDIDATES: &[&str] = &["PasteCanvas.exe", "pastecanvas-desktop.exe"];

#[cfg(not(target_os = "windows"))]
const MAIN_EXE_CANDIDATES: &[&str] = &["PasteCanvas", "pastecanvas-desktop"];

/// True when the given launch arguments carry the capture-notification marker.
pub fn is_capture_notify(args: &[String]) -> bool {
    args.iter().any(|a| a == CAPTURE_NOTIFY_ARG)
}

/// Best-effort, non-blocking notification of the main app.
///
/// `current_exe` is the integration's own executable path; the main app binary
/// is expected next to it. Returns true when a notification launch started.
/// Never panics — failures are reported as `false`.
pub fn notify_capture_app(current_exe: &Path) -> bool {
    let Some(dir) = current_exe.parent() else {
        return false;
    };
    for name in MAIN_EXE_CANDIDATES {
        let candidate = dir.join(name);
        if !candidate.exists() || candidate == current_exe {
            continue;
        }
        match std::process::Command::new(&candidate)
            .arg(CAPTURE_NOTIFY_ARG)
            .stdin(std::process::Stdio::null())
            .stdout(std::process::Stdio::null())
            .stderr(std::process::Stdio::null())
            .spawn()
        {
            Ok(_) => {
                tracing::info!(event = "capture_notify_sent");
                return true;
            }
            Err(e) => {
                tracing::debug!(event = "capture_notify_candidate_failed", binary = %candidate.display(), error = %e);
            }
        }
    }
    false
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn detects_marker_argument() {
        assert!(is_capture_notify(&[CAPTURE_NOTIFY_ARG.to_string()]));
        assert!(is_capture_notify(&[
            "pastecanvas.exe".to_string(),
            CAPTURE_NOTIFY_ARG.to_string()
        ]));
    }

    #[test]
    fn ignores_unrelated_arguments() {
        assert!(!is_capture_notify(&[]));
        assert!(!is_capture_notify(&["--minimized".to_string()]));
        assert!(!is_capture_notify(&["C:\\file.txt".to_string()]));
    }

    #[test]
    fn notification_fails_gracely_without_main_binary() {
        // A directory with no main exe candidates: nothing to launch.
        let temp = std::env::temp_dir().join(format!("pc-notify-test-{}", std::process::id()));
        let _ = std::fs::create_dir_all(&temp);
        let fake_bridge = temp.join("pastecanvas-explorer.exe");
        assert!(!notify_capture_app(&fake_bridge));
        let _ = std::fs::remove_dir_all(&temp);
    }

    #[test]
    fn notification_fails_gracely_without_parent_dir() {
        assert!(!notify_capture_app(Path::new("no-parent")));
    }

    #[test]
    fn never_launches_itself() {
        // Even if a candidate name collides with the current binary path,
        // notify must skip itself to avoid recursion.
        let temp = std::env::temp_dir().join(format!("pc-notify-self-{}", std::process::id()));
        let _ = std::fs::create_dir_all(&temp);
        let self_named = temp.join(MAIN_EXE_CANDIDATES[0]);
        let _ = std::fs::write(&self_named, b"stub");
        assert!(!notify_capture_app(&self_named));
        let _ = std::fs::remove_dir_all(&temp);
    }
}
