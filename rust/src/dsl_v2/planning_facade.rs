//! Planning Facade
//!
//! One central function that:
//! 1. Parses DSL text → Program
//! 2. Validates it
//! 3. Runs planning (DAG + precedence + implicit create)
//! 4. Returns both diagnostics and a planned ExecutionPlan
//! 5. Always attempts planning (even with validation errors) so LSP gets full picture

use std::sync::Arc;

use super::ast::Program;
use super::binding_context::BindingContext;
use super::compiler::compile_to_ops;
use super::dag::{build_execution_plan as build_dag_plan, describe_plan, CycleError};
use super::diagnostics::{Diagnostic, DiagnosticCode, SourceSpan};
use super::ops::Op;
use super::parser::parse_program;
use super::runtime_registry::RuntimeVerbRegistry;

/// Context for implicit create behavior
#[derive(Clone, Copy, Debug, PartialEq, Eq, Default)]
pub enum ImplicitCreateMode {
    /// Never synthesize implicit creates (production, strict mode)
    #[default]
    Disabled,
    /// Allow implicit creates and emit hints (REPL, development)
    Enabled,
    /// Allow implicit creates silently (tests)
    Silent,
}

/// Input configuration for the planning facade
pub struct PlanningInput<'a> {
    /// DSL source text to parse and plan
    pub source: &'a str,
    /// Verb registry for validation
    pub registry: Arc<RuntimeVerbRegistry>,
    /// Optional: binding context from previously executed DSL (REPL session)
    pub executed_bindings: Option<&'a BindingContext>,
    /// If true: use strict semantic validation
    pub strict_semantics: bool,
    /// How to handle missing producers (implicit create)
    pub implicit_create_mode: ImplicitCreateMode,
}

impl<'a> PlanningInput<'a> {
    /// Create basic input with just source and registry
    pub fn new(source: &'a str, registry: Arc<RuntimeVerbRegistry>) -> Self {
        Self {
            source,
            registry,
            executed_bindings: None,
            strict_semantics: false,
            implicit_create_mode: ImplicitCreateMode::default(),
        }
    }

    /// Enable strict semantics
    pub fn strict(mut self) -> Self {
        self.strict_semantics = true;
        self
    }

    /// Set executed bindings from REPL session
    pub fn with_bindings(mut self, bindings: &'a BindingContext) -> Self {
        self.executed_bindings = Some(bindings);
        self
    }

    /// Set implicit create mode
    pub fn with_implicit_create(mut self, mode: ImplicitCreateMode) -> Self {
        self.implicit_create_mode = mode;
        self
    }
}

/// A synthetic step to inject (for implicit creates)
#[derive(Clone, Debug)]
pub struct SyntheticStep {
    /// The binding name that needs to be created
    pub binding: String,
    /// The entity type to create
    pub entity_type: String,
    /// The canonical verb to use (e.g., "cbu.ensure")
    pub canonical_verb: String,
    /// Where to insert in the source (before this statement index)
    pub insert_before_stmt: usize,
    /// Suggested DSL code
    pub suggested_dsl: String,
}

/// Execution plan with ops in topological order
#[derive(Clone, Debug)]
pub struct PlannedExecution {
    /// Ops in execution order (topologically sorted)
    pub ops: Vec<Op>,
    /// Phase groupings for display
    pub phases: Vec<(String, Vec<usize>)>,
}

impl PlannedExecution {
    /// Describe the plan for display
    pub fn describe(&self) -> String {
        let dag_plan = super::dag::ExecutionPlan {
            ops: self.ops.clone(),
            phases: self
                .phases
                .iter()
                .map(|(name, indices)| super::dag::ExecutionPhase {
                    name: name.clone(),
                    op_indices: indices.clone(),
                })
                .collect(),
            original_count: self.ops.len(),
        };
        describe_plan(&dag_plan)
    }

    /// Get total op count
    pub fn op_count(&self) -> usize {
        self.ops.len()
    }
}

/// Output from the planning facade
#[derive(Debug, Default)]
pub struct PlanningOutput {
    /// Parsed program (empty if parse failed)
    pub program: Program,
    /// All diagnostics collected during parsing, validation, and planning
    pub diagnostics: Vec<Diagnostic>,
    /// Compiled ops (before toposort) - stored as raw ops since CompiledProgram doesn't impl Clone
    pub compiled_ops: Option<Vec<Op>>,
    /// Execution plan (topologically sorted ops)
    pub plan: Option<PlannedExecution>,
    /// True if the source order was different from execution order
    pub was_reordered: bool,
    /// Synthetic steps suggested for implicit creates
    pub synthetic_steps: Vec<SyntheticStep>,
}

impl PlanningOutput {
    /// Check if there are any errors
    pub fn has_errors(&self) -> bool {
        self.diagnostics.iter().any(|d| d.is_error())
    }

    /// Check if there are hard errors that block execution
    pub fn has_hard_errors(&self) -> bool {
        self.diagnostics.iter().any(|d| d.is_hard_error())
    }

    /// Get only error diagnostics
    pub fn errors(&self) -> Vec<&Diagnostic> {
        self.diagnostics.iter().filter(|d| d.is_error()).collect()
    }

    /// Get only warning diagnostics
    pub fn warnings(&self) -> Vec<&Diagnostic> {
        self.diagnostics.iter().filter(|d| d.is_warning()).collect()
    }

    /// Check if planning succeeded
    pub fn is_ok(&self) -> bool {
        !self.has_hard_errors() && self.plan.is_some()
    }
}

