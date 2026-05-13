//! Structured workbook diagnostics for bounded Sage/Coder revision.
//!
//! Diagnostics are deliberately machine-usable. Invalid private DSL drafts
//! must not collapse into prose-only failures.

use sem_os_core::state_simulation::StateSimulationError;
use serde::{Deserialize, Serialize};

use crate::dsl_coder::DslDrafterValidationError;
use crate::kyc_dry_run::KycUpdateStatusDryRunRefusal;
use crate::language_pack::SemOsLanguagePack;
use crate::workbook::ExecutionWorkbookValidationError;

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct WorkbookDiagnostic {
    pub error_code: String,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub attempted_transition: Option<String>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub attempted_verb: Option<String>,
    pub source_path: String,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub expected_state: Option<String>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub actual_state: Option<String>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub missing_uuid_binding: Option<String>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub blocked_transition_reason: Option<String>,
    #[serde(default)]
    pub suggested_transitions: Vec<String>,
    #[serde(default)]
    pub suggested_verbs: Vec<String>,
    pub pack_ref: String,
    pub configuration_version: String,
    pub state_snapshot_id: String,
}

impl WorkbookDiagnostic {
    pub fn new(
        error_code: impl Into<String>,
        source_path: impl Into<String>,
        pack: &SemOsLanguagePack,
    ) -> Self {
        Self {
            error_code: error_code.into(),
            attempted_transition: None,
            attempted_verb: None,
            source_path: source_path.into(),
            expected_state: None,
            actual_state: None,
            missing_uuid_binding: None,
            blocked_transition_reason: None,
            suggested_transitions: pack
                .candidate_transitions
                .iter()
                .map(|transition| transition.transition_ref.clone())
                .collect(),
            suggested_verbs: pack
                .valid_verbs
                .iter()
                .map(|verb| verb.verb.clone())
                .collect(),
            pack_ref: format!("{}@{}", pack.pack_id, pack.pack_version),
            configuration_version: pack.configuration_version.clone(),
            state_snapshot_id: pack.state_snapshot_id.clone(),
        }
    }

    pub fn invented_verb(pack: &SemOsLanguagePack, attempted_verb: impl Into<String>) -> Self {
        let mut diag = Self::new("invented_verb", "draft.verb", pack);
        diag.attempted_verb = Some(attempted_verb.into());
        diag.blocked_transition_reason =
            Some("verb is not present in the bounded SemOS language pack".to_string());
        diag
    }

    pub fn missing_uuid(pack: &SemOsLanguagePack, field: impl Into<String>) -> Self {
        let field = field.into();
        let mut diag = Self::new("missing_uuid_binding", format!("draft.{field}"), pack);
        diag.missing_uuid_binding = Some(field);
        diag
    }

    pub fn stale_anchor(
        pack: &SemOsLanguagePack,
        field: impl Into<String>,
        actual: impl Into<String>,
    ) -> Self {
        let field = field.into();
        let expected = match field.as_str() {
            "configuration_version" => pack.configuration_version.clone(),
            "state_snapshot_id" => pack.state_snapshot_id.clone(),
            _ => String::new(),
        };
        let mut diag = Self::new("stale_replan_required", format!("draft.{field}"), pack);
        diag.expected_state = Some(expected);
        diag.actual_state = Some(actual.into());
        diag.blocked_transition_reason =
            Some("draft anchor differs from language pack anchor".to_string());
        diag
    }

    pub fn missing_evidence(pack: &SemOsLanguagePack) -> Self {
        let mut diag = Self::new("missing_evidence_digest", "draft.evidence_digest", pack);
        diag.blocked_transition_reason =
            Some("required evidence digest is missing from workbook draft".to_string());
        diag
    }

    pub fn wrong_subject_kind(pack: &SemOsLanguagePack, actual: impl Into<String>) -> Self {
        let mut diag = Self::new("wrong_subject_kind", "draft.subject_kind", pack);
        diag.expected_state = Some(pack.subject.kind.clone());
        diag.actual_state = Some(actual.into());
        diag
    }

