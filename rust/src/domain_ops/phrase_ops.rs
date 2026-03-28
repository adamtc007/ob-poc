//! Phrase authoring custom operations (Governed Phrase Authoring — Phases 1+2)
//!
//! Operations for phrase observation, coverage analysis, collision checking,
//! and governed phrase lifecycle (propose/approve/reject/defer).
//!
//! Phase 1: Observation infrastructure (observe-misses, coverage-report)
//! Phase 2: Phrase bank + collision checking (check-collisions)
//! Phase 3/4: Proposal lifecycle (propose, batch-propose, review, approve, reject, defer) — stubs

use anyhow::Result;
use async_trait::async_trait;
use ob_poc_macros::register_custom_op;
use serde::{Deserialize, Serialize};

use super::{CustomOperation, ExecutionContext, ExecutionResult, VerbCall};

#[cfg(feature = "database")]
use sqlx::PgPool;

// ============================================================================
// Result Types (Type Safety First — CLAUDE.md §1)
// ============================================================================

/// Result from phrase observation run
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ObserveMissesResult {
    pub miss_count: i64,
    pub wrong_match_count: i64,
    pub top_miss_patterns: Vec<MissPattern>,
    pub top_wrong_match_patterns: Vec<WrongMatchPattern>,
    pub watermark_advanced_to: i64,
}

/// A miss pattern extracted from session traces
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct MissPattern {
    pub utterance: String,
    pub occurrences: i64,
    pub first_seen: String,
    pub last_seen: String,
}

/// A wrong-match pattern extracted from session traces
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct WrongMatchPattern {
    pub utterance: String,
    pub matched_verb: String,
    pub occurrences: i64,
}

/// Per-workspace coverage entry
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct WorkspaceCoverage {
    pub domain: String,
    pub verb_count: i64,
    pub phrase_count: i64,
    pub avg_phrases_per_verb: f64,
}

/// Collision check result
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct CollisionReport {
    pub candidate_phrase: String,
    pub target_verb: String,
    pub workspace: Option<String>,
    pub exact_conflicts: Vec<ExactConflict>,
    pub semantic_near_misses: Vec<SemanticNearMiss>,
    pub cross_workspace_conflicts: Vec<CrossWorkspaceConflict>,
    pub safe_to_propose: bool,
}

/// An exact phrase conflict
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ExactConflict {
    pub existing_verb: String,
    pub source: String,
}

/// A semantically similar phrase that might cause confusion
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct SemanticNearMiss {
    pub phrase: String,
    pub verb_fqn: String,
    pub similarity: f64,
}

/// A conflict in a different workspace
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct CrossWorkspaceConflict {
    pub phrase: String,
    pub verb_fqn: String,
    pub workspace: Option<String>,
}

/// Stub result for unimplemented operations
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct StubResult {
    pub status: String,
    pub message: String,
}

// ============================================================================
// Helper Functions
// ============================================================================

/// Extract an optional integer argument from verb call
#[cfg(feature = "database")]
fn get_optional_integer(verb_call: &VerbCall, key: &str) -> Option<i64> {
    verb_call
        .arguments
        .iter()
        .find(|a| a.key == key)
        .and_then(|a| a.value.as_integer())
}

/// Extract a required string argument from verb call
#[cfg(feature = "database")]
fn get_required_string(verb_call: &VerbCall, key: &str) -> Result<String> {
    verb_call
        .arguments
        .iter()
        .find(|a| a.key == key)
        .and_then(|a| a.value.as_string().map(|s| s.to_string()))
        .ok_or_else(|| anyhow::anyhow!("Missing required argument :{}", key))
}

/// Extract an optional string argument from verb call
#[cfg(feature = "database")]
fn get_optional_string(verb_call: &VerbCall, key: &str) -> Option<String> {
    verb_call
        .arguments
        .iter()
        .find(|a| a.key == key)
        .and_then(|a| a.value.as_string().map(|s| s.to_string()))
}

// ============================================================================
// Phase 1: Observation Infrastructure
// ============================================================================

