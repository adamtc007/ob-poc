//! Execution Plan - Compiled representation of DSL for dependency-ordered execution
//!
//! The DSL compiler transforms a declarative AST (tree) into an ExecutionPlan (linear)
//! that respects data dependencies. This is analogous to:
//! - Lisp's evaluation order with let-bindings
//! - SQL query planner's CTE ordering
//! - Stack-based compile-time analysis
//!
//! # Architecture
//!
//! ```text
//! DSL Source (declarative)     AST (tree)           Plan (linear)
//! ─────────────────────────    ──────────────       ─────────────────
//! (cbu.create :name "Fund"  →  VerbCall {        →  Step 0: cbu.create → $0
//!   :roles [                     children: [        Step 1: assign-role($0, aviva)
//!     (cbu.assign-role ...)      VerbCall...        Step 2: assign-role($0, bob)
//!   ])                         ]
//!                            }
//! ```
//!
//! # Planning
//!
//! The `compile_with_planning` function extends basic compilation with:
//! - Missing producer detection (unbound @refs)
//! - Synthetic step injection (auto-create entities when allowed)
//! - Lifecycle-aware ordering (state transitions respected)
//!
//! This is driven by config from:
//! - `config/ontology/entity_taxonomy.yaml` - Entity definitions, lifecycles, FK relationships
//! - `config/verbs.yaml` - Verb lifecycle constraints

use super::ast::{Argument, AstNode, Program, Span, Statement, VerbCall};
use super::runtime_registry::runtime_registry;
use super::verb_registry::{registry, VerbBehavior};
use crate::ontology::ontology;
use std::collections::{HashMap, HashSet};

/// A compiled execution plan - dependency sorted sequence of steps
#[derive(Debug, Clone)]
pub struct ExecutionPlan {
    pub steps: Vec<ExecutionStep>,
}

/// A single step in the execution plan
#[derive(Debug, Clone)]
pub struct ExecutionStep {
    /// The verb call to execute (with nested children removed)
    pub verb_call: VerbCall,

    /// Values to inject from previous steps' results
    pub injections: Vec<Injection>,

    /// Optional symbol binding (from :as @name syntax)
    pub bind_as: Option<String>,

    /// Step index (for debugging/logging)
    pub step_index: usize,

    /// How this step should be executed (CRUD, CustomOp, Composite)
    pub behavior: VerbBehavior,

    /// For custom ops, the handler ID (e.g., "document.catalog")
    pub custom_op_id: Option<String>,
}

/// Instruction to inject a previous step's result into this step's arguments
#[derive(Debug, Clone)]
pub struct Injection {
    /// Index of the step that produces the value
    pub from_step: usize,

    /// Argument key to inject into (e.g., "cbu-id")
    pub into_arg: String,
}

/// Compilation errors
#[derive(Debug, Clone)]
pub enum CompileError {
    /// Verb not found in registry
    UnknownVerb {
        domain: String,
        verb: String,
        suggestions: Vec<String>,
    },

    /// Circular dependency detected
    CircularDependency { steps: Vec<usize> },

    /// Cannot determine parent FK for nested verb
    UnknownParentRelation {
        parent_domain: String,
        child_domain: String,
    },
}

impl std::fmt::Display for CompileError {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            CompileError::UnknownVerb {
                domain,
                verb,
                suggestions,
            } => {
                write!(f, "Unknown verb: {}.{}", domain, verb)?;
                if !suggestions.is_empty() {
                    write!(f, "\n  Did you mean: {}?", suggestions[0])?;
                    write!(f, "\n  Available verbs: {}", suggestions.join(", "))?;
                }
                Ok(())
            }
            CompileError::CircularDependency { steps } => {
                write!(f, "Circular dependency between steps: {:?}", steps)
            }
            CompileError::UnknownParentRelation {
                parent_domain,
                child_domain,
            } => {
                write!(
                    f,
                    "Cannot determine FK relation: {} → {}",
                    parent_domain, child_domain
                )
            }
        }
    }
}

impl std::error::Error for CompileError {}

// ============================================================================
// Planning Types
// ============================================================================

/// Context for the planning pass - tracks available bindings from session/environment
#[derive(Debug, Clone, Default)]
pub struct PlanningContext {
    /// Bindings available from session context (e.g., @last_cbu)
    available_bindings: HashMap<String, BindingInfo>,
}

/// Information about an available binding
#[derive(Debug, Clone)]
pub struct BindingInfo {
    /// The type of entity this binding refers to
    pub entity_type: String,
    /// Optional subtype (for entities with subtypes)
    pub subtype: Option<String>,
    /// Current state (for lifecycle tracking)
    pub state: Option<String>,
}

impl PlanningContext {
    /// Create empty context
    pub fn new() -> Self {
        Self::default()
    }

    /// Add a binding to the context
    pub fn add_binding(&mut self, name: &str, entity_type: &str) {
        self.available_bindings.insert(
            name.to_string(),
            BindingInfo {
                entity_type: entity_type.to_string(),
                subtype: None,
                state: None,
            },
        );
    }

    /// Add a binding with full info
    pub fn add_binding_info(&mut self, name: &str, info: BindingInfo) {
        self.available_bindings.insert(name.to_string(), info);
    }

    /// Check if a binding is available
    pub fn has_binding(&self, name: &str) -> bool {
        self.available_bindings.contains_key(name)
    }

    /// Get binding info
    pub fn get_binding(&self, name: &str) -> Option<&BindingInfo> {
        self.available_bindings.get(name)
    }
}

/// Result of compile_with_planning
#[derive(Debug, Clone)]
pub struct PlanningResult {
    /// The compiled execution plan
    pub plan: ExecutionPlan,
    /// Synthetic steps that were injected
    pub synthetic_steps: Vec<SyntheticStep>,
    /// Whether the plan was reordered for lifecycle compliance
    pub reordered: bool,
    /// Diagnostics (warnings and errors)
    pub diagnostics: Vec<PlannerDiagnostic>,
}

/// A synthetic step that was injected by the planner
#[derive(Debug, Clone)]
pub struct SyntheticStep {
    /// The binding this step produces
    pub binding: String,
    /// The verb that was injected (e.g., "cbu.create")
    pub verb: String,
    /// Entity type being created
    pub entity_type: String,
    /// Index in the final plan
    pub plan_index: usize,
}

/// Diagnostic message from the planner
#[derive(Debug, Clone)]
pub enum PlannerDiagnostic {
    /// A synthetic step was injected to create a missing binding
    SyntheticStepInjected {
        binding: String,
        verb: String,
        entity_type: String,
        before_stmt: usize,
    },

