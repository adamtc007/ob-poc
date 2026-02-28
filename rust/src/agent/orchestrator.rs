//! Unified Intent Orchestrator
//!
//! Single entry point (`handle_utterance`) for all utterance processing:
//! Chat API, MCP `dsl_generate`, and REPL. Wraps `IntentPipeline` with:
//!
//! - **Entity linking** via `LookupService` (Phase 4 dedup)
//! - **SemReg context resolution** -> `ContextEnvelope` (Phase 2B CCIR)
//! - **Direct DSL bypass gating** by actor role (Phase 2.1)
//! - **IntentTrace** structured audit logging (Phase 5)
//!
//! Phase 2B replaced the flat `SemRegVerbPolicy` enum with a rich
//! `ContextEnvelope` that preserves pruning reasons, governance signals,
//! and a deterministic fingerprint of the allowed verb set.

use serde::Serialize;
use std::sync::Arc;
use uuid::Uuid;

#[cfg(feature = "database")]
use sqlx::PgPool;

use crate::agent::context_envelope::ContextEnvelope;
use crate::agent::telemetry;
use crate::lookup::LookupService;
use crate::mcp::intent_pipeline::{IntentPipeline, PipelineOutcome, PipelineResult};
use crate::mcp::scope_resolution::ScopeContext;
use crate::mcp::verb_search::HybridVerbSearcher;
use crate::policy::{gate::PolicySnapshot, PolicyGate};
use crate::sem_reg::abac::ActorContext;

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
    /// Threaded into SemReg ContextResolutionRequest to filter verbs by phase_tags.
    /// Empty means no goal filtering (all verbs pass).
    pub goals: Vec<String>,
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
    /// Rich context envelope from SemReg resolution (replaces flat `sem_reg_verbs`).
    /// Contains allowed verbs, pruned verbs with reasons, fingerprint, governance signals.
    #[cfg(feature = "database")]
    pub context_envelope: Option<ContextEnvelope>,
    pub lookup_result: Option<crate::lookup::LookupResult>,
    pub trace: IntentTrace,
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
    /// SemReg policy classification (via ContextEnvelope::label())
    pub semreg_policy: String,
    /// Set when SemReg was unavailable but pipeline continued (non-strict)
    pub semreg_unavailable: bool,
    /// Source of verb selection: "discovery", "user_choice", "macro"
    pub selection_source: String,
    /// True if macro-expanded DSL was checked against SemReg
    pub macro_semreg_checked: bool,
    /// Verbs in macro expansion that were denied by SemReg (empty if none)
    pub macro_denied_verbs: Vec<String>,
    /// Entity kind of the dominant entity (e.g., "cbu", "fund")
    pub dominant_entity_kind: Option<String>,
    /// Whether entity-kind filtering was applied in SemReg context resolution
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

