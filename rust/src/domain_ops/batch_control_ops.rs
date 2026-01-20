//! Batch Control Operations
//!
//! Implements batch control verbs for pause/resume/status of DSL-native batch execution:
//! - `batch.pause` - Pause current batch execution
//! - `batch.resume` - Resume paused batch execution
//! - `batch.continue` - Continue N more items
//! - `batch.skip` - Skip current item
//! - `batch.abort` - Abort batch execution
//! - `batch.status` - Get batch status

use async_trait::async_trait;
use serde::{Deserialize, Serialize};
use uuid::Uuid;

use super::CustomOperation;
use crate::dsl_v2::ast::VerbCall;
use crate::dsl_v2::executor::{ExecutionContext, ExecutionResult};

#[cfg(feature = "database")]
use anyhow::{anyhow, Result};
#[cfg(feature = "database")]
use sqlx::PgPool;

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

// ============================================================================
// batch.pause
// ============================================================================

/// Pause the current batch execution
///
/// DSL: `(batch.pause)`
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

    #[cfg(feature = "database")]
    async fn execute(
        &self,
        _verb_call: &VerbCall,
        _ctx: &mut ExecutionContext,
        _pool: &PgPool,
    ) -> Result<ExecutionResult> {
        // Note: In a full implementation, this would interact with session state
        // For now, we check if there's batch state in context

        // This operation requires session context which isn't directly available
        // In practice, this would be called via session routes that have access
        // to ActiveBatchState

        tracing::info!("batch.pause called");

        Ok(ExecutionResult::BatchControl(BatchControlResult {
            operation: "pause".to_string(),
            success: true,
            status: None,
            message: "Batch pause requested. Session handler will process.".to_string(),
        }))
    }
}

// ============================================================================
// batch.resume
// ============================================================================

/// Resume a paused batch execution
///
/// DSL: `(batch.resume)`
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

    #[cfg(feature = "database")]
    async fn execute(
        &self,
        _verb_call: &VerbCall,
        _ctx: &mut ExecutionContext,
        _pool: &PgPool,
    ) -> Result<ExecutionResult> {
        tracing::info!("batch.resume called");

        Ok(ExecutionResult::BatchControl(BatchControlResult {
            operation: "resume".to_string(),
            success: true,
            status: None,
            message: "Batch resume requested. Session handler will process.".to_string(),
        }))
    }
}

// ============================================================================
// batch.continue
// ============================================================================

/// Continue batch execution for N more items
///
/// DSL: `(batch.continue :count 10)`
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

    #[cfg(feature = "database")]
    async fn execute(
        &self,
        verb_call: &VerbCall,
        _ctx: &mut ExecutionContext,
        _pool: &PgPool,
    ) -> Result<ExecutionResult> {
        // Extract :count parameter
        let count: usize = verb_call
            .arguments
            .iter()
            .find(|a| a.key == "count")
            .and_then(|a| a.value.as_integer())
            .map(|n| n as usize)
            .unwrap_or(1);

        tracing::info!(count, "batch.continue called");

        Ok(ExecutionResult::BatchControl(BatchControlResult {
            operation: "continue".to_string(),
            success: true,
            status: Some(serde_json::json!({ "count": count })),
            message: format!("Continue {} more items requested.", count),
        }))
    }
}

// ============================================================================
// batch.skip
// ============================================================================

/// Skip the current batch item
///
/// DSL: `(batch.skip)`
/// DSL: `(batch.skip :reason "Invalid data")`
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

    #[cfg(feature = "database")]
    async fn execute(
        &self,
        verb_call: &VerbCall,
        _ctx: &mut ExecutionContext,
        _pool: &PgPool,
    ) -> Result<ExecutionResult> {
        let reason = verb_call
            .arguments
            .iter()
            .find(|a| a.key == "reason")
            .and_then(|a| a.value.as_string())
            .map(|s| s.to_string());

        tracing::info!(?reason, "batch.skip called");

        Ok(ExecutionResult::BatchControl(BatchControlResult {
            operation: "skip".to_string(),
            success: true,
            status: reason.as_ref().map(|r| serde_json::json!({ "reason": r })),
            message: reason.unwrap_or_else(|| "Current item skipped.".to_string()),
        }))
    }
}