    /// A binding is referenced but not produced and cannot be auto-created
    MissingProducer {
        binding: String,
        entity_type: String,
        required_by_stmt: usize,
        reason: String,
    },

    /// Lifecycle state violation detected
    LifecycleViolation {
        binding: String,
        verb: String,
        current_state: String,
        required_states: Vec<String>,
        stmt_index: usize,
    },

    /// Statements were reordered for lifecycle compliance
    StatementsReordered {
        original_order: Vec<usize>,
        new_order: Vec<usize>,
        reason: String,
    },

    /// Warning about potential issue (non-blocking)
    Warning {
        message: String,
        stmt_index: Option<usize>,
    },
}

impl std::fmt::Display for PlannerDiagnostic {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            PlannerDiagnostic::SyntheticStepInjected {
                binding,
                verb,
                entity_type,
                before_stmt,
            } => {
                write!(
                    f,
                    "Injected synthetic {} to create @{} ({}) before statement {}",
                    verb, binding, entity_type, before_stmt
                )
            }
            PlannerDiagnostic::MissingProducer {
                binding,
                entity_type,
                required_by_stmt,
                reason,
            } => {
                write!(
                    f,
                    "Missing producer for @{} ({}) required by statement {}: {}",
                    binding, entity_type, required_by_stmt, reason
                )
            }
            PlannerDiagnostic::LifecycleViolation {
                binding,
                verb,
                current_state,
                required_states,
                stmt_index,
            } => {
                write!(
                    f,
                    "Lifecycle violation at statement {}: {} requires @{} in states {:?}, but current state is {}",
                    stmt_index, verb, binding, required_states, current_state
                )
            }
            PlannerDiagnostic::StatementsReordered {
                original_order,
                new_order,
                reason,
            } => {
                write!(
                    f,
                    "Statements reordered from {:?} to {:?}: {}",
                    original_order, new_order, reason
                )
            }
            PlannerDiagnostic::Warning {
                message,
                stmt_index,
            } => {
                if let Some(idx) = stmt_index {
                    write!(f, "Warning at statement {}: {}", idx, message)
                } else {
                    write!(f, "Warning: {}", message)
                }
            }
        }
    }
}

// ============================================================================
// Parent-Child FK Relationships
// ============================================================================

