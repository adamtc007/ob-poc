//! Task Queue Types
//!
//! Types for the async task return path. External systems push TaskResults
//! to the queue, and a listener drains them to advance workflows.

use chrono::{DateTime, Utc};
use serde::{Deserialize, Serialize};
use uuid::Uuid;

use crate::cargo_ref::CargoRef;

/// Status of a task result
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "lowercase")]
#[cfg_attr(feature = "database", derive(sqlx::Type))]
#[cfg_attr(
    feature = "database",
    sqlx(type_name = "text", rename_all = "lowercase")
)]
pub enum TaskStatus {
    Completed,
    Failed,
    Expired,
}

impl TaskStatus {
    pub fn as_str(&self) -> &'static str {
        match self {
            Self::Completed => "completed",
            Self::Failed => "failed",
            Self::Expired => "expired",
        }
    }
}

impl std::fmt::Display for TaskStatus {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        write!(f, "{}", self.as_str())
    }
}

/// Queue row (from database)
#[derive(Debug, Clone)]
#[cfg_attr(feature = "database", derive(sqlx::FromRow))]
pub struct TaskResultRow {
    pub id: i64,
    pub task_id: Uuid,
    #[cfg_attr(feature = "database", sqlx(try_from = "String"))]
    pub status: TaskStatus,
    pub cargo_type: Option<String>,
    pub cargo_ref: Option<String>,
    pub error: Option<String>,
    pub payload: Option<serde_json::Value>,
    pub queued_at: DateTime<Utc>,
    pub retry_count: i32,
    pub idempotency_key: String,
}

/// Parsed task result (from queue row)
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct TaskResult {
    pub task_id: Uuid,
    pub status: TaskStatus,
    pub cargo_type: Option<String>,
    pub cargo_ref: Option<CargoRef>,
    pub error: Option<String>,
    /// REQUIRED for deduplication (scoped to task_id)
    pub idempotency_key: String,
    /// Raw webhook body for audit
    pub payload: Option<serde_json::Value>,
}

impl From<&TaskResultRow> for TaskResult {
    fn from(row: &TaskResultRow) -> Self {
        Self {
            task_id: row.task_id,
            status: row.status,
            cargo_type: row.cargo_type.clone(),
            cargo_ref: row.cargo_ref.as_ref().and_then(|s| CargoRef::parse(s).ok()),
            error: row.error.clone(),
            idempotency_key: row.idempotency_key.clone(),
            payload: row.payload.clone(),
        }
    }
}

impl TryFrom<String> for TaskStatus {
    type Error = String;

    fn try_from(s: String) -> Result<Self, Self::Error> {
        match s.as_str() {
            "completed" => Ok(Self::Completed),
            "failed" => Ok(Self::Failed),
            "expired" => Ok(Self::Expired),
            _ => Err(format!("Unknown task status: {}", s)),
        }
    }
}

/// Status of a pending task
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "lowercase")]
pub enum PendingTaskStatus {
    Pending,
    Partial,
    Completed,
    Failed,
    Expired,
    Cancelled,
}

impl PendingTaskStatus {
    pub fn as_str(&self) -> &'static str {
        match self {
            Self::Pending => "pending",
            Self::Partial => "partial",
            Self::Completed => "completed",
            Self::Failed => "failed",
            Self::Expired => "expired",
            Self::Cancelled => "cancelled",
        }
    }

    pub fn is_terminal(&self) -> bool {
        matches!(
            self,
            Self::Completed | Self::Failed | Self::Expired | Self::Cancelled
        )
    }
}

impl std::str::FromStr for PendingTaskStatus {
    type Err = String;

    fn from_str(s: &str) -> Result<Self, Self::Err> {
        match s {
            "pending" => Ok(Self::Pending),
            "partial" => Ok(Self::Partial),
            "completed" => Ok(Self::Completed),
            "failed" => Ok(Self::Failed),
            "expired" => Ok(Self::Expired),
            "cancelled" => Ok(Self::Cancelled),
            _ => Err(format!("Unknown pending task status: {}", s)),
        }
    }
}

/// Outbound pending task
#[derive(Debug, Clone, Serialize, Deserialize)]
#[cfg_attr(feature = "database", derive(sqlx::FromRow))]
pub struct PendingTask {
    pub task_id: Uuid,
    pub instance_id: Uuid,
    pub blocker_type: String,
    pub blocker_key: Option<String>,
    pub verb: String,
    pub args: Option<serde_json::Value>,
    pub expected_cargo_count: i32,
    pub received_cargo_count: i32,
    pub failed_count: Option<i32>,
    pub status: String,
    pub created_at: DateTime<Utc>,
    pub expires_at: Option<DateTime<Utc>>,
    pub completed_at: Option<DateTime<Utc>>,
    pub last_error: Option<String>,
}

