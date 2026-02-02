//! Screening custom operations
//!
//! Operations for PEP, sanctions, and adverse media screening that require
//! external service API calls.

use anyhow::Result;
use async_trait::async_trait;
use ob_poc_macros::register_custom_op;

use super::CustomOperation;
use crate::dsl_v2::ast::VerbCall;
use crate::dsl_v2::executor::{ExecutionContext, ExecutionResult};

#[cfg(feature = "database")]
use sqlx::PgPool;

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

    #[cfg(feature = "database")]
    async fn execute(
        &self,
        verb_call: &VerbCall,
        ctx: &mut ExecutionContext,
        pool: &PgPool,
    ) -> Result<ExecutionResult> {
        use uuid::Uuid;

        // Get entity ID
        let entity_id: Uuid = verb_call
            .arguments
            .iter()
            .find(|a| a.key == "entity-id")
            .and_then(|a| {
                if let Some(name) = a.value.as_symbol() {
                    ctx.resolve(name)
                } else {
                    a.value.as_uuid()
                }
            })
            .ok_or_else(|| anyhow::anyhow!("Missing entity-id argument"))?;

        // Screenings now require a workstream_id (kyc.screenings table)
        // This verb should be called via case-screening.initiate which has workstream context
        // For backwards compatibility, check if there's an active workstream for this entity
        let workstream = sqlx::query!(
            r#"SELECT w.workstream_id FROM kyc.entity_workstreams w
               JOIN kyc.cases c ON c.case_id = w.case_id
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
                // No active workstream - return error
                return Err(anyhow::anyhow!(
                    "No active workstream for entity. Use case-screening.initiate instead."
                ));
            }
        };

        // Check for existing pending screening
        let existing = sqlx::query!(
            r#"SELECT screening_id FROM kyc.screenings
               WHERE workstream_id = $1 AND screening_type = 'PEP' AND status = 'PENDING'
               LIMIT 1"#,
            workstream_id
        )
        .fetch_optional(pool)
        .await?;

        if let Some(row) = existing {
            ctx.bind("screening", row.screening_id);
            return Ok(ExecutionResult::Uuid(row.screening_id));
        }

        // Create screening record
        let screening_id = Uuid::now_v7();

        sqlx::query!(
            r#"INSERT INTO kyc.screenings
               (screening_id, workstream_id, screening_type, status)
               VALUES ($1, $2, 'PEP', 'PENDING')"#,
            screening_id,
            workstream_id
        )
        .execute(pool)
        .await?;

        ctx.bind("screening", screening_id);

        Ok(ExecutionResult::Uuid(screening_id))
    }

    #[cfg(not(feature = "database"))]
    async fn execute(
        &self,
        _verb_call: &VerbCall,
        _ctx: &mut ExecutionContext,
    ) -> Result<ExecutionResult> {
        Ok(ExecutionResult::Uuid(uuid::Uuid::now_v7()))
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

    #[cfg(feature = "database")]
    async fn execute(
        &self,
        verb_call: &VerbCall,
        ctx: &mut ExecutionContext,
        pool: &PgPool,
    ) -> Result<ExecutionResult> {
        use uuid::Uuid;

        // Get entity ID
        let entity_id: Uuid = verb_call
            .arguments
            .iter()
            .find(|a| a.key == "entity-id")
            .and_then(|a| {
                if let Some(name) = a.value.as_symbol() {
                    ctx.resolve(name)
                } else {
                    a.value.as_uuid()
                }
            })
            .ok_or_else(|| anyhow::anyhow!("Missing entity-id argument"))?;

        // Screenings now require a workstream_id (kyc.screenings table)
        let workstream = sqlx::query!(
            r#"SELECT w.workstream_id FROM kyc.entity_workstreams w
               JOIN kyc.cases c ON c.case_id = w.case_id
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

        // Check for existing pending screening
        let existing = sqlx::query!(
            r#"SELECT screening_id FROM kyc.screenings
               WHERE workstream_id = $1 AND screening_type = 'SANCTIONS' AND status = 'PENDING'
               LIMIT 1"#,
            workstream_id
        )
        .fetch_optional(pool)
        .await?;

        if let Some(row) = existing {
            ctx.bind("screening", row.screening_id);
            return Ok(ExecutionResult::Uuid(row.screening_id));
        }

        // Create screening record
        let screening_id = Uuid::now_v7();

        sqlx::query!(
            r#"INSERT INTO kyc.screenings
               (screening_id, workstream_id, screening_type, status)
               VALUES ($1, $2, 'SANCTIONS', 'PENDING')"#,
            screening_id,
            workstream_id
        )
        .execute(pool)
        .await?;

        ctx.bind("screening", screening_id);

        Ok(ExecutionResult::Uuid(screening_id))
    }

    #[cfg(not(feature = "database"))]
    async fn execute(
        &self,
        _verb_call: &VerbCall,
        _ctx: &mut ExecutionContext,
    ) -> Result<ExecutionResult> {
        Ok(ExecutionResult::Uuid(uuid::Uuid::now_v7()))
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

    #[cfg(feature = "database")]
    async fn execute(
        &self,
        _verb_call: &VerbCall,
        _ctx: &mut ExecutionContext,
        _pool: &PgPool,
    ) -> Result<ExecutionResult> {
        Err(anyhow::anyhow!(
            "screening.adverse-media is not yet implemented"
        ))
    }

    #[cfg(not(feature = "database"))]
    async fn execute(
        &self,
        _verb_call: &VerbCall,
        _ctx: &mut ExecutionContext,
    ) -> Result<ExecutionResult> {
        Err(anyhow::anyhow!(
            "screening.adverse-media is not yet implemented"
        ))
    }
}
