//! Unified Intent Orchestrator
//!
//! Single entry point (`handle_utterance`) for all utterance processing:
//! Chat API, MCP `dsl_generate`, and REPL. Wraps `IntentPipeline` with:
//!
//! - **Entity linking** via `LookupService` (Phase 4 dedup)
//! - **SemReg context resolution** → verb filtering (Phase 3)
//! - **Direct DSL bypass gating** by actor role (Phase 2.1)
//! - **IntentTrace** structured audit logging (Phase 5)

use serde::Serialize;
use std::collections::HashSet;
use std::sync::Arc;
use uuid::Uuid;

#[cfg(feature = "database")]
use sqlx::PgPool;

use crate::lookup::LookupService;
use crate::policy::{PolicyGate, gate::PolicySnapshot};
use crate::mcp::intent_pipeline::{IntentPipeline, PipelineResult};
use crate::mcp::scope_resolution::ScopeContext;
use crate::mcp::verb_search::HybridVerbSearcher;
use crate::sem_reg::abac::ActorContext;

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
    #[cfg(feature = "database")]
    pub sem_reg_verbs: Option<Vec<String>>,
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
}

/// Process an utterance through the unified pipeline.
///
/// Flow:
/// 1. Entity linking (if LookupService available)
/// 2. SemReg context resolution → allowed verb set (if database feature)
/// 3. Build IntentPipeline with bypass gating
/// 4. Run pipeline
/// 5. Post-filter verb candidates by SemReg allowed set
/// 6. Build IntentTrace
#[cfg(feature = "database")]
pub async fn handle_utterance(
    ctx: &OrchestratorContext,
    utterance: &str,
) -> anyhow::Result<OrchestratorOutcome> {
    let policy = &ctx.policy_gate;

    // ── Step 1: Entity linking ──────────────────────────────────
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

    let entity_candidates: Vec<String> = lookup_result
        .as_ref()
        .map(|lr| lr.entities.iter().map(|e| e.mention_text.clone()).collect())
        .unwrap_or_default();

    // ── Step 2: SemReg context resolution ───────────────────────
    let (allowed_verbs, sem_reg_verb_names) = resolve_sem_reg_verbs(ctx).await;

    // ── Stage A: Discover candidates (no DSL generation yet) ───
    let searcher = (*ctx.verb_searcher).clone();
    let pipeline = IntentPipeline::with_pool(searcher, ctx.pool.clone())
        .set_allow_direct_dsl(policy.can_use_direct_dsl(&ctx.actor));

    // Run pipeline for candidate discovery
    let discovery_result = pipeline
        .process_with_scope(utterance, None, ctx.scope.clone())
        .await?;

    // Capture pre-SemReg candidates
    let pre_filter: Vec<(String, f32)> = discovery_result
        .verb_candidates
        .iter()
        .map(|v| (v.verb.clone(), v.score))
        .collect();

    // ── Stage A.2: Apply SemReg filter to candidates ────────────
    let mut sem_reg_denied_all = false;
    let mut blocked_reason: Option<String> = None;
    let mut filtered_candidates = discovery_result.verb_candidates.clone();

    if let Some(ref allowed) = allowed_verbs {
        let before_count = filtered_candidates.len();
        filtered_candidates.retain(|v| allowed.contains(&v.verb));
        if filtered_candidates.is_empty() && before_count > 0 {
            sem_reg_denied_all = true;
            if policy.semreg_fail_closed() {
                tracing::warn!(
                    before = before_count,
                    allowed_count = allowed.len(),
                    strict = true,
                    "SemReg filtered ALL verb candidates — fail-closed (strict mode)"
                );
                blocked_reason = Some("SemReg denied all verb candidates (strict mode)".into());
            } else {
                tracing::warn!(
                    before = before_count,
                    allowed_count = allowed.len(),
                    strict = false,
                    "SemReg filtered ALL verb candidates — falling back to unfiltered (permissive)"
                );
                // Restore original candidates in permissive mode
                filtered_candidates = discovery_result.verb_candidates.clone();
            }
        }
    }

    let post_filter: Vec<(String, f32)> = filtered_candidates
        .iter()
        .map(|v| (v.verb.clone(), v.score))
        .collect();

    // ── Stage B: Select verb + generate DSL ─────────────────────
    // If SemReg denied all in strict mode → return NoAllowedVerbs
    // If discovery already produced DSL (direct dsl, scope, macro) → use as-is
    // Otherwise → use forced-verb with the post-SemReg top candidate
    let result = if sem_reg_denied_all && policy.semreg_fail_closed() {
        // Strict deny-all: no DSL generation
        use crate::mcp::intent_pipeline::{PipelineOutcome, StructuredIntent};
        PipelineResult {
            intent: StructuredIntent::empty(),
            verb_candidates: filtered_candidates,
            dsl: String::new(),
            dsl_hash: None,
            valid: false,
            validation_error: Some("All verb candidates denied by Semantic Registry (strict mode)".into()),
            unresolved_refs: vec![],
            missing_required: vec![],
            outcome: PipelineOutcome::NoAllowedVerbs,
            scope_resolution: discovery_result.scope_resolution,
            scope_context: discovery_result.scope_context,
        }
    } else if !discovery_result.dsl.is_empty() {
        // Pipeline already produced DSL (direct dsl, scope resolution, macro, etc.)
        // Update verb_candidates with filtered list
        let mut result = discovery_result;
        result.verb_candidates = filtered_candidates;
        result
    } else if !filtered_candidates.is_empty() && discovery_result.outcome == crate::mcp::intent_pipeline::PipelineOutcome::NeedsClarification {
        // Ambiguous — return filtered candidates for user to pick
        let mut result = discovery_result;
        result.verb_candidates = filtered_candidates;
        result
    } else if let Some(top) = filtered_candidates.first() {
        // Clear winner post-SemReg → forced-verb generation
        let searcher2 = (*ctx.verb_searcher).clone();
        let pipeline2 = IntentPipeline::with_pool(searcher2, ctx.pool.clone())
            .set_allow_direct_dsl(policy.can_use_direct_dsl(&ctx.actor));
        let mut forced_result = pipeline2
            .process_with_forced_verb(utterance, &top.verb, ctx.scope.clone())
            .await?;
        forced_result.verb_candidates = filtered_candidates;
        forced_result
    } else {
        // No candidates at all
        let mut result = discovery_result;
        result.verb_candidates = filtered_candidates;
        result
    };

    // ── Step 6: Build IntentTrace ───────────────────────────────
    let final_verb = result.verb_candidates.first().map(|v| v.verb.clone());
    let final_confidence = result.verb_candidates.first().map(|v| v.score).unwrap_or(0.0);
    let bypass_used = if utterance.trim().starts_with("dsl:") && policy.can_use_direct_dsl(&ctx.actor) {
        Some("direct_dsl".to_string())
    } else {
        None
    };

    let trace = IntentTrace {
        utterance: utterance.to_string(),
        source: ctx.source.clone(),
        entity_candidates,
        dominant_entity: dominant_entity_name,
        sem_reg_verb_filter: sem_reg_verb_names.clone(),
        verb_candidates_pre_filter: pre_filter,
        verb_candidates_post_filter: post_filter,
        final_verb,
        final_confidence,
        dsl_generated: Some(result.dsl.clone()).filter(|d| !d.is_empty()),
        dsl_hash: result.dsl_hash.clone(),
        bypass_used,
        dsl_source: Some(format!("{:?}", ctx.source)),
        forced_verb: None,
        blocked_reason: blocked_reason.clone(),
        sem_reg_mode: if policy.semreg_fail_closed() { "strict".into() } else { "permissive".into() },
        sem_reg_denied_all,
        policy_gate_snapshot: policy.snapshot(),
    };

    tracing::info!(
        source = ?trace.source,
        final_verb = ?trace.final_verb,
        confidence = trace.final_confidence,
        sem_reg_filtered = trace.sem_reg_verb_filter.is_some(),
        bypass = ?trace.bypass_used,
        sem_reg_denied_all = trace.sem_reg_denied_all,
        sem_reg_mode = %trace.sem_reg_mode,
        "IntentTrace"
    );
    tracing::debug!(trace = %serde_json::to_string(&trace).unwrap_or_default(), "IntentTrace detail");

    Ok(OrchestratorOutcome {
        pipeline_result: result,
        sem_reg_verbs: sem_reg_verb_names,
        lookup_result,
        trace,
    })
}



