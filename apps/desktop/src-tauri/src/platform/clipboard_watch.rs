//! Event-driven clipboard change watcher (P0-FND-007, CAP-003/CAP-004).
//!
//! Uses a Windows message-only window + AddClipboardFormatListener so the app
//! receives WM_CLIPBOARDUPDATE notifications instead of polling (GOAL.md
//! performance: "Clipboard 使用事件 listener，不 polling").
//!
//! Privacy (CAP-004): when a change arrives, the pipeline first checks the
//! foreground process against the exclusion list and the Windows
//! "ExcludeClipboardFormat" monitor marker; skipped events log an anonymous
//! reason code only — never content.

#[cfg(target_os = "windows")]
mod win_impl {
    use std::ffi::c_void;
    use std::sync::atomic::{AtomicBool, Ordering};
    use std::sync::{Arc, Mutex};

    /// Callback invoked on every clipboard update.
    pub type WatchCallback = Arc<dyn Fn() + Send + Sync>;

    static CALLBACK: Mutex<Option<WatchCallback>> = Mutex::new(None);
    static WATCHER_HWND: Mutex<isize> = Mutex::new(0);
    static WATCHER_RUNNING: AtomicBool = AtomicBool::new(false);

    const WM_DESTROY: u32 = 0x0002;
    const WM_CLOSE: u32 = 0x0010;
    const WM_CLIPBOARDUPDATE: u32 = 0x031D;
    const HWND_MESSAGE: isize = -3;

    #[repr(C)]
    struct Point {
        x: i32,
        y: i32,
    }

    #[repr(C)]
    struct Msg {
        hwnd: *mut c_void,
        message: u32,
        wparam: usize,
        lparam: isize,
        time: u32,
        pt: Point,
    }

    type WndProc = unsafe extern "system" fn(*mut c_void, u32, usize, isize) -> isize;

    #[repr(C)]
    struct WndClassExW {
        cb_size: u32,
        style: u32,
        lpfn_wnd_proc: WndProc,
        cb_cls_extra: i32,
        cb_wnd_extra: i32,
        h_instance: *mut c_void,
        h_icon: *mut c_void,
        h_cursor: *mut c_void,
        hbr_background: *mut c_void,
        lpsz_menu_name: *const u16,
        lpsz_class_name: *const u16,
        h_icon_sm: *mut c_void,
    }

    unsafe extern "system" {
        fn GetModuleHandleW(name: *const u16) -> *mut c_void;
        fn RegisterClassExW(wc: *const WndClassExW) -> u16;
        fn CreateWindowExW(
            ex_style: u32,
            class_name: *const u16,
            window_name: *const u16,
            style: u32,
            x: i32,
            y: i32,
            w: i32,
            h: i32,
            parent: *mut c_void,
            menu: *mut c_void,
            instance: *mut c_void,
            param: *mut c_void,
        ) -> *mut c_void;
        fn AddClipboardFormatListener(hwnd: *mut c_void) -> i32;
        fn RemoveClipboardFormatListener(hwnd: *mut c_void) -> i32;
        fn GetMessageW(msg: *mut Msg, hwnd: *mut c_void, min: u32, max: u32) -> i32;
        fn TranslateMessage(msg: *const Msg) -> i32;
        fn DispatchMessageW(msg: *const Msg) -> isize;
        fn DestroyWindow(hwnd: *mut c_void) -> i32;
        fn PostMessageW(hwnd: *mut c_void, msg: u32, wparam: usize, lparam: isize) -> i32;
        fn DefWindowProcW(hwnd: *mut c_void, msg: u32, wparam: usize, lparam: isize) -> isize;
        fn PostQuitMessage(exit_code: i32);
        fn IsClipboardFormatAvailable(format: u32) -> i32;
        fn RegisterClipboardFormatW(name: *const u16) -> u32;
    }

    unsafe extern "system" fn wndproc(
        hwnd: *mut c_void,
        msg: u32,
        wparam: usize,
        lparam: isize,
    ) -> isize {
        unsafe {
            match msg {
                WM_CLIPBOARDUPDATE => {
                    // Clone the Arc out of the lock, then invoke — never call
                    // user code while holding the lock.
                    let cb = CALLBACK.lock().ok().and_then(|g| g.clone());
                    if let Some(cb) = cb {
                        cb();
                    }
                    0
                }
                WM_CLOSE => {
                    DestroyWindow(hwnd);
                    0
                }
                WM_DESTROY => {
                    PostQuitMessage(0);
                    0
                }
                _ => DefWindowProcW(hwnd, msg, wparam, lparam),
            }
        }
    }

