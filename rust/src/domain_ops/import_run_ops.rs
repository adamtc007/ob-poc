//! Import Run Operations
//!
//! Manages graph import provenance: begin, complete, and supersede import runs.
//! Import runs track the source and scope of structural graph data imports,
//! enabling rollback and correction replay.
//!
//! ## Supersession Cascade (Phase 4.2)
//!
//! When an import run is superseded, the following downstream re-derivation occurs:
//! 1. Soft-end all edges from the run (existing)
//! 2. Record corrections in `kyc.research_corrections` for linked decisions
//! 3. Mark UBO determination runs as stale (nullify coverage_snapshot)
//! 4. Mark outreach plans linked to stale determination runs as DRAFT
//! 5. Insert stale tollgate evaluations for affected cases
//!
//! This ensures that downstream computations (UBO chains, coverage, outreach,
//! tollgate checks) are re-derived after the graph data changes.

use anyhow::Result;
use async_trait::async_trait;
use ob_poc_macros::register_custom_op;
use serde::{Deserialize, Serialize};
use uuid::Uuid;

use crate::dsl_v2::ast::VerbCall;
use crate::dsl_v2::executor::{ExecutionContext, ExecutionResult};

use super::helpers::{extract_string, extract_string_opt, extract_uuid, extract_uuid_opt};
use super::CustomOperation;

#[cfg(feature = "database")]
use sqlx::PgPool;

