//! Changeset domain CustomOps (Spec §C.3 — 14 verbs).
//!
//! Most verbs delegate to Stewardship Phase 0 tools. `ChangesetListOp`
//! additionally keeps its direct-SQL fallback against `sem_reg.changesets`
//! for environments where the stewardship tool is not yet registered.
//! Allowed in Research mode only (authoring). Blocked in Governed mode.

use anyhow::{anyhow, Result};
use async_trait::async_trait;
use dsl_runtime_macros::register_custom_op;
use sqlx::PgPool;

use crate::custom_op::CustomOperation;
use crate::domain_ops::helpers::json_extract_string_opt;
use crate::execution::{VerbExecutionContext, VerbExecutionOutcome};
use crate::service_traits::StewardshipDispatch;

async fn dispatch_tool(
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

macro_rules! changeset_op {
    ($struct_name:ident, $verb:literal, $tool:literal, $rationale:literal) => {
        #[register_custom_op]
        pub struct $struct_name;

        #[async_trait]
        impl CustomOperation for $struct_name {
            fn domain(&self) -> &'static str {
                "changeset"
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
                dispatch_tool(ctx, $tool, args).await
            }

            fn is_migrated(&self) -> bool {
                true
            }
        }
    };
}

changeset_op!(
    ChangesetComposeOp,
    "compose",
    "stew_compose_changeset",
    "Delegates to stew_compose_changeset Phase 0 tool"
);

changeset_op!(
    ChangesetAddItemOp,
    "add-item",
    "stew_add_item",
    "Delegates to stew_add_item Phase 0 tool"
);

changeset_op!(
    ChangesetRemoveItemOp,
    "remove-item",
    "stew_remove_item",
    "Delegates to stew_remove_item Phase 0 tool"
);

changeset_op!(
    ChangesetRefineItemOp,
    "refine-item",
    "stew_refine_item",
    "Delegates to stew_refine_item Phase 0 tool"
);

changeset_op!(
    ChangesetSuggestOp,
    "suggest",
    "stew_suggest",
    "Delegates to stew_suggest Phase 0 tool"
);

changeset_op!(
    ChangesetApplyTemplateOp,
    "apply-template",
    "stew_apply_template",
    "Delegates to stew_apply_template Phase 0 tool"
);

changeset_op!(
    ChangesetAttachBasisOp,
    "attach-basis",
    "stew_attach_basis",
    "Delegates to stew_attach_basis Phase 0 tool"
);

changeset_op!(
    ChangesetValidateEditOp,
    "validate-edit",
    "stew_validate_edit",
    "Delegates to stew_validate_edit Phase 0 tool"
);

changeset_op!(
    ChangesetCrossReferenceOp,
    "cross-reference",
    "stew_cross_reference",
    "Delegates to stew_cross_reference Phase 0 tool"
);

changeset_op!(
    ChangesetImpactAnalysisOp,
    "impact-analysis",
    "stew_impact_analysis",
    "Delegates to stew_impact_analysis Phase 0 tool"
);

changeset_op!(
    ChangesetResolveConflictOp,
    "resolve-conflict",
    "stew_resolve_conflict",
    "Delegates to stew_resolve_conflict Phase 0 tool"
);

changeset_op!(
    ChangesetGetOp,
    "get",
    "stew_describe_object",
    "Delegates to stew_describe_object for changeset details"
);

changeset_op!(
    ChangesetDiffOp,
    "diff",
    "stew_get_diff",
    "Delegates to stew_get_diff for changeset comparison (Phase 1 tool)"
);

/// Lists changesets with optional status filter. Tries `stew_describe_object`
/// first; falls back to direct SQL on `sem_reg.changesets` so the verb
/// works in environments where the stewardship tool is not yet registered.
#[register_custom_op]
pub struct ChangesetListOp;

#[async_trait]
impl CustomOperation for ChangesetListOp {
    fn domain(&self) -> &'static str {
        "changeset"
    }
    fn verb(&self) -> &'static str {
        "list"
    }
    fn rationale(&self) -> &'static str {
        "Lists changesets via stew_describe_object; falls back to direct SQL if unavailable"
    }

    async fn execute_json(
        &self,
        args: &serde_json::Value,
        ctx: &mut VerbExecutionContext,
        pool: &PgPool,
    ) -> Result<VerbExecutionOutcome> {
        // Try stewardship dispatch first.
        if let Ok(outcome) = dispatch_tool(ctx, "stew_describe_object", args).await {
            return Ok(outcome);
        }
        // Fall back to direct SQL on sem_reg.changesets.
        let status = json_extract_string_opt(args, "status");
        let rows: Vec<(String, String, String)> = if let Some(status) = status {
            sqlx::query_as(
                "SELECT changeset_id::text, status, title FROM sem_reg.changesets WHERE status = $1 ORDER BY created_at DESC LIMIT 50",
            )
            .bind(status)
            .fetch_all(pool)
            .await?
        } else {
            sqlx::query_as(
                "SELECT changeset_id::text, status, title FROM sem_reg.changesets ORDER BY created_at DESC LIMIT 50",
            )
            .fetch_all(pool)
            .await?
        };
        let list: Vec<serde_json::Value> = rows
            .into_iter()
            .map(|(id, status, title)| {
                serde_json::json!({
                    "changeset_id": id,
                    "status": status,
                    "title": title,
                })
            })
            .collect();
        Ok(VerbExecutionOutcome::RecordSet(list))
    }

    fn is_migrated(&self) -> bool {
        true
    }
}