// ============================================================================
// batch.abort
// ============================================================================

/// Abort the current batch execution
///
/// DSL: `(batch.abort)`
/// DSL: `(batch.abort :reason "User requested")`
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

    #[cfg(feature = "database")]
    async fn execute(
        &self,
        verb_call: &VerbCall,
        _ctx: &mut ExecutionContext,
        _pool: &PgPool,
    ) -> Result<ExecutionResult> {
        let reason = verb_call
            .arguments
            .iter()
            .find(|a| a.key == "reason")
            .and_then(|a| a.value.as_string())
            .map(|s| s.to_string());

        tracing::warn!(?reason, "batch.abort called");

        Ok(ExecutionResult::BatchControl(BatchControlResult {
            operation: "abort".to_string(),
            success: true,
            status: reason.as_ref().map(|r| serde_json::json!({ "reason": r })),
            message: reason.unwrap_or_else(|| "Batch execution aborted.".to_string()),
        }))
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

    #[cfg(feature = "database")]
    async fn execute(
        &self,
        _verb_call: &VerbCall,
        _ctx: &mut ExecutionContext,
        _pool: &PgPool,
    ) -> Result<ExecutionResult> {
        // In a full implementation, this would read from session's ActiveBatchState
        // For now, return a placeholder that indicates the operation type

        tracing::debug!("batch.status called");

        Ok(ExecutionResult::BatchControl(BatchControlResult {
            operation: "status".to_string(),
            success: true,
            status: Some(serde_json::json!({
                "message": "Status query - session handler will populate actual state"
            })),
            message: "Batch status requested.".to_string(),
        }))
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

    #[cfg(feature = "database")]
    async fn execute(
        &self,
        verb_call: &VerbCall,
        ctx: &mut ExecutionContext,
        pool: &PgPool,
    ) -> Result<ExecutionResult> {
        use crate::dsl_v2::ast::{AstNode, Literal};

        // Extract :cbu-ids (can be list or @symbol reference)
        let cbu_ids: Vec<Uuid> = verb_call
            .arguments
            .iter()
            .find(|a| a.key == "cbu-ids")
            .map(|a| match &a.value {
                AstNode::List { items, .. } => items
                    .iter()
                    .filter_map(|item| match item {
                        AstNode::SymbolRef { name, .. } => ctx.resolve(name),
                        AstNode::Literal(Literal::String(s)) => s.parse().ok(),
                        AstNode::Literal(Literal::Uuid(u)) => Some(*u),
                        AstNode::EntityRef {
                            resolved_key: Some(key),
                            ..
                        } => key.parse().ok(),
                        _ => None,
                    })
                    .collect(),
                AstNode::SymbolRef { name, .. } => {
                    // Single symbol reference - resolve to a single UUID
                    if let Some(id) = ctx.resolve(name) {
                        vec![id]
                    } else {
                        Vec::new()
                    }
                }
                _ => Vec::new(),
            })
            .unwrap_or_default();

        // Extract :products list
        let products: Vec<String> = verb_call
            .arguments
            .iter()
            .find(|a| a.key == "products")
            .and_then(|a| {
                if let AstNode::List { items, .. } = &a.value {
                    Some(
                        items
                            .iter()
                            .filter_map(|item| match item {
                                AstNode::Literal(Literal::String(s)) => Some(s.clone()),
                                AstNode::EntityRef { value, .. } => Some(value.clone()),
                                _ => None,
                            })
                            .collect(),
                    )
                } else {
                    None
                }
            })
            .unwrap_or_default();

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

        // Execute cbu.add-product for each CBU × product combination
        let mut success_count = 0;
        let mut failure_count = 0;
        let mut errors: Vec<String> = Vec::new();

        for cbu_id in &cbu_ids {
            for product in &products {
                // Look up product ID
                let product_result: Result<Option<Uuid>, _> = sqlx::query_scalar(
                    r#"SELECT product_id FROM "ob-poc".products WHERE product_code = $1 OR name = $1"#
                )
                .bind(product)
                .fetch_optional(pool)
                .await;

                match product_result {
                    Ok(Some(product_id)) => {
                        // Insert CBU-product link
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

        Ok(ExecutionResult::BatchControl(BatchControlResult {
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
