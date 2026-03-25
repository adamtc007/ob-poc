//! Multi-workspace runbook plan types.
//!
//! A `RunbookPlan` sequences work across multiple workspaces, each step
//! potentially compiling down to a single-workspace `CompiledRunbook` for
//! execution. This is a higher-level orchestration artifact on top of the
//! existing `CompiledRunbook` execution model.

use std::collections::BTreeMap;

use chrono::{DateTime, Utc};
use serde::{Deserialize, Serialize};
use sha2::{Digest, Sha256};
use uuid::Uuid;

use crate::repl::types_v2::{SubjectKind, VerbRef, WorkspaceKind};

// ---------------------------------------------------------------------------
// RunbookPlanId — content-addressed plan identifier
// ---------------------------------------------------------------------------

/// Content-addressed plan identifier (SHA-256 hex of canonical plan bytes).
#[derive(Debug, Clone, PartialEq, Eq, Hash, Serialize, Deserialize)]
pub struct RunbookPlanId(pub String);

impl RunbookPlanId {
    /// Compute a content-addressed ID from canonical bytes.
    pub fn from_bytes(bytes: &[u8]) -> Self {
        let hash = Sha256::digest(bytes);
        Self(hex::encode(hash))
    }
}

impl std::fmt::Display for RunbookPlanId {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        write!(f, "{}", self.0)
    }
}

impl AsRef<str> for RunbookPlanId {
    fn as_ref(&self) -> &str {
        &self.0
    }
}

// ---------------------------------------------------------------------------
// EntityBinding — literal or forward-ref entity references
// ---------------------------------------------------------------------------

/// How a plan step references an entity — either a known UUID or a forward
/// reference to an output from a prior step.
#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq)]
#[serde(tag = "kind", rename_all = "snake_case")]
pub enum EntityBinding {
    /// A known UUID (entity already exists).
    Literal { id: Uuid },
    /// A forward reference to an output field of a prior step.
    ForwardRef {
        source_step: usize,
        output_field: String,
    },
}

// ---------------------------------------------------------------------------
// BindingTable — tracks entity bindings and their resolved values
// ---------------------------------------------------------------------------

/// Tracks named entity bindings and their resolved UUIDs.
#[derive(Debug, Clone, Default, Serialize, Deserialize, PartialEq, Eq)]
pub struct BindingTable {
    /// Named bindings (e.g. "$created_cbu_id" → EntityBinding).
    pub entries: BTreeMap<String, EntityBinding>,
    /// Resolved UUIDs (populated during execution as forward refs are fulfilled).
    pub resolved: BTreeMap<String, Uuid>,
}

impl BindingTable {
    /// Number of named bindings.
    #[must_use]
    pub fn len(&self) -> usize {
        self.entries.len()
    }

    /// Whether the binding table has no entries.
    #[must_use]
    pub fn is_empty(&self) -> bool {
        self.entries.is_empty()
    }

    /// Resolve a binding name to a UUID, if available.
    ///
    /// Literal bindings always resolve. Forward refs resolve only after the
    /// source step has executed and populated `resolved`.
    pub fn resolve(&self, name: &str) -> Option<Uuid> {
        match self.entries.get(name)? {
            EntityBinding::Literal { id } => Some(*id),
            EntityBinding::ForwardRef { output_field, .. } => {
                self.resolved.get(output_field).copied()
            }
        }
    }
}

// ---------------------------------------------------------------------------
// Plan step types
// ---------------------------------------------------------------------------

/// Status of an individual plan step.
#[derive(Debug, Clone, Copy, Serialize, Deserialize, PartialEq, Eq)]
#[serde(rename_all = "snake_case")]
pub enum PlanStepStatus {
    Pending,
    Ready,
    Executing,
    Succeeded,
    Failed,
    Skipped,
}

