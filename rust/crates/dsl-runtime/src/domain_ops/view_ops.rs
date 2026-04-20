//! View Operations — 15 `view.*` verbs.
//!
//! Implements the YAML contracts in `config/verbs/view.yaml`. Each op
//! is a thin wrapper that dispatches to the [`ViewService`] trait via
//! `ctx.service::<dyn ViewService>()` — the bridge handles all the
//! heavy lifting against `crate::session::ViewState` and
//! `crate::taxonomy::*` in ob-poc.
//!
//! Selection state crosses turns through `ctx.extensions["_selection"]`
//! (typed as a UUID array). Pending view state is also stashed in
//! `ctx.extensions["_pending_view_state"]` for the orchestrator to
//! propagate into `UnifiedSessionContext`.

use anyhow::Result;
use async_trait::async_trait;
use dsl_runtime_macros::register_custom_op;
use sqlx::PgPool;

use crate::custom_op::CustomOperation;
use crate::execution::{VerbExecutionContext, VerbExecutionOutcome};
use crate::service_traits::ViewService;

async fn dispatch(
    ctx: &mut VerbExecutionContext,
    pool: &PgPool,
    verb: &'static str,
    args: &serde_json::Value,
) -> Result<VerbExecutionOutcome> {
    let service = ctx.service::<dyn ViewService>()?;
    let result = service
        .dispatch_view_verb(pool, verb, args, &mut ctx.extensions)
        .await?;
    Ok(VerbExecutionOutcome::Record(result))
}

macro_rules! view_op {
    ($struct:ident, $verb:literal, $rationale:literal) => {
        #[register_custom_op]
        pub struct $struct;

        #[async_trait]
        impl CustomOperation for $struct {
            fn domain(&self) -> &'static str {
                "view"
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
                pool: &PgPool,
            ) -> Result<VerbExecutionOutcome> {
                dispatch(ctx, pool, $verb, args).await
            }
            fn is_migrated(&self) -> bool {
                true
            }
        }
    };
}

view_op!(ViewUniverseOp, "universe", "View all CBUs with optional filters");
view_op!(ViewBookOp, "book", "View all CBUs for a commercial client");
view_op!(ViewCbuOp, "cbu", "Focus on a single CBU (trading or UBO mode)");
view_op!(
    ViewEntityForestOp,
    "entity-forest",
    "View entities filtered by type/jurisdiction/role"
);
view_op!(
    ViewRefineOp,
    "refine",
    "Refine current view with include/exclude filters or explicit add/remove IDs"
);
view_op!(
    ViewClearRefinementsOp,
    "clear-refinements",
    "Clear all refinements from the current view"
);
view_op!(
    ViewClearOp,
    "clear",
    "Legacy alias for clear-refinements"
);
view_op!(
    ViewSetSelectionOp,
    "set-selection",
    "Explicitly set selection within the current view"
);
view_op!(
    ViewSetLayoutOp,
    "set-layout",
    "Configure layout strategy (mode, axis, size-by, color-by)"
);
view_op!(
    ViewReadStatusOp,
    "read-status",
    "Get current view state summary"
);
view_op!(
    ViewReadSelectionInfoOp,
    "read-selection-info",
    "Get detailed info about the current selection"
);
view_op!(ViewZoomInOp, "zoom-in", "Zoom into a node's child taxonomy");
view_op!(ViewZoomOutOp, "zoom-out", "Zoom out to the parent taxonomy");
view_op!(
    ViewNavigateBackToOp,
    "navigate-back-to",
    "Navigate back to a specific breadcrumb level"
);
view_op!(
    ViewReadBreadcrumbsOp,
    "read-breadcrumbs",
    "Get the current navigation breadcrumbs"
);
