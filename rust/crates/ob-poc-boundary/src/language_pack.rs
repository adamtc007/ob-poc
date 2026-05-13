//! Task-shaped SemOS language packs for update-status workbook drafting.
//!
//! This stays deliberately bounded: it teaches Sage/Drafter one active
//! dry-run-only transition surface from a Domain Pack, rather than dumping the
//! whole SemOS substrate into prompt context.

use sem_os_core::domain_pack::{DomainPackManifest, DomainTransition};
use serde::{Deserialize, Serialize};
use uuid::Uuid;

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct KycLanguagePackRequest {
    pub subject_id: Uuid,
    pub current_state: String,
    pub configuration_version: String,
    pub state_snapshot_id: String,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub objective: Option<String>,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct UpdateStatusLanguagePackRequest {
    pub subject_id: Uuid,
    pub subject_kind: String,
    pub verb: String,
    pub current_state: String,
    pub configuration_version: String,
    pub state_snapshot_id: String,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub objective: Option<String>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub subject_uuid_field: Option<String>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub state_field: Option<String>,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct SemOsLanguagePack {
    pub objective: String,
    pub pack_id: String,
    pub pack_version: String,
    pub configuration_version: String,
    pub state_snapshot_id: String,
    pub subject: LanguagePackSubject,
    pub current_state: String,
    pub candidate_transitions: Vec<LanguagePackTransition>,
    pub valid_verbs: Vec<LanguagePackVerb>,
    pub blocked_verbs: Vec<BlockedVerb>,
    pub argument_schema: Vec<LanguagePackArg>,
    pub transition_effects: Vec<TransitionEffect>,
    pub evidence_policy: EvidencePolicySummary,
    pub uuid_bindings: Vec<UuidBindingRequirement>,
    pub canonical_patterns: Vec<CanonicalMicroPattern>,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct LanguagePackSubject {
    pub kind: String,
    pub id: Uuid,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct LanguagePackTransition {
    pub transition_ref: String,
    pub verb: String,
    pub from_state: String,
    pub to_state: String,
    pub dry_run_enabled: bool,
    pub mutation_enabled: bool,
    pub hitl_required: bool,
    pub evidence_refs_required: Vec<String>,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct LanguagePackVerb {
    pub verb: String,
    pub reason: String,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct BlockedVerb {
    pub verb: String,
    pub reason: String,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct LanguagePackArg {
    pub name: String,
    pub arg_type: String,
    pub required: bool,
    pub binding: String,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct TransitionEffect {
    pub transition_ref: String,
    pub field: String,
    pub before: String,
    pub after: String,
    pub writes_since_push_delta: u64,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct EvidencePolicySummary {
    pub required_evidence_refs: Vec<String>,
    pub dry_run_only: bool,
    pub mutation_allowed: bool,
    pub hitl_required: bool,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct UuidBindingRequirement {
    pub field: String,
    pub subject_kind: String,
    pub required: bool,
    pub expected_uuid: Uuid,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct CanonicalMicroPattern {
    pub name: String,
    pub draft_shape: String,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct TransitionLanguagePackReadiness {
    pub pack_id: String,
    pub transition_ref: String,
    pub verb: Option<String>,
    pub entity_type: Option<String>,
    pub from_state: Option<String>,
    pub to_state: Option<String>,
    pub ready: bool,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub generator: Option<String>,
    #[serde(default)]
    pub missing_requirements: Vec<String>,
    #[serde(default)]
    pub blocked_reasons: Vec<String>,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(tag = "kind", rename_all = "snake_case")]
pub enum LanguagePackError {
    PackInvalid { diagnostics: Vec<String> },
    UnsupportedVerb { verb: String },
    NoUpdateStatusTransitions { verb: String, current_state: String },
    NoKycUpdateStatusTransitions { current_state: String },
}

pub fn transition_language_pack_readiness(
    manifest: &DomainPackManifest,
    transition_ref: &str,
) -> TransitionLanguagePackReadiness {
    let Some(transition) = manifest
        .allowed_transitions
        .iter()
        .find(|transition| transition.transition_ref == transition_ref)
    else {
        return TransitionLanguagePackReadiness {
            pack_id: manifest.pack_id.clone(),
            transition_ref: transition_ref.to_string(),
            verb: None,
            entity_type: None,
            from_state: None,
            to_state: None,
            ready: false,
            generator: None,
            missing_requirements: vec!["declared_transition".to_string()],
            blocked_reasons: vec!["transition_ref is not declared in the Domain Pack".to_string()],
        };
    };

    let mut missing_requirements = Vec::new();
    let mut blocked_reasons = Vec::new();
    let generator = Some(
        if transition.entity_type == "kyc_case" && transition.verb == "kyc-case.update-status" {
            "kyc_update_status_language_pack_v1".to_string()
        } else {
            "update_status_language_pack_v1".to_string()
        },
    );

    if !transition.dry_run_enabled {
        missing_requirements.push("dry_run_enabled".to_string());
        blocked_reasons.push("transition must support dry-run validation".to_string());
    }
    if transition.mutation_enabled {
        missing_requirements.push("dry_run_only_transition".to_string());
        blocked_reasons.push("language-pack loop is not a mutation authority".to_string());
    }
    if transition.verb.trim().is_empty() {
        missing_requirements.push("verb".to_string());
    }
    if !transition.verb.ends_with(".update-status") {
        missing_requirements.push("update_status_verb".to_string());
        blocked_reasons.push(
            "generic language-pack generator is currently bounded to update-status verbs"
                .to_string(),
        );
    }
    if transition.from_state.trim().is_empty() {
        missing_requirements.push("from_state".to_string());
    }
    if transition.to_state.trim().is_empty() {
        missing_requirements.push("to_state".to_string());
    }
    if transition.evidence_refs_required.is_empty() {
        missing_requirements.push("evidence_refs_required".to_string());
        blocked_reasons.push("transition must declare evidence binding requirements".to_string());
    }

    TransitionLanguagePackReadiness {
        pack_id: manifest.pack_id.clone(),
        transition_ref: transition.transition_ref.clone(),
        verb: Some(transition.verb.clone()),
        entity_type: Some(transition.entity_type.clone()),
        from_state: Some(transition.from_state.clone()),
        to_state: Some(transition.to_state.clone()),
        ready: missing_requirements.is_empty(),
        generator,
        missing_requirements,
        blocked_reasons,
    }
}

pub fn transition_language_pack_readiness_report(
    manifest: &DomainPackManifest,
) -> Vec<TransitionLanguagePackReadiness> {
    manifest
        .allowed_transitions
        .iter()
        .map(|transition| transition_language_pack_readiness(manifest, &transition.transition_ref))
        .collect()
}

pub fn build_kyc_update_status_language_pack(
    manifest: &DomainPackManifest,
    request: KycLanguagePackRequest,
) -> Result<SemOsLanguagePack, LanguagePackError> {
    build_update_status_language_pack(
        manifest,
        UpdateStatusLanguagePackRequest {
            subject_id: request.subject_id,
            subject_kind: "kyc_case".to_string(),
            verb: "kyc-case.update-status".to_string(),
            current_state: request.current_state,
            configuration_version: request.configuration_version,
            state_snapshot_id: request.state_snapshot_id,
            objective: request.objective,
            subject_uuid_field: Some("case_id".to_string()),
            state_field: Some("status".to_string()),
        },
    )
    .map_err(|error| match error {
        LanguagePackError::NoUpdateStatusTransitions { current_state, .. } => {
            LanguagePackError::NoKycUpdateStatusTransitions { current_state }
        }
        other => other,
    })
}

pub fn build_update_status_language_pack(
    manifest: &DomainPackManifest,
    request: UpdateStatusLanguagePackRequest,
) -> Result<SemOsLanguagePack, LanguagePackError> {
    if !request.verb.ends_with(".update-status") {
        return Err(LanguagePackError::UnsupportedVerb { verb: request.verb });
    }

    let validation = manifest.validate();
    if !validation.valid {
        return Err(LanguagePackError::PackInvalid {
            diagnostics: validation
                .diagnostics
                .into_iter()
                .map(|d| format!("{}: {}", d.code, d.message))
                .collect(),
        });
    }

    let transitions: Vec<&DomainTransition> = manifest
        .allowed_transitions
        .iter()
        .filter(|transition| {
            transition.verb == request.verb && transition.entity_type == request.subject_kind
        })
        .collect();

    if transitions.is_empty() {
        return Err(LanguagePackError::NoUpdateStatusTransitions {
            verb: request.verb,
            current_state: request.current_state,
        });
    }

    let subject_uuid_field = request
        .subject_uuid_field
        .clone()
        .unwrap_or_else(|| default_subject_uuid_field(&request.subject_kind));
    let state_field = request
        .state_field
        .clone()
        .unwrap_or_else(|| "status".to_string());

    let candidate_transitions: Vec<LanguagePackTransition> = transitions
        .iter()
        .filter(|transition| transition.from_state == request.current_state)
        .map(|transition| language_transition(transition))
        .collect();

    let blocked_verbs = transitions
        .iter()
        .filter(|transition| transition.from_state != request.current_state)
        .map(|transition| BlockedVerb {
            verb: transition.verb.clone(),
            reason: format!(
                "{} is blocked because current state is {}, not {}",
                transition.transition_ref, request.current_state, transition.from_state
            ),
        })
        .collect();

    let transition_effects = candidate_transitions
        .iter()
        .map(|transition| TransitionEffect {
            transition_ref: transition.transition_ref.clone(),
            field: state_field.clone(),
            before: transition.from_state.clone(),
            after: transition.to_state.clone(),
            writes_since_push_delta: 1,
        })
        .collect();

    let required_evidence_refs = candidate_transitions
        .iter()
        .flat_map(|transition| transition.evidence_refs_required.clone())
        .collect::<std::collections::BTreeSet<_>>()
        .into_iter()
        .collect::<Vec<_>>();

    Ok(SemOsLanguagePack {
        objective: request.objective.unwrap_or_else(|| {
            format!(
                "Draft a dry-run workbook for {} from {}",
                request.verb, request.current_state
            )
        }),
        pack_id: manifest.pack_id.clone(),
        pack_version: manifest.version.clone(),
        configuration_version: request.configuration_version,
        state_snapshot_id: request.state_snapshot_id,
        subject: LanguagePackSubject {
            kind: request.subject_kind.clone(),
            id: request.subject_id,
        },
        current_state: request.current_state,
        candidate_transitions,
        valid_verbs: vec![LanguagePackVerb {
            verb: request.verb.clone(),
            reason: format!(
                "Only {} workbook drafting is in scope for this language pack",
                request.verb
            ),
        }],
        blocked_verbs,
        argument_schema: argument_schema(&subject_uuid_field, &request.subject_kind, &state_field),
        transition_effects,
        evidence_policy: EvidencePolicySummary {
            required_evidence_refs,
            dry_run_only: true,
            mutation_allowed: false,
            hitl_required: true,
        },
        uuid_bindings: vec![UuidBindingRequirement {
            field: subject_uuid_field.clone(),
            subject_kind: request.subject_kind.clone(),
            required: true,
            expected_uuid: request.subject_id,
        }],
        canonical_patterns: canonical_patterns(
            &request.verb,
            &subject_uuid_field,
            &request.subject_kind,
        ),
    })
}

fn language_transition(transition: &DomainTransition) -> LanguagePackTransition {
    LanguagePackTransition {
        transition_ref: transition.transition_ref.clone(),
        verb: transition.verb.clone(),
        from_state: transition.from_state.clone(),
        to_state: transition.to_state.clone(),
        dry_run_enabled: transition.dry_run_enabled,
        mutation_enabled: transition.mutation_enabled,
        hitl_required: transition.hitl_required,
        evidence_refs_required: transition.evidence_refs_required.clone(),
    }
}

fn argument_schema(
    subject_uuid_field: &str,
    subject_kind: &str,
    state_field: &str,
) -> Vec<LanguagePackArg> {
    vec![
        arg(
            subject_uuid_field,
            "uuid",
            &format!("active {subject_kind} subject UUID"),
        ),
        arg(
            "transition_ref",
            "string",
            "declared Domain Pack transition_ref",
        ),
        arg(
            "current_state",
            "enum",
            &format!("observed current {state_field}"),
        ),
        arg(
            "requested_state",
            "enum",
            &format!("requested target {state_field}"),
        ),
        arg(
            "configuration_version",
            "string",
            "Domain Pack/config anchor",
        ),
        arg("state_snapshot_id", "string", "state snapshot anchor"),
        arg(
            "evidence_digest",
            "string",
            "digest for required case evidence",
        ),
    ]
}

fn arg(name: &str, arg_type: &str, binding: &str) -> LanguagePackArg {
    LanguagePackArg {
        name: name.to_string(),
        arg_type: arg_type.to_string(),
        required: true,
        binding: binding.to_string(),
    }
}

fn canonical_patterns(
    verb: &str,
    subject_uuid_field: &str,
    subject_kind: &str,
) -> Vec<CanonicalMicroPattern> {
    vec![
        pattern(
            "happy_path",
            &format!(
                "Use verb {verb} with the candidate transition whose from_state equals current_state."
            ),
        ),
        pattern(
            "uuid_binding",
            &format!(
                "Bind {subject_uuid_field} to the active {subject_kind} UUID from uuid_bindings; do not invent a UUID."
            ),
        ),
        pattern(
            "state_binding",
            "Set current_state to the observed language-pack current_state.",
        ),
        pattern(
            "target_binding",
            "Set requested_state to the selected transition to_state.",
        ),
        pattern(
            "dry_run_only",
            "Produce a dry-run workbook only; ACP mutation and direct execution are out of scope.",
        ),
    ]
}

fn default_subject_uuid_field(subject_kind: &str) -> String {
    match subject_kind {
        "kyc_case" => "case_id".to_string(),
        "deal" => "deal_id".to_string(),
        other => format!("{}_id", other.trim_end_matches("_case")),
    }
}

fn pattern(name: &str, draft_shape: &str) -> CanonicalMicroPattern {
    CanonicalMicroPattern {
        name: name.to_string(),
        draft_shape: draft_shape.to_string(),
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn manifest() -> DomainPackManifest {
        serde_yaml::from_str(include_str!(
            "../../../config/sem_os_seeds/domain_packs/ob_poc_kyc.yaml"
        ))
        .expect("KYC Domain Pack parses")
    }

    #[test]
    fn transition_language_pack_readiness_marks_supported_kyc_transitions() {
        let report = transition_language_pack_readiness_report(&manifest());

        assert_eq!(report.len(), 2);
        assert!(report.iter().all(|entry| entry.ready));
        assert!(report
            .iter()
            .all(|entry| entry.generator.as_deref() == Some("kyc_update_status_language_pack_v1")));
    }

    #[test]
    fn transition_language_pack_readiness_blocks_unknown_transition() {
        let readiness = transition_language_pack_readiness(&manifest(), "kyc-case.close");

        assert!(!readiness.ready);
        assert_eq!(readiness.missing_requirements, vec!["declared_transition"]);
        assert!(readiness.generator.is_none());
    }

    #[test]
    fn transition_language_pack_readiness_marks_generic_dry_run_transition_ready() {
        let mut manifest = manifest();
        manifest.allowed_transitions.push(DomainTransition {
            transition_ref: "screening.ready-to-reviewed".to_string(),
            entity_type: "screening_case".to_string(),
            state_machine: "screening_lifecycle".to_string(),
            verb: "screening.update-status".to_string(),
            from_state: "READY".to_string(),
            to_state: "REVIEWED".to_string(),
            dry_run_enabled: true,
            mutation_enabled: false,
            hitl_required: true,
            evidence_refs_required: vec!["screening_id".to_string()],
        });

        let readiness =
            transition_language_pack_readiness(&manifest, "screening.ready-to-reviewed");

        assert!(readiness.ready);
        assert!(readiness.missing_requirements.is_empty());
        assert_eq!(
            readiness.generator.as_deref(),
            Some("update_status_language_pack_v1")
        );
    }

    #[test]
    fn build_update_status_language_pack_uses_requested_verb_and_subject() {
        let subject_id = Uuid::parse_str("11111111-1111-1111-1111-111111111111").unwrap();
        let mut manifest = manifest();
        manifest.allowed_transitions.push(DomainTransition {
            transition_ref: "screening.ready-to-reviewed".to_string(),
            entity_type: "screening_case".to_string(),
            state_machine: "screening_lifecycle".to_string(),
            verb: "screening.update-status".to_string(),
            from_state: "READY".to_string(),
            to_state: "REVIEWED".to_string(),
            dry_run_enabled: true,
            mutation_enabled: false,
            hitl_required: true,
            evidence_refs_required: vec!["screening_id".to_string()],
        });

        let pack = build_update_status_language_pack(
            &manifest,
            UpdateStatusLanguagePackRequest {
                subject_id,
                subject_kind: "screening_case".to_string(),
                verb: "screening.update-status".to_string(),
                current_state: "READY".to_string(),
                configuration_version: "config-screening".to_string(),
                state_snapshot_id: "snapshot-screening".to_string(),
                objective: None,
                subject_uuid_field: Some("screening_id".to_string()),
                state_field: Some("screening_status".to_string()),
            },
        )
        .expect("generic language pack");

        assert_eq!(pack.subject.kind, "screening_case");
        assert_eq!(pack.valid_verbs[0].verb, "screening.update-status");
        assert_eq!(
            pack.candidate_transitions[0].transition_ref,
            "screening.ready-to-reviewed"
        );
        assert_eq!(pack.transition_effects[0].field, "screening_status");
        assert_eq!(pack.uuid_bindings[0].field, "screening_id");
    }

    #[test]
    fn build_update_status_language_pack_refuses_non_update_status_verbs() {
        let subject_id = Uuid::parse_str("11111111-1111-1111-1111-111111111111").unwrap();
        let err = build_update_status_language_pack(
            &manifest(),
            UpdateStatusLanguagePackRequest {
                subject_id,
                subject_kind: "kyc_case".to_string(),
                verb: "kyc-case.close".to_string(),
                current_state: "DISCOVERY".to_string(),
                configuration_version: "config-1".to_string(),
                state_snapshot_id: "snapshot-1".to_string(),
                objective: None,
                subject_uuid_field: Some("case_id".to_string()),
                state_field: Some("status".to_string()),
            },
        )
        .expect_err("non update-status verb refused");

        assert_eq!(
            err,
            LanguagePackError::UnsupportedVerb {
                verb: "kyc-case.close".to_string()
            }
        );
    }
}
