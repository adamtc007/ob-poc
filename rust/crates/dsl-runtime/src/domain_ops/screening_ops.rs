//! Screening custom operations
//!
//! Operations for PEP, sanctions, and adverse media screening that require
//! external service API calls.

use anyhow::Result;
use async_trait::async_trait;
use dsl_runtime_macros::register_custom_op;
use sqlx::PgPool;

use crate::custom_op::CustomOperation;
use crate::execution::{VerbExecutionContext, VerbExecutionOutcome};

/// PEP (Politically Exposed Person) screening (Idempotent)
///
/// Rationale: Requires external PEP database API call and result processing.
/// Idempotency: Returns existing pending PEP screening for same entity if exists.
#[register_custom_op]
pub struct ScreeningPepOp;

#[async_trait]
impl CustomOperation for ScreeningPepOp {
    fn domain(&self) -> &'static str {
        "screening"
    }
    fn verb(&self) -> &'static str {
        "pep"
    }
    fn rationale(&self) -> &'static str {
        "Requires external PEP screening service API call"
    }

    async fn execute_json(
        &self,
        args: &serde_json::Value,
        ctx: &mut VerbExecutionContext,
        pool: &PgPool,
    ) -> Result<VerbExecutionOutcome> {
        use crate::domain_ops::helpers::json_extract_uuid;
        use uuid::Uuid;

        let entity_id: Uuid = json_extract_uuid(args, ctx, "entity-id")?;

        let workstream = sqlx::query!(
            r#"SELECT w.workstream_id FROM "ob-poc".entity_workstreams w
               JOIN "ob-poc".cases c ON c.case_id = w.case_id
               WHERE w.entity_id = $1 AND w.status NOT IN ('COMPLETE', 'BLOCKED')
               ORDER BY w.created_at DESC
               LIMIT 1"#,
            entity_id
        )
        .fetch_optional(pool)
        .await?;

        let workstream_id = match workstream {
            Some(row) => row.workstream_id,
            None => {
                return Err(anyhow::anyhow!(
                    "No active workstream for entity. Use case-screening.initiate instead."
                ));
            }
        };

        let existing = sqlx::query!(
            r#"SELECT screening_id FROM "ob-poc".screenings
               WHERE workstream_id = $1 AND screening_type = 'PEP' AND status = 'PENDING'
               LIMIT 1"#,
            workstream_id
        )
        .fetch_optional(pool)
        .await?;

        if let Some(row) = existing {
            ctx.bind("screening", row.screening_id);
            return Ok(VerbExecutionOutcome::Uuid(row.screening_id));
        }

        let screening_id = Uuid::new_v4();

        sqlx::query!(
            r#"INSERT INTO "ob-poc".screenings
               (screening_id, workstream_id, screening_type, status)
               VALUES ($1, $2, 'PEP', 'PENDING')"#,
            screening_id,
            workstream_id
        )
        .execute(pool)
        .await?;

        ctx.bind("screening", screening_id);

        Ok(VerbExecutionOutcome::Uuid(screening_id))
    }

    fn is_migrated(&self) -> bool {
        true
    }
}

/// Sanctions screening (Idempotent)
///
/// Rationale: Requires external sanctions database API call and result processing.
/// Idempotency: Returns existing pending sanctions screening for same entity if exists.
#[register_custom_op]
pub struct ScreeningSanctionsOp;

#[async_trait]
impl CustomOperation for ScreeningSanctionsOp {
    fn domain(&self) -> &'static str {
        "screening"
    }
    fn verb(&self) -> &'static str {
        "sanctions"
    }
    fn rationale(&self) -> &'static str {
        "Requires external sanctions screening service API call"
    }

    async fn execute_json(
        &self,
        args: &serde_json::Value,
        ctx: &mut VerbExecutionContext,
        pool: &PgPool,
    ) -> Result<VerbExecutionOutcome> {
        use crate::domain_ops::helpers::json_extract_uuid;
        use uuid::Uuid;

        let entity_id: Uuid = json_extract_uuid(args, ctx, "entity-id")?;

        let workstream = sqlx::query!(
            r#"SELECT w.workstream_id FROM "ob-poc".entity_workstreams w
               JOIN "ob-poc".cases c ON c.case_id = w.case_id
               WHERE w.entity_id = $1 AND w.status NOT IN ('COMPLETE', 'BLOCKED')
               ORDER BY w.created_at DESC
               LIMIT 1"#,
            entity_id
        )
        .fetch_optional(pool)
        .await?;

        let workstream_id = match workstream {
            Some(row) => row.workstream_id,
            None => {
                return Err(anyhow::anyhow!(
                    "No active workstream for entity. Use case-screening.initiate instead."
                ));
            }
        };

        let existing = sqlx::query!(
            r#"SELECT screening_id FROM "ob-poc".screenings
               WHERE workstream_id = $1 AND screening_type = 'SANCTIONS' AND status = 'PENDING'
               LIMIT 1"#,
            workstream_id
        )
        .fetch_optional(pool)
        .await?;

        if let Some(row) = existing {
            ctx.bind("screening", row.screening_id);
            return Ok(VerbExecutionOutcome::Uuid(row.screening_id));
        }

        let screening_id = Uuid::new_v4();

        sqlx::query!(
            r#"INSERT INTO "ob-poc".screenings
               (screening_id, workstream_id, screening_type, status)
               VALUES ($1, $2, 'SANCTIONS', 'PENDING')"#,
            screening_id,
            workstream_id
        )
        .execute(pool)
        .await?;

        ctx.bind("screening", screening_id);

        Ok(VerbExecutionOutcome::Uuid(screening_id))
    }

    fn is_migrated(&self) -> bool {
        true
    }
}

/// Adverse media screening (Not Implemented)
///
/// Rationale: Requires external adverse media API call and result processing.
/// Status: Stub - returns error indicating not implemented.
#[register_custom_op]
pub struct ScreeningAdverseMediaOp;

#[async_trait]
impl CustomOperation for ScreeningAdverseMediaOp {
    fn domain(&self) -> &'static str {
        "screening"
    }
    fn verb(&self) -> &'static str {
        "adverse-media"
    }
    fn rationale(&self) -> &'static str {
        "Requires external adverse media screening service API call"
    }

    async fn execute_json(
        &self,
        _args: &serde_json::Value,
        _ctx: &mut VerbExecutionContext,
        _pool: &PgPool,
    ) -> Result<VerbExecutionOutcome> {
        Err(anyhow::anyhow!(
            "screening.adverse-media is not yet implemented"
        ))
    }

    fn is_migrated(&self) -> bool {
        true
    }
}

/// Refresh screening coverage for all workstreams in a case.
#[register_custom_op]
pub struct ScreeningBulkRefreshOp;

#[async_trait]
impl CustomOperation for ScreeningBulkRefreshOp {
    fn domain(&self) -> &'static str {
        "screening"
    }

    fn verb(&self) -> &'static str {
        "bulk-refresh"
    }

    fn rationale(&self) -> &'static str {
        "Ensures required pending screenings exist for every workstream in a case"
    }

    async fn execute_json(
        &self,
        args: &serde_json::Value,
        ctx: &mut VerbExecutionContext,
        pool: &PgPool,
    ) -> Result<VerbExecutionOutcome> {
        use crate::domain_ops::helpers::{json_extract_string_opt, json_extract_uuid};
        use uuid::Uuid;

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
        .fetch_all(pool)
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
                .execute(pool)
                .await?;
                inserted += result.rows_affected();
            }
        }

        Ok(VerbExecutionOutcome::Affected(inserted))
    }

    fn is_migrated(&self) -> bool {
        true
    }
}
