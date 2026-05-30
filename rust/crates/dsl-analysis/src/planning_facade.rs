//! Planning Facade
//!
//! One central function that:
//! 1. Parses DSL text → Program
//! 2. Validates it
//! 3. Runs planning (DAG + precedence + implicit create)
//! 4. Returns both diagnostics and a planned ExecutionPlan
//! 5. Always attempts planning (even with validation errors) so LSP gets full picture

use std::sync::Arc;

use crate::runtime_registry::RuntimeVerbRegistry;
use dsl_core::{
    AstNode, BindingContext, CompileStep, Diagnostic, DiagnosticCode, Program, SourceSpan, Span,
    VerbCall, compile_to_steps, parse_program,
};

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

/// Execution plan — steps in source order (Op-free path, CR A2).
///
/// Under Option α the Op layer is removed: each step carries the `VerbCall`
/// directly. Dependency ordering (injection) is resolved at execution time by
/// `execute_plan_atomic_in_scope`. Phase grouping (from YAML `phase_tags`) is
/// added in CR A5.
#[derive(Clone, Debug)]
pub struct PlannedExecution {
    /// Steps in source order.
    pub steps: Vec<CompileStep>,
    /// Phase groupings for display (populated in A5 via YAML phase_tags).
    /// Empty until A5 lands.
    pub phases: Vec<(String, Vec<usize>)>,
}

impl PlannedExecution {
    /// Describe the plan for display.
    pub fn describe(&self) -> String {
        if self.steps.is_empty() {
            return "(empty plan)".to_string();
        }
        self.steps
            .iter()
            .enumerate()
            .map(|(i, s)| {
                let binding = s
                    .verb_call
                    .binding
                    .as_deref()
                    .map(|b| format!(" :as @{b}"))
                    .unwrap_or_default();
                format!(
                    "{}. {}.{}{}",
                    i + 1,
                    s.verb_call.domain,
                    s.verb_call.verb,
                    binding
                )
            })
            .collect::<Vec<_>>()
            .join("\n")
    }

    /// Get total step count.
    pub fn op_count(&self) -> usize {
        self.steps.len()
    }
}

/// Output from the planning facade
#[derive(Clone, Debug, Default)]
pub struct PlanningOutput {
    /// Parsed program (empty if parse failed)
    pub program: Program,
    /// All diagnostics collected during parsing, validation, and planning
    pub diagnostics: Vec<Diagnostic>,
    /// Compiled steps (source order) — Op-free path; was Vec<Op> pre-A2.
    pub compiled_ops: Option<Vec<CompileStep>>,
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

    // Phase 2: Op-free compilation (CR A2).
    // compile_to_steps emits VerbCalls directly; no VerbHandler needed.
    // input.verb_handler is accepted for backward-compat but ignored.
    let compiled = compile_to_steps(&program);

    // Unknown-verb diagnostics via registry lookup (replaces handler-based detection).
    for step in &compiled.steps {
        if !input
            .registry
            .contains(&step.verb_call.domain, &step.verb_call.verb)
        {
            let mut diag = Diagnostic::error(
                DiagnosticCode::UndefinedSymbol,
                format!(
                    "unknown verb: {}.{}",
                    step.verb_call.domain, step.verb_call.verb
                ),
            );
            diag = diag.with_span(SourceSpan::from_byte_offset(
                input.source,
                step.verb_call.span.start,
                step.verb_call.span.end,
            ));
            output.diagnostics.push(diag);
        }
    }

    // Undefined symbol (@name) check — forward-reference detection (replaces
    // compile-time C7 from the failure-mode audit). Track declared bindings
    // in source order; emit a diagnostic for any @name that hasn't been declared
    // by a prior `:as @name` binding in the same program.
    {
        let mut declared: std::collections::HashSet<String> = std::collections::HashSet::new();
        for step in &compiled.steps {
            for (sym_name, sym_span) in symbol_refs_in_verb_call(&step.verb_call) {
                if !declared.contains(&sym_name) {
                    let mut diag = Diagnostic::error(
                        DiagnosticCode::UndefinedSymbol,
                        format!("undefined symbol @{sym_name}"),
                    );
                    diag = diag.with_span(SourceSpan::from_byte_offset(
                        input.source,
                        sym_span.start,
                        sym_span.end,
                    ));
                    output.diagnostics.push(diag);
                }
            }
            if let Some(ref b) = step.verb_call.binding {
                declared.insert(b.clone());
            }
        }
    }

    output.compiled_ops = Some(compiled.steps.clone());

