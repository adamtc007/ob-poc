//! Import-run domain verbs (3 plugin verbs) — SemOS-side YAML-first
//! re-implementation of the plugin subset of
//! `rust/config/verbs/research/import-run.yaml`.
//!
//! Manages graph import provenance:
//! - `research.import-run.begin` — create or reuse an ACTIVE run
//!   scoped by (entity, source, source_ref, run_kind, as_of).
//!   Optionally links a case_id + decision_id via
//!   `case_import_runs`.
//! - `research.import-run.complete` — flip status to terminal,
//!   count live edges for the run.
//! - `research.import-run.supersede` — soft-end edges + propagate
//!   the invalidation cascade: log `research_corrections`, stale
//!   `ubo_determination_runs`, reset `outreach_plans` to DRAFT,
//!   and insert supersession tollgate evaluations.

use anyhow::Result;
use async_trait::async_trait;
use serde::{Deserialize, Serialize};
use serde_json::Value;
use uuid::Uuid;

use dsl_runtime::domain_ops::helpers::{
    json_extract_string, json_extract_string_opt, json_extract_uuid, json_extract_uuid_opt,
};
use dsl_runtime::tx::TransactionScope;
use dsl_runtime::{VerbExecutionContext, VerbExecutionOutcome};

use super::SemOsVerbOp;

