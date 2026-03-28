//! Phrase authoring custom operations (Governed Phrase Authoring — Phases 1-3)
//!
//! Operations for phrase observation, coverage analysis, collision checking,
//! and governed phrase lifecycle (propose/approve/reject/defer).
//!
//! Phase 1: Observation infrastructure (observe-misses, coverage-report)
//! Phase 2: Phrase bank + collision checking (check-collisions)
//! Phase 3: Proposal lifecycle (review-proposals, approve, reject, defer)
//! Phase 4: Batch operations (propose, batch-propose) — stubs

use anyhow::Result;
use async_trait::async_trait;
use ob_poc_macros::register_custom_op;
use serde::{Deserialize, Serialize};

use super::{CustomOperation, ExecutionContext, ExecutionResult, VerbCall};

#[cfg(feature = "database")]
use sqlx::PgPool;

#[cfg(feature = "database")]
use uuid::Uuid;

#[cfg(feature = "database")]
use anyhow::anyhow;

#[cfg(feature = "database")]
use sqlx::{Postgres, Transaction};

#[cfg(feature = "database")]
use crate::sem_reg::store::SnapshotStore;

#[cfg(feature = "database")]
use crate::sem_reg::types::{ChangeType, ObjectType, SnapshotMeta, SnapshotStatus};

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

/// Result from phrase approval
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct PhraseApproveResult {
    pub published_snapshot_id: Uuid,
    pub phrase: String,
    pub verb_fqn: String,
    pub status: String,
}

/// Result from phrase rejection or deferral
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct PhraseLifecycleResult {
    pub snapshot_id: Uuid,
    pub state: String,
    pub reason: Option<String>,
}

