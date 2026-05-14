//! ACP-facing adapter contracts.
//!
//! This module is transport-neutral: it models the lifecycle and safety
//! boundary an ACP server/client binding needs, without depending on a Zed ACP
//! crate or granting mutation capability. The adapter may authorize discovery,
//! assemble redacted Sage context, and request DSL Drafter dry-runs only.

use chrono::{DateTime, Utc};
use sem_os_policy::acp_projection::{
    AcpProjectionEnvelope, AcpProjectionEnvelopeInput, AcpProjectionKind, AcpProjectionSubject,
};
use sem_os_policy::context_policy::{assemble_prompt_context, PromptContextAssembly};
use sem_os_policy::domain_pack::{
    authorize_discovery_probe, AcpPersonaDeclaration, ClassificationLimit,
    DiscoveryAuthorizationError, DiscoveryRequest, DiscoveryResponse, DomainPackManifest,
    ExternalMcpTransport, MentionNamespace, PackCompatibilityTier, ProjectionCatalogEntry,
    ResourceUriScheme, TypedExtensionPoint, WorkflowPhase,
};
use serde::{Deserialize, Serialize};
use serde_json::json;
use std::time::Instant;
use uuid::Uuid;

use crate::kyc_dry_run::{
    build_kyc_update_status_dry_run, KycUpdateStatusDryRunInput, KycUpdateStatusDryRunOutput,
    KycUpdateStatusDryRunRefusal,
};
use crate::language_pack::{
    build_kyc_update_status_language_pack, build_update_status_language_pack,
    transition_language_pack_readiness_report, KycLanguagePackRequest, LanguagePackError,
    SemOsLanguagePack, UpdateStatusLanguagePackRequest,
};
use crate::workbook_revision::{
    run_kyc_update_status_revision_loop, KycUpdateStatusWorkbookDraft, WorkbookRevisionOutcome,
};

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct AcpKycLanguageLoopTimedOutcome {
    pub language_pack: SemOsLanguagePack,
    pub revision_outcome: WorkbookRevisionOutcome,
    pub timings: AcpKycLanguageLoopTimings,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct AcpKycLanguageLoopTimings {
    pub language_pack_ms: u64,
    pub language_pack_us: u64,
    pub revision_loop_ms: u64,
    pub revision_loop_us: u64,
    pub dry_run_ms: u64,
    pub dry_run_us: u64,
    pub total_ms: u64,
    pub total_us: u64,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct AcpSession {
    pub session_id: Uuid,
    pub adapter: AcpAdapterKind,
    pub persona: AcpPersonaMode,
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
pub enum AcpPersonaMode {
    #[serde(rename = "sage:planning")]
    SagePlanning,
    #[serde(rename = "sage:execution")]
    SageExecution,
}

impl AcpPersonaMode {
    pub fn as_str(self) -> &'static str {
        match self {
            Self::SagePlanning => "sage:planning",
            Self::SageExecution => "sage:execution",
        }
    }
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
pub struct AcpKycCaseStateSnapshot {
    pub session_id: Uuid,
    pub pack_id: String,
    pub subject_kind: String,
    pub subject_id: Uuid,
    pub current_state: String,
    pub configuration_version: String,
    pub state_snapshot_id: String,
    #[serde(default)]
    pub snapshot_refs: Vec<String>,
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
    pub workflow_phases: Vec<WorkflowPhase>,
    pub acp_personas: Vec<AcpPersonaDeclaration>,
    pub resource_uri_schemes: Vec<ResourceUriScheme>,
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
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub language_pack_request: Option<UpdateStatusLanguagePackRequest>,
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
    LanguagePackRefused {
        reason: String,
    },
    CaseStateDiscoveryRefused {
        reason: String,
    },
}

pub fn open_acp_session(session_id: Uuid, adapter: AcpAdapterKind) -> AcpSession {
    open_acp_session_with_persona(session_id, adapter, AcpPersonaMode::SagePlanning)
}

pub fn open_acp_session_with_persona(
    session_id: Uuid,
    adapter: AcpAdapterKind,
    persona: AcpPersonaMode,
) -> AcpSession {
    AcpSession {
        session_id,
        adapter,
        persona,
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
                execution_authority: mode.mode_id == AcpPersonaMode::SageExecution.as_str(),
            })
            .collect(),
        workflow_phases: manifest.workflow_phases.clone(),
        acp_personas: manifest.acp_personas.clone(),
        resource_uri_schemes: manifest.resource_uri_schemes.clone(),
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

fn acp_authority_surfaces(manifest: &DomainPackManifest) -> Vec<AcpAuthoritySurfaceDecision> {
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
            surface: "obpoc/language_pack/get".to_string(),
            permitted: true,
            reason: "bounded private DSL language pack is read-only and task-shaped".to_string(),
        },
        AcpAuthoritySurfaceDecision {
            surface: "obpoc/kyc_case_state/discover".to_string(),
            permitted: true,
            reason: "case state anchors are discovered through modeled read-only probes"
                .to_string(),
        },
        AcpAuthoritySurfaceDecision {
            surface: "obpoc/kyc_update_status_language_loop".to_string(),
            permitted: true,
            reason: "deterministic draft validation loop is dry-run only and non-mutating"
                .to_string(),
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
            surface: "fs/write_text_file".to_string(),
            permitted: false,
            reason: "ACP visibility never grants editor file-write authority".to_string(),
        },
        AcpAuthoritySurfaceDecision {
            surface: "create_text_file".to_string(),
            permitted: false,
            reason: "ACP visibility never grants editor file-create authority".to_string(),
        },
        AcpAuthoritySurfaceDecision {
            surface: "terminal/create".to_string(),
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

pub fn acp_discover_kyc_case_state(
    session: &AcpSession,
    manifest: &DomainPackManifest,
    subject_id: Uuid,
    response: DiscoveryResponse,
) -> Result<AcpKycCaseStateSnapshot, AcpAdapterError> {
    require_open(session)?;
    require_valid_pack(manifest)?;

    let request = DiscoveryRequest {
        pack_id: manifest.pack_id.clone(),
        probe_id: "kyc-case.read-state".to_string(),
        subject: sem_os_policy::domain_pack::DiscoverySubject {
            subject_kind: "kyc_case".to_string(),
            subject_id: subject_id.to_string(),
        },
        context: Default::default(),
    };
    authorize_discovery_probe(manifest, &request).map_err(map_discovery_error)?;

    if response.first_class_state_mutated {
        return Err(AcpAdapterError::DiscoveryMutatedState);
    }
    if response.probe_id != request.probe_id {
        return Err(AcpAdapterError::CaseStateDiscoveryRefused {
            reason: format!(
                "expected probe {}, got {}",
                request.probe_id, response.probe_id
            ),
        });
    }
    if response.subject.subject_kind != "kyc_case"
        || response.subject.subject_id != subject_id.to_string()
    {
        return Err(AcpAdapterError::CaseStateDiscoveryRefused {
            reason: "discovery response subject does not match requested KYC case".to_string(),
        });
    }

    let current_state = observation_string(
        &response,
        &["case.status", "kyc_case.status", "current_state"],
    )
    .ok_or_else(|| AcpAdapterError::CaseStateDiscoveryRefused {
        reason: "read-state response did not include case.status".to_string(),
    })?;
    let configuration_version = observation_string(
        &response,
        &["case.configuration_version", "configuration_version"],
    )
    .unwrap_or_else(|| format!("domain_pack:{}@{}", manifest.pack_id, manifest.version));
    let snapshot_refs = response
        .provenance
        .iter()
        .filter_map(|provenance| provenance.snapshot_ref.clone())
        .collect::<Vec<_>>();
    let state_snapshot_id =
        observation_string(&response, &["case.state_snapshot_id", "state_snapshot_id"])
            .or_else(|| snapshot_refs.first().cloned())
            .unwrap_or_else(|| {
                format!(
                    "discovery:{}:{}:{}",
                    response.probe_id, subject_id, configuration_version
                )
            });

    Ok(AcpKycCaseStateSnapshot {
        session_id: session.session_id,
        pack_id: manifest.pack_id.clone(),
        subject_kind: "kyc_case".to_string(),
        subject_id,
        current_state,
        configuration_version,
        state_snapshot_id,
        snapshot_refs,
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

/// Build the canonical ACP projection envelope from the Domain Pack manifest.
///
/// This returns the **declared-source** view of a projection: schema, source
/// reference, classification, and a `declared_projection_source` placeholder
/// payload for kinds that require host materialization. It is transport-neutral
/// and session-data-free.
///
/// HTTP callers that have a live `ReplSessionV2` should overlay live data via
/// `repl_routes_v2::build_live_acp_projection` before calling this — the live
/// overlay returns `Some(envelope)` for kinds with a live implementation and
/// falls through to this function otherwise. Stdio (Zed) callers do not have a
/// live REPL session in scope and use this function directly; they receive the
/// declared-source view, which is the correct ACP discovery surface.
///
/// Phase C of the ACP audit will encapsulate this overlay-then-static pattern
/// behind a single `AcpDomainFacade` so both transports use one entry point.
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
            "language_pack_readiness": transition_language_pack_readiness_report(manifest),
        }),
        AcpProjectionKind::LanguagePack => {
            let language_pack_request = request.language_pack_request.clone().ok_or_else(|| {
                AcpAdapterError::LanguagePackRefused {
                    reason: "language_pack projection requires subject_id, current_state, configuration_version, and state_snapshot_id".to_string(),
                }
            })?;
            serde_json::to_value(
                build_update_status_language_pack(manifest, language_pack_request)
                    .map_err(map_language_pack_error)?,
            )
            .expect("SemOS language pack serializes as ACP projection")
        }
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

pub fn acp_kyc_update_status_language_pack(
    session: &AcpSession,
    manifest: &DomainPackManifest,
    request: KycLanguagePackRequest,
) -> Result<SemOsLanguagePack, AcpAdapterError> {
    require_open(session)?;
    require_valid_pack(manifest)?;
    build_kyc_update_status_language_pack(manifest, request).map_err(map_language_pack_error)
}

pub fn acp_update_status_language_pack(
    session: &AcpSession,
    manifest: &DomainPackManifest,
    request: UpdateStatusLanguagePackRequest,
) -> Result<SemOsLanguagePack, AcpAdapterError> {
    require_open(session)?;
    require_valid_pack(manifest)?;
    build_update_status_language_pack(manifest, request).map_err(map_language_pack_error)
}

pub fn acp_run_kyc_update_status_language_loop(
    session: &AcpSession,
    manifest: &DomainPackManifest,
    request: KycLanguagePackRequest,
    draft: KycUpdateStatusWorkbookDraft,
) -> Result<(SemOsLanguagePack, WorkbookRevisionOutcome), AcpAdapterError> {
    let outcome = acp_run_kyc_update_status_language_loop_timed(session, manifest, request, draft)?;
    Ok((outcome.language_pack, outcome.revision_outcome))
}

pub fn acp_run_kyc_update_status_language_loop_timed(
    session: &AcpSession,
    manifest: &DomainPackManifest,
    request: KycLanguagePackRequest,
    draft: KycUpdateStatusWorkbookDraft,
) -> Result<AcpKycLanguageLoopTimedOutcome, AcpAdapterError> {
    let total_started_at = Instant::now();
    require_open(session)?;
    require_valid_pack(manifest)?;
    if draft.session_id != session.session_id {
        return Err(AcpAdapterError::LanguagePackRefused {
            reason: "draft session_id does not match ACP session".to_string(),
        });
    }
    let language_pack_started_at = Instant::now();
    let language_pack = build_kyc_update_status_language_pack(manifest, request)
        .map_err(map_language_pack_error)?;
    let language_pack_us = elapsed_us(language_pack_started_at);

    let revision_loop_started_at = Instant::now();
    let revision_outcome = run_kyc_update_status_revision_loop(manifest, &language_pack, draft);
    let revision_loop_us = elapsed_us(revision_loop_started_at);
    let dry_run_us = revision_outcome_dry_run_us(&revision_outcome);
    let total_us = elapsed_us(total_started_at);

    Ok(AcpKycLanguageLoopTimedOutcome {
        language_pack,
        revision_outcome,
        timings: AcpKycLanguageLoopTimings {
            language_pack_ms: millis_from_micros(language_pack_us),
            language_pack_us,
            revision_loop_ms: millis_from_micros(revision_loop_us),
            revision_loop_us,
            dry_run_ms: millis_from_micros(dry_run_us),
            dry_run_us,
            total_ms: millis_from_micros(total_us),
            total_us,
        },
    })
}

#[cfg(test)]
fn refuse_acp_mutation(session: &AcpSession) -> Result<(), AcpAdapterError> {
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

fn map_language_pack_error(error: LanguagePackError) -> AcpAdapterError {
    AcpAdapterError::LanguagePackRefused {
        reason: format!("{error:?}"),
    }
}

fn revision_outcome_dry_run_us(outcome: &WorkbookRevisionOutcome) -> u64 {
    match outcome {
        WorkbookRevisionOutcome::DryRunValid { metrics, .. }
        | WorkbookRevisionOutcome::Refused { metrics, .. } => metrics.dry_run_us,
    }
}

fn elapsed_us(started_at: Instant) -> u64 {
    u64::try_from(started_at.elapsed().as_micros()).unwrap_or(u64::MAX)
}

fn millis_from_micros(micros: u64) -> u64 {
    micros / 1_000
}

fn observation_string(response: &DiscoveryResponse, keys: &[&str]) -> Option<String> {
    response
        .observations
        .iter()
        .find(|observation| keys.iter().any(|key| observation.key == *key))
        .and_then(|observation| observation.value.as_str().map(str::to_string))
}

#[cfg(test)]
mod tests {
    use super::*;
    use sem_os_policy::domain_pack::{
        ClassificationLimit, DiscoveryObservation, DiscoveryProvenance, DiscoverySubject,
        PackCompatibilityTier,
    };
    use uuid::uuid;

    const SESSION_ID: Uuid = uuid!("aaaaaaaa-aaaa-aaaa-aaaa-aaaaaaaaaaaa");
    const CASE_ID: Uuid = uuid!("11111111-1111-1111-1111-111111111111");

    fn manifest() -> DomainPackManifest {
        serde_yaml::from_str(include_str!(
            "../../../config/sem_os_seeds/domain_packs/ob_poc_kyc.yaml"
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

    fn state_discovery_response() -> DiscoveryResponse {
        DiscoveryResponse {
            probe_id: "kyc-case.read-state".to_string(),
            subject: DiscoverySubject {
                subject_kind: "kyc_case".to_string(),
                subject_id: CASE_ID.to_string(),
            },
            observations: vec![
                DiscoveryObservation {
                    key: "case.status".to_string(),
                    value: serde_json::json!("DISCOVERY"),
                    classification: Some(ClassificationLimit::Internal),
                },
                DiscoveryObservation {
                    key: "case.configuration_version".to_string(),
                    value: serde_json::json!("config-live-1"),
                    classification: Some(ClassificationLimit::Internal),
                },
            ],
            provenance: vec![DiscoveryProvenance {
                source: "sem_os.session_state".to_string(),
                snapshot_ref: Some("snapshot-live-1".to_string()),
            }],
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
    fn acp_discovers_kyc_case_state_from_read_only_probe() {
        let session = open_acp_session(SESSION_ID, AcpAdapterKind::TestHarness);
        let state =
            acp_discover_kyc_case_state(&session, &manifest(), CASE_ID, state_discovery_response())
                .expect("state discovered");

        assert_eq!(state.subject_id, CASE_ID);
        assert_eq!(state.current_state, "DISCOVERY");
        assert_eq!(state.configuration_version, "config-live-1");
        assert_eq!(state.state_snapshot_id, "snapshot-live-1");
    }

    #[test]
    fn acp_case_state_discovery_refuses_missing_status() {
        let session = open_acp_session(SESSION_ID, AcpAdapterKind::TestHarness);
        let mut response = state_discovery_response();
        response.observations.clear();

        let err = acp_discover_kyc_case_state(&session, &manifest(), CASE_ID, response)
            .expect_err("missing status refused");

        assert!(matches!(
            err,
            AcpAdapterError::CaseStateDiscoveryRefused { .. }
        ));
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
                language_pack_request: None,
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
                language_pack_request: None,
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