    pub fn missing_required_workbook_field(
        pack: &SemOsLanguagePack,
        field: impl Into<String>,
        attempted_verb: Option<String>,
        attempted_transition: Option<String>,
    ) -> Self {
        let field = field.into();
        let mut diag = Self::new(
            "missing_required_workbook_field",
            format!("draft.{field}"),
            pack,
        );
        diag.attempted_verb = attempted_verb;
        diag.attempted_transition = attempted_transition;
        diag.actual_state = Some("missing".to_string());
        diag.blocked_transition_reason =
            Some("LLM tool arguments omitted a required workbook field".to_string());
        if field == "transition_ref" && pack.candidate_transitions.len() == 1 {
            diag.expected_state = pack
                .candidate_transitions
                .first()
                .map(|transition| transition.transition_ref.clone());
        }
        diag
    }

    pub fn repaired_required_workbook_field(
        pack: &SemOsLanguagePack,
        field: impl Into<String>,
        repaired_value: impl Into<String>,
        reason: impl Into<String>,
        attempted_verb: Option<String>,
        attempted_transition: Option<String>,
    ) -> Self {
        let field = field.into();
        let mut diag = Self::new(
            "repaired_required_workbook_field",
            format!("draft.{field}"),
            pack,
        );
        diag.attempted_verb = attempted_verb;
        diag.attempted_transition = attempted_transition;
        diag.expected_state = Some(repaired_value.into());
        diag.actual_state = Some("missing".to_string());
        diag.blocked_transition_reason = Some(reason.into());
        diag
    }

    pub fn invalid_llm_draft_shape(pack: &SemOsLanguagePack, actual: impl Into<String>) -> Self {
        let mut diag = Self::new("invalid_llm_draft_shape", "draft", pack);
        diag.actual_state = Some(actual.into());
        diag.blocked_transition_reason =
            Some("LLM tool arguments were not a JSON object workbook draft".to_string());
        diag
    }

    pub fn llm_draft_decode_failed(
        pack: &SemOsLanguagePack,
        reason: impl Into<String>,
        attempted_verb: Option<String>,
        attempted_transition: Option<String>,
    ) -> Self {
        let mut diag = Self::new("llm_draft_decode_failed", "draft", pack);
        diag.attempted_verb = attempted_verb;
        diag.attempted_transition = attempted_transition;
        diag.blocked_transition_reason = Some(reason.into());
        diag
    }

    pub fn llm_adapter_failure(
        pack: &SemOsLanguagePack,
        error_code: impl Into<String>,
        source_path: impl Into<String>,
        reason: impl Into<String>,
    ) -> Self {
        let mut diag = Self::new(error_code, source_path, pack);
        diag.blocked_transition_reason = Some(reason.into());
        diag
    }
}

pub fn diagnostics_from_dry_run_refusal(
    refusal: &KycUpdateStatusDryRunRefusal,
    pack: &SemOsLanguagePack,
) -> Vec<WorkbookDiagnostic> {
    match refusal {
        KycUpdateStatusDryRunRefusal::SimulationRefused { error } => {
            vec![diagnostic_from_state_simulation(error, pack)]
        }
        KycUpdateStatusDryRunRefusal::WorkbookRefused { reason } => {
            let mut diag = WorkbookDiagnostic::new("workbook_refused", "workbook", pack);
            diag.blocked_transition_reason = Some(reason.clone());
            vec![diag]
        }
        KycUpdateStatusDryRunRefusal::DslDrafterRefused { error } => {
            vec![diagnostic_from_dsl_coder(error, pack)]
        }
        KycUpdateStatusDryRunRefusal::PackInvalid { diagnostics } => diagnostics
            .iter()
            .map(|d| {
                let mut diag = WorkbookDiagnostic::new("pack_invalid", "domain_pack", pack);
                diag.blocked_transition_reason = Some(format!("{}: {}", d.code, d.message));
                diag
            })
            .collect(),
        KycUpdateStatusDryRunRefusal::PackParseFailed { reason } => {
            let mut diag = WorkbookDiagnostic::new("pack_parse_failed", "domain_pack", pack);
            diag.blocked_transition_reason = Some(reason.clone());
            vec![diag]
        }
    }
}

