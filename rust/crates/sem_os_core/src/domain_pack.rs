//! Domain Pack contract for configuration-native state-machine agents.
//!
//! This is distinct from Journey Pack manifests. A Domain Pack declares the
//! state-machine surface an adapter may discover, dry-run, and eventually mutate.

use serde::{Deserialize, Serialize};
use std::collections::{BTreeMap, HashSet};

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct DomainPackManifest {
    pub pack_id: String,
    pub name: String,
    pub version: String,
    pub implementation_mode: PackImplementationMode,
    pub compatibility_tier: PackCompatibilityTier,
    #[serde(default)]
    pub owned_constellations: Vec<String>,
    #[serde(default)]
    pub allowed_transitions: Vec<DomainTransition>,
    #[serde(default)]
    pub discovery_probes: Vec<DiscoveryProbe>,
    pub classification_policy: ContextClassificationPolicy,
}

impl DomainPackManifest {
    pub fn validate(&self) -> DomainPackValidationReport {
        validate_domain_pack(self)
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum PackImplementationMode {
    NativeCompiled,
    ExternalAdapter,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum PackCompatibilityTier {
    DryRunOnly,
    ReferenceMutation,
    ReuseProof,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct DomainTransition {
    pub transition_ref: String,
    pub entity_type: String,
    pub state_machine: String,
    pub verb: String,
    pub from_state: String,
    pub to_state: String,
    #[serde(default)]
    pub dry_run_enabled: bool,
    #[serde(default)]
    pub mutation_enabled: bool,
    #[serde(default)]
    pub hitl_required: bool,
    #[serde(default)]
    pub evidence_refs_required: Vec<String>,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct DiscoveryProbe {
    pub probe_id: String,
    pub operation: String,
    pub target: String,
    #[serde(default)]
    pub idempotent: bool,
    #[serde(default)]
    pub modeled: bool,
    #[serde(default)]
    pub first_class_state_mutation: bool,
}

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct DiscoveryRequest {
    pub pack_id: String,
    pub probe_id: String,
    pub subject: DiscoverySubject,
    #[serde(default)]
    pub context: BTreeMap<String, serde_json::Value>,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct DiscoverySubject {
    pub subject_kind: String,
    pub subject_id: String,
}

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct DiscoveryResponse {
    pub probe_id: String,
    pub subject: DiscoverySubject,
    #[serde(default)]
    pub observations: Vec<DiscoveryObservation>,
    #[serde(default)]
    pub provenance: Vec<DiscoveryProvenance>,
    #[serde(default)]
    pub first_class_state_mutated: bool,
}

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct DiscoveryObservation {
    pub key: String,
    pub value: serde_json::Value,
    #[serde(default)]
    pub classification: Option<ClassificationLimit>,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct DiscoveryProvenance {
    pub source: String,
    #[serde(default)]
    pub snapshot_ref: Option<String>,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub enum DiscoveryAuthorizationError {
    PackMismatch { expected: String, actual: String },
    UnknownProbe { probe_id: String },
    UnsafeProbe { probe_id: String, code: String },
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct ContextClassificationPolicy {
    pub max_prompt_classification: ClassificationLimit,
    #[serde(default)]
    pub allow_external_llm: bool,
    #[serde(default)]
    pub required_redactions: Vec<String>,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum ClassificationLimit {
    Public,
    Internal,
    Confidential,
    Restricted,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct DomainPackValidationReport {
    pub valid: bool,
    pub diagnostics: Vec<DomainPackDiagnostic>,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct DomainPackDiagnostic {
    pub code: String,
    pub message: String,
}

impl DomainPackValidationReport {
    fn from_diagnostics(diagnostics: Vec<DomainPackDiagnostic>) -> Self {
        Self {
            valid: diagnostics.is_empty(),
            diagnostics,
        }
    }
}

pub fn validate_domain_pack(manifest: &DomainPackManifest) -> DomainPackValidationReport {
    let mut diagnostics = Vec::new();

    require_non_empty(&mut diagnostics, "pack_id", &manifest.pack_id);
    require_non_empty(&mut diagnostics, "name", &manifest.name);
    require_non_empty(&mut diagnostics, "version", &manifest.version);

    if manifest.owned_constellations.is_empty() {
        diagnostics.push(diagnostic(
            "domain_pack.no_owned_constellations",
            "domain pack must declare at least one owned constellation",
        ));
    }

    if manifest.allowed_transitions.is_empty() {
        diagnostics.push(diagnostic(
            "domain_pack.no_allowed_transitions",
            "domain pack must declare at least one allowed transition",
        ));
    }

    let mut transition_refs = HashSet::new();
    for transition in &manifest.allowed_transitions {
        validate_transition(transition, &mut transition_refs, &mut diagnostics);
    }

    let mut probe_ids = HashSet::new();
    for probe in &manifest.discovery_probes {
        validate_probe(probe, &mut probe_ids, &mut diagnostics);
    }

    if matches!(
        manifest.compatibility_tier,
        PackCompatibilityTier::DryRunOnly | PackCompatibilityTier::ReuseProof
    ) {
        for transition in &manifest.allowed_transitions {
            if transition.mutation_enabled {
                diagnostics.push(diagnostic(
                    "domain_pack.mutation_not_allowed_for_tier",
                    format!(
                        "{} enables mutation but pack tier is {:?}",
                        transition.transition_ref, manifest.compatibility_tier
                    ),
                ));
            }
        }
    }

    if manifest.classification_policy.allow_external_llm
        && matches!(
            manifest.classification_policy.max_prompt_classification,
            ClassificationLimit::Confidential | ClassificationLimit::Restricted
        )
    {
        diagnostics.push(diagnostic(
            "domain_pack.external_llm_classification_limit",
            "external LLM prompts may not include confidential or restricted context",
        ));
    }

    DomainPackValidationReport::from_diagnostics(diagnostics)
}

pub fn authorize_discovery_probe<'a>(
    manifest: &'a DomainPackManifest,
    request: &DiscoveryRequest,
) -> Result<&'a DiscoveryProbe, DiscoveryAuthorizationError> {
    if request.pack_id != manifest.pack_id {
        return Err(DiscoveryAuthorizationError::PackMismatch {
            expected: manifest.pack_id.clone(),
            actual: request.pack_id.clone(),
        });
    }

    let Some(probe) = manifest
        .discovery_probes
        .iter()
        .find(|probe| probe.probe_id == request.probe_id)
    else {
        return Err(DiscoveryAuthorizationError::UnknownProbe {
            probe_id: request.probe_id.clone(),
        });
    };

    if !probe.idempotent {
        return Err(DiscoveryAuthorizationError::UnsafeProbe {
            probe_id: probe.probe_id.clone(),
            code: "domain_pack.probe_not_idempotent".to_string(),
        });
    }

    if !probe.modeled {
        return Err(DiscoveryAuthorizationError::UnsafeProbe {
            probe_id: probe.probe_id.clone(),
            code: "domain_pack.probe_not_modeled".to_string(),
        });
    }

    if probe.first_class_state_mutation {
        return Err(DiscoveryAuthorizationError::UnsafeProbe {
            probe_id: probe.probe_id.clone(),
            code: "domain_pack.probe_mutates_state".to_string(),
        });
    }

    Ok(probe)
}

fn validate_transition(
    transition: &DomainTransition,
    seen: &mut HashSet<String>,
    diagnostics: &mut Vec<DomainPackDiagnostic>,
) {
    require_non_empty(diagnostics, "transition_ref", &transition.transition_ref);
    require_non_empty(diagnostics, "entity_type", &transition.entity_type);
    require_non_empty(diagnostics, "state_machine", &transition.state_machine);
    require_non_empty(diagnostics, "verb", &transition.verb);
    require_non_empty(diagnostics, "from_state", &transition.from_state);
    require_non_empty(diagnostics, "to_state", &transition.to_state);

    if !transition.transition_ref.trim().is_empty()
        && !seen.insert(transition.transition_ref.clone())
    {
        diagnostics.push(diagnostic(
            "domain_pack.duplicate_transition_ref",
            format!("duplicate transition_ref {}", transition.transition_ref),
        ));
    }

    if transition.from_state == transition.to_state {
        diagnostics.push(diagnostic(
            "domain_pack.noop_transition",
            format!(
                "{} has the same from_state and to_state",
                transition.transition_ref
            ),
        ));
    }

    if transition.mutation_enabled && !transition.hitl_required {
        diagnostics.push(diagnostic(
            "domain_pack.mutation_requires_hitl",
            format!(
                "{} enables mutation without HITL",
                transition.transition_ref
            ),
        ));
    }

    if transition.mutation_enabled && !transition.dry_run_enabled {
        diagnostics.push(diagnostic(
            "domain_pack.mutation_requires_dry_run",
            format!(
                "{} enables mutation without dry-run",
                transition.transition_ref
            ),
        ));
    }
}

fn validate_probe(
    probe: &DiscoveryProbe,
    seen: &mut HashSet<String>,
    diagnostics: &mut Vec<DomainPackDiagnostic>,
) {
    require_non_empty(diagnostics, "probe_id", &probe.probe_id);
    require_non_empty(diagnostics, "operation", &probe.operation);
    require_non_empty(diagnostics, "target", &probe.target);

    if !probe.probe_id.trim().is_empty() && !seen.insert(probe.probe_id.clone()) {
        diagnostics.push(diagnostic(
            "domain_pack.duplicate_probe_id",
            format!("duplicate probe_id {}", probe.probe_id),
        ));
    }

    if !probe.idempotent {
        diagnostics.push(diagnostic(
            "domain_pack.probe_not_idempotent",
            format!("{} must be idempotent", probe.probe_id),
        ));
    }

    if !probe.modeled {
        diagnostics.push(diagnostic(
            "domain_pack.probe_not_modeled",
            format!("{} must be modeled", probe.probe_id),
        ));
    }

    if probe.first_class_state_mutation {
        diagnostics.push(diagnostic(
            "domain_pack.probe_mutates_state",
            format!("{} declares first-class state mutation", probe.probe_id),
        ));
    }
}

fn require_non_empty(
    diagnostics: &mut Vec<DomainPackDiagnostic>,
    field: &'static str,
    value: &str,
) {
    if value.trim().is_empty() {
        diagnostics.push(diagnostic(
            "domain_pack.required_field_empty",
            format!("{field} must not be empty"),
        ));
    }
}

fn diagnostic(code: &'static str, message: impl Into<String>) -> DomainPackDiagnostic {
    DomainPackDiagnostic {
        code: code.to_string(),
        message: message.into(),
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::fs;
    use std::path::PathBuf;

    fn valid_manifest() -> DomainPackManifest {
        DomainPackManifest {
            pack_id: "ob-poc.kyc".to_string(),
            name: "ob-poc KYC".to_string(),
            version: "0.1.0".to_string(),
            implementation_mode: PackImplementationMode::NativeCompiled,
            compatibility_tier: PackCompatibilityTier::DryRunOnly,
            owned_constellations: vec!["kyc.onboarding".to_string()],
            allowed_transitions: vec![DomainTransition {
                transition_ref: "kyc-case.intake-to-discovery".to_string(),
                entity_type: "kyc_case".to_string(),
                state_machine: "kyc_case_lifecycle".to_string(),
                verb: "kyc-case.update-status".to_string(),
                from_state: "INTAKE".to_string(),
                to_state: "DISCOVERY".to_string(),
                dry_run_enabled: true,
                mutation_enabled: false,
                hitl_required: true,
                evidence_refs_required: vec!["case_id".to_string()],
            }],
            discovery_probes: vec![DiscoveryProbe {
                probe_id: "kyc-case.read-state".to_string(),
                operation: "read_state".to_string(),
                target: "\"ob-poc\".cases.status".to_string(),
                idempotent: true,
                modeled: true,
                first_class_state_mutation: false,
            }],
            classification_policy: ContextClassificationPolicy {
                max_prompt_classification: ClassificationLimit::Internal,
                allow_external_llm: false,
                required_redactions: vec!["pii".to_string()],
            },
        }
    }

    #[test]
    fn valid_manifest_passes() {
        let report = valid_manifest().validate();
        assert!(report.valid, "{:?}", report.diagnostics);
        assert!(report.diagnostics.is_empty());
    }

    #[test]
    fn mutation_requires_hitl_and_dry_run() {
        let mut manifest = valid_manifest();
        let transition = &mut manifest.allowed_transitions[0];
        transition.dry_run_enabled = false;
        transition.mutation_enabled = true;
        transition.hitl_required = false;

        let report = manifest.validate();

        assert!(!report.valid);
        let codes: Vec<_> = report.diagnostics.iter().map(|d| d.code.as_str()).collect();
        assert!(codes.contains(&"domain_pack.mutation_requires_hitl"));
        assert!(codes.contains(&"domain_pack.mutation_requires_dry_run"));
        assert!(codes.contains(&"domain_pack.mutation_not_allowed_for_tier"));
    }

    #[test]
    fn probes_must_be_safe_and_modeled() {
        let mut manifest = valid_manifest();
        let probe = &mut manifest.discovery_probes[0];
        probe.idempotent = false;
        probe.modeled = false;
        probe.first_class_state_mutation = true;

        let report = manifest.validate();

        assert!(!report.valid);
        let codes: Vec<_> = report.diagnostics.iter().map(|d| d.code.as_str()).collect();
        assert!(codes.contains(&"domain_pack.probe_not_idempotent"));
        assert!(codes.contains(&"domain_pack.probe_not_modeled"));
        assert!(codes.contains(&"domain_pack.probe_mutates_state"));
    }

    #[test]
    fn duplicate_transition_refs_are_rejected() {
        let mut manifest = valid_manifest();
        manifest
            .allowed_transitions
            .push(manifest.allowed_transitions[0].clone());

        let report = manifest.validate();

        assert!(!report.valid);
        assert!(report
            .diagnostics
            .iter()
            .any(|d| d.code == "domain_pack.duplicate_transition_ref"));
    }

    #[test]
    fn discovery_probe_authorization_allows_declared_safe_probe() {
        let manifest = valid_manifest();
        let request = DiscoveryRequest {
            pack_id: "ob-poc.kyc".to_string(),
            probe_id: "kyc-case.read-state".to_string(),
            subject: DiscoverySubject {
                subject_kind: "kyc_case".to_string(),
                subject_id: "case-1".to_string(),
            },
            context: BTreeMap::new(),
        };

        let probe = authorize_discovery_probe(&manifest, &request).expect("probe authorized");

        assert_eq!(probe.operation, "read_state");
    }

    #[test]
    fn discovery_probe_authorization_refuses_unknown_probe() {
        let manifest = valid_manifest();
        let request = DiscoveryRequest {
            pack_id: "ob-poc.kyc".to_string(),
            probe_id: "kyc-case.write-state".to_string(),
            subject: DiscoverySubject {
                subject_kind: "kyc_case".to_string(),
                subject_id: "case-1".to_string(),
            },
            context: BTreeMap::new(),
        };

        let err = authorize_discovery_probe(&manifest, &request).expect_err("probe refused");

        assert_eq!(
            err,
            DiscoveryAuthorizationError::UnknownProbe {
                probe_id: "kyc-case.write-state".to_string()
            }
        );
    }

    #[test]
    fn ob_poc_kyc_seed_pack_parses_and_validates() {
        let path = PathBuf::from(env!("CARGO_MANIFEST_DIR"))
            .join("../../config/sem_os_seeds/domain_packs/ob_poc_kyc.yaml");
        let contents = fs::read_to_string(&path).expect("domain pack readable");
        let manifest: DomainPackManifest =
            serde_yaml::from_str(&contents).expect("domain pack parses");

        let report = manifest.validate();

        assert!(report.valid, "{:?}", report.diagnostics);
        assert_eq!(manifest.pack_id, "ob-poc.kyc");
        assert!(manifest
            .allowed_transitions
            .iter()
            .any(|t| t.verb == "kyc-case.update-status"));
    }
}
