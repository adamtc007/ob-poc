//! dsl-semos-frontend: SemOS frontend for the unified DSL v0.1.
//!
//! This crate lowers typed `AtomBag` atoms (from `dsl-ast`) into SemOS
//! registry object mutations — verb definitions, state machine declarations,
//! constellation maps, and governance metadata.
//!
//! # Scope
//!
//! - Consumes: `dsl_ast::AtomBag`, `dsl_diagnostics::DiagnosticBag`
//! - Produces: SemOS seed bundle fragments (DAG taxonomy seeds, verb configs,
//!   state machine seeds, constellation map seeds)
//!
//! # Status
//!
//! Populated in Tranche 4 (see docs/todo/master-implementation-plan-v0_1.md).
