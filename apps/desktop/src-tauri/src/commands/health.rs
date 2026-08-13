use serde::Serialize;

/// Health check response for verifying command boundary works.
#[derive(Debug, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct HealthResponse {
    pub status: String,
    pub version: String,
    pub correlation_id: String,
}

/// Health-check command — verifies the Tauri command boundary is operational.
#[tauri::command]
pub async fn health_check() -> Result<HealthResponse, String> {
    let correlation_id = uuid::Uuid::now_v7().to_string();

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