/// Trawl session traces for utterance miss and wrong-match patterns.
///
/// Rationale: Requires watermark-based incremental scan across session_traces,
/// pattern aggregation, and state update — multi-step transactional logic.
#[register_custom_op]
pub struct PhraseObserveMissesOp;

#[async_trait]
impl CustomOperation for PhraseObserveMissesOp {
    fn domain(&self) -> &'static str {
        "phrase"
    }
    fn verb(&self) -> &'static str {
        "observe-misses"
    }
    fn rationale(&self) -> &'static str {
        "Watermark-based incremental scan across session_traces with pattern aggregation"
    }

    #[cfg(feature = "database")]
    async fn execute(
        &self,
        verb_call: &VerbCall,
        _ctx: &mut ExecutionContext,
        pool: &PgPool,
    ) -> Result<ExecutionResult> {
        let limit = get_optional_integer(verb_call, "limit").unwrap_or(100);

        // 1. Read current watermark
        let watermark: (i64,) = sqlx::query_as(
            r#"SELECT last_observed_sequence FROM "ob-poc".phrase_observation_state WHERE id = 1"#,
        )
        .fetch_one(pool)
        .await?;
        let last_seq = watermark.0;

        // 2. Query session_traces for miss patterns since watermark
        //    Misses are traces where the op contains a verb_search that returned no results
        //    or returned results the user rejected (wrong-match).
        let miss_rows: Vec<(String, i64)> = sqlx::query_as(
            r#"
            SELECT
                op->>'utterance' AS utterance,
                COUNT(*)::bigint AS occurrences
            FROM "ob-poc".session_traces
            WHERE sequence > $1
              AND op->>'kind' = 'utterance'
              AND (
                  op->'result'->>'match_status' = 'no_match'
                  OR op->'result'->>'match_status' IS NULL
              )
              AND op->>'utterance' IS NOT NULL
            GROUP BY op->>'utterance'
            ORDER BY occurrences DESC
            LIMIT $2
            "#,
        )
        .bind(last_seq)
        .bind(limit)
        .fetch_all(pool)
        .await
        .unwrap_or_default();

        // 3. Query for wrong-match patterns (user rejected the matched verb)
        let wrong_match_rows: Vec<(String, String, i64)> = sqlx::query_as(
            r#"
            SELECT
                op->>'utterance' AS utterance,
                op->'result'->>'matched_verb' AS matched_verb,
                COUNT(*)::bigint AS occurrences
            FROM "ob-poc".session_traces
            WHERE sequence > $1
              AND op->>'kind' = 'utterance'
              AND op->'result'->>'match_status' = 'wrong_match'
              AND op->>'utterance' IS NOT NULL
            GROUP BY op->>'utterance', op->'result'->>'matched_verb'
            ORDER BY occurrences DESC
            LIMIT $2
            "#,
        )
        .bind(last_seq)
        .bind(limit)
        .fetch_all(pool)
        .await
        .unwrap_or_default();

        // 4. Find the new watermark (max sequence observed)
        let new_watermark: (Option<i64>,) = sqlx::query_as(
            r#"SELECT MAX(sequence) FROM "ob-poc".session_traces WHERE sequence > $1"#,
        )
        .bind(last_seq)
        .fetch_one(pool)
        .await?;
        let advanced_to = new_watermark.0.unwrap_or(last_seq);

        let miss_count = miss_rows.iter().map(|(_, c)| c).sum::<i64>();
        let wrong_match_count = wrong_match_rows.iter().map(|(_, _, c)| c).sum::<i64>();

        // 5. Update watermark
        sqlx::query(
            r#"
            UPDATE "ob-poc".phrase_observation_state
            SET last_observed_sequence = $1,
                last_run_at = NOW(),
                patterns_found = $2,
                wrong_match_patterns_found = $3
            WHERE id = 1
            "#,
        )
        .bind(advanced_to)
        .bind(miss_count as i32)
        .bind(wrong_match_count as i32)
        .execute(pool)
        .await?;

        // 6. Build result
        let top_miss_patterns: Vec<MissPattern> = miss_rows
            .into_iter()
            .map(|(utterance, occurrences)| MissPattern {
                utterance,
                occurrences,
                first_seen: String::new(),
                last_seen: String::new(),
            })
            .collect();

        let top_wrong_match_patterns: Vec<WrongMatchPattern> = wrong_match_rows
            .into_iter()
            .map(|(utterance, matched_verb, occurrences)| WrongMatchPattern {
                utterance,
                matched_verb,
                occurrences,
            })
            .collect();

        let result = ObserveMissesResult {
            miss_count,
            wrong_match_count,
            top_miss_patterns,
            top_wrong_match_patterns,
            watermark_advanced_to: advanced_to,
        };

        Ok(ExecutionResult::Record(serde_json::to_value(result)?))
    }

    #[cfg(not(feature = "database"))]
    async fn execute(
        &self,
        _verb_call: &VerbCall,
        _ctx: &mut ExecutionContext,
    ) -> Result<ExecutionResult> {
        Err(anyhow::anyhow!(
            "phrase.observe-misses requires database feature"
        ))
    }
}

