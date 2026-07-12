//! Phase 1C typed contracts for binding resolution.

use serde::{Deserialize, Serialize};

use super::operation_resolution::OperationResolutionOutput;

/// Input to binding resolution.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct BindingResolutionInput {
    /// Output from operation resolution.
    pub operation: OperationResolutionOutput,
}

/// Output from binding resolution.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct BindingResolutionOutput {
    /// Preserved operation-resolution output.
    pub operation: OperationResolutionOutput,
    /// Canonical binding summary for downstream candidate selection.
    #[serde(default)]
    pub resolved_bindings: Vec<(String, String)>,
}