    fn run_message_loop() {
        unsafe {
            let class_name: Vec<u16> = "PasteCanvasClipboardWatcher\0".encode_utf16().collect();
            let wc = WndClassExW {
                cb_size: std::mem::size_of::<WndClassExW>() as u32,
                style: 0,
                lpfn_wnd_proc: wndproc,
                cb_cls_extra: 0,
                cb_wnd_extra: 0,
                h_instance: GetModuleHandleW(std::ptr::null()),
                h_icon: std::ptr::null_mut(),
                h_cursor: std::ptr::null_mut(),
                hbr_background: std::ptr::null_mut(),
                lpsz_menu_name: std::ptr::null(),
                lpsz_class_name: class_name.as_ptr(),
                h_icon_sm: std::ptr::null_mut(),
            };
            // Re-registering an existing class fails harmlessly — ignore.
            RegisterClassExW(&wc);

            let hwnd = CreateWindowExW(
                0,
                class_name.as_ptr(),
                std::ptr::null(),
                0,
                0,
                0,
                0,
                0,
                HWND_MESSAGE as *mut c_void,
                std::ptr::null_mut(),
                GetModuleHandleW(std::ptr::null()),
                std::ptr::null_mut(),
            );
            if hwnd.is_null() {
                WATCHER_RUNNING.store(false, Ordering::SeqCst);
                tracing::error!(event = "clipboard_watcher_window_failed");
                return;
            }
            if let Ok(mut h) = WATCHER_HWND.lock() {
                *h = hwnd as isize;
            }
            AddClipboardFormatListener(hwnd);

            let mut msg = std::mem::zeroed::<Msg>();
            loop {
                let r = GetMessageW(&mut msg, std::ptr::null_mut(), 0, 0);
                if r <= 0 {
                    break; // 0 = WM_QUIT, -1 = error
                }
                TranslateMessage(&msg);
                DispatchMessageW(&msg);
            }

            RemoveClipboardFormatListener(hwnd);
            if let Ok(mut h) = WATCHER_HWND.lock() {
                *h = 0;
            }
            WATCHER_RUNNING.store(false, Ordering::SeqCst);
            tracing::info!(event = "clipboard_watcher_stopped");
        }
    }

    /// Starts (or re-points) the clipboard watcher. Idempotent while running.
    pub fn start_clipboard_watch(on_change: WatchCallback) -> Result<(), String> {
        if let Ok(mut cb) = CALLBACK.lock() {
            *cb = Some(on_change);
        }
        if WATCHER_RUNNING.swap(true, Ordering::SeqCst) {
            return Ok(()); // already running; callback replaced
        }
        std::thread::Builder::new()
            .name("clipboard-watcher".into())
            .spawn(run_message_loop)
            .map_err(|e| e.to_string())?;
        tracing::info!(event = "clipboard_watcher_started");
        Ok(())
    }

    /// Stops the watcher (posts WM_CLOSE to its message-only window).
    pub fn stop_clipboard_watch() {
        if let Ok(mut cb) = CALLBACK.lock() {
            *cb = None;
        }
        let hwnd = WATCHER_HWND.lock().map(|h| *h).unwrap_or(0);
        if hwnd != 0 {
            unsafe {
                PostMessageW(hwnd as *mut c_void, WM_CLOSE, 0, 0);
            }
        }
    }

    /// CAP-004: true when the clipboard carries the Windows monitor-exclusion
    /// marker format (e.g. set by clipboard managers protecting sensitive
    /// entries). Anonymous — inspects format availability only, never content.
    pub fn has_clipboard_exclusion_marker() -> bool {
        unsafe {
            let name: Vec<u16> = "ExcludeClipboardFormat\0".encode_utf16().collect();
            let fmt = RegisterClipboardFormatW(name.as_ptr());
            fmt != 0 && IsClipboardFormatAvailable(fmt) != 0
        }
    }
}

#[cfg(target_os = "windows")]
pub use win_impl::*;

// Non-Windows stubs keep the portable callers compiling everywhere.
#[cfg(not(target_os = "windows"))]
mod stub {
    use std::sync::Arc;

    pub type WatchCallback = Arc<dyn Fn() + Send + Sync>;

    pub fn start_clipboard_watch(_on_change: WatchCallback) -> Result<(), String> {
        Ok(())
    }

    pub fn stop_clipboard_watch() {}

    pub fn has_clipboard_exclusion_marker() -> bool {
        false
    }
}

#[cfg(not(target_os = "windows"))]
pub use stub::*;