impl PendingTask {
    /// Check if all expected cargo has been received successfully
    pub fn is_complete(&self) -> bool {
        self.received_cargo_count >= self.expected_cargo_count
    }

    /// Check if task has reached a terminal state (all results in, success or failure)
    pub fn is_terminal(&self) -> bool {
        let total = self.received_cargo_count + self.failed_count.unwrap_or(0);
        total >= self.expected_cargo_count
    }

    /// Get parsed status enum
    pub fn parsed_status(&self) -> Result<PendingTaskStatus, String> {
        self.status.parse()
    }

    /// Check if task is past its expiration time
    pub fn is_expired(&self) -> bool {
        self.expires_at.map(|exp| Utc::now() > exp).unwrap_or(false)
    }
}

/// Single item in a bundle callback
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct BundleItem {
    /// URI: `version://ob-poc/{version_id}`
    pub cargo_ref: String,
    /// Document type: 'passport', 'proof_of_address'
    pub doc_type: String,
    /// Status of this item
    pub status: TaskStatus,
    /// Error message if failed
    #[serde(default)]
    pub error: Option<String>,
}

/// Request payload for task completion webhook
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct TaskCompleteRequest {
    pub task_id: Uuid,
    /// Overall bundle status
    pub status: TaskStatus,
    /// REQUIRED: unique key for deduplication (scoped to task_id)
    pub idempotency_key: String,
    /// Bundle items - always present, even for single-doc returns
    pub items: Vec<BundleItem>,
    /// Overall error if all failed
    #[serde(default)]
    pub error: Option<String>,
}

/// Task event types for audit trail
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum TaskEventType {
    Created,
    ResultReceived,
    Completed,
    Failed,
    Expired,
    Cancelled,
}

impl TaskEventType {
    pub fn as_str(&self) -> &'static str {
        match self {
            Self::Created => "created",
            Self::ResultReceived => "result_received",
            Self::Completed => "completed",
            Self::Failed => "failed",
            Self::Expired => "expired",
            Self::Cancelled => "cancelled",
        }
    }
}

/// Task event for audit trail
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct TaskEvent {
    pub event_id: Uuid,
    pub task_id: Uuid,
    pub event_type: String,
    pub result_status: Option<String>,
    pub cargo_type: Option<String>,
    pub cargo_ref: Option<String>,
    pub error: Option<String>,
    pub payload: Option<serde_json::Value>,
    pub source: Option<String>,
    pub idempotency_key: Option<String>,
    pub occurred_at: DateTime<Utc>,
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_pending_task_completion() {
        let task = PendingTask {
            task_id: Uuid::new_v4(),
            instance_id: Uuid::new_v4(),
            blocker_type: "MissingDocument".to_string(),
            blocker_key: None,
            verb: "document.solicit".to_string(),
            args: None,
            expected_cargo_count: 2,
            received_cargo_count: 1,
            failed_count: Some(0),
            status: "partial".to_string(),
            created_at: Utc::now(),
            expires_at: None,
            completed_at: None,
            last_error: None,
        };

        assert!(!task.is_complete());
        assert!(!task.is_terminal());
    }

    #[test]
    fn test_pending_task_terminal_with_failures() {
        let task = PendingTask {
            task_id: Uuid::new_v4(),
            instance_id: Uuid::new_v4(),
            blocker_type: "MissingDocument".to_string(),
            blocker_key: None,
            verb: "document.solicit".to_string(),
            args: None,
            expected_cargo_count: 2,
            received_cargo_count: 1,
            failed_count: Some(1),
            status: "failed".to_string(),
            created_at: Utc::now(),
            expires_at: None,
            completed_at: None,
            last_error: Some("One document failed".to_string()),
        };

        assert!(!task.is_complete()); // Not all successful
        assert!(task.is_terminal()); // But all results are in
    }

    #[test]
    fn test_task_status_parsing() {
        assert_eq!(
            TaskStatus::try_from("completed".to_string()),
            Ok(TaskStatus::Completed)
        );
        assert_eq!(
            TaskStatus::try_from("failed".to_string()),
            Ok(TaskStatus::Failed)
        );
        assert!(TaskStatus::try_from("unknown".to_string()).is_err());
    }
}
