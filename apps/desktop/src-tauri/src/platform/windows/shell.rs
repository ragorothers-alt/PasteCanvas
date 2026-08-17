//! Opens URLs/files with the Windows shell (ShellExecuteW).
//! Used by SRC-003 "open source" search action.
//!
//! Per AGENTS.md: All Windows-only code lives behind platform abstraction.
//! Security: only http(s) URLs and existing file paths are allowed —
//! prevents executing arbitrary protocols or pasted content.

/// Validates and opens a URL or file path with the default Windows handler.
pub fn open_with_shell(target: &str) -> Result<(), String> {
    let target = target.trim();
    if target.is_empty() {
        return Err("empty target".to_string());
    }

    // Security: restrict URL schemes to http/https; anything else must be an existing file
    let lower = target.to_lowercase();
    if lower.starts_with("http://") || lower.starts_with("https://") {
        // Disallow control characters and quotes in URLs
        if target
            .chars()
            .any(|c| c.is_control() || c == '"' || c == '\'')
        {
            return Err("invalid characters in URL".to_string());
        }
    } else if lower.contains("://") {
        // Other schemes (javascript:, file:, custom) are rejected
        return Err("unsupported URL scheme".to_string());
    } else {
        // Treat as file path — must exist, and must not be an executable /
        // script / shortcut. ShellExecuteW "open" would otherwise launch it.
        let path = std::path::Path::new(target);
        if !path.exists() {
            return Err("file not found".to_string());
        }
        if is_blocked_executable(path) {
            return Err("opening executable files is not allowed".to_string());
        }
    }

    shell_execute(target)
}

/// Extensions that ShellExecuteW "open" would run rather than reveal.
const BLOCKED_EXECUTABLE_EXTS: &[&str] = &[
    "exe", "bat", "cmd", "com", "cpl", "hta", "jar", "js", "jse", "lnk", "msi", "msp", "pif",
    "ps1", "psm1", "reg", "scr", "vbe", "vbs", "wsf", "wsh",
];

fn is_blocked_executable(path: &std::path::Path) -> bool {
    path.extension()
        .and_then(|e| e.to_str())
        .is_some_and(|ext| {
            BLOCKED_EXECUTABLE_EXTS
                .iter()
                .any(|blocked| ext.eq_ignore_ascii_case(blocked))
        })
}

#[cfg(target_os = "windows")]
fn shell_execute(target: &str) -> Result<(), String> {
    use std::os::windows::ffi::OsStrExt;

    type HInstance = *mut std::ffi::c_void;
    type HWnd = *mut std::ffi::c_void;

    unsafe extern "system" {
        fn ShellExecuteW(
            hwnd: HWnd,
            operation: *const u16,
            file: *const u16,
            parameters: *const u16,
            directory: *const u16,
            show_cmd: i32,
        ) -> HInstance;
    }

    let wide: Vec<u16> = std::ffi::OsStr::new(target)
        .encode_wide()
        .chain(std::iter::once(0))
        .collect();
    let verb: Vec<u16> = "open\0".encode_utf16().collect();

    // ShellExecuteW returns a value > 32 on success
    let result = unsafe {
        ShellExecuteW(
            std::ptr::null_mut(),
            verb.as_ptr(),
            wide.as_ptr(),
            std::ptr::null(),
            std::ptr::null(),
            1, // SW_SHOWNORMAL
        )
    };

    if (result as usize) > 32 {
        Ok(())
    } else {
        Err(format!("ShellExecute failed with code {}", result as usize))
    }
}

#[cfg(not(target_os = "windows"))]
fn shell_execute(_target: &str) -> Result<(), String> {
    Err("open_with_shell is only supported on Windows".to_string())
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn rejects_empty_target() {
        assert!(open_with_shell("").is_err());
        assert!(open_with_shell("   ").is_err());
    }

    #[test]
    fn rejects_javascript_scheme() {
        assert!(open_with_shell("javascript:alert(1)").is_err());
    }

    #[test]
    fn rejects_custom_scheme() {
        assert!(open_with_shell("myapp://run-command").is_err());
    }

    #[test]
    fn rejects_url_with_control_chars() {
        assert!(open_with_shell("https://example.com/\u{0000}").is_err());
    }

    #[test]
    fn rejects_nonexistent_file() {
        assert!(open_with_shell("C:\\definitely\\not\\a\\real\\file_xyz.txt").is_err());
    }

    #[test]
    fn rejects_executable_extensions() {
        let dir = std::env::temp_dir().join(format!("pc-shell-{}", uuid::Uuid::now_v7()));
        std::fs::create_dir_all(&dir).expect("tmpdir");
        for ext in ["exe", "bat", "ps1", "lnk", "cmd"] {
            let path = dir.join(format!("payload.{ext}"));
            std::fs::write(&path, b"not really executable").expect("write");
            let err = open_with_shell(path.to_str().unwrap()).unwrap_err();
            assert!(
                err.contains("executable"),
                "expected executable rejection for .{ext}, got {err}"
            );
        }
        let _ = std::fs::remove_dir_all(&dir);
    }
}