/// Look up which argument a child verb needs from its parent.
///
/// This now delegates to the ontology service which loads relationships from
/// `config/ontology/entity_taxonomy.yaml`. The relationships section defines:
/// - Same-domain self-references (cbu -> cbu uses cbu-id)
/// - Cross-domain relationships (cbu -> document uses cbu-id)
///
/// Falls back to a simple heuristic if ontology is not loaded.
fn infer_parent_fk(parent_domain: &str, child_domain: &str) -> Option<&'static str> {
    use crate::ontology::ontology;

    // Try ontology lookup first
    if let Some(fk) = ontology().get_fk(parent_domain, child_domain) {
        // The ontology returns a &str with the same lifetime as the service,
        // which is 'static since it's a global singleton.
        // SAFETY: The ontology service is a static singleton, so the returned
        // string slice has 'static lifetime.
        return Some(unsafe { std::mem::transmute::<&str, &'static str>(fk) });
    }

    // Fallback heuristic: if same domain, use "{domain}-id"
    if parent_domain == child_domain {
        return match parent_domain {
            "cbu" => Some("cbu-id"),
            "entity" => Some("entity-id"),
            "document" => Some("document-id"),
            "investigation" => Some("investigation-id"),
            "decision" => Some("decision-id"),
            "screening" => Some("screening-id"),
            "product" => Some("product-id"),
            "service" => Some("service-id"),
            "monitoring" => Some("monitoring-id"),
            "risk" => Some("risk-id"),
            "kyc" => Some("case-id"),
            "workstream" => Some("workstream-id"),
            _ => None,
        };
    }

    None
}

// ============================================================================
// Compiler
// ============================================================================

/// Minimum similarity threshold for verb suggestions
const VERB_SIMILARITY_THRESHOLD: f64 = 0.5;

/// Get verb suggestions for an unknown verb using Jaro-Winkler similarity
fn get_verb_suggestions(domain: &str, verb: &str) -> Vec<String> {
    let reg = registry();

    // First, check if the domain exists and get verbs from that domain
    let domain_verbs: Vec<String> = reg
        .verbs_for_domain(domain)
        .into_iter()
        .map(|v| format!("{}.{}", v.domain, v.verb))
        .collect();

    // If domain exists, suggest verbs from that domain
    if !domain_verbs.is_empty() {
        // Sort by Jaro-Winkler similarity (higher is better)
        let mut scored: Vec<(String, f64)> = domain_verbs
            .into_iter()
            .map(|full_verb| {
                let v = full_verb.split('.').nth(1).unwrap_or("");
                let score = strsim::jaro_winkler(verb, v);
                (full_verb, score)
            })
            .filter(|(_, score)| *score >= VERB_SIMILARITY_THRESHOLD)
            .collect();
        scored.sort_by(|a, b| b.1.partial_cmp(&a.1).unwrap_or(std::cmp::Ordering::Equal));
        return scored.into_iter().take(5).map(|(v, _)| v).collect();
    }

    // Domain doesn't exist - suggest similar verbs from all domains
    let full_verb = format!("{}.{}", domain, verb);

    let mut scored: Vec<(String, f64)> = reg
        .all_verbs()
        .map(|v| {
            let full = format!("{}.{}", v.domain, v.verb);
            let score = strsim::jaro_winkler(&full_verb, &full);
            (full, score)
        })
        .filter(|(_, score)| *score >= VERB_SIMILARITY_THRESHOLD)
        .collect();
    scored.sort_by(|a, b| b.1.partial_cmp(&a.1).unwrap_or(std::cmp::Ordering::Equal));
    scored.into_iter().take(5).map(|(v, _)| v).collect()
}

/// Compile an AST into an execution plan
///
/// The compiler performs:
/// 1. Collect all top-level statements
/// 2. Build dependency graph from @reference usage
/// 3. Topologically sort to ensure definitions come before uses
/// 4. Detect circular dependencies
pub fn compile(program: &Program) -> Result<ExecutionPlan, CompileError> {
    // Collect top-level verb calls (ignore comments)
    let verb_calls: Vec<&VerbCall> = program
        .statements
        .iter()
        .filter_map(|s| match s {
            Statement::VerbCall(vc) => Some(vc),
            Statement::Comment(_) => None,
        })
        .collect();

    if verb_calls.is_empty() {
        return Ok(ExecutionPlan { steps: Vec::new() });
    }

    // Build dependency graph and topologically sort
    let sorted_indices = topological_sort(&verb_calls)?;

    // Compile in sorted order
    let mut compiler = Compiler::new();
    for idx in sorted_indices {
        compiler.compile_verb_call(verb_calls[idx], None)?;
    }

    Ok(ExecutionPlan {
        steps: compiler.steps,
    })
}

// ============================================================================
// Planning-Aware Compilation
// ============================================================================

/// Extended compilation with planning pass
///
/// This function adds intelligent planning capabilities to the basic compile:
/// 1. Detects missing producers (unbound @refs)
/// 2. Injects synthetic create statements when allowed by ontology config
/// 3. Validates lifecycle constraints (future: Phase 5)
/// 4. Reorders statements for lifecycle compliance (future: Phase 5)
///
/// # Arguments
/// * `program` - The AST to compile
/// * `context` - Planning context with available bindings from session
///
/// # Returns
/// * `PlanningResult` containing the plan, synthetic steps, and diagnostics
pub fn compile_with_planning(
    program: &Program,
    context: &PlanningContext,
) -> Result<PlanningResult, CompileError> {
    // Phase 1: Collect verb calls
    let verb_calls: Vec<&VerbCall> = program
        .statements
        .iter()
        .filter_map(|s| match s {
            Statement::VerbCall(vc) => Some(vc),
            Statement::Comment(_) => None,
        })
        .collect();

    if verb_calls.is_empty() {
        return Ok(PlanningResult {
            plan: ExecutionPlan { steps: Vec::new() },
            synthetic_steps: Vec::new(),
            reordered: false,
            diagnostics: Vec::new(),
        });
    }

    // Phase 2: Analyze bindings - what's produced and what's consumed
    let mut binding_producers: HashMap<String, (usize, String)> = HashMap::new(); // name -> (stmt_idx, entity_type)
    let mut required_bindings: Vec<RequiredBinding> = Vec::new();

    for (idx, vc) in verb_calls.iter().enumerate() {
        // Record what this verb produces
        if let Some(ref binding) = vc.binding {
            let entity_type = get_produced_type(vc);
            binding_producers.insert(binding.clone(), (idx, entity_type));
        }

        // Record what this verb consumes
        collect_consumed_bindings(vc, idx, &mut required_bindings);
    }

    // Phase 3: Detect missing producers and inject synthetic creates
    let mut synthetic_statements: Vec<Statement> = Vec::new();
    let mut diagnostics: Vec<PlannerDiagnostic> = Vec::new();
    let mut synthetic_steps: Vec<SyntheticStep> = Vec::new();
    let mut injected_bindings: HashSet<String> = HashSet::new();

    for req in &required_bindings {
        // Skip if already produced by a statement
        if binding_producers.contains_key(&req.binding) {
            continue;
        }

        // Skip if already injected
        if injected_bindings.contains(&req.binding) {
            continue;
        }

        // Check if it's in session context
        if context.has_binding(&req.binding) {
            continue;
        }

        // Try to inject synthetic create
        let resolved_type = ontology().resolve_alias(&req.entity_type);

        if ontology().allows_implicit_create(resolved_type) {
            if let Some(creator_verb) =
                ontology().canonical_creator(resolved_type, req.subtype.as_deref())
            {
                // Create synthetic statement
                let synthetic = create_synthetic_verb_call(&creator_verb, &req.binding);

                diagnostics.push(PlannerDiagnostic::SyntheticStepInjected {
                    binding: req.binding.clone(),
                    verb: creator_verb.clone(),
                    entity_type: resolved_type.to_string(),
                    before_stmt: req.required_by_stmt,
                });

                synthetic_steps.push(SyntheticStep {
                    binding: req.binding.clone(),
                    verb: creator_verb,
                    entity_type: resolved_type.to_string(),
                    plan_index: synthetic_statements.len(), // Will be adjusted after merge
                });

                synthetic_statements.push(Statement::VerbCall(synthetic));
                injected_bindings.insert(req.binding.clone());
            } else {
                // No canonical creator available
                diagnostics.push(PlannerDiagnostic::MissingProducer {
                    binding: req.binding.clone(),
                    entity_type: req.entity_type.clone(),
                    required_by_stmt: req.required_by_stmt,
                    reason: "No canonical creator verb defined in ontology".to_string(),
                });
            }
        } else {
            // Implicit create not allowed
            diagnostics.push(PlannerDiagnostic::MissingProducer {
                binding: req.binding.clone(),
                entity_type: req.entity_type.clone(),
                required_by_stmt: req.required_by_stmt,
                reason: format!(
                    "Implicit creation not allowed for entity type '{}'. \
                     Add an explicit create statement or provide the binding in session context.",
                    resolved_type
                ),
            });
        }
    }

    // Phase 4: Merge synthetic statements with original program
    let merged_program = if synthetic_statements.is_empty() {
        program.clone()
    } else {
        let mut all_statements = synthetic_statements;
        all_statements.extend(program.statements.iter().cloned());
        Program {
            statements: all_statements,
        }
    };

    // Phase 5: Compile the merged program using standard compilation
    // (Lifecycle-aware topological sort will be added in Phase 5)
    let plan = compile(&merged_program)?;

    // Update synthetic step indices to match final plan
    for (i, step) in synthetic_steps.iter_mut().enumerate() {
        step.plan_index = i; // Synthetic steps are at the front
    }

    Ok(PlanningResult {
        plan,
        synthetic_steps,
        reordered: false, // Will be set by lifecycle-aware sort in Phase 5
        diagnostics,
    })
}

/// Information about a required binding
struct RequiredBinding {
    /// The binding name (e.g., "fund")
    binding: String,
    /// The expected entity type (e.g., "cbu")
    entity_type: String,
    /// Optional subtype for entities (e.g., "proper_person")
    subtype: Option<String>,
    /// Which statement requires this binding
    required_by_stmt: usize,
    /// Which argument references this binding
    #[allow(dead_code)]
    arg_name: String,
}

/// Get the entity type that a verb produces
fn get_produced_type(vc: &VerbCall) -> String {
    // Check runtime registry for produces config
    if let Some(verb_def) = runtime_registry().get(&vc.domain, &vc.verb) {
        if let Some(ref produces) = verb_def.produces {
            return produces.produced_type.clone();
        }
    }

    // Fallback: use domain as type
    vc.domain.clone()
}

/// Collect all bindings consumed by a verb call
fn collect_consumed_bindings(vc: &VerbCall, stmt_idx: usize, out: &mut Vec<RequiredBinding>) {
    // Check runtime registry for consumes config
    if let Some(verb_def) = runtime_registry().get(&vc.domain, &vc.verb) {
        for consume in &verb_def.consumes {
            // Find the argument that matches this consume
            if let Some(arg) = vc.arguments.iter().find(|a| a.key == consume.arg) {
                // Check if it's a symbol reference
                collect_symbol_refs_with_type(
                    &arg.value,
                    &consume.consumed_type,
                    None,
                    stmt_idx,
                    &consume.arg,
                    out,
                );
            }
        }
    }

    // Also collect any symbol refs that aren't covered by consumes config
    for arg in &vc.arguments {
        collect_untyped_symbol_refs(&arg.value, stmt_idx, &arg.key, out);
    }
}

/// Collect symbol refs with known type from consumes config
fn collect_symbol_refs_with_type(
    node: &AstNode,
    entity_type: &str,
    subtype: Option<&str>,
    stmt_idx: usize,
    arg_name: &str,
    out: &mut Vec<RequiredBinding>,
) {
    match node {
        AstNode::SymbolRef { name, .. } => {
            // Check if we already have this binding
            if !out.iter().any(|r| r.binding == *name) {
                out.push(RequiredBinding {
                    binding: name.clone(),
                    entity_type: entity_type.to_string(),
                    subtype: subtype.map(String::from),
                    required_by_stmt: stmt_idx,
                    arg_name: arg_name.to_string(),
                });
            }
        }
        AstNode::List { items, .. } => {
            for item in items {
                collect_symbol_refs_with_type(item, entity_type, subtype, stmt_idx, arg_name, out);
            }
        }
        _ => {}
    }
}

/// Collect symbol refs without type info (for bindings not in consumes config)
fn collect_untyped_symbol_refs(
    node: &AstNode,
    stmt_idx: usize,
    arg_name: &str,
    out: &mut Vec<RequiredBinding>,
) {
    match node {
        AstNode::SymbolRef { name, .. } => {
            // Only add if we don't already have this binding with a known type
            if !out.iter().any(|r| r.binding == *name) {
                // Try to infer type from arg name pattern
                let entity_type = infer_entity_type_from_arg(arg_name);
                out.push(RequiredBinding {
                    binding: name.clone(),
                    entity_type,
                    subtype: None,
                    required_by_stmt: stmt_idx,
                    arg_name: arg_name.to_string(),
                });
            }
        }
        AstNode::List { items, .. } => {
            for item in items {
                collect_untyped_symbol_refs(item, stmt_idx, arg_name, out);
            }
        }
        AstNode::Map { entries, .. } => {
            for (_, v) in entries {
                collect_untyped_symbol_refs(v, stmt_idx, arg_name, out);
            }
        }
        AstNode::Nested(nested_vc) => {
            for arg in &nested_vc.arguments {
                collect_untyped_symbol_refs(&arg.value, stmt_idx, &arg.key, out);
            }
        }
        _ => {}
    }
}

/// Infer entity type from argument name
fn infer_entity_type_from_arg(arg_name: &str) -> String {
    match arg_name {
        "cbu-id" => "cbu".to_string(),
        "entity-id" | "owner-entity-id" | "person-entity-id" | "subject-entity-id" => {
            "entity".to_string()
        }
        "case-id" => "kyc_case".to_string(),
        "workstream-id" => "kyc_workstream".to_string(),
        "document-id" | "doc-id" => "document".to_string(),
        "product-id" => "product".to_string(),
        "service-id" => "service".to_string(),
        "instance-id" => "cbu_resource_instance".to_string(),
        _ => "unknown".to_string(),
    }
}

/// Create a synthetic verb call for implicit entity creation
///
/// The synthetic call has minimal arguments - just the binding.
/// Required args will need to be provided by the user or inferred from context.
fn create_synthetic_verb_call(creator_verb: &str, binding: &str) -> VerbCall {
    let parts: Vec<&str> = creator_verb.split('.').collect();
    let (domain, verb) = if parts.len() >= 2 {
        (parts[0], parts[1])
    } else {
        ("unknown", creator_verb)
    };

    VerbCall {
        domain: domain.to_string(),
        verb: verb.to_string(),
        arguments: vec![], // Minimal - user must fill in required args
        binding: Some(binding.to_string()),
        span: Span::synthetic(),
    }
}

// ============================================================================
// Topological Sort for @reference Dependencies
// ============================================================================

/// Extract symbol binding from a VerbCall (the :as @name part)
fn get_binding(vc: &VerbCall) -> Option<&str> {
    vc.binding.as_deref()
}

/// Extract all @references used in a VerbCall's arguments
fn get_references(vc: &VerbCall) -> Vec<&str> {
    let mut refs = Vec::new();
    for arg in &vc.arguments {
        collect_references_from_node(&arg.value, &mut refs);
    }
    refs
}

/// Recursively collect @references from an AstNode
fn collect_references_from_node<'a>(node: &'a AstNode, refs: &mut Vec<&'a str>) {
    match node {
        AstNode::SymbolRef { name, .. } => {
            refs.push(name.as_str());
        }
        AstNode::List { items, .. } => {
            for item in items {
                collect_references_from_node(item, refs);
            }
        }
        AstNode::Map { entries, .. } => {
            for (_, v) in entries {
                collect_references_from_node(v, refs);
            }
        }
        AstNode::Nested(nested_vc) => {
            // Nested calls also have their own references
            for arg in &nested_vc.arguments {
                collect_references_from_node(&arg.value, refs);
            }
        }
        // Other nodes don't contain references
        AstNode::Literal(_) | AstNode::EntityRef { .. } => {}
    }
}

