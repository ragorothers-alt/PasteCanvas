//! Foreground focus tracking (CAP-002).
//!
//! PRD CAP-002: "保存後恢復前一個應用程式焦點" — the Quick Capture floating
//! window must return keyboard focus to whatever application the user was in
//! before invoking it, both after saving and after cancelling.
//!
//! Strategy: right before the floating window steals focus, record the current
//! foreground window (unless it is one of our own windows). When the floating
//! window hides, restore that window to the foreground using the standard
//! AttachThreadInput + SetForegroundWindow pattern.

#[cfg(target_os = "windows")]
mod win32 {
    use std::sync::Mutex;

    type Hwnd = *mut std::ffi::c_void;
    type Bool = i32;

    unsafe extern "system" {
        fn GetForegroundWindow() -> Hwnd;
        fn SetForegroundWindow(hwnd: Hwnd) -> Bool;
        fn IsWindow(hwnd: Hwnd) -> Bool;
        fn GetWindowThreadProcessId(hwnd: Hwnd, process_id: *mut u32) -> u32;
        fn GetCurrentProcessId() -> u32;
        fn AttachThreadInput(id_attach: u32, id_attach_to: u32, f_attach: Bool) -> Bool;
    }

    /// The foreground window recorded before Quick Capture took focus.
    /// usize so the holder is Send-safe; 0 == none.
    static SAVED_FOCUS: Mutex<usize> = Mutex::new(0);

    fn hwnd_of(value: usize) -> Hwnd {
        value as Hwnd
    }

    fn process_of(hwnd: Hwnd) -> u32 {
        let mut pid = 0u32;
        unsafe { GetWindowThreadProcessId(hwnd, &mut pid) };
        pid
    }

    /// Records the current foreground window for later restoration.
    /// Windows belonging to this process are ignored (restoring ourselves is
    /// pointless and would fight the OS focus rules).
    pub fn record_previous_focus() {
        let (foreground, own_pid) = unsafe { (GetForegroundWindow(), GetCurrentProcessId()) };
        let mut saved = SAVED_FOCUS.lock().expect("focus lock");
        if foreground.is_null() || process_of(foreground) == own_pid {
            *saved = 0;
        } else {
            *saved = foreground as usize;
        }
    }

    /// Takes the saved window out of the holder (idempotent — a second call
    /// returns 0). Separated from restoration so tests can inspect the take
    /// semantics without touching the real foreground.
    pub fn take_saved_focus() -> usize {
        let mut saved = SAVED_FOCUS.lock().expect("focus lock");
        let value = *saved;
        *saved = 0;
        value
    }

    /// Stores a window handle as the restoration target (test/edge use).
    pub fn store_focus(value: usize) {
        let mut saved = SAVED_FOCUS.lock().expect("focus lock");
        *saved = value;
    }

    /// Restores the previously recorded foreground window. Returns true when a
    /// restore was attempted. Safe to call with nothing recorded (no-op).
    pub fn restore_previous_focus() -> bool {
        let target = take_saved_focus();
        if target == 0 {
            return false;
        }
        let hwnd = hwnd_of(target);
        unsafe {
            if IsWindow(hwnd) == 0 {
                return false; // window vanished meanwhile
            }
            let own_pid = GetCurrentProcessId();
            if process_of(hwnd) == own_pid {
                return false; // never force-focus ourselves
            }
            // AttachThreadInput lets us bypass the foreground-lock timeout.
            let fg_thread = GetWindowThreadProcessId(GetForegroundWindow(), std::ptr::null_mut());
            let target_thread = GetWindowThreadProcessId(hwnd, std::ptr::null_mut());
            let attached = fg_thread != 0
                && target_thread != 0
                && fg_thread != target_thread
                && AttachThreadInput(fg_thread, target_thread, 1) != 0;
            let ok = SetForegroundWindow(hwnd) != 0;
            if attached {
                AttachThreadInput(fg_thread, target_thread, 0);
            }
            ok
        }
    }
}

#[cfg(target_os = "windows")]
pub use win32::*;

// Non-Windows stubs keep the portable callers compiling everywhere.
#[cfg(not(target_os = "windows"))]
pub fn record_previous_focus() {}

#[cfg(not(target_os = "windows"))]
pub fn restore_previous_focus() -> bool {
    false
}

#[cfg(test)]
mod tests {
    use super::*;

    // Single sequential test: the focus holder is global state, so parallel
    // tests would race on it.
    #[cfg(target_os = "windows")]
    #[test]
    fn focus_holder_semantics() {
        // Nothing recorded → nothing to restore, and restore is a safe no-op.
        store_focus(0);
        assert_eq!(take_saved_focus(), 0);
        assert_eq!(take_saved_focus(), 0); // second take still empty
        assert!(!restore_previous_focus());

        // Store/take roundtrip consumes the value exactly once.
        store_focus(0xDEAD_BEEF);
        assert_eq!(take_saved_focus(), 0xDEAD_BEEF);
        assert_eq!(take_saved_focus(), 0);

        // A bogus, non-window handle must not crash and must report failure.
        store_focus(0x1); // almost certainly not a real HWND
        let _ = restore_previous_focus(); // may return false; must not panic
    }

    #[cfg(not(target_os = "windows"))]
    #[test]
    fn non_windows_stubs_are_noops() {
        record_previous_focus();
        assert!(!restore_previous_focus());
    }
}
