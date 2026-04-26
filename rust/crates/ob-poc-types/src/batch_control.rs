//! `BatchControlResult` — shape returned by `batch.*` control verbs.
//!
//! Relocated from `dsl-runtime::domain_ops::batch_control_ops` in
//! Phase 5c-migrate Phase B slice #71 so the type lives upstream of
//! both `dsl-runtime` and `sem_os_postgres`. Consumed as a variant of
//! `dsl_v2::executor::ExecutionResult::BatchControl` plus the
//! idempotency cache projection.

use serde::{Deserialize, Serialize};

/// Result of batch control operations (`batch.pause` / `batch.resume`
/// / `batch.continue` / `batch.skip` / `batch.abort` / `batch.status`
/// / `batch.add-products`).
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct BatchControlResult {
    /// Operation that was performed.
    pub operation: String,
    /// Whether operation succeeded.
    pub success: bool,
    /// Batch status after operation.
    pub status: Option<serde_json::Value>,
    /// Message describing result.
    pub message: String,
}