/// A single step in a multi-workspace runbook plan.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct RunbookPlanStep {
    pub seq: usize,
    pub workspace: WorkspaceKind,
    pub constellation_map: String,
    pub subject_kind: SubjectKind,
    pub subject_binding: EntityBinding,
    pub verb: VerbRef,
    pub sentence: String,
    pub args: BTreeMap<String, String>,
    #[serde(default, skip_serializing_if = "Vec::is_empty")]
    pub preconditions: Vec<String>,
    pub expected_effect: String,
    #[serde(default, skip_serializing_if = "Vec::is_empty")]
    pub depends_on: Vec<usize>,
    pub status: PlanStepStatus,
}

// ---------------------------------------------------------------------------
// Plan-level types
// ---------------------------------------------------------------------------

/// Overall status of a runbook plan.
#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq)]
#[serde(tag = "status", rename_all = "snake_case")]
pub enum RunbookPlanStatus {
    Compiled,
    AwaitingApproval,
    Approved,
    Executing { cursor: usize },
    Completed { completed_at: DateTime<Utc> },
    Failed { error: String, failed_step: Option<usize> },
    Cancelled,
}

/// Approval record for a runbook plan.
#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq)]
pub struct RunbookApproval {
    pub approved_by: String,
    pub approved_at: DateTime<Utc>,
    pub plan_hash: String,
}

/// Result of executing a single plan step.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct StepResult {
    pub step_seq: usize,
    pub verb_fqn: String,
    pub status: PlanStepStatus,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub output: Option<serde_json::Value>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub error: Option<String>,
    pub executed_at: DateTime<Utc>,
}

/// A multi-workspace runbook plan.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct RunbookPlan {
    pub id: RunbookPlanId,
    pub session_id: Uuid,
    pub compiled_at: DateTime<Utc>,
    /// Trace entry sequence numbers that informed this plan.
    #[serde(default, skip_serializing_if = "Vec::is_empty")]
    pub source_research: Vec<u64>,
    pub steps: Vec<RunbookPlanStep>,
    pub bindings: BindingTable,
    pub status: RunbookPlanStatus,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub approval: Option<RunbookApproval>,
}

impl RunbookPlan {
    /// Compute a content-addressed ID from the plan's canonical representation.
    pub fn compute_id(steps: &[RunbookPlanStep], bindings: &BindingTable) -> RunbookPlanId {
        let canonical = serde_json::to_vec(&(steps, bindings))
            .expect("RunbookPlan steps and bindings must be serializable");
        RunbookPlanId::from_bytes(&canonical)
    }

    /// Create a new plan with a content-addressed ID.
    pub fn new(
        session_id: Uuid,
        steps: Vec<RunbookPlanStep>,
        bindings: BindingTable,
        source_research: Vec<u64>,
    ) -> Self {
        let id = Self::compute_id(&steps, &bindings);
        Self {
            id,
            session_id,
            compiled_at: Utc::now(),
            source_research,
            steps,
            bindings,
            status: RunbookPlanStatus::Compiled,
            approval: None,
        }
    }
}

