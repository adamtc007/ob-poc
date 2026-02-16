//! Core types for the compiled runbook execution model.
//!
//! A `CompiledRunbook` is the **sole executable artifact** — no raw DSL may be
//! executed outside this wrapper. The existing `repl::Runbook` (entries, DAG,
//! status tracking) remains the inner work-in-progress model; `CompiledRunbook`
//! wraps a frozen snapshot of it with an immutable ID, version, replay envelope,
//! and lifecycle status.
//!
//! ## Invariants
//!
//! - **INV-1a**: A `CompiledRunbook` is immutable once created. Status
//!   transitions (`Compiled → Executing → …`) do NOT mutate the step list or
//!   envelope — they are tracked in `CompiledRunbookStatus`.
//! - **INV-3**: `execute_runbook()` is the only function that may drive the
//!   underlying executor; it requires a valid `CompiledRunbookId`.

use chrono::{DateTime, Utc};
use serde::{Deserialize, Serialize};
use uuid::Uuid;

use super::envelope::ReplayEnvelope;

// ---------------------------------------------------------------------------
// CompiledRunbookId — opaque execution handle
// ---------------------------------------------------------------------------

/// Opaque execution handle wrapping a UUID.
///
/// This is the **only** token accepted by `execute_runbook()`.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, Serialize, Deserialize)]
pub struct CompiledRunbookId(pub Uuid);

impl CompiledRunbookId {
    pub fn new() -> Self {
        Self(Uuid::new_v4())
    }
}

impl Default for CompiledRunbookId {
    fn default() -> Self {
        Self::new()
    }
}

impl std::fmt::Display for CompiledRunbookId {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        write!(f, "{}", self.0)
    }
}

// ---------------------------------------------------------------------------
// CompiledRunbook — the immutable compilation artifact
// ---------------------------------------------------------------------------

/// An immutable compilation artifact that wraps a set of runbook steps.
///
/// Created by `process_utterance()`, consumed by `execute_runbook()`.
/// The step list and envelope are frozen at creation time and never mutated.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct CompiledRunbook {
    /// Unique execution handle.
    pub id: CompiledRunbookId,

    /// Session that produced this runbook.
    pub session_id: Uuid,

    /// Monotonically increasing version within the session.
    pub version: u64,

    /// Frozen steps to execute (verb FQN + DSL + args + dependencies).
    pub steps: Vec<CompiledStep>,

    /// Replay envelope capturing the determinism boundary.
    pub envelope: ReplayEnvelope,

    /// Current lifecycle status.
    pub status: CompiledRunbookStatus,

    /// When this runbook was compiled.
    pub created_at: DateTime<Utc>,
}

impl CompiledRunbook {
    /// Create a new compiled runbook in `Compiled` status.
    pub fn new(
        session_id: Uuid,
        version: u64,
        steps: Vec<CompiledStep>,
        envelope: ReplayEnvelope,
    ) -> Self {
        Self {
            id: CompiledRunbookId::new(),
            session_id,
            version,
            steps,
            envelope,
            status: CompiledRunbookStatus::Compiled,
            created_at: Utc::now(),
        }
    }

    /// Number of steps.
    pub fn step_count(&self) -> usize {
        self.steps.len()
    }

    /// Whether this runbook can be submitted to `execute_runbook()`.
    pub fn is_executable(&self) -> bool {
        matches!(
            self.status,
            CompiledRunbookStatus::Compiled | CompiledRunbookStatus::Parked { .. }
        )
    }
}

// ---------------------------------------------------------------------------
// CompiledStep — a single frozen step
// ---------------------------------------------------------------------------

/// A single step inside a compiled runbook.
///
/// Maps 1:1 with the existing `RunbookEntry` but is frozen and cannot be
/// edited after compilation.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct CompiledStep {
    /// Stable step ID (from the originating `RunbookEntry.id`).
    pub step_id: Uuid,

    /// Human-readable sentence describing the action.
    pub sentence: String,

    /// Verb fully-qualified name (e.g., `"cbu.create"`).
    pub verb: String,

    /// The DSL s-expression to execute.
    pub dsl: String,

    /// Extracted arguments for audit/display.
    pub args: std::collections::HashMap<String, String>,

    /// Step IDs this step depends on (DAG edges).
    pub depends_on: Vec<Uuid>,

    /// Execution mode governing how the step is run.
    pub execution_mode: ExecutionMode,

    /// Entity UUIDs that this step will write to (derived at compile time).
    /// Used to compute the pre-lock set.
    pub write_set: Vec<Uuid>,
}

