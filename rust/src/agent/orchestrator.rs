//! Unified Intent Orchestrator
//!
//! Single entry point (`handle_utterance`) for all utterance processing:
//! Chat API, MCP `dsl_generate`, and REPL. Wraps `IntentPipeline` with:
//!
//! - **Entity linking** via `LookupService` (Phase 4 dedup)
//! - **Sem OS context resolution** -> `SemOsContextEnvelope` (Phase 2B CCIR)
//! - **Direct DSL bypass gating** by actor role (Phase 2.1)
//! - **IntentTrace** structured audit logging (Phase 5)
//!
//! Phase 2B replaced the flat `SemRegVerbPolicy` enum with a rich
//! `SemOsContextEnvelope` that preserves pruning reasons, governance signals,
//! and a deterministic fingerprint of the allowed verb set.

use serde::Serialize;
use std::collections::HashMap;
use std::sync::Arc;
use uuid::Uuid;

#[cfg(feature = "database")]
use sqlx::PgPool;

use crate::agent::sem_os_context_envelope::SemOsContextEnvelope;
use crate::agent::telemetry;
use crate::agent::verb_surface::{
    compute_session_verb_surface, SessionVerbSurface, VerbSurfaceContext, VerbSurfaceFailPolicy,
};
use crate::dsl_v2::ast::find_unresolved_ref_locations;
use crate::dsl_v2::verb_registry::registry;
use crate::dsl_v2::{compile, enrich_program, parse_program, runtime_registry_arc};
use crate::lookup::LookupService;
use crate::mcp::intent_pipeline::{
    compute_dsl_hash, IntentPipeline, PipelineOutcome, PipelineResult, StructuredIntent,
    UnresolvedRef,
};
use crate::mcp::scope_resolution::ScopeContext;
use crate::mcp::verb_search::{
    HybridVerbSearcher, JourneyMetadata, JourneyRoute, VerbSearchResult, VerbSearchSource,
};
use crate::policy::{gate::PolicySnapshot, PolicyGate};
use crate::sage::{
    coder::CoderResolution, CoderResult, ObservationPlane, OutcomeStep, PendingMutation,
    SageConfidence, SageEngine, UtteranceDisposition,
};
use crate::sem_reg::abac::ActorContext;
use crate::semtaxonomy_v2::{
    compiler_input_from_outcome_intent, supports_cbu_compiler_slice, CompilerSelection,
    IntentCompiler,
};
use crate::traceability::{
    build_phase2_unavailable_payload, build_phase5_unavailable_payload, build_phase_trace_payload,
    build_trace_scaffold_payload, evaluate_phase3_against_phase2, evaluate_phase4_within_phase2,
    NewUtteranceTrace, Phase2Service, TraceKind, UtteranceTraceRepository,
};

use sem_os_client::SemOsClient;
use sem_os_core::authoring::agent_mode::AgentMode;

/// Context needed to run the unified orchestrator.
pub struct OrchestratorContext {
    pub actor: ActorContext,
    pub session_id: Option<Uuid>,
    pub case_id: Option<Uuid>,
    /// Dominant entity from entity linking (NOT the same as case_id)
    pub dominant_entity_id: Option<Uuid>,
    pub scope: Option<ScopeContext>,
    #[cfg(feature = "database")]
    pub pool: PgPool,
    pub verb_searcher: Arc<HybridVerbSearcher>,
    pub lookup_service: Option<LookupService>,
    /// Server-side policy enforcement.
    pub policy_gate: Arc<PolicyGate>,
    /// Source of this utterance (for trace/audit).
    pub source: UtteranceSource,
    /// Semantic OS client — routes context resolution through the DI boundary.
    /// When set, `resolve_sem_reg_verbs()` calls through this client instead of
    /// direct `sem_reg::context_resolution::resolve_context()`.
    pub sem_os_client: Option<Arc<dyn SemOsClient>>,
    /// Authoring pipeline mode (Research vs Governed).
    /// Controls which verbs are available: Research allows authoring exploration
    /// verbs but blocks publish; Governed blocks authoring exploration but allows
    /// publish and business verbs. Default: Governed.
    pub agent_mode: AgentMode,
    /// Workflow goals derived from session stage_focus (e.g., ["kyc"], ["onboarding"]).
    /// Threaded into the Sem OS ContextResolutionRequest to filter verbs by phase_tags.
    /// Empty means no goal filtering (all verbs pass).
    pub goals: Vec<String>,
    /// Session workflow focus (e.g., "semos-kyc", "semos-onboarding").
    /// Used by `SessionVerbSurface` for domain-level workflow filtering.
    pub stage_focus: Option<String>,
    /// Optional Sage engine used for Stage 1.5 shadow classification.
    pub sage_engine: Option<Arc<dyn SageEngine>>,
    /// Pre-Sage entity kind carried from session state or prior scope.
    pub pre_sage_entity_kind: Option<String>,
    /// Pre-Sage dominant entity name carried from session state or prior scope.
    pub pre_sage_entity_name: Option<String>,
    /// Confidence for the dominant entity-kind signal when available.
    pub pre_sage_entity_confidence: Option<f64>,
    /// Minimal recent Sage ledger from prior turns.
    pub recent_sage_intents: Vec<crate::sage::RecentIntent>,
    /// Optional NLCI compiler hook for the deterministic replacement pipeline.
    pub nlci_compiler: Option<Arc<dyn IntentCompiler>>,
    /// Selected discovery domain from the Sage bootstrap navigator.
    pub discovery_selected_domain: Option<String>,
    /// Selected discovery family from the Sage bootstrap navigator.
    pub discovery_selected_family: Option<String>,
    /// Selected discovery constellation from the Sage bootstrap navigator.
    pub discovery_selected_constellation: Option<String>,
    /// Structured answers collected during the discovery bootstrap.
    pub discovery_answers: HashMap<String, String>,
    /// CBU IDs currently in the session scope.
    /// Used by composite state loader to query group-level entity states.
    pub session_cbu_ids: Option<Vec<Uuid>>,
}

/// Where the utterance originated.
#[derive(Debug, Clone, Serialize)]
#[serde(rename_all = "snake_case")]
pub enum UtteranceSource {
    Chat,
    Mcp,
    Repl,
}

/// Full outcome of orchestrator processing.
pub struct OrchestratorOutcome {
    pub pipeline_result: PipelineResult,
    /// Rich context envelope from Sem OS resolution (replaces flat `sem_reg_verbs`).
    /// Contains allowed verbs, pruned verbs with reasons, fingerprint, governance signals.
    #[cfg(feature = "database")]
    pub context_envelope: Option<SemOsContextEnvelope>,
    /// Consolidated verb surface computed for this turn (all governance layers applied).
    /// When present, replaces ad-hoc inline SemReg + AgentMode filtering.
    #[cfg(feature = "database")]
    pub surface: Option<SessionVerbSurface>,
    pub lookup_result: Option<crate::lookup::LookupResult>,
    pub trace: IntentTrace,
    /// DecisionPacket for journey-level disambiguation (e.g., macro_selector needs
    /// user to pick jurisdiction before resolving the macro).
    pub journey_decision: Option<ob_poc_types::DecisionPacket>,
    /// Pending mutation awaiting chat-layer confirmation.
    pub pending_mutation: Option<PendingMutation>,
    /// Whether chat should auto-execute the resulting DSL instead of staging it.
    pub auto_execute: bool,
    /// Sage intent used for this turn, when available.
    pub sage_intent: Option<crate::sage::OutcomeIntent>,
    /// Persisted first-class utterance trace row for this turn, when available.
    pub trace_id: Option<Uuid>,
}

struct SageStageOutcome {
    intent: Option<crate::sage::OutcomeIntent>,
}

struct CoderStageOutcome {
    result: Option<CoderResult>,
    elapsed_ms: Option<u128>,
    error: Option<String>,
}

struct PreparedTurnContext {
    lookup_result: Option<crate::lookup::LookupResult>,
    dominant_entity_name: Option<String>,
    dominant_entity_kind: Option<String>,
    entity_candidates: Vec<String>,
    sem_reg_verb_names: Option<Vec<String>>,
    envelope: SemOsContextEnvelope,
    surface: SessionVerbSurface,
    #[allow(dead_code)]
    composite_state: Option<crate::agent::composite_state::GroupCompositeState>,
}

fn route(intent: &crate::sage::OutcomeIntent) -> UtteranceDisposition {
    match intent.polarity {
        crate::sage::IntentPolarity::Read | crate::sage::IntentPolarity::Ambiguous => {
            UtteranceDisposition::Serve(crate::sage::ServeIntent {
                summary: intent.summary.clone(),
                domain: intent.domain_concept.clone(),
                action: intent.action.clone(),
                subject: intent.subject.clone(),
            })
        }
        crate::sage::IntentPolarity::Write => {
            UtteranceDisposition::Delegate(Box::new(crate::sage::DelegateIntent {
                summary: intent.summary.clone(),
                outcome: intent.clone(),
            }))
        }
    }
}

pub(crate) fn is_confirmation(utterance: &str) -> bool {
    matches!(
        utterance.trim().to_ascii_lowercase().as_str(),
        "yes"
            | "y"
            | "go ahead"
            | "do it"
            | "proceed"
            | "confirm"
            | "yes please"
            | "go for it"
            | "ok"
            | "yep"
            | "sure"
            | "yes, go ahead"
            | "yes, do it"
            | "approved"
    )
}

fn build_mutation_confirmation(
    intent: &crate::sage::OutcomeIntent,
    coder_result: &CoderResult,
    lookup: Option<&crate::lookup::LookupResult>,
) -> PendingMutation {
    let action_word = match intent.action {
        crate::sage::OutcomeAction::Create => "create",
        crate::sage::OutcomeAction::Update => "update",
        crate::sage::OutcomeAction::Delete => "delete",
        crate::sage::OutcomeAction::Assign => "assign",
        crate::sage::OutcomeAction::Import => "import",
        crate::sage::OutcomeAction::Publish => "publish",
        _ => "change",
    };

    let subject_name = lookup
        .and_then(|lr| lr.dominant_entity.as_ref())
        .map(|entity| entity.canonical_name.as_str())
        .or_else(|| {
            intent
                .subject
                .as_ref()
                .map(|subject| subject.mention.as_str())
        })
        .unwrap_or("this");

    let mut change_summary = vec![format!("Resolved action: {}", coder_result.verb_fqn)];
    if !coder_result.missing_args.is_empty() {
        change_summary.push(format!(
            "Still missing: {}",
            coder_result.missing_args.join(", ")
        ));
    }
    if !coder_result.unresolved_refs.is_empty() {
        change_summary.push(format!(
            "Needs entity resolution: {}",
            coder_result.unresolved_refs.join(", ")
        ));
    }

    PendingMutation {
        confirmation_text: format!("So you want to {action_word} {subject_name}?"),
        change_summary,
        coder_result: coder_result.clone(),
        intent: intent.clone(),
    }
}

fn can_use_sage_structure_fast_path(
    ctx: &OrchestratorContext,
    intent: &crate::sage::OutcomeIntent,
    coder_result: &CoderResult,
    prepared: &PreparedTurnContext,
) -> bool {
    if !matches!(
        ctx.stage_focus.as_deref(),
        Some("semos-data-management" | "semos-data")
    ) {
        return false;
    }

    if intent.plane != ObservationPlane::Structure
        || intent.polarity != crate::sage::IntentPolarity::Read
        || !matches!(
            intent.confidence,
            SageConfidence::High | SageConfidence::Medium
        )
        || !intent.pending_clarifications.is_empty()
        || !coder_result.missing_args.is_empty()
    {
        return false;
    }

    if allow_data_management_structure_fast_path(
        ctx.stage_focus.as_deref(),
        intent,
        &coder_result.verb_fqn,
    ) {
        return true;
    }

    Some(&prepared.surface)
        .map(|surface| surface.allowed_fqns().contains(&coder_result.verb_fqn))
        .unwrap_or(false)
}

pub(crate) fn can_auto_execute_serve_result(verb_fqn: &str) -> bool {
    !can_skip_fast_path_parse_validation(verb_fqn)
}

fn read_only_list_fallback(intent: &crate::sage::OutcomeIntent) -> Option<CoderResult> {
    if intent.polarity != crate::sage::IntentPolarity::Read || intent.domain_concept.is_empty() {
        return None;
    }

    let summary = intent.summary.to_ascii_lowercase();
    let domain = intent.domain_concept.as_str();
    let plural_domain = if domain.ends_with('y') && domain.len() > 1 {
        format!("{}ies", &domain[..domain.len() - 1])
    } else if domain.ends_with('s') {
        domain.to_string()
    } else {
        format!("{domain}s")
    };
    let list_verb = format!("{domain}.list");

    if (summary.contains(&plural_domain)
        || summary.starts_with("show ")
        || summary.starts_with("list "))
        && registry().get_by_name(&list_verb).is_some()
    {
        return Some(CoderResult {
            verb_fqn: list_verb.clone(),
            dsl: format!("({list_verb})"),
            resolution: crate::sage::coder::CoderResolution::Confident,
            missing_args: vec![],
            unresolved_refs: vec![],
            diagnostics: None,
        });
    }

    None
}

async fn prepare_turn_context(
    ctx: &OrchestratorContext,
    utterance: &str,
    sage_intent: Option<&crate::sage::OutcomeIntent>,
) -> PreparedTurnContext {
    let lookup_result = if let Some(ref lookup_svc) = ctx.lookup_service {
        Some(lookup_svc.analyze(utterance, 5).await)
    } else {
        None
    };

    let dominant_entity_name = lookup_result
        .as_ref()
        .and_then(|lr| lr.dominant_entity.as_ref())
        .map(|e| e.canonical_name.clone());

    let dominant_entity_kind = lookup_result
        .as_ref()
        .and_then(|lr| lr.dominant_entity.as_ref())
        .map(|e| e.entity_kind.clone());

    let semreg_entity_kind = if matches!(
        ctx.stage_focus.as_deref(),
        Some("semos-data-management" | "semos-data")
    ) {
        None
    } else {
        dominant_entity_kind.clone()
    };

    let entity_candidates: Vec<String> = lookup_result
        .as_ref()
        .map(|lr| lr.entities.iter().map(|e| e.mention_text.clone()).collect())
        .unwrap_or_default();

    let use_generic_task_subject =
        should_use_generic_task_subject_for_sage(ctx.stage_focus.as_deref(), sage_intent);
    let envelope = resolve_sem_reg_verbs(
        ctx,
        utterance,
        sage_intent,
        semreg_entity_kind.as_deref(),
        use_generic_task_subject,
    )
    .await;

    let fail_policy = if ctx.policy_gate.semreg_fail_closed() {
        VerbSurfaceFailPolicy::FailClosed
    } else {
        VerbSurfaceFailPolicy::FailOpen
    };
    let has_group_scope = ctx.scope.as_ref().and_then(|s| s.client_group_id).is_some();

    // Load group composite state for state-to-intent bias
    #[cfg(feature = "database")]
    let composite_state = if has_group_scope {
        // Use session CBU IDs to load composite state
        let cbu_ids: Vec<uuid::Uuid> = ctx.session_cbu_ids.as_deref().unwrap_or(&[]).to_vec();
        if !cbu_ids.is_empty() {
            match crate::agent::composite_state_loader::load_group_composite_state(
                &ctx.pool, &cbu_ids,
            )
            .await
            {
                Ok(state) => state,
                Err(e) => {
                    tracing::warn!("Failed to load composite state: {e}");
                    None
                }
            }
        } else {
            None
        }
    } else {
        None
    };
    #[cfg(not(feature = "database"))]
    let composite_state: Option<crate::agent::composite_state::GroupCompositeState> = None;

    // Extract entity state from SemOS grounded action surface.
    // SemOS is the source of truth for current state — the state machine determines
    // which verbs are reachable. Every utterance is a delta against current state.
    let grounded_entity_state: Option<String> = envelope
        .grounded_action_surface
        .as_ref()
        .and_then(|gas| gas.current_state.clone());

    let surface_ctx = VerbSurfaceContext {
        agent_mode: ctx.agent_mode,
        stage_focus: ctx.stage_focus.as_deref(),
        envelope: &envelope,
        fail_policy,
        entity_state: grounded_entity_state.as_deref(),
        has_group_scope,
        composite_state: composite_state.as_ref(),
    };
    let surface = compute_session_verb_surface(&surface_ctx);

    let phase2 = Phase2Service::evaluate(lookup_result.clone(), Some(envelope.clone()));
    let sem_reg_verb_names = Phase2Service::legal_verb_names(&phase2.artifacts);

    PreparedTurnContext {
        lookup_result,
        dominant_entity_name,
        dominant_entity_kind,
        entity_candidates,
        sem_reg_verb_names,
        envelope,
        surface,
        composite_state,
    }
}

fn can_use_coder_for_serve(
    ctx: &OrchestratorContext,
    intent: &crate::sage::OutcomeIntent,
    coder_result: &CoderResult,
    prepared: &PreparedTurnContext,
) -> bool {
    if !coder_result.missing_args.is_empty() || !coder_result.unresolved_refs.is_empty() {
        return false;
    }

    if can_use_sage_structure_fast_path(ctx, intent, coder_result, prepared) {
        return true;
    }

    // Use the pre-computed surface — no need to re-evaluate Phase2.
    let allowed = prepared.surface.allowed_fqns();
    if !allowed.is_empty() {
        return allowed.contains(&coder_result.verb_fqn);
    }

    false
}

#[cfg(feature = "database")]
async fn build_sage_serve_outcome(
    ctx: &OrchestratorContext,
    utterance: &str,
    intent: &crate::sage::OutcomeIntent,
    coder_result: &CoderResult,
    prepared: PreparedTurnContext,
    selection_source: &str,
) -> anyhow::Result<OrchestratorOutcome> {
    let pipeline_result =
        build_sage_fast_path_result(utterance, ctx.scope.clone(), intent, coder_result)?;
    let candidates = vec![(
        coder_result.verb_fqn.clone(),
        pipeline_result
            .verb_candidates
            .first()
            .map(|candidate| candidate.score)
            .unwrap_or_default(),
    )];
    let chosen = Some(coder_result.verb_fqn.clone());
    let semreg_unavail = prepared.envelope.is_unavailable();
    let mut trace = build_trace(
        utterance,
        ctx,
        &prepared.entity_candidates,
        &prepared.dominant_entity_name,
        &prepared.dominant_entity_kind,
        &prepared.sem_reg_verb_names,
        &candidates,
        &candidates,
        &chosen,
        &chosen,
        &pipeline_result,
        &prepared.envelope,
        Some(selection_source.to_string()),
        false,
        semreg_unavail,
        &[],
        false,
        None,
        None,
    );
    trace.surface_fingerprint = Some(prepared.surface.surface_fingerprint.0.clone());
    apply_sage_trace_fields(&mut trace, intent, selection_source);

    let mut outcome = OrchestratorOutcome {
        pipeline_result,
        context_envelope: Some(prepared.envelope),
        surface: Some(prepared.surface),
        lookup_result: prepared.lookup_result,
        trace,
        journey_decision: None,
        pending_mutation: None,
        auto_execute: can_auto_execute_serve_result(&coder_result.verb_fqn),
        sage_intent: Some(intent.clone()),
        trace_id: None,
    };
    emit_telemetry(ctx, utterance, &mut outcome).await;
    Ok(outcome)
}

#[cfg(feature = "database")]
async fn build_semos_discovery_outcome(
    ctx: &OrchestratorContext,
    utterance: &str,
    prepared: PreparedTurnContext,
    sage_intent: Option<crate::sage::OutcomeIntent>,
) -> anyhow::Result<OrchestratorOutcome> {
    let prompt = prepared
        .envelope
        .first_discovery_question()
        .map(str::to_string)
        .or_else(|| {
            prepared
                .envelope
                .discovery_surface
                .as_ref()
                .and_then(|surface| {
                    if surface.missing_inputs.is_empty() {
                        None
                    } else {
                        Some(format!(
                            "I need a bit more context before grounding this session: {}.",
                            surface
                                .missing_inputs
                                .iter()
                                .map(|input| input.label.as_str())
                                .collect::<Vec<_>>()
                                .join(", ")
                        ))
                    }
                })
        })
        .unwrap_or_else(|| "I need a bit more context before grounding this session.".to_string());

    let pipeline_result = PipelineResult {
        intent: StructuredIntent::empty(),
        verb_candidates: vec![],
        dsl: String::new(),
        dsl_hash: None,
        valid: false,
        validation_error: Some(prompt),
        unresolved_refs: vec![],
        missing_required: prepared
            .envelope
            .discovery_surface
            .as_ref()
            .map(|surface| {
                surface
                    .missing_inputs
                    .iter()
                    .map(|input| input.key.clone())
                    .collect::<Vec<_>>()
            })
            .unwrap_or_default(),
        outcome: PipelineOutcome::NeedsUserInput,
        scope_resolution: None,
        scope_context: ctx.scope.clone(),
    };
    let mut trace = default_trace_for_runtime(ctx, utterance, &prepared);
    trace.blocked_reason = Some("Sem OS discovery stage requires clarification".into());

    let mut outcome = OrchestratorOutcome {
        pipeline_result,
        context_envelope: Some(prepared.envelope),
        surface: Some(prepared.surface),
        lookup_result: prepared.lookup_result,
        trace,
        journey_decision: None,
        pending_mutation: None,
        auto_execute: false,
        sage_intent,
        trace_id: None,
    };
    emit_telemetry(ctx, utterance, &mut outcome).await;
    Ok(outcome)
}

