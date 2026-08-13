use crate::domain::value_objects::EntityId;
use crate::infrastructure::database::pool::AppDatabase;
use crate::platform::clipboard::listener::ClipboardListenerState;
use std::path::PathBuf;
use std::sync::Mutex;

/// Shared application state managed by Tauri.
/// Holds the database connection, device identity, data directory, clipboard
/// listener, and startup diagnostics.
pub struct AppState {
    pub db: AppDatabase,
    pub device_id: EntityId,
    pub data_dir: PathBuf,
    pub clipboard_listener: ClipboardListenerState,
    /// FND-006: hotkey actions that failed to register at startup.
    /// Surfaced to the UI once (drained on read) so failures are visible
    /// and recoverable via Settings.
    pub hotkey_failures: Mutex<Vec<String>>,
}