// ---------------------------------------------------------------------------
// ExecutionMode
// ---------------------------------------------------------------------------

/// How a step should be executed.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum ExecutionMode {
    /// Synchronous — execute and return immediately.
    Sync,
    /// Durable — park and wait for external completion signal.
    Durable,
    /// Human gate — park and wait for human approval.
    HumanGate,
}

// ---------------------------------------------------------------------------
// CompiledRunbookStatus — lifecycle state machine
// ---------------------------------------------------------------------------

/// Lifecycle status of a compiled runbook.
///
/// ```text
/// Compiled ──► Executing ──► Completed
///                  │
///                  ├──► Parked { reason, cursor }
///                  │       │
///                  │       └──► (resume) ──► Executing
///                  │
///                  └──► Failed { error }
/// ```
///
/// `Compiled` is the initial (and only valid entry) state.
/// There is intentionally **no Draft state** — drafts live in the REPL
/// `Runbook` model and only become `CompiledRunbook` when frozen.
#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(tag = "status", rename_all = "snake_case")]
pub enum CompiledRunbookStatus {
    /// Freshly compiled, ready for execution.
    Compiled,

    /// Currently executing steps.
    Executing {
        /// Index of the step currently being executed (0-based).
        current_step: usize,
    },

    /// Parked — waiting for an external signal or human action.
    Parked {
        /// Why execution is parked.
        reason: ParkReason,
        /// Cursor pointing to the step that caused parking.
        cursor: StepCursor,
    },

    /// All steps completed successfully.
    Completed { completed_at: DateTime<Utc> },

    /// Execution failed.
    Failed {
        error: String,
        /// Step that failed (if known).
        failed_step: Option<StepCursor>,
    },
}

// ---------------------------------------------------------------------------
// ParkReason
// ---------------------------------------------------------------------------

/// Why a compiled runbook was parked mid-execution.
#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum ParkReason {
    /// Waiting for an external callback (e.g., BPMN task completion).
    AwaitingCallback { correlation_key: String },
    /// User explicitly paused execution.
    UserPaused,
    /// A required resource is temporarily unavailable.
    ResourceUnavailable { resource: String },
    /// Waiting for human approval gate.
    HumanGate { entry_id: Uuid },
}

// ---------------------------------------------------------------------------
// StepCursor
// ---------------------------------------------------------------------------

/// Points to a specific step for resume or error reporting.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
pub struct StepCursor {
    /// 0-based index into the step list.
    pub index: usize,
    /// Step ID for cross-reference.
    pub step_id: Uuid,
}

// ---------------------------------------------------------------------------
// Tests
// ---------------------------------------------------------------------------

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn compiled_runbook_id_display() {
        let id = CompiledRunbookId::new();
        let s = id.to_string();
        assert!(!s.is_empty());
        // UUIDv7 parses back
        let parsed: Uuid = s.parse().unwrap();
        assert_eq!(parsed, id.0);
    }

    #[test]
    fn is_executable() {
        let rb = CompiledRunbook::new(Uuid::new_v4(), 1, vec![], ReplayEnvelope::empty());
        assert!(rb.is_executable());

        let mut parked = rb.clone();
        parked.status = CompiledRunbookStatus::Parked {
            reason: ParkReason::UserPaused,
            cursor: StepCursor {
                index: 0,
                step_id: Uuid::new_v4(),
            },
        };
        assert!(parked.is_executable());

        let mut completed = rb.clone();
        completed.status = CompiledRunbookStatus::Completed {
            completed_at: Utc::now(),
        };
        assert!(!completed.is_executable());

        let mut failed = rb;
        failed.status = CompiledRunbookStatus::Failed {
            error: "boom".into(),
            failed_step: None,
        };
        assert!(!failed.is_executable());
    }
}
