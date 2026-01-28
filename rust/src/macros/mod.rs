//! Operator Macro Registry
//!
//! Loads and indexes operator macro definitions from YAML files.
//! These macros provide business vocabulary (structure, case, mandate)
//! over the technical DSL (cbu, kyc-case, trading-profile).
//!
//! See: rust/config/verb_schemas/macros/

mod definition;
mod registry;

pub use definition::*;
pub use registry::*;
