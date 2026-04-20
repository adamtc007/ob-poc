//! Document Bundle custom operations
//!
//! Operations for applying versioned document bundles to CBUs.
//! Bundles define sets of required documents for fund structures.

use anyhow::Result;
use async_trait::async_trait;
use dsl_runtime_macros::register_custom_op;
use sqlx::PgPool;

use crate::custom_op::CustomOperation;
use crate::domain_ops::helpers::{json_extract_bool_opt, json_extract_string_opt, json_extract_uuid};
use crate::execution::{VerbExecutionContext, VerbExecutionOutcome};

#[register_custom_op]
pub struct DocsBundleApplyOp;

#[async_trait]
impl CustomOperation for DocsBundleApplyOp {
    fn domain(&self) -> &'static str {
        "docs-bundle"
    }
    fn verb(&self) -> &'static str {
        "apply"
    }
    fn rationale(&self) -> &'static str {
        "Requires bundle registry lookup, inheritance resolution, and conditional document creation"
    }

    async fn execute_json(
        &self,
        args: &serde_json::Value,
        ctx: &mut VerbExecutionContext,
        pool: &PgPool,
    ) -> Result<VerbExecutionOutcome> {
        use crate::document_bundles::{BundleContext, DocsBundleRegistry, DocsBundleService};
        use std::path::Path;

        let cbu_id = json_extract_uuid(args, ctx, "cbu-id")?;
        let bundle_id = json_extract_string_opt(args, "bundle")
            .ok_or_else(|| anyhow::anyhow!("Missing bundle argument"))?;

        let mut bundle_context = BundleContext::new();

        if let Some(has_pb) = json_extract_bool_opt(args, "has-prime-broker") {
            bundle_context = bundle_context.with_flag("has-prime-broker", has_pb);
        }
        if let Some(has_mm) = json_extract_bool_opt(args, "has-market-maker") {
            bundle_context = bundle_context.with_flag("has-market-maker", has_mm);
        }
        if let Some(wrapper) = json_extract_string_opt(args, "wrapper") {
            bundle_context = bundle_context.with_value("wrapper", &wrapper);
        }

        let config_dir = std::env::var("DSL_CONFIG_DIR").unwrap_or_else(|_| "config".to_string());
        let bundles_dir = Path::new(&config_dir).join("document_bundles");

        let registry = DocsBundleRegistry::load_from_dir(&bundles_dir)
            .map_err(|e| anyhow::anyhow!("Failed to load bundle registry: {}", e))?;

        let service = DocsBundleService::new(pool.clone(), registry);

        let macro_id = json_extract_string_opt(args, "macro-id");

        let result = service
            .apply_bundle(
                cbu_id,
                &bundle_id,
                &bundle_context,
                macro_id.as_deref(),
                Some("dsl"),
            )
            .await
            .map_err(|e| anyhow::anyhow!("Failed to apply bundle: {}", e))?;

        let requirement_ids: Vec<serde_json::Value> = result
            .requirements
            .iter()
            .map(|r| serde_json::Value::String(r.requirement_id.to_string()))
            .collect();

        Ok(VerbExecutionOutcome::Record(serde_json::json!({
            "applied_bundle_id": result.applied_bundle.applied_id.to_string(),
            "bundle_id": result.applied_bundle.bundle_id,
            "bundle_version": result.applied_bundle.bundle_version,
            "requirements_created": result.requirements.len(),
            "requirement_ids": requirement_ids,
        })))
    }

    fn is_migrated(&self) -> bool {
        true
    }
}

#[register_custom_op]
pub struct DocsBundleListAppliedOp;

#[async_trait]
impl CustomOperation for DocsBundleListAppliedOp {
    fn domain(&self) -> &'static str {
        "docs-bundle"
    }
    fn verb(&self) -> &'static str {
        "list-applied"
    }
    fn rationale(&self) -> &'static str {
        "Queries applied_bundles table for CBU"
    }

    async fn execute_json(
        &self,
        args: &serde_json::Value,
        ctx: &mut VerbExecutionContext,
        pool: &PgPool,
    ) -> Result<VerbExecutionOutcome> {
        let cbu_id = json_extract_uuid(args, ctx, "cbu-id")?;

        let bundles: Vec<(
            String,
            String,
            chrono::DateTime<chrono::Utc>,
            Option<String>,
        )> = sqlx::query_as(
            r#"
                SELECT
                    bundle_id,
                    bundle_version,
                    applied_at,
                    macro_id
                FROM "ob-poc".applied_bundles
                WHERE cbu_id = $1
                ORDER BY applied_at DESC
                "#,
        )
        .bind(cbu_id)
        .fetch_all(pool)
        .await?;

        let records: Vec<serde_json::Value> = bundles
            .into_iter()
            .map(|(bundle_id, bundle_version, applied_at, macro_id)| {
                serde_json::json!({
                    "bundle_id": bundle_id,
                    "bundle_version": bundle_version,
                    "applied_at": applied_at.to_rfc3339(),
                    "macro_id": macro_id,
                })
            })
            .collect();

        Ok(VerbExecutionOutcome::RecordSet(records))
    }

    fn is_migrated(&self) -> bool {
        true
    }
}

#[register_custom_op]
pub struct DocsBundleListAvailableOp;

#[async_trait]
impl CustomOperation for DocsBundleListAvailableOp {
    fn domain(&self) -> &'static str {
        "docs-bundle"
    }
    fn verb(&self) -> &'static str {
        "list-available"
    }
    fn rationale(&self) -> &'static str {
        "Loads bundle registry from YAML and returns metadata"
    }

    async fn execute_json(
        &self,
        _args: &serde_json::Value,
        _ctx: &mut VerbExecutionContext,
        _pool: &PgPool,
    ) -> Result<VerbExecutionOutcome> {
        use crate::document_bundles::DocsBundleRegistry;
        use std::path::Path;

        let config_dir = std::env::var("DSL_CONFIG_DIR").unwrap_or_else(|_| "config".to_string());
        let bundles_dir = Path::new(&config_dir).join("document_bundles");

        let registry = DocsBundleRegistry::load_from_dir(&bundles_dir)
            .map_err(|e| anyhow::anyhow!("Failed to load bundle registry: {}", e))?;

        let records: Vec<serde_json::Value> = registry
            .list_effective()
            .into_iter()
            .map(|b| {
                let resolved_docs = registry.get_resolved(&b.id);
                let doc_count = resolved_docs.map(|d| d.len()).unwrap_or(0);

                serde_json::json!({
                    "bundle_id": b.id,
                    "display_name": b.display_name,
                    "version": b.version,
                    "extends": b.extends,
                    "document_count": doc_count,
                    "effective_from": b.effective_from.to_string(),
                    "effective_to": b.effective_to.map(|d| d.to_string()),
                })
            })
            .collect();

        Ok(VerbExecutionOutcome::RecordSet(records))
    }

    fn is_migrated(&self) -> bool {
        true
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_op_metadata() {
        let op = DocsBundleApplyOp;
        assert_eq!(op.domain(), "docs-bundle");
        assert_eq!(op.verb(), "apply");
    }
}