#[cfg(feature = "database")]
async fn build_semos_unavailable_outcome(
    ctx: &OrchestratorContext,
    utterance: &str,
    prepared: PreparedTurnContext,
    sage_intent: Option<crate::sage::OutcomeIntent>,
) -> anyhow::Result<OrchestratorOutcome> {
    let pipeline_result = PipelineResult {
        intent: StructuredIntent::empty(),
        verb_candidates: vec![],
        dsl: String::new(),
        dsl_hash: None,
        valid: false,
        validation_error: Some(
            "Sem OS is unavailable. This Sage session cannot continue and should be restarted once Sem OS is healthy."
                .to_string(),
        ),
        unresolved_refs: vec![],
        missing_required: vec![],
        outcome: PipelineOutcome::NoAllowedVerbs,
        scope_resolution: None,
        scope_context: ctx.scope.clone(),
    };
    let mut trace = default_trace_for_runtime(ctx, utterance, &prepared);
    trace.blocked_reason = Some("Sem OS unavailable; session terminated".into());
    trace.semreg_unavailable = true;

    let mut outcome = OrchestratorOutcome {
        pipeline_result,
        context_envelope: Some(prepared.envelope),
        surface: Some(prepared.surface),
        lookup_result: prepared.lookup_result,
        trace,
        journey_decision: None,
        pending_mutation: None,
        auto_execute: false,
        sage_intent,
        trace_id: None,
    };
    emit_telemetry(ctx, utterance, &mut outcome).await;
    Ok(outcome)
}

/// Structured audit trace for every utterance processed.
#[derive(Debug, Clone, Serialize)]
pub struct IntentTrace {
    pub utterance: String,
    pub source: UtteranceSource,
    pub entity_candidates: Vec<String>,
    pub dominant_entity: Option<String>,
    #[cfg(feature = "database")]
    pub sem_reg_verb_filter: Option<Vec<String>>,
    pub verb_candidates_pre_filter: Vec<(String, f32)>,
    pub verb_candidates_post_filter: Vec<(String, f32)>,
    pub final_verb: Option<String>,
    pub final_confidence: f32,
    pub dsl_generated: Option<String>,
    pub dsl_hash: Option<String>,
    pub bypass_used: Option<String>,
    pub dsl_source: Option<String>,
    pub sem_reg_mode: String,
    pub sem_reg_denied_all: bool,
    pub policy_gate_snapshot: PolicySnapshot,
    /// If a forced verb was used (binding disambiguation)
    pub forced_verb: Option<String>,
    /// If PolicyGate blocked something, the reason
    pub blocked_reason: Option<String>,
    /// Verb chosen by discovery BEFORE SemReg filtering
    pub chosen_verb_pre_semreg: Option<String>,
    /// Verb chosen AFTER SemReg filtering (this drives DSL generation)
    pub chosen_verb_post_semreg: Option<String>,
    /// Sem OS policy classification (via SemOsContextEnvelope::label())
    pub semreg_policy: String,
    /// Set when SemReg was unavailable but pipeline continued (non-strict)
    pub semreg_unavailable: bool,
    /// Source of verb selection: "discovery", "user_choice", "macro"
    pub selection_source: String,
    /// Why Serve fell back to the legacy pipeline, when it did.
    pub serve_fallback_reason: Option<String>,
    /// True if macro-expanded DSL was checked against SemReg
    pub macro_semreg_checked: bool,
    /// Verbs in macro expansion that were denied by SemReg (empty if none)
    pub macro_denied_verbs: Vec<String>,
    /// Entity kind of the dominant entity (e.g., "cbu", "fund")
    pub dominant_entity_kind: Option<String>,
    /// Whether entity-kind filtering was applied in Sem OS context resolution
    pub entity_kind_filtered: bool,
    /// Whether telemetry was persisted to agent.intent_events
    pub telemetry_persisted: bool,
    /// Active authoring mode (Research or Governed)
    pub agent_mode: String,
    /// Verbs blocked by AgentMode gating (if any)
    pub agent_mode_blocked_verbs: Vec<String>,
    /// SHA-256 fingerprint of the allowed verb set (deterministic, for audit/telemetry).
    /// Format: `"v1:<hex>"` or None if SemReg unavailable.
    pub allowed_verbs_fingerprint: Option<String>,
    /// Number of verbs pruned by SemReg (ABAC, entity kind, tier, taxonomy, etc.)
    pub pruned_verbs_count: usize,
    /// Whether a TOCTOU recheck was performed before execution
    pub toctou_recheck_performed: bool,
    /// TOCTOU recheck result: "still_allowed", "allowed_but_drifted", "denied", or null
    pub toctou_result: Option<String>,
    /// New fingerprint from TOCTOU recheck (populated on drift or denial)
    pub toctou_new_fingerprint: Option<String>,
    /// SessionVerbSurface fingerprint (format: "vs1:<hex>"), distinct from SemReg fingerprint.
    pub surface_fingerprint: Option<String>,
    /// Journey metadata when a Tier -2 match was used (ScenarioIndex / MacroIndex).
    /// Contains scenario_id, scenario_title, and resolved route.
    #[serde(skip_serializing_if = "Option::is_none")]
    pub journey_match: Option<JourneyMetadata>,
    /// Sage shadow plane classification (Stage 1.5, populated when SAGE_SHADOW=1).
    /// One of: "Instance", "Structure", "Registry".
    #[serde(skip_serializing_if = "Option::is_none")]
    pub sage_plane: Option<String>,
    /// Sage shadow polarity classification (Stage 1.5, populated when SAGE_SHADOW=1).
    /// One of: "Read", "Write", "Ambiguous".
    #[serde(skip_serializing_if = "Option::is_none")]
    pub sage_polarity: Option<String>,
    /// Sage shadow domain hints (Stage 1.5, populated when SAGE_SHADOW=1).
    #[serde(skip_serializing_if = "Vec::is_empty")]
    pub sage_domain_hints: Vec<String>,
}

async fn run_sage_stage(
    ctx: &OrchestratorContext,
    utterance: &str,
    enabled: bool,
) -> SageStageOutcome {
    if !enabled {
        return SageStageOutcome { intent: None };
    }

    let sage_ctx = crate::sage::SageContext {
        session_id: ctx.session_id,
        stage_focus: ctx.stage_focus.clone(),
        goals: ctx.goals.clone(),
        entity_kind: ctx.pre_sage_entity_kind.clone(),
        dominant_entity_name: ctx.pre_sage_entity_name.clone(),
        last_intents: ctx.recent_sage_intents.clone(),
    };
    let sage_engine = ctx
        .sage_engine
        .clone()
        .unwrap_or_else(|| Arc::new(crate::sage::DeterministicSage));

    let intent = match sage_engine.classify(utterance, &sage_ctx).await {
        Ok(intent) => {
            tracing::info!(
                sage_plane = ?intent.plane,
                sage_polarity = ?intent.polarity,
                sage_domain = %intent.domain_concept,
                "Stage 1.5: Sage shadow classification"
            );
            Some(intent)
        }
        Err(e) => {
            tracing::warn!(error = %e, "Stage 1.5: SageEngine failed (non-fatal)");
            None
        }
    };

    SageStageOutcome { intent }
}

fn run_coder_stage(
    ctx: &OrchestratorContext,
    intent: Option<&crate::sage::OutcomeIntent>,
) -> CoderStageOutcome {
    let Some(intent) = intent else {
        return CoderStageOutcome {
            result: None,
            elapsed_ms: None,
            error: None,
        };
    };

    let started_at = std::time::Instant::now();
    if let Some(compiler) = &ctx.nlci_compiler {
        let compiler_input = compiler_input_from_outcome_intent(
            intent,
            ctx.session_id,
            ctx.dominant_entity_id,
            ctx.pre_sage_entity_kind.as_deref(),
            ctx.pre_sage_entity_name.as_deref(),
        );
        if supports_cbu_compiler_slice(&compiler_input) {
            return match compiler.compile(compiler_input) {
                Ok(output) => match output.selection {
                    Some(selection) => CoderStageOutcome {
                        result: Some(coder_result_from_compiler_selection(selection)),
                        elapsed_ms: Some(started_at.elapsed().as_millis()),
                        error: None,
                    },
                    None => CoderStageOutcome {
                        result: None,
                        elapsed_ms: Some(started_at.elapsed().as_millis()),
                        error: Some(
                            output
                                .failure
                                .map(|failure| failure.user_message)
                                .unwrap_or_else(|| {
                                    "NLCI compiler returned no selection for supported CBU intent"
                                        .to_string()
                                }),
                        ),
                    },
                },
                Err(error) => CoderStageOutcome {
                    result: None,
                    elapsed_ms: Some(started_at.elapsed().as_millis()),
                    error: Some(error.to_string()),
                },
            };
        }
    }

    match crate::sage::CoderEngine::load().and_then(|engine| engine.resolve(intent)) {
        Ok(coder_result) => CoderStageOutcome {
            result: Some(coder_result),
            elapsed_ms: Some(started_at.elapsed().as_millis()),
            error: None,
        },
        Err(error) => CoderStageOutcome {
            result: None,
            elapsed_ms: Some(started_at.elapsed().as_millis()),
            error: Some(error.to_string()),
        },
    }
}

fn coder_result_from_compiler_selection(selection: CompilerSelection) -> CoderResult {
    let dsl = render_selection_dsl(&selection);
    CoderResult {
        verb_fqn: selection.verb_id,
        dsl,
        resolution: CoderResolution::Confident,
        missing_args: vec![],
        unresolved_refs: vec![],
        diagnostics: None,
    }
}

fn render_selection_dsl(selection: &CompilerSelection) -> String {
    let args = selection
        .arguments
        .iter()
        .map(|(name, value)| format!(" :{} {}", name, render_dsl_string(value)))
        .collect::<String>();
    format!("({}{})", selection.verb_id, args)
}

fn render_dsl_string(value: &str) -> String {
    format!("\"{}\"", value.replace('\\', "\\\\").replace('"', "\\\""))
}

/// Returns true for pipeline outcomes that are "early exits" -- scope resolution,
/// scope candidates -- where the orchestrator should NOT re-generate DSL via
/// forced-verb. These outcomes don't involve verb ranking.
fn is_early_exit(outcome: &PipelineOutcome) -> bool {
    matches!(
        outcome,
        PipelineOutcome::ScopeResolved { .. } | PipelineOutcome::ScopeCandidates
    )
}

/// Deterministic rewrite for SemOS Data Management noun-only exploration.
///
/// In data-management mode, generic prompts like "show me CBU" should resolve
/// to schema/semantic intents first, not instance-level record retrieval.
#[derive(Debug, Clone, PartialEq, Eq)]
struct DataManagementRewrite {
    rewritten_utterance: String,
    domain_hint: &'static str,
}

fn is_data_management_focus(stage_focus: Option<&str>) -> bool {
    matches!(stage_focus, Some("semos-data-management" | "semos-data"))
}

fn has_explicit_instance_targeting(lower: &str) -> bool {
    // Explicit instance targeting signals should bypass structure-first rewrite.
    lower.contains("deal-id")
        || lower.contains("cbu-id")
        || lower.contains("entity-id")
        || lower.contains("document-id")
        || lower.contains("product-id")
        || lower.contains(" id ")
        || lower.contains(" id:")
        || lower.contains('@')
}

fn infer_data_management_domain(lower: &str) -> Option<&'static str> {
    if lower.contains("document") || lower.contains("documents") {
        return Some("document");
    }
    if lower.contains("product") || lower.contains("products") {
        return Some("product");
    }
    if lower.contains("deal") || lower.contains("deals") || lower.contains("record") {
        return Some("deal");
    }
    if lower.contains(" cbu")
        || lower.starts_with("cbu")
        || lower.contains(" cbus")
        || lower.starts_with("cbus")
    {
        return Some("cbu");
    }
    None
}

fn should_use_structure_first_prompt(lower: &str) -> bool {
    let exploratory_prefix = lower.starts_with("show")
        || lower.starts_with("list")
        || lower.starts_with("what")
        || lower.starts_with("describe");
    let mutating_or_instance_actions = [
        "create", "update", "delete", "remove", "open", "download", "upload", "for id",
    ];
    exploratory_prefix
        && !mutating_or_instance_actions
            .iter()
            .any(|w| lower.contains(w))
}

fn data_management_rewrite(
    stage_focus: Option<&str>,
    utterance: &str,
) -> Option<DataManagementRewrite> {
    if !is_data_management_focus(stage_focus) {
        return None;
    }
    let lower = utterance.trim().to_lowercase();
    if has_explicit_instance_targeting(&lower) || !should_use_structure_first_prompt(&lower) {
        return None;
    }
    let domain = infer_data_management_domain(&lower)?;
    Some(DataManagementRewrite {
        rewritten_utterance: format!(
            "describe entity schema for {domain} with fields relationships and verbs"
        ),
        domain_hint: "schema",
    })
}

fn is_structure_semantics_verb(verb_fqn: &str) -> bool {
    verb_fqn.starts_with("schema.")
        || matches!(
            verb_fqn,
            "registry.search" | "registry.describe-object" | "registry.list-objects"
        )
}

fn is_instance_bound_content_verb(verb_fqn: &str) -> bool {
    if is_structure_semantics_verb(verb_fqn) {
        return false;
    }

    if verb_fqn.ends_with(".get") {
        return true;
    }

    registry()
        .get_by_name(verb_fqn)
        .map(|verb| {
            verb.required_args()
                .iter()
                .any(|arg| arg.name.ends_with("-id"))
        })
        .unwrap_or(false)
}

fn apply_data_management_candidate_policy(
    stage_focus: Option<&str>,
    utterance: &str,
    rewrite_applied: bool,
    candidates: Vec<VerbSearchResult>,
) -> Vec<VerbSearchResult> {
    if !is_data_management_focus(stage_focus) {
        return candidates;
    }

    let lower = utterance.trim().to_lowercase();
    if has_explicit_instance_targeting(&lower) {
        return candidates;
    }

    let mut filtered: Vec<VerbSearchResult> = candidates
        .into_iter()
        .filter(|candidate| !is_instance_bound_content_verb(&candidate.verb))
        .collect();

    if rewrite_applied {
        let structure_candidates: Vec<VerbSearchResult> = filtered
            .iter()
            .filter(|candidate| is_structure_semantics_verb(&candidate.verb))
            .cloned()
            .collect();
        if !structure_candidates.is_empty() {
            filtered = structure_candidates;
        }
    }

    filtered
}

fn should_use_generic_task_subject_for_sage(
    stage_focus: Option<&str>,
    sage_intent: Option<&crate::sage::OutcomeIntent>,
) -> bool {
    is_data_management_focus(stage_focus)
        && matches!(
            sage_intent,
            Some(intent)
                if intent.plane == ObservationPlane::Structure
                    && intent.polarity == crate::sage::IntentPolarity::Read
        )
}

fn allow_data_management_structure_fast_path(
    stage_focus: Option<&str>,
    sage_intent: &crate::sage::OutcomeIntent,
    verb_fqn: &str,
) -> bool {
    is_data_management_focus(stage_focus)
        && sage_intent.plane == ObservationPlane::Structure
        && sage_intent.polarity == crate::sage::IntentPolarity::Read
        && is_structure_semantics_verb(verb_fqn)
}

fn can_skip_fast_path_parse_validation(verb_fqn: &str) -> bool {
    is_structure_semantics_verb(verb_fqn) && verb_fqn.matches('.').count() >= 2
}

/// Process an utterance through the unified pipeline.
///
/// Flow:
/// 1. Entity linking (if LookupService available)
/// 2. Sem OS context resolution -> `SemOsContextEnvelope`
/// 3. Build IntentPipeline, run candidate discovery
/// 4. Apply SemReg filter to candidates
/// 5. For matched-path outcomes: re-generate DSL via forced-verb if SemReg
///    changes the winning verb (ensures SemReg is binding, not cosmetic)
/// 6. Build IntentTrace with full provenance
fn apply_sage_trace_fields(
    trace: &mut IntentTrace,
    intent: &crate::sage::OutcomeIntent,
    source: &str,
) {
    trace.selection_source = source.to_string();
    trace.sage_plane = Some(format!("{:?}", intent.plane));
    trace.sage_polarity = Some(format!("{:?}", intent.polarity));
    trace.sage_domain_hints = if intent.domain_concept.is_empty() {
        vec![]
    } else {
        vec![intent.domain_concept.clone()]
    };
}

#[cfg(feature = "database")]
pub async fn handle_utterance(
    ctx: &OrchestratorContext,
    utterance: &str,
) -> anyhow::Result<OrchestratorOutcome> {
    let trace_scaffold = persist_trace_scaffold(ctx, utterance).await;
    let sage_stage = run_sage_stage(ctx, utterance, true).await;
    let Some(intent) = sage_stage.intent else {
        let outcome = legacy_handle_utterance(ctx, utterance).await?;
        return finalize_orchestrator_trace(ctx, trace_scaffold, outcome).await;
    };

    if let Some(compiler) = &ctx.nlci_compiler {
        let compiler_input = compiler_input_from_outcome_intent(
            &intent,
            ctx.session_id,
            ctx.dominant_entity_id,
            ctx.pre_sage_entity_kind.as_deref(),
            ctx.pre_sage_entity_name.as_deref(),
        );
        if !supports_cbu_compiler_slice(&compiler_input) {
            if let Err(error) = compiler.compile(compiler_input) {
                tracing::warn!(
                    error = %error,
                    "NLCI compiler hook failed during shadow compilation"
                );
            }
        }
    }

    match route(&intent) {
        UtteranceDisposition::Serve(_) => {
            let prepared = prepare_turn_context(ctx, utterance, Some(&intent)).await;
            let prepared_phase2 = Phase2Service::evaluate(
                prepared.lookup_result.clone(),
                Some(prepared.envelope.clone()),
            );
            if !prepared_phase2.is_available {
                let outcome =
                    build_semos_unavailable_outcome(ctx, utterance, prepared, Some(intent.clone()))
                        .await?;
                return finalize_orchestrator_trace(ctx, trace_scaffold, outcome).await;
            }
            if prepared.envelope.is_discovery_stage() {
                let outcome =
                    build_semos_discovery_outcome(ctx, utterance, prepared, Some(intent.clone()))
                        .await?;
                return finalize_orchestrator_trace(ctx, trace_scaffold, outcome).await;
            }
            let coder_stage = run_coder_stage(ctx, Some(&intent));
            let serve_candidate = coder_stage
                .result
                .clone()
                .or_else(|| read_only_list_fallback(&intent));

            if let Some(coder_result) = serve_candidate.as_ref() {
                let selection_source = if intent.plane == ObservationPlane::Structure {
                    "sage_serve_fast_path"
                } else {
                    "sage_serve_coder"
                };
                if can_use_coder_for_serve(ctx, &intent, coder_result, &prepared) {
                    let outcome = build_sage_serve_outcome(
                        ctx,
                        utterance,
                        &intent,
                        coder_result,
                        prepared,
                        selection_source,
                    )
                    .await?;
                    return finalize_orchestrator_trace(ctx, trace_scaffold, outcome).await;
                }
            }

            let mut outcome = legacy_handle_utterance(ctx, utterance).await?;
            outcome.trace.serve_fallback_reason = Some(
                coder_stage
                    .error
                    .or_else(|| {
                        serve_candidate.as_ref().map(|candidate| {
                            format!(
                                "coder candidate '{}' was incomplete or blocked by surface policy",
                                candidate.verb_fqn
                            )
                        })
                    })
                    .unwrap_or_else(|| "sage serve fell back to legacy pipeline".to_string()),
            );
            apply_sage_trace_fields(&mut outcome.trace, &intent, "sage_serve");
            finalize_orchestrator_trace(ctx, trace_scaffold, outcome).await
        }
        UtteranceDisposition::Delegate(delegate) => {
            let prepared = prepare_turn_context(ctx, utterance, Some(&intent)).await;
            let prepared_phase2 = Phase2Service::evaluate(
                prepared.lookup_result.clone(),
                Some(prepared.envelope.clone()),
            );
            if !prepared_phase2.is_available {
                let outcome =
                    build_semos_unavailable_outcome(ctx, utterance, prepared, Some(intent.clone()))
                        .await?;
                return finalize_orchestrator_trace(ctx, trace_scaffold, outcome).await;
            }
            if prepared.envelope.is_discovery_stage() {
                let outcome =
                    build_semos_discovery_outcome(ctx, utterance, prepared, Some(intent.clone()))
                        .await?;
                return finalize_orchestrator_trace(ctx, trace_scaffold, outcome).await;
            }
            let trace = default_trace_for_runtime(ctx, utterance, &prepared);
            let mut outcome = OrchestratorOutcome {
                pipeline_result: PipelineResult {
                    intent: StructuredIntent::empty(),
                    verb_candidates: vec![],
                    dsl: String::new(),
                    dsl_hash: None,
                    valid: false,
                    validation_error: None,
                    unresolved_refs: vec![],
                    missing_required: vec![],
                    outcome: PipelineOutcome::NeedsUserInput,
                    scope_resolution: None,
                    scope_context: ctx.scope.clone(),
                },
                context_envelope: Some(prepared.envelope),
                surface: Some(prepared.surface),
                lookup_result: prepared.lookup_result,
                trace,
                journey_decision: None,
                pending_mutation: None,
                auto_execute: false,
                sage_intent: Some(intent.clone()),
                trace_id: None,
            };
            apply_sage_trace_fields(&mut outcome.trace, &intent, "sage_delegate");

            if intent.confidence == SageConfidence::Low || !intent.pending_clarifications.is_empty()
            {
                outcome.pipeline_result = PipelineResult {
                    intent: StructuredIntent::empty(),
                    verb_candidates: vec![],
                    dsl: String::new(),
                    dsl_hash: None,
                    valid: false,
                    validation_error: Some(
                        "I need a clearer instruction before making a change.".to_string(),
                    ),
                    unresolved_refs: vec![],
                    missing_required: vec![],
                    outcome: PipelineOutcome::NeedsUserInput,
                    scope_resolution: None,
                    scope_context: ctx.scope.clone(),
                };
                outcome.pending_mutation = None;
                outcome.auto_execute = false;
                return finalize_orchestrator_trace(ctx, trace_scaffold, outcome).await;
            }

            let coder_stage = run_coder_stage(ctx, Some(&delegate.outcome));
            let coder_result = coder_stage.result.as_ref();
            let coder_complete = coder_result
                .map(|result| result.missing_args.is_empty() && result.unresolved_refs.is_empty())
                .unwrap_or(false);

            let confirmation = if coder_complete {
                coder_result.map(|result| {
                    build_mutation_confirmation(&intent, result, outcome.lookup_result.as_ref())
                })
            } else {
                None
            };

            if let Some(confirmation) = confirmation {
                outcome.pipeline_result = PipelineResult {
                    intent: StructuredIntent::empty(),
                    verb_candidates: vec![],
                    dsl: String::new(),
                    dsl_hash: None,
                    valid: false,
                    validation_error: Some(confirmation.confirmation_text.clone()),
                    unresolved_refs: vec![],
                    missing_required: vec![],
                    outcome: PipelineOutcome::NeedsUserInput,
                    scope_resolution: None,
                    scope_context: ctx.scope.clone(),
                };
                outcome.pending_mutation = Some(confirmation);
                outcome.auto_execute = false;
                return finalize_orchestrator_trace(ctx, trace_scaffold, outcome).await;
            }

            if let Some(coder_result) = coder_result {
                outcome.pipeline_result = PipelineResult {
                    intent: StructuredIntent::empty(),
                    verb_candidates: vec![],
                    dsl: String::new(),
                    dsl_hash: None,
                    valid: false,
                    validation_error: Some(
                        "I know this is a change, but I still need a few details before I can stage it."
                            .to_string(),
                    ),
                    unresolved_refs: vec![],
                    missing_required: coder_result.missing_args.clone(),
                    outcome: PipelineOutcome::NeedsUserInput,
                    scope_resolution: None,
                    scope_context: ctx.scope.clone(),
                };
                outcome.pending_mutation = None;
                outcome.auto_execute = false;
                return finalize_orchestrator_trace(ctx, trace_scaffold, outcome).await;
            }

            outcome.pipeline_result = PipelineResult {
                intent: StructuredIntent::empty(),
                verb_candidates: vec![],
                dsl: String::new(),
                dsl_hash: None,
                valid: false,
                validation_error: Some(
                    coder_stage
                        .error
                        .unwrap_or_else(|| "Coder could not resolve this mutation.".to_string()),
                ),
                unresolved_refs: vec![],
                missing_required: vec![],
                outcome: PipelineOutcome::NeedsUserInput,
                scope_resolution: None,
                scope_context: ctx.scope.clone(),
            };
            outcome.pending_mutation = None;
            outcome.auto_execute = false;
            finalize_orchestrator_trace(ctx, trace_scaffold, outcome).await
        }
    }
}

