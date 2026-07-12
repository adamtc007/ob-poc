//! Phase 1B typed contracts for operation resolution.

use serde::{Deserialize, Serialize};

use super::surface_object_resolution::SurfaceObjectResolutionOutput;

/// Input to operation resolution.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct OperationResolutionInput {
    /// Output from surface object resolution.
    pub surface: SurfaceObjectResolutionOutput,
}

/// Output from operation resolution.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct OperationResolutionOutput {
    /// Preserved surface-resolution output.
    pub surface: SurfaceObjectResolutionOutput,
    /// Canonical operation labels selected for downstream binding.
    #[serde(default)]
    pub resolved_operations: Vec<String>,
}
