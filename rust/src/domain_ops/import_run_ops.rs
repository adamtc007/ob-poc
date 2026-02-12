//! Import Run Operations
//!
//! Manages graph import provenance: begin, complete, and supersede import runs.
//! Import runs track the source and scope of structural graph data imports,
//! enabling rollback and correction replay.

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
        let case_id = extract_uuid_opt(verb_call, ctx, "case-id");
        let decision_id = extract_uuid_opt(verb_call, ctx, "decision-id");

        // Check for idempotent match (same scope_root + source + source_ref + ACTIVE)
        let existing: Option<(Uuid,)> = sqlx::query_as(
            r#"SELECT run_id FROM "ob-poc".graph_import_runs
               WHERE scope_root_entity_id = $1 AND source = $2
                 AND COALESCE(source_ref, '') = COALESCE($3, '')
                 AND status = 'ACTIVE' AND run_kind = $4
               LIMIT 1"#,
        )
        .bind(scope_root)
        .bind(&source)
        .bind(&source_ref)
        .bind(&run_kind)
        .fetch_optional(pool)
        .await?;

        if let Some((run_id,)) = existing {
            let result = ImportRunBeginResult {
                run_id,
                run_kind,
                source,
                scope_root_entity_id: scope_root,
            };
            return Ok(ExecutionResult::Record(serde_json::to_value(result)?));
        }

        let run_id: Uuid = sqlx::query_scalar(
            r#"INSERT INTO "ob-poc".graph_import_runs
               (scope_root_entity_id, source, run_kind, source_ref, source_query)
               VALUES ($1, $2, $3, $4, $5)
               RETURNING run_id"#,
        )
        .bind(scope_root)
        .bind(&source)
        .bind(&run_kind)
        .bind(&source_ref)
        .bind(&source_query)
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
        "Supersedes an import run: soft-ends all edges, logs corrections for linked cases"
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

        // Mark the run as superseded
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

        // Soft-end all edges from this run
        let edges_ended = sqlx::query(
            r#"UPDATE "ob-poc".entity_relationships
               SET effective_to = CURRENT_DATE
               WHERE import_run_id = $1 AND effective_to IS NULL"#,
        )
        .bind(run_id)
        .execute(pool)
        .await?
        .rows_affected() as i64;

        // Log corrections for linked cases
        let cases_affected = sqlx::query_scalar::<_, i64>(
            r#"SELECT count(*) FROM kyc.case_import_runs WHERE run_id = $1"#,
        )
        .bind(run_id)
        .fetch_one(pool)
        .await?;

        // Record correction in research audit trail for each linked case
        let linked_cases: Vec<(Uuid,)> =
            sqlx::query_as(r#"SELECT case_id FROM kyc.case_import_runs WHERE run_id = $1"#)
                .bind(run_id)
                .fetch_all(pool)
                .await?;

        for (case_id,) in &linked_cases {
            sqlx::query(
                r#"INSERT INTO kyc.research_corrections
                   (case_id, correction_type, description, created_at)
                   VALUES ($1, 'IMPORT_SUPERSEDED', $2, NOW())
                   ON CONFLICT DO NOTHING"#,
            )
            .bind(case_id)
            .bind(format!("Import run {} superseded: {}", run_id, reason))
            .execute(pool)
            .await
            .ok(); // Best-effort — table may not exist yet
        }

        let result = ImportRunSupersedeResult {
            run_id,
            superseded_by,
            edges_ended,
            cases_affected,
        };
        Ok(ExecutionResult::Record(serde_json::to_value(result)?))
    }
}