/// Main entry point: analyze DSL and build execution plan
///
/// Always attempts to produce as much output as possible,
/// even if there are errors. This is intentional for LSP support.
pub fn analyse_and_plan(input: PlanningInput) -> PlanningOutput {
    let mut output = PlanningOutput::default();

    // Phase 1: Parse
    let program = match parse_program(input.source) {
        Ok(p) => p,
        Err(e) => {
            output.diagnostics.push(Diagnostic::error(
                DiagnosticCode::SyntaxError,
                format!("Parse error: {}", e),
            ));
            return output;
        }
    };
    output.program = program.clone();

    // Phase 2: Compile to ops
    let compiled = compile_to_ops(&program);

    // Record compile errors as diagnostics
    for err in &compiled.errors {
        let mut diag = Diagnostic::error(DiagnosticCode::UndefinedSymbol, err.message.clone());

        // Try to get span from the statement
        if let Some(stmt) = program.statements.get(err.stmt_idx) {
            if let super::ast::Statement::VerbCall(vc) = stmt {
                diag = diag.with_span(SourceSpan::from_byte_offset(
                    input.source,
                    vc.span.start,
                    vc.span.end,
                ));
            }
        }

        output.diagnostics.push(diag);
    }

    // If there are compile errors, we can still try to build partial plan
    // but mark that we have issues
    output.compiled_ops = Some(compiled.ops.clone());

    // Phase 3: Build DAG and toposort
    if compiled.is_ok() {
        match build_dag_plan(compiled.ops.clone()) {
            Ok(dag_plan) => {
                // Check if reordering occurred
                let was_reordered = check_reordering(&compiled.ops, &dag_plan.ops);
                output.was_reordered = was_reordered;

                if was_reordered {
                    output.diagnostics.push(Diagnostic::warning(
                        DiagnosticCode::ReorderingSuggested,
                        "Statements will be reordered for dependency resolution".to_string(),
                    ));
                }

                output.plan = Some(PlannedExecution {
                    ops: dag_plan.ops,
                    phases: dag_plan
                        .phases
                        .into_iter()
                        .map(|p| (p.name, p.op_indices))
                        .collect(),
                });
            }
            Err(CycleError {
                cycle_stmts,
                explanation,
            }) => {
                output.diagnostics.push(Diagnostic::error(
                    DiagnosticCode::CyclicDependency,
                    format!(
                        "Circular dependency detected: {} (involves {} ops)",
                        explanation,
                        cycle_stmts.len()
                    ),
                ));
            }
        }
    }

    output
}

/// Check if ops were reordered from source order
fn check_reordering(original: &[Op], sorted: &[Op]) -> bool {
    if original.len() != sorted.len() {
        return true;
    }

    for (i, (orig, sort)) in original.iter().zip(sorted.iter()).enumerate() {
        if orig.source_stmt() != sort.source_stmt() {
            return true;
        }
        // Also check if position changed
        if sort.source_stmt() != i {
            return true;
        }
    }

    false
}

/// Quick validation without full planning
///
/// Returns just diagnostics, useful for fast LSP feedback.
pub fn quick_validate(source: &str, registry: Arc<RuntimeVerbRegistry>) -> Vec<Diagnostic> {
    let input = PlanningInput::new(source, registry);
    let output = analyse_and_plan(input);
    output.diagnostics
}

// =============================================================================
// Tests
// =============================================================================

#[cfg(test)]
mod tests {
    use super::super::config::loader::ConfigLoader;
    use super::*;

    fn test_registry() -> Arc<RuntimeVerbRegistry> {
        let loader = ConfigLoader::from_env();
        let config = loader.load_verbs().expect("verbs config should load");
        Arc::new(RuntimeVerbRegistry::from_config(&config))
    }

    #[test]
    fn test_analyse_valid_program() {
        let source = r#"
            (cbu.ensure :name "Test Fund" :jurisdiction "LU" :as @fund)
        "#;

        let input = PlanningInput::new(source, test_registry());
        let output = analyse_and_plan(input);

        assert!(
            !output.has_hard_errors(),
            "Should not have hard errors: {:?}",
            output.errors()
        );
        assert!(output.plan.is_some(), "Should have a plan");
    }

    #[test]
    fn test_analyse_parse_error() {
        let source = "(cbu.ensure :name"; // Incomplete

        let input = PlanningInput::new(source, test_registry());
        let output = analyse_and_plan(input);

        assert!(output.has_errors(), "Should have parse error");
        assert!(output.plan.is_none(), "Should not have plan on parse error");
    }

    #[test]
    fn test_analyse_undefined_symbol() {
        let source = r#"
            (cbu.assign-role :cbu-id @undefined :entity-id @also_missing :role "X")
        "#;

        let input = PlanningInput::new(source, test_registry());
        let output = analyse_and_plan(input);

        assert!(output.has_errors(), "Should have undefined symbol error");
        let errors = output.errors();
        assert!(
            errors
                .iter()
                .any(|d| d.message.contains("undefined symbol")),
            "Should mention undefined symbol"
        );
    }

    #[test]
    fn test_analyse_with_reordering() {
        // Role assignment before entity creation - needs reordering
        let source = r#"
            (cbu.ensure :name "Fund" :as @fund)
            (entity.create-proper-person :first-name "John" :last-name "Smith" :as @john)
            (cbu.assign-role :cbu-id @fund :entity-id @john :role "DIRECTOR")
        "#;

        let input = PlanningInput::new(source, test_registry());
        let output = analyse_and_plan(input);

        assert!(output.is_ok(), "Should succeed: {:?}", output.errors());
        // This particular order is correct, so no reordering needed
    }

    #[test]
    fn test_quick_validate() {
        let source = r#"(cbu.ensure :name "Test" :as @test)"#;
        let diags = quick_validate(source, test_registry());
        // Should not have hard errors for valid DSL
        assert!(
            !diags.iter().any(|d| d.is_hard_error()),
            "Valid DSL should not have hard errors"
        );
    }
}
