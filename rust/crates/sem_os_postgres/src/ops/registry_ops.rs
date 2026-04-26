//! Registry domain verbs (Spec §C.1 — 20 verbs) — SemOS-side YAML-first
//! re-implementation.
//!
//! Most verbs delegate to general SemReg MCP tools (`sem_reg_*`) via
//! [`StewardshipDispatch`]. Two are polymorphic (describe-object,
//! list-objects) and route by `object-type` arg. One (`active-manifest`)
//! does direct SQL against `sem_reg_pub.active_snapshot_set`. Allowed
//! in BOTH Research and Governed AgentModes (read-only).

use anyhow::Result;
use async_trait::async_trait;

use dsl_runtime::tx::TransactionScope;
use dsl_runtime::{VerbExecutionContext, VerbExecutionOutcome};

use super::stewardship_helper::dispatch_stewardship_tool;
use super::SemOsVerbOp;

macro_rules! registry_op {
    ($struct:ident, $verb:literal, $tool:literal) => {
        pub struct $struct;

        #[async_trait]
        impl SemOsVerbOp for $struct {
            fn fqn(&self) -> &str {
                concat!("registry.", $verb)
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

registry_op!(Search, "search", "sem_reg_search");
registry_op!(ResolveContext, "resolve-context", "sem_reg_resolve_context");
registry_op!(VerbSurface, "verb-surface", "sem_reg_verb_surface");
registry_op!(
    AttributeProducers,
    "attribute-producers",
    "sem_reg_attribute_producers"
);
registry_op!(Lineage, "lineage", "sem_reg_lineage");
registry_op!(
    RegulationTrace,
    "regulation-trace",
    "sem_reg_regulation_trace"
);
registry_op!(TaxonomyTree, "taxonomy-tree", "sem_reg_taxonomy_tree");
registry_op!(
    TaxonomyMembers,
    "taxonomy-members",
    "sem_reg_taxonomy_members"
);
registry_op!(Classify, "classify", "sem_reg_classify");
registry_op!(DescribeView, "describe-view", "sem_reg_describe_view");
registry_op!(ApplyView, "apply-view", "sem_reg_apply_view");
registry_op!(DescribePolicy, "describe-policy", "sem_reg_describe_policy");
registry_op!(CoverageReport, "coverage-report", "sem_reg_coverage_report");
registry_op!(
    EvidenceFreshness,
    "evidence-freshness",
    "sem_reg_check_evidence_freshness"
);
registry_op!(
    EvidenceGaps,
    "evidence-gaps",
    "sem_reg_identify_evidence_gaps"
);
registry_op!(
    SnapshotHistory,
    "snapshot-history",
    "sem_reg_resolve_context"
);
registry_op!(SnapshotDiff, "snapshot-diff", "sem_reg_impact_analysis");

/// `registry.describe-object` — polymorphic routing by object_type arg.
pub struct DescribeObject;

#[async_trait]
impl SemOsVerbOp for DescribeObject {
    fn fqn(&self) -> &str {
        "registry.describe-object"
    }

    async fn execute(
        &self,
        args: &serde_json::Value,
        ctx: &mut VerbExecutionContext,
        _scope: &mut dyn TransactionScope,
    ) -> Result<VerbExecutionOutcome> {
        let object_type = args
            .get("object-type")
            .or_else(|| args.get("object_type"))
            .and_then(|v| v.as_str());
        let tool_name = match object_type {
            Some("verb_contract") | Some("verb") => "sem_reg_describe_verb",
            Some("entity_type_def") | Some("entity_type") => "sem_reg_describe_entity_type",
            Some("policy_rule") | Some("policy") => "sem_reg_describe_policy",
            Some("view_def") | Some("view") => "sem_reg_describe_view",
            _ => "sem_reg_describe_attribute",
        };
        dispatch_stewardship_tool(ctx, tool_name, args).await
    }
}

/// `registry.list-objects` — polymorphic routing by object_type arg.
pub struct ListObjects;

#[async_trait]
impl SemOsVerbOp for ListObjects {
    fn fqn(&self) -> &str {
        "registry.list-objects"
    }

    async fn execute(
        &self,
        args: &serde_json::Value,
        ctx: &mut VerbExecutionContext,
        _scope: &mut dyn TransactionScope,
    ) -> Result<VerbExecutionOutcome> {
        let object_type = args
            .get("object-type")
            .or_else(|| args.get("object_type"))
            .and_then(|v| v.as_str())
            .unwrap_or("verb_contract");
        let tool_name = match object_type {
            "attribute_def" | "attribute" => "sem_reg_list_attributes",
            _ => "sem_reg_list_verbs",
        };
        dispatch_stewardship_tool(ctx, tool_name, args).await
    }
}

/// `registry.active-manifest` — direct SQL against
/// `sem_reg_pub.active_snapshot_set`. No MCP tool equivalent.
pub struct ActiveManifest;

#[async_trait]
impl SemOsVerbOp for ActiveManifest {
    fn fqn(&self) -> &str {
        "registry.active-manifest"
    }

    async fn execute(
        &self,
        _args: &serde_json::Value,
        _ctx: &mut VerbExecutionContext,
        scope: &mut dyn TransactionScope,
    ) -> Result<VerbExecutionOutcome> {
        let row: Option<(String, i64)> = sqlx::query_as(
            "SELECT snapshot_set_id, object_count FROM sem_reg_pub.active_snapshot_set LIMIT 1",
        )
        .fetch_optional(scope.executor())
        .await?;

        match row {
            Some((set_id, count)) => Ok(VerbExecutionOutcome::Record(serde_json::json!({
                "snapshot_set_id": set_id,
                "object_count": count,
            }))),
            None => Ok(VerbExecutionOutcome::Record(serde_json::json!({
                "snapshot_set_id": null,
                "object_count": 0,
                "status": "no active manifest"
            }))),
        }
    }
}
