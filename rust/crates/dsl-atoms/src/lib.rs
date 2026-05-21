//! dsl-atoms: Shared atom kind taxonomy for the unified DSL v0.1.
//!
//! Provides the closed catalogue of structural and declarative atom kinds,
//! the `classify` function for kind string mapping, and slot parameter type
//! descriptors used by the type-checker (Tranche 5+).

pub mod kinds;
pub mod param_type;

pub use kinds::{classify, AtomKindClass, DeclarativeKind, StructuralKind};
pub use param_type::ParamType;