/// Topologically sort statements based on @reference dependencies AND table-level dependencies
/// Returns indices in execution order, or error if circular dependency
fn topological_sort(verb_calls: &[&VerbCall]) -> Result<Vec<usize>, CompileError> {
    use std::collections::{HashMap, VecDeque};

    let n = verb_calls.len();
    let registry = runtime_registry();

    // Build symbol -> statement index map
    let mut symbol_to_idx: HashMap<&str, usize> = HashMap::new();
    for (idx, vc) in verb_calls.iter().enumerate() {
        if let Some(binding) = get_binding(vc) {
            symbol_to_idx.insert(binding, idx);
        }
    }

    // Track which statements write to which tables (for table-level dependency ordering)
    // Key: table name (e.g., "custody.cbu_ssi"), Value: statement indices that write to it
    let mut table_writers: HashMap<String, Vec<usize>> = HashMap::new();

    // First pass: record table writers
    for (idx, vc) in verb_calls.iter().enumerate() {
        if let Some(runtime_verb) = registry.get(&vc.domain, &vc.verb) {
            if let Some(ref lifecycle) = runtime_verb.lifecycle {
                for table in &lifecycle.writes_tables {
                    table_writers.entry(table.clone()).or_default().push(idx);
                }
            }
        }
    }

    // Build adjacency list (edges from dependency to dependent)
    // If statement B uses @foo defined by statement A, then A -> B
    let mut adj: Vec<Vec<usize>> = vec![Vec::new(); n];
    let mut in_degree: Vec<usize> = vec![0; n];

    for (idx, vc) in verb_calls.iter().enumerate() {
        // Symbol reference dependencies
        for ref_name in get_references(vc) {
            if let Some(&dep_idx) = symbol_to_idx.get(ref_name) {
                // dep_idx defines the symbol, idx uses it
                // So dep_idx must come before idx
                adj[dep_idx].push(idx);
                in_degree[idx] += 1;
            }
            // If reference not found in our statements, it might be:
            // - Pre-bound in context (e.g., @last_cbu)
            // - An error (caught at execution time)
        }

        // Table-level dependencies: reads_tables
        // If this verb reads from tables, it must come AFTER any verb that writes to those tables
        if let Some(runtime_verb) = registry.get(&vc.domain, &vc.verb) {
            if let Some(ref lifecycle) = runtime_verb.lifecycle {
                for table in &lifecycle.reads_tables {
                    if let Some(writer_indices) = table_writers.get(table) {
                        for &writer_idx in writer_indices {
                            if writer_idx != idx {
                                // writer_idx writes to the table, idx reads from it
                                // So writer_idx must come before idx
                                adj[writer_idx].push(idx);
                                in_degree[idx] += 1;
                            }
                        }
                    }
                }
            }
        }
    }

    // Kahn's algorithm for topological sort
    // Use VecDeque with pop_front to preserve original order for independent nodes
    let mut queue: VecDeque<usize> = VecDeque::new();
    for (idx, &deg) in in_degree.iter().enumerate() {
        if deg == 0 {
            queue.push_back(idx);
        }
    }

    let mut sorted: Vec<usize> = Vec::with_capacity(n);
    while let Some(u) = queue.pop_front() {
        sorted.push(u);
        for &v in &adj[u] {
            in_degree[v] -= 1;
            if in_degree[v] == 0 {
                queue.push_back(v);
            }
        }
    }

    // Check for circular dependency
    if sorted.len() != n {
        // Find the cycle - statements with remaining in_degree > 0
        let cycle_members: Vec<usize> = in_degree
            .iter()
            .enumerate()
            .filter(|(_, &deg)| deg > 0)
            .map(|(idx, _)| idx)
            .collect();

        return Err(CompileError::CircularDependency {
            steps: cycle_members,
        });
    }

    Ok(sorted)
}

