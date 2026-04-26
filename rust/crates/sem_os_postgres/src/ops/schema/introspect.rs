//! Schema introspection / extraction verbs (5) — delegate to the
//! `db_introspect` MCP tool via [`StewardshipDispatch`] (slice #7 cascade
//! trick).
//!
//! The target tool performs physical schema inspection, attribute /
//! verb / entity-type extraction, and drift detection. A shared
//! macro mints each unit-struct op.

use anyhow::{anyhow, Result};
use async_trait::async_trait;
use serde_json::Value;

use dsl_runtime::service_traits::StewardshipDispatch;
use dsl_runtime::tx::TransactionScope;
use dsl_runtime::{VerbExecutionContext, VerbExecutionOutcome};

use crate::ops::SemOsVerbOp;

async fn dispatch_db_introspect(
    ctx: &VerbExecutionContext,
    args: &Value,
) -> Result<VerbExecutionOutcome> {
    let dispatcher = ctx.service::<dyn StewardshipDispatch>()?;
    let outcome = dispatcher
        .dispatch("db_introspect", args, &ctx.principal)
        .await?
        .ok_or_else(|| anyhow!("db_introspect tool not registered"))?;
    if outcome.success {
        Ok(VerbExecutionOutcome::Record(outcome.data))
    } else {
        Err(anyhow!(
            "{}",
            outcome
                .message
                .unwrap_or_else(|| "db_introspect failed".to_string())
        ))
    }
}

macro_rules! introspect_op {
    ($struct:ident, $fqn:literal) => {
        pub struct $struct;

        #[async_trait]
        impl SemOsVerbOp for $struct {
            fn fqn(&self) -> &str {
                $fqn
            }
            async fn execute(
                &self,
                args: &Value,
                ctx: &mut VerbExecutionContext,
                _scope: &mut dyn TransactionScope,
            ) -> Result<VerbExecutionOutcome> {
                dispatch_db_introspect(ctx, args).await
            }
        }
    };
}

introspect_op!(SchemaIntrospect, "schema.introspect");
introspect_op!(SchemaExtractAttributes, "schema.extract-attributes");
introspect_op!(SchemaExtractVerbs, "schema.extract-verbs");
introspect_op!(SchemaExtractEntities, "schema.extract-entities");
introspect_op!(SchemaCrossReference, "schema.cross-reference");
