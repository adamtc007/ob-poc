//! Focus domain CustomOps (Spec §C.5 — 6 verbs).
//!
//! All verbs delegate to Stewardship Phase 1 tools via the platform
//! [`crate::service_traits::StewardshipDispatch`] service. Allowed in BOTH
//! Research and Governed AgentModes (visualization).

use anyhow::{anyhow, Result};
use async_trait::async_trait;
use dsl_runtime_macros::register_custom_op;
use sqlx::PgPool;

use crate::custom_op::CustomOperation;
use crate::execution::{VerbExecutionContext, VerbExecutionOutcome};
use crate::service_traits::StewardshipDispatch;

async fn dispatch_stewardship_tool(
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

macro_rules! focus_op {
    ($struct_name:ident, $verb:literal, $tool:literal, $rationale:literal) => {
        #[register_custom_op]
        pub struct $struct_name;

        #[async_trait]
        impl CustomOperation for $struct_name {
            fn domain(&self) -> &'static str {
                "focus"
            }
            fn verb(&self) -> &'static str {
                $verb
            }
            fn rationale(&self) -> &'static str {
                $rationale
            }

            async fn execute_json(
                &self,
                args: &serde_json::Value,
                ctx: &mut VerbExecutionContext,
                _pool: &PgPool,
            ) -> Result<VerbExecutionOutcome> {
                dispatch_stewardship_tool(ctx, $tool, args).await
            }

            fn is_migrated(&self) -> bool {
                true
            }
        }
    };
}

focus_op!(
    FocusGetOp,
    "get",
    "stew_get_focus",
    "Delegates to stew_get_focus Phase 1 tool"
);

focus_op!(
    FocusSetOp,
    "set",
    "stew_set_focus",
    "Delegates to stew_set_focus Phase 1 tool"
);

focus_op!(
    FocusRenderOp,
    "render",
    "stew_show",
    "Delegates to stew_show Phase 1 tool"
);

focus_op!(
    FocusViewportOp,
    "viewport",
    "stew_get_viewport",
    "Delegates to stew_get_viewport Phase 1 tool"
);

focus_op!(
    FocusDiffOp,
    "diff",
    "stew_get_diff",
    "Delegates to stew_get_diff Phase 1 tool"
);

focus_op!(
    FocusCaptureManifestOp,
    "capture-manifest",
    "stew_capture_manifest",
    "Delegates to stew_capture_manifest Phase 1 tool"
);
