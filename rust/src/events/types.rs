//! Core event types for the DSL event infrastructure.
//!
//! These types are designed to be:
//! - Cheap to create (< 1μs)
//! - Cheap to clone (for channel sending)
//! - Serializable to JSON for storage
//!
//! The executor creates events synchronously in the hot path,
//! so allocation and copying must be minimal.

use chrono::{DateTime, Utc};
use serde::{Deserialize, Serialize};
use uuid::Uuid;

/// Lightweight DSL execution event.
///
/// Created in the executor hot path, so must be cheap to construct.
/// The drain task handles serialization and storage asynchronously.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct DslEvent {
    /// When the event occurred
    pub timestamp: DateTime<Utc>,

    /// Session context (if available)
    pub session_id: Option<Uuid>,

    /// Event payload (success or failure)
    pub payload: EventPayload,
}

/// Event payload variants.
///
/// Tagged enum for clean JSON serialization.
#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(tag = "type")]
pub enum EventPayload {
    /// DSL command completed successfully
    CommandSucceeded {
        /// Full verb name (e.g., "entity.create-limited-company")
        verb: String,
        /// Execution time in milliseconds
        duration_ms: u64,
    },

    /// DSL command failed
    CommandFailed {
        /// Full verb name
        verb: String,
        /// Execution time in milliseconds
        duration_ms: u64,
        /// Error snapshot (minimal, for hot path)
        error: ErrorSnapshot,
    },

    /// Session started
    SessionStarted {
        /// Where the session originated
        source: SessionSource,
    },

    /// Session ended
    SessionEnded {
        /// Total commands executed
        command_count: u32,
        /// Commands that failed
        error_count: u32,
        /// Session duration in seconds
        duration_secs: u64,
    },
}

impl EventPayload {
    /// Get the event type as a string (for DB storage)
    pub fn event_type_str(&self) -> &'static str {
        match self {
            EventPayload::CommandSucceeded { .. } => "command_succeeded",
            EventPayload::CommandFailed { .. } => "command_failed",
            EventPayload::SessionStarted { .. } => "session_started",
            EventPayload::SessionEnded { .. } => "session_ended",
        }
    }
}

/// Minimal error snapshot for the hot path.
///
/// We capture just enough to identify and categorize the error.
/// Full error details can be reconstructed from logs if needed.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ErrorSnapshot {
    /// Error type name (e.g., "sqlx::Error", "anyhow::Error")
    pub error_type: String,

    /// Error message (truncated to 500 chars to avoid large allocations)
    pub message: String,

    /// Source identifier if available (e.g., LEI, company number)
    pub source_id: Option<String>,

    /// HTTP status code if this was an API error
    pub http_status: Option<u16>,

    /// The verb that failed
    pub verb: String,
}

/// Session source - where did this session originate?
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "lowercase")]
pub enum SessionSource {
    /// Command-line REPL
    Repl,
    /// Web UI (egui)
    Egui,
    /// MCP server
    Mcp,
    /// REST API
    Api,
}

impl std::fmt::Display for SessionSource {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            SessionSource::Repl => write!(f, "repl"),
            SessionSource::Egui => write!(f, "egui"),
            SessionSource::Mcp => write!(f, "mcp"),
            SessionSource::Api => write!(f, "api"),
        }
    }
}

impl DslEvent {
    /// Create a success event.
    ///
    /// This is called in the executor hot path, so it must be fast.
    /// Only allocates for the verb string (which is already owned).
    #[inline]
    pub fn succeeded(session_id: Option<Uuid>, verb: String, duration_ms: u64) -> Self {
        Self {
            timestamp: Utc::now(),
            session_id,
            payload: EventPayload::CommandSucceeded { verb, duration_ms },
        }
    }

    /// Create a failure event.
    ///
    /// Truncates the error message to avoid large allocations in the hot path.
    #[inline]
    pub fn failed(
        session_id: Option<Uuid>,
        verb: String,
        duration_ms: u64,
        error: &anyhow::Error,
    ) -> Self {
        let message = error.to_string();
        let message = if message.len() > 500 {
            format!("{}...", &message[..497])
        } else {
            message
        };

        // Try to extract error type name
        let error_type = error
            .chain()
            .next()
            .map(|e| std::any::type_name_of_val(e).to_string())
            .unwrap_or_else(|| "unknown".to_string());

        Self {
            timestamp: Utc::now(),
            session_id,
            payload: EventPayload::CommandFailed {
                verb: verb.clone(),
                duration_ms,
                error: ErrorSnapshot {
                    error_type,
                    message,
                    source_id: None,
                    http_status: None,
                    verb,
                },
            },
        }
    }

