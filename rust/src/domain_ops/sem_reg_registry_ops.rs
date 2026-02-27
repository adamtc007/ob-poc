//! Registry domain CustomOps (Spec §C.1 — 20 verbs).
//!
//! All verbs delegate to existing sem_reg MCP tools via `dispatch_tool()`.
//! Allowed in BOTH Research and Governed AgentModes (read-only).

use anyhow::Result;
use async_trait::async_trait;

use ob_poc_macros::register_custom_op;

use super::sem_reg_helpers::delegate_to_tool;
use super::{CustomOperation, ExecutionContext, ExecutionResult, VerbCall};

#[cfg(feature = "database")]
use sqlx::PgPool;

// ── Macro to reduce boilerplate for simple delegation ops ──────────

macro_rules! registry_op {
    ($struct_name:ident, $verb:literal, $tool:literal, $rationale:literal) => {
        #[register_custom_op]
        pub struct $struct_name;

        #[async_trait]
        impl CustomOperation for $struct_name {
            fn domain(&self) -> &'static str {
                "registry"
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
                delegate_to_tool(pool, ctx, verb_call, $tool).await
            }

            #[cfg(not(feature = "database"))]
            async fn execute(
                &self,
                _verb_call: &VerbCall,
                _ctx: &mut ExecutionContext,
            ) -> Result<ExecutionResult> {
                Err(anyhow::anyhow!("registry.{} requires database", $verb))
            }
        }
    };
}

// ── Object Description ─────────────────────────────────────────────

/// Polymorphic describe: routes to the correct MCP tool based on object_type arg.
#[register_custom_op]
pub struct RegistryDescribeObjectOp;

#[async_trait]
impl CustomOperation for RegistryDescribeObjectOp {
    fn domain(&self) -> &'static str {
        "registry"
    }
    fn verb(&self) -> &'static str {
        "describe-object"
    }
    fn rationale(&self) -> &'static str {
        "Polymorphic routing by object_type to sem_reg_describe_* tools"
    }

    #[cfg(feature = "database")]
    async fn execute(
        &self,
        verb_call: &VerbCall,
        ctx: &mut ExecutionContext,
        pool: &PgPool,
    ) -> Result<ExecutionResult> {
        use super::sem_reg_helpers::get_string_arg;

        let object_type = get_string_arg(verb_call, "object-type")
            .or_else(|| get_string_arg(verb_call, "object_type"));

        let tool_name = match object_type.as_deref() {
            Some("verb_contract") | Some("verb") => "sem_reg_describe_verb",
            Some("entity_type_def") | Some("entity_type") => "sem_reg_describe_entity_type",
            Some("policy_rule") | Some("policy") => "sem_reg_describe_policy",
            Some("view_def") | Some("view") => "sem_reg_describe_view",
            // Default to attribute (most common)
            _ => "sem_reg_describe_attribute",
        };

        delegate_to_tool(pool, ctx, verb_call, tool_name).await
    }

    #[cfg(not(feature = "database"))]
    async fn execute(
        &self,
        _verb_call: &VerbCall,
        _ctx: &mut ExecutionContext,
    ) -> Result<ExecutionResult> {
        Err(anyhow::anyhow!(
            "registry.describe-object requires database"
        ))
    }
}

// ── Search & List ──────────────────────────────────────────────────

registry_op!(
    RegistrySearchOp,
    "search",
    "sem_reg_search",
    "Delegates to sem_reg_search MCP tool"
);

/// Polymorphic list: routes by object_type arg.
#[register_custom_op]
pub struct RegistryListObjectsOp;

#[async_trait]
impl CustomOperation for RegistryListObjectsOp {
    fn domain(&self) -> &'static str {
        "registry"
    }
    fn verb(&self) -> &'static str {
        "list-objects"
    }
    fn rationale(&self) -> &'static str {
        "Polymorphic routing by object_type to sem_reg_list_* tools"
    }

    #[cfg(feature = "database")]
    async fn execute(
        &self,
        verb_call: &VerbCall,
        ctx: &mut ExecutionContext,
        pool: &PgPool,
    ) -> Result<ExecutionResult> {
        use super::sem_reg_helpers::get_string_arg;

        let object_type = get_string_arg(verb_call, "object-type")
            .or_else(|| get_string_arg(verb_call, "object_type"))
            .unwrap_or_else(|| "verb_contract".to_string());

        let tool_name = match object_type.as_str() {
            "attribute_def" | "attribute" => "sem_reg_list_attributes",
            _ => "sem_reg_list_verbs",
        };

        delegate_to_tool(pool, ctx, verb_call, tool_name).await
    }

    #[cfg(not(feature = "database"))]
    async fn execute(
        &self,
        _verb_call: &VerbCall,
        _ctx: &mut ExecutionContext,
    ) -> Result<ExecutionResult> {
        Err(anyhow::anyhow!("registry.list-objects requires database"))
    }
}

// ── Context Resolution ─────────────────────────────────────────────

registry_op!(
    RegistryResolveContextOp,
    "resolve-context",
    "sem_reg_resolve_context",
    "Delegates to sem_reg_resolve_context MCP tool"
);

