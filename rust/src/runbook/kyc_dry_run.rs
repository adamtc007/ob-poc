//! KYC dry-run workbook builder.
//!
//! This is the first narrow integration path for the configuration-native
//! toolkit: load the ob-poc KYC Domain Pack, simulate the approved transition,
//! bind the result into an Execution Workbook, and pass it through the DSL
//! Coder dry-run validator. It performs no storage and no mutation.

use sem_os_core::domain_pack::{DomainPackDiagnostic, DomainPackManifest};
use sem_os_core::state_simulation::{
    simulate_transition_from_pack, StateSimulationError, StateSimulationRequest,
};
use serde::{Deserialize, Serialize};
use std::collections::BTreeMap;
use uuid::Uuid;

use super::dsl_coder::{
    validate_workbook_for_dry_run, DslCoderDryRunResult, DslCoderExecutionMode,
    DslCoderValidationError,
};
use super::workbook::{
    EvidenceRef, ExecutionWorkbook, ExecutionWorkbookCore, LlmTraceRef, StaleWorkbookPolicy,
    WorkbookActor, WorkbookSubject,
};

const OB_POC_KYC_PACK: &str =
    include_str!("../../config/sem_os_seeds/domain_packs/ob_poc_kyc.yaml");

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct KycUpdateStatusDryRunInput {
    pub session_id: Uuid,
    pub case_id: Uuid,
    pub actor_id: String,
    #[serde(default)]
    pub actor_roles: Vec<String>,
    pub transition_ref: String,
    pub current_state: String,
    pub requested_state: String,
    pub configuration_version: String,
    pub state_snapshot_id: String,
    pub evidence_digest: String,
    #[serde(default)]
    pub llm_trace_ref: Option<LlmTraceRef>,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct KycUpdateStatusDryRunOutput {
    pub workbook: ExecutionWorkbook,
    pub dry_run: DslCoderDryRunResult,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(tag = "kind", rename_all = "snake_case")]
pub enum KycUpdateStatusDryRunRefusal {
    PackParseFailed {
        reason: String,
    },
    PackInvalid {
        diagnostics: Vec<DomainPackDiagnostic>,
    },
    SimulationRefused {
        error: StateSimulationError,
    },
    WorkbookRefused {
        reason: String,
    },
    DslCoderRefused {
        error: DslCoderValidationError,
    },
}

pub fn build_kyc_update_status_dry_run(
    input: KycUpdateStatusDryRunInput,
) -> Result<KycUpdateStatusDryRunOutput, KycUpdateStatusDryRunRefusal> {
    let manifest: DomainPackManifest = serde_yaml::from_str(OB_POC_KYC_PACK).map_err(|err| {
        KycUpdateStatusDryRunRefusal::PackParseFailed {
            reason: err.to_string(),
        }
    })?;

    let validation = manifest.validate();
    if !validation.valid {
        return Err(KycUpdateStatusDryRunRefusal::PackInvalid {
            diagnostics: validation.diagnostics,
        });
    }

    let simulation = simulate_transition_from_pack(
        &manifest,
        &StateSimulationRequest {
            pack_id: manifest.pack_id.clone(),
            transition_ref: input.transition_ref.clone(),
            entity_id: input.case_id,
            entity_type: "kyc_case".to_string(),
            state_machine: "kyc_case_lifecycle".to_string(),
            current_state: input.current_state.clone(),
            requested_state: input.requested_state.clone(),
            state_snapshot_id: Some(input.state_snapshot_id.clone()),
            configuration_version: Some(input.configuration_version.clone()),
        },
    )
    .map_err(|error| KycUpdateStatusDryRunRefusal::SimulationRefused { error })?;

    let workbook = ExecutionWorkbook::new(ExecutionWorkbookCore {
        schema_version: 1,
        pack_id: manifest.pack_id,
        transition_ref: input.transition_ref,
        session_id: input.session_id,
        subject: WorkbookSubject {
            subject_kind: "kyc_case".to_string(),
            subject_id: input.case_id,
        },
        actor: WorkbookActor {
            actor_id: input.actor_id,
            roles: input.actor_roles,
        },
        configuration_version: input.configuration_version,
        state_snapshot_id: input.state_snapshot_id,
        evidence_refs: vec![EvidenceRef {
            kind: "case_id".to_string(),
            ref_id: input.case_id.to_string(),
            digest: input.evidence_digest,
        }],
        llm_trace_ref: input.llm_trace_ref,
        simulation,
        stale_policy: StaleWorkbookPolicy::Revalidate,
        previous_workbook_id: None,
        metadata: BTreeMap::new(),
    })
    .map_err(|err| KycUpdateStatusDryRunRefusal::WorkbookRefused {
        reason: format!("{err:?}"),
    })?;

    let dry_run = validate_workbook_for_dry_run(&workbook, DslCoderExecutionMode::DryRun)
        .map_err(|error| KycUpdateStatusDryRunRefusal::DslCoderRefused { error })?;

    Ok(KycUpdateStatusDryRunOutput { workbook, dry_run })
}

#[cfg(test)]
mod tests {
    use super::*;
    use uuid::uuid;

    const SESSION_ID: Uuid = uuid!("aaaaaaaa-aaaa-aaaa-aaaa-aaaaaaaaaaaa");
    const CASE_ID: Uuid = uuid!("11111111-1111-1111-1111-111111111111");

    fn input(current_state: &str, requested_state: &str) -> KycUpdateStatusDryRunInput {
        KycUpdateStatusDryRunInput {
            session_id: SESSION_ID,
            case_id: CASE_ID,
            actor_id: "analyst@example.com".to_string(),
            actor_roles: vec!["analyst".to_string()],
            transition_ref: "kyc-case.intake-to-discovery".to_string(),
            current_state: current_state.to_string(),
            requested_state: requested_state.to_string(),
            configuration_version: "config-1".to_string(),
            state_snapshot_id: "state-snapshot-1".to_string(),
            evidence_digest: "sha256:case".to_string(),
            llm_trace_ref: None,
        }
    }

    #[test]
    fn builds_validated_dry_run_workbook_for_kyc_update_status() {
        let output =
            build_kyc_update_status_dry_run(input("INTAKE", "DISCOVERY")).expect("dry-run built");

        assert_eq!(
            output.workbook.core.transition_ref,
            "kyc-case.intake-to-discovery"
        );
        assert_eq!(output.workbook.core.subject.subject_id, CASE_ID);
        assert_eq!(output.dry_run.semantic_diff.from_state, "INTAKE");
        assert_eq!(output.dry_run.semantic_diff.to_state, "DISCOVERY");
        assert_eq!(
            output.dry_run.semantic_diff.predicted_advance.to_node,
            "kyc-case:discovery"
        );
    }

    #[test]
    fn refuses_illegal_kyc_transition() {
        let err = build_kyc_update_status_dry_run(input("REVIEW", "DISCOVERY"))
            .expect_err("transition refused");

        assert!(matches!(
            err,
            KycUpdateStatusDryRunRefusal::SimulationRefused {
                error: StateSimulationError::CurrentStateMismatch { .. }
            }
        ));
    }

    #[test]
    fn refuses_unknown_transition_ref() {
        let mut input = input("INTAKE", "DISCOVERY");
        input.transition_ref = "kyc-case.review-to-approved".to_string();

        let err = build_kyc_update_status_dry_run(input).expect_err("transition refused");

        assert!(matches!(
            err,
            KycUpdateStatusDryRunRefusal::SimulationRefused {
                error: StateSimulationError::UnknownTransition { .. }
            }
        ));
    }
}