// ============================================================================
// Types
// ============================================================================

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ImportRunBeginResult {
    pub run_id: Uuid,
    pub run_kind: String,
    pub source: String,
    pub scope_root_entity_id: Uuid,
    pub as_of: String,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ImportRunCompleteResult {
    pub run_id: Uuid,
    pub status: String,
    pub entities_created: i32,
    pub entities_updated: i32,
    pub edges_created: i32,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ImportRunSupersedeResult {
    pub run_id: Uuid,
    pub superseded_by: Option<Uuid>,
    pub edges_ended: i64,
    pub cases_affected: i64,
    pub corrections_logged: i64,
    pub ubo_runs_staled: i64,
    pub outreach_plans_reset: i64,
    pub tollgates_staled: i64,
}

/// Row returned when querying linked cases with their decision IDs.
#[cfg(feature = "database")]
#[derive(Debug, Clone)]
struct LinkedCaseRow {
    case_id: Uuid,
    decision_id: Option<Uuid>,
}

// ============================================================================
// ImportRunBeginOp
// ============================================================================

#[register_custom_op]
pub struct ImportRunBeginOp;

#[async_trait]
impl CustomOperation for ImportRunBeginOp {
    fn domain(&self) -> &'static str {
        "research.import-run"
    }

    fn verb(&self) -> &'static str {
        "begin"
    }

    fn rationale(&self) -> &'static str {
        "Creates import run with optional case linkage — multi-table insert"
    }

    #[cfg(feature = "database")]
    async fn execute(
        &self,
        verb_call: &VerbCall,
        ctx: &mut ExecutionContext,
        pool: &PgPool,
    ) -> Result<ExecutionResult> {
        let scope_root = extract_uuid(verb_call, ctx, "scope-root-entity-id")?;
        let source = extract_string(verb_call, "source")?;
        let run_kind = extract_string_opt(verb_call, "run-kind")
            .unwrap_or_else(|| "SKELETON_BUILD".to_string());
        let source_ref = extract_string_opt(verb_call, "source-ref");
        let source_query = extract_string_opt(verb_call, "source-query");
        let as_of = extract_string_opt(verb_call, "as-of");
        let case_id = extract_uuid_opt(verb_call, ctx, "case-id");
        let decision_id = extract_uuid_opt(verb_call, ctx, "decision-id");

        // Check for idempotent match (same scope_root + source + source_ref + as_of + ACTIVE)
        let existing: Option<(Uuid,)> = sqlx::query_as(
            r#"SELECT run_id FROM "ob-poc".graph_import_runs
               WHERE scope_root_entity_id = $1 AND source = $2
                 AND COALESCE(source_ref, '') = COALESCE($3, '')
                 AND status = 'ACTIVE' AND run_kind = $4
                 AND as_of = COALESCE($5::date, CURRENT_DATE)
               LIMIT 1"#,
        )
        .bind(scope_root)
        .bind(&source)
        .bind(&source_ref)
        .bind(&run_kind)
        .bind(&as_of)
        .fetch_optional(pool)
        .await?;

        if let Some((run_id,)) = existing {
            // Invariant: any case-id provided MUST appear in case_import_runs
            // regardless of whether the run was newly created or already existed.
            if let Some(cid) = case_id {
                sqlx::query(
                    r#"INSERT INTO kyc.case_import_runs (case_id, run_id, decision_id)
                       VALUES ($1, $2, $3)
                       ON CONFLICT DO NOTHING"#,
                )
                .bind(cid)
                .bind(run_id)
                .bind(decision_id)
                .execute(pool)
                .await?;
            }

            let result = ImportRunBeginResult {
                run_id,
                run_kind,
                source,
                scope_root_entity_id: scope_root,
                as_of: as_of.clone().unwrap_or_else(|| "today".to_string()),
            };
            return Ok(ExecutionResult::Record(serde_json::to_value(result)?));
        }

        let run_id: Uuid = sqlx::query_scalar(
            r#"INSERT INTO "ob-poc".graph_import_runs
               (scope_root_entity_id, source, run_kind, source_ref, source_query, as_of)
               VALUES ($1, $2, $3, $4, $5, COALESCE($6::date, CURRENT_DATE))
               RETURNING run_id"#,
        )
        .bind(scope_root)
        .bind(&source)
        .bind(&run_kind)
        .bind(&source_ref)
        .bind(&source_query)
        .bind(&as_of)
        .fetch_one(pool)
        .await?;

        // Link to case if provided
        if let Some(cid) = case_id {
            sqlx::query(
                r#"INSERT INTO kyc.case_import_runs (case_id, run_id, decision_id)
                   VALUES ($1, $2, $3)
                   ON CONFLICT DO NOTHING"#,
            )
            .bind(cid)
            .bind(run_id)
            .bind(decision_id)
            .execute(pool)
            .await?;
        }

        let result = ImportRunBeginResult {
            run_id,
            run_kind,
            source,
            scope_root_entity_id: scope_root,
            as_of: as_of.unwrap_or_else(|| "today".to_string()),
        };
        Ok(ExecutionResult::Record(serde_json::to_value(result)?))
    }
}

// ============================================================================
// ImportRunCompleteOp
// ============================================================================

#[register_custom_op]
pub struct ImportRunCompleteOp;

#[async_trait]
impl CustomOperation for ImportRunCompleteOp {
    fn domain(&self) -> &'static str {
        "research.import-run"
    }

    fn verb(&self) -> &'static str {
        "complete"
    }

    fn rationale(&self) -> &'static str {
        "Updates import run status and counts after import completes"
    }

    #[cfg(feature = "database")]
    async fn execute(
        &self,
        verb_call: &VerbCall,
        ctx: &mut ExecutionContext,
        pool: &PgPool,
    ) -> Result<ExecutionResult> {
        let run_id = extract_uuid(verb_call, ctx, "run-id")?;
        let status =
            extract_string_opt(verb_call, "status").unwrap_or_else(|| "ACTIVE".to_string());

        // Count entities and edges created by this run
        let edge_count: (i64,) = sqlx::query_as(
            r#"SELECT count(*) FROM "ob-poc".entity_relationships
               WHERE import_run_id = $1"#,
        )
        .bind(run_id)
        .fetch_one(pool)
        .await?;

        sqlx::query(
            r#"UPDATE "ob-poc".graph_import_runs
               SET status = $2, edges_created = $3
               WHERE run_id = $1"#,
        )
        .bind(run_id)
        .bind(&status)
        .bind(edge_count.0 as i32)
        .execute(pool)
        .await?;

        let result = ImportRunCompleteResult {
            run_id,
            status,
            entities_created: 0,
            entities_updated: 0,
            edges_created: edge_count.0 as i32,
        };
        Ok(ExecutionResult::Record(serde_json::to_value(result)?))
    }
}

