//! Settings service — manages application settings, capture exclusions, and hotkey bindings.
//!
//! Privacy-by-default:
//! - Automatic capture is OFF by default
//! - Exclusion list blocks password managers and sensitive apps
//! - Built-in exclusion templates for common password managers
//! - User can edit exclusions

use crate::domain::value_objects::UnixMs;
use crate::infrastructure::database::pool::AppDatabase;
use serde::{Deserialize, Serialize};
use thiserror::Error;

#[derive(Debug, Error)]
pub enum SettingsError {
    #[error("database error: {0}")]
    Database(#[from] sqlx::Error),

    #[error("invalid setting value: {0}")]
    InvalidValue(String),
}

#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct ExclusionDto {
    pub id: String,
    pub match_type: String,
    pub match_value: String,
    pub enabled: bool,
    pub source: String,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct HotkeyBindingDto {
    pub action: String,
    pub accelerator: String,
    pub enabled: bool,
}

/// Built-in password manager exclusion templates.
const BUILT_IN_EXCLUSIONS: &[(&str, &str)] = &[
    ("process_name", "1Password.exe"),
    ("process_name", "1Password7.exe"),
    ("process_name", "Bitwarden.exe"),
    ("process_name", "KeePass.exe"),
    ("process_name", "KeePassXC.exe"),
    ("process_name", "LastPass.exe"),
    ("process_name", "Dashlane.exe"),
    ("process_name", "mSecure.exe"),
    ("process_name", "PasswordSafe.exe"),
    ("process_name", "RoboForm.exe"),
    ("process_name", "Enpass.exe"),
    ("process_name", "NordPass.exe"),
    ("process_name", "ProtonPass.exe"),
];

pub struct SettingsService {
    db: AppDatabase,
}

impl SettingsService {
    pub fn new(db: AppDatabase) -> Self {
        Self { db }
    }

    /// Initializes default settings and built-in exclusions if not present.
    pub async fn initialize_defaults(&self) -> Result<(), SettingsError> {
        let now = UnixMs::now().as_millis();

        // Insert built-in exclusions (skip if already exist)
        for (match_type, match_value) in BUILT_IN_EXCLUSIONS {
            sqlx::query(
                "INSERT OR IGNORE INTO capture_exclusions (id, match_type, match_value, enabled, source, created_at, updated_at)
                 VALUES (?, ?, ?, 1, 'built_in', ?, ?)",
            )
            .bind(uuid::Uuid::now_v7().to_string())
            .bind(match_type)
            .bind(match_value)
            .bind(now)
            .bind(now)
            .execute(self.db.pool())
            .await?;
        }

        // Insert default hotkey bindings
        let defaults = [
            ("capture_clipboard", "CommandOrControl+Shift+V"),
            ("quick_capture", "CommandOrControl+Shift+C"),
            ("screenshot", "CommandOrControl+Shift+S"),
            ("toggle_main_window", "CommandOrControl+Shift+P"),
            ("global_search", "CommandOrControl+Shift+F"),
        ];

        for (action, accelerator) in defaults {
            sqlx::query(
                "INSERT OR IGNORE INTO hotkey_bindings (action, accelerator, enabled, updated_at)
                 VALUES (?, ?, 1, ?)",
            )
            .bind(action)
            .bind(accelerator)
            .bind(now)
            .execute(self.db.pool())
            .await?;
        }

        // Default settings
        let setting_defaults = [
            ("capture.automatic_enabled", "false"),
            ("capture.max_text_size", "1000000"),
            ("ui.theme", "\"dark\""),
            ("ui.close_to_tray", "true"),
            ("privacy.store_window_titles", "true"),
            ("privacy.store_file_paths", "false"),
        ];

        for (key, value) in setting_defaults {
            sqlx::query(
                "INSERT OR IGNORE INTO settings (key, value_json, schema_version, updated_at)
                 VALUES (?, ?, 1, ?)",
            )
            .bind(key)
            .bind(value)
            .bind(now)
            .execute(self.db.pool())
            .await?;
        }

        Ok(())
    }

    /// Gets a setting value by key.
    pub async fn get_setting(&self, key: &str) -> Result<Option<String>, SettingsError> {
        let row: Option<(String,)> =
            sqlx::query_as("SELECT value_json FROM settings WHERE key = ?")
                .bind(key)
                .fetch_optional(self.db.pool())
                .await?;
        Ok(row.map(|(v,)| v))
    }

    /// Sets a setting value.
    pub async fn set_setting(&self, key: &str, value_json: &str) -> Result<(), SettingsError> {
        let now = UnixMs::now().as_millis();
        sqlx::query(
            "INSERT INTO settings (key, value_json, schema_version, updated_at)
             VALUES (?, ?, 1, ?)
             ON CONFLICT(key) DO UPDATE SET value_json = ?, updated_at = ?",
        )
        .bind(key)
        .bind(value_json)
        .bind(now)
        .bind(value_json)
        .bind(now)
        .execute(self.db.pool())
        .await?;
        Ok(())
    }

