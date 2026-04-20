//! Batch Control Operations
//!
//! Implements batch control verbs for pause/resume/status of DSL-native batch execution:
//! - `batch.pause` - Pause current batch execution
//! - `batch.resume` - Resume paused batch execution
//! - `batch.continue` - Continue N more items
//! - `batch.skip` - Skip current item
//! - `batch.abort` - Abort batch execution
//! - `batch.status` - Get batch status

use anyhow::{anyhow, Result};
use async_trait::async_trait;
use dsl_runtime_macros::register_custom_op;
use serde::{Deserialize, Serialize};
use sqlx::PgPool;
use uuid::Uuid;

use crate::custom_op::CustomOperation;
use crate::domain_ops::helpers::{
    json_extract_int_opt, json_extract_string_list, json_extract_string_opt, json_extract_uuid,
};
use crate::execution::{VerbExecutionContext, VerbExecutionOutcome};

// ============================================================================
// Result Types
// ============================================================================

/// Result of batch control operations
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct BatchControlResult {
    /// Operation that was performed
    pub operation: String,
    /// Whether operation succeeded
    pub success: bool,
    /// Batch status after operation
    pub status: Option<serde_json::Value>,
    /// Message describing result
    pub message: String,
}

fn batch_control_outcome(result: BatchControlResult) -> VerbExecutionOutcome {
    VerbExecutionOutcome::Record(serde_json::json!({
        "_type": "batch_control",
        "operation": result.operation,
        "success": result.success,
        "status": result.status,
        "message": result.message
    }))
}

// ============================================================================
// batch.pause
// ============================================================================

/// Pause the current batch execution
///
/// DSL: `(batch.pause)`
#[register_custom_op]
pub struct BatchPauseOp;

#[async_trait]
impl CustomOperation for BatchPauseOp {
    fn domain(&self) -> &'static str {
        "batch"
    }

    fn verb(&self) -> &'static str {
        "pause"
    }

    fn rationale(&self) -> &'static str {
        "Requires session state mutation to pause active batch"
    }

    async fn execute_json(
        &self,
        _args: &serde_json::Value,
        _ctx: &mut VerbExecutionContext,
        _pool: &PgPool,
    ) -> Result<VerbExecutionOutcome> {
        tracing::info!("batch.pause called");
        Ok(batch_control_outcome(BatchControlResult {
            operation: "pause".to_string(),
            success: true,
            status: None,
            message: "Batch pause requested. Session handler will process.".to_string(),
        }))
    }

    fn is_migrated(&self) -> bool {
        true
    }
}

// ============================================================================
// batch.resume
// ============================================================================

/// Resume a paused batch execution
///
/// DSL: `(batch.resume)`
#[register_custom_op]
pub struct BatchResumeOp;

#[async_trait]
impl CustomOperation for BatchResumeOp {
    fn domain(&self) -> &'static str {
        "batch"
    }

    fn verb(&self) -> &'static str {
        "resume"
    }

    fn rationale(&self) -> &'static str {
        "Requires session state mutation to resume paused batch"
    }

    async fn execute_json(
        &self,
        _args: &serde_json::Value,
        _ctx: &mut VerbExecutionContext,
        _pool: &PgPool,
    ) -> Result<VerbExecutionOutcome> {
        tracing::info!("batch.resume called");
        Ok(batch_control_outcome(BatchControlResult {
            operation: "resume".to_string(),
            success: true,
            status: None,
            message: "Batch resume requested. Session handler will process.".to_string(),
        }))
    }

    fn is_migrated(&self) -> bool {
        true
    }
}

// ============================================================================
// batch.continue
// ============================================================================

/// Continue batch execution for N more items
///
/// DSL: `(batch.continue :count 10)`
#[register_custom_op]
pub struct BatchContinueOp;

#[async_trait]
impl CustomOperation for BatchContinueOp {
    fn domain(&self) -> &'static str {
        "batch"
    }

    fn verb(&self) -> &'static str {
        "continue"
    }

    fn rationale(&self) -> &'static str {
        "Requires session state to continue batch for N more items"
    }

    async fn execute_json(
        &self,
        args: &serde_json::Value,
        _ctx: &mut VerbExecutionContext,
        _pool: &PgPool,
    ) -> Result<VerbExecutionOutcome> {
        let count = json_extract_int_opt(args, "count").unwrap_or(1) as usize;
        tracing::info!(count, "batch.continue called");
        Ok(batch_control_outcome(BatchControlResult {
            operation: "continue".to_string(),
            success: true,
            status: Some(serde_json::json!({ "count": count })),
            message: format!("Continue {} more items requested.", count),
        }))
    }

    fn is_migrated(&self) -> bool {
        true
    }
}

// ============================================================================
// batch.skip
// ============================================================================

/// Skip the current batch item
///
/// DSL: `(batch.skip)`
/// DSL: `(batch.skip :reason "Invalid data")`
#[register_custom_op]
pub struct BatchSkipOp;

