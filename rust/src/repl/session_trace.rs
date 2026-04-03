//! Session trace infrastructure — append-only log capturing every session mutation.
//!
//! The trace is a monotonically sequenced log of operations applied to a session.
//! It powers replay (R9), compliance auditing, and regression testing.

use chrono::{DateTime, Utc};
use serde::{Deserialize, Serialize};
use uuid::Uuid;

use super::types_v2::{AgentMode, WorkspaceKind};

// ---------------------------------------------------------------------------
// SnapshotPolicy
// ---------------------------------------------------------------------------

/// Controls when hydrated state snapshots are captured in trace entries.
#[derive(Debug, Clone, Copy, Default, Serialize, Deserialize, PartialEq, Eq)]
#[serde(rename_all = "snake_case")]
pub enum SnapshotPolicy {
    /// Never capture snapshots.
    #[default]
    Never,
    /// Capture every N operations.
    EveryN(u32),
    /// Capture on every stack operation.
    OnStackOp,
    /// Capture on every verb execution.
    OnExecution,
}

// ---------------------------------------------------------------------------
// FrameRef — lightweight stack snapshot
// ---------------------------------------------------------------------------

/// Lightweight reference to a workspace frame captured at trace time.
#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq)]
pub struct FrameRef {
    pub workspace: WorkspaceKind,
    pub constellation_map: String,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub subject_id: Option<Uuid>,
    #[serde(default)]
    pub stale: bool,
}

// ---------------------------------------------------------------------------
// TraceOp — discriminated operation tag
// ---------------------------------------------------------------------------

/// The operation that occurred at this trace entry.
#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq)]
#[serde(tag = "op", rename_all = "snake_case")]
pub enum TraceOp {
    StackPush {
        workspace: WorkspaceKind,
    },
    StackPop {
        workspace: WorkspaceKind,
    },
    StackCommit,
    VerbExecuted {
        verb_fqn: String,
        step_id: Uuid,
    },
    RunbookCompiled {
        runbook_id: String,
    },
    RunbookApproved {
        runbook_id: String,
    },
    StateTransition {
        from: String,
        to: String,
    },
    Input {
        utterance_hash: String,
    },
    /// A shared fact was superseded (cross-workspace consistency).
    SharedFactSuperseded {
        atom_path: String,
        entity_id: Uuid,
        new_version: i32,
    },
    /// A consuming constellation was replayed after shared fact change.
    ConstellationReplayed {
        workspace: String,
        constellation_family: String,
        outcome: String,
    },
    /// A remediation event changed state.
    RemediationStateChange {
        remediation_id: Uuid,
        from_status: String,
        to_status: String,
    },
}

// ---------------------------------------------------------------------------
// TraceEntry — one row in the append-only trace log
// ---------------------------------------------------------------------------

/// A single entry in the session trace log.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct TraceEntry {
    pub session_id: Uuid,
    pub sequence: u64,
    pub timestamp: DateTime<Utc>,
    pub agent_mode: AgentMode,
    pub op: TraceOp,
    pub stack_snapshot: Vec<FrameRef>,
    /// Hydrated state snapshot (when `SnapshotPolicy` triggers).
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub snapshot: Option<serde_json::Value>,
    /// Session feedback snapshot at the time of this operation.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub session_feedback: Option<serde_json::Value>,
    /// Verb FQN if a verb was resolved during this turn.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub verb_resolved: Option<String>,
    /// Execution result snapshot (step outcome).
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub execution_result: Option<serde_json::Value>,
}

impl TraceEntry {
    /// Create a new trace entry.
    pub fn new(
        session_id: Uuid,
        sequence: u64,
        agent_mode: AgentMode,
        op: TraceOp,
        stack_snapshot: Vec<FrameRef>,
    ) -> Self {
        Self {
            session_id,
            sequence,
            timestamp: Utc::now(),
            agent_mode,
            op,
            stack_snapshot,
            snapshot: None,
            session_feedback: None,
            verb_resolved: None,
            execution_result: None,
        }
    }

    /// Attach a session feedback snapshot.
    pub fn with_session_feedback(mut self, feedback: serde_json::Value) -> Self {
        self.session_feedback = Some(feedback);
        self
    }

    /// Attach the resolved verb FQN.
    pub fn with_verb_resolved(mut self, verb_fqn: String) -> Self {
        self.verb_resolved = Some(verb_fqn);
        self
    }

    /// Attach an execution result snapshot.
    pub fn with_execution_result(mut self, result: serde_json::Value) -> Self {
        self.execution_result = Some(result);
        self
    }

    /// Attach a hydrated state snapshot.
    pub fn with_snapshot(mut self, snapshot: serde_json::Value) -> Self {
        self.snapshot = Some(snapshot);
        self
    }
}

// ---------------------------------------------------------------------------
// Tests
// ---------------------------------------------------------------------------

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn trace_entry_serde_round_trip() {
        let entry = TraceEntry::new(
            Uuid::nil(),
            1,
            AgentMode::Sage,
            TraceOp::StackPush {
                workspace: WorkspaceKind::Deal,
            },
            vec![FrameRef {
                workspace: WorkspaceKind::Cbu,
                constellation_map: "cbu-onboarding".into(),
                subject_id: None,
                stale: false,
            }],
        );
        let json = serde_json::to_value(&entry).unwrap();
        let back: TraceEntry = serde_json::from_value(json.clone()).unwrap();
        assert_eq!(back.session_id, Uuid::nil());
        assert_eq!(back.sequence, 1);
        assert_eq!(
            back.op,
            TraceOp::StackPush {
                workspace: WorkspaceKind::Deal
            }
        );
        assert_eq!(back.stack_snapshot.len(), 1);
    }

    #[test]
    fn trace_op_serde_variants() {
        let ops = vec![
            TraceOp::StackPush {
                workspace: WorkspaceKind::Kyc,
            },
            TraceOp::StackPop {
                workspace: WorkspaceKind::Deal,
            },
            TraceOp::StackCommit,
            TraceOp::VerbExecuted {
                verb_fqn: "cbu.create".into(),
                step_id: Uuid::nil(),
            },
            TraceOp::RunbookCompiled {
                runbook_id: "abc123".into(),
            },
            TraceOp::RunbookApproved {
                runbook_id: "abc123".into(),
            },
            TraceOp::StateTransition {
                from: "draft".into(),
                to: "ready".into(),
            },
            TraceOp::Input {
                utterance_hash: "sha256:...".into(),
            },
        ];
        for op in &ops {
            let json = serde_json::to_value(op).unwrap();
            let back: TraceOp = serde_json::from_value(json).unwrap();
            assert_eq!(&back, op);
        }
    }

    #[test]
    fn sequence_monotonicity() {
        let mut seq = 0u64;
        let entries: Vec<TraceEntry> = (0..5)
            .map(|_| {
                seq += 1;
                TraceEntry::new(
                    Uuid::nil(),
                    seq,
                    AgentMode::Sage,
                    TraceOp::StackCommit,
                    vec![],
                )
            })
            .collect();
        for (i, entry) in entries.iter().enumerate() {
            assert_eq!(entry.sequence, (i + 1) as u64);
        }
    }

    #[test]
    fn snapshot_policy_default() {
        assert_eq!(SnapshotPolicy::default(), SnapshotPolicy::Never);
    }
}
