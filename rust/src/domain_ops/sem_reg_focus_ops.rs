//! Focus domain CustomOps (Spec §C.5 — 6 verbs).
//!
//! All verbs delegate to Stewardship Phase 1 tools via `delegate_to_stew_tool()`.
//! Allowed in BOTH Research and Governed AgentModes (visualization).

use anyhow::Result;
use async_trait::async_trait;

use ob_poc_macros::register_custom_op;

use super::sem_reg_helpers::delegate_to_stew_tool;
use super::{CustomOperation, ExecutionContext, ExecutionResult, VerbCall};

#[cfg(feature = "database")]
use sqlx::PgPool;

// ── Macro to reduce boilerplate for stewardship delegation ops ─────

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

            #[cfg(feature = "database")]
            async fn execute(
                &self,
                verb_call: &VerbCall,
                ctx: &mut ExecutionContext,
                pool: &PgPool,
            ) -> Result<ExecutionResult> {
                delegate_to_stew_tool(pool, ctx, verb_call, $tool).await
            }

            #[cfg(not(feature = "database"))]
            async fn execute(
                &self,
                _verb_call: &VerbCall,
                _ctx: &mut ExecutionContext,
            ) -> Result<ExecutionResult> {
                Err(anyhow::anyhow!("focus.{} requires database", $verb))
            }
        }
    };
}

// ── Focus State ───────────────────────────────────────────────────

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

// ── Focus Display ─────────────────────────────────────────────────

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

// ── Focus Diff ────────────────────────────────────────────────────

focus_op!(
    FocusDiffOp,
    "diff",
    "stew_get_diff",
    "Delegates to stew_get_diff Phase 1 tool"
);

// ── Manifest Capture ──────────────────────────────────────────────

focus_op!(
    FocusCaptureManifestOp,
    "capture-manifest",
    "stew_capture_manifest",
    "Delegates to stew_capture_manifest Phase 1 tool"
);
