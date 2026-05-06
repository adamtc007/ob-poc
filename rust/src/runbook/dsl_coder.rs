//! DSL Coder validation boundary.
//!
//! This module is intentionally dry-run-only for the MVP slice. It validates an
//! Execution Workbook and returns the already-bound SemOS simulation result
//! without invoking the runbook executor or mutating state.

use sem_os_core::state_simulation::StateSimulationResult;
use serde::{Deserialize, Serialize};

use super::workbook::{
    ExecutionWorkbook, ExecutionWorkbookStatus, ExecutionWorkbookValidationError,
};

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum DslCoderExecutionMode {
    DryRun,
    Mutate,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct DslCoderDryRunResult {
    pub workbook_id: String,
    pub transition_ref: String,
    pub semantic_diff: StateSimulationResult,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct DslCoderValidationError {
    pub code: DslCoderRefusalCode,
    pub message: String,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum DslCoderRefusalCode {
    WorkbookIntegrityFailed,
    MutationNotEnabled,
    WorkbookSuperseded,
    WorkbookAlreadyExecuted,
    WorkbookRejected,
    TransitionBindingMismatch,
}

pub fn validate_workbook_for_dry_run(
    workbook: &ExecutionWorkbook,
    mode: DslCoderExecutionMode,
) -> Result<DslCoderDryRunResult, DslCoderValidationError> {
    if mode != DslCoderExecutionMode::DryRun {
        return Err(refusal(
            DslCoderRefusalCode::MutationNotEnabled,
            "mutation execution is disabled for the MVP dry-run boundary",
        ));
    }

    workbook.validate_integrity().map_err(map_workbook_error)?;

    match workbook.status {
        ExecutionWorkbookStatus::Draft | ExecutionWorkbookStatus::Validated => {}
        ExecutionWorkbookStatus::Superseded => {
            return Err(refusal(
                DslCoderRefusalCode::WorkbookSuperseded,
                "superseded workbook cannot be dry-run",
            ));
        }
        ExecutionWorkbookStatus::Executed => {
            return Err(refusal(
                DslCoderRefusalCode::WorkbookAlreadyExecuted,
                "executed workbook cannot be dry-run again",
            ));
        }
        ExecutionWorkbookStatus::Rejected => {
            return Err(refusal(
                DslCoderRefusalCode::WorkbookRejected,
                "rejected workbook cannot be dry-run",
            ));
        }
    }

    Ok(DslCoderDryRunResult {
        workbook_id: workbook.id.to_string(),
        transition_ref: workbook.core.transition_ref.clone(),
        semantic_diff: workbook.core.simulation.clone(),
    })
}

fn map_workbook_error(err: ExecutionWorkbookValidationError) -> DslCoderValidationError {
    let code = match err {
        ExecutionWorkbookValidationError::TransitionRefMismatch { .. }
        | ExecutionWorkbookValidationError::SubjectMismatch { .. } => {
            DslCoderRefusalCode::TransitionBindingMismatch
        }
        ExecutionWorkbookValidationError::HashMismatch { .. }
        | ExecutionWorkbookValidationError::RequiredFieldEmpty { .. }
        | ExecutionWorkbookValidationError::MissingEvidenceRefs => {
            DslCoderRefusalCode::WorkbookIntegrityFailed
        }
    };

    refusal(code, format!("{err:?}"))
}

fn refusal(code: DslCoderRefusalCode, message: impl Into<String>) -> DslCoderValidationError {
    DslCoderValidationError {
        code,
        message: message.into(),
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::runbook::workbook::{
        EvidenceRef, ExecutionWorkbook, ExecutionWorkbookCore, ExecutionWorkbookId, LlmTraceRef,
        StaleWorkbookPolicy, WorkbookActor, WorkbookSubject,
    };
    use sem_os_core::state_simulation::{
        SemanticStateDiff, SimulatedStateAdvance, StateSimulationResult,
    };
    use std::collections::BTreeMap;
    use uuid::{uuid, Uuid};

    const SESSION_ID: Uuid = uuid!("aaaaaaaa-aaaa-aaaa-aaaa-aaaaaaaaaaaa");
    const CASE_ID: Uuid = uuid!("11111111-1111-1111-1111-111111111111");
    const TRACE_ID: Uuid = uuid!("22222222-2222-2222-2222-222222222222");

    fn simulation() -> StateSimulationResult {
        StateSimulationResult {
            transition_ref: "kyc-case.intake-to-discovery".to_string(),
            entity_id: CASE_ID,
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
                entity_id: CASE_ID,
                to_node: "kyc-case:discovery".to_string(),
                slot_path: "kyc-case/workstream".to_string(),
                reason: "kyc-case.update-status - INTAKE -> DISCOVERY".to_string(),
                writes_since_push_delta: 1,
            },
            state_snapshot_id: Some("state-snapshot-1".to_string()),
            configuration_version: Some("config-1".to_string()),
        }
    }

    fn workbook() -> ExecutionWorkbook {
        ExecutionWorkbook::new(ExecutionWorkbookCore {
            schema_version: 1,
            pack_id: "ob-poc.kyc".to_string(),
            transition_ref: "kyc-case.intake-to-discovery".to_string(),
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
            evidence_refs: vec![EvidenceRef {
                kind: "case_id".to_string(),
                ref_id: CASE_ID.to_string(),
                digest: "sha256:case".to_string(),
            }],
            llm_trace_ref: Some(LlmTraceRef {
                trace_id: TRACE_ID,
                prompt_hash: "sha256:prompt".to_string(),
                response_hash: "sha256:response".to_string(),
            }),
            simulation: simulation(),
            stale_policy: StaleWorkbookPolicy::Revalidate,
            previous_workbook_id: None,
            metadata: BTreeMap::new(),
        })
        .expect("workbook")
    }

    #[test]
    fn validates_workbook_for_dry_run() {
        let workbook = workbook();

        let result = validate_workbook_for_dry_run(&workbook, DslCoderExecutionMode::DryRun)
            .expect("dry-run accepted");

        assert_eq!(result.workbook_id, workbook.id.to_string());
        assert_eq!(result.transition_ref, "kyc-case.intake-to-discovery");
        assert_eq!(result.semantic_diff.to_state, "DISCOVERY");
    }

    #[test]
    fn refuses_mutation_mode() {
        let err = validate_workbook_for_dry_run(&workbook(), DslCoderExecutionMode::Mutate)
            .expect_err("mutation refused");

        assert_eq!(err.code, DslCoderRefusalCode::MutationNotEnabled);
    }

    #[test]
    fn refuses_hash_mismatch() {
        let mut workbook = workbook();
        workbook.id = ExecutionWorkbookId("ewb:v1:tampered".to_string());

        let err = validate_workbook_for_dry_run(&workbook, DslCoderExecutionMode::DryRun)
            .expect_err("hash mismatch refused");

        assert_eq!(err.code, DslCoderRefusalCode::WorkbookIntegrityFailed);
    }

    #[test]
    fn refuses_superseded_workbook() {
        let mut workbook = workbook();
        workbook.status = ExecutionWorkbookStatus::Superseded;

        let err = validate_workbook_for_dry_run(&workbook, DslCoderExecutionMode::DryRun)
            .expect_err("superseded refused");

        assert_eq!(err.code, DslCoderRefusalCode::WorkbookSuperseded);
    }

    #[test]
    fn refuses_binding_mismatch() {
        let mut workbook = workbook();
        workbook.core.subject.subject_id = uuid!("33333333-3333-3333-3333-333333333333");
        workbook.id = crate::runbook::compute_workbook_id(&workbook.core);

        let err = validate_workbook_for_dry_run(&workbook, DslCoderExecutionMode::DryRun)
            .expect_err("binding mismatch refused");

        assert_eq!(err.code, DslCoderRefusalCode::TransitionBindingMismatch);
    }
}