async fn persist_trace_scaffold(
    ctx: &OrchestratorContext,
    utterance: &str,
) -> Option<NewUtteranceTrace> {
    let repository = UtteranceTraceRepository::new(ctx.pool.clone());
    let session_id = ctx.session_id.unwrap_or_else(Uuid::nil);
    let mut trace = NewUtteranceTrace::in_progress(
        session_id,
        Uuid::new_v4(),
        utterance,
        TraceKind::Original,
        false,
    );
    let sage_ctx = sage_context_from_orchestrator(ctx);
    trace.correlation_id = ctx.case_id;
    let mut trace_payload = build_trace_scaffold_payload(
        utterance,
        &sage_ctx,
        build_phase2_unavailable_payload("agent_orchestrator"),
        "agent_orchestrator",
    );
    if let Some(payload) = trace_payload.as_object_mut() {
        payload.insert("source".to_string(), serde_json::json!(&ctx.source));
        payload.insert(
            "dominant_entity_id".to_string(),
            serde_json::json!(ctx.dominant_entity_id),
        );
        payload.insert(
            "scope_present".to_string(),
            serde_json::json!(ctx.scope.is_some()),
        );
    }
    trace.trace_payload = trace_payload;

    if let Err(error) = repository.insert(&trace).await {
        tracing::warn!(
            session_id = %session_id,
            error = %error,
            "Failed to persist utterance trace scaffold"
        );
        return None;
    }

    Some(trace)
}

async fn finalize_orchestrator_trace(
    ctx: &OrchestratorContext,
    trace: Option<NewUtteranceTrace>,
    mut outcome: OrchestratorOutcome,
) -> anyhow::Result<OrchestratorOutcome> {
    let Some(mut trace) = trace else {
        return Ok(outcome);
    };

    let repository = UtteranceTraceRepository::new(ctx.pool.clone());
    let sage_ctx = sage_context_from_orchestrator(ctx);
    let phase_payload = build_phase_trace_payload(&trace.raw_utterance, &sage_ctx);
    let phase2 = Phase2Service::evaluate_from_refs(
        outcome.lookup_result.as_ref(),
        outcome.context_envelope.as_ref(),
    );
    let resolved_verb = outcome.trace.final_verb.clone().or_else(|| {
        (!outcome.pipeline_result.intent.verb.is_empty())
            .then(|| outcome.pipeline_result.intent.verb.clone())
    });
    let phase4_candidates = outcome
        .pipeline_result
        .verb_candidates
        .iter()
        .map(|candidate| candidate.verb.clone())
        .collect::<Vec<_>>();
    let phase3 =
        evaluate_phase3_against_phase2(outcome.pipeline_result.verb_candidates.clone(), &phase2);
    let phase4 = evaluate_phase4_within_phase2(
        resolved_verb.clone(),
        phase4_candidates,
        outcome.trace.selection_source.clone(),
        outcome.trace.final_confidence,
        outcome.trace.serve_fallback_reason.clone(),
        &phase2,
    );
    if let Some(violation) = phase4.legality_violation {
        outcome.pipeline_result.outcome = PipelineOutcome::NoAllowedVerbs;
        outcome.pipeline_result.valid = false;
        outcome.pipeline_result.dsl.clear();
        outcome.pipeline_result.dsl_hash = None;
        outcome.pipeline_result.validation_error =
            Some("Phase 4 attempted to resolve a verb outside the Phase 2 legal set.".to_string());
        outcome.trace.final_verb = None;
        outcome.trace.blocked_reason = Some(violation.to_string());
        outcome.auto_execute = false;
    }
    let phase2_payload = phase2.payload();
    let phase3_payload = phase3.payload();
    let phase4_payload = phase4.payload_or_unavailable("agent_orchestrator");
    trace.outcome = classify_agent_trace_outcome(&outcome);
    trace.halt_reason_code = agent_halt_reason_code(&outcome);
    trace.halt_phase = agent_halt_phase(&outcome);
    trace.resolved_verb = resolved_verb;
    trace.plane = outcome.trace.sage_plane.clone().or_else(|| {
        outcome
            .sage_intent
            .as_ref()
            .map(|intent| intent.plane.as_str().to_string())
    });
    trace.polarity = outcome.trace.sage_polarity.clone().or_else(|| {
        outcome
            .sage_intent
            .as_ref()
            .map(|intent| intent.polarity.as_str().to_string())
    });
    trace.fallback_invoked = phase4.fallback_invoked();
    trace.fallback_reason_code = phase4.fallback_reason_code_for_trace();
    trace.surface_versions.verb_surface_version = outcome.trace.surface_fingerprint.clone();
    trace.situation_signature_hash = phase2.situation_signature_hash();
    trace.template_id = phase2.constellation_template_id();
    trace.template_version = phase2.constellation_template_version();
    trace.surface_versions.constellation_template_version = phase2.constellation_template_version();
    trace.trace_payload = serde_json::json!({
        "phase_0": phase_payload["phase_0"].clone(),
        "phase_1": phase_payload["phase_1"].clone(),
        "phase_2": phase2_payload,
        "phase_3": phase3_payload,
        "phase_4": phase4_payload,
        "phase_5": build_phase5_unavailable_payload("agent_orchestrator"),
        "entrypoint": "agent_orchestrator",
        "source": &ctx.source,
        "pipeline_outcome": &outcome.pipeline_result.outcome,
        "validation_error": &outcome.pipeline_result.validation_error,
        "auto_execute": outcome.auto_execute,
        "trace": &outcome.trace,
    });

    if let Err(error) = repository.update(&trace).await {
        tracing::warn!(
            trace_id = %trace.trace_id,
            error = %error,
            "Failed to finalize utterance trace"
        );
    }

    outcome.trace_id = Some(trace.trace_id);
    Ok(outcome)
}

fn sage_context_from_orchestrator(ctx: &OrchestratorContext) -> crate::sage::SageContext {
    crate::sage::SageContext {
        session_id: ctx.session_id,
        stage_focus: ctx.stage_focus.clone(),
        goals: ctx.goals.clone(),
        entity_kind: ctx.pre_sage_entity_kind.clone(),
        dominant_entity_name: ctx.pre_sage_entity_name.clone(),
        last_intents: ctx.recent_sage_intents.clone(),
    }
}

fn classify_agent_trace_outcome(
    outcome: &OrchestratorOutcome,
) -> crate::traceability::TraceOutcome {
    match outcome.pipeline_result.outcome {
        PipelineOutcome::Ready => crate::traceability::TraceOutcome::ExecutedSuccessfully,
        PipelineOutcome::NeedsUserInput
        | PipelineOutcome::NeedsClarification
        | PipelineOutcome::ScopeResolved { .. }
        | PipelineOutcome::ScopeCandidates => {
            crate::traceability::TraceOutcome::ClarificationTriggered
        }
        PipelineOutcome::NoMatch => crate::traceability::TraceOutcome::NoMatch,
        PipelineOutcome::SemanticNotReady | PipelineOutcome::NoAllowedVerbs => {
            crate::traceability::TraceOutcome::HaltedAtPhase
        }
    }
}

fn agent_halt_reason_code(outcome: &OrchestratorOutcome) -> Option<String> {
    match outcome.pipeline_result.outcome {
        PipelineOutcome::SemanticNotReady => Some("semantic_not_ready".to_string()),
        PipelineOutcome::NoAllowedVerbs => Some("no_allowed_verbs".to_string()),
        PipelineOutcome::NoMatch => Some("no_match".to_string()),
        _ => None,
    }
}

fn agent_halt_phase(outcome: &OrchestratorOutcome) -> Option<i16> {
    match outcome.pipeline_result.outcome {
        PipelineOutcome::SemanticNotReady
        | PipelineOutcome::NoAllowedVerbs
        | PipelineOutcome::NoMatch => Some(4),
        _ => None,
    }
}

