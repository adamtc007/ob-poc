//! Phase 1A typed contracts for surface object resolution.

use serde::{Deserialize, Serialize};

use crate::semtaxonomy_v2::{CompilerInputEnvelope, SemanticIr};

/// Input to surface object resolution.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct SurfaceObjectResolutionInput {
    /// Canonical compiler input envelope.
    pub envelope: CompilerInputEnvelope,
}

/// Output from surface object resolution.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct SurfaceObjectResolutionOutput {
    /// Compiler IR preserved for later phases.
    pub semantic_ir: SemanticIr,
    /// Canonical entity/domain selected for downstream resolution.
    pub resolved_surface_entity: String,
}
