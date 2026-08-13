//! Shared serializable contract types for PasteCanvas.
//! These DTOs are used across the command boundary between frontend and backend.

use serde::{Deserialize, Serialize};

/// Stable command error shape returned from all Tauri commands.
#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct CommandErrorDto {
    pub code: CommandErrorCode,
    pub message: String,
    pub retryable: bool,
    pub correlation_id: String,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub details: Option<serde_json::Value>,
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq)]
#[serde(rename_all = "SCREAMING_SNAKE_CASE")]
pub enum CommandErrorCode {
    ValidationError,
    ClipboardBusy,
    UnsupportedContent,
    ContentTooLarge,
    AssetWriteFailed,
    DatabaseError,
    RevisionConflict,
    HotkeyConflict,
    PermissionDenied,
    NotFound,
    IntegrityError,
    InternalError,
}

impl CommandErrorDto {
    pub fn new(code: CommandErrorCode, message: impl Into<String>, correlation_id: String) -> Self {
        let retryable = matches!(
            code,
            CommandErrorCode::ClipboardBusy | CommandErrorCode::DatabaseError
        );
        Self {
            code,
            message: message.into(),
            retryable,
            correlation_id,
            details: None,
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn command_error_serializes_to_camel_case() {
        let err = CommandErrorDto::new(
            CommandErrorCode::ValidationError,
            "field too long",
            "test-id".to_string(),
        );
        let json = serde_json::to_value(&err).expect("serialize");
        assert_eq!(json["code"], "VALIDATION_ERROR");
        assert_eq!(json["correlationId"], "test-id");
        assert_eq!(json["retryable"], false);
    }

    #[test]
    fn clipboard_busy_is_retryable() {
        let err = CommandErrorDto::new(
            CommandErrorCode::ClipboardBusy,
            "clipboard locked",
            "id-1".to_string(),
        );
        assert!(err.retryable);
    }
}
