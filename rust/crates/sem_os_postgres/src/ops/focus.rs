//! Focus domain verbs (Spec §C.5 — 6 verbs) — SemOS-side YAML-first
//! re-implementation. All delegate to Stewardship Phase 1 tools via
//! [`StewardshipDispatch`]. Allowed in BOTH Research and Governed
//! AgentModes.

use anyhow::Result;
use async_trait::async_trait;

use dsl_runtime::tx::TransactionScope;
use dsl_runtime::{VerbExecutionContext, VerbExecutionOutcome};

use super::stewardship_helper::dispatch_stewardship_tool;
use super::SemOsVerbOp;

macro_rules! focus_op {
    ($struct:ident, $verb:literal, $tool:literal) => {
        pub struct $struct;

        #[async_trait]
        impl SemOsVerbOp for $struct {
            fn fqn(&self) -> &str {
                concat!("focus.", $verb)
            }
            async fn execute(
                &self,
                args: &serde_json::Value,
                ctx: &mut VerbExecutionContext,
                _scope: &mut dyn TransactionScope,
            ) -> Result<VerbExecutionOutcome> {
                dispatch_stewardship_tool(ctx, $tool, args).await
            }
        }
    };
}

focus_op!(Get, "get", "stew_get_focus");
focus_op!(Set, "set", "stew_set_focus");
focus_op!(Render, "render", "stew_show");
focus_op!(Viewport, "viewport", "stew_get_viewport");
focus_op!(Diff, "diff", "stew_get_diff");
focus_op!(CaptureManifest, "capture-manifest", "stew_capture_manifest");
