//! Core types for BPMN-Lite integration.
//!
//! All types follow ob-poc's type-safety-first rule: no `serde_json::json!`
//! for structured data, typed structs with `Serialize`/`Deserialize` everywhere.

use chrono::{DateTime, Utc};
use serde::{Deserialize, Serialize};
use uuid::Uuid;

// ─── Execution Routing ───────────────────────────────────────────────────────

/// How a verb should be executed.
#[derive(Debug, Clone, Copy, Default, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum ExecutionRoute {
    /// Execute directly via DslExecutor (no BPMN involvement).
    #[default]
    Direct,
    /// Route through bpmn-lite gRPC service.
    Orchestrated,
}

/// Configuration for a workflow-enabled verb.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct WorkflowBinding {
    /// Fully-qualified verb name (e.g., "kyc.open-case").
    pub verb_fqn: String,
    /// Execution route for this verb.
    pub route: ExecutionRoute,
    /// BPMN process key (required for orchestrated verbs).
    pub process_key: Option<String>,
    /// Task type → ob-poc verb mappings for this workflow.
    pub task_bindings: Vec<TaskBinding>,
}

/// Maps a BPMN service task type to an ob-poc verb.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct TaskBinding {
    /// BPMN service task type (e.g., "create_case_record").
    pub task_type: String,
    /// ob-poc verb to execute (e.g., "kyc.create-case").
    pub verb_fqn: String,
    /// Timeout for verb execution in milliseconds.
    #[serde(default)]
    pub timeout_ms: Option<u64>,
    /// Maximum retry attempts on transient failure.
    #[serde(default = "default_max_retries")]
    pub max_retries: u32,
}

fn default_max_retries() -> u32 {
    3
}

// ─── Correlation Record ──────────────────────────────────────────────────────

/// Links a BPMN process instance to an ob-poc REPL session/runbook entry.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct CorrelationRecord {
    /// Primary key — unique correlation identifier.
    pub correlation_id: Uuid,
    /// BPMN-Lite process instance ID (from StartResponse).
    pub process_instance_id: Uuid,
    /// ob-poc REPL session ID.
    pub session_id: Uuid,
    /// ob-poc runbook ID within the session.
    pub runbook_id: Uuid,
    /// Runbook entry ID that triggered the orchestration.
    pub entry_id: Uuid,
    /// BPMN process key (e.g., "kyc-open-case").
    pub process_key: String,
    /// SHA-256 hash of the domain payload sent to StartProcess.
    pub domain_payload_hash: Vec<u8>,
    /// Current status of the correlation.
    pub status: CorrelationStatus,
    /// When the correlation was created.
    pub created_at: DateTime<Utc>,
    /// When the correlation was completed/failed/cancelled.
    pub completed_at: Option<DateTime<Utc>>,
    /// Domain-level correlation key (e.g., case_id as string).
    /// Extracted from the DurableConfig.correlation_field at dispatch time.
    /// Enables lifecycle signal verbs to discover active BPMN processes
    /// for a given domain entity.
    pub domain_correlation_key: Option<String>,
}

/// Status of a BPMN correlation record.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum CorrelationStatus {
    Active,
    Completed,
    Failed,
    Cancelled,
}

impl CorrelationStatus {
    pub fn as_str(&self) -> &'static str {
        match self {
            Self::Active => "active",
            Self::Completed => "completed",
            Self::Failed => "failed",
            Self::Cancelled => "cancelled",
        }
    }

    pub fn parse(s: &str) -> Option<Self> {
        match s {
            "active" => Some(Self::Active),
            "completed" => Some(Self::Completed),
            "failed" => Some(Self::Failed),
            "cancelled" => Some(Self::Cancelled),
            _ => None,
        }
    }
}

// ─── Job Frame ───────────────────────────────────────────────────────────────

/// Tracks job activation/completion for dedupe in the job worker.
///
/// When a job is activated, a frame is inserted with `status = Active`.
/// On redelivery, the worker checks the frame: if `Completed`, it returns
/// the cached result without re-executing the verb.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct JobFrame {
    /// Primary key — stable, engine-provided idempotency key.
    pub job_key: String,
    /// BPMN-Lite process instance this job belongs to.
    pub process_instance_id: Uuid,
    /// BPMN service task type (e.g., "create_case_record").
    pub task_type: String,
    /// Worker ID that activated this job.
    pub worker_id: String,
    /// Current status.
    pub status: JobFrameStatus,
    /// When the job was first activated.
    pub activated_at: DateTime<Utc>,
    /// When the job was completed or failed.
    pub completed_at: Option<DateTime<Utc>>,
    /// Number of activation attempts.
    pub attempts: i32,
}

/// Status of a job frame.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum JobFrameStatus {
    Active,
    Completed,
    Failed,
}

impl JobFrameStatus {
    pub fn as_str(&self) -> &'static str {
        match self {
            Self::Active => "active",
            Self::Completed => "completed",
            Self::Failed => "failed",
        }
    }

    pub fn parse(s: &str) -> Option<Self> {
        match s {
            "active" => Some(Self::Active),
            "completed" => Some(Self::Completed),
            "failed" => Some(Self::Failed),
            _ => None,
        }
    }
}

// ─── Parked Token ────────────────────────────────────────────────────────────

