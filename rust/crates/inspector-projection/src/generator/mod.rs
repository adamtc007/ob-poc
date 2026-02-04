//! Projection generators that transform domain types into Inspector projections.
//!
//! Each generator converts a specific domain type (e.g., `CbuGraphResponse`)
//! into `InspectorProjection` nodes following the projection schema.
//!
//! ## Architecture
//!
//! ```text
//! CbuGraphResponse ──► CbuGenerator ──► InspectorProjection
//! TradingMatrixDocument ──► MatrixGenerator ──► InspectorProjection
//! ```
//!
//! Generators are deterministic: same input produces same output.

pub mod cbu;
pub mod matrix;

pub use cbu::CbuGenerator;
pub use matrix::MatrixGenerator;

use crate::model::InspectorProjection;
use crate::policy::RenderPolicy;

/// Trait for types that can generate an Inspector projection.
///
/// Implementations should be deterministic - same input and policy
/// should produce identical output.
pub trait ProjectionGenerator {
    /// The source data type this generator transforms.
    type Source;

    /// Generate a projection from the source data.
    ///
    /// # Arguments
    /// * `source` - The source data to transform
    /// * `policy` - Rendering policy (LOD, depth limits, filters)
    ///
    /// # Returns
    /// A complete `InspectorProjection` ready for rendering.
    fn generate(&self, source: &Self::Source, policy: &RenderPolicy) -> InspectorProjection;

    /// Generate a projection with default policy.
    fn generate_with_defaults(&self, source: &Self::Source) -> InspectorProjection {
        self.generate(source, &RenderPolicy::default())
    }
}