/// Compiler state
struct Compiler {
    steps: Vec<ExecutionStep>,
}

impl Compiler {
    fn new() -> Self {
        Self { steps: Vec::new() }
    }

    /// Compile a verb call, potentially with nested children
    /// Returns the step index of this verb call
    fn compile_verb_call(
        &mut self,
        vc: &VerbCall,
        parent: Option<ParentInfo>,
    ) -> Result<usize, CompileError> {
        // Look up verb in unified registry (includes both CRUD and custom ops)
        let verb_def = registry().get(&vc.domain, &vc.verb).ok_or_else(|| {
            // Get suggestions for the unknown verb
            let suggestions = get_verb_suggestions(&vc.domain, &vc.verb);
            CompileError::UnknownVerb {
                domain: vc.domain.clone(),
                verb: vc.verb.clone(),
                suggestions,
            }
        })?;

        // Build injections from parent
        let mut injections = Vec::new();
        if let Some(parent_info) = &parent {
            let fk_arg = infer_parent_fk(&parent_info.domain, &vc.domain).ok_or_else(|| {
                CompileError::UnknownParentRelation {
                    parent_domain: parent_info.domain.clone(),
                    child_domain: vc.domain.clone(),
                }
            })?;

            injections.push(Injection {
                from_step: parent_info.step_index,
                into_arg: fk_arg.to_string(),
            });
        }

        // Extract nested children and create a "flattened" verb call
        let (flat_vc, nested_children) = extract_nested_children(vc);

        // Add this step
        let my_step_index = self.steps.len();
        self.steps.push(ExecutionStep {
            verb_call: flat_vc,
            injections,
            bind_as: vc.binding.clone(),
            step_index: my_step_index,
            behavior: verb_def.behavior,
            custom_op_id: verb_def.custom_op_id.clone(),
        });

        // Recursively compile children with this step as parent
        let parent_info = ParentInfo {
            domain: vc.domain.clone(),
            step_index: my_step_index,
        };

        for child_vc in nested_children {
            self.compile_verb_call(&child_vc, Some(parent_info.clone()))?;
        }

        Ok(my_step_index)
    }
}

/// Info about the parent verb call for injection
#[derive(Clone)]
struct ParentInfo {
    domain: String,
    step_index: usize,
}

// ============================================================================
// Nested Children Extraction
// ============================================================================

/// Extract nested VerbCalls from a verb call's arguments
/// Returns (flattened_verb_call, nested_children)
fn extract_nested_children(vc: &VerbCall) -> (VerbCall, Vec<VerbCall>) {
    let mut flat_args = Vec::new();
    let mut nested = Vec::new();

    for arg in &vc.arguments {
        match &arg.value {
            // List might contain nested verb calls
            AstNode::List { items, span } => {
                let mut flat_items = Vec::new();
                for item in items {
                    if let AstNode::Nested(child_vc) = item {
                        nested.push((**child_vc).clone());
                    } else {
                        flat_items.push(item.clone());
                    }
                }
                // Keep non-nested items in the list
                if !flat_items.is_empty() {
                    flat_args.push(Argument {
                        key: arg.key.clone(),
                        value: AstNode::List {
                            items: flat_items,
                            span: *span,
                        },
                        span: arg.span,
                    });
                }
                // If list was purely nested calls, we might want to track the key
                // for semantic purposes (e.g., :roles, :children)
            }
            // Single nested call
            AstNode::Nested(child_vc) => {
                nested.push((**child_vc).clone());
            }
            // Regular values pass through
            _ => {
                flat_args.push(arg.clone());
            }
        }
    }

    let flat_vc = VerbCall {
        domain: vc.domain.clone(),
        verb: vc.verb.clone(),
        arguments: flat_args,
        binding: vc.binding.clone(),
        span: vc.span,
    };

    (flat_vc, nested)
}

// ============================================================================
// Plan Inspection / Debug
// ============================================================================

impl ExecutionPlan {
    /// Pretty print the plan for debugging
    pub fn debug_print(&self) -> String {
        let mut out = String::new();
        out.push_str("=== Execution Plan ===\n");

        for step in &self.steps {
            out.push_str(&format!(
                "Step {}: {}.{}",
                step.step_index, step.verb_call.domain, step.verb_call.verb
            ));

            if let Some(ref binding) = step.bind_as {
                out.push_str(&format!(" → @{}", binding));
            }
            out.push('\n');

            // Show injections
            for inj in &step.injections {
                out.push_str(&format!(
                    "  ← inject ${} as :{}\n",
                    inj.from_step, inj.into_arg
                ));
            }

            // Show args
            for arg in &step.verb_call.arguments {
                out.push_str(&format!("  :{} = {:?}\n", arg.key, arg.value));
            }
        }

        out
    }

