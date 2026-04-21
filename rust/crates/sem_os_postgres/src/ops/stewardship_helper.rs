//! Shared helper for stewardship-dispatch verbs (focus, governance,
//! changeset, audit, registry, schema, maintenance).
//!
//! Every op in those domains delegates to a SemOS tool name via
//! [`StewardshipDispatch::dispatch`]. The ob-poc-side impl
//! (`ObPocStewardshipDispatch`) cascades Phase 0 → Phase 1 → general
//! `sem_reg_*` MCP, so tools without the `stew_` prefix fall through
//! transparently. The helper unwraps the `StewardshipOutcome` and
//! converts to [`VerbExecutionOutcome`].

use anyhow::{anyhow, Result};

use dsl_runtime::service_traits::StewardshipDispatch;
use dsl_runtime::{VerbExecutionContext, VerbExecutionOutcome};

/// Dispatch `tool_name` to [`StewardshipDispatch`] and convert the outcome.
pub async fn dispatch_stewardship_tool(
    ctx: &VerbExecutionContext,
    tool_name: &str,
    args: &serde_json::Value,
) -> Result<VerbExecutionOutcome> {
    let dispatcher = ctx.service::<dyn StewardshipDispatch>()?;
    let outcome = dispatcher
        .dispatch(tool_name, args, &ctx.principal)
        .await?
        .ok_or_else(|| anyhow!("Unknown stewardship tool: {}", tool_name))?;
    if outcome.success {
        Ok(VerbExecutionOutcome::Record(outcome.data))
    } else {
        Err(anyhow!(
            "{}",
            outcome
                .message
                .unwrap_or_else(|| format!("Stewardship tool {} failed", tool_name))
        ))
    }
}