/// Represents an ob-poc REPL entry that is parked waiting for a BPMN signal.
///
/// Created by the EventBridge when BPMN-Lite emits wait events (WaitMsg,
/// WaitTimer, UserTask). Resolved when the corresponding signal arrives.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ParkedToken {
    /// Primary key.
    pub token_id: Uuid,
    /// Correlation key for O(1) lookup (format: "{runbook_id}:{entry_id}").
    pub correlation_key: String,
    /// ob-poc REPL session ID.
    pub session_id: Uuid,
    /// Runbook entry ID that is parked.
    pub entry_id: Uuid,
    /// BPMN-Lite process instance ID.
    pub process_instance_id: Uuid,
    /// What signal is expected (e.g., "docs_received", "reviewer_decision").
    pub expected_signal: String,
    /// Current status.
    pub status: ParkedTokenStatus,
    /// When the token was created.
    pub created_at: DateTime<Utc>,
    /// When the token was resolved.
    pub resolved_at: Option<DateTime<Utc>>,
    /// Result payload from the resolving signal.
    pub result_payload: Option<serde_json::Value>,
}

/// Status of a parked token.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum ParkedTokenStatus {
    Waiting,
    Resolved,
    TimedOut,
    Cancelled,
}

impl ParkedTokenStatus {
    pub fn as_str(&self) -> &'static str {
        match self {
            Self::Waiting => "waiting",
            Self::Resolved => "resolved",
            Self::TimedOut => "timed_out",
            Self::Cancelled => "cancelled",
        }
    }

    pub fn parse(s: &str) -> Option<Self> {
        match s {
            "waiting" => Some(Self::Waiting),
            "resolved" => Some(Self::Resolved),
            "timed_out" => Some(Self::TimedOut),
            "cancelled" => Some(Self::Cancelled),
            _ => None,
        }
    }
}

// ─── Outcome Events ──────────────────────────────────────────────────────────

/// Events translated from BPMN lifecycle events into ob-poc terms.
///
/// The REPL never sees BPMN-specific terminology (fiber_id, bytecode_addr).
/// These events are the bridge's output, consumed by the signal handler.
#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(tag = "kind", rename_all = "snake_case")]
pub enum OutcomeEvent {
    /// A BPMN service task (job) completed successfully.
    StepCompleted {
        process_instance_id: Uuid,
        job_key: String,
        task_type: String,
        result: serde_json::Value,
    },
    /// A BPMN service task (job) failed.
    StepFailed {
        process_instance_id: Uuid,
        job_key: String,
        task_type: String,
        error: String,
    },
    /// The entire BPMN process completed.
    ProcessCompleted { process_instance_id: Uuid },
    /// The BPMN process was cancelled.
    ProcessCancelled {
        process_instance_id: Uuid,
        reason: String,
    },
    /// An incident was created (unrecoverable error).
    IncidentCreated {
        process_instance_id: Uuid,
        service_task_id: String,
        error: String,
    },
}

// ─── Tests ───────────────────────────────────────────────────────────────────

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_execution_route_serde() {
        let direct = ExecutionRoute::Direct;
        let json = serde_json::to_string(&direct).unwrap();
        assert_eq!(json, "\"direct\"");

        let orchestrated: ExecutionRoute = serde_json::from_str("\"orchestrated\"").unwrap();
        assert_eq!(orchestrated, ExecutionRoute::Orchestrated);
    }

    #[test]
    fn test_correlation_status_roundtrip() {
        for status in [
            CorrelationStatus::Active,
            CorrelationStatus::Completed,
            CorrelationStatus::Failed,
            CorrelationStatus::Cancelled,
        ] {
            let s = status.as_str();
            let parsed = CorrelationStatus::parse(s).unwrap();
            assert_eq!(parsed, status);
        }
    }

    #[test]
    fn test_job_frame_status_roundtrip() {
        for status in [
            JobFrameStatus::Active,
            JobFrameStatus::Completed,
            JobFrameStatus::Failed,
        ] {
            let s = status.as_str();
            let parsed = JobFrameStatus::parse(s).unwrap();
            assert_eq!(parsed, status);
        }
    }

    #[test]
    fn test_parked_token_status_roundtrip() {
        for status in [
            ParkedTokenStatus::Waiting,
            ParkedTokenStatus::Resolved,
            ParkedTokenStatus::TimedOut,
            ParkedTokenStatus::Cancelled,
        ] {
            let s = status.as_str();
            let parsed = ParkedTokenStatus::parse(s).unwrap();
            assert_eq!(parsed, status);
        }
    }

    #[test]
    fn test_outcome_event_serde() {
        let event = OutcomeEvent::StepCompleted {
            process_instance_id: Uuid::nil(),
            job_key: "test-key".to_string(),
            task_type: "create_case".to_string(),
            result: serde_json::json!({"case_id": "123"}),
        };
        let json = serde_json::to_string(&event).unwrap();
        assert!(json.contains("\"kind\":\"step_completed\""));
        assert!(json.contains("\"task_type\":\"create_case\""));

        let deserialized: OutcomeEvent = serde_json::from_str(&json).unwrap();
        match deserialized {
            OutcomeEvent::StepCompleted { task_type, .. } => {
                assert_eq!(task_type, "create_case");
            }
            _ => panic!("Wrong variant"),
        }
    }

    #[test]
    fn test_task_binding_defaults() {
        let json = r#"{"task_type":"test","verb_fqn":"verb.test"}"#;
        let binding: TaskBinding = serde_json::from_str(json).unwrap();
        assert_eq!(binding.max_retries, 3);
        assert!(binding.timeout_ms.is_none());
    }
}
