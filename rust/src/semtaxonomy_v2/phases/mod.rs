//! Typed phase contracts for the deterministic NLCI compiler.

pub mod binding_resolution;
pub mod candidate_selection;
pub mod composition;
pub mod discrimination;
pub mod operation_resolution;
pub mod surface_object_resolution;

pub use binding_resolution::{BindingResolutionInput, BindingResolutionOutput};
pub use candidate_selection::{CandidateSelectionInput, CandidateSelectionOutput};
pub use composition::{CompositionInput, CompositionOutput};
pub use discrimination::{DiscriminationInput, DiscriminationOutput};
pub use operation_resolution::{OperationResolutionInput, OperationResolutionOutput};
pub use surface_object_resolution::{
    SurfaceObjectResolutionInput, SurfaceObjectResolutionOutput,
};
