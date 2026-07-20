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

// §9 item 9 slice 7 (2026-05-13): registry-loader subset (registry +
// schema + conditions + variable + scope) relocated to
// dsl_analysis::macros. The expansion engine (expander.rs +
// attribute_seed.rs) stays here because it reaches UnifiedSession +
// sem_os_obpoc_adapter.
mod attribute_seed;
mod expander;

// Re-export the relocated registry surface so existing
// `crate::dsl_v2::macros::*` callers keep working.
#[cfg(test)]
#[allow(unreachable_pub)]
pub use dsl_analysis::macros::schema::MacroTier;
#[cfg(test)]
#[allow(unreachable_pub)]
pub use dsl_analysis::macros::schema::{
    ArgStyle, MacroArg, MacroArgType, MacroArgs, MacroExpansionStep, MacroKind, MacroRouting,
    MacroTarget, MacroUi, SetState, VerbCallStep,
};
#[allow(unreachable_pub)]
pub use dsl_analysis::macros::{
    conditions, load_macro_registry, load_macro_registry_from_dir, registry, schema, variable,
    MacroRegistry,
};
#[allow(unreachable_pub)]
pub(crate) use expander::{
    expand_macro, expand_macro_fixpoint, ExpansionLimits, MacroExpansionError,
    MacroExpansionOutput, EXPANSION_LIMITS,
};
// ACP visibility parity types (v0.5 §7.2 / §7.7).
#[allow(unused_imports, unreachable_pub)]
pub use dsl_analysis::macros::schema::{MacroLifecycleState, MacroPlanKind, MacroSideEffect};
#[allow(unreachable_pub)]
pub use dsl_analysis::macros::schema::{MacroPrereq, MacroSchema};
