//! Core types for the bpmn-lite journey runtime.
//!
//! These are the identity and state types used across all runtime modules.
//! The design follows the hydrate/dehydrate model from §6.1 of
//! `docs/design/v0.1/session2-compiler-and-runtime.md`.

use chrono::{DateTime, Utc};
use serde::{Deserialize, Serialize};
use uuid::Uuid;

pub type InstanceId = Uuid;
pub type TokenId = Uuid;
pub type EventId = Uuid;

/// Lifecycle status of a workflow instance.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum InstanceStatus {
    Active,
    Completed,
    Failed,
    Cancelled,
}

impl std::fmt::Display for InstanceStatus {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        let s = match self {
            InstanceStatus::Active => "active",
            InstanceStatus::Completed => "completed",
            InstanceStatus::Failed => "failed",
            InstanceStatus::Cancelled => "cancelled",
        };
        write!(f, "{}", s)
    }
}

/// A running or completed workflow instance.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct WorkflowInstance {
    pub id: InstanceId,
    pub journey_name: String,
    pub version: i32,
    pub status: InstanceStatus,
    pub started_at: DateTime<Utc>,
    pub completed_at: Option<DateTime<Utc>>,
    pub data: serde_json::Value,
}

/// A single active execution path inside an instance.
///
/// In a sequential process there is exactly one token. Parallel forks
/// create one child token per outgoing branch.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ActiveToken {
    pub id: TokenId,
    pub instance_id: InstanceId,
    /// Name of the node the token is currently at.
    pub current_node: String,
    /// ID of the parallel/inclusive gateway that spawned this token, if any.
    pub fork_ref: Option<Uuid>,
    /// Ordered list of fork gateway names from the process root to this branch.
    pub branch_lineage: Vec<String>,
    /// Ordered log of data writes this token has produced.
    pub write_log: Vec<WriteLogEntry>,
}

/// A single data write recorded in a token's write log.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct WriteLogEntry {
    pub location: String,
    pub value: serde_json::Value,
}

/// A wrapped event ready to be processed by the event loop.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct EventEnvelope {
    pub id: EventId,
    pub instance_id: InstanceId,
    pub event_kind: EventKind,
    pub payload: serde_json::Value,
}

/// The eight event kinds defined in §6.3.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum EventKind {
    InstanceStart,
    VerbCompletion,
    TimerFired,
    MessageArrived,
    SwitchDecisionReply,
    HumanTaskComplete,
    SubProcessComplete,
    ErrorRaised,
    CancellationTriggered,
}

impl std::fmt::Display for EventKind {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        // Use the serde snake_case string representation.
        let s = serde_json::to_string(self).unwrap_or_default();
        write!(f, "{}", s.trim_matches('"'))
    }
}

impl std::str::FromStr for EventKind {
    type Err = String;
    fn from_str(s: &str) -> Result<Self, Self::Err> {
        serde_json::from_str(&format!("\"{}\"", s)).map_err(|e| e.to_string())
    }
}
