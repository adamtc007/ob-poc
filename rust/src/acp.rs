//! ACP-facing adapter contracts.
//!
//! This module is transport-neutral: it models the lifecycle and safety
//! boundary an ACP server/client binding needs, without depending on a Zed ACP
//! crate or granting mutation capability. The adapter may authorize discovery,
//! assemble redacted Sage context, and request DSL Coder dry-runs only.

use chrono::{DateTime, Utc};
use sem_os_core::acp_projection::{
    AcpProjectionEnvelope, AcpProjectionEnvelopeInput, AcpProjectionKind, AcpProjectionSubject,
};
use sem_os_core::context_policy::{assemble_prompt_context, PromptContextAssembly};
use sem_os_core::domain_pack::{
    authorize_discovery_probe, ClassificationLimit, DiscoveryAuthorizationError, DiscoveryRequest,
    DiscoveryResponse, DomainPackManifest, ExternalMcpTransport, MentionNamespace,
    PackCompatibilityTier, ProjectionCatalogEntry, TypedExtensionPoint,
};
use serde::{Deserialize, Serialize};
use serde_json::json;
use uuid::Uuid;

use crate::runbook::{
    build_kyc_update_status_dry_run, KycUpdateStatusDryRunInput, KycUpdateStatusDryRunOutput,
    KycUpdateStatusDryRunRefusal,
};

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct AcpSession {
    pub session_id: Uuid,
    pub adapter: AcpAdapterKind,
    pub state: AcpSessionState,
    pub opened_at: DateTime<Utc>,
    pub mutation_capability: AcpMutationCapability,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum AcpAdapterKind {
    Zed,
    TestHarness,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum AcpSessionState {
    Open,
    Closed,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum AcpMutationCapability {
    None,
}

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct AcpSageContextBundle {
    pub session_id: Uuid,
    pub pack_id: String,
    pub probe_id: String,
    pub prompt_context: PromptContextAssembly,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct AcpPolicyCapabilities {
    pub session_id: Uuid,
    pub pack_id: String,
    pub pack_version: String,
    pub compatibility_tier: PackCompatibilityTier,
    pub adapter_policy: AcpAdapterPolicy,
    pub authority_surfaces: Vec<AcpAuthoritySurfaceDecision>,
    pub projection_catalog: Vec<ProjectionCatalogEntry>,
    pub mention_namespaces: Vec<MentionNamespace>,
    pub declared_modes: Vec<AcpDeclaredModeCapability>,
    pub external_mcp_transports: Vec<ExternalMcpTransport>,
    pub typed_extension_points: Vec<TypedExtensionPoint>,
    pub context_policy: AcpContextPolicyView,
    pub discovery_policy: Vec<AcpDiscoveryPolicyDecision>,
    pub transition_policy: Vec<AcpTransitionPolicyDecision>,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct AcpAdapterPolicy {
    pub adapter: AcpAdapterKind,
    pub direct_mutation_supported: bool,
    pub mutation_boundary: String,
    pub policy_authority: String,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct AcpContextPolicyView {
    pub max_prompt_classification: ClassificationLimit,
    pub allow_external_llm: bool,
    pub required_redactions: Vec<String>,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct AcpDeclaredModeCapability {
    pub mode_id: String,
    pub label: String,
    pub description: String,
    pub discovery_visible: bool,
    pub execution_authority: bool,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct AcpAuthoritySurfaceDecision {
    pub surface: String,
    pub permitted: bool,
    pub reason: String,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct AcpDiscoveryPolicyDecision {
    pub probe_id: String,
    pub operation: String,
    pub target: String,
    pub allowed: bool,
    pub reason: String,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct AcpTransitionPolicyDecision {
    pub transition_ref: String,
    pub verb: String,
    pub from_state: String,
    pub to_state: String,
    pub dry_run_allowed: bool,
    pub mutation_allowed: bool,
    pub hitl_required: bool,
    pub evidence_refs_required: Vec<String>,
    pub mutation_reason: String,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct AcpProjectionRequest {
    pub kind: AcpProjectionKind,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub subject: Option<AcpProjectionSubject>,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(tag = "kind", rename_all = "snake_case")]
pub enum AcpAdapterError {
    SessionClosed,
    PackInvalid {
        reason: String,
    },
    DiscoveryRefused {
        reason: String,
    },
    DiscoveryMutatedState,
    MutationNotSupported,
    ProjectionUnknown {
        projection_kind: String,
    },
    ProjectionSubjectRefused {
        projection_kind: AcpProjectionKind,
        subject_kind: String,
    },
    DryRunRefused {
        refusal: KycUpdateStatusDryRunRefusal,
    },
}

pub fn open_acp_session(session_id: Uuid, adapter: AcpAdapterKind) -> AcpSession {
    AcpSession {
        session_id,
        adapter,
        state: AcpSessionState::Open,
        opened_at: Utc::now(),
        mutation_capability: AcpMutationCapability::None,
    }
}

pub fn close_acp_session(session: &mut AcpSession) {
    session.state = AcpSessionState::Closed;
}

pub fn acp_policy_capabilities(
    session: &AcpSession,
    manifest: &DomainPackManifest,
) -> Result<AcpPolicyCapabilities, AcpAdapterError> {
    require_open(session)?;
    require_valid_pack(manifest)?;

    let discovery_policy = manifest
        .discovery_probes
        .iter()
        .map(|probe| {
            let allowed = probe.idempotent && probe.modeled && !probe.first_class_state_mutation;
            let reason = if allowed {
                "probe is idempotent, modeled, and read-only".to_string()
            } else if probe.first_class_state_mutation {
                "probe would mutate first-class state".to_string()
            } else if !probe.idempotent {
                "probe is not idempotent".to_string()
            } else {
                "probe is not modeled in the Domain Pack".to_string()
            };
            AcpDiscoveryPolicyDecision {
                probe_id: probe.probe_id.clone(),
                operation: probe.operation.clone(),
                target: probe.target.clone(),
                allowed,
                reason,
            }
        })
        .collect();

    let transition_policy = manifest
        .allowed_transitions
        .iter()
        .map(|transition| {
            let tier_allows_mutation = matches!(
                manifest.compatibility_tier,
                PackCompatibilityTier::ReferenceMutation
            );
            let mutation_allowed =
                transition.mutation_enabled && transition.hitl_required && tier_allows_mutation;
            let mutation_reason = if mutation_allowed {
                "mutation requires workbook approval and compiled runbook execution gate"
                    .to_string()
            } else if !tier_allows_mutation {
                "Domain Pack compatibility tier is dry-run only for ACP".to_string()
            } else if !transition.mutation_enabled {
                "transition has mutation_enabled=false".to_string()
            } else {
                "transition mutation requires human-in-the-loop approval".to_string()
            };
            AcpTransitionPolicyDecision {
                transition_ref: transition.transition_ref.clone(),
                verb: transition.verb.clone(),
                from_state: transition.from_state.clone(),
                to_state: transition.to_state.clone(),
                dry_run_allowed: transition.dry_run_enabled,
                mutation_allowed,
                hitl_required: transition.hitl_required,
                evidence_refs_required: transition.evidence_refs_required.clone(),
                mutation_reason,
            }
        })
        .collect();

    Ok(AcpPolicyCapabilities {
        session_id: session.session_id,
        pack_id: manifest.pack_id.clone(),
        pack_version: manifest.version.clone(),
        compatibility_tier: manifest.compatibility_tier,
        adapter_policy: AcpAdapterPolicy {
            adapter: session.adapter,
            direct_mutation_supported: false,
            mutation_boundary: "workbook_approval_and_compiled_runbook_gate".to_string(),
            policy_authority: "SemOS Domain Pack + Workbook + Runbook Gate".to_string(),
        },
        authority_surfaces: acp_authority_surfaces(manifest),
        projection_catalog: manifest.projection_catalog.clone(),
        mention_namespaces: manifest.mention_namespaces.clone(),
        declared_modes: manifest
            .declared_modes
            .iter()
            .map(|mode| AcpDeclaredModeCapability {
                mode_id: mode.mode_id.clone(),
                label: mode.label.clone(),
                description: mode.description.clone(),
                discovery_visible: true,
                execution_authority: false,
            })
            .collect(),
        external_mcp_transports: manifest.external_mcp_transports.clone(),
        typed_extension_points: manifest.typed_extension_points.clone(),
        context_policy: AcpContextPolicyView {
            max_prompt_classification: manifest.classification_policy.max_prompt_classification,
            allow_external_llm: manifest.classification_policy.allow_external_llm,
            required_redactions: manifest.classification_policy.required_redactions.clone(),
        },
        discovery_policy,
        transition_policy,
    })
}

pub fn acp_authority_surfaces(manifest: &DomainPackManifest) -> Vec<AcpAuthoritySurfaceDecision> {
    vec![
        AcpAuthoritySurfaceDecision {
            surface: "session/prompt".to_string(),
            permitted: true,
            reason: "agent-editor conversation may request read-only discovery and planning"
                .to_string(),
        },
        AcpAuthoritySurfaceDecision {
            surface: "obpoc/projections/list".to_string(),
            permitted: true,
            reason: "projection catalogue is declared by the Domain Pack".to_string(),
        },
        AcpAuthoritySurfaceDecision {
            surface: "obpoc/projection/get".to_string(),
            permitted: true,
            reason: "projection payloads are policy-governed and classification tagged".to_string(),
        },
        AcpAuthoritySurfaceDecision {
            surface: "obpoc/context".to_string(),
            permitted: true,
            reason: "Sage context assembly is read-only and redacted by pack policy".to_string(),
        },
        AcpAuthoritySurfaceDecision {
            surface: "obpoc/kyc_update_status_dry_run".to_string(),
            permitted: true,
            reason: "dry-run validates a workbook-shaped transition without mutation".to_string(),
        },
        AcpAuthoritySurfaceDecision {
            surface: "request_permission".to_string(),
            permitted: true,
            reason: "permission requests are limited to HITL attestation metadata".to_string(),
        },
        AcpAuthoritySurfaceDecision {
            surface: "mcp_tunnel".to_string(),
            permitted: manifest
                .external_mcp_transports
                .iter()
                .all(|transport| transport.read_only),
            reason: "external MCP bindings must be declared by the pack and read-only".to_string(),
        },
        AcpAuthoritySurfaceDecision {
            surface: "write_text_file".to_string(),
            permitted: false,
            reason: "ACP visibility never grants editor file-write authority".to_string(),
        },
        AcpAuthoritySurfaceDecision {
            surface: "create_text_file".to_string(),
            permitted: false,
            reason: "ACP visibility never grants editor file-create authority".to_string(),
        },
        AcpAuthoritySurfaceDecision {
            surface: "terminal/new".to_string(),
            permitted: false,
            reason: "terminal execution is outside the ACP discovery surface".to_string(),
        },
        AcpAuthoritySurfaceDecision {
            surface: "obpoc/mutation".to_string(),
            permitted: false,
            reason:
                "mutation is only available through workbook approval and the compiled runbook gate"
                    .to_string(),
        },
    ]
}

pub fn assemble_sage_context_for_acp(
    session: &AcpSession,
    manifest: &DomainPackManifest,
    request: DiscoveryRequest,
    response: DiscoveryResponse,
) -> Result<AcpSageContextBundle, AcpAdapterError> {
    require_open(session)?;
    require_valid_pack(manifest)?;

    authorize_discovery_probe(manifest, &request).map_err(map_discovery_error)?;

    if response.first_class_state_mutated {
        return Err(AcpAdapterError::DiscoveryMutatedState);
    }

    Ok(AcpSageContextBundle {
        session_id: session.session_id,
        pack_id: manifest.pack_id.clone(),
        probe_id: request.probe_id,
        prompt_context: assemble_prompt_context(&manifest.classification_policy, &response),
    })
}

pub fn list_acp_projections(
    session: &AcpSession,
    manifest: &DomainPackManifest,
) -> Result<Vec<ProjectionCatalogEntry>, AcpAdapterError> {
    require_open(session)?;
    require_valid_pack(manifest)?;
    Ok(manifest.projection_catalog.clone())
}

pub fn build_acp_projection(
    session: &AcpSession,
    manifest: &DomainPackManifest,
    request: AcpProjectionRequest,
) -> Result<AcpProjectionEnvelope, AcpAdapterError> {
    require_open(session)?;
    require_valid_pack(manifest)?;

    let catalog_entry = manifest
        .projection_catalog
        .iter()
        .find(|entry| entry.kind == request.kind)
        .ok_or_else(|| AcpAdapterError::ProjectionUnknown {
            projection_kind: request.kind.as_str().to_string(),
        })?;

    if let Some(subject) = request.subject.as_ref() {
        if !catalog_entry.allowed_subject_kinds.is_empty()
            && !catalog_entry
                .allowed_subject_kinds
                .iter()
                .any(|kind| kind == &subject.subject_kind)
        {
            return Err(AcpAdapterError::ProjectionSubjectRefused {
                projection_kind: request.kind,
                subject_kind: subject.subject_kind.clone(),
            });
        }
    }

    let policy = acp_policy_capabilities(session, manifest)?;
    let payload = match request.kind {
        AcpProjectionKind::PackManifest => serde_json::to_value(manifest)
            .expect("Domain Pack manifest serializes as ACP projection"),
        AcpProjectionKind::ProbeCatalogue => json!({
            "probes": manifest.discovery_probes,
            "policy": policy.discovery_policy,
        }),
        AcpProjectionKind::Policy => {
            serde_json::to_value(policy).expect("ACP policy serializes as projection")
        }
        AcpProjectionKind::TransitionSurface => json!({
            "transitions": manifest.allowed_transitions,
            "policy": acp_policy_capabilities(session, manifest)?.transition_policy,
        }),
        AcpProjectionKind::DiscoverySurface
        | AcpProjectionKind::WorkspaceState
        | AcpProjectionKind::Dag
        | AcpProjectionKind::GraphScene
        | AcpProjectionKind::VerbSurface
        | AcpProjectionKind::Governance
        | AcpProjectionKind::EvidenceSchema
        | AcpProjectionKind::AffinityGraph
        | AcpProjectionKind::Lineage
        | AcpProjectionKind::DerivationRegistry
        | AcpProjectionKind::Materiality => json!({
            "status": "declared_projection_source",
            "source": catalog_entry.source,
            "source_binding": "live_semos_session_projection",
            "note": "Projection is declared in the Domain Pack and must be materialized by the host adapter from the named SemOS source.",
            "max_depth": catalog_entry.max_depth,
            "acp_visible_by_default": catalog_entry.acp_visible_by_default,
        }),
    };

    Ok(AcpProjectionEnvelope::new(AcpProjectionEnvelopeInput {
        projection_kind: request.kind,
        session_id: session.session_id,
        pack_id: manifest.pack_id.clone(),
        classification: catalog_entry.default_classification,
        subject: request.subject,
        snapshot_refs: vec![format!(
            "domain_pack:{}@{}",
            manifest.pack_id, manifest.version
        )],
        payload,
        redactions: vec![],
    }))
}

pub fn acp_dry_run_kyc_update_status(
    session: &AcpSession,
    input: KycUpdateStatusDryRunInput,
) -> Result<KycUpdateStatusDryRunOutput, AcpAdapterError> {
    require_open(session)?;

    if input.session_id != session.session_id {
        return Err(AcpAdapterError::DryRunRefused {
            refusal: KycUpdateStatusDryRunRefusal::WorkbookRefused {
                reason: "dry-run input session does not match ACP session".to_string(),
            },
        });
    }

    build_kyc_update_status_dry_run(input)
        .map_err(|refusal| AcpAdapterError::DryRunRefused { refusal })
}

pub fn refuse_acp_mutation(session: &AcpSession) -> Result<(), AcpAdapterError> {
    require_open(session)?;
    Err(AcpAdapterError::MutationNotSupported)
}

fn require_open(session: &AcpSession) -> Result<(), AcpAdapterError> {
    if session.state == AcpSessionState::Closed {
        return Err(AcpAdapterError::SessionClosed);
    }
    Ok(())
}

fn require_valid_pack(manifest: &DomainPackManifest) -> Result<(), AcpAdapterError> {
    let validation = manifest.validate();
    if validation.valid {
        return Ok(());
    }
    Err(AcpAdapterError::PackInvalid {
        reason: validation
            .diagnostics
            .iter()
            .map(|d| format!("{}: {}", d.code, d.message))
            .collect::<Vec<_>>()
            .join("; "),
    })
}

fn map_discovery_error(error: DiscoveryAuthorizationError) -> AcpAdapterError {
    AcpAdapterError::DiscoveryRefused {
        reason: match error {
            DiscoveryAuthorizationError::PackMismatch { expected, actual } => {
                format!("pack mismatch: expected {expected}, got {actual}")
            }
            DiscoveryAuthorizationError::UnknownProbe { probe_id } => {
                format!("unknown probe {probe_id}")
            }
            DiscoveryAuthorizationError::UnsafeProbe { probe_id, code } => {
                format!("unsafe probe {probe_id}: {code}")
            }
        },
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use sem_os_core::domain_pack::{
        ClassificationLimit, DiscoveryObservation, DiscoverySubject, PackCompatibilityTier,
    };
    use uuid::uuid;

    const SESSION_ID: Uuid = uuid!("aaaaaaaa-aaaa-aaaa-aaaa-aaaaaaaaaaaa");
    const CASE_ID: Uuid = uuid!("11111111-1111-1111-1111-111111111111");

    fn manifest() -> DomainPackManifest {
        serde_yaml::from_str(include_str!(
            "../config/sem_os_seeds/domain_packs/ob_poc_kyc.yaml"
        ))
        .expect("domain pack parses")
    }

    fn discovery_request(probe_id: &str) -> DiscoveryRequest {
        DiscoveryRequest {
            pack_id: "ob-poc.kyc".to_string(),
            probe_id: probe_id.to_string(),
            subject: DiscoverySubject {
                subject_kind: "kyc_case".to_string(),
                subject_id: CASE_ID.to_string(),
            },
            context: Default::default(),
        }
    }

    fn discovery_response() -> DiscoveryResponse {
        DiscoveryResponse {
            probe_id: "kyc-case.read-evidence-summary".to_string(),
            subject: DiscoverySubject {
                subject_kind: "kyc_case".to_string(),
                subject_id: CASE_ID.to_string(),
            },
            observations: vec![
                DiscoveryObservation {
                    key: "case.status".to_string(),
                    value: serde_json::json!("INTAKE"),
                    classification: Some(ClassificationLimit::Internal),
                },
                DiscoveryObservation {
                    key: "case.confidential_evidence.summary".to_string(),
                    value: serde_json::json!("raw evidence"),
                    classification: Some(ClassificationLimit::Internal),
                },
            ],
            provenance: vec![],
            first_class_state_mutated: false,
        }
    }

    fn dry_run_input() -> KycUpdateStatusDryRunInput {
        KycUpdateStatusDryRunInput {
            session_id: SESSION_ID,
            case_id: CASE_ID,
            actor_id: "analyst@example.com".to_string(),
            actor_roles: vec!["analyst".to_string()],
            transition_ref: "kyc-case.intake-to-discovery".to_string(),
            current_state: "INTAKE".to_string(),
            requested_state: "DISCOVERY".to_string(),
            configuration_version: "config-1".to_string(),
            state_snapshot_id: "state-snapshot-1".to_string(),
            evidence_digest: "sha256:case".to_string(),
            llm_trace_ref: None,
        }
    }

    #[test]
    fn acp_session_opens_without_mutation_capability() {
        let session = open_acp_session(SESSION_ID, AcpAdapterKind::Zed);

        assert_eq!(session.state, AcpSessionState::Open);
        assert_eq!(session.mutation_capability, AcpMutationCapability::None);
        assert!(matches!(
            refuse_acp_mutation(&session),
            Err(AcpAdapterError::MutationNotSupported)
        ));
    }

    #[test]
    fn acp_context_authorizes_probe_and_redacts_prompt_context() {
        let session = open_acp_session(SESSION_ID, AcpAdapterKind::TestHarness);
        let bundle = assemble_sage_context_for_acp(
            &session,
            &manifest(),
            discovery_request("kyc-case.read-evidence-summary"),
            discovery_response(),
        )
        .expect("context assembled");

        assert_eq!(bundle.pack_id, "ob-poc.kyc");
        assert_eq!(bundle.prompt_context.included.len(), 1);
        assert_eq!(bundle.prompt_context.included[0].key, "case.status");
        assert_eq!(bundle.prompt_context.redacted.len(), 1);
        assert_eq!(
            bundle.prompt_context.redacted[0].key,
            "case.confidential_evidence.summary"
        );
        assert!(!bundle.prompt_context.external_llm_allowed);
    }

    #[test]
    fn acp_policy_capabilities_exposes_semos_decisions() {
        let session = open_acp_session(SESSION_ID, AcpAdapterKind::Zed);
        let policy = acp_policy_capabilities(&session, &manifest()).expect("policy");

        assert_eq!(policy.pack_id, "ob-poc.kyc");
        assert!(!policy.adapter_policy.direct_mutation_supported);
        assert_eq!(
            policy.adapter_policy.mutation_boundary,
            "workbook_approval_and_compiled_runbook_gate"
        );
        assert!(policy
            .discovery_policy
            .iter()
            .any(|probe| probe.probe_id == "kyc-case.read-evidence-summary" && probe.allowed));
        assert!(policy.transition_policy.iter().any(|transition| {
            transition.transition_ref == "kyc-case.intake-to-discovery"
                && transition.dry_run_allowed
                && !transition.mutation_allowed
        }));
    }

    #[test]
    fn acp_projection_catalog_exposes_declared_visibility_surface() {
        let session = open_acp_session(SESSION_ID, AcpAdapterKind::Zed);
        let projections = list_acp_projections(&session, &manifest()).expect("projection catalog");

        assert!(projections
            .iter()
            .any(|entry| entry.kind == AcpProjectionKind::Dag));
        assert!(projections
            .iter()
            .any(|entry| entry.kind == AcpProjectionKind::VerbSurface));
        assert!(
            projections
                .iter()
                .any(|entry| entry.kind == AcpProjectionKind::Lineage
                    && !entry.acp_visible_by_default)
        );
    }

    #[test]
    fn acp_pack_manifest_projection_is_typed_and_hashed() {
        let session = open_acp_session(SESSION_ID, AcpAdapterKind::Zed);
        let envelope = build_acp_projection(
            &session,
            &manifest(),
            AcpProjectionRequest {
                kind: AcpProjectionKind::PackManifest,
                subject: None,
            },
        )
        .expect("projection");

        assert_eq!(envelope.projection_kind, AcpProjectionKind::PackManifest);
        assert_eq!(envelope.pack_id, "ob-poc.kyc");
        assert!(envelope.projection_hash.starts_with("sha256:"));
        assert_eq!(envelope.payload["pack_id"], "ob-poc.kyc");
    }

    #[test]
    fn acp_projection_refuses_disallowed_subject_kind() {
        let session = open_acp_session(SESSION_ID, AcpAdapterKind::Zed);
        let err = build_acp_projection(
            &session,
            &manifest(),
            AcpProjectionRequest {
                kind: AcpProjectionKind::TransitionSurface,
                subject: Some(AcpProjectionSubject {
                    subject_kind: "deal".to_string(),
                    subject_id: "deal-1".to_string(),
                }),
            },
        )
        .expect_err("subject refused");

        assert!(matches!(
            err,
            AcpAdapterError::ProjectionSubjectRefused { .. }
        ));
    }

    #[test]
    fn acp_context_refuses_unknown_probe() {
        let session = open_acp_session(SESSION_ID, AcpAdapterKind::TestHarness);
        let err = assemble_sage_context_for_acp(
            &session,
            &manifest(),
            discovery_request("kyc-case.write-state"),
            discovery_response(),
        )
        .expect_err("probe refused");

        assert!(matches!(err, AcpAdapterError::DiscoveryRefused { .. }));
    }

    #[test]
    fn acp_context_refuses_discovery_that_mutated_state() {
        let session = open_acp_session(SESSION_ID, AcpAdapterKind::TestHarness);
        let mut response = discovery_response();
        response.first_class_state_mutated = true;

        let err = assemble_sage_context_for_acp(
            &session,
            &manifest(),
            discovery_request("kyc-case.read-evidence-summary"),
            response,
        )
        .expect_err("mutating discovery refused");

        assert_eq!(err, AcpAdapterError::DiscoveryMutatedState);
    }

    #[test]
    fn acp_dry_run_builds_workbook_without_mutation() {
        let session = open_acp_session(SESSION_ID, AcpAdapterKind::TestHarness);
        let output =
            acp_dry_run_kyc_update_status(&session, dry_run_input()).expect("dry-run succeeds");

        assert_eq!(
            output.workbook.core.transition_ref,
            "kyc-case.intake-to-discovery"
        );
        assert_eq!(output.dry_run.semantic_diff.to_state, "DISCOVERY");
    }

    #[test]
    fn acp_refuses_closed_session() {
        let mut session = open_acp_session(SESSION_ID, AcpAdapterKind::TestHarness);
        close_acp_session(&mut session);

        let err =
            acp_dry_run_kyc_update_status(&session, dry_run_input()).expect_err("closed refused");

        assert_eq!(err, AcpAdapterError::SessionClosed);
    }

    #[test]
    fn acp_refuses_mutation_enabled_pack_in_dry_run_tier() {
        let session = open_acp_session(SESSION_ID, AcpAdapterKind::TestHarness);
        let mut manifest = manifest();
        manifest.compatibility_tier = PackCompatibilityTier::DryRunOnly;
        manifest.allowed_transitions[0].mutation_enabled = true;

        let err = assemble_sage_context_for_acp(
            &session,
            &manifest,
            discovery_request("kyc-case.read-evidence-summary"),
            discovery_response(),
        )
        .expect_err("invalid pack refused");

        assert!(matches!(err, AcpAdapterError::PackInvalid { .. }));
    }
}