    /// Get execution order as domain.verb sequence (for logging)
    pub fn execution_sequence(&self) -> Vec<String> {
        self.steps
            .iter()
            .map(|s| format!("{}.{}", s.verb_call.domain, s.verb_call.verb))
            .collect()
    }

    /// Count of steps
    pub fn len(&self) -> usize {
        self.steps.len()
    }

    /// Check if empty
    pub fn is_empty(&self) -> bool {
        self.steps.is_empty()
    }
}

// ============================================================================
// Tests
// ============================================================================

#[cfg(test)]
mod tests {
    use super::*;
    use crate::dsl_v2::ast::{Literal, Span};

    fn make_verb_call(domain: &str, verb: &str, args: Vec<(&str, AstNode)>) -> VerbCall {
        VerbCall {
            domain: domain.to_string(),
            verb: verb.to_string(),
            arguments: args
                .into_iter()
                .map(|(k, v)| Argument {
                    key: k.to_string(),
                    value: v,
                    span: Span::default(),
                })
                .collect(),
            binding: None,
            span: Span::default(),
        }
    }

    #[test]
    fn test_compile_flat_program() {
        // Simple sequential DSL - no nesting
        let program = Program {
            statements: vec![
                Statement::VerbCall(make_verb_call(
                    "cbu",
                    "create",
                    vec![(
                        "name",
                        AstNode::Literal(Literal::String("Test Fund".into())),
                    )],
                )),
                Statement::VerbCall(make_verb_call(
                    "entity",
                    "read",
                    vec![(
                        "entity-id",
                        AstNode::Literal(Literal::String("some-uuid".into())),
                    )],
                )),
            ],
        };

        let plan = compile(&program).unwrap();

        assert_eq!(plan.len(), 2);
        assert!(plan.steps[0].injections.is_empty());
        assert!(plan.steps[1].injections.is_empty());

        let seq = plan.execution_sequence();
        assert_eq!(seq, vec!["cbu.create", "entity.read"]);
    }

    #[test]
    fn test_compile_nested_children() {
        // Nested DSL - CBU with role assignments
        let child1 = make_verb_call(
            "cbu",
            "assign-role",
            vec![
                (
                    "entity-id",
                    AstNode::Literal(Literal::String("entity-uuid-1".into())),
                ),
                ("role", AstNode::Literal(Literal::String("Manager".into()))),
            ],
        );
        let child2 = make_verb_call(
            "cbu",
            "assign-role",
            vec![
                (
                    "entity-id",
                    AstNode::Literal(Literal::String("entity-uuid-2".into())),
                ),
                ("role", AstNode::Literal(Literal::String("Director".into()))),
            ],
        );

        let parent = VerbCall {
            domain: "cbu".to_string(),
            verb: "create".to_string(),
            arguments: vec![
                Argument {
                    key: "name".into(),
                    value: AstNode::Literal(Literal::String("Test Fund".into())),
                    span: Span::default(),
                },
                Argument {
                    key: "roles".into(),
                    value: AstNode::List {
                        items: vec![
                            AstNode::Nested(Box::new(child1)),
                            AstNode::Nested(Box::new(child2)),
                        ],
                        span: Span::default(),
                    },
                    span: Span::default(),
                },
            ],
            binding: None,
            span: Span::default(),
        };

        let program = Program {
            statements: vec![Statement::VerbCall(parent)],
        };

        let plan = compile(&program).unwrap();

        // Should have 3 steps: create + 2 assign-role
        assert_eq!(plan.len(), 3);

        // First step (create) has no injections
        assert!(plan.steps[0].injections.is_empty());
        assert_eq!(plan.steps[0].verb_call.verb, "create");

        // Second step (assign-role) injects cbu-id from step 0
        assert_eq!(plan.steps[1].injections.len(), 1);
        assert_eq!(plan.steps[1].injections[0].from_step, 0);
        assert_eq!(plan.steps[1].injections[0].into_arg, "cbu-id");

        // Third step also injects from step 0
        assert_eq!(plan.steps[2].injections.len(), 1);
        assert_eq!(plan.steps[2].injections[0].from_step, 0);

        println!("{}", plan.debug_print());
    }

    #[test]
    fn test_infer_parent_fk() {
        assert_eq!(infer_parent_fk("cbu", "cbu"), Some("cbu-id"));
        assert_eq!(infer_parent_fk("cbu", "investigation"), Some("cbu-id"));
        assert_eq!(infer_parent_fk("entity", "document"), Some("entity-id"));
        assert_eq!(infer_parent_fk("unknown", "unknown"), None);
    }

    #[test]
    fn test_unknown_verb_error() {
        let program = Program {
            statements: vec![Statement::VerbCall(make_verb_call("fake", "verb", vec![]))],
        };

        let result = compile(&program);
        assert!(matches!(result, Err(CompileError::UnknownVerb { .. })));
    }

    #[test]
    fn test_product_read_compiles() {
        // product.read should compile successfully (product is read-only reference data)
        let program = Program {
            statements: vec![Statement::VerbCall(make_verb_call(
                "product",
                "read",
                vec![(
                    "product-id",
                    AstNode::Literal(Literal::String(
                        "00000000-0000-0000-0000-000000000001".into(),
                    )),
                )],
            ))],
        };

        let result = compile(&program);
        assert!(result.is_ok(), "product.read should compile: {:?}", result);
        let plan = result.unwrap();
        assert_eq!(plan.len(), 1);
        assert_eq!(plan.steps[0].verb_call.domain, "product");
        assert_eq!(plan.steps[0].verb_call.verb, "read");
    }

    // ========================================================================
    // Planning Tests
    // ========================================================================

    fn make_verb_call_with_binding(
        domain: &str,
        verb: &str,
        args: Vec<(&str, AstNode)>,
        binding: Option<&str>,
    ) -> VerbCall {
        VerbCall {
            domain: domain.to_string(),
            verb: verb.to_string(),
            arguments: args
                .into_iter()
                .map(|(k, v)| Argument {
                    key: k.to_string(),
                    value: v,
                    span: Span::default(),
                })
                .collect(),
            binding: binding.map(String::from),
            span: Span::default(),
        }
    }

