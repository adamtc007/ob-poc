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
use crate::mcp::intent_pipeline::{IntentPipeline, PipelineResult};
use crate::mcp::scope_resolution::ScopeContext;
use crate::mcp::verb_search::HybridVerbSearcher;
use crate::sem_reg::abac::ActorContext;

/// Context needed to run the unified orchestrator.
pub struct OrchestratorContext {
    pub actor: ActorContext,
    pub session_id: Option<Uuid>,
    pub case_id: Option<Uuid>,
    pub scope: Option<ScopeContext>,
    #[cfg(feature = "database")]
    pub pool: PgPool,
    pub verb_searcher: Arc<HybridVerbSearcher>,
    pub lookup_service: Option<LookupService>,
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
    let is_operator = ctx.actor.roles.iter().any(|r| r == "operator" || r == "admin");

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

    // ── Step 3: Build and run IntentPipeline ────────────────────
    let searcher = (*ctx.verb_searcher).clone();
    let pipeline = IntentPipeline::with_pool(searcher, ctx.pool.clone())
        .set_allow_direct_dsl(is_operator);

    let mut result = pipeline
        .process_with_scope(utterance, None, ctx.scope.clone())
        .await?;

    // ── Step 4: Capture pre-filter candidates ───────────────────
    let pre_filter: Vec<(String, f32)> = result
        .verb_candidates
        .iter()
        .map(|v| (v.verb.clone(), v.score))
        .collect();

    // ── Step 5: Post-filter by SemReg allowed set ───────────────
    if let Some(ref allowed) = allowed_verbs {
        let before_count = result.verb_candidates.len();
        result.verb_candidates.retain(|v| allowed.contains(&v.verb));
        if result.verb_candidates.is_empty() && before_count > 0 {
            // All candidates filtered — log governance warning, restore originals
            tracing::warn!(
                before = before_count,
                allowed_count = allowed.len(),
                "SemReg filtered ALL verb candidates — falling back to unfiltered"
            );
            // Re-run without filter (graceful degradation)
            let searcher2 = (*ctx.verb_searcher).clone();
            let pipeline2 = IntentPipeline::with_pool(searcher2, ctx.pool.clone())
                .set_allow_direct_dsl(is_operator);
            result = pipeline2
                .process_with_scope(utterance, None, ctx.scope.clone())
                .await?;
        }
    }

    let post_filter: Vec<(String, f32)> = result
        .verb_candidates
        .iter()
        .map(|v| (v.verb.clone(), v.score))
        .collect();

    // ── Step 6: Build IntentTrace ───────────────────────────────
    let final_verb = result.verb_candidates.first().map(|v| v.verb.clone());
    let final_confidence = result.verb_candidates.first().map(|v| v.score).unwrap_or(0.0);
    let bypass_used = if utterance.trim().starts_with('(') && is_operator {
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
    };

    tracing::info!(
        source = ?trace.source,
        final_verb = ?trace.final_verb,
        confidence = trace.final_confidence,
        sem_reg_filtered = trace.sem_reg_verb_filter.is_some(),
        bypass = ?trace.bypass_used,
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

    let subject_id = ctx.case_id.unwrap_or_else(Uuid::new_v4);
    let request = ContextResolutionRequest {
        subject: SubjectRef::CaseId(subject_id),
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
        };

        let json = serde_json::to_string(&trace).unwrap();
        assert!(json.contains("kyc.open-case"));
        assert!(json.contains("chat"));
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