/// Process an utterance with a forced verb selection (binding disambiguation).
///
/// Used when the user has selected a specific verb from an ambiguity menu.
/// Skips verb discovery and SemReg filtering — the verb was already approved
/// during the initial discovery phase.
#[cfg(feature = "database")]
pub async fn handle_utterance_with_forced_verb(
    ctx: &OrchestratorContext,
    utterance: &str,
    forced_verb_fqn: &str,
) -> anyhow::Result<OrchestratorOutcome> {
    let policy = &ctx.policy_gate;

    // Entity linking (same as handle_utterance)
    let lookup_result = if let Some(ref lookup_svc) = ctx.lookup_service {
        Some(lookup_svc.analyze(utterance, 5).await)
    } else {
        None
    };

    let dominant_entity_name = lookup_result
        .as_ref()
        .and_then(|lr| lr.dominant_entity.as_ref())
        .map(|e| e.canonical_name.clone());

    let entity_candidates: Vec<String> = lookup_result
        .as_ref()
        .map(|lr| lr.entities.iter().map(|e| e.mention_text.clone()).collect())
        .unwrap_or_default();

    // Build pipeline and use forced verb
    let searcher = (*ctx.verb_searcher).clone();
    let pipeline = IntentPipeline::with_pool(searcher, ctx.pool.clone())
        .set_allow_direct_dsl(policy.can_use_direct_dsl(&ctx.actor));

    let result = pipeline
        .process_with_forced_verb(utterance, forced_verb_fqn, ctx.scope.clone())
        .await?;

    let trace = IntentTrace {
        utterance: utterance.to_string(),
        source: ctx.source.clone(),
        entity_candidates,
        dominant_entity: dominant_entity_name,
        sem_reg_verb_filter: None, // Skipped — verb was pre-approved
        verb_candidates_pre_filter: vec![],
        verb_candidates_post_filter: vec![(forced_verb_fqn.to_string(), 1.0)],
        final_verb: Some(forced_verb_fqn.to_string()),
        final_confidence: 1.0,
        dsl_generated: Some(result.dsl.clone()).filter(|d| !d.is_empty()),
        dsl_hash: result.dsl_hash.clone(),
        bypass_used: None,
        dsl_source: Some(format!("{:?}", ctx.source)),
        sem_reg_mode: if policy.semreg_fail_closed() { "strict".into() } else { "permissive".into() },
        sem_reg_denied_all: false,
        policy_gate_snapshot: policy.snapshot(),
        forced_verb: Some(forced_verb_fqn.to_string()),
        blocked_reason: None,
    };

    tracing::info!(
        source = ?trace.source,
        forced_verb = forced_verb_fqn,
        dsl_generated = trace.dsl_generated.is_some(),
        "IntentTrace (forced verb)"
    );

    Ok(OrchestratorOutcome {
        pipeline_result: result,
        sem_reg_verbs: None,
        lookup_result,
        trace,
    })
}
/// Resolve SemReg context and extract allowed verb FQNs.
///
/// Returns `(allowed_set, verb_names_for_trace)`. Both are None if
/// SemReg is unavailable or resolution fails (graceful degradation).
#[cfg(feature = "database")]
async fn resolve_sem_reg_verbs(
    ctx: &OrchestratorContext,
) -> (Option<HashSet<String>>, Option<Vec<String>>) {
    use crate::sem_reg::abac::AccessDecision;
    use crate::sem_reg::context_resolution::{
        resolve_context, ContextResolutionRequest, EvidenceMode, SubjectRef,
    };

    // Use dominant entity when available (more specific), fall back to case_id
    let subject = if let Some(entity_id) = ctx.dominant_entity_id {
        SubjectRef::EntityId(entity_id)
    } else {
        SubjectRef::CaseId(ctx.case_id.unwrap_or_else(|| ctx.session_id.unwrap_or_else(Uuid::new_v4)))
    };
    let request = ContextResolutionRequest {
        subject,
        intent: None,
        actor: ctx.actor.clone(),
        goals: vec![],
        constraints: Default::default(),
        evidence_mode: EvidenceMode::default(),
        point_in_time: None,
    };

    match resolve_context(&ctx.pool, &request).await {
        Ok(response) => {
            let allowed: HashSet<String> = response
                .candidate_verbs
                .iter()
                .filter(|v| matches!(v.access_decision, AccessDecision::Allow))
                .map(|v| v.fqn.clone())
                .collect();
            let names: Vec<String> = allowed.iter().cloned().collect();
            tracing::debug!(
                allowed_count = allowed.len(),
                total_candidates = response.candidate_verbs.len(),
                "SemReg verb filter resolved"
            );
            if allowed.is_empty() {
                (None, None) // No SemReg verbs → don't filter
            } else {
                (Some(allowed), Some(names))
            }
        }
        Err(e) => {
            tracing::warn!(
                error = %e,
                source = "sem_reg",
                fallback = "unfiltered",
                "SemReg context resolution failed — continuing without verb filter"
            );
            (None, None)
        }
    }
}