/// Summary of a phrase proposal for review listing
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct PhraseProposalSummary {
    pub snapshot_id: Uuid,
    pub phrase: String,
    pub verb_fqn: String,
    pub workspace: Option<String>,
    pub state: String,
    pub created_by: String,
    pub version: String,
    pub collision_report: Option<serde_json::Value>,
    pub evidence: Option<serde_json::Value>,
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
// SemOS Snapshot Helpers (Phase 3)
// ============================================================================

/// Build a `SnapshotMeta` for a phrase_mapping snapshot, optionally superseding
/// a predecessor.
#[cfg(feature = "database")]
fn next_phrase_meta(
    predecessor: Option<&crate::sem_reg::types::SnapshotRow>,
    object_id: Uuid,
    created_by: &str,
    change_type: ChangeType,
    change_rationale: Option<String>,
    status: SnapshotStatus,
) -> SnapshotMeta {
    let mut meta =
        SnapshotMeta::new_operational(ObjectType::PhraseMapping, object_id, created_by.to_string());
    meta.change_type = change_type;
    meta.change_rationale = change_rationale;
    meta.status = status;
    if let Some(pred) = predecessor {
        meta.version_major = pred.version_major;
        meta.version_minor = pred.version_minor + 1;
        meta.predecessor_id = Some(pred.snapshot_id);
    }
    meta
}

/// Publish a phrase_mapping snapshot within an existing transaction.
///
/// If the meta has a predecessor, the predecessor's `effective_until` is set to
/// NOW() before the new snapshot is inserted.
#[cfg(feature = "database")]
async fn publish_phrase_snapshot_in_tx(
    tx: &mut Transaction<'_, Postgres>,
    meta: &SnapshotMeta,
    definition: &serde_json::Value,
) -> Result<Uuid> {
    // Close predecessor if present
    if let Some(predecessor_id) = meta.predecessor_id {
        let affected = sqlx::query(
            r#"
            UPDATE sem_reg.snapshots
            SET effective_until = NOW()
            WHERE snapshot_id = $1 AND effective_until IS NULL
            "#,
        )
        .bind(predecessor_id)
        .execute(&mut **tx)
        .await?
        .rows_affected();
        if affected == 0 {
            return Err(anyhow!(
                "Predecessor snapshot {} not found or already superseded",
                predecessor_id
            ));
        }
    }

    let security_label = serde_json::to_value(&meta.security_label)?;
    let snapshot_id = sqlx::query_scalar::<_, Uuid>(
        r#"
        INSERT INTO sem_reg.snapshots (
            snapshot_set_id, object_type, object_id,
            version_major, version_minor, status,
            governance_tier, trust_class, security_label,
            predecessor_id, change_type, change_rationale,
            created_by, approved_by, definition
        ) VALUES (
            NULL, $1::sem_reg.object_type, $2,
            $3, $4, $5::sem_reg.snapshot_status,
            $6::sem_reg.governance_tier, $7::sem_reg.trust_class, $8,
            $9, $10::sem_reg.change_type, $11,
            $12, $13, $14
        )
        RETURNING snapshot_id
        "#,
    )
    .bind(meta.object_type.as_ref())
    .bind(meta.object_id)
    .bind(meta.version_major)
    .bind(meta.version_minor)
    .bind(meta.status.as_ref())
    .bind(meta.governance_tier.as_ref())
    .bind(meta.trust_class.as_ref())
    .bind(security_label)
    .bind(meta.predecessor_id)
    .bind(meta.change_type.as_ref())
    .bind(&meta.change_rationale)
    .bind(&meta.created_by)
    .bind(&meta.approved_by)
    .bind(definition)
    .fetch_one(&mut **tx)
    .await?;

    Ok(snapshot_id)
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

/// List pending phrase proposals from SemOS snapshots.
///
/// Queries `sem_reg.snapshots` for active `phrase_mapping` objects whose
/// definition state is not yet `published`, returning evidence and collision
/// reports embedded in each proposal's definition JSON.
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
        pool: &PgPool,
    ) -> Result<ExecutionResult> {
        // Query active phrase_mapping snapshots that are not yet published
        let rows: Vec<(Uuid, serde_json::Value, String, i32, i32)> = sqlx::query_as(
            r#"
            SELECT
                snapshot_id,
                definition,
                created_by,
                version_major,
                version_minor
            FROM sem_reg.snapshots
            WHERE object_type = 'phrase_mapping'
              AND status = 'active'
              AND effective_until IS NULL
              AND COALESCE(definition->>'state', 'proposed') != 'published'
            ORDER BY effective_from DESC
            "#,
        )
        .fetch_all(pool)
        .await?;

        let proposals: Vec<serde_json::Value> = rows
            .into_iter()
            .map(|(snapshot_id, definition, created_by, ver_major, ver_minor)| {
                let phrase = definition
                    .get("phrase")
                    .and_then(|v| v.as_str())
                    .unwrap_or("")
                    .to_string();
                let verb_fqn = definition
                    .get("verb_fqn")
                    .and_then(|v| v.as_str())
                    .unwrap_or("")
                    .to_string();
                let workspace = definition
                    .get("workspace")
                    .and_then(|v| v.as_str())
                    .map(|s| s.to_string());
                let state = definition
                    .get("state")
                    .and_then(|v| v.as_str())
                    .unwrap_or("proposed")
                    .to_string();
                let collision_report = definition.get("collision_report").cloned();
                let evidence = definition.get("evidence").cloned();

                serde_json::to_value(PhraseProposalSummary {
                    snapshot_id,
                    phrase,
                    verb_fqn,
                    workspace,
                    state,
                    created_by,
                    version: format!("{}.{}", ver_major, ver_minor),
                    collision_report,
                    evidence,
                })
                .unwrap_or_default()
            })
            .collect();

        Ok(ExecutionResult::RecordSet(proposals))
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
///
/// Looks up the proposal snapshot by `proposal-id`, extracts the phrase mapping
/// definition, creates a superseding snapshot with state=published and
/// status=active. The materialization trigger (Phase 2.5) automatically writes
/// the phrase to `phrase_bank`.
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
        verb_call: &VerbCall,
        ctx: &mut ExecutionContext,
        pool: &PgPool,
    ) -> Result<ExecutionResult> {
        let proposal_id_str = get_required_string(verb_call, "proposal-id")?;
        let proposal_id: Uuid = proposal_id_str
            .parse()
            .map_err(|_| anyhow!("Invalid UUID for proposal-id: {}", proposal_id_str))?;
        let rationale = get_optional_string(verb_call, "rationale");

        // 1. Look up the proposal snapshot
        let proposal = SnapshotStore::get_by_id(pool, proposal_id)
            .await?
            .ok_or_else(|| {
                anyhow!(
                    "Phrase proposal snapshot {} not found",
                    proposal_id
                )
            })?;

        // Verify it's a phrase_mapping
        if proposal.object_type != ObjectType::PhraseMapping {
            return Err(anyhow!(
                "Snapshot {} is not a phrase_mapping (found {:?})",
                proposal_id,
                proposal.object_type
            ));
        }

        // 2. Extract and update definition with published state
        let mut definition: serde_json::Value = proposal.definition.clone();
        definition["state"] = serde_json::Value::String("published".to_string());
        if let Some(ref r) = rationale {
            definition["approval_rationale"] = serde_json::Value::String(r.clone());
        }

        // 3. Create superseding published snapshot in a transaction
        let mut tx = pool.begin().await?;
        let meta = next_phrase_meta(
            Some(&proposal),
            proposal.object_id,
            ctx.audit_user.as_deref().unwrap_or("phrase.approve"),
            ChangeType::NonBreaking,
            rationale,
            SnapshotStatus::Active,
        );
        let published_snapshot_id =
            publish_phrase_snapshot_in_tx(&mut tx, &meta, &definition).await?;
        tx.commit().await?;

        // 4. Read back the phrase_bank entry (created by materialization trigger)
        let phrase = definition
            .get("phrase")
            .and_then(|v| v.as_str())
            .unwrap_or("")
            .to_string();
        let verb_fqn = definition
            .get("verb_fqn")
            .and_then(|v| v.as_str())
            .unwrap_or("")
            .to_string();

        let result = PhraseApproveResult {
            published_snapshot_id,
            phrase,
            verb_fqn,
            status: "published".to_string(),
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
///
/// Creates a superseding snapshot with state=rejected and records the rejection
/// reason in the definition. The predecessor snapshot is closed (effective_until set).
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
        verb_call: &VerbCall,
        ctx: &mut ExecutionContext,
        pool: &PgPool,
    ) -> Result<ExecutionResult> {
        let proposal_id_str = get_required_string(verb_call, "proposal-id")?;
        let proposal_id: Uuid = proposal_id_str
            .parse()
            .map_err(|_| anyhow!("Invalid UUID for proposal-id: {}", proposal_id_str))?;
        let reason = get_required_string(verb_call, "reason")?;

        // 1. Look up the proposal snapshot
        let proposal = SnapshotStore::get_by_id(pool, proposal_id)
            .await?
            .ok_or_else(|| anyhow!("Phrase proposal snapshot {} not found", proposal_id))?;

        if proposal.object_type != ObjectType::PhraseMapping {
            return Err(anyhow!(
                "Snapshot {} is not a phrase_mapping (found {:?})",
                proposal_id,
                proposal.object_type
            ));
        }

        // 2. Update definition with rejected state
        let mut definition: serde_json::Value = proposal.definition.clone();
        definition["state"] = serde_json::Value::String("rejected".to_string());
        definition["rejection_reason"] = serde_json::Value::String(reason.clone());

        // 3. Create superseding snapshot marking rejection
        let mut tx = pool.begin().await?;
        let meta = next_phrase_meta(
            Some(&proposal),
            proposal.object_id,
            ctx.audit_user.as_deref().unwrap_or("phrase.reject"),
            ChangeType::NonBreaking,
            Some(reason.clone()),
            SnapshotStatus::Deprecated,
        );
        let rejected_snapshot_id =
            publish_phrase_snapshot_in_tx(&mut tx, &meta, &definition).await?;
        tx.commit().await?;

        let result = PhraseLifecycleResult {
            snapshot_id: rejected_snapshot_id,
            state: "rejected".to_string(),
            reason: Some(reason),
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
///
/// Creates a superseding snapshot with state=deferred. Unlike rejection,
/// deferred proposals can later be resumed via `phrase.review-proposals`.
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
        verb_call: &VerbCall,
        ctx: &mut ExecutionContext,
        pool: &PgPool,
    ) -> Result<ExecutionResult> {
        let proposal_id_str = get_required_string(verb_call, "proposal-id")?;
        let proposal_id: Uuid = proposal_id_str
            .parse()
            .map_err(|_| anyhow!("Invalid UUID for proposal-id: {}", proposal_id_str))?;
        let reason = get_optional_string(verb_call, "reason");

        // 1. Look up the proposal snapshot
        let proposal = SnapshotStore::get_by_id(pool, proposal_id)
            .await?
            .ok_or_else(|| anyhow!("Phrase proposal snapshot {} not found", proposal_id))?;

        if proposal.object_type != ObjectType::PhraseMapping {
            return Err(anyhow!(
                "Snapshot {} is not a phrase_mapping (found {:?})",
                proposal_id,
                proposal.object_type
            ));
        }

        // 2. Update definition with deferred state
        let mut definition: serde_json::Value = proposal.definition.clone();
        definition["state"] = serde_json::Value::String("deferred".to_string());
        if let Some(ref r) = reason {
            definition["deferral_reason"] = serde_json::Value::String(r.clone());
        }

        // 3. Create superseding snapshot — status stays Active so it can be resumed
        let mut tx = pool.begin().await?;
        let meta = next_phrase_meta(
            Some(&proposal),
            proposal.object_id,
            ctx.audit_user.as_deref().unwrap_or("phrase.defer"),
            ChangeType::NonBreaking,
            reason.clone(),
            SnapshotStatus::Active,
        );
        let deferred_snapshot_id =
            publish_phrase_snapshot_in_tx(&mut tx, &meta, &definition).await?;
        tx.commit().await?;

        let result = PhraseLifecycleResult {
            snapshot_id: deferred_snapshot_id,
            state: "deferred".to_string(),
            reason,
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
