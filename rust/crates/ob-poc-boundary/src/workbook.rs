//! Execution Workbook contract.
//!
//! A workbook is the immutable handoff between Sage/discovery and the DSL Drafter
//! gate. It binds a declared Domain Pack transition to configuration/state
//! anchors, evidence references, execution mode, and a SemOS simulation result.

use chrono::{DateTime, Utc};
use sem_os_core::state_simulation::StateSimulationResult;
use serde::{Deserialize, Serialize};
use sha2::{Digest, Sha256};
use std::collections::BTreeMap;
use uuid::Uuid;

#[derive(Debug, Clone, PartialEq, Eq, Hash, Serialize, Deserialize)]
pub struct ExecutionWorkbookId(pub String);

impl std::fmt::Display for ExecutionWorkbookId {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        write!(f, "{}", self.0)
    }
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct ExecutionWorkbook {
    pub id: ExecutionWorkbookId,
    pub core: ExecutionWorkbookCore,
    pub status: ExecutionWorkbookStatus,
    pub created_at: DateTime<Utc>,
}

impl ExecutionWorkbook {
    pub fn new(core: ExecutionWorkbookCore) -> Result<Self, ExecutionWorkbookValidationError> {
        core.validate()?;
        let id = compute_workbook_id(&core);
        Ok(Self {
            id,
            core,
            status: ExecutionWorkbookStatus::Draft,
            created_at: Utc::now(),
        })
    }