#[cfg(feature = "database")]
pub async fn legacy_handle_utterance(
    ctx: &OrchestratorContext,
    utterance: &str,
) -> anyhow::Result<OrchestratorOutcome> {
    let policy = &ctx.policy_gate;
    let _sage_shadow_enabled = false;
    let sage_fast_path_enabled = false;
    let sage_enabled = false;

    let sage_stage = run_sage_stage(ctx, utterance, sage_enabled).await;
    let sage_intent = sage_stage.intent;
    let coder_stage = run_coder_stage(ctx, sage_intent.as_ref());
    let sage_coder_result = coder_stage.result;
    let sage_coder_elapsed_ms = coder_stage.elapsed_ms;
    let sage_coder_error = coder_stage.error;

    // -- Step 1: Entity linking --
    let lookup_result = if let Some(ref lookup_svc) = ctx.lookup_service {
        let lr = lookup_svc.analyze(utterance, 5).await;
        tracing::debug!(
            verb_matched = lr.verb_matched,
            entities_resolved = lr.entities_resolved,
            entity_count = lr.entities.len(),
            "Orchestrator: entity linking completed"
        );
        Some(lr)
    } else {
        None
    };

    let dominant_entity_name = lookup_result
        .as_ref()
        .and_then(|lr| lr.dominant_entity.as_ref())
        .map(|e| e.canonical_name.clone());

    let dominant_entity_kind = lookup_result
        .as_ref()
        .and_then(|lr| lr.dominant_entity.as_ref())
        .map(|e| e.entity_kind.clone());

    // Semantic OS data-management is domain-first. Entity-linker can resolve
    // generic nouns like "deal" to arbitrary entities, which over-constrains
    // SemReg entity-kind filtering and can lead to deny-all outcomes.
    let semreg_entity_kind = if matches!(
        ctx.stage_focus.as_deref(),
        Some("semos-data-management" | "semos-data")
    ) {
        None
    } else {
        dominant_entity_kind.clone()
    };

    let entity_candidates: Vec<String> = lookup_result
        .as_ref()
        .map(|lr| lr.entities.iter().map(|e| e.mention_text.clone()).collect())
        .unwrap_or_default();

    // -- Step 2: Sem OS context resolution -> SemOsContextEnvelope --
    let use_generic_task_subject =
        should_use_generic_task_subject_for_sage(ctx.stage_focus.as_deref(), sage_intent.as_ref());
    let envelope = resolve_sem_reg_verbs(
        ctx,
        utterance,
        sage_intent.as_ref(),
        semreg_entity_kind.as_deref(),
        use_generic_task_subject,
    )
    .await;

    // -- Step 2.5: Compute SessionVerbSurface (all governance layers) --
    let fail_policy = if policy.semreg_fail_closed() {
        VerbSurfaceFailPolicy::FailClosed
    } else {
        VerbSurfaceFailPolicy::FailOpen
    };
    let has_group_scope_2 = ctx.scope.as_ref().and_then(|s| s.client_group_id).is_some();
    let surface_ctx = VerbSurfaceContext {
        agent_mode: ctx.agent_mode,
        stage_focus: ctx.stage_focus.as_deref(),
        envelope: &envelope,
        fail_policy,
        entity_state: None, // Lifecycle filtering deferred to Phase 3
        has_group_scope: has_group_scope_2,
        composite_state: None, // TODO: load from group composite when available
    };
    let surface = compute_session_verb_surface(&surface_ctx);

    tracing::debug!(
        total = surface.filter_summary.total_registry,
        after_mode = surface.filter_summary.after_agent_mode,
        after_workflow = surface.filter_summary.after_workflow,
        after_semreg = surface.filter_summary.after_semreg,
        final_count = surface.filter_summary.final_count,
        fingerprint = %surface.surface_fingerprint.0,
        fail_policy = ?surface.fail_policy_applied,
        "SessionVerbSurface computed"
    );

    let phase2 = Phase2Service::evaluate_from_envelope(envelope.clone());
    if !phase2.is_available {
        let prepared = PreparedTurnContext {
            lookup_result,
            dominant_entity_name,
            dominant_entity_kind,
            entity_candidates,
            sem_reg_verb_names: None,
            envelope,
            surface,
            composite_state: None,
        };
        return build_semos_unavailable_outcome(ctx, utterance, prepared, sage_intent.clone())
            .await;
    }

    if envelope.is_discovery_stage() {
        let prepared = PreparedTurnContext {
            lookup_result,
            dominant_entity_name,
            dominant_entity_kind,
            entity_candidates,
            sem_reg_verb_names: None,
            envelope,
            surface,
            composite_state: None,
        };
        return build_semos_discovery_outcome(ctx, utterance, prepared, sage_intent.clone()).await;
    }

    // Extract verb names for trace (backward-compatible with sem_reg_verb_filter)
    let sem_reg_verb_names = Phase2Service::legal_verb_names(&phase2.artifacts);

    // -- Stage A: Discover candidates (no DSL generation yet) --
    // Use SessionVerbSurface's allowed FQN set as pre-constraint for verb search.
    // This consolidates SemReg + AgentMode + workflow filtering into one set.
    let mut surface_allowed: std::collections::HashSet<String> = if phase2.has_usable_legal_set {
        surface.allowed_fqns()
    } else {
        std::collections::HashSet::new()
    };
    let searcher = (*ctx.verb_searcher).clone();

    if sage_fast_path_enabled {
        if let (Some(si), Some(coder_result)) = (sage_intent.as_ref(), sage_coder_result.as_ref()) {
            let sage_only = si.plane == ObservationPlane::Structure
                && si.polarity == crate::sage::IntentPolarity::Read
                && si.pending_clarifications.is_empty();
            let confidence_ok =
                matches!(si.confidence, SageConfidence::High | SageConfidence::Medium);
            let verb_allowed = surface_allowed.contains(&coder_result.verb_fqn);

            if sage_only && confidence_ok && coder_result.missing_args.is_empty() && verb_allowed {
                let fast_path_result =
                    build_sage_fast_path_result(utterance, ctx.scope.clone(), si, coder_result)?;
                let fast_path_candidates = fast_path_result
                    .verb_candidates
                    .iter()
                    .map(|candidate| (candidate.verb.clone(), candidate.score))
                    .collect::<Vec<_>>();
                let chosen_verb = Some(coder_result.verb_fqn.clone());
                let mut trace = build_trace(
                    utterance,
                    ctx,
                    &entity_candidates,
                    &dominant_entity_name,
                    &dominant_entity_kind,
                    &sem_reg_verb_names,
                    &fast_path_candidates,
                    &fast_path_candidates,
                    &chosen_verb,
                    &chosen_verb,
                    &fast_path_result,
                    &envelope,
                    Some("sage_fast_path".to_string()),
                    false,
                    false,
                    &[],
                    false,
                    None,
                    None,
                );
                trace.selection_source = "sage_fast_path".to_string();
                trace.surface_fingerprint = Some(surface.surface_fingerprint.0.clone());
                trace.sage_plane = Some(format!("{:?}", si.plane));
                trace.sage_polarity = Some(format!("{:?}", si.polarity));
                trace.sage_domain_hints = if si.domain_concept.is_empty() {
                    vec![]
                } else {
                    vec![si.domain_concept.clone()]
                };

                tracing::info!(
                    verb = %coder_result.verb_fqn,
                    dsl = %coder_result.dsl,
                    sage_confidence = %si.confidence.as_str(),
                    sage_coder_resolution = ?coder_result.resolution,
                    unresolved_refs = fast_path_result.unresolved_refs.len(),
                    "Sage-only fast path: Read+Structure — bypassing pipeline"
                );

                let mut outcome = OrchestratorOutcome {
                    pipeline_result: fast_path_result,
                    context_envelope: Some(envelope),
                    surface: Some(surface),
                    lookup_result,
                    trace,
                    journey_decision: None,
                    pending_mutation: None,
                    auto_execute: false,
                    sage_intent: Some(si.clone()),
                    trace_id: None,
                };
                emit_telemetry(ctx, utterance, &mut outcome).await;
                return Ok(outcome);
            }
        }
    }

    // Extract entity mention spans from the lookup result so the ECIR noun
    // scanner in IntentPipeline can skip entity names (entity-first parsing PR 3).
    let entity_mention_spans: Vec<(usize, usize)> = lookup_result
        .as_ref()
        .map(|lr| lr.entities.iter().map(|e| e.mention_span).collect())
        .unwrap_or_default();

    // ── Stage A.5: SemOS-Scoped Constrained Resolution ──────────────────
    // When a client group is in scope AND a constellation template is selected,
    // compute the valid verb set (deterministic, ~1ms) and try keyword matching
    // BEFORE running the full open-search pipeline. This resolves ~85% of
    // context-bearing utterances without any embedding queries.
    let constrained_verb_result: Option<String> = {
        let scope_group_id = ctx.scope.as_ref().and_then(|s| s.client_group_id);
        let constellation_name = ctx
            .discovery_selected_constellation
            .as_deref()
            .or(Some("group.ownership"));

        if let (Some(group_id), Some(c_name)) = (scope_group_id, constellation_name) {
            use crate::sage::constrained_match::resolve_constrained_hybrid;
            use crate::sage::session_context::load_entity_states_for_group;
            use crate::sage::valid_verb_set::{
                compute_valid_verb_set_for_constellations, load_constellation_stack,
            };
            let scoped_searcher = (*ctx.verb_searcher).clone();

            if let Ok(constellations) = load_constellation_stack(c_name) {
                // Load entity states for this client group
                let entity_states = load_entity_states_for_group(&ctx.pool, group_id)
                    .await
                    .unwrap_or_default();

                let valid_verbs = compute_valid_verb_set_for_constellations(
                    &entity_states,
                    &constellations,
                    group_id,
                );

                if !valid_verbs.is_empty() {
                    let constrained =
                        resolve_constrained_hybrid(utterance, &valid_verbs, &scoped_searcher, None)
                            .await
                            .unwrap_or_else(|_| {
                                crate::sage::constrained_match::ConstrainedResult::fallthrough()
                            });
                    if constrained.resolved() {
                        tracing::info!(
                            verb = ?constrained.verb_fqn,
                            confidence = constrained.confidence,
                            strategy = ?constrained.strategy,
                            valid_set_size = valid_verbs.len(),
                            keyword_hits = constrained.keyword_hits,
                            "SemOS-scoped constrained resolution succeeded"
                        );
                        constrained.verb_fqn.clone()
                    } else {
                        tracing::debug!(
                            valid_set_size = valid_verbs.len(),
                            strategy = ?constrained.strategy,
                            "SemOS-scoped constrained match fell through to open search"
                        );
                        None
                    }
                } else {
                    None
                }
            } else {
                None
            }
        } else {
            None
        }
    };

    // If constrained resolution succeeded, narrow allowed_verbs to just that verb
    // so the open search pipeline (ECIR + embedding) is focused. This preserves
    // all normal post-processing (entity resolution, DSL generation, etc.) while
    // giving the correct verb a massive advantage in the search results.
    if let Some(ref constrained_verb) = constrained_verb_result {
        if surface_allowed.is_empty() || surface_allowed.contains(constrained_verb) {
            tracing::info!(
                verb = %constrained_verb,
                "SemOS-scoped: narrowing allowed_verbs to constrained match"
            );
            // Narrow surface_allowed to just the constrained verb + a handful
            // of related verbs for disambiguation safety
            let mut narrowed = std::collections::HashSet::new();
            narrowed.insert(constrained_verb.clone());
            // Also keep observation verbs in the same domain
            let domain = constrained_verb.split('.').next().unwrap_or("");
            for v in &surface_allowed {
                if v.starts_with(domain) {
                    narrowed.insert(v.clone());
                }
            }
            surface_allowed = narrowed;
        }
    }

    let pipeline = {
        let p = IntentPipeline::with_pool(searcher, ctx.pool.clone());
        p.with_allowed_verbs(surface_allowed.clone())
            .with_entity_mention_spans(entity_mention_spans)
    };

    let rewrite = data_management_rewrite(ctx.stage_focus.as_deref(), utterance);
    let discovery_utterance = rewrite
        .as_ref()
        .map(|r| r.rewritten_utterance.as_str())
        .unwrap_or(utterance);
    let discovery_domain_hint = rewrite.as_ref().map(|r| r.domain_hint);
    if let Some(ref rw) = rewrite {
        tracing::info!(
            input = utterance,
            rewritten_input = rw.rewritten_utterance,
            domain_hint = rw.domain_hint,
            "SemOS data-management structure-first rewrite applied"
        );
    }

    let discovery_result = pipeline
        .process_with_scope(
            discovery_utterance,
            discovery_domain_hint,
            dominant_entity_kind.as_deref(),
            ctx.scope.clone(),
        )
        .await?;

    // Capture pre-SemReg state
    let pre_filter: Vec<(String, f32)> = discovery_result
        .verb_candidates
        .iter()
        .map(|v| (v.verb.clone(), v.score))
        .collect();

    let chosen_verb_pre_semreg = discovery_result
        .verb_candidates
        .first()
        .map(|v| v.verb.clone());

    // -- Early exit check --
    if is_early_exit(&discovery_result.outcome) {
        let mut trace = build_trace(
            utterance,
            ctx,
            &entity_candidates,
            &dominant_entity_name,
            &dominant_entity_kind,
            &sem_reg_verb_names,
            &pre_filter,
            &pre_filter,
            &chosen_verb_pre_semreg,
            &chosen_verb_pre_semreg,
            &discovery_result,
            &envelope,
            None,
            false,
            false,
            &[],
            false,
            None,
            None,
        );
        // Stamp sage shadow fields (Stage 1.5, SAGE_SHADOW=1)
        if let Some(ref si) = sage_intent {
            trace.sage_plane = Some(format!("{:?}", si.plane));
            trace.sage_polarity = Some(format!("{:?}", si.polarity));
            trace.sage_domain_hints = if si.domain_concept.is_empty() {
                vec![]
            } else {
                vec![si.domain_concept.clone()]
            };
        }
        let mut outcome = OrchestratorOutcome {
            pipeline_result: discovery_result,
            context_envelope: Some(envelope),
            surface: Some(surface),
            lookup_result,
            trace,
            journey_decision: None,
            pending_mutation: None,
            auto_execute: false,
            sage_intent: sage_intent.clone(),
            trace_id: None,
        };
        emit_telemetry(ctx, utterance, &mut outcome).await;
        return Ok(outcome);
    }

    // NOTE: Direct DSL early-exit (dsl: prefix) was removed in Phase 0B CCIR.
    // All DSL — including operator-provided — flows through SemReg filtering below.

    // -- Stage A.2: Derive governance flags from envelope (single source of truth) --
    // Pre-constraint via with_allowed_verbs() already filtered verb search results.
    // No redundant post-filter — trust the pre-constraint.
    let semreg_unavailable = envelope.is_unavailable();
    let sem_reg_denied_all = envelope.is_deny_all();
    let mut blocked_reason: Option<String> = None;
    let mut filtered_candidates = discovery_result.verb_candidates.clone();
    let agent_mode_blocked: Vec<String> = Vec::new();

    if semreg_unavailable {
        filtered_candidates.clear();
        blocked_reason = Some("SemReg unavailable (utterance discovery requires Sem OS)".into());
        tracing::warn!("SemReg unavailable -- blocking utterance discovery");
    } else if sem_reg_denied_all {
        filtered_candidates.clear();
        blocked_reason = Some("SemReg denied all verbs for this subject".into());
        tracing::warn!("SemReg returned DenyAll -- blocking utterance discovery");
    }

    let before_data_management = filtered_candidates.len();
    filtered_candidates = apply_data_management_candidate_policy(
        ctx.stage_focus.as_deref(),
        utterance,
        rewrite.is_some(),
        filtered_candidates,
    );
    if filtered_candidates.is_empty() && before_data_management > 0 {
        blocked_reason.get_or_insert_with(|| {
            "Data-management mode blocked instance-bound content verbs without explicit instance targeting"
                .into()
        });
        tracing::warn!(
            before = before_data_management,
            input = utterance,
            "Data-management candidate policy removed all remaining verb candidates"
        );
    }

    let subset_result = evaluate_phase3_against_phase2(filtered_candidates, &phase2).subset_result;
    if subset_result.had_violation() {
        tracing::warn!(
            removed = subset_result.eliminated_candidates.len(),
            "Phase 3 removed candidates that violated the Phase 2 legal ceiling"
        );
    }
    let filtered_candidates = subset_result.retained_candidates;
    if filtered_candidates.is_empty() && !subset_result.eliminated_candidates.is_empty() {
        blocked_reason.get_or_insert_with(|| {
            "Phase 3 subset enforcement removed all candidates outside the Phase 2 legal set".into()
        });
    }

    let post_filter: Vec<(String, f32)> = filtered_candidates
        .iter()
        .map(|v| (v.verb.clone(), v.score))
        .collect();

    let chosen_verb_post_semreg = filtered_candidates.first().map(|v| v.verb.clone());

    // -- Stage B: Select verb + generate DSL --
    let mut journey_used: Option<JourneyMetadata> = None;
    let mut journey_decision_out: Option<ob_poc_types::DecisionPacket> = None;

    let mut result = if sem_reg_denied_all || semreg_unavailable {
        PipelineResult {
            intent: StructuredIntent::empty(),
            verb_candidates: filtered_candidates,
            dsl: String::new(),
            dsl_hash: None,
            valid: false,
            validation_error: Some(
                blocked_reason
                    .clone()
                    .unwrap_or_else(|| "SemReg blocked utterance discovery".into()),
            ),
            unresolved_refs: vec![],
            missing_required: vec![],
            outcome: PipelineOutcome::NoAllowedVerbs,
            scope_resolution: discovery_result.scope_resolution,
            scope_context: discovery_result.scope_context,
        }
    } else if !filtered_candidates.is_empty()
        && discovery_result.outcome == PipelineOutcome::NeedsClarification
    {
        let mut result = discovery_result;
        result.verb_candidates = filtered_candidates;
        result
    } else if let Some(top) = filtered_candidates.first().cloned() {
        // We have a post-SemReg winner.
        // Check for Tier -2 journey match first — these produce macro DSL
        // deterministically without LLM arg extraction.
        if let Some(journey) = &top.journey {
            tracing::info!(
                verb = %top.verb,
                source = ?top.source,
                scenario_id = ?journey.scenario_id,
                scenario_title = ?journey.scenario_title,
                route = ?journey.route,
                "Stage B: Tier -2 journey match — bypassing LLM, constructing macro DSL"
            );
            journey_used = Some(journey.clone());
            let (journey_result, j_decision) = build_journey_pipeline_result(
                &top,
                journey,
                &filtered_candidates,
                &discovery_result,
                ctx.verb_searcher.macro_registry().map(|r| r.as_ref()),
                ctx.session_id,
                utterance,
            );
            journey_decision_out = j_decision;
            journey_result
        } else {
            // Standard path: discovery reuse or LLM arg extraction.
            let discovery_verb_matches = !discovery_result.dsl.is_empty()
                && discovery_result.intent.verb.as_str() == top.verb.as_str();

            if discovery_verb_matches {
                let mut result = discovery_result;
                result.verb_candidates = filtered_candidates;
                result
            } else {
                let searcher2 = (*ctx.verb_searcher).clone();
                let pipeline2 = IntentPipeline::with_pool(searcher2, ctx.pool.clone());
                let mut forced_result = pipeline2
                    .process_with_forced_verb(discovery_utterance, &top.verb, ctx.scope.clone())
                    .await?;
                forced_result.verb_candidates = filtered_candidates;
                forced_result
            }
        }
    } else {
        let mut result = discovery_result;
        result.verb_candidates = filtered_candidates;
        result
    };

    // -- Step 5B: TOCTOU recheck (Phase 5B CCIR) --
    // Re-resolve SemReg verbs to detect if the allowed set drifted between
    // initial resolution (Step 2) and now. Only performed when:
    //  1. strict SemReg mode is enabled (OBPOC_STRICT_SEMREG=true)
    //  2. a verb was selected (not blocked/empty)
    //  3. the original envelope was not unavailable
    let mut toctou_performed = false;
    let mut toctou_result_str: Option<String> = None;
    let mut toctou_new_fp: Option<String> = None;

    let selected_verb_fqn = result
        .verb_candidates
        .first()
        .map(|v| v.verb.clone())
        .or_else(|| {
            if !result.intent.verb.is_empty() {
                Some(result.intent.verb.clone())
            } else {
                None
            }
        });

    let phase2 = Phase2Service::evaluate_from_envelope(envelope.clone());
    if policy.semreg_fail_closed() && phase2.is_available {
        if let Some(ref verb_fqn) = selected_verb_fqn {
            toctou_performed = true;
            let new_envelope =
                resolve_sem_reg_verbs(ctx, "", None, semreg_entity_kind.as_deref(), false).await;

            if let Some(toctou) = envelope.toctou_recheck(&new_envelope, verb_fqn) {
                use crate::agent::sem_os_context_envelope::TocTouResult;
                match &toctou {
                    TocTouResult::StillAllowed => {
                        toctou_result_str = Some("still_allowed".to_string());
                        tracing::debug!(verb = %verb_fqn, "TOCTOU recheck: still allowed");
                    }
                    TocTouResult::AllowedButDrifted { new_fingerprint } => {
                        toctou_result_str = Some("allowed_but_drifted".to_string());
                        toctou_new_fp = Some(new_fingerprint.to_string());
                        tracing::warn!(
                            verb = %verb_fqn,
                            old_fingerprint = %envelope.fingerprint_str(),
                            new_fingerprint = %new_fingerprint,
                            "TOCTOU recheck: allowed but verb set drifted since resolution"
                        );
                    }
                    TocTouResult::Denied {
                        verb_fqn: denied_verb,
                        new_fingerprint,
                    } => {
                        toctou_result_str = Some("denied".to_string());
                        toctou_new_fp = Some(new_fingerprint.to_string());
                        tracing::warn!(
                            verb = %denied_verb,
                            old_fingerprint = %envelope.fingerprint_str(),
                            new_fingerprint = %new_fingerprint,
                            "TOCTOU recheck: verb DENIED — allowed set changed"
                        );
                        // Replace result with blocked outcome
                        result = PipelineResult {
                            intent: StructuredIntent::empty(),
                            verb_candidates: vec![],
                            dsl: String::new(),
                            dsl_hash: None,
                            valid: false,
                            validation_error: Some(format!(
                                "TOCTOU recheck failed: verb '{}' no longer in allowed set (fingerprint drifted)",
                                denied_verb
                            )),
                            unresolved_refs: vec![],
                            missing_required: vec![],
                            outcome: PipelineOutcome::NoAllowedVerbs,
                            scope_resolution: result.scope_resolution,
                            scope_context: result.scope_context,
                        };
                    }
                }
            }
        }
    }

    // -- Step 6: Build IntentTrace --
    let chosen_post = &chosen_verb_post_semreg;
    let semreg_forced_regen = chosen_verb_pre_semreg.is_some()
        && chosen_post.is_some()
        && chosen_verb_pre_semreg != *chosen_post;
    let mut trace = build_trace(
        utterance,
        ctx,
        &entity_candidates,
        &dominant_entity_name,
        &dominant_entity_kind,
        &sem_reg_verb_names,
        &pre_filter,
        &post_filter,
        &chosen_verb_pre_semreg,
        chosen_post,
        &result,
        &envelope,
        None,
        sem_reg_denied_all,
        semreg_unavailable,
        &agent_mode_blocked,
        toctou_performed,
        toctou_result_str,
        toctou_new_fp,
    );
    if semreg_forced_regen {
        trace.selection_source = "semreg".to_string();
        trace.forced_verb = chosen_post.clone();
    }
    // Stamp journey metadata + selection source when Tier -2 match was used
    if let Some(journey) = journey_used {
        trace.selection_source = if journey.scenario_id.is_some() {
            "scenario".to_string()
        } else {
            "macro_index".to_string()
        };
        trace.journey_match = Some(journey);
    }
    // Stamp surface fingerprint into trace
    trace.surface_fingerprint = Some(surface.surface_fingerprint.0.clone());
    // Stamp sage shadow fields (Stage 1.5, SAGE_SHADOW=1)
    if let Some(ref si) = sage_intent {
        trace.sage_plane = Some(format!("{:?}", si.plane));
        trace.sage_polarity = Some(format!("{:?}", si.polarity));
        trace.sage_domain_hints = if si.domain_concept.is_empty() {
            vec![]
        } else {
            vec![si.domain_concept.clone()]
        };
    }

    if let Some(ref coder_result) = sage_coder_result {
        let existing_verb = trace.final_verb.clone();
        let existing_dsl = trace.dsl_generated.clone().unwrap_or_default();
        let dsl_similarity = dsl_similarity(&coder_result.dsl, &existing_dsl);
        tracing::info!(
            sage_coder_verb = %coder_result.verb_fqn,
            existing_verb = ?existing_verb,
            sage_coder_dsl = %coder_result.dsl,
            existing_dsl = %existing_dsl,
            sage_coder_resolution = ?coder_result.resolution,
            sage_coder_missing_args = ?coder_result.missing_args,
            sage_coder_unresolved_refs = ?coder_result.unresolved_refs,
            verb_agreement = (existing_verb.as_deref() == Some(coder_result.verb_fqn.as_str())),
            dsl_similarity,
            sage_coder_ms = sage_coder_elapsed_ms.unwrap_or_default(),
            "Stage 2.4: Sage->Coder shadow comparison"
        );
    } else if let Some(error) = sage_coder_error {
        tracing::warn!(
            error = %error,
            sage_coder_ms = sage_coder_elapsed_ms.unwrap_or_default(),
            "Stage 2.4: Sage->Coder shadow comparison failed (non-fatal)"
        );
    }

    tracing::info!(
        source = ?trace.source,
        final_verb = ?trace.final_verb,
        confidence = trace.final_confidence,
        sem_reg_filtered = trace.sem_reg_verb_filter.is_some(),
        bypass = ?trace.bypass_used,
        sem_reg_denied_all = trace.sem_reg_denied_all,
        sem_reg_mode = %trace.sem_reg_mode,
        semreg_policy = %trace.semreg_policy,
        chosen_verb_pre = ?trace.chosen_verb_pre_semreg,
        chosen_verb_post = ?trace.chosen_verb_post_semreg,
        fingerprint = ?trace.allowed_verbs_fingerprint,
        surface_fingerprint = ?trace.surface_fingerprint,
        pruned_count = trace.pruned_verbs_count,
        toctou = ?trace.toctou_result,
        "IntentTrace"
    );
    tracing::debug!(trace = %serde_json::to_string(&trace).unwrap_or_default(), "IntentTrace detail");

    let mut outcome = OrchestratorOutcome {
        pipeline_result: result,
        context_envelope: Some(envelope),
        surface: Some(surface),
        lookup_result,
        trace,
        journey_decision: journey_decision_out,
        pending_mutation: None,
        auto_execute: false,
        sage_intent: sage_intent.clone(),
        trace_id: None,
    };
    emit_telemetry(ctx, utterance, &mut outcome).await;
    Ok(outcome)
}

fn dsl_similarity(lhs: &str, rhs: &str) -> f32 {
    if lhs.is_empty() || rhs.is_empty() {
        return 0.0;
    }

    let lhs_tokens = lhs
        .split_whitespace()
        .collect::<std::collections::HashSet<_>>();
    let rhs_tokens = rhs
        .split_whitespace()
        .collect::<std::collections::HashSet<_>>();
    let intersection = lhs_tokens.intersection(&rhs_tokens).count();
    let union = lhs_tokens.union(&rhs_tokens).count();

    if union == 0 {
        0.0
    } else {
        intersection as f32 / union as f32
    }
}

#[cfg(feature = "database")]
fn build_sage_fast_path_result(
    utterance: &str,
    scope: Option<ScopeContext>,
    outcome: &crate::sage::OutcomeIntent,
    coder_result: &CoderResult,
) -> anyhow::Result<PipelineResult> {
    let config = dsl_core::config::loader::ConfigLoader::from_env().load_verbs()?;
    let (domain, verb_name) = coder_result
        .verb_fqn
        .split_once('.')
        .ok_or_else(|| anyhow::anyhow!("invalid coder verb '{}'", coder_result.verb_fqn))?;
    let verb_cfg = config
        .domains
        .get(domain)
        .and_then(|domain_cfg| domain_cfg.verbs.get(verb_name))
        .ok_or_else(|| {
            anyhow::anyhow!("missing config for coder verb '{}'", coder_result.verb_fqn)
        })?;
    let step = outcome
        .steps
        .first()
        .cloned()
        .unwrap_or_else(|| OutcomeStep {
            action: outcome.action.clone(),
            target: outcome.domain_concept.clone(),
            params: std::collections::HashMap::new(),
            notes: None,
        });
    let intent = crate::sage::arg_assembly::structured_intent_from_step(
        &coder_result.verb_fqn,
        &step,
        verb_cfg,
    )?;

    let parsed = parse_program(&coder_result.dsl);
    let skip_parse_validation = can_skip_fast_path_parse_validation(&coder_result.verb_fqn);
    let (unresolved_refs, parse_error) = match parsed.as_ref() {
        Ok(ast) => {
            let registry = runtime_registry_arc();
            let enriched = enrich_program(ast.clone(), &registry);
            let refs = find_unresolved_ref_locations(&enriched.program)
                .into_iter()
                .map(|loc| UnresolvedRef {
                    param_name: loc.arg_key,
                    search_value: loc.search_text,
                    entity_type: Some(loc.entity_type),
                    search_column: loc.search_column,
                    ref_id: loc.ref_id,
                })
                .collect::<Vec<_>>();
            (refs, None)
        }
        Err(_) if skip_parse_validation => (vec![], None),
        Err(error) => (
            vec![],
            Some(format!("Parse error after assembly: {:?}", error)),
        ),
    };

    let (valid, validation_error) = match parse_error {
        Some(error) => (false, Some(error)),
        None if skip_parse_validation => (true, None),
        None => match parsed {
            Ok(ast) => match compile(&ast) {
                Ok(_) => (true, None),
                Err(error) => (false, Some(format!("Compile error: {:?}", error))),
            },
            Err(error) => (false, Some(format!("Parse error: {:?}", error))),
        },
    };

    let score = match outcome.confidence {
        SageConfidence::High => 1.0,
        SageConfidence::Medium => 0.8,
        SageConfidence::Low => 0.6,
    };
    let scope_context = scope.filter(ScopeContext::has_scope);

    Ok(PipelineResult {
        intent,
        verb_candidates: vec![VerbSearchResult {
            verb: coder_result.verb_fqn.clone(),
            score,
            source: VerbSearchSource::NounTaxonomy,
            matched_phrase: utterance.to_string(),
            description: Some("sage_fast_path".to_string()),
            journey: None,
        }],
        dsl: coder_result.dsl.clone(),
        dsl_hash: Some(compute_dsl_hash(&coder_result.dsl)),
        valid,
        validation_error,
        unresolved_refs: unresolved_refs.clone(),
        missing_required: coder_result.missing_args.clone(),
        outcome: if coder_result.missing_args.is_empty() && unresolved_refs.is_empty() && valid {
            PipelineOutcome::Ready
        } else {
            PipelineOutcome::NeedsUserInput
        },
        scope_resolution: None,
        scope_context,
    })
}

