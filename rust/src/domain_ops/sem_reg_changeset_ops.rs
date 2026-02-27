//! Changeset domain CustomOps (Spec §C.3 — 14 verbs).
//!
//! All verbs delegate to Stewardship Phase 0 tools via `delegate_to_stew_tool()`.
//! Allowed in Research mode only (authoring). Blocked in Governed mode.

use anyhow::Result;
use async_trait::async_trait;

use ob_poc_macros::register_custom_op;

use super::sem_reg_helpers::delegate_to_stew_tool;
use super::{CustomOperation, ExecutionContext, ExecutionResult, VerbCall};

#[cfg(feature = "database")]
use sqlx::PgPool;

// ── Macro to reduce boilerplate for stewardship delegation ops ─────

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
                Err(anyhow::anyhow!("changeset.{} requires database", $verb))
            }
        }
    };
}

// ── Composition ───────────────────────────────────────────────────

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

// ── Suggestions & Templates ──────────────────────────────────────

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

// ── Evidence & Validation ─────────────────────────────────────────

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

// ── Cross-Referencing & Impact ────────────────────────────────────

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

// ── Conflict Resolution ──────────────────────────────────────────

changeset_op!(
    ChangesetResolveConflictOp,
    "resolve-conflict",
    "stew_resolve_conflict",
    "Delegates to stew_resolve_conflict Phase 0 tool"
);

// ── Query ─────────────────────────────────────────────────────────

/// List changesets with optional status filter.
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
        "Lists changesets via direct SQL query"
    }

    #[cfg(feature = "database")]
    async fn execute(
        &self,
        verb_call: &VerbCall,
        ctx: &mut ExecutionContext,
        pool: &PgPool,
    ) -> Result<ExecutionResult> {
        use super::sem_reg_helpers::get_string_arg;

        // Build args with optional status filter
        let mut args = super::sem_reg_helpers::extract_args_as_json(verb_call, ctx);
        if let Some(status) = get_string_arg(verb_call, "status") {
            if let Some(map) = args.as_object_mut() {
                map.insert("status".to_string(), serde_json::json!(status));
            }
        }

        // Delegate to stew_list_changesets which handles filtering
        let actor = super::sem_reg_helpers::build_actor_from_ctx(ctx);
        let tool_ctx = crate::sem_reg::agent::mcp_tools::SemRegToolContext {
            pool,
            actor: &actor,
        };
        if let Some(result) = crate::sem_reg::stewardship::dispatch_phase0_tool(
            &tool_ctx,
            "stew_describe_object",
            &args,
        )
        .await
        {
            return super::sem_reg_helpers::convert_tool_result(result);
        }

        // Fallback: direct query for changeset listing
        let rows: Vec<(String, String, String)> = sqlx::query_as(
            "SELECT changeset_id::text, status, title FROM sem_reg.changesets ORDER BY created_at DESC LIMIT 50"
        )
        .fetch_all(pool)
        .await?;

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

        Ok(ExecutionResult::RecordSet(list))
    }

    #[cfg(not(feature = "database"))]
    async fn execute(
        &self,
        _verb_call: &VerbCall,
        _ctx: &mut ExecutionContext,
    ) -> Result<ExecutionResult> {
        Err(anyhow::anyhow!("changeset.list requires database"))
    }
}

/// Get detailed changeset information.
#[register_custom_op]
pub struct ChangesetGetOp;

#[async_trait]
impl CustomOperation for ChangesetGetOp {
    fn domain(&self) -> &'static str {
        "changeset"
    }
    fn verb(&self) -> &'static str {
        "get"
    }
    fn rationale(&self) -> &'static str {
        "Delegates to stew_describe_object for changeset details"
    }

    #[cfg(feature = "database")]
    async fn execute(
        &self,
        verb_call: &VerbCall,
        ctx: &mut ExecutionContext,
        pool: &PgPool,
    ) -> Result<ExecutionResult> {
        delegate_to_stew_tool(pool, ctx, verb_call, "stew_describe_object").await
    }

    #[cfg(not(feature = "database"))]
    async fn execute(
        &self,
        _verb_call: &VerbCall,
        _ctx: &mut ExecutionContext,
    ) -> Result<ExecutionResult> {
        Err(anyhow::anyhow!("changeset.get requires database"))
    }
}

// Diff two changesets or a changeset against the active snapshot set.
changeset_op!(
    ChangesetDiffOp,
    "diff",
    "stew_get_diff",
    "Delegates to stew_get_diff for changeset comparison (Phase 1 tool)"
);
