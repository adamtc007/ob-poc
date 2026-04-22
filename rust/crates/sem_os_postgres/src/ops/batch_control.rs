//! Batch control verbs (7 plugin verbs) — YAML-first re-implementation
//! of `batch.*` from `rust/config/verbs/batch.yaml`.
//!
//! Ops:
//! - `batch.pause` / `batch.resume` / `batch.continue` / `batch.skip` /
//!   `batch.abort` / `batch.status` — session-state signals that the
//!   outer session handler processes. No DB side effects.
//! - `batch.add-products` — bulk post-batch operation that inserts
//!   into `cbu_products` for every (cbu_id, product) pair, resolving
//!   product codes against `products.product_code` / `name`.
//!
//! Phase 5c-migrate Phase B slice #71 pairs this port with the
//! relocation of `BatchControlResult` into
//! `ob-poc-types::batch_control`.

use anyhow::{anyhow, Result};
use async_trait::async_trait;
use ob_poc_types::batch_control::BatchControlResult;
use serde_json::{json, Value};
use uuid::Uuid;

use dsl_runtime::domain_ops::helpers::{
    json_extract_int_opt, json_extract_string_list, json_extract_string_opt, json_extract_uuid,
};
use dsl_runtime::tx::TransactionScope;
use dsl_runtime::{VerbExecutionContext, VerbExecutionOutcome};

use super::SemOsVerbOp;

fn batch_control_outcome(result: BatchControlResult) -> VerbExecutionOutcome {
    VerbExecutionOutcome::Record(json!({
        "_type": "batch_control",
        "operation": result.operation,
        "success": result.success,
        "status": result.status,
        "message": result.message,
    }))
}

pub struct Pause;

#[async_trait]
impl SemOsVerbOp for Pause {
    fn fqn(&self) -> &str {
        "batch.pause"
    }
    async fn execute(
        &self,
        _args: &Value,
        _ctx: &mut VerbExecutionContext,
        _scope: &mut dyn TransactionScope,
    ) -> Result<VerbExecutionOutcome> {
        tracing::info!("batch.pause called");
        Ok(batch_control_outcome(BatchControlResult {
            operation: "pause".to_string(),
            success: true,
            status: None,
            message: "Batch pause requested. Session handler will process.".to_string(),
        }))
    }
}

pub struct Resume;

#[async_trait]
impl SemOsVerbOp for Resume {
    fn fqn(&self) -> &str {
        "batch.resume"
    }
    async fn execute(
        &self,
        _args: &Value,
        _ctx: &mut VerbExecutionContext,
        _scope: &mut dyn TransactionScope,
    ) -> Result<VerbExecutionOutcome> {
        tracing::info!("batch.resume called");
        Ok(batch_control_outcome(BatchControlResult {
            operation: "resume".to_string(),
            success: true,
            status: None,
            message: "Batch resume requested. Session handler will process.".to_string(),
        }))
    }
}

pub struct Continue;

#[async_trait]
impl SemOsVerbOp for Continue {
    fn fqn(&self) -> &str {
        "batch.continue"
    }
    async fn execute(
        &self,
        args: &Value,
        _ctx: &mut VerbExecutionContext,
        _scope: &mut dyn TransactionScope,
    ) -> Result<VerbExecutionOutcome> {
        let count = json_extract_int_opt(args, "count").unwrap_or(1) as usize;
        tracing::info!(count, "batch.continue called");
        Ok(batch_control_outcome(BatchControlResult {
            operation: "continue".to_string(),
            success: true,
            status: Some(json!({ "count": count })),
            message: format!("Continue {} more items requested.", count),
        }))
    }
}

pub struct Skip;

#[async_trait]
impl SemOsVerbOp for Skip {
    fn fqn(&self) -> &str {
        "batch.skip"
    }
    async fn execute(
        &self,
        args: &Value,
        _ctx: &mut VerbExecutionContext,
        _scope: &mut dyn TransactionScope,
    ) -> Result<VerbExecutionOutcome> {
        let reason = json_extract_string_opt(args, "reason");
        tracing::info!(?reason, "batch.skip called");
        Ok(batch_control_outcome(BatchControlResult {
            operation: "skip".to_string(),
            success: true,
            status: reason.as_ref().map(|r| json!({ "reason": r })),
            message: reason.unwrap_or_else(|| "Current item skipped.".to_string()),
        }))
    }
}

pub struct Abort;