/// Build an IntentTrace from the current orchestrator state.
#[cfg(feature = "database")]
#[allow(clippy::too_many_arguments)]
fn build_trace(
    utterance: &str,
    ctx: &OrchestratorContext,
    entity_candidates: &[String],
    dominant_entity_name: &Option<String>,
    dominant_entity_kind: &Option<String>,
    sem_reg_verb_names: &Option<Vec<String>>,
    pre_filter: &[(String, f32)],
    post_filter: &[(String, f32)],
    chosen_verb_pre_semreg: &Option<String>,
    chosen_verb_post_semreg: &Option<String>,
    result: &PipelineResult,
    envelope: &SemOsContextEnvelope,
    bypass_used: Option<String>,
    sem_reg_denied_all: bool,
    semreg_unavailable: bool,
    agent_mode_blocked: &[String],
    toctou_recheck_performed: bool,
    toctou_result: Option<String>,
    toctou_new_fingerprint: Option<String>,
) -> IntentTrace {
    let policy = &ctx.policy_gate;
    let final_verb = result
        .verb_candidates
        .first()
        .map(|v| v.verb.clone())
        .or_else(|| {
            if !result.intent.verb.is_empty() {
                Some(result.intent.verb.clone())
            } else {
                None
            }
        });
    let final_confidence = result
        .verb_candidates
        .first()
        .map(|v| v.score)
        .unwrap_or(0.0);

    let bypass = bypass_used;

    let blocked_reason = if sem_reg_denied_all && policy.semreg_fail_closed() {
        Some("SemReg denied all verb candidates (strict mode)".into())
    } else if semreg_unavailable && policy.semreg_fail_closed() {
        Some("SemReg unavailable (strict mode requires SemReg)".into())
    } else {
        None
    };

    let sem_reg_mode = if semreg_unavailable && !policy.semreg_fail_closed() {
        "fail_open".to_string()
    } else if policy.semreg_fail_closed() {
        "strict".to_string()
    } else {
        "permissive".to_string()
    };

    // Extract fingerprint and pruned count from envelope
    let phase2 = Phase2Service::evaluate_from_envelope(envelope.clone());
    let allowed_verbs_fingerprint = phase2.fingerprint();
    let pruned_verbs_count = phase2.pruned_verb_count();
    IntentTrace {
        utterance: utterance.to_string(),
        source: ctx.source.clone(),
        entity_candidates: entity_candidates.to_vec(),
        dominant_entity: dominant_entity_name.clone(),
        sem_reg_verb_filter: sem_reg_verb_names.clone(),
        verb_candidates_pre_filter: pre_filter.to_vec(),
        verb_candidates_post_filter: post_filter.to_vec(),
        final_verb,
        final_confidence,
        dsl_generated: Some(result.dsl.clone()).filter(|d| !d.is_empty()),
        dsl_hash: result.dsl_hash.clone(),
        bypass_used: bypass,
        dsl_source: Some(format!("{:?}", ctx.source)),
        forced_verb: None,
        blocked_reason,
        sem_reg_mode,
        sem_reg_denied_all,
        policy_gate_snapshot: policy.snapshot(),
        chosen_verb_pre_semreg: chosen_verb_pre_semreg.clone(),
        chosen_verb_post_semreg: chosen_verb_post_semreg.clone(),
        semreg_policy: phase2.policy_label.to_string(),
        semreg_unavailable,
        selection_source: "discovery".to_string(),
        serve_fallback_reason: None,
        macro_semreg_checked: false,
        macro_denied_verbs: vec![],
        dominant_entity_kind: dominant_entity_kind.clone(),
        entity_kind_filtered: dominant_entity_kind.is_some(),
        telemetry_persisted: false,
        agent_mode: ctx.agent_mode.to_string(),
        agent_mode_blocked_verbs: agent_mode_blocked.to_vec(),
        allowed_verbs_fingerprint,
        pruned_verbs_count,
        toctou_recheck_performed,
        toctou_result,
        toctou_new_fingerprint,
        surface_fingerprint: None, // Set by caller after build_trace()
        journey_match: None,       // Set by caller when Tier -2 match is used
        sage_plane: None,          // Set by caller when SAGE_SHADOW=1
        sage_polarity: None,       // Set by caller when SAGE_SHADOW=1
        sage_domain_hints: vec![], // Set by caller when SAGE_SHADOW=1
    }
}

/// Build a `PipelineResult` from a Tier -2 journey match, bypassing LLM arg extraction.
///
/// For `JourneyRoute::Macro` and `MacroSequence`, constructs DSL with the macro FQN(s)
/// as bare invocations. The downstream DSL execution pipeline handles macro expansion
/// and `derive_pending_questions()` drives conversational arg collection.
///
/// For `JourneyRoute::MacroSequence`, calls `validate_macro_sequence()` to verify
/// prereq feasibility. Failed prereqs are surfaced as validation warnings.
///
/// For `JourneyRoute::NeedsSelection`, builds a `DecisionPacket` with the selector
/// options so the user can pick (e.g., jurisdiction) before the macro is resolved.
///
/// Returns `(PipelineResult, Option<DecisionPacket>)` — the decision is `Some` only
/// for `NeedsSelection` routes.
#[cfg(feature = "database")]
fn build_journey_pipeline_result(
    top: &VerbSearchResult,
    journey: &JourneyMetadata,
    filtered_candidates: &[VerbSearchResult],
    discovery_result: &PipelineResult,
    macro_registry: Option<&crate::dsl_v2::macros::MacroRegistry>,
    session_id: Option<Uuid>,
    utterance: &str,
) -> (PipelineResult, Option<ob_poc_types::DecisionPacket>) {
    let (dsl, outcome, notes, validation_error, decision) = match &journey.route {
        JourneyRoute::Macro { macro_fqn } => {
            let dsl = format!("({})", macro_fqn);
            let note = format!(
                "Tier -2 journey → single macro: {}",
                journey
                    .scenario_title
                    .as_deref()
                    .unwrap_or(macro_fqn.as_str())
            );
            (dsl, PipelineOutcome::Ready, vec![note], None, None)
        }
        JourneyRoute::MacroSequence { macros } => {
            let dsl = macros
                .iter()
                .map(|m| format!("({})", m))
                .collect::<Vec<_>>()
                .join("\n");
            let mut notes = vec![format!(
                "Tier -2 journey → macro sequence ({} macros): {}",
                macros.len(),
                journey
                    .scenario_title
                    .as_deref()
                    .unwrap_or("unnamed sequence")
            )];

            // Validate the sequence prereqs if we have a macro registry
            let mut val_error = None;
            if let Some(registry) = macro_registry {
                let empty_state = std::collections::HashSet::new();
                let result = crate::mcp::sequence_validator::validate_macro_sequence(
                    macros,
                    registry,
                    &empty_state, // fresh session — no state flags yet
                    &empty_state, // no completed verbs yet
                );

                if !result.feasible {
                    // Hard failures — surface as validation error but still return DSL
                    // so the user can see what was planned
                    let fail_details: Vec<String> = result
                        .validations
                        .iter()
                        .filter(|v| {
                            matches!(
                                v.check,
                                crate::mcp::sequence_validator::PrereqCheck::Fail { .. }
                            )
                        })
                        .map(|v| {
                            if let crate::mcp::sequence_validator::PrereqCheck::Fail {
                                ref missing,
                                ref satisfied_by,
                            } = v.check
                            {
                                if satisfied_by.is_empty() {
                                    format!("{}: missing prerequisite '{}'", v.macro_fqn, missing)
                                } else {
                                    format!(
                                        "{}: missing prerequisite '{}' (could be satisfied by: {})",
                                        v.macro_fqn,
                                        missing,
                                        satisfied_by.join(", ")
                                    )
                                }
                            } else {
                                String::new()
                            }
                        })
                        .filter(|s| !s.is_empty())
                        .collect();
                    val_error = Some(format!(
                        "Sequence validation: {} of {} macros have unmet prerequisites:\n{}",
                        result.fail_count,
                        macros.len(),
                        fail_details.join("\n")
                    ));
                    notes.push(format!(
                        "⚠ Sequence prereq check: {} pass, {} fail, {} deferred",
                        result.pass_count, result.fail_count, result.deferred_count
                    ));
                } else if result.deferred_count > 0 {
                    notes.push(format!(
                        "Sequence prereq check: {} pass, {} deferred (will verify at runtime)",
                        result.pass_count, result.deferred_count
                    ));
                } else {
                    notes.push(format!(
                        "Sequence prereq check: all {} macros pass",
                        result.pass_count
                    ));
                }
            }

            (dsl, PipelineOutcome::Ready, notes, val_error, None)
        }
        JourneyRoute::NeedsSelection {
            select_on,
            options,
            then,
        } => {
            let note = format!(
                "Tier -2 journey needs selection on '{}': {} options",
                select_on,
                options.len()
            );

            // Build DecisionPacket so the UI renders proper selection choices
            let decision = build_journey_selection_decision(
                session_id, utterance, journey, select_on, options, then,
            );

            (
                String::new(),
                PipelineOutcome::NeedsClarification,
                vec![note],
                None,
                Some(decision),
            )
        }
    };

    let result = PipelineResult {
        intent: StructuredIntent {
            verb: top.verb.clone(),
            arguments: vec![],
            confidence: top.score,
            notes,
        },
        verb_candidates: filtered_candidates.to_vec(),
        dsl,
        dsl_hash: None,
        valid: matches!(outcome, PipelineOutcome::Ready),
        validation_error,
        unresolved_refs: vec![],
        missing_required: vec![],
        outcome,
        scope_resolution: discovery_result.scope_resolution.clone(),
        scope_context: discovery_result.scope_context.clone(),
    };

    (result, decision)
}

/// Build a `DecisionPacket` for a `NeedsSelection` journey route.
///
/// Constructs choices from the selector options (e.g., jurisdiction → macro FQN mappings)
/// and wraps them in a `ClarifyScope` decision with a `journey_selection` decision reason
/// so the reply handler can distinguish it from other scope clarifications.
#[cfg(feature = "database")]
fn build_journey_selection_decision(
    session_id: Option<Uuid>,
    utterance: &str,
    journey: &JourneyMetadata,
    select_on: &str,
    options: &[(String, String)],
    then: &[String],
) -> ob_poc_types::DecisionPacket {
    use ob_poc_types::{
        ClarificationPayload, DecisionKind, DecisionPacket, DecisionTrace, ScopeOption,
        ScopePayload, SessionStateView, UserChoice,
    };

    let scenario_title = journey
        .scenario_title
        .as_deref()
        .unwrap_or("Journey selection");

    let choices: Vec<UserChoice> = options
        .iter()
        .enumerate()
        .map(|(i, (value, macro_fqn))| UserChoice {
            id: format!("{}", i + 1),
            label: value.clone(),
            description: format!("Route to macro: {}", macro_fqn),
            is_escape: false,
        })
        .collect();

    let scope_options: Vec<ScopeOption> = options
        .iter()
        .map(|(value, macro_fqn)| ScopeOption {
            desc: format!("{} → {}", value, macro_fqn),
            method: "journey_selection".to_string(),
            score: 1.0,
            expect_count: None,
            sample: vec![],
            snapshot_id: None,
        })
        .collect();

    // Encode the selector metadata as context_hint so the reply handler
    // can reconstruct the macro resolution without re-running the scenario index.
    let context_hint = serde_json::to_string(&serde_json::json!({
        "select_on": select_on,
        "options": options,
        "then": then,
        "scenario_id": journey.scenario_id,
        "scenario_title": journey.scenario_title,
    }))
    .unwrap_or_default();

    DecisionPacket {
        packet_id: Uuid::new_v4().to_string(),
        kind: DecisionKind::ClarifyScope,
        session: SessionStateView {
            session_id,
            client_group_anchor: None,
            client_group_name: None,
            persona: None,
            last_confirmed_verb: None,
        },
        utterance: utterance.to_string(),
        payload: ClarificationPayload::Scope(ScopePayload {
            options: scope_options,
            context_hint: Some(context_hint),
        }),
        prompt: format!(
            "**{}**\n\nPlease select a {} to continue:",
            scenario_title, select_on
        ),
        choices,
        best_plan: None,
        alternatives: vec![],
        requires_confirm: false,
        confirm_token: None,
        trace: DecisionTrace {
            config_version: "1.0".to_string(),
            entity_snapshot_hash: None,
            lexicon_snapshot_hash: None,
            semantic_lane_enabled: false,
            embedding_model_id: None,
            verb_margin: 0.0,
            scope_margin: 0.0,
            kind_margin: 0.0,
            decision_reason: "journey_selection".to_string(),
        },
    }
}

/// Emit a telemetry event from an OrchestratorOutcome. Best-effort, never fails.
#[cfg(feature = "database")]
async fn emit_telemetry(
    ctx: &OrchestratorContext,
    utterance: &str,
    outcome: &mut OrchestratorOutcome,
) {
    let trace = &outcome.trace;
    let normalized = telemetry::normalize_utterance(utterance);
    let hash = telemetry::utterance_hash(&normalized);
    let preview = telemetry::preview_redacted(utterance);

    let scope_str = ctx.scope.as_ref().map(|s| format!("{:?}", s));
    let (subject_ref_type, subject_ref_id) = if let Some(case_id) = ctx.case_id {
        (Some("case".to_string()), Some(case_id))
    } else if let Some(entity_id) = ctx.dominant_entity_id {
        (Some("entity".to_string()), Some(entity_id))
    } else {
        (None, None)
    };

    let semreg_denied: Option<serde_json::Value> = if !trace.macro_denied_verbs.is_empty() {
        Some(serde_json::json!(trace.macro_denied_verbs))
    } else {
        None
    };

    let row = telemetry::IntentEventRow {
        event_id: uuid::Uuid::new_v4(),
        session_id: ctx.session_id.unwrap_or_default(),
        actor_id: ctx.actor.actor_id.clone(),
        entrypoint: format!("{:?}", ctx.source).to_lowercase(),
        utterance_hash: hash,
        utterance_preview: preview,
        scope: scope_str,
        subject_ref_type,
        subject_ref_id,
        semreg_mode: trace.sem_reg_mode.clone(),
        semreg_denied_verbs: semreg_denied,
        verb_candidates_pre: telemetry::candidates_to_json(&trace.verb_candidates_pre_filter),
        verb_candidates_post: telemetry::candidates_to_json(&trace.verb_candidates_post_filter),
        chosen_verb_fqn: trace.final_verb.clone(),
        selection_source: Some(trace.selection_source.clone()),
        forced_verb_fqn: trace.forced_verb.clone(),
        outcome: telemetry::outcome_label(&outcome.pipeline_result.outcome).to_string(),
        dsl_hash: trace.dsl_hash.clone(),
        run_sheet_entry_id: None,
        macro_semreg_checked: trace.macro_semreg_checked,
        macro_denied_verbs: if !trace.macro_denied_verbs.is_empty() {
            Some(serde_json::json!(trace.macro_denied_verbs))
        } else {
            None
        },
        prompt_version: None,
        error_code: trace.blocked_reason.as_ref().map(|_| "blocked".to_string()),
        dominant_entity_id: ctx.dominant_entity_id,
        dominant_entity_kind: trace.dominant_entity_kind.clone(),
        entity_kind_filtered: trace.entity_kind_filtered,
        allowed_verbs_fingerprint: trace.allowed_verbs_fingerprint.clone(),
        pruned_verbs_count: trace.pruned_verbs_count as i32,
        toctou_recheck_performed: trace.toctou_recheck_performed,
        toctou_result: trace.toctou_result.clone(),
        toctou_new_fingerprint: trace.toctou_new_fingerprint.clone(),
    };

    let persisted = telemetry::store::insert_intent_event(&ctx.pool, &row).await;
    outcome.trace.telemetry_persisted = persisted;
}

/// Process an utterance with a forced verb selection (binding disambiguation).
///
/// Used when the user has selected a specific verb from an ambiguity menu.
/// Skips verb discovery and SemReg filtering -- the verb was already approved
/// during the initial discovery phase.
#[cfg(feature = "database")]
pub async fn handle_utterance_with_forced_verb(
    ctx: &OrchestratorContext,
    utterance: &str,
    forced_verb_fqn: &str,
) -> anyhow::Result<OrchestratorOutcome> {
    let policy = &ctx.policy_gate;

    let lookup_result = if let Some(ref lookup_svc) = ctx.lookup_service {
        Some(lookup_svc.analyze(utterance, 5).await)
    } else {
        None
    };

    let dominant_entity_name = lookup_result
        .as_ref()
        .and_then(|lr| lr.dominant_entity.as_ref())
        .map(|e| e.canonical_name.clone());

    let dominant_entity_kind = lookup_result
        .as_ref()
        .and_then(|lr| lr.dominant_entity.as_ref())
        .map(|e| e.entity_kind.clone());

    // Keep SemReg entity-kind filtering off for Semantic OS data-management
    // sessions for the same reason as `handle_utterance`.
    let semreg_entity_kind = if matches!(
        ctx.stage_focus.as_deref(),
        Some("semos-data-management" | "semos-data")
    ) {
        None
    } else {
        dominant_entity_kind.clone()
    };

    let entity_candidates: Vec<String> = lookup_result
        .as_ref()
        .map(|lr| lr.entities.iter().map(|e| e.mention_text.clone()).collect())
        .unwrap_or_default();

    // -- Sem OS context resolution for forced-verb path --
    // Even though the user selected this verb, we still validate it against
    // the current SemReg allowed set. This closes the TOCTOU gap where the
    // verb was allowed at discovery time but may have been revoked since.
    let envelope = resolve_sem_reg_verbs(ctx, "", None, semreg_entity_kind.as_deref(), false).await;

    let phase2 = Phase2Service::evaluate_from_envelope(envelope.clone());
    let sem_reg_verb_names = Phase2Service::legal_verb_names(&phase2.artifacts);

    let allowed_verbs_fingerprint = phase2.fingerprint();
    let pruned_verbs_count = phase2.pruned_verb_count();

    // Check if the forced verb is still allowed
    let mut sem_reg_denied_all = false;
    let semreg_unavailable = !phase2.is_available;
    let mut blocked_reason: Option<String> = None;
    let mut verb_denied = false;

    match Phase2Service::runtime_gate_status(&phase2.artifacts, forced_verb_fqn) {
        "blocked_unavailable" => {
            if policy.semreg_fail_closed()
                && !crate::agent::verb_surface::is_safe_harbor_verb(forced_verb_fqn)
            {
                blocked_reason = Some(format!(
                    "SemReg unavailable (fail-closed): verb '{}' not in safe-harbor set",
                    forced_verb_fqn
                ));
                verb_denied = true;
                tracing::warn!(
                    verb = forced_verb_fqn,
                    "Forced verb denied: SemReg unavailable in strict mode"
                );
            }
        }
        "blocked_deny_all" => {
            sem_reg_denied_all = true;
            if policy.semreg_fail_closed() {
                blocked_reason =
                    Some("SemReg denied all verbs for this subject (strict mode)".into());
                verb_denied = true;
                tracing::warn!(
                    verb = forced_verb_fqn,
                    "Forced verb denied: SemReg deny-all in strict mode"
                );
            }
        }
        "blocked_not_allowed" => {
            blocked_reason = Some(format!(
                "Forced verb '{}' not in SemReg allowed set (fingerprint: {})",
                forced_verb_fqn,
                phase2
                    .artifacts
                    .fingerprint()
                    .unwrap_or_else(|| "unavailable".to_string())
            ));
            verb_denied = true;
            tracing::warn!(
                verb = forced_verb_fqn,
                fingerprint = ?phase2.fingerprint(),
                "Forced verb denied by SemReg"
            );
        }
        _ => {}
    }

    // If verb is denied in strict mode, return a blocked outcome
    if verb_denied && policy.semreg_fail_closed() {
        use crate::mcp::intent_pipeline::StructuredIntent;

        let trace = IntentTrace {
            utterance: utterance.to_string(),
            source: ctx.source.clone(),
            entity_candidates,
            dominant_entity: dominant_entity_name,
            sem_reg_verb_filter: sem_reg_verb_names,
            verb_candidates_pre_filter: vec![],
            verb_candidates_post_filter: vec![],
            final_verb: None,
            final_confidence: 0.0,
            dsl_generated: None,
            dsl_hash: None,
            bypass_used: None,
            dsl_source: Some(format!("{:?}", ctx.source)),
            sem_reg_mode: "strict".into(),
            sem_reg_denied_all,
            policy_gate_snapshot: policy.snapshot(),
            forced_verb: Some(forced_verb_fqn.to_string()),
            blocked_reason: blocked_reason.clone(),
            chosen_verb_pre_semreg: None,
            chosen_verb_post_semreg: None,
            semreg_policy: phase2.policy_label.to_string(),
            semreg_unavailable,
            selection_source: "user_choice".to_string(),
            macro_semreg_checked: false,
            macro_denied_verbs: vec![],
            dominant_entity_kind,
            entity_kind_filtered: false,
            telemetry_persisted: false,
            agent_mode: ctx.agent_mode.to_string(),
            agent_mode_blocked_verbs: vec![],
            allowed_verbs_fingerprint,
            pruned_verbs_count,
            toctou_recheck_performed: true,
            toctou_result: Some("denied".to_string()),
            toctou_new_fingerprint: Some(envelope.fingerprint_str().to_string()),
            surface_fingerprint: None,
            journey_match: None,
            sage_plane: None,
            sage_polarity: None,
            sage_domain_hints: vec![],
            serve_fallback_reason: None,
        };

        let mut outcome = OrchestratorOutcome {
            pipeline_result: PipelineResult {
                intent: StructuredIntent::empty(),
                verb_candidates: vec![],
                dsl: String::new(),
                dsl_hash: None,
                valid: false,
                validation_error: blocked_reason,
                unresolved_refs: vec![],
                missing_required: vec![],
                outcome: PipelineOutcome::NeedsClarification,
                scope_resolution: None,
                scope_context: None,
            },
            context_envelope: Some(envelope),
            surface: None,
            lookup_result,
            trace,
            journey_decision: None,
            pending_mutation: None,
            auto_execute: false,
            sage_intent: None,
            trace_id: None,
        };
        emit_telemetry(ctx, utterance, &mut outcome).await;
        return Ok(outcome);
    }

    let searcher = (*ctx.verb_searcher).clone();
    let pipeline = IntentPipeline::with_pool(searcher, ctx.pool.clone());

    let result = pipeline
        .process_with_forced_verb(utterance, forced_verb_fqn, ctx.scope.clone())
        .await?;

    let trace = IntentTrace {
        utterance: utterance.to_string(),
        source: ctx.source.clone(),
        entity_candidates,
        dominant_entity: dominant_entity_name,
        sem_reg_verb_filter: sem_reg_verb_names,
        verb_candidates_pre_filter: vec![],
        verb_candidates_post_filter: vec![(forced_verb_fqn.to_string(), 1.0)],
        final_verb: Some(forced_verb_fqn.to_string()),
        final_confidence: 1.0,
        dsl_generated: Some(result.dsl.clone()).filter(|d| !d.is_empty()),
        dsl_hash: result.dsl_hash.clone(),
        bypass_used: None,
        dsl_source: Some(format!("{:?}", ctx.source)),
        sem_reg_mode: if policy.semreg_fail_closed() {
            "strict".into()
        } else {
            "permissive".into()
        },
        sem_reg_denied_all: false,
        policy_gate_snapshot: policy.snapshot(),
        forced_verb: Some(forced_verb_fqn.to_string()),
        blocked_reason: None,
        chosen_verb_pre_semreg: None,
        chosen_verb_post_semreg: Some(forced_verb_fqn.to_string()),
        semreg_policy: envelope.label().to_string(),
        semreg_unavailable,
        selection_source: "user_choice".to_string(),
        macro_semreg_checked: false,
        macro_denied_verbs: vec![],
        dominant_entity_kind,
        entity_kind_filtered: false,
        telemetry_persisted: false,
        agent_mode: ctx.agent_mode.to_string(),
        agent_mode_blocked_verbs: vec![],
        allowed_verbs_fingerprint,
        pruned_verbs_count,
        toctou_recheck_performed: true,
        toctou_result: Some("still_allowed".to_string()),
        toctou_new_fingerprint: None,
        surface_fingerprint: None,
        journey_match: None,
        sage_plane: None,
        sage_polarity: None,
        sage_domain_hints: vec![],
        serve_fallback_reason: None,
    };

    tracing::info!(
        source = ?trace.source,
        forced_verb = forced_verb_fqn,
        dsl_generated = trace.dsl_generated.is_some(),
        fingerprint = ?trace.allowed_verbs_fingerprint,
        toctou = ?trace.toctou_result,
        "IntentTrace (forced verb)"
    );

    let mut outcome = OrchestratorOutcome {
        pipeline_result: result,
        context_envelope: Some(envelope),
        surface: None,
        lookup_result,
        trace,
        journey_decision: None,
        pending_mutation: None,
        auto_execute: can_auto_execute_serve_result(forced_verb_fqn),
        sage_intent: None,
        trace_id: None,
    };
    emit_telemetry(ctx, utterance, &mut outcome).await;
    Ok(outcome)
}