    pub fn validate_integrity(&self) -> Result<(), ExecutionWorkbookValidationError> {
        self.core.validate()?;
        let expected = compute_workbook_id(&self.core);
        if expected != self.id {
            return Err(ExecutionWorkbookValidationError::HashMismatch {
                expected,
                actual: self.id.clone(),
            });
        }
        Ok(())
    }
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct ExecutionWorkbookCore {
    pub schema_version: u16,
    pub pack_id: String,
    pub transition_ref: String,
    pub execution_mode: WorkbookExecutionMode,
    pub session_id: Uuid,
    pub subject: WorkbookSubject,
    pub actor: WorkbookActor,
    pub configuration_version: String,
    pub state_snapshot_id: String,
    pub objective: String,
    #[serde(default)]
    pub user_prompt_ref: Option<String>,
    #[serde(default)]
    pub editor_context_refs: Vec<String>,
    pub evidence_refs: Vec<EvidenceRef>,
    #[serde(default)]
    pub llm_trace_ref: Option<LlmTraceRef>,
    #[serde(default)]
    pub expected_preconditions: Vec<String>,
    #[serde(default)]
    pub expected_postconditions: Vec<String>,
    #[serde(default)]
    pub invariant_checks: Vec<WorkbookCheck>,
    #[serde(default)]
    pub governance_checks: Vec<WorkbookCheck>,
    pub simulation: StateSimulationResult,
    pub stale_policy: StaleWorkbookPolicy,
    #[serde(default)]
    pub previous_workbook_id: Option<ExecutionWorkbookId>,
    #[serde(default)]
    pub metadata: BTreeMap<String, String>,
}

impl ExecutionWorkbookCore {
    pub fn validate(&self) -> Result<(), ExecutionWorkbookValidationError> {
        require_non_empty("pack_id", &self.pack_id)?;
        require_non_empty("transition_ref", &self.transition_ref)?;
        require_non_empty("configuration_version", &self.configuration_version)?;
        require_non_empty("state_snapshot_id", &self.state_snapshot_id)?;
        require_non_empty("objective", &self.objective)?;
        require_non_empty("subject_kind", &self.subject.subject_kind)?;
        require_non_empty("actor_id", &self.actor.actor_id)?;

        if self.transition_ref != self.simulation.transition_ref {
            return Err(ExecutionWorkbookValidationError::TransitionRefMismatch {
                workbook_transition_ref: self.transition_ref.clone(),
                simulation_transition_ref: self.simulation.transition_ref.clone(),
            });
        }

        if self.subject.subject_id != self.simulation.entity_id {
            return Err(ExecutionWorkbookValidationError::SubjectMismatch {
                workbook_subject_id: self.subject.subject_id,
                simulation_entity_id: self.simulation.entity_id,
            });
        }

        if self.evidence_refs.is_empty() {
            return Err(ExecutionWorkbookValidationError::MissingEvidenceRefs);
        }

        for evidence_ref in &self.evidence_refs {
            require_non_empty("evidence_ref.kind", &evidence_ref.kind)?;
            require_non_empty("evidence_ref.ref_id", &evidence_ref.ref_id)?;
            require_non_empty("evidence_ref.digest", &evidence_ref.digest)?;
        }

        Ok(())
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum ExecutionWorkbookStatus {
    Draft,
    Validated,
    Superseded,
    Executed,
    Rejected,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum WorkbookExecutionMode {
    DryRun,
    ExecuteAfterApproval,
    Execute,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct WorkbookSubject {
    pub subject_kind: String,
    pub subject_id: Uuid,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct WorkbookActor {
    pub actor_id: String,
    #[serde(default)]
    pub roles: Vec<String>,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct EvidenceRef {
    pub kind: String,
    pub ref_id: String,
    pub digest: String,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub source_system: Option<String>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub field_path: Option<String>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub classification: Option<String>,
}

// LlmTraceRef lives next door in this crate's llm_trace module.
pub use crate::llm_trace::LlmTraceRef;

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum StaleWorkbookPolicy {
    Reject,
    Revalidate,
    RebindIfEquivalent,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct WorkbookCheck {
    pub check_id: String,
    pub status: WorkbookCheckStatus,
    pub message: String,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum WorkbookCheckStatus {
    Passed,
    Failed,
    NotEvaluated,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(tag = "kind", rename_all = "snake_case")]
pub enum ExecutionWorkbookValidationError {
    RequiredFieldEmpty {
        field: String,
    },
    MissingEvidenceRefs,
    TransitionRefMismatch {
        workbook_transition_ref: String,
        simulation_transition_ref: String,
    },
    SubjectMismatch {
        workbook_subject_id: Uuid,
        simulation_entity_id: Uuid,
    },
    HashMismatch {
        expected: ExecutionWorkbookId,
        actual: ExecutionWorkbookId,
    },
}

pub fn compute_workbook_id(core: &ExecutionWorkbookCore) -> ExecutionWorkbookId {
    let bytes = canonical_workbook_bytes(core);
    let digest = Sha256::digest(bytes);
    ExecutionWorkbookId(format!("ewb:v1:{}", hex::encode(digest)))
}

pub fn canonical_workbook_bytes(core: &ExecutionWorkbookCore) -> Vec<u8> {
    serde_json::to_vec(core).expect("ExecutionWorkbookCore serializes to canonical JSON bytes")
}

fn require_non_empty(
    field: &'static str,
    value: &str,
) -> Result<(), ExecutionWorkbookValidationError> {
    if value.trim().is_empty() {
        return Err(ExecutionWorkbookValidationError::RequiredFieldEmpty {
            field: field.to_string(),
        });
    }
    Ok(())
}

#[cfg(test)]
mod tests {
    use super::*;
    use sem_os_core::state_simulation::{
        SemanticStateDiff, SimulatedStateAdvance, StateSimulationResult,
    };
    use uuid::uuid;

    const SESSION_ID: Uuid = uuid!("aaaaaaaa-aaaa-aaaa-aaaa-aaaaaaaaaaaa");
    const CASE_ID: Uuid = uuid!("11111111-1111-1111-1111-111111111111");
    const TRACE_ID: Uuid = uuid!("22222222-2222-2222-2222-222222222222");

    fn simulation(transition_ref: &str, entity_id: Uuid) -> StateSimulationResult {
        StateSimulationResult {
            transition_ref: transition_ref.to_string(),
            entity_id,
            entity_type: "kyc_case".to_string(),
            state_machine: "kyc_case_lifecycle".to_string(),
            from_state: "INTAKE".to_string(),
            to_state: "DISCOVERY".to_string(),
            verb: "kyc-case.update-status".to_string(),
            semantic_diff: SemanticStateDiff {
                field: "status".to_string(),
                before: "INTAKE".to_string(),
                after: "DISCOVERY".to_string(),
            },
            predicted_advance: SimulatedStateAdvance {
                entity_id,
                to_node: "kyc-case:discovery".to_string(),
                slot_path: "kyc-case/workstream".to_string(),
                reason: "kyc-case.update-status - INTAKE -> DISCOVERY".to_string(),
                writes_since_push_delta: 1,
            },
            state_snapshot_id: Some("state-snapshot-1".to_string()),
            configuration_version: Some("config-1".to_string()),
        }
    }

    fn core() -> ExecutionWorkbookCore {
        ExecutionWorkbookCore {
            schema_version: 1,
            pack_id: "ob-poc.kyc".to_string(),
            transition_ref: "kyc-case.intake-to-discovery".to_string(),
            execution_mode: WorkbookExecutionMode::DryRun,
            session_id: SESSION_ID,
            subject: WorkbookSubject {
                subject_kind: "kyc_case".to_string(),
                subject_id: CASE_ID,
            },
            actor: WorkbookActor {
                actor_id: "analyst@example.com".to_string(),
                roles: vec!["analyst".to_string()],
            },
            configuration_version: "config-1".to_string(),
            state_snapshot_id: "state-snapshot-1".to_string(),
            objective: "Move KYC case from intake to discovery".to_string(),
            user_prompt_ref: Some("prompt:sha256:test".to_string()),
            editor_context_refs: vec!["semos://entity/test".to_string()],
            evidence_refs: vec![EvidenceRef {
                kind: "case_id".to_string(),
                ref_id: CASE_ID.to_string(),
                digest: "sha256:case".to_string(),
                source_system: Some("ob-poc".to_string()),
                field_path: Some("cases.id".to_string()),
                classification: Some("internal".to_string()),
            }],
            llm_trace_ref: Some(LlmTraceRef {
                trace_id: TRACE_ID,
                prompt_hash: "sha256:prompt".to_string(),
                response_hash: "sha256:response".to_string(),
            }),
            expected_preconditions: vec!["status == INTAKE".to_string()],
            expected_postconditions: vec!["status == DISCOVERY".to_string()],
            invariant_checks: vec![WorkbookCheck {
                check_id: "kyc.transition.frontier".to_string(),
                status: WorkbookCheckStatus::Passed,
                message: "transition is declared in Domain Pack".to_string(),
            }],
            governance_checks: vec![WorkbookCheck {
                check_id: "kyc.evidence.case_id".to_string(),
                status: WorkbookCheckStatus::Passed,
                message: "case evidence reference present".to_string(),
            }],
            simulation: simulation("kyc-case.intake-to-discovery", CASE_ID),
            stale_policy: StaleWorkbookPolicy::Revalidate,
            previous_workbook_id: None,
            metadata: BTreeMap::new(),
        }
    }

    #[test]
    fn workbook_id_is_reproducible_from_core() {
        let core = core();
        let a = compute_workbook_id(&core);
        let b = compute_workbook_id(&core);

        assert_eq!(a, b);
        assert!(a.0.starts_with("ewb:v1:"));
    }

    #[test]
    fn workbook_timestamp_does_not_affect_integrity() {
        let workbook = ExecutionWorkbook::new(core()).expect("workbook");
        let mut later = workbook.clone();
        later.created_at = Utc::now();

        assert_eq!(workbook.id, later.id);
        later.validate_integrity().expect("integrity still valid");
    }

    #[test]
    fn workbook_records_supersession_and_stale_policy_in_hash() {
        let base = ExecutionWorkbook::new(core()).expect("base workbook");
        let mut next_core = core();
        next_core.previous_workbook_id = Some(base.id);
        next_core.stale_policy = StaleWorkbookPolicy::Reject;

        let next = ExecutionWorkbook::new(next_core).expect("superseding workbook");

        assert!(next.core.previous_workbook_id.is_some());
        assert_eq!(next.core.stale_policy, StaleWorkbookPolicy::Reject);
        assert_ne!(next.id, compute_workbook_id(&core()));
    }

    #[test]
    fn workbook_hash_uses_canonical_json_payload() {
        let core = core();
        let bytes = canonical_workbook_bytes(&core);

        assert!(std::str::from_utf8(&bytes)
            .unwrap()
            .contains("\"execution_mode\":\"dry_run\""));
        assert_eq!(compute_workbook_id(&core), compute_workbook_id(&core));
    }

    #[test]
    fn workbook_requires_typed_transition_binding() {
        let mut core = core();
        core.transition_ref = "kyc-case.discovery-to-assessment".to_string();

        let err = ExecutionWorkbook::new(core).expect_err("workbook refused");

        assert_eq!(
            err,
            ExecutionWorkbookValidationError::TransitionRefMismatch {
                workbook_transition_ref: "kyc-case.discovery-to-assessment".to_string(),
                simulation_transition_ref: "kyc-case.intake-to-discovery".to_string(),
            }
        );
    }

    #[test]
    fn workbook_requires_subject_to_match_simulation_entity() {
        let mut core = core();
        core.subject.subject_id = uuid!("33333333-3333-3333-3333-333333333333");

        let err = ExecutionWorkbook::new(core).expect_err("workbook refused");

        assert_eq!(
            err,
            ExecutionWorkbookValidationError::SubjectMismatch {
                workbook_subject_id: uuid!("33333333-3333-3333-3333-333333333333"),
                simulation_entity_id: CASE_ID,
            }
        );
    }

    #[test]
    fn workbook_integrity_detects_core_mutation() {
        let mut workbook = ExecutionWorkbook::new(core()).expect("workbook");
        workbook.core.configuration_version = "config-2".to_string();

        let err = workbook
            .validate_integrity()
            .expect_err("integrity mismatch");

        assert!(matches!(
            err,
            ExecutionWorkbookValidationError::HashMismatch { .. }
        ));
    }
}
