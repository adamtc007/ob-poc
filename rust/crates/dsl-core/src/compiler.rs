//! DSL compiler — Op-free path (Phase 3 CR A4)
//!
//! The Op-based `compile_to_ops_ext` / `VerbHandler` path has been removed.
//! The compiler emits `CompileStep`s (VerbCall wrappers) directly.
//!
//! # Design
//!
//! 1. Walk each VerbCall in the AST
//! 2. Emit a `CompileStep` carrying the VerbCall and source position
//! 3. Validation (unknown verb, missing args) happens at execution time via
//!    `runtime_registry()`, not at compile time

use crate::ast::{Program, Statement, VerbCall};

/// Error during compilation (parse/structural errors only).
///
/// Verb validation errors surface at execution time.
#[derive(Debug, Clone)]
pub struct CompileError {
    /// Source statement index
    pub stmt_idx: usize,
    /// Error message
    pub message: String,
}

impl std::fmt::Display for CompileError {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        write!(f, "statement {}: {}", self.stmt_idx + 1, self.message)
    }
}

/// A single step in the Op-free compilation output.
///
/// Carries the `VerbCall` directly — no domain-specific Op variant needed.
/// Binding metadata is preserved in `verb_call.binding`.
/// Verb validation (unknown verb, missing required args) happens at dispatch
/// time against `runtime_registry`, not at compile time.
#[derive(Debug, Clone)]
pub struct CompileStep {
    /// The verb call as parsed — domain, verb, arguments, and `:as @name` binding.
    pub verb_call: VerbCall,
    /// Index into the source `Program::statements` for error reporting.
    pub source_stmt: usize,
}

/// Result of Op-free compilation.
#[derive(Debug)]
pub struct CompiledSteps {
    /// Steps in source order. Dependency ordering is applied at plan-build
    /// time using the injection graph, not via Op-level `dependencies()`.
    pub steps: Vec<CompileStep>,
    /// Errors encountered during compilation (parse/syntax only).
    pub errors: Vec<CompileError>,
}

impl CompiledSteps {
    /// True when there were no compile errors.
    pub fn is_ok(&self) -> bool {
        self.errors.is_empty()
    }
}

/// Compile a DSL `Program` to a sequence of `CompileStep`s.
///
/// - **No `VerbHandler` required.** Each `VerbCall` in the AST is emitted
///   directly as a `CompileStep`. Verb existence and argument validation
///   happen at `execute_verb_in_scope` time against `runtime_registry()`.
/// - **No domain coupling.** The compiler has no knowledge of ob-poc verbs.
/// - **Binding names preserved.** `:as @name` is carried in
///   `CompileStep::verb_call.binding`; injection resolution happens at
///   plan-build time.
pub fn compile_to_steps(program: &Program) -> CompiledSteps {
    let steps = program
        .statements
        .iter()
        .enumerate()
        .filter_map(|(source_stmt, stmt)| {
            if let Statement::VerbCall(vc) = stmt {
                Some(CompileStep {
                    verb_call: vc.clone(),
                    source_stmt,
                })
            } else {
                None
            }
        })
        .collect();

    CompiledSteps {
        steps,
        errors: vec![],
    }
}