    /// Lists all capture exclusions.
    pub async fn list_exclusions(&self) -> Result<Vec<ExclusionDto>, SettingsError> {
        let rows = sqlx::query_as::<_, (String, String, String, i32, String)>(
            "SELECT id, match_type, match_value, enabled, source FROM capture_exclusions ORDER BY source, match_value",
        )
        .fetch_all(self.db.pool())
        .await?;

        Ok(rows
            .into_iter()
            .map(
                |(id, match_type, match_value, enabled, source)| ExclusionDto {
                    id,
                    match_type,
                    match_value,
                    enabled: enabled == 1,
                    source,
                },
            )
            .collect())
    }

    /// Adds a user exclusion.
    pub async fn add_exclusion(
        &self,
        match_type: &str,
        match_value: &str,
    ) -> Result<ExclusionDto, SettingsError> {
        let id = uuid::Uuid::now_v7().to_string();
        let now = UnixMs::now().as_millis();

        sqlx::query(
            "INSERT INTO capture_exclusions (id, match_type, match_value, enabled, source, created_at, updated_at)
             VALUES (?, ?, ?, 1, 'user', ?, ?)
             ON CONFLICT(match_type, match_value) DO UPDATE SET enabled = 1, updated_at = ?",
        )
        .bind(&id)
        .bind(match_type)
        .bind(match_value)
        .bind(now)
        .bind(now)
        .bind(now)
        .execute(self.db.pool())
        .await?;

        Ok(ExclusionDto {
            id,
            match_type: match_type.to_string(),
            match_value: match_value.to_string(),
            enabled: true,
            source: "user".to_string(),
        })
    }

    /// Toggles an exclusion's enabled state.
    pub async fn toggle_exclusion(&self, id: &str) -> Result<bool, SettingsError> {
        let now = UnixMs::now().as_millis();
        sqlx::query(
            "UPDATE capture_exclusions SET enabled = CASE WHEN enabled = 0 THEN 1 ELSE 0 END, updated_at = ? WHERE id = ?",
        )
        .bind(now)
        .bind(id)
        .execute(self.db.pool())
        .await?;

        let row: Option<(i32,)> =
            sqlx::query_as("SELECT enabled FROM capture_exclusions WHERE id = ?")
                .bind(id)
                .fetch_optional(self.db.pool())
                .await?;

        Ok(row.map(|(v,)| v == 1).unwrap_or(false))
    }

    /// Removes a user exclusion (built-in cannot be removed, only disabled).
    pub async fn remove_exclusion(&self, id: &str) -> Result<(), SettingsError> {
        sqlx::query("DELETE FROM capture_exclusions WHERE id = ? AND source = 'user'")
            .bind(id)
            .execute(self.db.pool())
            .await?;
        Ok(())
    }

    /// Checks if a process name is excluded from capture.
    pub async fn is_process_excluded(&self, process_name: &str) -> Result<bool, SettingsError> {
        let row: Option<(i32,)> = sqlx::query_as(
            "SELECT 1 FROM capture_exclusions WHERE match_type = 'process_name' AND match_value = ? AND enabled = 1",
        )
        .bind(process_name)
        .fetch_optional(self.db.pool())
        .await?;

        Ok(row.is_some())
    }

    /// Lists hotkey bindings.
    pub async fn list_hotkeys(&self) -> Result<Vec<HotkeyBindingDto>, SettingsError> {
        let rows = sqlx::query_as::<_, (String, String, i32)>(
            "SELECT action, accelerator, enabled FROM hotkey_bindings ORDER BY action",
        )
        .fetch_all(self.db.pool())
        .await?;

        Ok(rows
            .into_iter()
            .map(|(action, accelerator, enabled)| HotkeyBindingDto {
                action,
                accelerator,
                enabled: enabled == 1,
            })
            .collect())
    }

    /// Updates a hotkey binding.
    pub async fn update_hotkey(
        &self,
        action: &str,
        accelerator: &str,
    ) -> Result<(), SettingsError> {
        let now = UnixMs::now().as_millis();
        let result = sqlx::query(
            "UPDATE hotkey_bindings SET accelerator = ?, updated_at = ? WHERE action = ?",
        )
        .bind(accelerator)
        .bind(now)
        .bind(action)
        .execute(self.db.pool())
        .await?;

        if result.rows_affected() == 0 {
            return Err(SettingsError::InvalidValue(format!(
                "unknown action: {action}"
            )));
        }
        Ok(())
    }
}