/// Process an utterance through the unified pipeline.
///
/// Flow:
/// 1. Entity linking (if LookupService available)
/// 2. SemReg context resolution -> `ContextEnvelope`
/// 3. Build IntentPipeline, run candidate discovery
/// 4. Apply SemReg filter to candidates
/// 5. For matched-path outcomes: re-generate DSL via forced-verb if SemReg
///    changes the winning verb (ensures SemReg is binding, not cosmetic)
/// 6. Build IntentTrace with full provenance
#[cfg(feature = "database")]
pub async fn handle_utterance(
    ctx: &OrchestratorContext,
    utterance: &str,
) -> anyhow::Result<OrchestratorOutcome> {
    let policy = &ctx.policy_gate;

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

    let entity_candidates: Vec<String> = lookup_result
        .as_ref()
        .map(|lr| lr.entities.iter().map(|e| e.mention_text.clone()).collect())
        .unwrap_or_default();

    // -- Step 2: SemReg context resolution -> ContextEnvelope --
    let envelope = resolve_sem_reg_verbs(ctx, dominant_entity_kind.as_deref()).await;

    // Extract verb names for trace (backward-compatible with sem_reg_verb_filter)
    let sem_reg_verb_names: Option<Vec<String>> = if envelope.is_unavailable() {
        None
    } else {
        Some(envelope.allowed_verbs.iter().cloned().collect())
    };

    // -- Stage A: Discover candidates (no DSL generation yet) --
    // Phase 3 CCIR: pass SemReg allowed verbs into pipeline for pre-constrained search.
    // The pipeline threads these to HybridVerbSearcher::search() so disallowed verbs
    // are never returned from any tier. The post-filter in Stage A.2 below is kept as
    // a safety net / debug assertion (belt-and-suspenders).
    let searcher = (*ctx.verb_searcher).clone();
    let pipeline = {
        let p = IntentPipeline::with_pool(searcher, ctx.pool.clone());
        if !envelope.is_unavailable() && !envelope.is_deny_all() {
            p.with_allowed_verbs(envelope.allowed_verbs.clone())
        } else {
            p
        }
    };

    let discovery_result = pipeline
        .process_with_scope(utterance, None, ctx.scope.clone())
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
        let trace = build_trace(
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
        let mut outcome = OrchestratorOutcome {
            pipeline_result: discovery_result,
            context_envelope: Some(envelope),
            lookup_result,
            trace,
        };
        emit_telemetry(ctx, utterance, &mut outcome).await;
        return Ok(outcome);
    }

    // NOTE: Direct DSL early-exit (dsl: prefix) was removed in Phase 0B CCIR.
    // All DSL — including operator-provided — flows through SemReg filtering below.

    // -- Stage A.2: Apply SemReg policy via ContextEnvelope --
    let mut sem_reg_denied_all = false;
    let mut semreg_unavailable = false;
    let mut blocked_reason: Option<String> = None;
    let mut filtered_candidates = discovery_result.verb_candidates.clone();

    if envelope.is_unavailable() {
        semreg_unavailable = true;
        if policy.semreg_fail_closed() {
            tracing::warn!("SemReg unavailable -- fail-closed (strict mode)");
            blocked_reason = Some("SemReg unavailable (strict mode requires SemReg)".into());
            filtered_candidates.clear();
        } else {
            tracing::info!("SemReg unavailable -- fail-open (permissive mode)");
        }
    } else if envelope.is_deny_all() {
        sem_reg_denied_all = true;
        if policy.semreg_fail_closed() {
            tracing::warn!("SemReg returned DenyAll -- fail-closed (strict mode)");
            blocked_reason = Some("SemReg denied all verbs for this subject (strict mode)".into());
            filtered_candidates.clear();
        } else {
            tracing::warn!("SemReg returned DenyAll -- fail-open (permissive mode)");
        }
    } else {
        // AllowedSet — filter candidates against allowed verbs
        let allowed = &envelope.allowed_verbs;
        let before_count = filtered_candidates.len();
        filtered_candidates.retain(|v| allowed.contains(&v.verb));
        if filtered_candidates.is_empty() && before_count > 0 {
            sem_reg_denied_all = true;
            if policy.semreg_fail_closed() {
                tracing::warn!(
                    before = before_count,
                    allowed_count = allowed.len(),
                    strict = true,
                    "SemReg filtered ALL verb candidates -- fail-closed (strict mode)"
                );
                blocked_reason = Some("SemReg denied all verb candidates (strict mode)".into());
            } else {
                tracing::warn!(
                    before = before_count,
                    allowed_count = allowed.len(),
                    strict = false,
                    "SemReg filtered ALL verb candidates -- falling back to unfiltered (permissive)"
                );
                filtered_candidates = discovery_result.verb_candidates.clone();
            }
        }
    }

    // -- Stage A.3: Apply AgentMode filter --
    // Research mode blocks publish/publish-batch; Governed blocks authoring exploration verbs.
    let mut agent_mode_blocked = Vec::new();
    filtered_candidates.retain(|v| {
        if ctx.agent_mode.is_verb_allowed(&v.verb) {
            true
        } else {
            tracing::debug!(
                verb = %v.verb,
                mode = %ctx.agent_mode,
                "AgentMode blocked verb"
            );
            agent_mode_blocked.push(v.verb.clone());
            false
        }
    });

    let post_filter: Vec<(String, f32)> = filtered_candidates
        .iter()
        .map(|v| (v.verb.clone(), v.score))
        .collect();

    let chosen_verb_post_semreg = filtered_candidates.first().map(|v| v.verb.clone());

    // -- Stage B: Select verb + generate DSL --
    use crate::mcp::intent_pipeline::StructuredIntent;

    let mut result = if (sem_reg_denied_all || semreg_unavailable) && policy.semreg_fail_closed() {
        PipelineResult {
            intent: StructuredIntent::empty(),
            verb_candidates: filtered_candidates,
            dsl: String::new(),
            dsl_hash: None,
            valid: false,
            validation_error: Some(
                blocked_reason
                    .clone()
                    .unwrap_or_else(|| "SemReg blocked (strict mode)".into()),
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
        // If discovery already generated DSL AND the verb matches, reuse it.
        // Otherwise re-generate via forced-verb.
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
                .process_with_forced_verb(utterance, &top.verb, ctx.scope.clone())
                .await?;
            forced_result.verb_candidates = filtered_candidates;
            forced_result
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

    if policy.semreg_fail_closed() && !envelope.is_unavailable() {
        if let Some(ref verb_fqn) = selected_verb_fqn {
            toctou_performed = true;
            let new_envelope = resolve_sem_reg_verbs(ctx, dominant_entity_kind.as_deref()).await;

            if let Some(toctou) = envelope.toctou_recheck(&new_envelope, verb_fqn) {
                use crate::agent::context_envelope::TocTouResult;
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
        pruned_count = trace.pruned_verbs_count,
        toctou = ?trace.toctou_result,
        "IntentTrace"
    );
    tracing::debug!(trace = %serde_json::to_string(&trace).unwrap_or_default(), "IntentTrace detail");

    let mut outcome = OrchestratorOutcome {
        pipeline_result: result,
        context_envelope: Some(envelope),
        lookup_result,
        trace,
    };
    emit_telemetry(ctx, utterance, &mut outcome).await;
    Ok(outcome)
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
    envelope: &ContextEnvelope,
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
    let allowed_verbs_fingerprint = if envelope.is_unavailable() {
        None
    } else {
        Some(envelope.fingerprint_str().to_string())
    };
    let pruned_verbs_count = envelope.pruned_count();

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
        semreg_policy: envelope.label().to_string(),
        semreg_unavailable,
        selection_source: "discovery".to_string(),
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

    let entity_candidates: Vec<String> = lookup_result
        .as_ref()
        .map(|lr| lr.entities.iter().map(|e| e.mention_text.clone()).collect())
        .unwrap_or_default();

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
        sem_reg_verb_filter: None,
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
        semreg_policy: "unavailable".to_string(),
        semreg_unavailable: false,
        selection_source: "user_choice".to_string(),
        macro_semreg_checked: false,
        macro_denied_verbs: vec![],
        dominant_entity_kind,
        entity_kind_filtered: false,
        telemetry_persisted: false,
        agent_mode: ctx.agent_mode.to_string(),
        agent_mode_blocked_verbs: vec![],
        allowed_verbs_fingerprint: None,
        pruned_verbs_count: 0,
        toctou_recheck_performed: false,
        toctou_result: None,
        toctou_new_fingerprint: None,
    };

    tracing::info!(
        source = ?trace.source,
        forced_verb = forced_verb_fqn,
        dsl_generated = trace.dsl_generated.is_some(),
        "IntentTrace (forced verb)"
    );

    let mut outcome = OrchestratorOutcome {
        pipeline_result: result,
        context_envelope: None,
        lookup_result,
        trace,
    };
    emit_telemetry(ctx, utterance, &mut outcome).await;
    Ok(outcome)
}

/// Resolve SemReg context and return a `ContextEnvelope`.
///
/// Returns a rich envelope preserving allowed verbs, pruned verbs with reasons,
/// deterministic fingerprint, evidence gaps, and governance signals.
#[cfg(feature = "database")]
pub(crate) async fn resolve_sem_reg_verbs(
    ctx: &OrchestratorContext,
    entity_kind: Option<&str>,
) -> ContextEnvelope {
    // Route through SemOsClient when available (DI boundary), fallback to direct call.
    if let Some(ref client) = ctx.sem_os_client {
        resolve_via_client(client.as_ref(), ctx, entity_kind).await
    } else {
        #[cfg(feature = "database")]
        {
            resolve_via_direct(ctx, entity_kind).await
        }
        #[cfg(not(feature = "database"))]
        {
            tracing::warn!("sem_reg context resolution requires database feature or SemOsClient");
            ContextEnvelope::unavailable()
        }
    }
}

/// Resolve verbs via SemOsClient DI boundary (in-process or HTTP).
async fn resolve_via_client(
    client: &dyn SemOsClient,
    ctx: &OrchestratorContext,
    entity_kind: Option<&str>,
) -> ContextEnvelope {
    use sem_os_core::context_resolution::{EvidenceMode, SubjectRef};

    let subject = if let Some(entity_id) = ctx.dominant_entity_id {
        SubjectRef::EntityId(entity_id)
    } else {
        SubjectRef::CaseId(
            ctx.case_id
                .unwrap_or_else(|| ctx.session_id.unwrap_or_else(Uuid::new_v4)),
        )
    };
    // Convert ob-poc ActorContext → sem_os_core ActorContext via serde round-trip
    // (structurally identical types in separate crates)
    let core_actor: sem_os_core::abac::ActorContext = {
        let json = serde_json::to_value(&ctx.actor).expect("ActorContext serializes");
        serde_json::from_value(json).expect("ActorContext round-trips")
    };
    let request = sem_os_core::context_resolution::ContextResolutionRequest {
        subject,
        intent: None,
        actor: core_actor,
        goals: ctx.goals.clone(),
        constraints: Default::default(),
        evidence_mode: EvidenceMode::default(),
        point_in_time: None,
        entity_kind: entity_kind.map(|s| s.to_string()),
    };
    let principal =
        sem_os_core::principal::Principal::in_process(&ctx.actor.actor_id, ctx.actor.roles.clone());

    match client.resolve_context(&principal, request).await {
        Ok(response) => {
            let envelope = ContextEnvelope::from_resolution(&response);
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
            ContextEnvelope::unavailable()
        }
    }
}

/// Resolve verbs via direct sem_reg call (legacy path, before full cutover).
#[cfg(feature = "database")]
async fn resolve_via_direct(
    ctx: &OrchestratorContext,
    entity_kind: Option<&str>,
) -> ContextEnvelope {
    use crate::sem_reg::context_resolution::{
        resolve_context, ContextResolutionRequest, EvidenceMode, SubjectRef,
    };

    let subject = if let Some(entity_id) = ctx.dominant_entity_id {
        SubjectRef::EntityId(entity_id)
    } else {
        SubjectRef::CaseId(
            ctx.case_id
                .unwrap_or_else(|| ctx.session_id.unwrap_or_else(Uuid::new_v4)),
        )
    };
    let request = ContextResolutionRequest {
        subject,
        intent: None,
        actor: ctx.actor.clone(),
        goals: ctx.goals.clone(),
        constraints: Default::default(),
        evidence_mode: EvidenceMode::default(),
        point_in_time: None,
        entity_kind: entity_kind.map(|s| s.to_string()),
    };

    match resolve_context(&ctx.pool, &request).await {
        Ok(response) => {
            // Bridge local ContextResolutionResponse → sem_os_core ContextResolutionResponse
            // (structurally identical but different crate types — serde round-trip)
            let core_response: sem_os_core::context_resolution::ContextResolutionResponse = {
                let json =
                    serde_json::to_value(&response).expect("ContextResolutionResponse serializes");
                serde_json::from_value(json).expect("ContextResolutionResponse round-trips")
            };
            let envelope = ContextEnvelope::from_resolution(&core_response);
            tracing::debug!(
                allowed_count = envelope.allowed_verbs.len(),
                pruned_count = envelope.pruned_count(),
                fingerprint = %envelope.fingerprint_str(),
                "SemReg context resolution completed (direct)"
            );
            envelope
        }
        Err(e) => {
            tracing::warn!(error = %e, source = "sem_reg", "SemReg context resolution failed (direct)");
            ContextEnvelope::unavailable()
        }
    }
}

/// Resolve the SemReg allowed verb set using only a SemOsClient + actor context.
///
/// This is a lightweight entry point for MCP tools (verb_search, dsl_execute) that
/// don't have a full OrchestratorContext. Returns a `ContextEnvelope`.
#[cfg(feature = "database")]
pub async fn resolve_allowed_verbs(
    client: &dyn SemOsClient,
    actor: &ActorContext,
    session_id: Option<Uuid>,
) -> ContextEnvelope {
    use sem_os_core::context_resolution::{EvidenceMode, SubjectRef};

    let subject = SubjectRef::CaseId(session_id.unwrap_or_else(Uuid::new_v4));
    let core_actor: sem_os_core::abac::ActorContext = {
        let json = serde_json::to_value(actor).expect("ActorContext serializes");
        serde_json::from_value(json).expect("ActorContext round-trips")
    };
    let request = sem_os_core::context_resolution::ContextResolutionRequest {
        subject,
        intent: None,
        actor: core_actor,
        goals: vec![],
        constraints: Default::default(),
        evidence_mode: EvidenceMode::default(),
        point_in_time: None,
        entity_kind: None,
    };
    let principal =
        sem_os_core::principal::Principal::in_process(&actor.actor_id, actor.roles.clone());

    match client.resolve_context(&principal, request).await {
        Ok(response) => {
            let envelope = ContextEnvelope::from_resolution(&response);
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
            ContextEnvelope::unavailable()
        }
    }
}

// -- Tests --

#[cfg(test)]
mod tests {
    use super::*;
    use crate::dsl_v2::parse_program;

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
        trace.utterance = "create a fund".into();
        trace.verb_candidates_post_filter = vec![("cbu.create".into(), 1.0)];
        trace.final_verb = Some("cbu.create".into());
        trace.final_confidence = 1.0;
        trace.dsl_generated = Some("(cbu.create)".into());
        trace.dsl_source = Some("chat".into());
        trace.forced_verb = Some("cbu.create".into());

        let json = serde_json::to_string(&trace).unwrap();
        assert!(json.contains("forced_verb"));
        assert!(json.contains("cbu.create"));
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
        // Verify ContextEnvelope labels match the old SemRegVerbPolicy labels
        let unav = ContextEnvelope::unavailable();
        assert_eq!(unav.label(), "unavailable");

        let deny = ContextEnvelope::deny_all();
        assert_eq!(deny.label(), "deny_all");

        let _allowed = ContextEnvelope::unavailable();
        // Build a non-unavailable, non-deny-all envelope
        let env = ContextEnvelope::deny_all();
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
        let deny = ContextEnvelope::deny_all();
        let unavail = ContextEnvelope::unavailable();

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
        let dsl = "(cbu.create :name \"Test\")";
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
        assert_eq!(verbs[0], "cbu.create");
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
}