#[derive(Debug, Clone, Serialize, Deserialize)]
struct ImportRunBeginResult {
    run_id: Uuid,
    run_kind: String,
    source: String,
    scope_root_entity_id: Uuid,
    as_of: String,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
struct ImportRunCompleteResult {
    run_id: Uuid,
    status: String,
    entities_created: i32,
    entities_updated: i32,
    edges_created: i32,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
struct ImportRunSupersedeResult {
    run_id: Uuid,
    superseded_by: Option<Uuid>,
    edges_ended: i64,
    cases_affected: i64,
    corrections_logged: i64,
    ubo_runs_staled: i64,
    outreach_plans_reset: i64,
    tollgates_staled: i64,
}

// ── research.import-run.begin ─────────────────────────────────────────────────

pub struct Begin;

#[async_trait]
impl SemOsVerbOp for Begin {
    fn fqn(&self) -> &str {
        "research.import-run.begin"
    }
    async fn execute(
        &self,
        args: &Value,
        ctx: &mut VerbExecutionContext,
        scope: &mut dyn TransactionScope,
    ) -> Result<VerbExecutionOutcome> {
        let scope_root = json_extract_uuid(args, ctx, "scope-root-entity-id")?;
        let source = json_extract_string(args, "source")?;
        let run_kind = json_extract_string_opt(args, "run-kind")
            .unwrap_or_else(|| "SKELETON_BUILD".to_string());
        let source_ref = json_extract_string_opt(args, "source-ref");
        let source_query = json_extract_string_opt(args, "source-query");
        let as_of = json_extract_string_opt(args, "as-of");
        let case_id = json_extract_uuid_opt(args, ctx, "case-id");
        let decision_id = json_extract_uuid_opt(args, ctx, "decision-id");

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
        .fetch_optional(scope.executor())
        .await?;

        let run_id = if let Some((id,)) = existing {
            id
        } else {
            let row: (Uuid,) = sqlx::query_as(
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
            .fetch_one(scope.executor())
            .await?;
            row.0
        };

        if let Some(cid) = case_id {
            sqlx::query(
                r#"INSERT INTO "ob-poc".case_import_runs (case_id, run_id, decision_id)
                   VALUES ($1, $2, $3)
                   ON CONFLICT DO NOTHING"#,
            )
            .bind(cid)
            .bind(run_id)
            .bind(decision_id)
            .execute(scope.executor())
            .await?;
        }

        let result = ImportRunBeginResult {
            run_id,
            run_kind,
            source,
            scope_root_entity_id: scope_root,
            as_of: as_of.unwrap_or_else(|| "today".to_string()),
        };
        Ok(VerbExecutionOutcome::Record(serde_json::to_value(result)?))
    }
}

// ── research.import-run.complete ──────────────────────────────────────────────

pub struct Complete;

#[async_trait]
impl SemOsVerbOp for Complete {
    fn fqn(&self) -> &str {
        "research.import-run.complete"
    }
    async fn execute(
        &self,
        args: &Value,
        ctx: &mut VerbExecutionContext,
        scope: &mut dyn TransactionScope,
    ) -> Result<VerbExecutionOutcome> {
        let run_id = json_extract_uuid(args, ctx, "run-id")?;
        let status =
            json_extract_string_opt(args, "status").unwrap_or_else(|| "ACTIVE".to_string());

        let (edge_count,): (i64,) = sqlx::query_as(
            r#"SELECT count(*) FROM "ob-poc".entity_relationships
               WHERE import_run_id = $1"#,
        )
        .bind(run_id)
        .fetch_one(scope.executor())
        .await?;

        sqlx::query(
            r#"UPDATE "ob-poc".graph_import_runs
               SET status = $2, edges_created = $3
               WHERE run_id = $1"#,
        )
        .bind(run_id)
        .bind(&status)
        .bind(edge_count as i32)
        .execute(scope.executor())
        .await?;

        let result = ImportRunCompleteResult {
            run_id,
            status,
            entities_created: 0,
            entities_updated: 0,
            edges_created: edge_count as i32,
        };
        Ok(VerbExecutionOutcome::Record(serde_json::to_value(result)?))
    }
}

// ── research.import-run.supersede ─────────────────────────────────────────────

pub struct Supersede;

#[async_trait]
impl SemOsVerbOp for Supersede {
    fn fqn(&self) -> &str {
        "research.import-run.supersede"
    }
    async fn execute(
        &self,
        args: &Value,
        ctx: &mut VerbExecutionContext,
        scope: &mut dyn TransactionScope,
    ) -> Result<VerbExecutionOutcome> {
        let run_id = json_extract_uuid(args, ctx, "run-id")?;
        let reason = json_extract_string(args, "reason")?;
        let superseded_by = json_extract_uuid_opt(args, ctx, "superseded-by");
        let corrected_by: Uuid = ctx
            .extensions
            .as_object()
            .and_then(|o| o.get("audit_user"))
            .and_then(|v| v.as_str())
            .and_then(|s| s.parse().ok())
            .unwrap_or_else(Uuid::nil);

        sqlx::query(
            r#"UPDATE "ob-poc".graph_import_runs
               SET status = 'SUPERSEDED', superseded_by = $2, superseded_reason = $3
               WHERE run_id = $1 AND status = 'ACTIVE'"#,
        )
        .bind(run_id)
        .bind(superseded_by)
        .bind(&reason)
        .execute(scope.executor())
        .await?;

        let edges_ended = sqlx::query(
            r#"UPDATE "ob-poc".entity_relationships
               SET effective_to = CURRENT_DATE
               WHERE import_run_id = $1 AND effective_to IS NULL"#,
        )
        .bind(run_id)
        .execute(scope.executor())
        .await?
        .rows_affected() as i64;

        let linked_cases: Vec<(Uuid, Option<Uuid>)> = sqlx::query_as(
            r#"SELECT case_id, decision_id FROM "ob-poc".case_import_runs WHERE run_id = $1"#,
        )
        .bind(run_id)
        .fetch_all(scope.executor())
        .await?;

        let cases_affected = linked_cases.len() as i64;

        let mut corrections_logged: i64 = 0;
        for (case_id, decision_id) in &linked_cases {
            if let Some(decision_id) = decision_id {
                let inserted = sqlx::query(
                    r#"INSERT INTO "ob-poc".research_corrections
                       (correction_id, original_decision_id, correction_type,
                        correction_reason, corrected_by)
                       VALUES ($1, $2, 'STALE_DATA', $3, $4)"#,
                )
                .bind(Uuid::new_v4())
                .bind(decision_id)
                .bind(format!(
                    "Import run {} superseded (case {}): {}",
                    run_id, case_id, reason
                ))
                .bind(corrected_by)
                .execute(scope.executor())
                .await;

                if inserted.is_ok() {
                    corrections_logged += 1;
                }
            }
        }

        let case_ids: Vec<Uuid> = linked_cases.iter().map(|(c, _)| *c).collect();

        let ubo_runs_staled: i64 = if !case_ids.is_empty() {
            sqlx::query(
                r#"UPDATE "ob-poc".ubo_determination_runs
                   SET coverage_snapshot = NULL
                   WHERE case_id = ANY($1)
                     AND coverage_snapshot IS NOT NULL"#,
            )
            .bind(&case_ids)
            .execute(scope.executor())
            .await?
            .rows_affected() as i64
        } else {
            0
        };

        let outreach_plans_reset: i64 = if !case_ids.is_empty() {
            sqlx::query(
                r#"UPDATE "ob-poc".outreach_plans
                   SET status = 'DRAFT'
                   WHERE case_id = ANY($1)
                     AND status IN ('APPROVED', 'SENT')
                     AND determination_run_id IN (
                         SELECT run_id FROM "ob-poc".ubo_determination_runs
                         WHERE case_id = ANY($1)
                     )"#,
            )
            .bind(&case_ids)
            .execute(scope.executor())
            .await?
            .rows_affected() as i64
        } else {
            0
        };

        let mut tollgates_staled: i64 = 0;
        for case_id in &case_ids {
            let passed_tollgates: Vec<(String,)> = sqlx::query_as(
                r#"SELECT DISTINCT tollgate_id
                   FROM "ob-poc".tollgate_evaluations
                   WHERE case_id = $1 AND passed = true"#,
            )
            .bind(case_id)
            .fetch_all(scope.executor())
            .await?;

            for (tollgate_id,) in &passed_tollgates {
                let detail = serde_json::json!({
                    "stale_reason": "import_run_superseded",
                    "superseded_run_id": run_id,
                    "superseded_by": superseded_by,
                    "reason": reason,
                    "requires_recomputation": true
                });

                let inserted = sqlx::query(
                    r#"INSERT INTO "ob-poc".tollgate_evaluations
                       (evaluation_id, case_id, tollgate_id, passed,
                        evaluation_detail, config_version)
                       VALUES ($1, $2, $3, false, $4, 'supersession')"#,
                )
                .bind(Uuid::new_v4())
                .bind(case_id)
                .bind(tollgate_id)
                .bind(detail)
                .execute(scope.executor())
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
        Ok(VerbExecutionOutcome::Record(serde_json::to_value(result)?))
    }
}