#[async_trait]
impl SemOsVerbOp for Abort {
    fn fqn(&self) -> &str {
        "batch.abort"
    }
    async fn execute(
        &self,
        args: &Value,
        _ctx: &mut VerbExecutionContext,
        _scope: &mut dyn TransactionScope,
    ) -> Result<VerbExecutionOutcome> {
        let reason = json_extract_string_opt(args, "reason");
        tracing::warn!(?reason, "batch.abort called");
        Ok(batch_control_outcome(BatchControlResult {
            operation: "abort".to_string(),
            success: true,
            status: reason.as_ref().map(|r| json!({ "reason": r })),
            message: reason.unwrap_or_else(|| "Batch execution aborted.".to_string()),
        }))
    }
}

pub struct Status;

#[async_trait]
impl SemOsVerbOp for Status {
    fn fqn(&self) -> &str {
        "batch.status"
    }
    async fn execute(
        &self,
        _args: &Value,
        _ctx: &mut VerbExecutionContext,
        _scope: &mut dyn TransactionScope,
    ) -> Result<VerbExecutionOutcome> {
        tracing::debug!("batch.status called");
        Ok(batch_control_outcome(BatchControlResult {
            operation: "status".to_string(),
            success: true,
            status: Some(json!({
                "message": "Status query - session handler will populate actual state"
            })),
            message: "Batch status requested.".to_string(),
        }))
    }
}

pub struct AddProducts;

#[async_trait]
impl SemOsVerbOp for AddProducts {
    fn fqn(&self) -> &str {
        "batch.add-products"
    }
    async fn execute(
        &self,
        args: &Value,
        ctx: &mut VerbExecutionContext,
        scope: &mut dyn TransactionScope,
    ) -> Result<VerbExecutionOutcome> {
        let cbu_ids = args
            .get("cbu-ids")
            .and_then(|v| v.as_array())
            .map(|items| {
                items
                    .iter()
                    .filter_map(|item| {
                        item.as_str().and_then(|s| {
                            if let Some(sym) = s.strip_prefix('@') {
                                ctx.resolve(sym)
                            } else {
                                Uuid::parse_str(s).ok()
                            }
                        })
                    })
                    .collect::<Vec<_>>()
            })
            .or_else(|| {
                json_extract_uuid(args, ctx, "cbu-ids")
                    .ok()
                    .map(|id| vec![id])
            })
            .unwrap_or_default();
        let products = json_extract_string_list(args, "products").unwrap_or_default();

        if cbu_ids.is_empty() {
            return Err(anyhow!("No CBU IDs provided for batch.add-products"));
        }
        if products.is_empty() {
            return Err(anyhow!("No products specified for batch.add-products"));
        }

        tracing::info!(
            cbu_count = cbu_ids.len(),
            products = ?products,
            "batch.add-products executing"
        );

        let mut success_count = 0;
        let mut failure_count = 0;
        let mut errors: Vec<String> = Vec::new();

        for cbu_id in &cbu_ids {
            for product in &products {
                let product_result: Result<Option<Uuid>, _> = sqlx::query_scalar(
                    r#"SELECT product_id FROM "ob-poc".products WHERE product_code = $1 OR name = $1"#,
                )
                .bind(product)
                .fetch_optional(scope.executor())
                .await;

                match product_result {
                    Ok(Some(product_id)) => {
                        let insert_result = sqlx::query(
                            r#"
                            INSERT INTO "ob-poc".cbu_products (cbu_id, product_id)
                            VALUES ($1, $2)
                            ON CONFLICT (cbu_id, product_id) DO NOTHING
                            "#,
                        )
                        .bind(cbu_id)
                        .bind(product_id)
                        .execute(scope.executor())
                        .await;

                        match insert_result {
                            Ok(_) => success_count += 1,
                            Err(e) => {
                                failure_count += 1;
                                errors.push(format!("{}/{}: {}", cbu_id, product, e));
                            }
                        }
                    }
                    Ok(None) => {
                        failure_count += 1;
                        errors.push(format!("Product not found: {}", product));
                    }
                    Err(e) => {
                        failure_count += 1;
                        errors.push(format!("Product lookup failed: {}", e));
                    }
                }
            }
        }

        let total_ops = cbu_ids.len() * products.len();

        tracing::info!(
            success_count,
            failure_count,
            total_ops,
            "batch.add-products completed"
        );

        Ok(batch_control_outcome(BatchControlResult {
            operation: "add-products".to_string(),
            success: failure_count == 0,
            status: Some(json!({
                "cbu_count": cbu_ids.len(),
                "products": products,
                "total_operations": total_ops,
                "success_count": success_count,
                "failure_count": failure_count,
                "errors": errors,
            })),
            message: format!(
                "Added products to {} CBUs: {} success, {} failed",
                cbu_ids.len(),
                success_count,
                failure_count
            ),
        }))
    }
}