// ============================================================================
// ImportRunSupersedeOp
// ============================================================================

#[register_custom_op]
pub struct ImportRunSupersedeOp;

#[async_trait]
impl CustomOperation for ImportRunSupersedeOp {
    fn domain(&self) -> &'static str {
        "research.import-run"
    }

    fn verb(&self) -> &'static str {
        "supersede"
    }

    fn rationale(&self) -> &'static str {
        "Supersedes an import run: soft-ends edges, logs corrections, triggers re-derivation cascade"
    }

    #[cfg(feature = "database")]
    async fn execute(
        &self,
        verb_call: &VerbCall,
        ctx: &mut ExecutionContext,
        pool: &PgPool,
    ) -> Result<ExecutionResult> {
        let run_id = extract_uuid(verb_call, ctx, "run-id")?;
        let reason = extract_string(verb_call, "reason")?;
        let superseded_by = extract_uuid_opt(verb_call, ctx, "superseded-by");

        // Resolve audit user for corrected_by (falls back to a system UUID)
        let corrected_by: Uuid = ctx
            .audit_user
            .as_ref()
            .and_then(|s| s.parse().ok())
            .unwrap_or_else(Uuid::nil);

        // ====================================================================
        // Step 1: Mark the run as superseded
        // ====================================================================
        sqlx::query(
            r#"UPDATE "ob-poc".graph_import_runs
               SET status = 'SUPERSEDED', superseded_by = $2, superseded_reason = $3
               WHERE run_id = $1 AND status = 'ACTIVE'"#,
        )
        .bind(run_id)
        .bind(superseded_by)
        .bind(&reason)
        .execute(pool)
        .await?;

        // ====================================================================
        // Step 2: Soft-end all edges from this run
        // ====================================================================
        let edges_ended = sqlx::query(
            r#"UPDATE "ob-poc".entity_relationships
               SET effective_to = CURRENT_DATE
               WHERE import_run_id = $1 AND effective_to IS NULL"#,
        )
        .bind(run_id)
        .execute(pool)
        .await?
        .rows_affected() as i64;

        // ====================================================================
        // Step 3: Find linked cases (with their decision IDs for corrections)
        // ====================================================================
        let linked_cases: Vec<LinkedCaseRow> = sqlx::query_as::<_, (Uuid, Option<Uuid>)>(
            r#"SELECT case_id, decision_id FROM kyc.case_import_runs WHERE run_id = $1"#,
        )
        .bind(run_id)
        .fetch_all(pool)
        .await?
        .into_iter()
        .map(|(case_id, decision_id)| LinkedCaseRow {
            case_id,
            decision_id,
        })
        .collect();

        let cases_affected = linked_cases.len() as i64;

        // ====================================================================
        // Step 4: Record corrections in research_corrections for linked decisions
        //
        // research_corrections requires original_decision_id (NOT NULL) and
        // corrected_by (NOT NULL). We can only insert when a decision_id exists
        // in the case_import_runs linkage.
        // ====================================================================
        let mut corrections_logged: i64 = 0;
        for linked in &linked_cases {
            if let Some(decision_id) = linked.decision_id {
                let correction_id = Uuid::new_v4();
                let inserted = sqlx::query(
                    r#"INSERT INTO kyc.research_corrections
                       (correction_id, original_decision_id, correction_type,
                        correction_reason, corrected_by)
                       VALUES ($1, $2, 'STALE_DATA', $3, $4)"#,
                )
                .bind(correction_id)
                .bind(decision_id)
                .bind(format!(
                    "Import run {} superseded (case {}): {}",
                    run_id, linked.case_id, reason
                ))
                .bind(corrected_by)
                .execute(pool)
                .await;

                if inserted.is_ok() {
                    corrections_logged += 1;
                }
            }
        }

        // ====================================================================
        // Step 5: Mark UBO determination runs as stale for affected cases
        //
        // ubo_determination_runs has no status column — it stores immutable
        // snapshots. We mark staleness by nullifying coverage_snapshot so that
        // downstream consumers know re-computation is needed. The chains_snapshot
        // is preserved for audit/diff purposes.
        // ====================================================================
        let case_ids: Vec<Uuid> = linked_cases.iter().map(|lc| lc.case_id).collect();
        let ubo_runs_staled: i64 = if !case_ids.is_empty() {
            sqlx::query(
                r#"UPDATE kyc.ubo_determination_runs
                   SET coverage_snapshot = NULL
                   WHERE case_id = ANY($1)
                     AND coverage_snapshot IS NOT NULL"#,
            )
            .bind(&case_ids)
            .execute(pool)
            .await?
            .rows_affected() as i64
        } else {
            0
        };

        // ====================================================================
        // Step 6: Reset outreach plans linked to stale determination runs
        //
        // Any DRAFT or APPROVED outreach plans whose determination_run_id
        // points to an affected case should be reset to DRAFT so they get
        // regenerated after re-computation.
        // ====================================================================
        let outreach_plans_reset: i64 = if !case_ids.is_empty() {
            sqlx::query(
                r#"UPDATE kyc.outreach_plans
                   SET status = 'DRAFT'
                   WHERE case_id = ANY($1)
                     AND status IN ('APPROVED', 'SENT')
                     AND determination_run_id IN (
                         SELECT run_id FROM kyc.ubo_determination_runs
                         WHERE case_id = ANY($1)
                     )"#,
            )
            .bind(&case_ids)
            .execute(pool)
            .await?
            .rows_affected() as i64
        } else {
            0
        };

        // ====================================================================
        // Step 7: Mark existing tollgate evaluations as stale
        //
        // Insert a new evaluation for each affected case with passed=false
        // and a detail payload explaining the supersession. This preserves
        // the old evaluation for audit while signalling that re-evaluation
        // is required.
        // ====================================================================
        let mut tollgates_staled: i64 = 0;
        for case_id in &case_ids {
            // Find tollgates that previously passed for this case
            let passed_tollgates: Vec<(String,)> = sqlx::query_as(
                r#"SELECT DISTINCT tollgate_id
                   FROM kyc.tollgate_evaluations
                   WHERE case_id = $1 AND passed = true"#,
            )
            .bind(case_id)
            .fetch_all(pool)
            .await?;

            for (tollgate_id,) in &passed_tollgates {
                let eval_id = Uuid::new_v4();
                let detail = serde_json::json!({
                    "stale_reason": "import_run_superseded",
                    "superseded_run_id": run_id,
                    "superseded_by": superseded_by,
                    "reason": reason,
                    "requires_recomputation": true
                });

                let inserted = sqlx::query(
                    r#"INSERT INTO kyc.tollgate_evaluations
                       (evaluation_id, case_id, tollgate_id, passed,
                        evaluation_detail, config_version)
                       VALUES ($1, $2, $3, false, $4, 'supersession')"#,
                )
                .bind(eval_id)
                .bind(case_id)
                .bind(tollgate_id)
                .bind(detail)
                .execute(pool)
                .await;

                if inserted.is_ok() {
                    tollgates_staled += 1;
                }
            }
        }

        let result = ImportRunSupersedeResult {
            run_id,
            superseded_by,
            edges_ended,
            cases_affected,
            corrections_logged,
            ubo_runs_staled,
            outreach_plans_reset,
            tollgates_staled,
        };
        Ok(ExecutionResult::Record(serde_json::to_value(result)?))
    }
}