// ── Verb Surface & Producers ───────────────────────────────────────

registry_op!(
    RegistryVerbSurfaceOp,
    "verb-surface",
    "sem_reg_verb_surface",
    "Delegates to sem_reg_verb_surface MCP tool"
);

registry_op!(
    RegistryAttributeProducersOp,
    "attribute-producers",
    "sem_reg_attribute_producers",
    "Delegates to sem_reg_attribute_producers MCP tool"
);

// ── Lineage & Tracing ──────────────────────────────────────────────

registry_op!(
    RegistryLineageOp,
    "lineage",
    "sem_reg_lineage",
    "Delegates to sem_reg_lineage MCP tool"
);

registry_op!(
    RegistryRegulationTraceOp,
    "regulation-trace",
    "sem_reg_regulation_trace",
    "Delegates to sem_reg_regulation_trace MCP tool"
);

// ── Taxonomy Navigation ────────────────────────────────────────────

registry_op!(
    RegistryTaxonomyTreeOp,
    "taxonomy-tree",
    "sem_reg_taxonomy_tree",
    "Delegates to sem_reg_taxonomy_tree MCP tool"
);

registry_op!(
    RegistryTaxonomyMembersOp,
    "taxonomy-members",
    "sem_reg_taxonomy_members",
    "Delegates to sem_reg_taxonomy_members MCP tool"
);

registry_op!(
    RegistryClassifyOp,
    "classify",
    "sem_reg_classify",
    "Delegates to sem_reg_classify MCP tool"
);

// ── View Operations ────────────────────────────────────────────────

registry_op!(
    RegistryDescribeViewOp,
    "describe-view",
    "sem_reg_describe_view",
    "Delegates to sem_reg_describe_view MCP tool (alias for describe-object with type=view)"
);

registry_op!(
    RegistryApplyViewOp,
    "apply-view",
    "sem_reg_apply_view",
    "Delegates to sem_reg_apply_view MCP tool"
);

// ── Policy Description ─────────────────────────────────────────────

registry_op!(
    RegistryDescribePolicyOp,
    "describe-policy",
    "sem_reg_describe_policy",
    "Delegates to sem_reg_describe_policy MCP tool (alias for describe-object with type=policy)"
);

// ── Coverage & Evidence ────────────────────────────────────────────

registry_op!(
    RegistryCoverageReportOp,
    "coverage-report",
    "sem_reg_coverage_report",
    "Delegates to sem_reg_coverage_report MCP tool"
);

registry_op!(
    RegistryEvidenceFreshnessOp,
    "evidence-freshness",
    "sem_reg_check_evidence_freshness",
    "Delegates to sem_reg_check_evidence_freshness MCP tool"
);

registry_op!(
    RegistryEvidenceGapsOp,
    "evidence-gaps",
    "sem_reg_identify_evidence_gaps",
    "Delegates to sem_reg_identify_evidence_gaps MCP tool"
);

// ── History & Diff ─────────────────────────────────────────────────

registry_op!(
    RegistrySnapshotHistoryOp,
    "snapshot-history",
    "sem_reg_resolve_context",
    "Delegates to sem_reg context resolution with history mode"
);

registry_op!(
    RegistrySnapshotDiffOp,
    "snapshot-diff",
    "sem_reg_impact_analysis",
    "Delegates to sem_reg_impact_analysis for snapshot comparison"
);

// ── Active Manifest ────────────────────────────────────────────────

/// Gets the active manifest via the SemOsClient.
#[register_custom_op]
pub struct RegistryActiveManifestOp;

#[async_trait]
impl CustomOperation for RegistryActiveManifestOp {
    fn domain(&self) -> &'static str {
        "registry"
    }
    fn verb(&self) -> &'static str {
        "active-manifest"
    }
    fn rationale(&self) -> &'static str {
        "Queries active_snapshot_set via direct SQL — no matching MCP tool"
    }

    #[cfg(feature = "database")]
    async fn execute(
        &self,
        _verb_call: &VerbCall,
        _ctx: &mut ExecutionContext,
        pool: &PgPool,
    ) -> Result<ExecutionResult> {
        let row: Option<(String, i64)> = sqlx::query_as(
            "SELECT snapshot_set_id, object_count FROM sem_reg_pub.active_snapshot_set LIMIT 1",
        )
        .fetch_optional(pool)
        .await?;

        match row {
            Some((set_id, count)) => Ok(ExecutionResult::Record(serde_json::json!({
                "snapshot_set_id": set_id,
                "object_count": count,
            }))),
            None => Ok(ExecutionResult::Record(serde_json::json!({
                "snapshot_set_id": null,
                "object_count": 0,
                "status": "no active manifest"
            }))),
        }
    }

    #[cfg(not(feature = "database"))]
    async fn execute(
        &self,
        _verb_call: &VerbCall,
        _ctx: &mut ExecutionContext,
    ) -> Result<ExecutionResult> {
        Err(anyhow::anyhow!(
            "registry.active-manifest requires database"
        ))
    }
}