/// Per-workspace phrase coverage and gap analysis.
///
/// Rationale: Requires cross-join between dsl_verbs and verb_pattern_embeddings
/// with domain-level aggregation — complex reporting query.
#[register_custom_op]
pub struct PhraseCoverageReportOp;

#[async_trait]
impl CustomOperation for PhraseCoverageReportOp {
    fn domain(&self) -> &'static str {
        "phrase"
    }
    fn verb(&self) -> &'static str {
        "coverage-report"
    }
    fn rationale(&self) -> &'static str {
        "Cross-join between dsl_verbs and verb_pattern_embeddings with domain-level aggregation"
    }

    #[cfg(feature = "database")]
    async fn execute(
        &self,
        _verb_call: &VerbCall,
        _ctx: &mut ExecutionContext,
        pool: &PgPool,
    ) -> Result<ExecutionResult> {
        // Query verb count per domain and phrase count per domain
        let rows: Vec<(String, i64, i64)> = sqlx::query_as(
            r#"
            SELECT
                v.domain,
                COUNT(DISTINCT v.verb_fqn)::bigint AS verb_count,
                COALESCE(e.phrase_count, 0)::bigint AS phrase_count
            FROM "ob-poc".dsl_verbs v
            LEFT JOIN (
                SELECT
                    split_part(verb_fqn, '.', 1) AS domain,
                    COUNT(*)::bigint AS phrase_count
                FROM "ob-poc".verb_pattern_embeddings
                GROUP BY split_part(verb_fqn, '.', 1)
            ) e ON e.domain = v.domain
            GROUP BY v.domain, e.phrase_count
            ORDER BY verb_count DESC
            "#,
        )
        .fetch_all(pool)
        .await?;

        let entries: Vec<serde_json::Value> = rows
            .into_iter()
            .map(|(domain, verb_count, phrase_count)| {
                let avg = if verb_count > 0 {
                    phrase_count as f64 / verb_count as f64
                } else {
                    0.0
                };
                let coverage = WorkspaceCoverage {
                    domain,
                    verb_count,
                    phrase_count,
                    avg_phrases_per_verb: (avg * 100.0).round() / 100.0,
                };
                serde_json::to_value(coverage).unwrap_or_default()
            })
            .collect();

        Ok(ExecutionResult::RecordSet(entries))
    }

    #[cfg(not(feature = "database"))]
    async fn execute(
        &self,
        _verb_call: &VerbCall,
        _ctx: &mut ExecutionContext,
    ) -> Result<ExecutionResult> {
        Err(anyhow::anyhow!(
            "phrase.coverage-report requires database feature"
        ))
    }
}

// ============================================================================
// Phase 2: Collision Checking
// ============================================================================

/// Check a candidate phrase for collisions against the phrase bank and embeddings.
///
/// Rationale: Requires exact match against phrase_bank + verb_pattern_embeddings,
/// plus semantic similarity check — multi-source query with aggregation.
#[register_custom_op]
pub struct PhraseCheckCollisionsOp;

