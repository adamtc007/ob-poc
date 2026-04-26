//! Document bundle verbs — SemOS-side YAML-first re-implementation.
//!
//! Three `docs-bundle.*` verbs apply versioned document bundles to CBUs:
//! - `apply` — resolves a bundle registry from disk, creates applied
//!   bundle + document requirements via `DocsBundleService`.
//! - `list-applied` — reads `applied_bundles` for a CBU.
//! - `list-available` — loads the on-disk registry and lists
//!   currently-effective bundles with document counts.
//!
//! `DocsBundleService::new(pool, registry)` takes an owned pool, so
//! we pass `scope.pool().clone()` (cheap Arc-internal clone). Queries
//! the service issues run on fresh connections, separate from the
//! Sequencer txn — acceptable until the service migrates to
//! scope-aware signatures.

use anyhow::Result;
use async_trait::async_trait;
use std::path::Path;

use dsl_runtime::document_bundles::{BundleContext, DocsBundleRegistry, DocsBundleService};
use dsl_runtime::domain_ops::helpers::{
    json_extract_bool_opt, json_extract_string_opt, json_extract_uuid,
};
use dsl_runtime::tx::TransactionScope;
use dsl_runtime::{VerbExecutionContext, VerbExecutionOutcome};

use super::SemOsVerbOp;

pub struct Apply;

#[async_trait]
impl SemOsVerbOp for Apply {
    fn fqn(&self) -> &str {
        "docs-bundle.apply"
    }

    async fn execute(
        &self,
        args: &serde_json::Value,
        ctx: &mut VerbExecutionContext,
        scope: &mut dyn TransactionScope,
    ) -> Result<VerbExecutionOutcome> {
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

        let service = DocsBundleService::new(scope.pool().clone(), registry);

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
}

pub struct ListApplied;

#[async_trait]
impl SemOsVerbOp for ListApplied {
    fn fqn(&self) -> &str {
        "docs-bundle.list-applied"
    }

    async fn execute(
        &self,
        args: &serde_json::Value,
        ctx: &mut VerbExecutionContext,
        scope: &mut dyn TransactionScope,
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
        .fetch_all(scope.executor())
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
}

pub struct ListAvailable;

#[async_trait]
impl SemOsVerbOp for ListAvailable {
    fn fqn(&self) -> &str {
        "docs-bundle.list-available"
    }

    async fn execute(
        &self,
        _args: &serde_json::Value,
        _ctx: &mut VerbExecutionContext,
        _scope: &mut dyn TransactionScope,
    ) -> Result<VerbExecutionOutcome> {
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
}
