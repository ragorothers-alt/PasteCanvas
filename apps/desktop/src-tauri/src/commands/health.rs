use serde::Serialize;
use tauri::State;

use crate::bootstrap::app_state::AppState;

/// Health check response for verifying command boundary works.
#[derive(Debug, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct HealthResponse {
    pub status: String,
    pub version: String,
    pub correlation_id: String,
}

/// Health-check command — pings the database so a broken store cannot look healthy.
#[tauri::command]
pub async fn health_check(state: State<'_, AppState>) -> Result<HealthResponse, String> {
    let correlation_id = uuid::Uuid::now_v7().to_string();

    sqlx::query_scalar::<_, i64>("SELECT 1")
        .fetch_one(state.db.pool())
        .await
        .map_err(|e| format!("database unavailable: {e}"))?;

    tracing::debug!(
        event = "health_check",
        correlation_id = %correlation_id,
    );

    Ok(HealthResponse {
        status: "ok".to_string(),
        version: env!("CARGO_PKG_VERSION").to_string(),
        correlation_id,
    })
}
