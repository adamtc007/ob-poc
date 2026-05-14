//! Restricted mutation preflight.
//!
//! This module prepares the v1.0 mutation gate without executing any state
//! change. A preflight may only succeed after workbook integrity, approval
//! token, transition enablement, replay, and drift checks all pass. The actual
//! mutation remains the responsibility of the existing runbook execution gate.

use chrono::{DateTime, Utc};
use sem_os_policy::domain_pack::DomainPackManifest;
use sem_os_policy::state_simulation::StateSimulationResult;
use serde::{Deserialize, Serialize};
use std::collections::{BTreeMap, BTreeSet};

use super::approval_token::{
    validate_restricted_mutation_approval, ApprovalTokenId, ApprovalTokenValidationError,
    MutationApprovalToken, ObservedMutationAnchors, RestrictedMutationApprovalCheck,
};
use super::workbook::{ExecutionWorkbook, ExecutionWorkbookId};

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct RestrictedMutationPreflight {
    pub workbook_id: ExecutionWorkbookId,
    pub approval: RestrictedMutationApprovalCheck,
    pub verb: String,
    pub transition_ref: String,
    pub intended_diff: MutationSemanticDiff,
    pub predicted_diff: StateSimulationResult,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub actual_diff: Option<MutationSemanticDiff>,
    pub executor: MutationExecutor,
    #[serde(default)]
    pub runbook_args: BTreeMap<String, String>,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct MutationSemanticDiff {
    pub subject_id: uuid::Uuid,
    pub field: String,
    pub before: String,
    pub after: String,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum MutationExecutor {
    ExistingRunbookGateOnly,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(tag = "kind", rename_all = "snake_case")]
pub enum RestrictedMutationPreflightError {
    ApprovalRefused {
        error: ApprovalTokenValidationError,
    },
    PredictedDiffMismatch {
        intended: Box<MutationSemanticDiff>,
        predicted: Box<MutationSemanticDiff>,
    },
}

pub fn prepare_restricted_mutation_preflight(
    workbook: &ExecutionWorkbook,
    token: Option<&MutationApprovalToken>,
    manifest: &DomainPackManifest,
    observed: &ObservedMutationAnchors,
    consumed_token_ids: &BTreeSet<ApprovalTokenId>,
    checked_at: DateTime<Utc>,
) -> Result<RestrictedMutationPreflight, RestrictedMutationPreflightError> {
    let approval = validate_restricted_mutation_approval(
        workbook,
        token,
        manifest,
        observed,
        consumed_token_ids,
        checked_at,
    )
    .map_err(|error| RestrictedMutationPreflightError::ApprovalRefused { error })?;

    let intended_diff = MutationSemanticDiff {
        subject_id: workbook.core.subject.subject_id,
        field: workbook.core.simulation.semantic_diff.field.clone(),
        before: workbook.core.simulation.from_state.clone(),
        after: workbook.core.simulation.to_state.clone(),
    };
    let predicted_diff = MutationSemanticDiff {
        subject_id: workbook.core.simulation.entity_id,
        field: workbook.core.simulation.semantic_diff.field.clone(),
        before: workbook.core.simulation.semantic_diff.before.clone(),
        after: workbook.core.simulation.semantic_diff.after.clone(),
    };

    if intended_diff != predicted_diff {
        return Err(RestrictedMutationPreflightError::PredictedDiffMismatch {
            intended: Box::new(intended_diff),
            predicted: Box::new(predicted_diff),
        });
    }

    let mut runbook_args = BTreeMap::new();
    runbook_args.insert(
        "case-id".to_string(),
        workbook.core.subject.subject_id.to_string(),
    );
    runbook_args.insert(
        "from-state".to_string(),
        workbook.core.simulation.from_state.clone(),
    );
    runbook_args.insert(
        "to-state".to_string(),
        workbook.core.simulation.to_state.clone(),
    );
    runbook_args.insert(
        "status".to_string(),
        workbook.core.simulation.to_state.clone(),
    );
    runbook_args.insert("workbook-id".to_string(), workbook.id.to_string());
    runbook_args.insert(
        "approval-token-id".to_string(),
        approval.approval_token_id.to_string(),
    );

    Ok(RestrictedMutationPreflight {
        workbook_id: workbook.id.clone(),
        approval,
        verb: workbook.core.simulation.verb.clone(),
        transition_ref: workbook.core.transition_ref.clone(),
        intended_diff,
        predicted_diff: workbook.core.simulation.clone(),
        actual_diff: None,
        executor: MutationExecutor::ExistingRunbookGateOnly,
        runbook_args,
    })
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::approval_token::create_approval_token_for_workbook;
    use crate::workbook::{
        EvidenceRef, ExecutionWorkbook, ExecutionWorkbookCore, StaleWorkbookPolicy, WorkbookActor,
        WorkbookExecutionMode, WorkbookSubject,
    };
    use chrono::TimeZone;
    use sem_os_policy::domain_pack::{
        ClassificationLimit, ContextClassificationPolicy, DomainPackManifest, DomainTransition,
        PackCompatibilityTier, PackImplementationMode,
    };
    use sem_os_policy::state_simulation::{
        SemanticStateDiff, SimulatedStateAdvance, StateSimulationResult,
    };
    use uuid::{uuid, Uuid};

    const SESSION_ID: Uuid = uuid!("aaaaaaaa-aaaa-aaaa-aaaa-aaaaaaaaaaaa");
    const CASE_ID: Uuid = uuid!("11111111-1111-1111-1111-111111111111");

    fn now() -> DateTime<Utc> {
        Utc.with_ymd_and_hms(2026, 5, 5, 12, 0, 0).unwrap()
    }

    fn expires_at() -> DateTime<Utc> {
        Utc.with_ymd_and_hms(2026, 5, 5, 13, 0, 0).unwrap()
    }

    fn simulation() -> StateSimulationResult {
        StateSimulationResult {
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
        }
    }

    fn workbook() -> ExecutionWorkbook {
        ExecutionWorkbook::new(ExecutionWorkbookCore {
            schema_version: 1,
            pack_id: "ob-poc.kyc".to_string(),
            transition_ref: "kyc-case.discovery-to-assessment".to_string(),
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
            state_snapshot_id: "snapshot-1".to_string(),
            objective: "Move KYC case from discovery to assessment".to_string(),
            user_prompt_ref: None,
            editor_context_refs: vec![],
            evidence_refs: vec![EvidenceRef {
                kind: "case_id".to_string(),
                ref_id: CASE_ID.to_string(),
                digest: "sha256:evidence".to_string(),
                source_system: None,
                field_path: None,
                classification: None,
            }],
            llm_trace_ref: None,
            expected_preconditions: vec![],
            expected_postconditions: vec![],
            invariant_checks: vec![],
            governance_checks: vec![],
            simulation: simulation(),
            stale_policy: StaleWorkbookPolicy::Revalidate,
            previous_workbook_id: None,
            metadata: Default::default(),
        })
        .expect("workbook")
    }

    fn manifest(mutation_enabled: bool) -> DomainPackManifest {
        DomainPackManifest {
            pack_id: "ob-poc.kyc".to_string(),
            name: "KYC".to_string(),
            version: "1.0.0".to_string(),
            implementation_mode: PackImplementationMode::NativeCompiled,
            compatibility_tier: PackCompatibilityTier::ReferenceMutation,
            owned_constellations: vec!["kyc.onboarding".to_string()],
            allowed_transitions: vec![DomainTransition {
                transition_ref: "kyc-case.discovery-to-assessment".to_string(),
                entity_type: "kyc_case".to_string(),
                state_machine: "kyc_case_lifecycle".to_string(),
                verb: "kyc-case.update-status".to_string(),
                from_state: "DISCOVERY".to_string(),
                to_state: "ASSESSMENT".to_string(),
                dry_run_enabled: true,
                mutation_enabled,
                hitl_required: true,
                evidence_refs_required: vec!["case_id".to_string()],
            }],
            discovery_probes: vec![],
            projection_catalog: vec![],
            mention_namespaces: vec![],
            declared_modes: vec![],
            workflow_phases: vec![],
            acp_personas: vec![],
            resource_uri_schemes: vec![],
            external_mcp_transports: vec![],
            typed_extension_points: vec![],
            classification_policy: ContextClassificationPolicy {
                max_prompt_classification: ClassificationLimit::Internal,
                allow_external_llm: false,
                required_redactions: vec![],
            },
        }
    }

    fn observed(workbook: &ExecutionWorkbook) -> ObservedMutationAnchors {
        ObservedMutationAnchors {
            configuration_version: workbook.core.configuration_version.clone(),
            state_snapshot_id: workbook.core.state_snapshot_id.clone(),
            evidence_refs: workbook.core.evidence_refs.clone(),
        }
    }

    #[test]
    fn prepares_runbook_gate_only_mutation_preflight_after_approval() {
        let workbook = workbook();
        let token = create_approval_token_for_workbook(
            &workbook,
            "approver@example.com",
            "Approved for restricted KYC mutation",
            expires_at(),
            now(),
        )
        .expect("token");

        let preflight = prepare_restricted_mutation_preflight(
            &workbook,
            Some(&token),
            &manifest(true),
            &observed(&workbook),
            &BTreeSet::new(),
            now(),
        )
        .expect("preflight");

        assert_eq!(
            preflight.executor,
            MutationExecutor::ExistingRunbookGateOnly
        );
        assert_eq!(preflight.actual_diff, None);
        assert_eq!(preflight.verb, "kyc-case.update-status");
        assert_eq!(preflight.intended_diff.after, "ASSESSMENT");
        assert_eq!(
            preflight.runbook_args.get("approval-token-id"),
            Some(&token.id.to_string())
        );
    }

    #[test]
    fn refuses_preflight_without_approval() {
        let workbook = workbook();

        let err = prepare_restricted_mutation_preflight(
            &workbook,
            None,
            &manifest(true),
            &observed(&workbook),
            &BTreeSet::new(),
            now(),
        )
        .expect_err("missing token refused");

        assert!(matches!(
            err,
            RestrictedMutationPreflightError::ApprovalRefused {
                error: ApprovalTokenValidationError::MissingApprovalToken
            }
        ));
    }

    #[test]
    fn refuses_preflight_for_dry_run_only_transition() {
        let workbook = workbook();
        let token = create_approval_token_for_workbook(
            &workbook,
            "approver@example.com",
            "Approved for restricted KYC mutation",
            expires_at(),
            now(),
        )
        .expect("token");

        let err = prepare_restricted_mutation_preflight(
            &workbook,
            Some(&token),
            &manifest(false),
            &observed(&workbook),
            &BTreeSet::new(),
            now(),
        )
        .expect_err("dry-run-only transition refused");

        assert!(matches!(
            err,
            RestrictedMutationPreflightError::ApprovalRefused {
                error: ApprovalTokenValidationError::TransitionMutationNotEnabled { .. }
            }
        ));
    }

    #[test]
    fn refuses_preflight_when_predicted_diff_does_not_match_intent() {
        let mut workbook = workbook();
        workbook.core.simulation.semantic_diff.after = "REVIEW".to_string();
        workbook.id = crate::workbook::compute_workbook_id(&workbook.core);
        let token = create_approval_token_for_workbook(
            &workbook,
            "approver@example.com",
            "Approved for restricted KYC mutation",
            expires_at(),
            now(),
        )
        .expect("token");

        let err = prepare_restricted_mutation_preflight(
            &workbook,
            Some(&token),
            &manifest(true),
            &observed(&workbook),
            &BTreeSet::new(),
            now(),
        )
        .expect_err("diff mismatch refused");

        assert!(matches!(
            err,
            RestrictedMutationPreflightError::PredictedDiffMismatch { .. }
        ));
    }
}