// ---------------------------------------------------------------------------
// Tests
// ---------------------------------------------------------------------------

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn serde_round_trip_plan() {
        let plan = RunbookPlan::new(
            Uuid::nil(),
            vec![RunbookPlanStep {
                seq: 0,
                workspace: WorkspaceKind::Cbu,
                constellation_map: "cbu-onboarding".into(),
                subject_kind: SubjectKind::Cbu,
                subject_binding: EntityBinding::Literal { id: Uuid::nil() },
                verb: VerbRef {
                    verb_fqn: "cbu.create".into(),
                    display_name: "Create CBU".into(),
                },
                sentence: "Create a new CBU".into(),
                args: BTreeMap::from([("name".into(), "Test Corp".into())]),
                preconditions: vec![],
                expected_effect: "CBU created".into(),
                depends_on: vec![],
                status: PlanStepStatus::Pending,
            }],
            BindingTable::default(),
            vec![],
        );
        let json = serde_json::to_value(&plan).unwrap();
        let back: RunbookPlan = serde_json::from_value(json.clone()).unwrap();
        assert_eq!(back.id, plan.id);
        assert_eq!(back.steps.len(), 1);
        // Round-trip
        let json2 = serde_json::to_value(&back).unwrap();
        assert_eq!(json, json2);
    }

    #[test]
    fn content_addressed_id_deterministic() {
        let steps = vec![RunbookPlanStep {
            seq: 0,
            workspace: WorkspaceKind::Deal,
            constellation_map: "deal-lifecycle".into(),
            subject_kind: SubjectKind::Deal,
            subject_binding: EntityBinding::Literal { id: Uuid::nil() },
            verb: VerbRef {
                verb_fqn: "deal.create".into(),
                display_name: "Create Deal".into(),
            },
            sentence: "Create a deal".into(),
            args: BTreeMap::new(),
            preconditions: vec![],
            expected_effect: "Deal created".into(),
            depends_on: vec![],
            status: PlanStepStatus::Pending,
        }];
        let bindings = BindingTable::default();
        let id1 = RunbookPlan::compute_id(&steps, &bindings);
        let id2 = RunbookPlan::compute_id(&steps, &bindings);
        assert_eq!(id1, id2, "Same inputs must produce same ID");
    }

    #[test]
    fn binding_table_resolve_literal() {
        let mut table = BindingTable::default();
        let id = Uuid::new_v4();
        table
            .entries
            .insert("$cbu_id".into(), EntityBinding::Literal { id });
        assert_eq!(table.resolve("$cbu_id"), Some(id));
    }

    #[test]
    fn binding_table_resolve_forward_ref_unresolved() {
        let mut table = BindingTable::default();
        table.entries.insert(
            "$new_cbu".into(),
            EntityBinding::ForwardRef {
                source_step: 0,
                output_field: "created_cbu_id".into(),
            },
        );
        // Not yet resolved
        assert_eq!(table.resolve("$new_cbu"), None);
    }

    #[test]
    fn binding_table_resolve_forward_ref_resolved() {
        let mut table = BindingTable::default();
        let id = Uuid::new_v4();
        table.entries.insert(
            "$new_cbu".into(),
            EntityBinding::ForwardRef {
                source_step: 0,
                output_field: "created_cbu_id".into(),
            },
        );
        table.resolved.insert("created_cbu_id".into(), id);
        assert_eq!(table.resolve("$new_cbu"), Some(id));
    }

    #[test]
    fn plan_step_status_serde() {
        let statuses = vec![
            PlanStepStatus::Pending,
            PlanStepStatus::Ready,
            PlanStepStatus::Executing,
            PlanStepStatus::Succeeded,
            PlanStepStatus::Failed,
            PlanStepStatus::Skipped,
        ];
        for s in &statuses {
            let json = serde_json::to_value(s).unwrap();
            let back: PlanStepStatus = serde_json::from_value(json).unwrap();
            assert_eq!(&back, s);
        }
    }

    #[test]
    fn plan_status_serde() {
        let statuses: Vec<RunbookPlanStatus> = vec![
            RunbookPlanStatus::Compiled,
            RunbookPlanStatus::AwaitingApproval,
            RunbookPlanStatus::Approved,
            RunbookPlanStatus::Executing { cursor: 3 },
            RunbookPlanStatus::Completed {
                completed_at: Utc::now(),
            },
            RunbookPlanStatus::Failed {
                error: "boom".into(),
                failed_step: Some(2),
            },
            RunbookPlanStatus::Cancelled,
        ];
        for s in &statuses {
            let json = serde_json::to_value(s).unwrap();
            let back: RunbookPlanStatus = serde_json::from_value(json).unwrap();
            // Verify variant discrimination survived round-trip
            assert_eq!(
                std::mem::discriminant(&back),
                std::mem::discriminant(s),
                "Status variant mismatch for {:?}",
                s
            );
        }
    }

    #[test]
    fn binding_table_len_and_is_empty() {
        let table = BindingTable::default();
        assert!(table.is_empty());
        assert_eq!(table.len(), 0);

        let mut table2 = BindingTable::default();
        table2
            .entries
            .insert("$x".into(), EntityBinding::Literal { id: Uuid::nil() });
        assert!(!table2.is_empty());
        assert_eq!(table2.len(), 1);
    }

    #[test]
    fn binding_table_resolve_missing_name() {
        let table = BindingTable::default();
        assert_eq!(table.resolve("$nonexistent"), None);
    }
}
