//! Session logger for conversation context capture.
//!
//! The session logger captures the full conversation context for a session:
//! - User input
//! - Agent thoughts (in agent mode)
//! - DSL commands
//! - Responses
//! - Errors
//!
//! This enables session replay and context-aware failure analysis.
//! Unlike the event emitter, the session logger uses async database writes
//! since it's called from the REPL/UI, not the executor hot path.

use anyhow::Result;
use chrono::{DateTime, Utc};
use serde::{Deserialize, Serialize};
use serde_json::Value as JsonValue;
use uuid::Uuid;

#[cfg(feature = "database")]
use sqlx::PgPool;

/// Entry type for session log entries.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum EntryType {
    /// User input (typed command or natural language)
    UserInput,
    /// Agent thought process (in agent mode)
    AgentThought,
    /// DSL command being executed
    DslCommand,
    /// Response from command execution
    Response,
    /// Error from command execution
    Error,
}

impl EntryType {
    /// Convert to database string representation.
    pub fn as_str(&self) -> &'static str {
        match self {
            EntryType::UserInput => "user_input",
            EntryType::AgentThought => "agent_thought",
            EntryType::DslCommand => "dsl_command",
            EntryType::Response => "response",
            EntryType::Error => "error",
        }
    }
}

impl std::fmt::Display for EntryType {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        write!(f, "{}", self.as_str())
    }
}

/// A session log entry.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct SessionLogEntry {
    pub id: i64,
    pub session_id: Uuid,
    pub timestamp: DateTime<Utc>,
    pub entry_type: String,
    pub content: String,
    pub event_id: Option<i64>,
    pub source: String,
    pub metadata: JsonValue,
}

/// Session logger for capturing conversation context.
///
/// This is used by the REPL, UI, and API to log the conversation flow.
/// It's NOT used in the executor hot path (that uses EventEmitter).
#[cfg(feature = "database")]
pub struct SessionLogger {
    pool: PgPool,
    session_id: Uuid,
    source: String,
}

#[cfg(feature = "database")]
impl SessionLogger {
    /// Create a new session logger.
    pub fn new(pool: PgPool, session_id: Uuid, source: &str) -> Self {
        Self {
            pool,
            session_id,
            source: source.to_string(),
        }
    }

    /// Log user input.
    pub async fn log_user_input(&self, content: &str) -> Result<i64> {
        self.log_entry(EntryType::UserInput, content, None, None)
            .await
    }

    /// Log user input with metadata.
    pub async fn log_user_input_with_metadata(
        &self,
        content: &str,
        metadata: JsonValue,
    ) -> Result<i64> {
        self.log_entry(EntryType::UserInput, content, None, Some(metadata))
            .await
    }

    /// Log agent thought (for agent mode).
    pub async fn log_agent_thought(&self, content: &str) -> Result<i64> {
        self.log_entry(EntryType::AgentThought, content, None, None)
            .await
    }

    /// Log DSL command (links to event).
    pub async fn log_dsl_command(&self, content: &str, event_id: Option<i64>) -> Result<i64> {
        self.log_entry(EntryType::DslCommand, content, event_id, None)
            .await
    }

    /// Log response.
    pub async fn log_response(&self, content: &str) -> Result<i64> {
        self.log_entry(EntryType::Response, content, None, None)
            .await
    }

    /// Log error (links to event).
    pub async fn log_error(&self, content: &str, event_id: Option<i64>) -> Result<i64> {
        self.log_entry(EntryType::Error, content, event_id, None)
            .await
    }

    /// Log error with metadata.
    pub async fn log_error_with_metadata(
        &self,
        content: &str,
        event_id: Option<i64>,
        metadata: JsonValue,
    ) -> Result<i64> {
        self.log_entry(EntryType::Error, content, event_id, Some(metadata))
            .await
    }

    /// Generic log entry method.
    async fn log_entry(
        &self,
        entry_type: EntryType,
        content: &str,
        event_id: Option<i64>,
        metadata: Option<JsonValue>,
    ) -> Result<i64> {
        let metadata = metadata.unwrap_or(serde_json::json!({}));

        let id = sqlx::query_scalar!(
            r#"
            INSERT INTO sessions.log (session_id, entry_type, content, event_id, source, metadata)
            VALUES ($1, $2, $3, $4, $5, $6)
            RETURNING id
            "#,
            self.session_id,
            entry_type.as_str(),
            content,
            event_id,
            self.source,
            metadata,
        )
        .fetch_one(&self.pool)
        .await?;

        Ok(id)
    }

    /// Get all entries for this session.
    pub async fn get_entries(&self) -> Result<Vec<SessionLogEntry>> {
        let entries = sqlx::query_as!(
            SessionLogEntry,
            r#"
            SELECT id, session_id, timestamp, entry_type, content, event_id, source, metadata
            FROM sessions.log
            WHERE session_id = $1
            ORDER BY timestamp ASC
            "#,
            self.session_id,
        )
        .fetch_all(&self.pool)
        .await?;

        Ok(entries)
    }