#[async_trait]
impl CustomOperation for PhraseCheckCollisionsOp {
    fn domain(&self) -> &'static str {
        "phrase"
    }
    fn verb(&self) -> &'static str {
        "check-collisions"
    }
    fn rationale(&self) -> &'static str {
        "Multi-source collision check: phrase_bank exact, embeddings exact, semantic similarity"
    }

    #[cfg(feature = "database")]
    async fn execute(
        &self,
        verb_call: &VerbCall,
        _ctx: &mut ExecutionContext,
        pool: &PgPool,
    ) -> Result<ExecutionResult> {
        let phrase = get_required_string(verb_call, "phrase")?;
        let target_verb = get_required_string(verb_call, "target-verb")?;
        let workspace = get_optional_string(verb_call, "workspace");

        let mut exact_conflicts = Vec::new();
        let mut cross_workspace_conflicts = Vec::new();

        // 1. Check exact match in phrase_bank (if table exists)
        let bank_rows: Vec<(String, Option<String>, String)> = sqlx::query_as(
            r#"
            SELECT verb_fqn, workspace, source
            FROM "ob-poc".phrase_bank
            WHERE phrase = $1 AND active = TRUE
            "#,
        )
        .bind(&phrase)
        .fetch_all(pool)
        .await
        .unwrap_or_default();

        for (verb_fqn, ws, source) in &bank_rows {
            if verb_fqn != &target_verb {
                if ws.as_deref() == workspace.as_deref() {
                    exact_conflicts.push(ExactConflict {
                        existing_verb: verb_fqn.clone(),
                        source: format!("phrase_bank ({})", source),
                    });
                } else {
                    cross_workspace_conflicts.push(CrossWorkspaceConflict {
                        phrase: phrase.clone(),
                        verb_fqn: verb_fqn.clone(),
                        workspace: ws.clone(),
                    });
                }
            }
        }

        // 2. Check exact match in verb_pattern_embeddings
        let embed_rows: Vec<(String,)> = sqlx::query_as(
            r#"
            SELECT DISTINCT verb_fqn
            FROM "ob-poc".verb_pattern_embeddings
            WHERE pattern = $1 AND verb_fqn != $2
            "#,
        )
        .bind(&phrase)
        .bind(&target_verb)
        .fetch_all(pool)
        .await
        .unwrap_or_default();

        for (verb_fqn,) in embed_rows {
            exact_conflicts.push(ExactConflict {
                existing_verb: verb_fqn,
                source: "verb_pattern_embeddings".to_string(),
            });
        }

        // 3. Semantic near-miss check via embedding cosine similarity
        //    Query the top-N most similar embeddings to the candidate phrase.
        //    We use pgvector cosine distance operator (<=>).
        let semantic_rows: Vec<(String, String, f64)> = sqlx::query_as(
            r#"
            SELECT pattern, verb_fqn, 1.0 - (embedding <=> (
                SELECT embedding FROM "ob-poc".verb_pattern_embeddings
                WHERE pattern = $1
                LIMIT 1
            )) AS similarity
            FROM "ob-poc".verb_pattern_embeddings
            WHERE verb_fqn != $2
              AND pattern != $1
            ORDER BY embedding <=> (
                SELECT embedding FROM "ob-poc".verb_pattern_embeddings
                WHERE pattern = $1
                LIMIT 1
            )
            LIMIT 10
            "#,
        )
        .bind(&phrase)
        .bind(&target_verb)
        .fetch_all(pool)
        .await
        .unwrap_or_default();

        let semantic_near_misses: Vec<SemanticNearMiss> = semantic_rows
            .into_iter()
            .filter(|(_, _, sim)| *sim > 0.85)
            .map(|(p, v, sim)| SemanticNearMiss {
                phrase: p,
                verb_fqn: v,
                similarity: (sim * 1000.0).round() / 1000.0,
            })
            .collect();

        let safe_to_propose =
            exact_conflicts.is_empty() && cross_workspace_conflicts.is_empty();

        let report = CollisionReport {
            candidate_phrase: phrase,
            target_verb,
            workspace,
            exact_conflicts,
            semantic_near_misses,
            cross_workspace_conflicts,
            safe_to_propose,
        };

        Ok(ExecutionResult::Record(serde_json::to_value(report)?))
    }

    #[cfg(not(feature = "database"))]
    async fn execute(
        &self,
        _verb_call: &VerbCall,
        _ctx: &mut ExecutionContext,
    ) -> Result<ExecutionResult> {
        Err(anyhow::anyhow!(
            "phrase.check-collisions requires database feature"
        ))
    }
}

