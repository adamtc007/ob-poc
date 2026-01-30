//! Document Bundle custom operations
//!
//! Operations for applying versioned document bundles to CBUs.
//! Bundles define sets of required documents for fund structures.
//!
//! ## Operations
//!
//! - `docs-bundle.apply` - Apply a bundle to CBU (creates document requirements)
//! - `docs-bundle.list-applied` - List bundles applied to a CBU
//! - `docs-bundle.list-available` - List all available bundles

use anyhow::Result;
use async_trait::async_trait;
use ob_poc_macros::register_custom_op;

use super::CustomOperation;
use crate::dsl_v2::ast::VerbCall;
use crate::dsl_v2::executor::{ExecutionContext, ExecutionResult};

#[cfg(feature = "database")]
use sqlx::PgPool;

#[cfg(feature = "database")]
use super::helpers::{extract_bool_opt, extract_string_opt, extract_uuid};

/// Apply a document bundle to a CBU
///
/// Creates document_requirements for each document in the bundle.
/// Handles inheritance and required_if conditions.
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

    #[cfg(feature = "database")]
    async fn execute(
        &self,
        verb_call: &VerbCall,
        ctx: &mut ExecutionContext,
        pool: &PgPool,
    ) -> Result<ExecutionResult> {
        use crate::document_bundles::{BundleContext, DocsBundleRegistry, DocsBundleService};
        use std::path::Path;

        // Extract required arguments using helpers
        let cbu_id = extract_uuid(verb_call, ctx, "cbu-id")?;

        let bundle_id = extract_string_opt(verb_call, "bundle")
            .ok_or_else(|| anyhow::anyhow!("Missing bundle argument"))?;

        // Build context from optional arguments
        let mut bundle_context = BundleContext::new();

        if let Some(has_pb) = extract_bool_opt(verb_call, "has-prime-broker") {
            bundle_context = bundle_context.with_flag("has-prime-broker", has_pb);
        }

        if let Some(has_mm) = extract_bool_opt(verb_call, "has-market-maker") {
            bundle_context = bundle_context.with_flag("has-market-maker", has_mm);
        }

        if let Some(wrapper) = extract_string_opt(verb_call, "wrapper") {
            bundle_context = bundle_context.with_value("wrapper", &wrapper);
        }

        // Load bundle registry
        // Note: In production, this should be cached at app startup
        let config_dir = std::env::var("DSL_CONFIG_DIR").unwrap_or_else(|_| "config".to_string());
        let bundles_dir = Path::new(&config_dir).join("document_bundles");

        let registry = DocsBundleRegistry::load_from_dir(&bundles_dir)
            .map_err(|e| anyhow::anyhow!("Failed to load bundle registry: {}", e))?;

        let service = DocsBundleService::new(pool.clone(), registry);

        // Get macro_id from optional argument
        let macro_id = extract_string_opt(verb_call, "macro-id");

        // Apply the bundle
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

        // Build result
        let requirement_ids: Vec<serde_json::Value> = result
            .requirements
            .iter()
            .map(|r| serde_json::Value::String(r.requirement_id.to_string()))
            .collect();

        Ok(ExecutionResult::Record(serde_json::json!({
            "applied_bundle_id": result.applied_bundle.applied_id.to_string(),
            "bundle_id": result.applied_bundle.bundle_id,
            "bundle_version": result.applied_bundle.bundle_version,
            "requirements_created": result.requirements.len(),
            "requirement_ids": requirement_ids,
        })))
    }
}

/// List bundles applied to a CBU
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

    #[cfg(feature = "database")]
    async fn execute(
        &self,
        verb_call: &VerbCall,
        ctx: &mut ExecutionContext,
        pool: &PgPool,
    ) -> Result<ExecutionResult> {
        // Extract CBU ID using helper
        let cbu_id = extract_uuid(verb_call, ctx, "cbu-id")?;

        // Query applied bundles
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

        Ok(ExecutionResult::RecordSet(records))
    }
}

/// List all available document bundles
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

    #[cfg(feature = "database")]
    async fn execute(
        &self,
        _verb_call: &VerbCall,
        _ctx: &mut ExecutionContext,
        _pool: &PgPool,
    ) -> Result<ExecutionResult> {
        use crate::document_bundles::DocsBundleRegistry;
        use std::path::Path;

        // Load bundle registry
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

        Ok(ExecutionResult::RecordSet(records))
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
