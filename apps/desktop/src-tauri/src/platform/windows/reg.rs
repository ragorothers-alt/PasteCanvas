//! Thin wrapper around the Windows `reg` CLI.
//! Callers must surface errors — swallowed `reg` output was returning fake success.

use std::process::Command;

pub fn run(args: &[&str]) -> Result<(), String> {
    let output = Command::new("reg")
        .args(args)
        .output()
        .map_err(|e| format!("failed to spawn reg: {e}"))?;
    if output.status.success() {
        return Ok(());
    }
    let stderr = String::from_utf8_lossy(&output.stderr);
    let stdout = String::from_utf8_lossy(&output.stdout);
    let msg = if !stderr.trim().is_empty() {
        stderr.trim()
    } else {
        stdout.trim()
    };
    Err(format!("reg {} failed: {msg}", args.join(" ")))
}

pub fn add_default(key: &str, data: &str) -> Result<(), String> {
    run(&["add", key, "/ve", "/t", "REG_SZ", "/d", data, "/f"])
}

pub fn add_value(key: &str, name: &str, data: &str) -> Result<(), String> {
    run(&["add", key, "/v", name, "/t", "REG_SZ", "/d", data, "/f"])
}

/// Deletes a key. Missing keys are treated as success (unregister is idempotent).
pub fn delete_key(key: &str) -> Result<(), String> {
    match run(&["delete", key, "/f"]) {
        Ok(()) => Ok(()),
        Err(e) if is_not_found(&e) => Ok(()),
        Err(e) => Err(e),
    }
}

/// Deletes a named value. Missing values are treated as success.
pub fn delete_value(key: &str, name: &str) -> Result<(), String> {
    match run(&["delete", key, "/v", name, "/f"]) {
        Ok(()) => Ok(()),
        Err(e) if is_not_found(&e) => Ok(()),
        Err(e) => Err(e),
    }
}

fn is_not_found(err: &str) -> bool {
    let lower = err.to_ascii_lowercase();
    lower.contains("unable to find") || lower.contains("cannot find")
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn not_found_matcher_covers_reg_wording() {
        assert!(is_not_found(
            "ERROR: The system was unable to find the specified registry key or value."
        ));
        assert!(!is_not_found("ERROR: Access is denied."));
    }
}