// ============================================================================
// Phase 3/4 Stubs: Proposal Lifecycle
// ============================================================================

/// Generate a governed phrase proposal with evidence and collision report.
#[register_custom_op]
pub struct PhraseProposeOp;

#[async_trait]
impl CustomOperation for PhraseProposeOp {
    fn domain(&self) -> &'static str {
        "phrase"
    }
    fn verb(&self) -> &'static str {
        "propose"
    }
    fn rationale(&self) -> &'static str {
        "Proposal creation with collision check, risk tier assignment, and SemOS changeset wiring"
    }

    #[cfg(feature = "database")]
    async fn execute(
        &self,
        _verb_call: &VerbCall,
        _ctx: &mut ExecutionContext,
        _pool: &PgPool,
    ) -> Result<ExecutionResult> {
        let result = StubResult {
            status: "not_implemented".to_string(),
            message: "phrase.propose is not yet implemented — Phase 3/4".to_string(),
        };
        Ok(ExecutionResult::Record(serde_json::to_value(result)?))
    }

    #[cfg(not(feature = "database"))]
    async fn execute(
        &self,
        _verb_call: &VerbCall,
        _ctx: &mut ExecutionContext,
    ) -> Result<ExecutionResult> {
        Err(anyhow::anyhow!("phrase.propose requires database feature"))
    }
}

/// Generate bulk proposals from aggregated miss analysis (max 50).
#[register_custom_op]
pub struct PhraseBatchProposeOp;

#[async_trait]
impl CustomOperation for PhraseBatchProposeOp {
    fn domain(&self) -> &'static str {
        "phrase"
    }
    fn verb(&self) -> &'static str {
        "batch-propose"
    }
    fn rationale(&self) -> &'static str {
        "Batch proposal generation with per-phrase collision checks and risk tier aggregation"
    }

    #[cfg(feature = "database")]
    async fn execute(
        &self,
        _verb_call: &VerbCall,
        _ctx: &mut ExecutionContext,
        _pool: &PgPool,
    ) -> Result<ExecutionResult> {
        let result = StubResult {
            status: "not_implemented".to_string(),
            message: "phrase.batch-propose is not yet implemented — Phase 3/4".to_string(),
        };
        Ok(ExecutionResult::Record(serde_json::to_value(result)?))
    }

    #[cfg(not(feature = "database"))]
    async fn execute(
        &self,
        _verb_call: &VerbCall,
        _ctx: &mut ExecutionContext,
    ) -> Result<ExecutionResult> {
        Err(anyhow::anyhow!(
            "phrase.batch-propose requires database feature"
        ))
    }
}

/// List pending phrase proposals grouped by risk tier.
#[register_custom_op]
pub struct PhraseReviewProposalsOp;

#[async_trait]
impl CustomOperation for PhraseReviewProposalsOp {
    fn domain(&self) -> &'static str {
        "phrase"
    }
    fn verb(&self) -> &'static str {
        "review-proposals"
    }
    fn rationale(&self) -> &'static str {
        "Multi-table join across proposals, collision reports, and risk tiers with grouping"
    }

    #[cfg(feature = "database")]
    async fn execute(
        &self,
        _verb_call: &VerbCall,
        _ctx: &mut ExecutionContext,
        _pool: &PgPool,
    ) -> Result<ExecutionResult> {
        let result = StubResult {
            status: "not_implemented".to_string(),
            message: "phrase.review-proposals is not yet implemented — Phase 3/4".to_string(),
        };
        Ok(ExecutionResult::Record(serde_json::to_value(result)?))
    }

    #[cfg(not(feature = "database"))]
    async fn execute(
        &self,
        _verb_call: &VerbCall,
        _ctx: &mut ExecutionContext,
    ) -> Result<ExecutionResult> {
        Err(anyhow::anyhow!(
            "phrase.review-proposals requires database feature"
        ))
    }
}