#[async_trait]
impl CustomOperation for BatchSkipOp {
    fn domain(&self) -> &'static str {
        "batch"
    }

    fn verb(&self) -> &'static str {
        "skip"
    }

    fn rationale(&self) -> &'static str {
        "Requires session state to skip current batch item"
    }

    async fn execute_json(
        &self,
        args: &serde_json::Value,
        _ctx: &mut VerbExecutionContext,
        _pool: &PgPool,
    ) -> Result<VerbExecutionOutcome> {
        let reason = json_extract_string_opt(args, "reason");
        tracing::info!(?reason, "batch.skip called");
        Ok(batch_control_outcome(BatchControlResult {
            operation: "skip".to_string(),
            success: true,
            status: reason.as_ref().map(|r| serde_json::json!({ "reason": r })),
            message: reason.unwrap_or_else(|| "Current item skipped.".to_string()),
        }))
    }

    fn is_migrated(&self) -> bool {
        true
    }
}

// ============================================================================
// batch.abort
// ============================================================================

/// Abort the current batch execution
///
/// DSL: `(batch.abort)`
/// DSL: `(batch.abort :reason "User requested")`
#[register_custom_op]
pub struct BatchAbortOp;

#[async_trait]
impl CustomOperation for BatchAbortOp {
    fn domain(&self) -> &'static str {
        "batch"
    }

    fn verb(&self) -> &'static str {
        "abort"
    }

    fn rationale(&self) -> &'static str {
        "Requires session state mutation to abort batch execution"
    }

    async fn execute_json(
        &self,
        args: &serde_json::Value,
        _ctx: &mut VerbExecutionContext,
        _pool: &PgPool,
    ) -> Result<VerbExecutionOutcome> {
        let reason = json_extract_string_opt(args, "reason");
        tracing::warn!(?reason, "batch.abort called");
        Ok(batch_control_outcome(BatchControlResult {
            operation: "abort".to_string(),
            success: true,
            status: reason.as_ref().map(|r| serde_json::json!({ "reason": r })),
            message: reason.unwrap_or_else(|| "Batch execution aborted.".to_string()),
        }))
    }

    fn is_migrated(&self) -> bool {
        true
    }
}

// ============================================================================
// batch.status
// ============================================================================

/// Get the current batch execution status
///
/// DSL: `(batch.status)`
///
/// Returns:
/// ```json
/// {
///   "template": "onboard-fund-cbu",
///   "status": "running",
///   "progress": {"current": 47, "total": 205, "success": 45, "failed": 2},
///   "current_item": "Allianz Dynamic Multi Asset Strategy",
///   "elapsed": "00:02:34"
/// }
/// ```
#[register_custom_op]
pub struct BatchStatusOp;

#[async_trait]
impl CustomOperation for BatchStatusOp {
    fn domain(&self) -> &'static str {
        "batch"
    }

    fn verb(&self) -> &'static str {
        "status"
    }

    fn rationale(&self) -> &'static str {
        "Requires session state to read batch execution status"
    }

    async fn execute_json(
        &self,
        _args: &serde_json::Value,
        _ctx: &mut VerbExecutionContext,
        _pool: &PgPool,
    ) -> Result<VerbExecutionOutcome> {
        tracing::debug!("batch.status called");
        Ok(batch_control_outcome(BatchControlResult {
            operation: "status".to_string(),
            success: true,
            status: Some(serde_json::json!({
                "message": "Status query - session handler will populate actual state"
            })),
            message: "Batch status requested.".to_string(),
        }))
    }

    fn is_migrated(&self) -> bool {
        true
    }
}

// ============================================================================
// batch.add-products (Bulk Post-Batch Operation)
// ============================================================================

/// Add products to multiple CBUs in bulk
///
/// DSL: `(batch.add-products :cbu-ids @batch_result.cbu_ids :products ["CUSTODY" "FUND_ACCOUNTING"])`
///
/// This is a convenience verb for post-batch operations that applies products
/// to all CBUs created during a batch execution.
#[register_custom_op]
pub struct BatchAddProductsOp;

#[async_trait]
impl CustomOperation for BatchAddProductsOp {
    fn domain(&self) -> &'static str {
        "batch"
    }

    fn verb(&self) -> &'static str {
        "add-products"
    }

    fn rationale(&self) -> &'static str {
        "Bulk operation applying products to multiple CBUs from batch results"
    }

    async fn execute_json(
        &self,
        args: &serde_json::Value,
        ctx: &mut VerbExecutionContext,
        pool: &PgPool,
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
                .fetch_optional(pool)
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
                        .execute(pool)
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
            status: Some(serde_json::json!({
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

    fn is_migrated(&self) -> bool {
        true
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_batch_control_result_serialization() {
        let result = BatchControlResult {
            operation: "pause".to_string(),
            success: true,
            status: Some(serde_json::json!({"paused": true})),
            message: "Batch paused".to_string(),
        };

        let json = serde_json::to_string(&result).unwrap();
        assert!(json.contains("\"operation\":\"pause\""));
        assert!(json.contains("\"success\":true"));
    }
}