pub fn diagnostic_from_state_simulation(
    error: &StateSimulationError,
    pack: &SemOsLanguagePack,
) -> WorkbookDiagnostic {
    match error {
        StateSimulationError::UnknownTransition { transition_ref } => {
            let mut diag =
                WorkbookDiagnostic::new("unknown_transition", "draft.transition_ref", pack);
            diag.attempted_transition = Some(transition_ref.clone());
            diag.blocked_transition_reason =
                Some("transition_ref is not declared in the Domain Pack".to_string());
            diag
        }
        StateSimulationError::DryRunDisabled { transition_ref } => {
            let mut diag =
                WorkbookDiagnostic::new("dry_run_disabled", "draft.transition_ref", pack);
            diag.attempted_transition = Some(transition_ref.clone());
            diag.blocked_transition_reason =
                Some("transition has dry_run_enabled=false".to_string());
            diag
        }
        StateSimulationError::TransitionShapeMismatch {
            transition_ref,
            field,
            expected,
            actual,
        } => {
            let mut diag = WorkbookDiagnostic::new(
                format!("transition_shape_mismatch_{field}"),
                format!("draft.{field}"),
                pack,
            );
            diag.attempted_transition = Some(transition_ref.clone());
            diag.expected_state = Some(expected.clone());
            diag.actual_state = Some(actual.clone());
            diag
        }
        StateSimulationError::CurrentStateMismatch {
            transition_ref,
            expected,
            actual,
        } => {
            let mut diag =
                WorkbookDiagnostic::new("current_state_mismatch", "draft.current_state", pack);
            diag.attempted_transition = Some(transition_ref.clone());
            diag.expected_state = Some(expected.clone());
            diag.actual_state = Some(actual.clone());
            diag
        }
        StateSimulationError::RequestedStateMismatch {
            transition_ref,
            expected,
            actual,
        } => {
            let mut diag =
                WorkbookDiagnostic::new("requested_state_mismatch", "draft.requested_state", pack);
            diag.attempted_transition = Some(transition_ref.clone());
            diag.expected_state = Some(expected.clone());
            diag.actual_state = Some(actual.clone());
            diag
        }
        StateSimulationError::PackMismatch { expected, actual } => {
            let mut diag = WorkbookDiagnostic::new("pack_mismatch", "draft.pack_id", pack);
            diag.expected_state = Some(expected.clone());
            diag.actual_state = Some(actual.clone());
            diag
        }
    }
}

pub fn diagnostic_from_workbook_validation(
    error: &ExecutionWorkbookValidationError,
    pack: &SemOsLanguagePack,
) -> WorkbookDiagnostic {
    match error {
        ExecutionWorkbookValidationError::RequiredFieldEmpty { field } => {
            WorkbookDiagnostic::new("required_field_empty", format!("workbook.{field}"), pack)
        }
        ExecutionWorkbookValidationError::MissingEvidenceRefs => {
            WorkbookDiagnostic::missing_evidence(pack)
        }
        ExecutionWorkbookValidationError::TransitionRefMismatch {
            workbook_transition_ref,
            simulation_transition_ref,
        } => {
            let mut diag =
                WorkbookDiagnostic::new("transition_ref_mismatch", "workbook.transition_ref", pack);
            diag.attempted_transition = Some(workbook_transition_ref.clone());
            diag.suggested_transitions = vec![simulation_transition_ref.clone()];
            diag
        }
        ExecutionWorkbookValidationError::SubjectMismatch {
            workbook_subject_id,
            simulation_entity_id,
        } => {
            let mut diag =
                WorkbookDiagnostic::new("subject_mismatch", "workbook.subject.subject_id", pack);
            diag.actual_state = Some(workbook_subject_id.to_string());
            diag.expected_state = Some(simulation_entity_id.to_string());
            diag
        }
        ExecutionWorkbookValidationError::HashMismatch { expected, actual } => {
            let mut diag = WorkbookDiagnostic::new("workbook_hash_mismatch", "workbook.id", pack);
            diag.expected_state = Some(expected.to_string());
            diag.actual_state = Some(actual.to_string());
            diag
        }
    }
}

fn diagnostic_from_dsl_coder(
    error: &DslDrafterValidationError,
    pack: &SemOsLanguagePack,
) -> WorkbookDiagnostic {
    let mut diag = WorkbookDiagnostic::new(
        format!("{:?}", error.code).to_ascii_lowercase(),
        "dsl_coder",
        pack,
    );
    diag.blocked_transition_reason = Some(error.message.clone());
    diag
}