/// Approve a phrase proposal and publish through SemOS governance.
#[register_custom_op]
pub struct PhraseApproveOp;

#[async_trait]
impl CustomOperation for PhraseApproveOp {
    fn domain(&self) -> &'static str {
        "phrase"
    }
    fn verb(&self) -> &'static str {
        "approve"
    }
    fn rationale(&self) -> &'static str {
        "Approval requires SemOS changeset creation, phrase_bank insertion, and embedding generation"
    }

    #[cfg(feature = "database")]
    async fn execute(
        &self,
        _verb_call: &VerbCall,
        _ctx: &mut ExecutionContext,
        _pool: &PgPool,
    ) -> Result<ExecutionResult> {
        let result = StubResult {
            status: "not_implemented".to_string(),
            message: "phrase.approve is not yet implemented — Phase 3/4".to_string(),
        };
        Ok(ExecutionResult::Record(serde_json::to_value(result)?))
    }

    #[cfg(not(feature = "database"))]
    async fn execute(
        &self,
        _verb_call: &VerbCall,
        _ctx: &mut ExecutionContext,
    ) -> Result<ExecutionResult> {
        Err(anyhow::anyhow!("phrase.approve requires database feature"))
    }
}

/// Reject a phrase proposal with reason code.
#[register_custom_op]
pub struct PhraseRejectOp;

#[async_trait]
impl CustomOperation for PhraseRejectOp {
    fn domain(&self) -> &'static str {
        "phrase"
    }
    fn verb(&self) -> &'static str {
        "reject"
    }
    fn rationale(&self) -> &'static str {
        "Rejection requires state transition and audit trail recording"
    }

    #[cfg(feature = "database")]
    async fn execute(
        &self,
        _verb_call: &VerbCall,
        _ctx: &mut ExecutionContext,
        _pool: &PgPool,
    ) -> Result<ExecutionResult> {
        let result = StubResult {
            status: "not_implemented".to_string(),
            message: "phrase.reject is not yet implemented — Phase 3/4".to_string(),
        };
        Ok(ExecutionResult::Record(serde_json::to_value(result)?))
    }

    #[cfg(not(feature = "database"))]
    async fn execute(
        &self,
        _verb_call: &VerbCall,
        _ctx: &mut ExecutionContext,
    ) -> Result<ExecutionResult> {
        Err(anyhow::anyhow!("phrase.reject requires database feature"))
    }
}

/// Park a phrase proposal for later review.
#[register_custom_op]
pub struct PhraseDeferOp;

#[async_trait]
impl CustomOperation for PhraseDeferOp {
    fn domain(&self) -> &'static str {
        "phrase"
    }
    fn verb(&self) -> &'static str {
        "defer"
    }
    fn rationale(&self) -> &'static str {
        "Deferral requires state transition and optional reason recording"
    }

    #[cfg(feature = "database")]
    async fn execute(
        &self,
        _verb_call: &VerbCall,
        _ctx: &mut ExecutionContext,
        _pool: &PgPool,
    ) -> Result<ExecutionResult> {
        let result = StubResult {
            status: "not_implemented".to_string(),
            message: "phrase.defer is not yet implemented — Phase 3/4".to_string(),
        };
        Ok(ExecutionResult::Record(serde_json::to_value(result)?))
    }

    #[cfg(not(feature = "database"))]
    async fn execute(
        &self,
        _verb_call: &VerbCall,
        _ctx: &mut ExecutionContext,
    ) -> Result<ExecutionResult> {
        Err(anyhow::anyhow!("phrase.defer requires database feature"))
    }
}
