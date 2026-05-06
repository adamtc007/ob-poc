//! Restricted-mutation approval token contract.
//!
//! The token binds a human approval to the immutable Execution Workbook and the
//! state/configuration/evidence anchors that made the dry-run meaningful. This
//! module does not execute mutations; it only decides whether a future mutation
//! gate has a valid, non-replayed approval.

use chrono::{DateTime, Utc};
use sem_os_core::domain_pack::DomainPackManifest;
use serde::{Deserialize, Serialize};
use sha2::{Digest, Sha256};
use std::collections::BTreeSet;

use super::workbook::{
    EvidenceRef, ExecutionWorkbook, ExecutionWorkbookId, ExecutionWorkbookValidationError,
    WorkbookSubject,
};

#[derive(Debug, Clone, PartialEq, Eq, PartialOrd, Ord, Hash, Serialize, Deserialize)]
pub struct ApprovalTokenId(pub String);

impl std::fmt::Display for ApprovalTokenId {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        write!(f, "{}", self.0)
    }
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct MutationApprovalToken {
    pub id: ApprovalTokenId,
    pub core: MutationApprovalTokenCore,
    pub issued_at: DateTime<Utc>,
    pub status: MutationApprovalTokenStatus,
}

impl MutationApprovalToken {
    pub fn new(
        core: MutationApprovalTokenCore,
        issued_at: DateTime<Utc>,
    ) -> Result<Self, ApprovalTokenValidationError> {
        core.validate()?;
        Ok(Self {
            id: compute_approval_token_id(&core),
            core,
            issued_at,
            status: MutationApprovalTokenStatus::Active,
        })
    }