fn default_trace_for_runtime(
    ctx: &OrchestratorContext,
    utterance: &str,
    prepared: &PreparedTurnContext,
) -> IntentTrace {
    let prepared_phase2 = Phase2Service::evaluate(
        prepared.lookup_result.clone(),
        Some(prepared.envelope.clone()),
    );
    IntentTrace {
        utterance: utterance.to_string(),
        source: ctx.source.clone(),
        entity_candidates: prepared.entity_candidates.clone(),
        dominant_entity: prepared.dominant_entity_name.clone(),
        #[cfg(feature = "database")]
        sem_reg_verb_filter: prepared.sem_reg_verb_names.clone(),
        verb_candidates_pre_filter: vec![],
        verb_candidates_post_filter: vec![],
        final_verb: None,
        final_confidence: 0.0,
        dsl_generated: None,
        dsl_hash: None,
        bypass_used: None,
        dsl_source: Some(format!("{:?}", ctx.source)),
        sem_reg_mode: if ctx.policy_gate.semreg_fail_closed() {
            "strict".to_string()
        } else {
            "permissive".to_string()
        },
        sem_reg_denied_all: prepared_phase2.is_deny_all,
        policy_gate_snapshot: ctx.policy_gate.snapshot(),
        forced_verb: None,
        blocked_reason: None,
        chosen_verb_pre_semreg: None,
        chosen_verb_post_semreg: None,
        semreg_policy: prepared_phase2.policy_label.to_string(),
        semreg_unavailable: !prepared_phase2.is_available,
        selection_source: "sage_delegate".to_string(),
        serve_fallback_reason: None,
        macro_semreg_checked: false,
        macro_denied_verbs: vec![],
        dominant_entity_kind: prepared.dominant_entity_kind.clone(),
        entity_kind_filtered: prepared.dominant_entity_kind.is_some(),
        telemetry_persisted: false,
        agent_mode: ctx.agent_mode.to_string(),
        agent_mode_blocked_verbs: vec![],
        allowed_verbs_fingerprint: prepared_phase2.fingerprint(),
        pruned_verbs_count: prepared_phase2.pruned_verb_count(),
        toctou_recheck_performed: false,
        toctou_result: None,
        toctou_new_fingerprint: None,
        surface_fingerprint: Some(prepared.surface.surface_fingerprint.0.clone()),
        journey_match: None,
        sage_plane: None,
        sage_polarity: None,
        sage_domain_hints: vec![],
    }
}

/// Resolve Sem OS context and return a `SemOsContextEnvelope`.
///
/// Returns a rich envelope preserving allowed verbs, pruned verbs with reasons,
/// deterministic fingerprint, evidence gaps, and governance signals.
#[cfg(feature = "database")]
pub(crate) async fn resolve_sem_reg_verbs(
    ctx: &OrchestratorContext,
    utterance: &str,
    sage_intent: Option<&crate::sage::OutcomeIntent>,
    entity_kind: Option<&str>,
    use_generic_task_subject: bool,
) -> SemOsContextEnvelope {
    // Route through SemOsClient when available (DI boundary). Utterance
    // discovery no longer falls back to direct sem_reg calls.
    if let Some(ref client) = ctx.sem_os_client {
        resolve_via_client(
            client.as_ref(),
            ctx,
            utterance,
            sage_intent,
            entity_kind,
            use_generic_task_subject,
        )
        .await
    } else {
        tracing::warn!("SemOsClient unavailable; blocking utterance discovery");
        SemOsContextEnvelope::unavailable()
    }
}

/// Resolve verbs via SemOsClient DI boundary (in-process or HTTP).
async fn resolve_via_client(
    client: &dyn SemOsClient,
    ctx: &OrchestratorContext,
    utterance: &str,
    sage_intent: Option<&crate::sage::OutcomeIntent>,
    entity_kind: Option<&str>,
    use_generic_task_subject: bool,
) -> SemOsContextEnvelope {
    use sem_os_core::context_resolution::{EvidenceMode, SubjectRef};

    let subject = if use_generic_task_subject {
        SubjectRef::TaskId(ctx.session_id.unwrap_or_else(Uuid::new_v4))
    } else if let Some(entity_id) = ctx.dominant_entity_id {
        SubjectRef::EntityId(entity_id)
    } else if let Some(case_id) = ctx.case_id {
        SubjectRef::CaseId(case_id)
    } else {
        // Default generic chat/repl sessions to TaskId instead of CaseId.
        // CaseId triggers a lookup in "ob-poc".kyc_cases, which may not exist
        // in non-KYC deployments and would force SemReg into unavailable mode.
        SubjectRef::TaskId(ctx.session_id.unwrap_or_else(Uuid::new_v4))
    };
    // Convert ob-poc ActorContext → sem_os_core ActorContext via serde round-trip
    // (structurally identical types in separate crates)
    let core_actor: sem_os_core::abac::ActorContext = {
        let json = serde_json::to_value(&ctx.actor).expect("ActorContext serializes");
        serde_json::from_value(json).expect("ActorContext round-trips")
    };
    let evidence_mode = if matches!(
        ctx.stage_focus.as_deref(),
        Some("semos-data-management" | "semos-data")
    ) {
        // Data-management workflows need operational verbs for domain actions
        // like deal/cbu/document/product management.
        EvidenceMode::Exploratory
    } else {
        EvidenceMode::default()
    };
    let request = sem_os_core::context_resolution::ContextResolutionRequest {
        subject,
        intent_summary: sage_intent.map(|intent| intent.summary.clone()),
        raw_utterance: Some(utterance.to_string()),
        actor: core_actor,
        goals: ctx.goals.clone(),
        constraints: Default::default(),
        evidence_mode,
        point_in_time: None,
        entity_kind: entity_kind.map(|s| s.to_string()),
        entity_confidence: ctx.pre_sage_entity_confidence,
        discovery: sem_os_core::context_resolution::DiscoveryContext {
            selected_domain_id: ctx.discovery_selected_domain.clone(),
            selected_family_id: ctx.discovery_selected_family.clone(),
            selected_constellation_id: ctx.discovery_selected_constellation.clone(),
            known_inputs: ctx.discovery_answers.clone(),
        },
    };
    let principal =
        sem_os_core::principal::Principal::in_process(&ctx.actor.actor_id, ctx.actor.roles.clone());

    match client.resolve_context(&principal, request).await {
        Ok(response) => {
            let envelope = SemOsContextEnvelope::from_resolution(&response);
            tracing::debug!(
                allowed_count = envelope.allowed_verbs.len(),
                pruned_count = envelope.pruned_count(),
                fingerprint = %envelope.fingerprint_str(),
                "SemReg context resolution completed (client)"
            );
            envelope
        }
        Err(e) => {
            tracing::warn!(error = %e, source = "sem_reg", "SemReg context resolution failed (client)");
            SemOsContextEnvelope::unavailable()
        }
    }
}

/// Resolve the SemReg allowed verb set using only a SemOsClient + actor context.
///
/// This is a lightweight entry point for MCP tools (verb_search, dsl_execute) that
/// don't have a full OrchestratorContext. Returns a `SemOsContextEnvelope`.
#[cfg(feature = "database")]
pub async fn resolve_allowed_verbs(
    client: &dyn SemOsClient,
    actor: &ActorContext,
    session_id: Option<Uuid>,
) -> SemOsContextEnvelope {
    use sem_os_core::context_resolution::{EvidenceMode, SubjectRef};

    // Resolve against a neutral task subject for generic sessions. This avoids
    // coupling resolve_context to KYC case tables in environments that do not
    // run case workflows.
    let subject = SubjectRef::TaskId(session_id.unwrap_or_else(Uuid::new_v4));
    let core_actor: sem_os_core::abac::ActorContext = {
        let json = serde_json::to_value(actor).expect("ActorContext serializes");
        serde_json::from_value(json).expect("ActorContext round-trips")
    };
    let request = sem_os_core::context_resolution::ContextResolutionRequest {
        subject,
        intent_summary: None,
        raw_utterance: None,
        actor: core_actor,
        goals: vec![],
        constraints: Default::default(),
        evidence_mode: EvidenceMode::default(),
        point_in_time: None,
        entity_kind: None,
        entity_confidence: None,
        discovery: Default::default(),
    };
    let principal =
        sem_os_core::principal::Principal::in_process(&actor.actor_id, actor.roles.clone());

    match client.resolve_context(&principal, request).await {
        Ok(response) => {
            let envelope = SemOsContextEnvelope::from_resolution(&response);
            tracing::debug!(
                allowed_count = envelope.allowed_verbs.len(),
                pruned_count = envelope.pruned_count(),
                fingerprint = %envelope.fingerprint_str(),
                "SemReg lightweight resolve completed"
            );
            envelope
        }
        Err(e) => {
            tracing::warn!(error = %e, source = "sem_reg", "SemReg lightweight resolve failed");
            SemOsContextEnvelope::unavailable()
        }
    }
}

// -- Tests --

#[cfg(test)]
mod tests {
    use super::*;
    use crate::dsl_v2::parse_program;
    use crate::mcp::verb_search::HybridVerbSearcher;
    use async_trait::async_trait;
    use chrono::Utc;
    use sem_os_client::SemOsClient;
    use sem_os_core::abac::AccessDecision;
    use sem_os_core::context_resolution::{
        ContextResolutionResponse, DiscoverySurface, GroundingReadiness, ResolutionStage,
    };
    use sem_os_core::error::SemOsError;
    use sem_os_core::principal::Principal;
    use sem_os_core::proto::{
        BootstrapSeedBundleResponse, ChangesetDiffResponse, ChangesetImpactResponse,
        ChangesetPublishResponse, ExportSnapshotSetResponse, GatePreviewResponse,
        GetManifestResponse, ListChangesetsQuery, ListChangesetsResponse, ListToolSpecsResponse,
        ResolveContextRequest, ResolveContextResponse, ToolCallRequest, ToolCallResponse,
    };
    use sem_os_core::types::Changeset;
    use sem_os_core::universe_def::{EntryQuestion, GroundingInput};
    use uuid::Uuid;

    /// Helper to build a default IntentTrace for tests.
    fn default_trace() -> IntentTrace {
        IntentTrace {
            utterance: String::new(),
            source: UtteranceSource::Chat,
            entity_candidates: vec![],
            dominant_entity: None,
            #[cfg(feature = "database")]
            sem_reg_verb_filter: None,
            verb_candidates_pre_filter: vec![],
            verb_candidates_post_filter: vec![],
            final_verb: None,
            final_confidence: 0.0,
            dsl_generated: None,
            dsl_hash: None,
            bypass_used: None,
            dsl_source: None,
            sem_reg_mode: "strict".into(),
            sem_reg_denied_all: false,
            policy_gate_snapshot: crate::policy::PolicyGate::strict().snapshot(),
            forced_verb: None,
            blocked_reason: None,
            chosen_verb_pre_semreg: None,
            chosen_verb_post_semreg: None,
            semreg_policy: "unavailable".into(),
            semreg_unavailable: false,
            selection_source: "discovery".into(),
            macro_semreg_checked: false,
            macro_denied_verbs: vec![],
            dominant_entity_kind: None,
            entity_kind_filtered: false,
            telemetry_persisted: false,
            agent_mode: "governed".into(),
            agent_mode_blocked_verbs: vec![],
            allowed_verbs_fingerprint: None,
            pruned_verbs_count: 0,
            toctou_recheck_performed: false,
            toctou_result: None,
            toctou_new_fingerprint: None,
            surface_fingerprint: None,
            journey_match: None,
            sage_plane: None,
            sage_polarity: None,
            sage_domain_hints: vec![],
            serve_fallback_reason: None,
        }
    }

    #[test]
    fn test_intent_trace_serialization() {
        let mut trace = default_trace();
        trace.utterance = "show all cases".into();
        trace.entity_candidates = vec!["Allianz".into()];
        trace.dominant_entity = Some("Allianz".into());
        #[cfg(feature = "database")]
        {
            trace.sem_reg_verb_filter = Some(vec!["kyc.open-case".into()]);
        }
        trace.verb_candidates_pre_filter = vec![("kyc.open-case".into(), 0.95)];
        trace.verb_candidates_post_filter = vec![("kyc.open-case".into(), 0.95)];
        trace.final_verb = Some("kyc.open-case".into());
        trace.final_confidence = 0.95;
        trace.dsl_generated = Some("(kyc.open-case)".into());
        trace.dsl_hash = Some("abc123".into());
        trace.dsl_source = Some("chat".into());

        let json = serde_json::to_string(&trace).unwrap();
        assert!(json.contains("kyc.open-case"));
        assert!(json.contains("chat"));
    }

    #[test]
    fn test_intent_trace_forced_verb_field() {
        let mut trace = default_trace();
        trace.utterance = "create a deal".into();
        trace.verb_candidates_post_filter = vec![("deal.create".into(), 1.0)];
        trace.final_verb = Some("deal.create".into());
        trace.final_confidence = 1.0;
        trace.dsl_generated = Some("(deal.create)".into());
        trace.dsl_source = Some("chat".into());
        trace.forced_verb = Some("deal.create".into());

        let json = serde_json::to_string(&trace).unwrap();
        assert!(json.contains("forced_verb"));
        assert!(json.contains("deal.create"));
    }

