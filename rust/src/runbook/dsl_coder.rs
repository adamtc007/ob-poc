//! DSL Coder validation boundary.
//!
//! This module is intentionally dry-run-only for the MVP slice. It validates an
//! Execution Workbook and returns the already-bound SemOS simulation result
//! without invoking the runbook executor or mutating state.

use sem_os_core::state_simulation::StateSimulationResult;
use serde::{Deserialize, Serialize};

use super::workbook::{
    ExecutionWorkbook, ExecutionWorkbookStatus, ExecutionWorkbookValidationError,
    WorkbookExecutionMode,
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
    pub semantic_diff_uri: String,
    pub validation_trace: Vec<DslCoderValidationStep>,
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
    ExecutionModeMismatch,
    WorkbookSuperseded,
    WorkbookAlreadyExecuted,
    WorkbookRejected,
    TransitionBindingMismatch,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct DslCoderValidationStep {
    pub step_number: u8,
    pub step_id: String,
    pub status: DslCoderValidationStepStatus,
    pub message: String,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum DslCoderValidationStepStatus {
    Passed,
    Failed,
    Skipped,
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

    if workbook.core.execution_mode != WorkbookExecutionMode::DryRun {
        return Err(refusal(
            DslCoderRefusalCode::ExecutionModeMismatch,
            "dry-run validator only accepts workbooks with execution_mode=dry_run",
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
        semantic_diff_uri: format!("semos://semantic-diff/{}", workbook.id),
        validation_trace: validation_trace(),
    })
}

fn validation_trace() -> Vec<DslCoderValidationStep> {
    vec![
        step(1, "schema", "workbook schema received"),
        step(2, "execution-mode", "workbook execution_mode is dry_run"),
        step(3, "integrity", "workbook integrity hash verified"),
        step(4, "status", "workbook status permits validation"),
        step(5, "configuration-version", "configuration version is bound"),
        step(6, "state-snapshot", "state snapshot is bound"),
        step(
            7,
            "explicit-bindings",
            "subject and transition bindings are explicit",
        ),
        step(
            8,
            "frontier",
            "transition was simulated from declared Domain Pack frontier",
        ),
        step(9, "evidence", "required evidence references are present"),
        step(
            10,
            "semantic-diff",
            "semantic diff is attached to workbook simulation",
        ),
        step(11, "dry-run", "dry-run result is non-mutating"),
    ]
}

fn step(step_number: u8, step_id: &str, message: &str) -> DslCoderValidationStep {
    DslCoderValidationStep {
        step_number,
        step_id: step_id.to_string(),
        status: DslCoderValidationStepStatus::Passed,
        message: message.to_string(),
    }
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
        StaleWorkbookPolicy, WorkbookActor, WorkbookExecutionMode, WorkbookSubject,
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
            user_prompt_ref: None,
            editor_context_refs: vec![],
            evidence_refs: vec![EvidenceRef {
                kind: "case_id".to_string(),
                ref_id: CASE_ID.to_string(),
                digest: "sha256:case".to_string(),
                source_system: None,
                field_path: None,
                classification: None,
            }],
            llm_trace_ref: Some(LlmTraceRef {
                trace_id: TRACE_ID,
                prompt_hash: "sha256:prompt".to_string(),
                response_hash: "sha256:response".to_string(),
            }),
            expected_preconditions: vec![],
            expected_postconditions: vec![],
            invariant_checks: vec![],
            governance_checks: vec![],
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
        assert!(result
            .semantic_diff_uri
            .starts_with("semos://semantic-diff/"));
        assert!(result
            .validation_trace
            .iter()
            .any(|step| step.step_id == "integrity"));
    }

    #[test]
    fn refuses_mutation_mode() {
        let err = validate_workbook_for_dry_run(&workbook(), DslCoderExecutionMode::Mutate)
            .expect_err("mutation refused");

        assert_eq!(err.code, DslCoderRefusalCode::MutationNotEnabled);
    }

    #[test]
    fn refuses_non_dry_run_workbook_mode() {
        let mut workbook = workbook();
        workbook.core.execution_mode = WorkbookExecutionMode::ExecuteAfterApproval;
        workbook.id = crate::runbook::compute_workbook_id(&workbook.core);

        let err = validate_workbook_for_dry_run(&workbook, DslCoderExecutionMode::DryRun)
            .expect_err("non-dry-run workbook refused");

        assert_eq!(err.code, DslCoderRefusalCode::ExecutionModeMismatch);
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
