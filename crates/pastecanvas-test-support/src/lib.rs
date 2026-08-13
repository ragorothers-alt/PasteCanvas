//! Test support utilities for PasteCanvas.
//! Provides shared test fixtures, temp database helpers, and assertion utilities.

/// Creates a temporary directory path for test databases.
pub fn temp_db_path() -> std::path::PathBuf {
    let dir = std::env::temp_dir().join("pastecanvas-test");
    std::fs::create_dir_all(&dir).expect("create temp test dir");
    dir.join(format!("test-{}.sqlite3", uuid::Uuid::now_v7()))
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn temp_db_path_creates_unique_paths() {
        let p1 = temp_db_path();
        let p2 = temp_db_path();
        assert_ne!(p1, p2);
    }
}