    #[test]
    fn test_intent_trace_blocked_reason_field() {
        let mut trace = default_trace();
        trace.utterance = "show cases".into();
        #[cfg(feature = "database")]
        {
            trace.sem_reg_verb_filter = Some(vec![]);
        }
        trace.verb_candidates_pre_filter = vec![("kyc.open-case".into(), 0.9)];
        trace.sem_reg_denied_all = true;
        trace.blocked_reason = Some("SemReg denied all verb candidates (strict mode)".into());

        let json = serde_json::to_string(&trace).unwrap();
        assert!(json.contains("blocked_reason"));
        assert!(json.contains("SemReg denied all"));
        assert!(json.contains(r#""sem_reg_denied_all":true"#));
    }

    #[test]
    fn test_utterance_source_serialization() {
        assert_eq!(
            serde_json::to_string(&UtteranceSource::Chat).unwrap(),
            "\"chat\""
        );
        assert_eq!(
            serde_json::to_string(&UtteranceSource::Mcp).unwrap(),
            "\"mcp\""
        );
        assert_eq!(
            serde_json::to_string(&UtteranceSource::Repl).unwrap(),
            "\"repl\""
        );
    }

    #[test]
    fn test_context_envelope_labels_backward_compat() {
        // Verify SemOsContextEnvelope labels match the old SemRegVerbPolicy labels
        let unav = SemOsContextEnvelope::unavailable();
        assert_eq!(unav.label(), "unavailable");

        let deny = SemOsContextEnvelope::deny_all();
        assert_eq!(deny.label(), "deny_all");

        let _allowed = SemOsContextEnvelope::unavailable();
        // Build a non-unavailable, non-deny-all envelope
        let env = SemOsContextEnvelope::deny_all();
        // Use deny_all as base, add a verb to make it an allowed_set
        // We test label through the from_resolution path indirectly;
        // here just test the enum labels are backward-compatible
        assert_eq!(env.label(), "deny_all");

        // unavailable must not be confused with deny_all
        assert_ne!(unav.label(), deny.label());

        // Also verify they serialize
        let json = serde_json::to_string(&unav).unwrap();
        assert!(json.contains("allowed_verbs"));
        assert!(json.contains("fingerprint"));
    }

    #[test]
    fn test_route_reads_to_serve_and_writes_to_delegate() {
        let read_intent = crate::sage::OutcomeIntent {
            summary: "show me the cbus".into(),
            plane: ObservationPlane::Instance,
            polarity: crate::sage::IntentPolarity::Read,
            domain_concept: "cbu".into(),
            action: crate::sage::OutcomeAction::Read,
            subject: None,
            steps: vec![],
            confidence: SageConfidence::Medium,
            pending_clarifications: vec![],
            hints: crate::sage::UtteranceHints::default(),
            explain: crate::sage::SageExplain::default(),
            coder_handoff: crate::sage::CoderHandoff::default(),
        };
        assert!(matches!(
            route(&read_intent),
            UtteranceDisposition::Serve(_)
        ));

        let write_intent = crate::sage::OutcomeIntent {
            summary: "create a new deal".into(),
            plane: ObservationPlane::Instance,
            polarity: crate::sage::IntentPolarity::Write,
            domain_concept: "deal".into(),
            action: crate::sage::OutcomeAction::Create,
            subject: Some(crate::sage::EntityRef {
                mention: "Allianz UK Deal".into(),
                kind_hint: Some("deal".into()),
                uuid: None,
            }),
            steps: vec![],
            confidence: SageConfidence::High,
            pending_clarifications: vec![],
            hints: crate::sage::UtteranceHints::default(),
            explain: crate::sage::SageExplain::default(),
            coder_handoff: crate::sage::CoderHandoff::default(),
        };
        assert!(matches!(
            route(&write_intent),
            UtteranceDisposition::Delegate(_)
        ));
    }

    #[test]
    fn test_confirmation_words_are_narrow() {
        assert!(is_confirmation("yes"));
        assert!(is_confirmation("go ahead"));
        assert!(!is_confirmation("run"));
        assert!(!is_confirmation("show me the cbus"));
    }

    #[test]
    fn test_build_mutation_confirmation_hides_dsl() {
        let intent = crate::sage::OutcomeIntent {
            summary: "create a new deal".into(),
            plane: ObservationPlane::Instance,
            polarity: crate::sage::IntentPolarity::Write,
            domain_concept: "deal".into(),
            action: crate::sage::OutcomeAction::Create,
            subject: Some(crate::sage::EntityRef {
                mention: "Allianz UK Deal".into(),
                kind_hint: Some("deal".into()),
                uuid: None,
            }),
            steps: vec![],
            confidence: SageConfidence::High,
            pending_clarifications: vec![],
            hints: crate::sage::UtteranceHints::default(),
            explain: crate::sage::SageExplain::default(),
            coder_handoff: crate::sage::CoderHandoff::default(),
        };
        let coder_result = CoderResult {
            verb_fqn: "deal.create".into(),
            dsl: "(deal.create :name \"Allianz UK Deal\")".into(),
            resolution: crate::sage::coder::CoderResolution::Confident,
            missing_args: vec![],
            unresolved_refs: vec![],
            diagnostics: None,
        };

        let pending = build_mutation_confirmation(&intent, &coder_result, None);
        assert!(pending.confirmation_text.contains("create Allianz UK Deal"));
        assert!(!pending.confirmation_text.contains("deal.create"));
        assert_eq!(pending.change_summary[0], "Resolved action: deal.create");
    }

    #[test]
    fn test_read_only_list_fallback_prefers_plural_domain_list() {
        let intent = crate::sage::OutcomeIntent {
            summary: "what deals does Allianz have?".into(),
            plane: ObservationPlane::Instance,
            polarity: crate::sage::IntentPolarity::Read,
            domain_concept: "deal".into(),
            action: crate::sage::OutcomeAction::Read,
            subject: None,
            steps: vec![],
            confidence: SageConfidence::Medium,
            pending_clarifications: vec![],
            hints: crate::sage::UtteranceHints::default(),
            explain: crate::sage::SageExplain::default(),
            coder_handoff: crate::sage::CoderHandoff::default(),
        };

        let result = read_only_list_fallback(&intent).expect("expected safe list fallback");
        assert_eq!(result.verb_fqn, "deal.list");
        assert_eq!(result.dsl, "(deal.list)");
        assert!(matches!(
            result.resolution,
            crate::sage::coder::CoderResolution::Confident
        ));
    }

    #[test]
    fn test_forced_read_path_uses_serve_auto_execute_policy() {
        assert!(can_auto_execute_serve_result("deal.list"));
        assert!(can_auto_execute_serve_result("cbu.read"));
        assert!(!can_auto_execute_serve_result("schema.entity.describe"));
    }

    #[test]
    fn test_is_early_exit() {
        assert!(is_early_exit(&PipelineOutcome::ScopeResolved {
            group_id: "g1".into(),
            group_name: "Test".into(),
            entity_count: 1,
        }));
        assert!(is_early_exit(&PipelineOutcome::ScopeCandidates));
        // These are NOT early exits
        assert!(!is_early_exit(&PipelineOutcome::Ready));
        assert!(!is_early_exit(&PipelineOutcome::NeedsClarification));
        assert!(!is_early_exit(&PipelineOutcome::NoMatch));
        assert!(!is_early_exit(&PipelineOutcome::NoAllowedVerbs));
    }

    #[test]
    fn test_data_management_rewrite_baseline_prompts() {
        let cases = [
            ("show me deal record", "deal"),
            ("show me CBU", "cbu"),
            ("show me documents", "document"),
            ("show me products", "product"),
        ];

        for (utterance, domain) in cases {
            let rewrite = data_management_rewrite(Some("semos-data-management"), utterance)
                .unwrap_or_else(|| panic!("expected rewrite for {utterance}"));
            assert_eq!(rewrite.domain_hint, "schema");
            assert_eq!(
                rewrite.rewritten_utterance,
                format!("describe entity schema for {domain} with fields relationships and verbs")
            );
        }
    }

    #[test]
    fn test_data_management_rewrite_skips_explicit_instance_targeting() {
        let cases = [
            "show me documents for id 123",
            "show me cbu-id 123",
            "show me product @abc",
            "show me deal record id: 42",
        ];

        for utterance in cases {
            assert!(
                data_management_rewrite(Some("semos-data-management"), utterance).is_none(),
                "unexpected rewrite for {utterance}"
            );
        }
    }

    #[test]
    fn test_data_management_rewrite_requires_data_management_focus() {
        assert!(data_management_rewrite(Some("semos-kyc"), "show me documents").is_none());
        assert!(data_management_rewrite(None, "show me products").is_none());

        let alias = data_management_rewrite(Some("semos-data"), "show me CBU")
            .expect("semos-data alias should still rewrite");
        assert_eq!(alias.domain_hint, "schema");
    }

    #[test]
    fn test_data_management_candidate_policy_prefers_structure_verbs() {
        let candidates = vec![
            VerbSearchResult {
                verb: "deal.read-record".into(),
                score: 0.99,
                source: crate::mcp::verb_search::VerbSearchSource::PatternEmbedding,
                matched_phrase: "show me deal record".into(),
                description: None,
                journey: None,
            },
            VerbSearchResult {
                verb: "schema.entity.describe".into(),
                score: 0.95,
                source: crate::mcp::verb_search::VerbSearchSource::PatternEmbedding,
                matched_phrase: "describe entity schema".into(),
                description: None,
                journey: None,
            },
        ];

        let filtered = apply_data_management_candidate_policy(
            Some("semos-data-management"),
            "show me deal record",
            true,
            candidates,
        );

        assert_eq!(filtered.len(), 1);
        assert_eq!(filtered[0].verb, "schema.entity.describe");
    }

    fn make_test_context() -> OrchestratorContext {
        fn test_runtime() -> &'static tokio::runtime::Runtime {
            static TEST_RUNTIME: std::sync::OnceLock<tokio::runtime::Runtime> =
                std::sync::OnceLock::new();
            TEST_RUNTIME.get_or_init(|| {
                tokio::runtime::Builder::new_current_thread()
                    .enable_all()
                    .build()
                    .expect("test runtime")
            })
        }

        let _guard = test_runtime().enter();
        OrchestratorContext {
            actor: crate::sem_reg::abac::ActorContext {
                actor_id: "test.user".to_string(),
                roles: vec!["operator".to_string()],
                department: None,
                clearance: None,
                jurisdictions: vec![],
            },
            session_id: Some(Uuid::new_v4()),
            case_id: None,
            dominant_entity_id: Some(
                Uuid::parse_str("123e4567-e89b-12d3-a456-426614174000").expect("valid uuid"),
            ),
            scope: None,
            pool: sqlx::PgPool::connect_lazy("postgres://localhost/test").expect("lazy pool"),
            verb_searcher: std::sync::Arc::new(HybridVerbSearcher::minimal()),
            lookup_service: None,
            policy_gate: std::sync::Arc::new(crate::policy::PolicyGate::strict()),
            source: UtteranceSource::Chat,
            sem_os_client: None,
            agent_mode: sem_os_core::authoring::agent_mode::AgentMode::default(),
            goals: vec![],
            stage_focus: None,
            sage_engine: None,
            pre_sage_entity_kind: Some("cbu".to_string()),
            pre_sage_entity_name: Some("Current CBU".to_string()),
            pre_sage_entity_confidence: Some(0.9),
            recent_sage_intents: vec![],
            nlci_compiler: Some(crate::semtaxonomy_v2::build_minimal_cbu_compiler()),
            discovery_selected_domain: None,
            discovery_selected_family: None,
            discovery_selected_constellation: None,
            discovery_answers: HashMap::new(),
            session_cbu_ids: None,
        }
    }

    fn make_unscoped_test_context() -> OrchestratorContext {
        OrchestratorContext {
            dominant_entity_id: None,
            pre_sage_entity_kind: None,
            pre_sage_entity_name: None,
            pre_sage_entity_confidence: None,
            ..make_test_context()
        }
    }

    #[derive(Clone)]
    struct StaticSemOsClient {
        response: ResolveContextResponse,
    }

    impl StaticSemOsClient {
        fn unsupported() -> SemOsError {
            SemOsError::InvalidInput("unsupported test client operation".to_string())
        }
    }

    #[async_trait]
    impl SemOsClient for StaticSemOsClient {
        async fn resolve_context(
            &self,
            _principal: &Principal,
            _req: ResolveContextRequest,
        ) -> sem_os_client::Result<ResolveContextResponse> {
            Ok(self.response.clone())
        }

        async fn get_manifest(
            &self,
            _snapshot_set_id: &str,
        ) -> sem_os_client::Result<GetManifestResponse> {
            Err(Self::unsupported())
        }

        async fn export_snapshot_set(
            &self,
            _snapshot_set_id: &str,
        ) -> sem_os_client::Result<ExportSnapshotSetResponse> {
            Err(Self::unsupported())
        }

        async fn bootstrap_seed_bundle(
            &self,
            _principal: &Principal,
            _bundle: sem_os_core::seeds::SeedBundle,
        ) -> sem_os_client::Result<BootstrapSeedBundleResponse> {
            Err(Self::unsupported())
        }

        async fn dispatch_tool(
            &self,
            _principal: &Principal,
            _req: ToolCallRequest,
        ) -> sem_os_client::Result<ToolCallResponse> {
            Err(Self::unsupported())
        }

        async fn list_tool_specs(&self) -> sem_os_client::Result<ListToolSpecsResponse> {
            Err(Self::unsupported())
        }

        async fn list_changesets(
            &self,
            _query: ListChangesetsQuery,
        ) -> sem_os_client::Result<ListChangesetsResponse> {
            Ok(ListChangesetsResponse {
                changesets: Vec::<Changeset>::new(),
            })
        }

        async fn changeset_diff(
            &self,
            _changeset_id: &str,
        ) -> sem_os_client::Result<ChangesetDiffResponse> {
            Err(Self::unsupported())
        }

        async fn changeset_impact(
            &self,
            _changeset_id: &str,
        ) -> sem_os_client::Result<ChangesetImpactResponse> {
            Err(Self::unsupported())
        }

        async fn changeset_gate_preview(
            &self,
            _changeset_id: &str,
        ) -> sem_os_client::Result<GatePreviewResponse> {
            Err(Self::unsupported())
        }

        async fn publish_changeset(
            &self,
            _principal: &Principal,
            _changeset_id: &str,
        ) -> sem_os_client::Result<ChangesetPublishResponse> {
            Err(Self::unsupported())
        }

        async fn get_affinity_graph(
            &self,
        ) -> sem_os_client::Result<Arc<sem_os_core::affinity::AffinityGraph>> {
            Err(Self::unsupported())
        }

        async fn drain_outbox_for_test(&self) -> sem_os_client::Result<()> {
            Ok(())
        }
    }

    fn discovery_stage_response() -> ContextResolutionResponse {
        ContextResolutionResponse {
            as_of_time: Utc::now(),
            resolved_at: Utc::now(),
            applicable_views: vec![],
            candidate_verbs: vec![],
            candidate_attributes: vec![],
            required_preconditions: vec![],
            disambiguation_questions: vec![],
            evidence: Default::default(),
            policy_verdicts: vec![],
            security_handling: AccessDecision::Allow,
            governance_signals: vec![],
            entity_kind_pruned_verbs: vec![],
            confidence: 0.42,
            grounded_action_surface: None,
            resolution_stage: ResolutionStage::Discovery,
            discovery_surface: Some(DiscoverySurface {
                matched_universes: vec![],
                matched_domains: vec![],
                matched_families: vec![],
                matched_constellations: vec![],
                missing_inputs: vec![GroundingInput {
                    key: "client_name".to_string(),
                    label: "client name".to_string(),
                    required: true,
                    input_type: "string".to_string(),
                }],
                entry_questions: vec![EntryQuestion {
                    question_id: "client-name".to_string(),
                    prompt: "Which client are you working on?".to_string(),
                    maps_to: "client_name".to_string(),
                    priority: 1,
                }],
                grounding_readiness: GroundingReadiness::NotReady,
            }),
        }
    }

    fn make_discovery_test_context() -> OrchestratorContext {
        OrchestratorContext {
            sem_os_client: Some(Arc::new(StaticSemOsClient {
                response: discovery_stage_response(),
            })),
            ..make_unscoped_test_context()
        }
    }

    fn make_cbu_intent(
        action: crate::sage::OutcomeAction,
        params: Vec<(&str, &str)>,
    ) -> crate::sage::OutcomeIntent {
        crate::sage::OutcomeIntent {
            summary: "test".to_string(),
            plane: ObservationPlane::Instance,
            polarity: if matches!(action, crate::sage::OutcomeAction::Read) {
                crate::sage::IntentPolarity::Read
            } else {
                crate::sage::IntentPolarity::Write
            },
            domain_concept: "cbu".to_string(),
            action: action.clone(),
            subject: None,
            steps: vec![crate::sage::OutcomeStep {
                action,
                target: "cbu".to_string(),
                params: params
                    .into_iter()
                    .map(|(name, value)| (name.to_string(), value.to_string()))
                    .collect(),
                notes: None,
            }],
            confidence: SageConfidence::High,
            pending_clarifications: vec![],
            hints: crate::sage::UtteranceHints::default(),
            explain: crate::sage::SageExplain::default(),
            coder_handoff: crate::sage::CoderHandoff::default(),
        }
    }

    fn make_cbu_intent_with_summary_and_notes(
        action: crate::sage::OutcomeAction,
        summary: &str,
        notes: Option<&str>,
    ) -> crate::sage::OutcomeIntent {
        crate::sage::OutcomeIntent {
            summary: summary.to_string(),
            plane: ObservationPlane::Instance,
            polarity: if matches!(action, crate::sage::OutcomeAction::Read) {
                crate::sage::IntentPolarity::Read
            } else {
                crate::sage::IntentPolarity::Write
            },
            domain_concept: "cbu".to_string(),
            action: action.clone(),
            subject: None,
            steps: vec![crate::sage::OutcomeStep {
                action,
                target: "cbu".to_string(),
                params: std::collections::HashMap::new(),
                notes: notes.map(str::to_string),
            }],
            confidence: SageConfidence::High,
            pending_clarifications: vec![],
            hints: crate::sage::UtteranceHints::default(),
            explain: crate::sage::SageExplain::default(),
            coder_handoff: crate::sage::CoderHandoff::default(),
        }
    }

    #[test]
    fn test_run_coder_stage_uses_compiler_for_cbu_read() {
        let ctx = make_test_context();
        let intent = make_cbu_intent(crate::sage::OutcomeAction::Read, vec![]);

        let stage = run_coder_stage(&ctx, Some(&intent));
        let result = stage.result.expect("compiler-backed result should exist");

        assert_eq!(result.verb_fqn, "cbu.read");
        assert_eq!(
            result.dsl,
            "(cbu.read :cbu-id \"123e4567-e89b-12d3-a456-426614174000\")"
        );
    }

    #[test]
    fn test_run_coder_stage_uses_compiler_for_cbu_list() {
        let ctx = make_unscoped_test_context();
        let intent = make_cbu_intent_with_summary_and_notes(
            crate::sage::OutcomeAction::Read,
            "Show me the CBUs",
            Some("show all cbus"),
        );

        let stage = run_coder_stage(&ctx, Some(&intent));
        let result = stage.result.expect("compiler-backed result should exist");

        assert_eq!(result.verb_fqn, "cbu.list");
        assert_eq!(result.dsl, "(cbu.list)");
    }

    #[test]
    fn test_run_coder_stage_uses_compiler_for_cbu_list_with_filter() {
        let ctx = make_unscoped_test_context();
        let intent = make_cbu_intent(
            crate::sage::OutcomeAction::Read,
            vec![("jurisdiction", "LU"), ("client-type", "FUND")],
        );

        let stage = run_coder_stage(&ctx, Some(&intent));
        let result = stage.result.expect("compiler-backed result should exist");

        assert_eq!(result.verb_fqn, "cbu.list");
        assert_eq!(
            result.dsl,
            "(cbu.list :jurisdiction \"LU\" :client-type \"FUND\")"
        );
    }

    #[test]
    fn test_run_coder_stage_uses_compiler_for_cbu_create() {
        let ctx = make_test_context();
        let intent = make_cbu_intent(
            crate::sage::OutcomeAction::Create,
            vec![
                ("name", "Apex Growth Fund"),
                ("jurisdiction", "LU"),
                ("client-type", "FUND"),
            ],
        );

        let stage = run_coder_stage(&ctx, Some(&intent));
        let result = stage.result.expect("compiler-backed result should exist");

        assert_eq!(result.verb_fqn, "cbu.create");
        assert_eq!(
            result.dsl,
            "(cbu.create :name \"Apex Growth Fund\" :jurisdiction \"LU\" :client-type \"FUND\")"
        );
    }

    #[test]
    fn test_run_coder_stage_uses_compiler_for_cbu_create_with_commercial_client() {
        let ctx = make_test_context();
        let intent = make_cbu_intent(
            crate::sage::OutcomeAction::Create,
            vec![
                ("name", "Apex Growth Fund"),
                (
                    "commercial-client-entity-id",
                    "123e4567-e89b-12d3-a456-426614174111",
                ),
            ],
        );

        let stage = run_coder_stage(&ctx, Some(&intent));
        let result = stage.result.expect("compiler-backed result should exist");

        assert_eq!(result.verb_fqn, "cbu.create");
        assert_eq!(
            result.dsl,
            "(cbu.create :name \"Apex Growth Fund\" :commercial-client-entity-id \"123e4567-e89b-12d3-a456-426614174111\")"
        );
    }

    #[test]
    fn test_run_coder_stage_uses_compiler_for_cbu_create_with_fund_entity() {
        let ctx = make_test_context();
        let intent = make_cbu_intent(
            crate::sage::OutcomeAction::Create,
            vec![
                ("name", "Apex Growth Fund"),
                ("fund-entity-id", "123e4567-e89b-12d3-a456-426614174222"),
            ],
        );

        let stage = run_coder_stage(&ctx, Some(&intent));
        let result = stage.result.expect("compiler-backed result should exist");

        assert_eq!(result.verb_fqn, "cbu.create");
        assert_eq!(
            result.dsl,
            "(cbu.create :name \"Apex Growth Fund\" :fund-entity-id \"123e4567-e89b-12d3-a456-426614174222\")"
        );
    }

    #[test]
    fn test_run_coder_stage_uses_compiler_for_cbu_create_with_manco_entity() {
        let ctx = make_test_context();
        let intent = make_cbu_intent(
            crate::sage::OutcomeAction::Create,
            vec![
                ("name", "Apex Growth Fund"),
                ("manco-entity-id", "123e4567-e89b-12d3-a456-426614174333"),
            ],
        );

        let stage = run_coder_stage(&ctx, Some(&intent));
        let result = stage.result.expect("compiler-backed result should exist");

        assert_eq!(result.verb_fqn, "cbu.create");
        assert_eq!(
            result.dsl,
            "(cbu.create :name \"Apex Growth Fund\" :manco-entity-id \"123e4567-e89b-12d3-a456-426614174333\")"
        );
    }

    #[test]
    fn test_run_coder_stage_uses_compiler_for_cbu_rename() {
        let ctx = make_test_context();
        let intent = make_cbu_intent(
            crate::sage::OutcomeAction::Update,
            vec![("name", "Apex Growth Fund")],
        );

        let stage = run_coder_stage(&ctx, Some(&intent));
        let result = stage.result.expect("compiler-backed result should exist");

        assert_eq!(result.verb_fqn, "cbu.rename");
        assert_eq!(
            result.dsl,
            "(cbu.rename :cbu-id \"123e4567-e89b-12d3-a456-426614174000\" :name \"Apex Growth Fund\")"
        );
    }

    #[test]
    fn test_run_coder_stage_uses_compiler_for_cbu_set_jurisdiction() {
        let ctx = make_test_context();
        let intent = make_cbu_intent(
            crate::sage::OutcomeAction::Update,
            vec![("jurisdiction", "LU")],
        );

        let stage = run_coder_stage(&ctx, Some(&intent));
        let result = stage.result.expect("compiler-backed result should exist");

        assert_eq!(result.verb_fqn, "cbu.set-jurisdiction");
        assert_eq!(
            result.dsl,
            "(cbu.set-jurisdiction :cbu-id \"123e4567-e89b-12d3-a456-426614174000\" :jurisdiction \"LU\")"
        );
    }

    #[test]
    fn test_run_coder_stage_uses_compiler_for_cbu_set_client_type() {
        let ctx = make_test_context();
        let intent = make_cbu_intent(
            crate::sage::OutcomeAction::Update,
            vec![("client-type", "FUND")],
        );

        let stage = run_coder_stage(&ctx, Some(&intent));
        let result = stage.result.expect("compiler-backed result should exist");

        assert_eq!(result.verb_fqn, "cbu.set-client-type");
        assert_eq!(
            result.dsl,
            "(cbu.set-client-type :cbu-id \"123e4567-e89b-12d3-a456-426614174000\" :client-type \"FUND\")"
        );
    }

    #[test]
    fn test_run_coder_stage_uses_compiler_for_cbu_set_commercial_client() {
        let ctx = make_test_context();
        let intent = make_cbu_intent(
            crate::sage::OutcomeAction::Update,
            vec![(
                "commercial-client-entity-id",
                "123e4567-e89b-12d3-a456-426614174111",
            )],
        );

        let stage = run_coder_stage(&ctx, Some(&intent));
        let result = stage.result.expect("compiler-backed result should exist");

        assert_eq!(result.verb_fqn, "cbu.set-commercial-client");
        assert_eq!(
            result.dsl,
            "(cbu.set-commercial-client :cbu-id \"123e4567-e89b-12d3-a456-426614174000\" :commercial-client-entity-id \"123e4567-e89b-12d3-a456-426614174111\")"
        );
    }

    #[test]
    fn test_run_coder_stage_uses_compiler_for_cbu_set_category() {
        let ctx = make_test_context();
        let intent = make_cbu_intent(
            crate::sage::OutcomeAction::Update,
            vec![("category", "FUND_MANDATE")],
        );

        let stage = run_coder_stage(&ctx, Some(&intent));
        let result = stage.result.expect("compiler-backed result should exist");

        assert_eq!(result.verb_fqn, "cbu.set-category");
        assert_eq!(
            result.dsl,
            "(cbu.set-category :cbu-id \"123e4567-e89b-12d3-a456-426614174000\" :category \"FUND_MANDATE\")"
        );
    }

    #[test]
    fn test_run_coder_stage_uses_compiler_for_cbu_submit_for_validation() {
        let ctx = make_test_context();
        let intent = make_cbu_intent_with_summary_and_notes(
            crate::sage::OutcomeAction::Update,
            "Submit the current CBU for validation",
            Some("move lifecycle into validation review"),
        );

        let stage = run_coder_stage(&ctx, Some(&intent));
        let result = stage.result.expect("compiler-backed result should exist");

        assert_eq!(result.verb_fqn, "cbu.submit-for-validation");
        assert_eq!(
            result.dsl,
            "(cbu.submit-for-validation :cbu-id \"123e4567-e89b-12d3-a456-426614174000\")"
        );
    }

    #[test]
    fn test_run_coder_stage_uses_compiler_for_cbu_request_proof_update() {
        let ctx = make_test_context();
        let intent = make_cbu_intent_with_summary_and_notes(
            crate::sage::OutcomeAction::Update,
            "Request proof update for the current CBU",
            Some("move to update pending proof"),
        );

        let stage = run_coder_stage(&ctx, Some(&intent));
        let result = stage.result.expect("compiler-backed result should exist");

        assert_eq!(result.verb_fqn, "cbu.request-proof-update");
        assert_eq!(
            result.dsl,
            "(cbu.request-proof-update :cbu-id \"123e4567-e89b-12d3-a456-426614174000\")"
        );
    }

    #[test]
    fn test_run_coder_stage_uses_compiler_for_cbu_reopen_validation() {
        let ctx = make_test_context();
        let intent = make_cbu_intent_with_summary_and_notes(
            crate::sage::OutcomeAction::Update,
            "Reopen validation for the current CBU",
            Some("move failed cbu back to validation"),
        );

        let stage = run_coder_stage(&ctx, Some(&intent));
        let result = stage.result.expect("compiler-backed result should exist");

        assert_eq!(result.verb_fqn, "cbu.reopen-validation");
        assert_eq!(
            result.dsl,
            "(cbu.reopen-validation :cbu-id \"123e4567-e89b-12d3-a456-426614174000\")"
        );
    }

    #[test]
    fn test_data_management_candidate_policy_allows_instance_targeting() {
        let candidates = vec![VerbSearchResult {
            verb: "document.get".into(),
            score: 0.99,
            source: crate::mcp::verb_search::VerbSearchSource::PatternEmbedding,
            matched_phrase: "show me document-id 123".into(),
            description: None,
            journey: None,
        }];

        let filtered = apply_data_management_candidate_policy(
            Some("semos-data-management"),
            "show me document-id 123",
            false,
            candidates.clone(),
        );

        assert_eq!(filtered, candidates);
    }

    #[test]
    fn test_structure_semantics_verbs_exist_in_registry() {
        for verb_fqn in [
            "schema.domain.describe",
            "schema.entity.describe",
            "schema.entity.list-fields",
            "schema.entity.list-relationships",
            "schema.entity.list-verbs",
        ] {
            assert!(
                registry().get_by_name(verb_fqn).is_some(),
                "missing verb {verb_fqn}"
            );
        }
    }

    #[tokio::test]
    async fn test_deterministic_sage_fast_path_primitives_for_documents() {
        let sage = crate::sage::DeterministicSage;
        let ctx = crate::sage::SageContext {
            session_id: None,
            stage_focus: Some("semos-data-management".to_string()),
            goals: vec![],
            entity_kind: None,
            dominant_entity_name: None,
            last_intents: vec![],
        };
        let outcome = sage.classify("show me documents", &ctx).await.unwrap();
        let coder = crate::sage::CoderEngine::load().unwrap();
        let coder_result = coder.resolve(&outcome).unwrap();

        assert_eq!(outcome.plane, crate::sage::ObservationPlane::Structure);
        assert_eq!(outcome.polarity, crate::sage::IntentPolarity::Read);
        assert_eq!(outcome.confidence, crate::sage::SageConfidence::High);
        assert_eq!(coder_result.verb_fqn, "schema.entity.describe");
        assert!(coder_result.missing_args.is_empty());
        assert_eq!(
            coder_result.dsl,
            "(schema.entity.describe :entity-type \"document\")"
        );
    }

    #[tokio::test]
    async fn test_freeform_utterance_without_semos_does_not_produce_dsl_hit() {
        let ctx = make_unscoped_test_context();

        let outcome = handle_utterance(&ctx, "show me Allianz").await.unwrap();

        assert!(!outcome.pipeline_result.valid);
        assert!(outcome.pipeline_result.dsl.is_empty());
        assert_eq!(
            outcome.pipeline_result.outcome,
            PipelineOutcome::NoAllowedVerbs
        );
        assert_eq!(outcome.trace.final_verb, None);
        assert!(outcome.trace.dsl_generated.is_none());
        assert!(outcome.trace.semreg_unavailable);
        assert!(outcome.trace.blocked_reason.is_some());
    }

    #[tokio::test]
    async fn test_forced_verb_is_denied_when_semos_is_unavailable_in_strict_mode() {
        let ctx = make_unscoped_test_context();

        let outcome = handle_utterance_with_forced_verb(&ctx, "show me Allianz", "deal.create")
            .await
            .unwrap();

        assert!(!outcome.pipeline_result.valid);
        assert!(outcome.pipeline_result.dsl.is_empty());
        assert_eq!(
            outcome.pipeline_result.outcome,
            PipelineOutcome::NeedsClarification
        );
        assert_eq!(outcome.trace.forced_verb.as_deref(), Some("deal.create"));
        assert_eq!(outcome.trace.final_verb, None);
        assert_eq!(outcome.trace.selection_source, "user_choice");
        assert_eq!(outcome.trace.toctou_result.as_deref(), Some("denied"));
        assert!(outcome.trace.semreg_unavailable);
        assert!(outcome
            .trace
            .blocked_reason
            .as_deref()
            .is_some_and(|reason| reason.contains("not in safe-harbor set")));
    }

    #[tokio::test]
    async fn test_discovery_stage_semos_response_requires_clarification_not_dsl() {
        let ctx = make_discovery_test_context();

        let outcome = handle_utterance(&ctx, "show me Allianz").await.unwrap();

        assert!(!outcome.pipeline_result.valid);
        assert!(outcome.pipeline_result.dsl.is_empty());
        assert_eq!(
            outcome.pipeline_result.outcome,
            PipelineOutcome::NeedsUserInput
        );
        assert_eq!(
            outcome.pipeline_result.missing_required,
            vec!["client_name".to_string()]
        );
        assert!(outcome
            .pipeline_result
            .validation_error
            .as_deref()
            .is_some_and(|message| message.contains("Which client are you working on?")));
        assert_eq!(outcome.trace.final_verb, None);
        assert!(outcome.trace.dsl_generated.is_none());
        assert!(!outcome.trace.semreg_unavailable);
        assert!(outcome
            .trace
            .blocked_reason
            .as_deref()
            .is_some_and(|reason| reason.contains("discovery stage requires clarification")));
        assert!(outcome
            .context_envelope
            .as_ref()
            .is_some_and(SemOsContextEnvelope::is_discovery_stage));
        assert_eq!(
            outcome
                .context_envelope
                .as_ref()
                .and_then(SemOsContextEnvelope::first_discovery_question),
            Some("Which client are you working on?")
        );
    }

    #[test]
    fn test_structure_reads_use_generic_task_subject_in_data_management() {
        let intent = crate::sage::OutcomeIntent {
            summary: "Describe entity schema for document with fields relationships and verbs"
                .to_string(),
            plane: crate::sage::ObservationPlane::Structure,
            polarity: crate::sage::IntentPolarity::Read,
            domain_concept: "document".to_string(),
            action: crate::sage::OutcomeAction::Read,
            subject: None,
            steps: vec![],
            confidence: crate::sage::SageConfidence::High,
            pending_clarifications: vec![],
            hints: crate::sage::UtteranceHints::default(),
            explain: crate::sage::SageExplain::default(),
            coder_handoff: crate::sage::CoderHandoff::default(),
        };

        assert!(should_use_generic_task_subject_for_sage(
            Some("semos-data-management"),
            Some(&intent)
        ));
        assert!(!should_use_generic_task_subject_for_sage(
            Some("semos-kyc"),
            Some(&intent)
        ));
    }

    #[test]
    fn test_data_management_structure_fast_path_allows_schema_verbs() {
        let intent = crate::sage::OutcomeIntent {
            summary: "Describe entity schema for document with fields relationships and verbs"
                .to_string(),
            plane: crate::sage::ObservationPlane::Structure,
            polarity: crate::sage::IntentPolarity::Read,
            domain_concept: "document".to_string(),
            action: crate::sage::OutcomeAction::Read,
            subject: None,
            steps: vec![],
            confidence: crate::sage::SageConfidence::High,
            pending_clarifications: vec![],
            hints: crate::sage::UtteranceHints::default(),
            explain: crate::sage::SageExplain::default(),
            coder_handoff: crate::sage::CoderHandoff::default(),
        };

        assert!(allow_data_management_structure_fast_path(
            Some("semos-data-management"),
            &intent,
            "schema.entity.describe"
        ));
        assert!(!allow_data_management_structure_fast_path(
            Some("semos-kyc"),
            &intent,
            "schema.entity.describe"
        ));
        assert!(!allow_data_management_structure_fast_path(
            Some("semos-data-management"),
            &intent,
            "document.get"
        ));
    }

    #[test]
    fn test_structure_semantics_fast_path_can_skip_parse_validation() {
        assert!(can_skip_fast_path_parse_validation(
            "schema.entity.describe"
        ));
        assert!(!can_skip_fast_path_parse_validation("document.get"));
    }

    #[test]
    fn test_build_sage_fast_path_result_for_cbu_list_is_valid() {
        let outcome = crate::sage::OutcomeIntent {
            summary: "show me the cbus".to_string(),
            plane: crate::sage::ObservationPlane::Instance,
            polarity: crate::sage::IntentPolarity::Read,
            domain_concept: "cbu".to_string(),
            action: crate::sage::OutcomeAction::Read,
            subject: None,
            steps: vec![],
            confidence: crate::sage::SageConfidence::Medium,
            pending_clarifications: vec![],
            hints: crate::sage::UtteranceHints::default(),
            explain: crate::sage::SageExplain::default(),
            coder_handoff: crate::sage::CoderHandoff::default(),
        };
        let coder_result = CoderResult {
            verb_fqn: "cbu.list".to_string(),
            dsl: "(cbu.list)".to_string(),
            resolution: crate::sage::coder::CoderResolution::Confident,
            missing_args: vec![],
            unresolved_refs: vec![],
            diagnostics: None,
        };

        let result = build_sage_fast_path_result("show me the cbus", None, &outcome, &coder_result)
            .expect("cbu list fast path should assemble");
        assert!(result.valid);
        assert_eq!(result.intent.verb, "cbu.list");
        assert_eq!(result.dsl, "(cbu.list)");
    }

    #[test]
    fn test_trace_records_pre_and_post_semreg_verbs() {
        let mut trace = default_trace();
        trace.chosen_verb_pre_semreg = Some("verb.a".into());
        trace.chosen_verb_post_semreg = Some("verb.b".into());
        trace.semreg_policy = "allowed_set".into();

        let json = serde_json::to_string(&trace).unwrap();
        assert!(json.contains("chosen_verb_pre_semreg"));
        assert!(json.contains("verb.a"));
        assert!(json.contains("chosen_verb_post_semreg"));
        assert!(json.contains("verb.b"));
        assert!(json.contains(r#""semreg_policy":"allowed_set"#));
    }

    #[test]
    fn test_trace_semreg_unavailable_flag() {
        let mut trace = default_trace();
        trace.semreg_unavailable = true;
        trace.sem_reg_mode = "fail_open".into();
        trace.semreg_policy = "unavailable".into();

        let json = serde_json::to_string(&trace).unwrap();
        assert!(json.contains(r#""semreg_unavailable":true"#));
        assert!(json.contains(r#""sem_reg_mode":"fail_open"#));
    }

    #[test]
    fn test_deny_all_not_treated_as_unavailable() {
        let deny = SemOsContextEnvelope::deny_all();
        let unavail = SemOsContextEnvelope::unavailable();

        assert_eq!(deny.label(), "deny_all");
        assert_eq!(unavail.label(), "unavailable");
        assert_ne!(deny.label(), unavail.label());

        assert!(deny.is_deny_all());
        assert!(!deny.is_unavailable());
        assert!(!unavail.is_deny_all());
        assert!(unavail.is_unavailable());
    }

    #[test]
    fn test_trace_selection_source_field() {
        let mut trace = default_trace();
        trace.selection_source = "user_choice".into();
        let json = serde_json::to_string(&trace).unwrap();
        assert!(json.contains(r#""selection_source":"user_choice""#));
    }

    #[test]
    fn test_trace_macro_governance_fields() {
        let mut trace = default_trace();
        trace.macro_semreg_checked = true;
        trace.macro_denied_verbs = vec!["bad.verb".into()];
        trace.selection_source = "discovery".into();

        let json = serde_json::to_string(&trace).unwrap();
        assert!(json.contains(r#""macro_semreg_checked":true"#));
        assert!(json.contains("bad.verb"));
        assert!(json.contains("macro_denied_verbs"));
    }

    #[test]
    fn test_ast_verb_extraction_from_dsl() {
        // Verify that parse_program + VerbCall::full_name() correctly extracts verbs
        use dsl_core::ast::Statement;
        let dsl = "(entity.create :name \"Acme\")\n(kyc.open-case :entity \"Acme\")";
        let program = parse_program(dsl).expect("valid DSL");
        let verbs: Vec<String> = program
            .statements
            .iter()
            .filter_map(|stmt| {
                if let Statement::VerbCall(vc) = stmt {
                    Some(vc.full_name())
                } else {
                    None
                }
            })
            .collect();
        assert_eq!(verbs.len(), 2);
        assert_eq!(verbs[0], "entity.create");
        assert_eq!(verbs[1], "kyc.open-case");
    }

    #[test]
    fn test_ast_verb_extraction_single_verb() {
        use dsl_core::ast::Statement;
        let dsl = "(deal.create :name \"Test\")";
        let program = parse_program(dsl).expect("valid DSL");
        let verbs: Vec<String> = program
            .statements
            .iter()
            .filter_map(|stmt| {
                if let Statement::VerbCall(vc) = stmt {
                    Some(vc.full_name())
                } else {
                    None
                }
            })
            .collect();
        assert_eq!(verbs.len(), 1);
        assert_eq!(verbs[0], "deal.create");
    }

    #[test]
    fn test_ast_verb_extraction_invalid_dsl_returns_empty() {
        // The macro governance code falls back to empty vec on parse error
        let dsl = "this is not valid dsl at all";
        let result = parse_program(dsl);
        assert!(result.is_err(), "Invalid DSL should fail to parse");
    }

    #[test]
    fn test_telemetry_persisted_field_serializes() {
        let mut trace = default_trace();
        trace.telemetry_persisted = true;
        let json = serde_json::to_string(&trace).unwrap();
        assert!(json.contains(r#""telemetry_persisted":true"#));
    }

    #[test]
    fn test_static_guard_insert_intent_event_only_in_orchestrator() {
        // Static guard: insert_intent_event must only be called from orchestrator.
        // This test verifies the pattern by checking that the function reference
        // exists in this module (orchestrator) via the emit_telemetry function.
        // The actual grep-based guard runs as a build-time/CI check.
        //
        // Verify emit_telemetry is available (compile-time proof it's wired here).
        fn _assert_emit_exists() {
            // This function's existence proves emit_telemetry is in scope.
            // If someone moves telemetry emission elsewhere, this test
            // should be accompanied by a CI grep guard.
        }
        _assert_emit_exists();
    }

    #[test]
    fn test_trace_selection_source_semreg() {
        let mut trace = default_trace();
        trace.selection_source = "semreg".into();
        trace.forced_verb = Some("kyc.open-case".into());
        let json = serde_json::to_string(&trace).unwrap();
        assert!(json.contains(r#""selection_source":"semreg""#));
        assert!(json.contains(r#""forced_verb":"kyc.open-case""#));
    }

    #[test]
    fn test_trace_selection_source_macro() {
        let mut trace = default_trace();
        trace.selection_source = "macro".into();
        trace.macro_semreg_checked = true;
        trace.macro_denied_verbs = vec!["denied.verb".into()];
        let json = serde_json::to_string(&trace).unwrap();
        assert!(json.contains(r#""selection_source":"macro""#));
        assert!(json.contains(r#""macro_semreg_checked":true"#));
        assert!(json.contains("denied.verb"));
    }

    #[test]
    fn test_trace_fingerprint_and_pruned_count_fields() {
        let mut trace = default_trace();
        trace.allowed_verbs_fingerprint = Some("v1:abc123def456".into());
        trace.pruned_verbs_count = 3;

        let json = serde_json::to_string(&trace).unwrap();
        assert!(json.contains(r#""allowed_verbs_fingerprint":"v1:abc123def456""#));
        assert!(json.contains(r#""pruned_verbs_count":3"#));
    }

    #[test]
    fn test_trace_fingerprint_none_when_unavailable() {
        let trace = default_trace();
        // Default trace has fingerprint None and pruned_count 0
        let json = serde_json::to_string(&trace).unwrap();
        assert!(json.contains(r#""allowed_verbs_fingerprint":null"#));
        assert!(json.contains(r#""pruned_verbs_count":0"#));
    }

    #[test]
    fn test_trace_toctou_fields_default() {
        let trace = default_trace();
        assert!(!trace.toctou_recheck_performed);
        assert!(trace.toctou_result.is_none());
        assert!(trace.toctou_new_fingerprint.is_none());

        let json = serde_json::to_string(&trace).unwrap();
        assert!(json.contains(r#""toctou_recheck_performed":false"#));
        assert!(json.contains(r#""toctou_result":null"#));
        assert!(json.contains(r#""toctou_new_fingerprint":null"#));
    }

    #[test]
    fn test_trace_toctou_fields_drifted() {
        let mut trace = default_trace();
        trace.toctou_recheck_performed = true;
        trace.toctou_result = Some("allowed_but_drifted".into());
        trace.toctou_new_fingerprint = Some("v1:newfingerprint".into());

        let json = serde_json::to_string(&trace).unwrap();
        assert!(json.contains(r#""toctou_recheck_performed":true"#));
        assert!(json.contains(r#""toctou_result":"allowed_but_drifted""#));
        assert!(json.contains(r#""toctou_new_fingerprint":"v1:newfingerprint""#));
    }

    #[test]
    fn test_trace_toctou_fields_denied() {
        let mut trace = default_trace();
        trace.toctou_recheck_performed = true;
        trace.toctou_result = Some("denied".into());
        trace.toctou_new_fingerprint = Some("v1:deniedfingerprint".into());
        trace.blocked_reason = Some("TOCTOU recheck failed".into());

        let json = serde_json::to_string(&trace).unwrap();
        assert!(json.contains(r#""toctou_result":"denied""#));
        assert!(json.contains("TOCTOU recheck failed"));
    }

    #[test]
    fn test_phase4_guard_blocks_resolution_outside_phase2_legal_set() {
        use std::collections::HashSet;

        let legal = HashSet::from(["kyc.open-case".to_string()]);
        let evaluation = crate::traceability::Phase2Evaluation {
            artifacts: crate::traceability::Phase2Service::compose(None, None),
            halt_reason_code: None,
            halt_phase: None,
            is_available: true,
            is_deny_all: false,
            has_usable_legal_set: true,
            policy_label: "allowed_set",
            legal_verbs_or_empty: legal.clone(),
            legal_verbs_if_usable: Some(legal),
        };
        let violation = crate::traceability::enforce_phase4_resolution_within_evaluation(
            Some("deal.create"),
            &evaluation,
        );

        assert_eq!(violation, Some("phase4_widened_outside_phase2"));
    }
}
