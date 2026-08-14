//! Job runner — processes durable background jobs from the jobs table.
//! Runs as a background task, claims jobs with lease, processes them,
//! and updates results. Failed enrichment never invalidates the original capture.

use crate::domain::value_objects::{EntityId, UnixMs};
use crate::infrastructure::database::pool::AppDatabase;
use crate::jobs::link_metadata;
use std::sync::Arc;
use std::sync::atomic::{AtomicBool, Ordering};
use tauri::Emitter;

/// WIN-004: minimum wall-clock gap between idle ticks that indicates the
/// system was suspended. The poll interval is 5s; 60s leaves ample headroom
/// for scheduler jitter while still catching real sleep/hibernate.
pub const SUSPEND_GAP_THRESHOLD_MS: i64 = 60_000;

/// WIN-004: deterministic suspend detection. The async scheduler does not run
/// while the machine sleeps, so a wall-clock gap between ticks larger than the
/// threshold proves the system was suspended/hibernated since the last tick.
pub fn suspend_gap_detected(last_tick_ms: i64, now_ms: i64) -> bool {
    now_ms.saturating_sub(last_tick_ms) > SUSPEND_GAP_THRESHOLD_MS
}

/// Spawns the background job processor.
/// Returns a shutdown flag that can be set to stop processing.
pub fn spawn_job_runner(db: AppDatabase, app: tauri::AppHandle) -> Arc<AtomicBool> {
    let shutdown = Arc::new(AtomicBool::new(false));
    let shutdown_clone = shutdown.clone();

    tauri::async_runtime::spawn(async move {
        tracing::info!(event = "job_runner_started");
        // WIN-004: wall-clock anchor for suspend detection
        let mut last_idle_tick_ms = UnixMs::now().as_millis();

        loop {
            if shutdown_clone.load(Ordering::Relaxed) {
                tracing::info!(event = "job_runner_stopping");
                break;
            }

            match process_next_job(&db).await {
                Ok(true) => {
                    // Job processed, immediately check for more
                    continue;
                }
                Ok(false) => {
                    let now = UnixMs::now();
                    let session_svc = crate::application::session_service::SessionService::new(
                        db.clone(),
                        crate::domain::value_objects::EntityId::new(),
                    );

                    // WIN-004: detect system suspend via wall-clock gap, then
                    // pause capture sessions (privacy-by-default: never auto-resume).
                    if suspend_gap_detected(last_idle_tick_ms, now.as_millis()) {
                        tracing::info!(
                            event = "system_suspend_detected",
                            gap_ms = now.as_millis() - last_idle_tick_ms
                        );
                        match session_svc.handle_system_suspend().await {
                            Ok(paused) if paused > 0 => {
                                let _ = app.emit("paste-canvas://session-suspended", paused);
                            }
                            Ok(_) => {}
                            Err(e) => {
                                tracing::warn!(event = "suspend_pause_failed", error = %e);
                            }
                        }
                    }
                    last_idle_tick_ms = now.as_millis();

                    // CAP-003: auto-end idle capture sessions on each idle tick
                    if let Err(e) = session_svc.auto_end_idle_sessions(now.as_millis()).await {
                        tracing::warn!(event = "idle_session_check_failed", error = %e);
                    }
                    // No jobs available, wait before polling again
                    tokio::time::sleep(std::time::Duration::from_secs(5)).await;
                }
                Err(e) => {
                    tracing::warn!(event = "job_runner_error", error = %e);
                    tokio::time::sleep(std::time::Duration::from_secs(10)).await;
                }
            }
        }
    });

    shutdown
}

/// Queues a fetch_link_metadata job for a captured URL item.
pub async fn queue_link_metadata_job(
    db: &AppDatabase,
    item_id: &str,
    url: &str,
) -> Result<(), sqlx::Error> {
    let job_id = EntityId::new();
    let now = UnixMs::now();
    let payload = serde_json::json!({
        "itemId": item_id,
        "url": url,
    });

    sqlx::query(
        "INSERT INTO jobs (id, job_type, payload_json, state, priority, available_at, attempts, max_attempts, created_at, updated_at)
         VALUES (?, 'fetch_link_metadata', ?, 'queued', 0, ?, 0, 3, ?, ?)",
    )
    .bind(job_id.as_str())
    .bind(payload.to_string())
    .bind(now.as_millis())
    .bind(now.as_millis())
    .bind(now.as_millis())
    .execute(db.pool())
    .await?;

    tracing::debug!(
        event = "job_queued",
        job_type = "fetch_link_metadata",
        item_id = item_id
    );
    Ok(())
}

