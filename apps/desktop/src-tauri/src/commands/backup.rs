use std::path::PathBuf;

use tauri::State;

use crate::application::backup_service::{
    BackupResultDto, BackupService, ExportResultDto, RestoreValidationDto,
};
use crate::bootstrap::app_state::AppState;

#[tauri::command]
pub async fn create_backup(state: State<'_, AppState>) -> Result<BackupResultDto, String> {
    let data_dir = state.data_dir.clone();
    let svc = BackupService::new(state.db.clone(), data_dir);
    svc.create_backup().await.map_err(|e| e.to_string())
}

#[tauri::command]
pub async fn create_export(state: State<'_, AppState>) -> Result<ExportResultDto, String> {
    let data_dir = state.data_dir.clone();
    let svc = BackupService::new(state.db.clone(), data_dir);
    svc.create_export().await.map_err(|e| e.to_string())
}

/// DAT-002: validates a backup (schema version + files) without restoring.
#[tauri::command]
pub async fn validate_backup(
    state: State<'_, AppState>,
    backup_path: String,
) -> Result<RestoreValidationDto, String> {
    let svc = BackupService::new(state.db.clone(), state.data_dir.clone());
    Ok(svc.validate_backup(&PathBuf::from(backup_path)))
}

/// DAT-002: stages a restore to be applied on next startup.
#[tauri::command]
pub async fn prepare_restore(
    state: State<'_, AppState>,
    backup_path: String,
) -> Result<RestoreValidationDto, String> {
    let svc = BackupService::new(state.db.clone(), state.data_dir.clone());
    svc.prepare_restore(&PathBuf::from(backup_path))
        .map_err(|e| e.to_string())
}
