use crate::domain::value_objects::{EntityId, UnixMs};
use crate::infrastructure::database::pool::AppDatabase;
use serde::Serialize;
use thiserror::Error;

#[derive(Debug, Error)]
pub enum SessionError {
    #[error("database error: {0}")]
    Database(#[from] sqlx::Error),

    #[error("a session is already active")]
    AlreadyActive,

    #[error("no active session found")]
    NoActiveSession,

    #[error("invalid state transition: {from} → {to}")]
    InvalidTransition { from: String, to: String },
}

#[derive(Debug, Clone, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct SessionDto {
    pub id: String,
    pub name: Option<String>,
    pub state: String,
    pub started_at: i64,
    pub item_count: i64,
}

/// CaptureSession service — manages the explicit capture session lifecycle.
/// Privacy: monitoring is OFF by default and after restart.
/// Only one active session at a time.
pub struct SessionService {
    db: AppDatabase,
    device_id: EntityId,
}

impl SessionService {
    pub fn new(db: AppDatabase, device_id: EntityId) -> Self {
        Self { db, device_id }
    }

    /// Starts a new capture session. Fails if one is already active.
    /// `idle_timeout_ms` (CAP-003): if > 0, the session auto-finishes after this
    /// much idle time; None/0 disables auto-end.
    pub async fn start_session(
        &self,
        name: Option<&str>,
        idle_timeout_ms: Option<i64>,
    ) -> Result<SessionDto, SessionError> {
        // Check for existing active/paused session
        let existing: Option<(String,)> = sqlx::query_as(
            "SELECT id FROM capture_sessions WHERE state IN ('active','paused') AND deleted_at IS NULL",
        )
        .fetch_optional(self.db.pool())
        .await?;

        if existing.is_some() {
            return Err(SessionError::AlreadyActive);
        }

        let id = EntityId::new();
        let now = UnixMs::now();

        sqlx::query(
            "INSERT INTO capture_sessions (id, name, state, started_at, idle_timeout_ms, created_at, updated_at, revision,
              origin_device_id, last_modified_device_id, sync_state)
             VALUES (?, ?, 'active', ?, ?, ?, ?, 1, ?, ?, 'local_only')",
        )
        .bind(id.as_str())
        .bind(name)
        .bind(now.as_millis())
        .bind(idle_timeout_ms)
        .bind(now.as_millis())
        .bind(now.as_millis())
        .bind(self.device_id.as_str())
        .bind(self.device_id.as_str())
        .execute(self.db.pool())
        .await?;

        tracing::info!(event = "capture_session_started", session_id = %id);