/// Attempts to claim and process the next available job.
/// Returns true if a job was processed, false if none available.
async fn process_next_job(db: &AppDatabase) -> Result<bool, sqlx::Error> {
    let now = UnixMs::now().as_millis();
    let lease_until = now + 30_000; // 30 second lease

    // Claim next available job
    let job: Option<(String, String, String, i32)> = sqlx::query_as(
        "SELECT id, job_type, payload_json, attempts FROM jobs
         WHERE state = 'queued' AND available_at <= ?
         ORDER BY priority DESC, created_at ASC
         LIMIT 1",
    )
    .bind(now)
    .fetch_optional(db.pool())
    .await?;

    let (job_id, job_type, payload_json, attempts) = match job {
        Some(j) => j,
        None => return Ok(false),
    };

    // Mark as running with lease
    sqlx::query(
        "UPDATE jobs SET state = 'running', lease_until = ?, attempts = ?, updated_at = ? WHERE id = ?",
    )
    .bind(lease_until)
    .bind(attempts + 1)
    .bind(now)
    .bind(&job_id)
    .execute(db.pool())
    .await?;

    // Process based on type
    let result = match job_type.as_str() {
        "fetch_link_metadata" => process_link_metadata(db, &payload_json).await,
        other => {
            tracing::warn!(event = "unknown_job_type", job_type = other);
            Ok(())
        }
    };

    let finish_now = UnixMs::now().as_millis();
    match result {
        Ok(()) => {
            sqlx::query("UPDATE jobs SET state = 'succeeded', updated_at = ? WHERE id = ?")
                .bind(finish_now)
                .bind(&job_id)
                .execute(db.pool())
                .await?;
        }
        Err(e) => {
            let max_attempts = 3;
            if attempts + 1 >= max_attempts {
                sqlx::query(
                    "UPDATE jobs SET state = 'failed', last_error_code = ?, updated_at = ? WHERE id = ?",
                )
                .bind(e.to_string())
                .bind(finish_now)
                .bind(&job_id)
                .execute(db.pool())
                .await?;
                tracing::warn!(event = "job_failed_permanently", job_id = %job_id, error = %e);
            } else {
                // Exponential backoff: 5s, 25s, 125s
                let backoff_ms = 5000i64 * 5i64.pow(attempts as u32);
                let available_at = finish_now + backoff_ms;
                sqlx::query(
                    "UPDATE jobs SET state = 'queued', available_at = ?, last_error_code = ?, updated_at = ? WHERE id = ?",
                )
                .bind(available_at)
                .bind(e.to_string())
                .bind(finish_now)
                .bind(&job_id)
                .execute(db.pool())
                .await?;
            }
        }
    }

    Ok(true)
}

/// Processes a fetch_link_metadata job.
async fn process_link_metadata(db: &AppDatabase, payload_json: &str) -> Result<(), String> {
    let payload: serde_json::Value =
        serde_json::from_str(payload_json).map_err(|e| e.to_string())?;

    let item_id = payload["itemId"].as_str().ok_or("missing itemId")?;
    let url = payload["url"].as_str().ok_or("missing url")?;

    // Fetch metadata with SSRF protection
    let metadata = link_metadata::fetch_link_metadata(url)
        .await
        .map_err(|e| e.to_string())?;

    // Update item title if we got one and item still has no title
    if let Some(ref title) = metadata.title {
        let now = UnixMs::now().as_millis();
        sqlx::query(
            "UPDATE items SET title = ?, updated_at = ?, revision = revision + 1
             WHERE id = ? AND (title IS NULL OR title = '') AND deleted_at IS NULL",
        )
        .bind(title)
        .bind(now)
        .bind(item_id)
        .execute(db.pool())
        .await
        .map_err(|e| e.to_string())?;

        // Update search document
        sqlx::query("UPDATE search_documents SET title = ?, updated_at = ? WHERE item_id = ?")
            .bind(title)
            .bind(now)
            .bind(item_id)
            .execute(db.pool())
            .await
            .map_err(|e| e.to_string())?;
    }

    tracing::info!(
        event = "link_metadata_fetched",
        item_id = item_id,
        has_title = metadata.title.is_some(),
    );

    Ok(())
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn normal_tick_gap_is_not_suspend() {
        let last = 1_000_000;
        assert!(!suspend_gap_detected(last, last + 5_000)); // one poll cycle
        assert!(!suspend_gap_detected(last, last + 30_000)); // jittery but fine
        assert!(!suspend_gap_detected(last, last + SUSPEND_GAP_THRESHOLD_MS)); // boundary
    }

    #[test]
    fn large_wall_clock_gap_is_suspend() {
        let last = 1_000_000;
        assert!(suspend_gap_detected(
            last,
            last + SUSPEND_GAP_THRESHOLD_MS + 1
        ));
        assert!(suspend_gap_detected(last, last + 3_600_000)); // slept 1 hour
    }

    #[test]
    fn clock_skew_does_not_trigger_false_positive() {
        // saturating_sub keeps a backwards clock from firing the detector
        assert!(!suspend_gap_detected(2_000_000, 1_000_000));
    }
}
