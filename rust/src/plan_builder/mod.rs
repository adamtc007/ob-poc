//! Plan Builder — compilation pipeline decomposition.
//!
//! The plan builder orchestrates the transformation of a user utterance into
//! a `CompiledRunbook` through three stages:
//!
//! 1. **Verb Classification** (`verb_classifier`) — determines whether a verb
//!    name resolves to a primitive verb, a macro, or is unknown.
//!
//! 2. **Constraint Checking** (`constraint_gate`) — enforces pack constraints
//!    on the expanded verb set, rejecting verbs that violate active pack
//!    rules.
//!
//! 3. **Plan Assembly** (`plan_assembler`) — analyses binding dependencies
//!    between compiled steps, topologically sorts them, populates
//!    `depends_on` fields, and computes execution phases.
//!
//! ## Error Typing
//!
//! Each stage has typed errors in `errors.rs` that map cleanly to
//! `OrchestratorResponse` variants:
//!
//! | Error | Response |
//! |-------|----------|
//! | `ClassificationError` | `Clarification` |
//! | `AssemblyError` | `Clarification` (diagnostic) |
//! | `ConstraintError` | `ConstraintViolation` |
//!
//! ## Feature Gate
//!
//! The entire module is gated behind `vnext-repl` because it depends on
//! `VerbConfigIndex` and `journey::pack_manager`.

pub mod errors;
pub mod plan_assembler;

// Re-export verb_classifier and constraint_gate from runbook module.
// These remain physically in runbook/ because compiler.rs uses them via
// `super::` paths. The plan_builder module provides a unified public surface.
pub use crate::runbook::constraint_gate::{self, check_pack_constraints};
pub use crate::runbook::verb_classifier::{self, classify_verb, VerbClassification};

// Re-export plan_assembler types
pub use plan_assembler::{
    assemble_plan, AssemblyDiagnostic, DiagnosticKind, ExecutionPhase, PlanAssemblyResult,
};

// Re-export error types
pub use errors::{AssemblyError, ClassificationError, ConstraintError, PlanBuilderError};
