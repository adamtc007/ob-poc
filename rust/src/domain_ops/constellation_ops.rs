//! Constellation custom operations.

use anyhow::Result;
use async_trait::async_trait;
use ob_poc_macros::register_custom_op;

use super::helpers::{
    extract_string, extract_uuid, extract_uuid_opt, json_extract_string, json_extract_uuid,
    json_extract_uuid_opt,
};
use super::{CustomOperation, ExecutionContext, ExecutionResult, VerbCall};
use crate::sem_os_runtime::constellation_runtime::{
    handle_constellation_hydrate, handle_constellation_summary,
};

#[cfg(feature = "database")]
use sqlx::PgPool;

#[register_custom_op]
pub struct ConstellationHydrateOp;

#[async_trait]
impl CustomOperation for ConstellationHydrateOp {
    fn domain(&self) -> &'static str {
        "constellation"
    }

    fn verb(&self) -> &'static str {
        "hydrate"
    }

    fn rationale(&self) -> &'static str {
        "Constellation hydration requires custom reducer and slot-walking logic"
    }

    #[cfg(feature = "database")]
    async fn execute(
        &self,
        verb_call: &VerbCall,
        ctx: &mut ExecutionContext,
        pool: &PgPool,
    ) -> Result<ExecutionResult> {
        let cbu_id = extract_uuid(verb_call, ctx, "cbu-id")?;
        let case_id = extract_uuid_opt(verb_call, ctx, "case-id");
        let map_name = extract_string(verb_call, "map-name")?;
        let result = handle_constellation_hydrate(pool, cbu_id, case_id, &map_name).await?;
        Ok(ExecutionResult::Record(serde_json::to_value(result)?))
    }

    #[cfg(not(feature = "database"))]
    async fn execute(
        &self,
        _verb_call: &VerbCall,
        _ctx: &mut ExecutionContext,
    ) -> Result<ExecutionResult> {
        Err(anyhow::anyhow!("constellation.hydrate requires database"))
    }

    #[cfg(feature = "database")]
    async fn execute_json(
        &self,
        args: &serde_json::Value,
        ctx: &mut sem_os_core::execution::VerbExecutionContext,
        pool: &PgPool,
    ) -> Result<sem_os_core::execution::VerbExecutionOutcome> {
        let cbu_id = json_extract_uuid(args, ctx, "cbu-id")?;
        let case_id = json_extract_uuid_opt(args, ctx, "case-id");
        let map_name = json_extract_string(args, "map-name")?;
        let result = handle_constellation_hydrate(pool, cbu_id, case_id, &map_name).await?;
        Ok(sem_os_core::execution::VerbExecutionOutcome::Record(
            serde_json::to_value(result)?,
        ))
    }

    fn is_migrated(&self) -> bool {
        true
    }
}

#[register_custom_op]
pub struct ConstellationSummaryOp;

#[async_trait]
impl CustomOperation for ConstellationSummaryOp {
    fn domain(&self) -> &'static str {
        "constellation"
    }

    fn verb(&self) -> &'static str {
        "summary"
    }

    fn rationale(&self) -> &'static str {
        "Constellation summary requires hydrated slot and reducer state aggregation"
    }

    #[cfg(feature = "database")]
    async fn execute(
        &self,
        verb_call: &VerbCall,
        ctx: &mut ExecutionContext,
        pool: &PgPool,
    ) -> Result<ExecutionResult> {
        let cbu_id = extract_uuid(verb_call, ctx, "cbu-id")?;
        let case_id = extract_uuid_opt(verb_call, ctx, "case-id");
        let map_name = extract_string(verb_call, "map-name")?;
        let result = handle_constellation_summary(pool, cbu_id, case_id, &map_name).await?;
        Ok(ExecutionResult::Record(serde_json::to_value(result)?))
    }

    #[cfg(not(feature = "database"))]
    async fn execute(
        &self,
        _verb_call: &VerbCall,
        _ctx: &mut ExecutionContext,
    ) -> Result<ExecutionResult> {
        Err(anyhow::anyhow!("constellation.summary requires database"))
    }

    #[cfg(feature = "database")]
    async fn execute_json(
        &self,
        args: &serde_json::Value,
        ctx: &mut sem_os_core::execution::VerbExecutionContext,
        pool: &PgPool,
    ) -> Result<sem_os_core::execution::VerbExecutionOutcome> {
        let cbu_id = json_extract_uuid(args, ctx, "cbu-id")?;
        let case_id = json_extract_uuid_opt(args, ctx, "case-id");
        let map_name = json_extract_string(args, "map-name")?;
        let result = handle_constellation_summary(pool, cbu_id, case_id, &map_name).await?;
        Ok(sem_os_core::execution::VerbExecutionOutcome::Record(
            serde_json::to_value(result)?,
        ))
    }

    fn is_migrated(&self) -> bool {
        true
    }
}