    /// Get entries of a specific type.
    pub async fn get_entries_by_type(&self, entry_type: EntryType) -> Result<Vec<SessionLogEntry>> {
        let entries = sqlx::query_as!(
            SessionLogEntry,
            r#"
            SELECT id, session_id, timestamp, entry_type, content, event_id, source, metadata
            FROM sessions.log
            WHERE session_id = $1 AND entry_type = $2
            ORDER BY timestamp ASC
            "#,
            self.session_id,
            entry_type.as_str(),
        )
        .fetch_all(&self.pool)
        .await?;

        Ok(entries)
    }

    /// Get the session ID.
    pub fn session_id(&self) -> Uuid {
        self.session_id
    }

    /// Get the source.
    pub fn source(&self) -> &str {
        &self.source
    }
}

/// Query session logs across sessions (for analysis).
#[cfg(feature = "database")]
pub struct SessionLogQuery {
    pool: PgPool,
}

#[cfg(feature = "database")]
impl SessionLogQuery {
    /// Create a new query helper.
    pub fn new(pool: PgPool) -> Self {
        Self { pool }
    }

    /// Get all entries for a session.
    pub async fn get_session(&self, session_id: Uuid) -> Result<Vec<SessionLogEntry>> {
        let entries = sqlx::query_as!(
            SessionLogEntry,
            r#"
            SELECT id, session_id, timestamp, entry_type, content, event_id, source, metadata
            FROM sessions.log
            WHERE session_id = $1
            ORDER BY timestamp ASC
            "#,
            session_id,
        )
        .fetch_all(&self.pool)
        .await?;

        Ok(entries)
    }

    /// Get entries linked to a specific event.
    pub async fn get_by_event(&self, event_id: i64) -> Result<Vec<SessionLogEntry>> {
        let entries = sqlx::query_as!(
            SessionLogEntry,
            r#"
            SELECT id, session_id, timestamp, entry_type, content, event_id, source, metadata
            FROM sessions.log
            WHERE event_id = $1
            ORDER BY timestamp ASC
            "#,
            event_id,
        )
        .fetch_all(&self.pool)
        .await?;

        Ok(entries)
    }

    /// Get recent errors across all sessions.
    pub async fn get_recent_errors(&self, limit: i64) -> Result<Vec<SessionLogEntry>> {
        let entries = sqlx::query_as!(
            SessionLogEntry,
            r#"
            SELECT id, session_id, timestamp, entry_type, content, event_id, source, metadata
            FROM sessions.log
            WHERE entry_type = 'error'
            ORDER BY timestamp DESC
            LIMIT $1
            "#,
            limit,
        )
        .fetch_all(&self.pool)
        .await?;

        Ok(entries)
    }

    /// Get context around an error (entries before and after).
    pub async fn get_error_context(
        &self,
        event_id: i64,
        context_entries: i32,
    ) -> Result<Vec<SessionLogEntry>> {
        // First, find the session and timestamp of the error
        let error_entry = sqlx::query_as!(
            SessionLogEntry,
            r#"
            SELECT id, session_id, timestamp, entry_type, content, event_id, source, metadata
            FROM sessions.log
            WHERE event_id = $1
            LIMIT 1
            "#,
            event_id,
        )
        .fetch_optional(&self.pool)
        .await?;

        let Some(error) = error_entry else {
            return Ok(vec![]);
        };

        // Get entries around this timestamp in the same session
        let entries = sqlx::query_as!(
            SessionLogEntry,
            r#"
            SELECT id, session_id, timestamp, entry_type, content, event_id, source, metadata
            FROM sessions.log
            WHERE session_id = $1
            ORDER BY timestamp ASC
            "#,
            error.session_id,
        )
        .fetch_all(&self.pool)
        .await?;

        // Find the error position and extract context
        let error_pos = entries.iter().position(|e| e.id == error.id).unwrap_or(0);
        let start = error_pos.saturating_sub(context_entries as usize);
        let end = (error_pos + context_entries as usize + 1).min(entries.len());

        Ok(entries[start..end].to_vec())
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_entry_type_as_str() {
        assert_eq!(EntryType::UserInput.as_str(), "user_input");
        assert_eq!(EntryType::AgentThought.as_str(), "agent_thought");
        assert_eq!(EntryType::DslCommand.as_str(), "dsl_command");
        assert_eq!(EntryType::Response.as_str(), "response");
        assert_eq!(EntryType::Error.as_str(), "error");
    }

    #[test]
    fn test_entry_type_display() {
        assert_eq!(format!("{}", EntryType::UserInput), "user_input");
        assert_eq!(format!("{}", EntryType::Error), "error");
    }
}