    #[test]
    fn test_planning_context_bindings() {
        let mut ctx = PlanningContext::new();
        assert!(!ctx.has_binding("fund"));

        ctx.add_binding("fund", "cbu");
        assert!(ctx.has_binding("fund"));

        let info = ctx.get_binding("fund").unwrap();
        assert_eq!(info.entity_type, "cbu");
    }

    #[test]
    fn test_compile_with_planning_empty_program() {
        let program = Program { statements: vec![] };
        let ctx = PlanningContext::new();

        let result = compile_with_planning(&program, &ctx).unwrap();

        assert!(result.plan.is_empty());
        assert!(result.synthetic_steps.is_empty());
        assert!(result.diagnostics.is_empty());
        assert!(!result.reordered);
    }

    #[test]
    fn test_compile_with_planning_no_missing_bindings() {
        // Program where all bindings are produced
        let program = Program {
            statements: vec![
                Statement::VerbCall(make_verb_call_with_binding(
                    "cbu",
                    "create",
                    vec![("name", AstNode::Literal(Literal::String("Test".into())))],
                    Some("fund"),
                )),
                Statement::VerbCall(make_verb_call(
                    "cbu",
                    "assign-role",
                    vec![
                        (
                            "cbu-id",
                            AstNode::SymbolRef {
                                name: "fund".to_string(),
                                span: Span::default(),
                            },
                        ),
                        (
                            "entity-id",
                            AstNode::Literal(Literal::String("some-uuid".into())),
                        ),
                        ("role", AstNode::Literal(Literal::String("DIRECTOR".into()))),
                    ],
                )),
            ],
        };

        let ctx = PlanningContext::new();
        let result = compile_with_planning(&program, &ctx).unwrap();

        // No synthetic steps needed
        assert!(result.synthetic_steps.is_empty());
        // No missing producer diagnostics
        assert!(
            !result
                .diagnostics
                .iter()
                .any(|d| matches!(d, PlannerDiagnostic::MissingProducer { .. })),
            "Should not have missing producer diagnostics"
        );
    }

    #[test]
    fn test_compile_with_planning_binding_in_context() {
        // Program references @fund but it's in session context
        let program = Program {
            statements: vec![Statement::VerbCall(make_verb_call(
                "cbu",
                "assign-role",
                vec![
                    (
                        "cbu-id",
                        AstNode::SymbolRef {
                            name: "fund".to_string(),
                            span: Span::default(),
                        },
                    ),
                    (
                        "entity-id",
                        AstNode::Literal(Literal::String("some-uuid".into())),
                    ),
                    ("role", AstNode::Literal(Literal::String("DIRECTOR".into()))),
                ],
            ))],
        };

        let mut ctx = PlanningContext::new();
        ctx.add_binding("fund", "cbu");

        let result = compile_with_planning(&program, &ctx).unwrap();

        // No synthetic steps needed - binding is in context
        assert!(result.synthetic_steps.is_empty());
        assert!(
            !result
                .diagnostics
                .iter()
                .any(|d| matches!(d, PlannerDiagnostic::MissingProducer { .. })),
            "Should not have missing producer diagnostics when binding is in context"
        );
    }

    #[test]
    fn test_infer_entity_type_from_arg() {
        assert_eq!(infer_entity_type_from_arg("cbu-id"), "cbu");
        assert_eq!(infer_entity_type_from_arg("entity-id"), "entity");
        assert_eq!(infer_entity_type_from_arg("case-id"), "kyc_case");
        assert_eq!(
            infer_entity_type_from_arg("workstream-id"),
            "kyc_workstream"
        );
        assert_eq!(infer_entity_type_from_arg("document-id"), "document");
        assert_eq!(infer_entity_type_from_arg("unknown-arg"), "unknown");
    }

    #[test]
    fn test_get_produced_type() {
        // cbu.create produces "cbu"
        let vc = make_verb_call(
            "cbu",
            "create",
            vec![("name", AstNode::Literal(Literal::String("Test".into())))],
        );
        let produced = get_produced_type(&vc);
        assert_eq!(produced, "cbu");
    }

    #[test]
    fn test_create_synthetic_verb_call() {
        let synthetic = create_synthetic_verb_call("cbu.create", "my_fund");

        assert_eq!(synthetic.domain, "cbu");
        assert_eq!(synthetic.verb, "create");
        assert_eq!(synthetic.binding, Some("my_fund".to_string()));
        assert!(synthetic.arguments.is_empty());
        assert!(synthetic.span.is_synthetic());
    }

    #[test]
    fn test_span_synthetic() {
        let normal = Span::new(0, 10);
        assert!(!normal.is_synthetic());

        let synthetic = Span::synthetic();
        assert!(synthetic.is_synthetic());
    }

    #[test]
    fn test_planner_diagnostic_display() {
        let diag = PlannerDiagnostic::SyntheticStepInjected {
            binding: "fund".to_string(),
            verb: "cbu.create".to_string(),
            entity_type: "cbu".to_string(),
            before_stmt: 0,
        };
        let display = format!("{}", diag);
        assert!(display.contains("cbu.create"));
        assert!(display.contains("@fund"));

        let diag = PlannerDiagnostic::MissingProducer {
            binding: "unknown".to_string(),
            entity_type: "entity".to_string(),
            required_by_stmt: 1,
            reason: "test reason".to_string(),
        };
        let display = format!("{}", diag);
        assert!(display.contains("@unknown"));
        assert!(display.contains("test reason"));
    }

    // ========================================================================
    // Integration Tests for Planner Scenarios
    // ========================================================================

    #[test]
    fn test_planning_missing_cbu_injects_synthetic_create() {
        // Scenario: Reference @fund without creating it
        // Expected: Synthetic cbu.create is injected (CBU allows implicit create)
        let program = Program {
            statements: vec![Statement::VerbCall(make_verb_call(
                "cbu",
                "assign-role",
                vec![
                    (
                        "cbu-id",
                        AstNode::SymbolRef {
                            name: "fund".to_string(),
                            span: Span::default(),
                        },
                    ),
                    (
                        "entity-id",
                        AstNode::Literal(Literal::String("some-entity-uuid".into())),
                    ),
                    ("role", AstNode::Literal(Literal::String("DIRECTOR".into()))),
                ],
            ))],
        };

        let ctx = PlanningContext::new();
        let result = compile_with_planning(&program, &ctx).unwrap();

        // CBU allows implicit create, so a synthetic step should be injected
        let has_synthetic_cbu = result.diagnostics.iter().any(|d| {
            matches!(d, PlannerDiagnostic::SyntheticStepInjected { binding, entity_type, .. }
                if binding == "fund" && entity_type == "cbu")
        });

        assert!(
            has_synthetic_cbu,
            "Should inject synthetic cbu.create for @fund. Diagnostics: {:?}",
            result.diagnostics
        );

        // Should also have a synthetic step in the result
        assert!(
            result
                .synthetic_steps
                .iter()
                .any(|s| s.binding == "fund" && s.entity_type == "cbu"),
            "Should have synthetic step for @fund. Steps: {:?}",
            result.synthetic_steps
        );
    }

