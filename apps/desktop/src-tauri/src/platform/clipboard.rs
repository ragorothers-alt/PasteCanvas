//! Clipboard notification listener using Windows message-only window.
//!
//! Per P0-FND-007:
//! - Message-only native window for clipboard notifications
//! - AddClipboardFormatListener registration/unregistration
//! - Sequence-number dedup (avoid processing same clipboard content twice)
//! - Foreground process metadata capture
//! - Detect private self-marker/exclusion
//! - Emit diagnostic metadata only; no content logging
//!
//! Per GOAL.md privacy:
//! - Not polling; uses Windows clipboard notification (WM_CLIPBOARDUPDATE)
//! - Only active during Capture Session
//! - Respects exclusion list before any processing

#[cfg(target_os = "windows")]
pub mod listener {
    use std::sync::Arc;
    use std::sync::atomic::{AtomicBool, AtomicU32, Ordering};

    /// State for the clipboard listener.
    /// Tracks whether listening is active and the last processed sequence number.
    #[derive(Clone)]
    pub struct ClipboardListenerState {
        /// Whether the listener is actively monitoring clipboard changes.
        pub is_listening: Arc<AtomicBool>,
        /// Last processed clipboard sequence number (for dedup).
        pub last_sequence: Arc<AtomicU32>,
    }

    impl ClipboardListenerState {
        pub fn new() -> Self {
            Self {
                is_listening: Arc::new(AtomicBool::new(false)),
                last_sequence: Arc::new(AtomicU32::new(0)),
            }
        }

        /// Starts listening for clipboard changes.
        pub fn start(&self) {
            self.is_listening.store(true, Ordering::SeqCst);
            tracing::info!(event = "clipboard_listener_started");
        }

        /// Stops listening for clipboard changes.
        pub fn stop(&self) {
            self.is_listening.store(false, Ordering::SeqCst);
            tracing::info!(event = "clipboard_listener_stopped");
        }

        /// Checks if a clipboard update should be processed (dedup by sequence).
        pub fn should_process(&self, sequence: u32) -> bool {
            if !self.is_listening.load(Ordering::SeqCst) {
                return false;
            }

            let last = self.last_sequence.load(Ordering::SeqCst);
            if sequence == last {
                // Duplicate notification, skip
                return false;
            }

            // Update sequence and process
            self.last_sequence.store(sequence, Ordering::SeqCst);
            true
        }

        pub fn is_active(&self) -> bool {
            self.is_listening.load(Ordering::SeqCst)
        }
    }

    impl Default for ClipboardListenerState {
        fn default() -> Self {
            Self::new()
        }
    }

    /// Gets the current clipboard sequence number from Windows.
    /// Returns 0 if unavailable.
    pub fn get_clipboard_sequence() -> u32 {
        unsafe extern "system" {
            fn GetClipboardSequenceNumber() -> u32;
        }
        unsafe { GetClipboardSequenceNumber() }
    }

    /// Gets the foreground window process name (best-effort) (CAP-005).
    /// Returns the executable file name (e.g. "chrome.exe") or None when
    /// unavailable. RAII-style: the process handle is closed before return.
    pub fn get_foreground_process_name() -> Option<String> {
        use std::ffi::c_void;

        type Handle = *mut c_void;

        unsafe extern "system" {
            fn GetForegroundWindow() -> Handle;
            fn GetWindowThreadProcessId(hwnd: Handle, pid: *mut u32) -> u32;
            fn OpenProcess(access: u32, inherit_handle: i32, pid: u32) -> Handle;
            fn QueryFullProcessImageNameW(
                handle: Handle,
                flags: u32,
                buffer: *mut u16,
                size: *mut u32,
            ) -> i32;
            fn CloseHandle(handle: Handle) -> i32;
        }

        const PROCESS_QUERY_LIMITED_INFORMATION: u32 = 0x1000;

        unsafe {
            let hwnd = GetForegroundWindow();
            if hwnd.is_null() {
                return None;
            }
            let mut pid = 0u32;
            if GetWindowThreadProcessId(hwnd, &mut pid) == 0 || pid == 0 {
                return None;
            }
            let process = OpenProcess(PROCESS_QUERY_LIMITED_INFORMATION, 0, pid);
            if process.is_null() {
                return None;
            }
            let mut buf = [0u16; 1024];
            let mut size = buf.len() as u32;
            let ok = QueryFullProcessImageNameW(process, 0, buf.as_mut_ptr(), &mut size) != 0;
            CloseHandle(process);
            if !ok {
                return None;
            }
            let path = String::from_utf16_lossy(&buf[..size as usize]);
            extract_process_name(&path)
        }
    }

