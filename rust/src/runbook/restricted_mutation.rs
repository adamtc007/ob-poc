//! Restricted mutation compilation.
//!
//! This module turns an approved mutation preflight into the existing immutable
//! `CompiledRunbook` artifact. It does not execute the runbook; execution still
//! requires `execute_runbook()` with the returned `CompiledRunbookId`.

use serde::{Deserialize, Serialize};
use std::collections::BTreeMap;
use uuid::Uuid;

use super::envelope::{EnvelopeCore, ReplayEnvelope};
use super::mutation_preflight::{
    MutationExecutor, MutationSemanticDiff, RestrictedMutationPreflight,
};
use super::types::{CompiledRunbook, CompiledRunbookId, CompiledStep, ExecutionMode};

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct RestrictedMutationRunbookCompilation {
    pub compiled_runbook_id: CompiledRunbookId,
    pub workbook_id: super::workbook::ExecutionWorkbookId,
    pub approval_token_id: super::approval_token::ApprovalTokenId,
    pub transition_ref: String,
    pub expected_diff: MutationSemanticDiff,
    pub compiled_runbook: CompiledRunbook,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct RestrictedMutationExecutionReceipt {
    pub compiled_runbook_id: CompiledRunbookId,
    pub workbook_id: super::workbook::ExecutionWorkbookId,
    pub approval_token_id: super::approval_token::ApprovalTokenId,
    pub transition_ref: String,
    pub intended_diff: MutationSemanticDiff,
    pub predicted_diff: sem_os_policy::state_simulation::StateSimulationResult,
    pub actual_diff: MutationSemanticDiff,
    pub executed_at: chrono::DateTime<chrono::Utc>,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(tag = "kind", rename_all = "snake_case")]
pub enum RestrictedMutationRunbookCompilationError {
    UnsupportedExecutor {
        executor: MutationExecutor,
    },
    UnsupportedVerb {
        verb: String,
    },
    AlreadyExecuted {
        actual_diff: MutationSemanticDiff,
    },
    ArgMismatch {
        field: String,
        expected: String,
        actual: Option<String>,
    },
    ReceiptBindingMismatch {
        field: String,
        expected: String,
        actual: String,
    },
    ActualDiffMismatch {
        expected: MutationSemanticDiff,
        actual: MutationSemanticDiff,
    },
}

#[allow(clippy::result_large_err)]
pub fn compile_restricted_mutation_preflight(
    session_id: Uuid,
    runbook_version: u64,
    preflight: &RestrictedMutationPreflight,
) -> Result<RestrictedMutationRunbookCompilation, RestrictedMutationRunbookCompilationError> {
    if preflight.executor != MutationExecutor::ExistingRunbookGateOnly {
        return Err(
            RestrictedMutationRunbookCompilationError::UnsupportedExecutor {
                executor: preflight.executor,
            },
        );
    }

    if preflight.verb != "kyc-case.update-status" {
        return Err(RestrictedMutationRunbookCompilationError::UnsupportedVerb {
            verb: preflight.verb.clone(),
        });
    }

    if let Some(actual_diff) = &preflight.actual_diff {
        return Err(RestrictedMutationRunbookCompilationError::AlreadyExecuted {
            actual_diff: actual_diff.clone(),
        });
    }

    require_arg(
        preflight,
        "case-id",
        &preflight.intended_diff.subject_id.to_string(),
    )?;
    require_arg(preflight, "status", &preflight.intended_diff.after)?;
    require_arg(preflight, "to-state", &preflight.intended_diff.after)?;
    require_arg(preflight, "from-state", &preflight.intended_diff.before)?;
    require_arg(preflight, "workbook-id", &preflight.workbook_id.to_string())?;
    require_arg(
        preflight,
        "approval-token-id",
        &preflight.approval.approval_token_id.to_string(),
    )?;

    let mut execution_args = BTreeMap::new();
    execution_args.insert(
        "case-id".to_string(),
        preflight.intended_diff.subject_id.to_string(),
    );
    execution_args.insert("status".to_string(), preflight.intended_diff.after.clone());

    let dsl = build_dsl(&preflight.verb, &execution_args);
    let step = CompiledStep {
        step_id: Uuid::new_v4(),
        sentence: format!(
            "Apply approved KYC case status update from {} to {}",
            preflight.intended_diff.before, preflight.intended_diff.after
        ),
        verb: preflight.verb.clone(),
        dsl,
        args: execution_args,
        depends_on: vec![],
        execution_mode: ExecutionMode::Sync,
        write_set: vec![preflight.intended_diff.subject_id],
        verb_contract_snapshot_id: None,
    };

    let mut entity_bindings = BTreeMap::new();
    entity_bindings.insert("case-id".to_string(), preflight.intended_diff.subject_id);

    let mut snapshot_manifest = BTreeMap::new();
    if let Ok(snapshot_id) = Uuid::parse_str(
        &preflight
            .predicted_diff
            .state_snapshot_id
            .clone()
            .unwrap_or_default(),
    ) {
        snapshot_manifest.insert(preflight.intended_diff.subject_id, snapshot_id);
    }

    let envelope = ReplayEnvelope {
        core: EnvelopeCore {
            session_cursor: runbook_version,
            entity_bindings,
            external_lookup_digests: vec![
                format!("workbook:{}", preflight.workbook_id),
                format!("approval-token:{}", preflight.approval.approval_token_id),
                format!("transition:{}", preflight.transition_ref),
            ],
            macro_audit_digests: vec![],
            snapshot_manifest,
        },
        external_lookups: vec![],
        macro_audits: vec![],
        sealed_at: chrono::Utc::now(),
    };

    let compiled_runbook = CompiledRunbook::new(session_id, runbook_version, vec![step], envelope);

    Ok(RestrictedMutationRunbookCompilation {
        compiled_runbook_id: compiled_runbook.id,
        workbook_id: preflight.workbook_id.clone(),
        approval_token_id: preflight.approval.approval_token_id.clone(),
        transition_ref: preflight.transition_ref.clone(),
        expected_diff: preflight.intended_diff.clone(),
        compiled_runbook,
    })
}

#[allow(clippy::result_large_err)]
pub fn record_restricted_mutation_execution_receipt(
    compilation: &RestrictedMutationRunbookCompilation,
    preflight: &RestrictedMutationPreflight,
    actual_diff: MutationSemanticDiff,
    executed_at: chrono::DateTime<chrono::Utc>,
) -> Result<RestrictedMutationExecutionReceipt, RestrictedMutationRunbookCompilationError> {
    require_receipt_binding(
        "workbook_id",
        &compilation.workbook_id.to_string(),
        &preflight.workbook_id.to_string(),
    )?;
    require_receipt_binding(
        "approval_token_id",
        &compilation.approval_token_id.to_string(),
        &preflight.approval.approval_token_id.to_string(),
    )?;
    require_receipt_binding(
        "transition_ref",
        &compilation.transition_ref,
        &preflight.transition_ref,
    )?;
    if compilation.expected_diff != preflight.intended_diff {
        return Err(
            RestrictedMutationRunbookCompilationError::ReceiptBindingMismatch {
                field: "intended_diff".to_string(),
                expected: format!("{:?}", compilation.expected_diff),
                actual: format!("{:?}", preflight.intended_diff),
            },
        );
    }
    if actual_diff != compilation.expected_diff {
        return Err(
            RestrictedMutationRunbookCompilationError::ActualDiffMismatch {
                expected: compilation.expected_diff.clone(),
                actual: actual_diff,
            },
        );
    }

    Ok(RestrictedMutationExecutionReceipt {
        compiled_runbook_id: compilation.compiled_runbook_id,
        workbook_id: compilation.workbook_id.clone(),
        approval_token_id: compilation.approval_token_id.clone(),
        transition_ref: compilation.transition_ref.clone(),
        intended_diff: preflight.intended_diff.clone(),
        predicted_diff: preflight.predicted_diff.clone(),
        actual_diff,
        executed_at,
    })
}

#[allow(clippy::result_large_err)]
fn require_arg(
    preflight: &RestrictedMutationPreflight,
    field: &str,
    expected: &str,
) -> Result<(), RestrictedMutationRunbookCompilationError> {
    let actual = preflight.runbook_args.get(field).cloned();
    if actual.as_deref() != Some(expected) {
        return Err(RestrictedMutationRunbookCompilationError::ArgMismatch {
            field: field.to_string(),
            expected: expected.to_string(),
            actual,
        });
    }
    Ok(())
}

#[allow(clippy::result_large_err)]
fn require_receipt_binding(
    field: &str,
    expected: &str,
    actual: &str,
) -> Result<(), RestrictedMutationRunbookCompilationError> {
    if expected != actual {
        return Err(
            RestrictedMutationRunbookCompilationError::ReceiptBindingMismatch {
                field: field.to_string(),
                expected: expected.to_string(),
                actual: actual.to_string(),
            },
        );
    }
    Ok(())
}

fn build_dsl(verb: &str, args: &BTreeMap<String, String>) -> String {
    let rendered_args = args
        .iter()
        .map(|(key, value)| format!(" :{} \"{}\"", key, escape_dsl_string(value)))
        .collect::<String>();
    format!("({verb}{rendered_args})")
}

fn escape_dsl_string(value: &str) -> String {
    value.replace('\\', "\\\\").replace('"', "\\\"")
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::runbook::mutation_preflight::RestrictedMutationPreflight;
    use crate::runbook::{
        ApprovalTokenId, ExecutionWorkbookId, MutationSemanticDiff, RestrictedMutationApprovalCheck,
    };
    use sem_os_policy::state_simulation::{
        SemanticStateDiff, SimulatedStateAdvance, StateSimulationResult,
    };
    use uuid::uuid;

    const SESSION_ID: Uuid = uuid!("aaaaaaaa-aaaa-aaaa-aaaa-aaaaaaaaaaaa");
    const CASE_ID: Uuid = uuid!("11111111-1111-1111-1111-111111111111");

    fn preflight() -> RestrictedMutationPreflight {
        let workbook_id = ExecutionWorkbookId(
            "wb_3b5d5c3712955042212316173ccf37be98048ca0eda6b913fc0e4f87c894528c".to_string(),
        );
        let approval_token_id = ApprovalTokenId(
            "approval_27f45bb6794b06ad9f1c787404f172b13e76950a5d217c49c4dacb3d6504bfc2".to_string(),
        );
        let mut runbook_args = BTreeMap::new();
        runbook_args.insert("case-id".to_string(), CASE_ID.to_string());
        runbook_args.insert("from-state".to_string(), "DISCOVERY".to_string());
        runbook_args.insert("to-state".to_string(), "ASSESSMENT".to_string());
        runbook_args.insert("status".to_string(), "ASSESSMENT".to_string());
        runbook_args.insert("workbook-id".to_string(), workbook_id.to_string());
        runbook_args.insert(
            "approval-token-id".to_string(),
            approval_token_id.to_string(),
        );

        RestrictedMutationPreflight {
            workbook_id: workbook_id.clone(),
            approval: RestrictedMutationApprovalCheck {
                workbook_id: workbook_id.clone(),
                approval_token_id,
                transition_ref: "kyc-case.discovery-to-assessment".to_string(),
                approved_by_actor_id: "approver@example.com".to_string(),
                expires_at: chrono::Utc::now() + chrono::Duration::hours(1),
            },
            verb: "kyc-case.update-status".to_string(),
            transition_ref: "kyc-case.discovery-to-assessment".to_string(),
            intended_diff: MutationSemanticDiff {
                subject_id: CASE_ID,
                field: "status".to_string(),
                before: "DISCOVERY".to_string(),
                after: "ASSESSMENT".to_string(),
            },
            predicted_diff: StateSimulationResult {
                transition_ref: "kyc-case.discovery-to-assessment".to_string(),
                entity_id: CASE_ID,
                entity_type: "kyc_case".to_string(),
                state_machine: "kyc_case_lifecycle".to_string(),
                from_state: "DISCOVERY".to_string(),
                to_state: "ASSESSMENT".to_string(),
                verb: "kyc-case.update-status".to_string(),
                semantic_diff: SemanticStateDiff {
                    field: "status".to_string(),
                    before: "DISCOVERY".to_string(),
                    after: "ASSESSMENT".to_string(),
                },
                predicted_advance: SimulatedStateAdvance {
                    entity_id: CASE_ID,
                    to_node: "ASSESSMENT".to_string(),
                    slot_path: "kyc-case/workstream".to_string(),
                    reason: "configuration transition".to_string(),
                    writes_since_push_delta: 0,
                },
                state_snapshot_id: Some("snapshot-1".to_string()),
                configuration_version: Some("config-1".to_string()),
            },
            actual_diff: None,
            executor: MutationExecutor::ExistingRunbookGateOnly,
            runbook_args,
        }
    }

    #[test]
    fn compiles_preflight_to_existing_runbook_gate_artifact() {
        let compiled = compile_restricted_mutation_preflight(SESSION_ID, 7, &preflight()).unwrap();

        assert_eq!(compiled.compiled_runbook.session_id, SESSION_ID);
        assert_eq!(compiled.compiled_runbook.version, 7);
        assert_eq!(compiled.compiled_runbook.step_count(), 1);

        let step = &compiled.compiled_runbook.steps[0];
        assert_eq!(step.verb, "kyc-case.update-status");
        assert_eq!(step.args.get("case-id").unwrap(), &CASE_ID.to_string());
        assert_eq!(step.args.get("status").unwrap(), "ASSESSMENT");
        assert_eq!(
            step.dsl,
            format!(
                "(kyc-case.update-status :case-id \"{}\" :status \"ASSESSMENT\")",
                CASE_ID
            )
        );
        assert_eq!(step.write_set, vec![CASE_ID]);
        assert!(compiled.compiled_runbook.is_executable());
    }

    #[test]
    fn refuses_preflight_with_mismatched_execution_args() {
        let mut preflight = preflight();
        preflight
            .runbook_args
            .insert("status".to_string(), "APPROVED".to_string());

        let err = compile_restricted_mutation_preflight(SESSION_ID, 7, &preflight).unwrap_err();

        assert!(matches!(
            err,
            RestrictedMutationRunbookCompilationError::ArgMismatch { field, .. }
                if field == "status"
        ));
    }

    #[test]
    fn refuses_preflight_that_already_has_actual_diff() {
        let mut preflight = preflight();
        preflight.actual_diff = Some(preflight.intended_diff.clone());

        let err = compile_restricted_mutation_preflight(SESSION_ID, 7, &preflight).unwrap_err();

        assert!(matches!(
            err,
            RestrictedMutationRunbookCompilationError::AlreadyExecuted { .. }
        ));
    }

    #[test]
    fn records_execution_receipt_when_actual_diff_matches_intended_and_predicted() {
        let preflight = preflight();
        let compilation = compile_restricted_mutation_preflight(SESSION_ID, 7, &preflight).unwrap();

        let receipt = record_restricted_mutation_execution_receipt(
            &compilation,
            &preflight,
            preflight.intended_diff.clone(),
            chrono::Utc::now(),
        )
        .unwrap();

        assert_eq!(receipt.compiled_runbook_id, compilation.compiled_runbook_id);
        assert_eq!(receipt.workbook_id, preflight.workbook_id);
        assert_eq!(
            receipt.approval_token_id,
            preflight.approval.approval_token_id
        );
        assert_eq!(receipt.intended_diff, preflight.intended_diff);
        assert_eq!(receipt.predicted_diff, preflight.predicted_diff);
        assert_eq!(receipt.actual_diff, preflight.intended_diff);
    }

    #[test]
    fn refuses_execution_receipt_when_actual_diff_does_not_match_expected_diff() {
        let preflight = preflight();
        let compilation = compile_restricted_mutation_preflight(SESSION_ID, 7, &preflight).unwrap();
        let mut actual_diff = preflight.intended_diff.clone();
        actual_diff.after = "APPROVED".to_string();

        let err = record_restricted_mutation_execution_receipt(
            &compilation,
            &preflight,
            actual_diff,
            chrono::Utc::now(),
        )
        .unwrap_err();

        assert!(matches!(
            err,
            RestrictedMutationRunbookCompilationError::ActualDiffMismatch { .. }
        ));
    }
}