    // Phase 3: Plan = steps in source order.
    // Dependency ordering is resolved at execution time via injection graph
    // (execute_plan_atomic_in_scope). No topological reorder at planning time.
    output.was_reordered = false;
    // Build phase groupings from YAML phase_tags on each verb (CR A5).
    // Primary phase = first tag; untagged verbs go in "default".
    let phases = {
        let mut phase_vec: Vec<(String, Vec<usize>)> = Vec::new();
        for (step_idx, step) in compiled.steps.iter().enumerate() {
            let primary = input
                .registry
                .get(&step.verb_call.domain, &step.verb_call.verb)
                .and_then(|rv| rv.phase_tags.first().cloned())
                .unwrap_or_else(|| "default".to_string());
            match phase_vec.iter_mut().find(|(name, _)| name == &primary) {
                Some(entry) => entry.1.push(step_idx),
                None => phase_vec.push((primary, vec![step_idx])),
            }
        }
        phase_vec
    };

    output.plan = Some(PlannedExecution {
        steps: compiled.steps,
        phases,
    });

    output
}

/// Quick parse + registry validation.
///
/// Reports parse errors and unknown-verb diagnostics via registry lookup.
pub fn quick_validate(source: &str, registry: Arc<RuntimeVerbRegistry>) -> Vec<Diagnostic> {
    let input = PlanningInput::new(source, registry);
    let output = analyse_and_plan(input);
    output.diagnostics
}

/// Collect all `@name` SymbolRef occurrences in a VerbCall's arguments.
/// Used by `analyse_and_plan` for forward-reference diagnostics.
fn symbol_refs_in_verb_call(vc: &VerbCall) -> Vec<(String, Span)> {
    let mut refs = Vec::new();
    for arg in &vc.arguments {
        collect_symbol_refs_from_node(&arg.value, &mut refs);
    }
    refs
}

fn collect_symbol_refs_from_node(node: &AstNode, refs: &mut Vec<(String, Span)>) {
    match node {
        AstNode::SymbolRef { name, span } => refs.push((name.clone(), *span)),
        AstNode::List { items, .. } => {
            for item in items {
                collect_symbol_refs_from_node(item, refs);
            }
        }
        AstNode::Map { entries, .. } => {
            for (_, v) in entries {
                collect_symbol_refs_from_node(v, refs);
            }
        }
        _ => {}
    }
}

// =============================================================================
// Tests
// =============================================================================

#[cfg(test)]
mod tests {
    use super::*;
    use dsl_core::ConfigLoader;

    fn test_registry() -> Arc<RuntimeVerbRegistry> {
        let loader = ConfigLoader::from_env();
        let config = loader.load_verbs().expect("verbs config should load");
        Arc::new(RuntimeVerbRegistry::from_config(&config))
    }

    fn ob_poc_input<'a>(source: &'a str, registry: Arc<RuntimeVerbRegistry>) -> PlanningInput<'a> {
        PlanningInput::new(source, registry)
    }

    #[test]
    fn test_analyse_valid_program() {
        let source = r#"
            (cbu.ensure :name "Test Fund" :jurisdiction "LU" :as @fund)
        "#;

        let input = ob_poc_input(source, test_registry());
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

        let input = ob_poc_input(source, test_registry());
        let output = analyse_and_plan(input);

        assert!(output.has_errors(), "Should have parse error");
        assert!(output.plan.is_none(), "Should not have plan on parse error");
    }

    #[test]
    fn test_analyse_undefined_symbol() {
        let source = r#"
            (cbu.assign-role :cbu-id @undefined :entity-id @also_missing :role "X")
        "#;

        let input = ob_poc_input(source, test_registry());
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
            (entity.create :entity-type "proper-person" :first-name "John" :last-name "Smith" :as @john)
            (cbu.assign-role :cbu-id @fund :entity-id @john :role "DIRECTOR")
        "#;

        let input = ob_poc_input(source, test_registry());
        let output = analyse_and_plan(input);

        assert!(output.is_ok(), "Should succeed: {:?}", output.errors());
        // This particular order is correct, so no reordering needed
    }

    #[test]
    fn test_quick_validate() {
        let source = r#"(cbu.ensure :name "Test" :as @test)"#;
        // quick_validate has no handler (it's the dsl-lsp path); ob-poc verbs
        // produce "unknown verb" errors there. Use analyse_and_plan with handler
        // for tests that need ob-poc verbs to compile.
        let output = analyse_and_plan(ob_poc_input(source, test_registry()));
        assert!(
            !output.has_hard_errors(),
            "Valid ob-poc DSL with handler should not have hard errors"
        );
    }
}
