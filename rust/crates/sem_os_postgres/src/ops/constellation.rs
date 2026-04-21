//! Constellation verbs — SemOS-side YAML-first re-implementation.
//!
//! # Contracts (from `rust/config/verbs/constellation.yaml`)
//!
//! Both verbs project a SemOS constellation map (e.g. `struct.lux.ucits.sicav`)
//! over a CBU's persisted state. The actual reducer walk + slot hydration lives
//! behind [`ConstellationRuntime`] — a service trait registered on
//! `VerbExecutionContext.services` by ob-poc at startup. Neither op touches
//! the database directly, so the scoped txn is accepted but unused.
//!
//! ```yaml
//! constellation:
//!   verbs:
//!     hydrate:    # facts_only / read_only
//!       args:
//!         - name: cbu-id    type: uuid    required: true
//!         - name: case-id   type: uuid    required: false
//!         - name: map-name  type: string  required: true
//!       returns: record
//!     summary:    # identical arg shape, returns record
//! ```

use anyhow::Result;
use async_trait::async_trait;

use dsl_runtime::domain_ops::helpers::{
    json_extract_string, json_extract_uuid, json_extract_uuid_opt,
};
use dsl_runtime::service_traits::ConstellationRuntime;
use dsl_runtime::tx::TransactionScope;
use dsl_runtime::{VerbExecutionContext, VerbExecutionOutcome};

use super::SemOsVerbOp;

pub struct Hydrate;

#[async_trait]
impl SemOsVerbOp for Hydrate {
    fn fqn(&self) -> &str {
        "constellation.hydrate"
    }

    async fn execute(
        &self,
        args: &serde_json::Value,
        ctx: &mut VerbExecutionContext,
        _scope: &mut dyn TransactionScope,
    ) -> Result<VerbExecutionOutcome> {
        let cbu_id = json_extract_uuid(args, ctx, "cbu-id")?;
        let case_id = json_extract_uuid_opt(args, ctx, "case-id");
        let map_name = json_extract_string(args, "map-name")?;
        let runtime = ctx.service::<dyn ConstellationRuntime>()?;
        let result = runtime.hydrate(cbu_id, case_id, &map_name).await?;
        Ok(VerbExecutionOutcome::Record(result))
    }
}

pub struct Summary;

#[async_trait]
impl SemOsVerbOp for Summary {
    fn fqn(&self) -> &str {
        "constellation.summary"
    }

    async fn execute(
        &self,
        args: &serde_json::Value,
        ctx: &mut VerbExecutionContext,
        _scope: &mut dyn TransactionScope,
    ) -> Result<VerbExecutionOutcome> {
        let cbu_id = json_extract_uuid(args, ctx, "cbu-id")?;
        let case_id = json_extract_uuid_opt(args, ctx, "case-id");
        let map_name = json_extract_string(args, "map-name")?;
        let runtime = ctx.service::<dyn ConstellationRuntime>()?;
        let result = runtime.summary(cbu_id, case_id, &map_name).await?;
        Ok(VerbExecutionOutcome::Record(result))
    }
}
