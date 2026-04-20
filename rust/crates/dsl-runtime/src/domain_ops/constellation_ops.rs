//! Constellation custom operations.
//!
//! Both ops project a SemOS constellation map (e.g.
//! `struct.lux.ucits.sicav`) over a CBU's persisted state. Map
//! resolution + reducer walk live in `ob_poc::sem_os_runtime`; we
//! reach them through [`crate::service_traits::ConstellationRuntime`],
//! which projects each result through `serde_json::to_value` to keep
//! the internal `HydratedConstellation` / `ConstellationSummary`
//! types out of the data plane.

use anyhow::Result;
use async_trait::async_trait;
use dsl_runtime_macros::register_custom_op;
use sqlx::PgPool;

use crate::custom_op::CustomOperation;
use crate::domain_ops::helpers::{json_extract_string, json_extract_uuid, json_extract_uuid_opt};
use crate::execution::{VerbExecutionContext, VerbExecutionOutcome};
use crate::service_traits::ConstellationRuntime;

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

    async fn execute_json(
        &self,
        args: &serde_json::Value,
        ctx: &mut VerbExecutionContext,
        _pool: &PgPool,
    ) -> Result<VerbExecutionOutcome> {
        let cbu_id = json_extract_uuid(args, ctx, "cbu-id")?;
        let case_id = json_extract_uuid_opt(args, ctx, "case-id");
        let map_name = json_extract_string(args, "map-name")?;
        let runtime = ctx.service::<dyn ConstellationRuntime>()?;
        let result = runtime.hydrate(cbu_id, case_id, &map_name).await?;
        Ok(VerbExecutionOutcome::Record(result))
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

    async fn execute_json(
        &self,
        args: &serde_json::Value,
        ctx: &mut VerbExecutionContext,
        _pool: &PgPool,
    ) -> Result<VerbExecutionOutcome> {
        let cbu_id = json_extract_uuid(args, ctx, "cbu-id")?;
        let case_id = json_extract_uuid_opt(args, ctx, "case-id");
        let map_name = json_extract_string(args, "map-name")?;
        let runtime = ctx.service::<dyn ConstellationRuntime>()?;
        let result = runtime.summary(cbu_id, case_id, &map_name).await?;
        Ok(VerbExecutionOutcome::Record(result))
    }

    fn is_migrated(&self) -> bool {
        true
    }
}
