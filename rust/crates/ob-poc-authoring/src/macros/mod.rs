//! Operator Macro Registry (DEPRECATED)
//!
//! This module is deprecated. Use `crate::dsl_v2::macros` instead.
//! The V2 `MacroRegistry` is the single source of truth for macro definitions.
//!
//! V1 types (`OperatorMacroDef`, `OperatorMacroRegistry`) are retained here
//! for reference but should not be used in new code.

mod definition;
mod registry;

#[allow(deprecated)]
pub use definition::{
    MacroArgDef, MacroArgs, MacroEnumValue, MacroExpansion, MacroPrereq, MacroRouting,
    MacroStateSet, MacroSummary, MacroTarget, MacroUi, OperatorMacroDef,
};
#[allow(deprecated)]
pub use registry::{DomainNode, MacroFilter, MacroNode, MacroTaxonomy, OperatorMacroRegistry};
