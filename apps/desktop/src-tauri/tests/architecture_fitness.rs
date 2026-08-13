//! ARC-001 architecture fitness tests.
//!
//! Enforces the modular-monolith layer boundaries (ADR-001) by scanning the
//! actual source files — a regression that reintroduces a forbidden dependency
//! fails CI even if everything still compiles.

#[cfg(test)]
mod tests {
    use std::path::{Path, PathBuf};

    fn src_root() -> PathBuf {
        Path::new(env!("CARGO_MANIFEST_DIR")).join("src")
    }

    /// Collects all .rs file contents under a directory.
    fn collect_rs(dir: &Path) -> Vec<(PathBuf, String)> {
        let mut out = Vec::new();
        let Ok(entries) = std::fs::read_dir(dir) else {
            return out;
        };
        for entry in entries.flatten() {
            let path = entry.path();
            if path.is_dir() {
                out.extend(collect_rs(&path));
            } else if path.extension().and_then(|e| e.to_str()) == Some("rs") {
                if let Ok(content) = std::fs::read_to_string(&path) {
                    out.push((path, content));
                }
            }
        }
        out
    }

    fn assert_no_match(dir: &str, patterns: &[&str], rule: &str) {
        let files = collect_rs(&src_root().join(dir));
        assert!(!files.is_empty(), "no source files found under src/{dir}");
        for (path, content) in &files {
            for pat in patterns {
                assert!(
                    !content.contains(pat),
                    "layer violation ({rule}): src/{dir}/{} contains {pat:?}",
                    path.strip_prefix(src_root())
                        .map(|p| p.display().to_string())
                        .unwrap_or_default()
                );
            }
        }
    }

    #[test]
    fn domain_layer_has_no_framework_dependencies() {
        assert_no_match(
            "domain",
            &["tauri", "sqlx", "windows::", "win32"],
            "domain must not depend on Tauri/sqlx/Win32",
        );
    }

    #[test]
    fn application_layer_does_not_call_commands() {
        assert_no_match(
            "application",
            &["crate::commands::", "tauri::"],
            "application must not depend on the commands layer or Tauri",
        );
    }

    #[test]
    fn platform_layer_does_not_call_commands() {
        // ARC-001: platform adapters use application services and
        // platform::window_manager — never the commands layer.
        assert_no_match(
            "platform",
            &["crate::commands::"],
            "platform must not depend on the commands layer",
        );
    }

    #[test]
    fn infrastructure_layer_is_independent_of_platform_and_commands() {
        assert_no_match(
            "infrastructure",
            &["crate::commands::", "crate::platform::", "tauri::"],
            "infrastructure must not depend on platform/commands/Tauri",
        );
    }

    #[test]
    fn frontend_does_not_touch_fs_or_win32_directly() {
        // Prohibition: React must never operate the filesystem/Windows API —
        // all effects go through typed invoke wrappers (api.ts).
        let frontend = Path::new(env!("CARGO_MANIFEST_DIR")).join("../src");
        let mut offenders = Vec::new();
        for (path, content) in collect_rs_with_ext(&frontend, &["ts", "tsx"]) {
            for pat in [
                "@tauri-apps/plugin-fs",
                "@tauri-apps/plugin-shell",
                "__TAURI_INTERNALS__",
            ] {
                // runtime.ts is the sanctioned runtime-detection helper.
                if path.ends_with("runtime.ts") || path.ends_with("TauriEvents.test.ts") {
                    continue;
                }
                if content.contains(pat) {
                    offenders.push(format!("{}: {pat}", path.display()));
                }
            }
        }
        assert!(
            offenders.is_empty(),
            "frontend layer violations: {offenders:?}"
        );
    }

    /// Collects files with any of the given extensions under a directory.
    fn collect_rs_with_ext(dir: &Path, exts: &[&str]) -> Vec<(PathBuf, String)> {
        let mut out = Vec::new();
        let Ok(entries) = std::fs::read_dir(dir) else {
            return out;
        };
        for entry in entries.flatten() {
            let path = entry.path();
            if path.is_dir() {
                out.extend(collect_rs_with_ext(&path, exts));
            } else if exts.contains(&path.extension().and_then(|e| e.to_str()).unwrap_or("")) {
                if let Ok(content) = std::fs::read_to_string(&path) {
                    out.push((path, content));
                }
            }
        }
        out
    }
}