    #[test]
    fn test_planning_missing_entity_reports_missing_producer() {
        // Scenario: Reference @john without creating the entity
        // Expected: MissingProducer diagnostic for @john
        let program = Program {
            statements: vec![
                Statement::VerbCall(make_verb_call_with_binding(
                    "cbu",
                    "create",
                    vec![(
                        "name",
                        AstNode::Literal(Literal::String("Test Fund".into())),
                    )],
                    Some("fund"),
                )),
                Statement::VerbCall(make_verb_call(
                    "cbu",
                    "assign-role",
                    vec![
                        (
                            "cbu-id",
                            AstNode::SymbolRef {
                                name: "fund".to_string(),
                                span: Span::default(),
                            },
                        ),
                        (
                            "entity-id",
                            AstNode::SymbolRef {
                                name: "john".to_string(),
                                span: Span::default(),
                            },
                        ),
                        ("role", AstNode::Literal(Literal::String("DIRECTOR".into()))),
                    ],
                )),
            ],
        };

        let ctx = PlanningContext::new();
        let result = compile_with_planning(&program, &ctx).unwrap();

        // Should have a missing producer diagnostic for @john
        let has_missing_john = result.diagnostics.iter().any(|d| {
            matches!(d, PlannerDiagnostic::MissingProducer { binding, entity_type, .. }
                if binding == "john" && entity_type == "entity")
        });

        assert!(
            has_missing_john,
            "Should report missing producer for @john. Diagnostics: {:?}",
            result.diagnostics
        );
    }

    #[test]
    fn test_planning_all_bindings_satisfied() {
        // Scenario: All bindings are properly created before use
        // Expected: No missing producer diagnostics
        let program = Program {
            statements: vec![
                Statement::VerbCall(make_verb_call_with_binding(
                    "cbu",
                    "create",
                    vec![(
                        "name",
                        AstNode::Literal(Literal::String("Test Fund".into())),
                    )],
                    Some("fund"),
                )),
                Statement::VerbCall(make_verb_call_with_binding(
                    "entity",
                    "create-proper-person",
                    vec![
                        (
                            "first-name",
                            AstNode::Literal(Literal::String("John".into())),
                        ),
                        (
                            "last-name",
                            AstNode::Literal(Literal::String("Smith".into())),
                        ),
                    ],
                    Some("john"),
                )),
                Statement::VerbCall(make_verb_call(
                    "cbu",
                    "assign-role",
                    vec![
                        (
                            "cbu-id",
                            AstNode::SymbolRef {
                                name: "fund".to_string(),
                                span: Span::default(),
                            },
                        ),
                        (
                            "entity-id",
                            AstNode::SymbolRef {
                                name: "john".to_string(),
                                span: Span::default(),
                            },
                        ),
                        ("role", AstNode::Literal(Literal::String("DIRECTOR".into()))),
                    ],
                )),
            ],
        };

        let ctx = PlanningContext::new();
        let result = compile_with_planning(&program, &ctx).unwrap();

        // Should have NO missing producer diagnostics
        let missing_producer_count = result
            .diagnostics
            .iter()
            .filter(|d| matches!(d, PlannerDiagnostic::MissingProducer { .. }))
            .count();

        assert_eq!(
            missing_producer_count, 0,
            "Should have no missing producer diagnostics. Got: {:?}",
            result.diagnostics
        );

        // Plan should compile successfully
        assert_eq!(result.plan.len(), 3);
    }

    #[test]
    fn test_planning_context_satisfies_binding() {
        // Scenario: @fund is in session context (already created)
        // Expected: No missing producer diagnostic for @fund
        let program = Program {
            statements: vec![Statement::VerbCall(make_verb_call(
                "cbu",
                "assign-role",
                vec![
                    (
                        "cbu-id",
                        AstNode::SymbolRef {
                            name: "fund".to_string(),
                            span: Span::default(),
                        },
                    ),
                    (
                        "entity-id",
                        AstNode::Literal(Literal::String("some-entity-uuid".into())),
                    ),
                    ("role", AstNode::Literal(Literal::String("DIRECTOR".into()))),
                ],
            ))],
        };

        // Pre-add @fund to planning context
        let mut ctx = PlanningContext::new();
        ctx.add_binding("fund", "cbu");

        let result = compile_with_planning(&program, &ctx).unwrap();

        // Should NOT have a missing producer diagnostic for @fund
        let has_missing_fund = result.diagnostics.iter().any(|d| {
            matches!(d, PlannerDiagnostic::MissingProducer { binding, .. } if binding == "fund")
        });

        assert!(
            !has_missing_fund,
            "Should NOT report missing producer when @fund is in context. Diagnostics: {:?}",
            result.diagnostics
        );
    }

    #[test]
    fn test_planning_multiple_missing_bindings() {
        // Scenario: Reference multiple undefined bindings
        // Expected: CBU gets synthetic inject, entity gets MissingProducer
        let program = Program {
            statements: vec![Statement::VerbCall(make_verb_call(
                "cbu",
                "assign-role",
                vec![
                    (
                        "cbu-id",
                        AstNode::SymbolRef {
                            name: "my_cbu".to_string(),
                            span: Span::default(),
                        },
                    ),
                    (
                        "entity-id",
                        AstNode::SymbolRef {
                            name: "my_entity".to_string(),
                            span: Span::default(),
                        },
                    ),
                    ("role", AstNode::Literal(Literal::String("DIRECTOR".into()))),
                ],
            ))],
        };

        let ctx = PlanningContext::new();
        let result = compile_with_planning(&program, &ctx).unwrap();

        // CBU should get a synthetic inject (allows implicit create)
        let has_synthetic_cbu = result.diagnostics.iter().any(|d| {
            matches!(d, PlannerDiagnostic::SyntheticStepInjected { binding, entity_type, .. }
                if binding == "my_cbu" && entity_type == "cbu")
        });

        assert!(
            has_synthetic_cbu,
            "Should inject synthetic cbu.create for @my_cbu. Diagnostics: {:?}",
            result.diagnostics
        );

        // Entity should get a missing producer diagnostic (no implicit create)
        let has_missing_entity = result.diagnostics.iter().any(|d| {
            matches!(d, PlannerDiagnostic::MissingProducer { binding, .. } if binding == "my_entity")
        });

        assert!(
            has_missing_entity,
            "Should report missing @my_entity. Diagnostics: {:?}",
            result.diagnostics
        );
    }
}
