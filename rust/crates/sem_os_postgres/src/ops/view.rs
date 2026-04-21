//! View verbs — SemOS-side YAML-first re-implementation of 15 `view.*` ops.
//!
//! Every verb dispatches to [`ViewService::dispatch_view_verb`], which
//! owns the ViewState + taxonomy logic in ob-poc. Selection state
//! crosses turns via `ctx.extensions` (keyed `_selection` /
//! `_pending_view_state`). YAML contracts in `config/verbs/view.yaml`.

use anyhow::Result;
use async_trait::async_trait;

use dsl_runtime::service_traits::ViewService;
use dsl_runtime::tx::TransactionScope;
use dsl_runtime::{VerbExecutionContext, VerbExecutionOutcome};

use super::SemOsVerbOp;

macro_rules! view_op {
    ($struct:ident, $verb:literal) => {
        pub struct $struct;

        #[async_trait]
        impl SemOsVerbOp for $struct {
            fn fqn(&self) -> &str {
                concat!("view.", $verb)
            }
            async fn execute(
                &self,
                args: &serde_json::Value,
                ctx: &mut VerbExecutionContext,
                scope: &mut dyn TransactionScope,
            ) -> Result<VerbExecutionOutcome> {
                let service = ctx.service::<dyn ViewService>()?;
                let result = service
                    .dispatch_view_verb(scope.pool(), $verb, args, &mut ctx.extensions)
                    .await?;
                Ok(VerbExecutionOutcome::Record(result))
            }
        }
    };
}

view_op!(Universe, "universe");
view_op!(Book, "book");
view_op!(Cbu, "cbu");
view_op!(EntityForest, "entity-forest");
view_op!(Refine, "refine");
view_op!(ClearRefinements, "clear-refinements");
view_op!(Clear, "clear");
view_op!(SetSelection, "set-selection");
view_op!(SetLayout, "set-layout");
view_op!(ReadStatus, "read-status");
view_op!(ReadSelectionInfo, "read-selection-info");
view_op!(ZoomIn, "zoom-in");
view_op!(ZoomOut, "zoom-out");
view_op!(NavigateBackTo, "navigate-back-to");
view_op!(ReadBreadcrumbs, "read-breadcrumbs");
