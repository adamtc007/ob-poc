//! Screening domain verbs (4 plugin verbs) — SemOS-side YAML-first
//! re-implementation of the plugin subset of
//! `rust/config/verbs/screening.yaml`.
//!
//! - `screening.pep` / `screening.sanctions` — idempotent enqueue of
//!   a PENDING screening row against the entity's active workstream.
//!   If a PENDING row already exists for that `(workstream, type)`
//!   pair, the existing id is bound + returned (no duplicate).
//! - `screening.adverse-media` — stub until the external service
//!   integration lands.
//! - `screening.bulk-refresh` — enqueues SANCTIONS + PEP +
//!   ADVERSE_MEDIA PENDING rows for every workstream in a case,
//!   `NOT EXISTS` guard so re-runs are safe.
//!
//! Slice #10 pattern applied: `sqlx::query!` macros rewritten as
//! runtime `sqlx::query_as` / `sqlx::query` so we dodge the
//! sqlx-offline cache entirely.

use anyhow::{anyhow, Result};
use async_trait::async_trait;
use serde_json::Value;
use uuid::Uuid;

use dsl_runtime::domain_ops::helpers::{
    json_extract_string_opt, json_extract_uuid,
};
use dsl_runtime::tx::TransactionScope;
use dsl_runtime::{VerbExecutionContext, VerbExecutionOutcome};

use super::SemOsVerbOp;

/// Enqueue a PENDING screening for an entity's active workstream.
/// Returns the existing PENDING screening if one already exists,
/// otherwise inserts a new one. Binds `:screening` in ctx.
async fn enqueue_workstream_screening(
    ctx: &mut VerbExecutionContext,
    scope: &mut dyn TransactionScope,
    entity_id: Uuid,
    screening_type: &str,
) -> Result<Uuid> {
    let workstream: Option<(Uuid,)> = sqlx::query_as(
        r#"SELECT w.workstream_id FROM "ob-poc".entity_workstreams w
           JOIN "ob-poc".cases c ON c.case_id = w.case_id
           WHERE w.entity_id = $1 AND w.status NOT IN ('COMPLETE', 'BLOCKED')
           ORDER BY w.created_at DESC
           LIMIT 1"#,
    )
    .bind(entity_id)
    .fetch_optional(scope.executor())
    .await?;

    let workstream_id = workstream
        .map(|(id,)| id)
        .ok_or_else(|| anyhow!("No active workstream for entity. Use case-screening.initiate instead."))?;

    let existing: Option<(Uuid,)> = sqlx::query_as(
        r#"SELECT screening_id FROM "ob-poc".screenings
           WHERE workstream_id = $1 AND screening_type = $2 AND status = 'PENDING'
           LIMIT 1"#,
    )
    .bind(workstream_id)
    .bind(screening_type)
    .fetch_optional(scope.executor())
    .await?;

    if let Some((screening_id,)) = existing {
        ctx.bind("screening", screening_id);
        return Ok(screening_id);
    }

    let screening_id = Uuid::new_v4();
    sqlx::query(
        r#"INSERT INTO "ob-poc".screenings
           (screening_id, workstream_id, screening_type, status)
           VALUES ($1, $2, $3, 'PENDING')"#,
    )
    .bind(screening_id)
    .bind(workstream_id)
    .bind(screening_type)
    .execute(scope.executor())
    .await?;

    ctx.bind("screening", screening_id);
    Ok(screening_id)
}

// ── screening.pep ─────────────────────────────────────────────────────────────

pub struct Pep;

#[async_trait]
impl SemOsVerbOp for Pep {
    fn fqn(&self) -> &str {
        "screening.pep"
    }
    async fn execute(
        &self,
        args: &Value,
        ctx: &mut VerbExecutionContext,
        scope: &mut dyn TransactionScope,
    ) -> Result<VerbExecutionOutcome> {
        let entity_id = json_extract_uuid(args, ctx, "entity-id")?;
        let id = enqueue_workstream_screening(ctx, scope, entity_id, "PEP").await?;
        Ok(VerbExecutionOutcome::Uuid(id))
    }
}

// ── screening.sanctions ───────────────────────────────────────────────────────

pub struct Sanctions;

#[async_trait]
impl SemOsVerbOp for Sanctions {
    fn fqn(&self) -> &str {
        "screening.sanctions"
    }
    async fn execute(
        &self,
        args: &Value,
        ctx: &mut VerbExecutionContext,
        scope: &mut dyn TransactionScope,
    ) -> Result<VerbExecutionOutcome> {
        let entity_id = json_extract_uuid(args, ctx, "entity-id")?;
        let id = enqueue_workstream_screening(ctx, scope, entity_id, "SANCTIONS").await?;
        Ok(VerbExecutionOutcome::Uuid(id))
    }
}

// ── screening.adverse-media ───────────────────────────────────────────────────

pub struct AdverseMedia;

#[async_trait]
impl SemOsVerbOp for AdverseMedia {
    fn fqn(&self) -> &str {
        "screening.adverse-media"
    }
    async fn execute(
        &self,
        _args: &Value,
        _ctx: &mut VerbExecutionContext,
        _scope: &mut dyn TransactionScope,
    ) -> Result<VerbExecutionOutcome> {
        Err(anyhow!("screening.adverse-media is not yet implemented"))
    }
}

// ── screening.bulk-refresh ────────────────────────────────────────────────────

pub struct BulkRefresh;

#[async_trait]
impl SemOsVerbOp for BulkRefresh {
    fn fqn(&self) -> &str {
        "screening.bulk-refresh"
    }
    async fn execute(
        &self,
        args: &Value,
        ctx: &mut VerbExecutionContext,
        scope: &mut dyn TransactionScope,
    ) -> Result<VerbExecutionOutcome> {
        let case_id = json_extract_uuid(args, ctx, "case-id")?;
        let screening_type = json_extract_string_opt(args, "screening-type");

        let target_types: Vec<String> = match screening_type.as_deref() {
            Some(kind) => vec![kind.to_string()],
            None => vec![
                "SANCTIONS".to_string(),
                "PEP".to_string(),
                "ADVERSE_MEDIA".to_string(),
            ],
        };

        let workstream_ids: Vec<Uuid> = sqlx::query_scalar(
            r#"
            SELECT workstream_id
            FROM "ob-poc".entity_workstreams
            WHERE case_id = $1
            "#,
        )
        .bind(case_id)
        .fetch_all(scope.executor())
        .await?;

        let mut inserted = 0_u64;
        for workstream_id in workstream_ids {
            for st in &target_types {
                let screening_id = Uuid::new_v4();
                let result = sqlx::query(
                    r#"
                    INSERT INTO "ob-poc".screenings
                        (screening_id, workstream_id, screening_type, status)
                    SELECT $1, $2, $3, 'PENDING'
                    WHERE NOT EXISTS (
                        SELECT 1
                        FROM "ob-poc".screenings
                        WHERE workstream_id = $2 AND screening_type = $3
                    )
                    "#,
                )
                .bind(screening_id)
                .bind(workstream_id)
                .bind(st)
                .execute(scope.executor())
                .await?;
                inserted += result.rows_affected();
            }
        }

        Ok(VerbExecutionOutcome::Affected(inserted))
    }
}