        Ok(SessionDto {
            id: id.to_string(),
            name: name.map(String::from),
            state: "active".to_string(),
            started_at: now.as_millis(),
            item_count: 0,
        })
    }

    /// CAP-003: finishes active sessions that have been idle longer than their
    /// configured `idle_timeout_ms`. Returns the number of sessions auto-ended.
    pub async fn auto_end_idle_sessions(&self, now_ms: i64) -> Result<i64, SessionError> {
        let result = sqlx::query(
            "UPDATE capture_sessions
             SET state = 'finished', ended_at = ?, updated_at = ?, revision = revision + 1
             WHERE state = 'active' AND deleted_at IS NULL
               AND idle_timeout_ms IS NOT NULL AND idle_timeout_ms > 0
               AND (? - updated_at) > idle_timeout_ms",
        )
        .bind(now_ms)
        .bind(now_ms)
        .bind(now_ms)
        .execute(self.db.pool())
        .await?;

        let n = result.rows_affected() as i64;
        if n > 0 {
            tracing::info!(event = "capture_sessions_idle_ended", count = n);
        }
        Ok(n)
    }

    /// WIN-004: handles system suspend (sleep/hibernate). Privacy-by-default:
    /// every active session is paused immediately and is NEVER auto-resumed on
    /// wake — the user must explicitly resume capture. Returns sessions paused.
    pub async fn handle_system_suspend(&self) -> Result<i64, SessionError> {
        let now = UnixMs::now();
        let result = sqlx::query(
            "UPDATE capture_sessions
             SET state = 'paused', paused_at = ?, updated_at = ?, revision = revision + 1
             WHERE state = 'active' AND deleted_at IS NULL",
        )
        .bind(now.as_millis())
        .bind(now.as_millis())
        .execute(self.db.pool())
        .await?;

        let n = result.rows_affected() as i64;
        if n > 0 {
            tracing::info!(event = "capture_sessions_paused_on_suspend", count = n);
        }
        Ok(n)
    }

    /// Pauses the active session.
    pub async fn pause_session(&self) -> Result<SessionDto, SessionError> {
        let session = self.get_unfinished_session().await?;
        if session.2 != "active" {
            return Err(SessionError::InvalidTransition {
                from: session.2.clone(),
                to: "paused".to_string(),
            });
        }

        let now = UnixMs::now();
        sqlx::query(
            "UPDATE capture_sessions SET state = 'paused', paused_at = ?, updated_at = ?, revision = revision + 1 WHERE id = ?",
        )
        .bind(now.as_millis())
        .bind(now.as_millis())
        .bind(&session.0)
        .execute(self.db.pool())
        .await?;

        tracing::info!(event = "capture_session_paused", session_id = %session.0);
        self.build_dto(&session.0).await
    }

    /// Resumes a paused session.
    pub async fn resume_session(&self) -> Result<SessionDto, SessionError> {
        let session = self.get_unfinished_session().await?;
        if session.2 != "paused" {
            return Err(SessionError::InvalidTransition {
                from: session.2.clone(),
                to: "active".to_string(),
            });
        }

        let now = UnixMs::now();
        sqlx::query(
            "UPDATE capture_sessions SET state = 'active', paused_at = NULL, updated_at = ?, revision = revision + 1 WHERE id = ?",
        )
        .bind(now.as_millis())
        .bind(&session.0)
        .execute(self.db.pool())
        .await?;

        tracing::info!(event = "capture_session_resumed", session_id = %session.0);
        self.build_dto(&session.0).await
    }

    /// Finishes the active/paused session.
    pub async fn finish_session(&self) -> Result<SessionDto, SessionError> {
        let session = self.get_unfinished_session().await?;
        let now = UnixMs::now();

        sqlx::query(
            "UPDATE capture_sessions SET state = 'finished', ended_at = ?, updated_at = ?, revision = revision + 1 WHERE id = ?",
        )
        .bind(now.as_millis())
        .bind(now.as_millis())
        .bind(&session.0)
        .execute(self.db.pool())
        .await?;

        tracing::info!(event = "capture_session_finished", session_id = %session.0);
        self.build_dto(&session.0).await
    }

    /// Gets the current session state (or None if no active session).
    pub async fn get_current_session(&self) -> Result<Option<SessionDto>, SessionError> {
        let row: Option<(String,)> = sqlx::query_as(
            "SELECT id FROM capture_sessions WHERE state IN ('active','paused') AND deleted_at IS NULL",
        )
        .fetch_optional(self.db.pool())
        .await?;

        match row {
            Some((id,)) => Ok(Some(self.build_dto(&id).await?)),
            None => Ok(None),
        }
    }

    async fn get_unfinished_session(
        &self,
    ) -> Result<(String, Option<String>, String), SessionError> {
        let row: Option<(String, Option<String>, String)> = sqlx::query_as(
            "SELECT id, name, state FROM capture_sessions WHERE state IN ('active','paused') AND deleted_at IS NULL",
        )
        .fetch_optional(self.db.pool())
        .await?;

        row.ok_or(SessionError::NoActiveSession)
    }

    async fn build_dto(&self, id: &str) -> Result<SessionDto, SessionError> {
        let row: (Option<String>, String, i64) =
            sqlx::query_as("SELECT name, state, started_at FROM capture_sessions WHERE id = ?")
                .bind(id)
                .fetch_one(self.db.pool())
                .await?;

        let count: (i64,) = sqlx::query_as(
            "SELECT COUNT(*) FROM items WHERE capture_session_id = ? AND deleted_at IS NULL",
        )
        .bind(id)
        .fetch_one(self.db.pool())
        .await?;

        Ok(SessionDto {
            id: id.to_string(),
            name: row.0,
            state: row.1,
            started_at: row.2,
            item_count: count.0,
        })
    }
}