    /// Create a failure event with additional context.
    #[inline]
    pub fn failed_with_context(
        session_id: Option<Uuid>,
        verb: String,
        duration_ms: u64,
        error: &anyhow::Error,
        source_id: Option<String>,
        http_status: Option<u16>,
    ) -> Self {
        let message = error.to_string();
        let message = if message.len() > 500 {
            format!("{}...", &message[..497])
        } else {
            message
        };

        let error_type = error
            .chain()
            .next()
            .map(|e| std::any::type_name_of_val(e).to_string())
            .unwrap_or_else(|| "unknown".to_string());

        Self {
            timestamp: Utc::now(),
            session_id,
            payload: EventPayload::CommandFailed {
                verb: verb.clone(),
                duration_ms,
                error: ErrorSnapshot {
                    error_type,
                    message,
                    source_id,
                    http_status,
                    verb,
                },
            },
        }
    }

    /// Create a session started event.
    #[inline]
    pub fn session_started(session_id: Uuid, source: SessionSource) -> Self {
        Self {
            timestamp: Utc::now(),
            session_id: Some(session_id),
            payload: EventPayload::SessionStarted { source },
        }
    }

    /// Create a session ended event.
    #[inline]
    pub fn session_ended(
        session_id: Uuid,
        command_count: u32,
        error_count: u32,
        duration_secs: u64,
    ) -> Self {
        Self {
            timestamp: Utc::now(),
            session_id: Some(session_id),
            payload: EventPayload::SessionEnded {
                command_count,
                error_count,
                duration_secs,
            },
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_succeeded_event() {
        let event = DslEvent::succeeded(Some(Uuid::new_v4()), "entity.create".to_string(), 100);

        assert!(event.session_id.is_some());
        match event.payload {
            EventPayload::CommandSucceeded { verb, duration_ms } => {
                assert_eq!(verb, "entity.create");
                assert_eq!(duration_ms, 100);
            }
            _ => panic!("Expected CommandSucceeded"),
        }
    }

    #[test]
    fn test_failed_event_truncates_message() {
        let long_message = "x".repeat(1000);
        let error = anyhow::anyhow!("{}", long_message);
        let event = DslEvent::failed(None, "test.verb".to_string(), 50, &error);

        match event.payload {
            EventPayload::CommandFailed { error, .. } => {
                assert!(error.message.len() <= 503); // 500 + "..."
                assert!(error.message.ends_with("..."));
            }
            _ => panic!("Expected CommandFailed"),
        }
    }

    #[test]
    fn test_event_serialization() {
        let event = DslEvent::succeeded(Some(Uuid::new_v4()), "cbu.create".to_string(), 42);
        let json = serde_json::to_string(&event).unwrap();

        assert!(json.contains("\"type\":\"CommandSucceeded\""));
        assert!(json.contains("\"verb\":\"cbu.create\""));
        assert!(json.contains("\"duration_ms\":42"));
    }

    #[test]
    fn test_session_source_display() {
        assert_eq!(SessionSource::Repl.to_string(), "repl");
        assert_eq!(SessionSource::Egui.to_string(), "egui");
        assert_eq!(SessionSource::Mcp.to_string(), "mcp");
        assert_eq!(SessionSource::Api.to_string(), "api");
    }

    #[test]
    fn test_event_type_str() {
        let succeeded = EventPayload::CommandSucceeded {
            verb: "test".to_string(),
            duration_ms: 0,
        };
        assert_eq!(succeeded.event_type_str(), "command_succeeded");

        let failed = EventPayload::CommandFailed {
            verb: "test".to_string(),
            duration_ms: 0,
            error: ErrorSnapshot {
                error_type: "test".to_string(),
                message: "test".to_string(),
                source_id: None,
                http_status: None,
                verb: "test".to_string(),
            },
        };
        assert_eq!(failed.event_type_str(), "command_failed");
    }
}