    pub fn validate_integrity(&self) -> Result<(), ApprovalTokenValidationError> {
        self.core.validate()?;
        let expected = compute_approval_token_id(&self.core);
        if expected != self.id {
            return Err(ApprovalTokenValidationError::TokenHashMismatch {
                expected,
                actual: self.id.clone(),
            });
        }
        Ok(())
    }
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct MutationApprovalTokenCore {
    pub schema_version: u16,
    pub workbook_id: ExecutionWorkbookId,
    pub session_id: uuid::Uuid,
    pub pack_id: String,
    pub transition_ref: String,
    pub subject: WorkbookSubject,
    pub requested_by_actor_id: String,
    pub approved_by_actor_id: String,
    pub approval_text: String,
    pub configuration_version: String,
    pub state_snapshot_id: String,
    pub evidence_refs: Vec<EvidenceRef>,
    pub expires_at: DateTime<Utc>,
}

impl MutationApprovalTokenCore {
    fn validate(&self) -> Result<(), ApprovalTokenValidationError> {
        require_non_empty("pack_id", &self.pack_id)?;
        require_non_empty("transition_ref", &self.transition_ref)?;
        require_non_empty("requested_by_actor_id", &self.requested_by_actor_id)?;
        require_non_empty("approved_by_actor_id", &self.approved_by_actor_id)?;
        require_non_empty("approval_text", &self.approval_text)?;
        require_non_empty("configuration_version", &self.configuration_version)?;
        require_non_empty("state_snapshot_id", &self.state_snapshot_id)?;

        if self.evidence_refs.is_empty() {
            return Err(ApprovalTokenValidationError::MissingEvidenceRefs);
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
pub enum MutationApprovalTokenStatus {
    Active,
    Consumed,
    Revoked,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct RestrictedMutationApprovalCheck {
    pub workbook_id: ExecutionWorkbookId,
    pub approval_token_id: ApprovalTokenId,
    pub transition_ref: String,
    pub approved_by_actor_id: String,
    pub expires_at: DateTime<Utc>,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct ObservedMutationAnchors {
    pub configuration_version: String,
    pub state_snapshot_id: String,
    pub evidence_refs: Vec<EvidenceRef>,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(tag = "kind", rename_all = "snake_case")]
pub enum ApprovalTokenValidationError {
    MissingApprovalToken,
    RequiredFieldEmpty {
        field: String,
    },
    MissingEvidenceRefs,
    WorkbookIntegrity {
        error: ExecutionWorkbookValidationError,
    },
    TokenHashMismatch {
        expected: ApprovalTokenId,
        actual: ApprovalTokenId,
    },
    TokenNotActive {
        status: MutationApprovalTokenStatus,
    },
    TokenExpired {
        expires_at: DateTime<Utc>,
        checked_at: DateTime<Utc>,
    },
    TokenReplay {
        token_id: ApprovalTokenId,
    },
    WorkbookBindingMismatch {
        field: String,
        expected: String,
        actual: String,
    },
    StateDrift {
        field: String,
        approved: String,
        observed: String,
    },
    EvidenceDrift {
        approved_digests: Vec<String>,
        observed_digests: Vec<String>,
    },
    TransitionMutationNotEnabled {
        transition_ref: String,
    },
}

pub fn create_approval_token_for_workbook(
    workbook: &ExecutionWorkbook,
    approved_by_actor_id: impl Into<String>,
    approval_text: impl Into<String>,
    expires_at: DateTime<Utc>,
    issued_at: DateTime<Utc>,
) -> Result<MutationApprovalToken, ApprovalTokenValidationError> {
    workbook
        .validate_integrity()
        .map_err(|error| ApprovalTokenValidationError::WorkbookIntegrity { error })?;

    MutationApprovalToken::new(
        MutationApprovalTokenCore {
            schema_version: 1,
            workbook_id: workbook.id.clone(),
            session_id: workbook.core.session_id,
            pack_id: workbook.core.pack_id.clone(),
            transition_ref: workbook.core.transition_ref.clone(),
            subject: workbook.core.subject.clone(),
            requested_by_actor_id: workbook.core.actor.actor_id.clone(),
            approved_by_actor_id: approved_by_actor_id.into(),
            approval_text: approval_text.into(),
            configuration_version: workbook.core.configuration_version.clone(),
            state_snapshot_id: workbook.core.state_snapshot_id.clone(),
            evidence_refs: workbook.core.evidence_refs.clone(),
            expires_at,
        },
        issued_at,
    )
}

pub fn validate_restricted_mutation_approval(
    workbook: &ExecutionWorkbook,
    token: Option<&MutationApprovalToken>,
    manifest: &DomainPackManifest,
    observed: &ObservedMutationAnchors,
    consumed_token_ids: &BTreeSet<ApprovalTokenId>,
    checked_at: DateTime<Utc>,
) -> Result<RestrictedMutationApprovalCheck, ApprovalTokenValidationError> {
    workbook
        .validate_integrity()
        .map_err(|error| ApprovalTokenValidationError::WorkbookIntegrity { error })?;

    let transition = manifest
        .allowed_transitions
        .iter()
        .find(|transition| transition.transition_ref == workbook.core.transition_ref)
        .ok_or_else(
            || ApprovalTokenValidationError::TransitionMutationNotEnabled {
                transition_ref: workbook.core.transition_ref.clone(),
            },
        )?;

    if !transition.mutation_enabled {
        return Err(ApprovalTokenValidationError::TransitionMutationNotEnabled {
            transition_ref: workbook.core.transition_ref.clone(),
        });
    }

    let token = token.ok_or(ApprovalTokenValidationError::MissingApprovalToken)?;
    token.validate_integrity()?;

    if token.status != MutationApprovalTokenStatus::Active {
        return Err(ApprovalTokenValidationError::TokenNotActive {
            status: token.status,
        });
    }

    if token.core.expires_at <= checked_at {
        return Err(ApprovalTokenValidationError::TokenExpired {
            expires_at: token.core.expires_at,
            checked_at,
        });
    }

    if consumed_token_ids.contains(&token.id) {
        return Err(ApprovalTokenValidationError::TokenReplay {
            token_id: token.id.clone(),
        });
    }

    assert_token_matches_workbook(token, workbook)?;
    assert_observed_anchors_match(token, observed)?;

    Ok(RestrictedMutationApprovalCheck {
        workbook_id: workbook.id.clone(),
        approval_token_id: token.id.clone(),
        transition_ref: workbook.core.transition_ref.clone(),
        approved_by_actor_id: token.core.approved_by_actor_id.clone(),
        expires_at: token.core.expires_at,
    })
}

pub fn compute_approval_token_id(core: &MutationApprovalTokenCore) -> ApprovalTokenId {
    let bytes = bincode::serialize(core)
        .expect("bincode serialization of MutationApprovalTokenCore is infallible");
    let digest = Sha256::digest(bytes);
    ApprovalTokenId(format!("approval:v1:{}", hex::encode(digest)))
}

fn assert_token_matches_workbook(
    token: &MutationApprovalToken,
    workbook: &ExecutionWorkbook,
) -> Result<(), ApprovalTokenValidationError> {
    let comparisons = [
        (
            "workbook_id",
            workbook.id.to_string(),
            token.core.workbook_id.to_string(),
        ),
        (
            "session_id",
            workbook.core.session_id.to_string(),
            token.core.session_id.to_string(),
        ),
        (
            "pack_id",
            workbook.core.pack_id.clone(),
            token.core.pack_id.clone(),
        ),
        (
            "transition_ref",
            workbook.core.transition_ref.clone(),
            token.core.transition_ref.clone(),
        ),
        (
            "subject_id",
            workbook.core.subject.subject_id.to_string(),
            token.core.subject.subject_id.to_string(),
        ),
        (
            "requested_by_actor_id",
            workbook.core.actor.actor_id.clone(),
            token.core.requested_by_actor_id.clone(),
        ),
        (
            "configuration_version",
            workbook.core.configuration_version.clone(),
            token.core.configuration_version.clone(),
        ),
        (
            "state_snapshot_id",
            workbook.core.state_snapshot_id.clone(),
            token.core.state_snapshot_id.clone(),
        ),
    ];

    for (field, expected, actual) in comparisons {
        if expected != actual {
            return Err(ApprovalTokenValidationError::WorkbookBindingMismatch {
                field: field.to_string(),
                expected,
                actual,
            });
        }
    }

    if evidence_digests(&workbook.core.evidence_refs) != evidence_digests(&token.core.evidence_refs)
    {
        return Err(ApprovalTokenValidationError::WorkbookBindingMismatch {
            field: "evidence_refs".to_string(),
            expected: evidence_digests(&workbook.core.evidence_refs).join(","),
            actual: evidence_digests(&token.core.evidence_refs).join(","),
        });
    }

    Ok(())
}

fn assert_observed_anchors_match(
    token: &MutationApprovalToken,
    observed: &ObservedMutationAnchors,
) -> Result<(), ApprovalTokenValidationError> {
    if token.core.configuration_version != observed.configuration_version {
        return Err(ApprovalTokenValidationError::StateDrift {
            field: "configuration_version".to_string(),
            approved: token.core.configuration_version.clone(),
            observed: observed.configuration_version.clone(),
        });
    }

    if token.core.state_snapshot_id != observed.state_snapshot_id {
        return Err(ApprovalTokenValidationError::StateDrift {
            field: "state_snapshot_id".to_string(),
            approved: token.core.state_snapshot_id.clone(),
            observed: observed.state_snapshot_id.clone(),
        });
    }

    let approved_digests = evidence_digests(&token.core.evidence_refs);
    let observed_digests = evidence_digests(&observed.evidence_refs);
    if approved_digests != observed_digests {
        return Err(ApprovalTokenValidationError::EvidenceDrift {
            approved_digests,
            observed_digests,
        });
    }

    Ok(())
}

fn evidence_digests(evidence_refs: &[EvidenceRef]) -> Vec<String> {
    let mut digests = evidence_refs
        .iter()
        .map(|evidence_ref| evidence_ref.digest.clone())
        .collect::<Vec<_>>();
    digests.sort();
    digests
}

fn require_non_empty(field: &'static str, value: &str) -> Result<(), ApprovalTokenValidationError> {
    if value.trim().is_empty() {
        return Err(ApprovalTokenValidationError::RequiredFieldEmpty {
            field: field.to_string(),
        });
    }
    Ok(())
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::runbook::{
        EvidenceRef, ExecutionWorkbook, ExecutionWorkbookCore, StaleWorkbookPolicy, WorkbookActor,
        WorkbookSubject,
    };
    use chrono::TimeZone;
    use sem_os_core::domain_pack::{
        ClassificationLimit, ContextClassificationPolicy, DomainPackManifest, DomainTransition,
        PackCompatibilityTier, PackImplementationMode,
    };
    use sem_os_core::state_simulation::{
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
            evidence_refs: vec![EvidenceRef {
                kind: "case_id".to_string(),
                ref_id: CASE_ID.to_string(),
                digest: "sha256:evidence".to_string(),
            }],
            llm_trace_ref: None,
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
            owned_constellations: vec!["kyc_case_lifecycle".to_string()],
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

    fn token(workbook: &ExecutionWorkbook) -> MutationApprovalToken {
        create_approval_token_for_workbook(
            workbook,
            "approver@example.com",
            "Approved for restricted KYC status update",
            expires_at(),
            now(),
        )
        .expect("approval token")
    }

    #[test]
    fn validates_token_bound_to_workbook_state_evidence_and_approval_text() {
        let workbook = workbook();
        let token = token(&workbook);
        let proof = validate_restricted_mutation_approval(
            &workbook,
            Some(&token),
            &manifest(true),
            &observed(&workbook),
            &BTreeSet::new(),
            now(),
        )
        .expect("approval accepted");

        assert_eq!(proof.workbook_id, workbook.id);
        assert_eq!(proof.approval_token_id, token.id);
        assert!(token.core.approval_text.contains("restricted KYC"));
    }

    #[test]
    fn refuses_unapproved_execution() {
        let workbook = workbook();
        let err = validate_restricted_mutation_approval(
            &workbook,
            None,
            &manifest(true),
            &observed(&workbook),
            &BTreeSet::new(),
            now(),
        )
        .expect_err("missing approval refused");

        assert_eq!(err, ApprovalTokenValidationError::MissingApprovalToken);
    }

    #[test]
    fn refuses_token_replay() {
        let workbook = workbook();
        let token = token(&workbook);
        let consumed = BTreeSet::from([token.id.clone()]);
        let err = validate_restricted_mutation_approval(
            &workbook,
            Some(&token),
            &manifest(true),
            &observed(&workbook),
            &consumed,
            now(),
        )
        .expect_err("replay refused");

        assert!(matches!(
            err,
            ApprovalTokenValidationError::TokenReplay { .. }
        ));
    }

    #[test]
    fn refuses_drifted_state_snapshot() {
        let workbook = workbook();
        let token = token(&workbook);
        let mut observed = observed(&workbook);
        observed.state_snapshot_id = "snapshot-2".to_string();

        let err = validate_restricted_mutation_approval(
            &workbook,
            Some(&token),
            &manifest(true),
            &observed,
            &BTreeSet::new(),
            now(),
        )
        .expect_err("drift refused");

        assert!(matches!(
            err,
            ApprovalTokenValidationError::StateDrift { field, .. } if field == "state_snapshot_id"
        ));
    }

    #[test]
    fn refuses_non_enabled_transition() {
        let workbook = workbook();
        let token = token(&workbook);

        let err = validate_restricted_mutation_approval(
            &workbook,
            Some(&token),
            &manifest(false),
            &observed(&workbook),
            &BTreeSet::new(),
            now(),
        )
        .expect_err("non-enabled mutation refused");

        assert!(matches!(
            err,
            ApprovalTokenValidationError::TransitionMutationNotEnabled { .. }
        ));
    }

    #[test]
    fn refuses_expired_token() {
        let workbook = workbook();
        let token = token(&workbook);
        let checked_at = Utc.with_ymd_and_hms(2026, 5, 5, 14, 0, 0).unwrap();

        let err = validate_restricted_mutation_approval(
            &workbook,
            Some(&token),
            &manifest(true),
            &observed(&workbook),
            &BTreeSet::new(),
            checked_at,
        )
        .expect_err("expired token refused");

        assert!(matches!(
            err,
            ApprovalTokenValidationError::TokenExpired { .. }
        ));
    }
}
