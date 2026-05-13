//! §9 item 9 slice 7 (2026-05-13): macros registry-loader subset
//! relocated from rust/src/dsl_v2/macros/. The expansion engine itself
//! (`expander.rs`, `attribute_seed.rs`) stays in ob-poc because it reaches
//! `crate::session::unified::UnifiedSession` and `sem_os_obpoc_adapter`.
//! What lives here is the load-time / parse-time surface: schema,
//! registry loader, condition evaluator, variable substitution, scope.

pub mod conditions;
pub mod registry;
pub mod schema;
pub mod scope;
pub mod variable;

pub use registry::{load_macro_registry, load_macro_registry_from_dir, MacroRegistry};
pub use schema::{
    MacroArg, MacroKind, MacroLifecycleState, MacroPlanKind, MacroPrereq, MacroSchema,
    MacroSideEffect, MacroTier, WhenCondition,
};