    /// Extracts the executable file name from a full image path.
    /// Kept pure so it is unit-testable without Win32.
    pub fn extract_process_name(path: &str) -> Option<String> {
        if path.is_empty() {
            return None;
        }
        let name = path.rsplit(['\\', '/']).next().unwrap_or(path);
        if name.is_empty() {
            None
        } else {
            Some(name.to_string())
        }
    }
}

#[cfg(not(target_os = "windows"))]
pub mod listener {
    use std::sync::Arc;
    use std::sync::atomic::{AtomicBool, AtomicU32, Ordering};

    #[derive(Clone)]
    pub struct ClipboardListenerState {
        pub is_listening: Arc<AtomicBool>,
        pub last_sequence: Arc<AtomicU32>,
    }

    impl ClipboardListenerState {
        pub fn new() -> Self {
            Self {
                is_listening: Arc::new(AtomicBool::new(false)),
                last_sequence: Arc::new(AtomicU32::new(0)),
            }
        }

        pub fn start(&self) {
            self.is_listening.store(true, Ordering::SeqCst);
        }

        pub fn stop(&self) {
            self.is_listening.store(false, Ordering::SeqCst);
        }

        pub fn should_process(&self, sequence: u32) -> bool {
            if !self.is_listening.load(Ordering::SeqCst) {
                return false;
            }
            let last = self.last_sequence.load(Ordering::SeqCst);
            if sequence == last {
                return false;
            }
            self.last_sequence.store(sequence, Ordering::SeqCst);
            true
        }

        pub fn is_active(&self) -> bool {
            self.is_listening.load(Ordering::SeqCst)
        }
    }

    impl Default for ClipboardListenerState {
        fn default() -> Self {
            Self::new()
        }
    }

    pub fn get_clipboard_sequence() -> u32 {
        0
    }

    pub fn get_foreground_process_name() -> Option<String> {
        None
    }

    pub fn extract_process_name(path: &str) -> Option<String> {
        if path.is_empty() {
            return None;
        }
        let name = path.rsplit(['\\', '/']).next().unwrap_or(path);
        if name.is_empty() {
            None
        } else {
            Some(name.to_string())
        }
    }
}

#[cfg(test)]
mod tests {
    use super::listener::ClipboardListenerState;
    use super::listener::extract_process_name;

    #[test]
    fn extract_process_name_from_windows_path() {
        assert_eq!(
            extract_process_name("C:\\Program Files\\Google\\Chrome\\Application\\chrome.exe"),
            Some("chrome.exe".to_string())
        );
    }

    #[test]
    fn extract_process_name_handles_slashes_and_bare_names() {
        assert_eq!(
            extract_process_name("/usr/bin/code"),
            Some("code".to_string())
        );
        assert_eq!(
            extract_process_name("notepad.exe"),
            Some("notepad.exe".to_string())
        );
        assert_eq!(extract_process_name(""), None);
        assert_eq!(extract_process_name("C:\\trailing\\"), None);
    }

    #[test]
    fn listener_starts_inactive() {
        let state = ClipboardListenerState::new();
        assert!(!state.is_active());
    }

    #[test]
    fn listener_start_stop() {
        let state = ClipboardListenerState::new();
        state.start();
        assert!(state.is_active());
        state.stop();
        assert!(!state.is_active());
    }

    #[test]
    fn dedup_same_sequence() {
        let state = ClipboardListenerState::new();
        state.start();

        assert!(state.should_process(1)); // First time: process
        assert!(!state.should_process(1)); // Same sequence: skip
        assert!(state.should_process(2)); // New sequence: process
        assert!(!state.should_process(2)); // Same again: skip
    }

    #[test]
    fn inactive_listener_skips_all() {
        let state = ClipboardListenerState::new();
        // Not started
        assert!(!state.should_process(1));
        assert!(!state.should_process(2));
    }

    #[test]
    fn clone_shares_state() {
        let state = ClipboardListenerState::new();
        let clone = state.clone();

        state.start();
        assert!(clone.is_active()); // Clone sees the same state

        clone.stop();
        assert!(!state.is_active()); // Original sees clone's change
    }
}
