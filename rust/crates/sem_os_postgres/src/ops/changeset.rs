//! Changeset domain verbs (Spec §C.3 — 14 verbs) — SemOS-side YAML-first
//! re-implementation. Most route through [`StewardshipDispatch`] Phase 0
//! tools; `changeset.list` additionally keeps the direct-SQL fallback on
//! `sem_reg.changesets` so the verb works in environments without the
//! stewardship tool registered. Allowed in Research mode only.

use anyhow::Result;
use async_trait::async_trait;

use dsl_runtime::domain_ops::helpers::json_extract_string_opt;
use dsl_runtime::tx::TransactionScope;
use dsl_runtime::{VerbExecutionContext, VerbExecutionOutcome};

use super::stewardship_helper::dispatch_stewardship_tool;
use super::SemOsVerbOp;

macro_rules! changeset_op {
    ($struct:ident, $verb:literal, $tool:literal) => {
        pub struct $struct;

        #[async_trait]
        impl SemOsVerbOp for $struct {
            fn fqn(&self) -> &str {
                concat!("changeset.", $verb)
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

changeset_op!(Compose, "compose", "stew_compose_changeset");
changeset_op!(AddItem, "add-item", "stew_add_item");
changeset_op!(RemoveItem, "remove-item", "stew_remove_item");
changeset_op!(RefineItem, "refine-item", "stew_refine_item");
changeset_op!(Suggest, "suggest", "stew_suggest");
changeset_op!(ApplyTemplate, "apply-template", "stew_apply_template");
changeset_op!(AttachBasis, "attach-basis", "stew_attach_basis");
changeset_op!(ValidateEdit, "validate-edit", "stew_validate_edit");
changeset_op!(CrossReference, "cross-reference", "stew_cross_reference");
changeset_op!(ImpactAnalysis, "impact-analysis", "stew_impact_analysis");
changeset_op!(ResolveConflict, "resolve-conflict", "stew_resolve_conflict");
changeset_op!(Get, "get", "stew_describe_object");
changeset_op!(Diff, "diff", "stew_get_diff");

/// `changeset.list` — tries `stew_describe_object` first; falls back to
/// direct SQL on `sem_reg.changesets` with an optional status filter.
pub struct List;

#[async_trait]
impl SemOsVerbOp for List {
    fn fqn(&self) -> &str {
        "changeset.list"
    }

    async fn execute(
        &self,
        args: &serde_json::Value,
        ctx: &mut VerbExecutionContext,
        scope: &mut dyn TransactionScope,
    ) -> Result<VerbExecutionOutcome> {
        if let Ok(outcome) = dispatch_stewardship_tool(ctx, "stew_describe_object", args).await {
            return Ok(outcome);
        }
        let status = json_extract_string_opt(args, "status");
        let rows: Vec<(String, String, String)> = if let Some(status) = status {
            sqlx::query_as(
                "SELECT changeset_id::text, status, title FROM sem_reg.changesets WHERE status = $1 ORDER BY created_at DESC LIMIT 50",
            )
            .bind(status)
            .fetch_all(scope.executor())
            .await?
        } else {
            sqlx::query_as(
                "SELECT changeset_id::text, status, title FROM sem_reg.changesets ORDER BY created_at DESC LIMIT 50",
            )
            .fetch_all(scope.executor())
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
}
