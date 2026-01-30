//! Macro Expansion Module
//!
//! Operator vocabulary layer that maps business-friendly macros to DSL primitives.
//!
//! ## Overview
//!
//! Macros provide an operator-facing vocabulary that hides implementation details:
//! - `structure.setup` → `cbu.create`
//! - `case.open` → `kyc-case.create`
//! - `mandate.create` → `trading-profile.create`
//!
//! ## Key Principles
//!
//! 1. **UI Never Sees Implementation Terms**
//!    - Operators see "Structure", "Party", "Case", "Mandate"
//!    - Never: "CBU", "entity_ref", "trading-profile"
//!
//! 2. **Expansion is Pure (No DB)**
//!    - Same input → same output, always
//!    - Variable substitution from session/scope context
//!
//! 3. **Enum Keys ≠ Internal Tokens**
//!    - UI shows `pe`, `gp`, `im`
//!    - Internal DSL uses `private-equity`, `general-partner`, `investment-manager`
//!    - Always use `${arg.X.internal}` for enum args in expansion
//!
//! ## Pipeline
//!
//! ```text
//! User: "structure.setup :type pe :name 'Acme Fund'"
//!     ↓
//! MacroRegistry.expand()
//!     ↓
//! "(cbu.create :kind private-equity :name 'Acme Fund' :client_id @client)"
//!     ↓
//! Normal DSL Pipeline (parse → enrich → compile → execute)
//! ```

mod expander;
mod registry;
mod schema;
mod scope;
mod variable;

pub use expander::{expand_macro, MacroExpansionError, MacroExpansionOutput};
pub use registry::{load_macro_registry, load_macro_registry_from_dir, MacroRegistry};
pub use schema::{
    InvokeMacroStep, MacroArg, MacroArgType, MacroEnumValue, MacroExpansionStep, MacroPrereq,
    MacroRouting, MacroSchema, MacroTarget, MacroUi, SetState, VerbCallStep,
};
pub use scope::{BoundValue, MacroExpansionScope, MacroScopeError, SymbolScope};
pub use variable::{substitute_variables, VariableContext, VariableError};