// ── Tests ─────────────────────────────────────────────────────

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_intent_trace_serialization() {
        let trace = IntentTrace {
            utterance: "show all cases".into(),
            source: UtteranceSource::Chat,
            entity_candidates: vec!["Allianz".into()],
            dominant_entity: Some("Allianz".into()),
            #[cfg(feature = "database")]
            sem_reg_verb_filter: Some(vec!["kyc.open-case".into()]),
            verb_candidates_pre_filter: vec![("kyc.open-case".into(), 0.95)],
            verb_candidates_post_filter: vec![("kyc.open-case".into(), 0.95)],
            final_verb: Some("kyc.open-case".into()),
            final_confidence: 0.95,
            dsl_generated: Some("(kyc.open-case)".into()),
            dsl_hash: Some("abc123".into()),
            bypass_used: None,
            dsl_source: Some("chat".into()),
            sem_reg_mode: "strict".into(),
            sem_reg_denied_all: false,
            policy_gate_snapshot: crate::policy::PolicyGate::strict().snapshot(),
            forced_verb: None,
            blocked_reason: None,
        };

        let json = serde_json::to_string(&trace).unwrap();
        assert!(json.contains("kyc.open-case"));
        assert!(json.contains("chat"));
    }


    #[test]
    fn test_intent_trace_forced_verb_field() {
        let trace = IntentTrace {
            utterance: "create a fund".into(),
            source: UtteranceSource::Chat,
            entity_candidates: vec![],
            dominant_entity: None,
            #[cfg(feature = "database")]
            sem_reg_verb_filter: None,
            verb_candidates_pre_filter: vec![],
            verb_candidates_post_filter: vec![("cbu.create".into(), 1.0)],
            final_verb: Some("cbu.create".into()),
            final_confidence: 1.0,
            dsl_generated: Some("(cbu.create)".into()),
            dsl_hash: None,
            bypass_used: None,
            dsl_source: Some("chat".into()),
            sem_reg_mode: "strict".into(),
            sem_reg_denied_all: false,
            policy_gate_snapshot: crate::policy::PolicyGate::strict().snapshot(),
            forced_verb: Some("cbu.create".into()),
            blocked_reason: None,
        };

        let json = serde_json::to_string(&trace).unwrap();
        assert!(json.contains("forced_verb"));
        assert!(json.contains("cbu.create"));
    }

    #[test]
    fn test_intent_trace_blocked_reason_field() {
        let trace = IntentTrace {
            utterance: "show cases".into(),
            source: UtteranceSource::Chat,
            entity_candidates: vec![],
            dominant_entity: None,
            #[cfg(feature = "database")]
            sem_reg_verb_filter: Some(vec![]),
            verb_candidates_pre_filter: vec![("kyc.open-case".into(), 0.9)],
            verb_candidates_post_filter: vec![],
            final_verb: None,
            final_confidence: 0.0,
            dsl_generated: None,
            dsl_hash: None,
            bypass_used: None,
            dsl_source: Some("chat".into()),
            sem_reg_mode: "strict".into(),
            sem_reg_denied_all: true,
            policy_gate_snapshot: crate::policy::PolicyGate::strict().snapshot(),
            forced_verb: None,
            blocked_reason: Some("SemReg denied all verb candidates (strict mode)".into()),
        };

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
}
