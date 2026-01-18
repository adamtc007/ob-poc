//! Topological sorting for DSL statements
//!
//! Reorders statements so that producers come before consumers.
//! This enables "write in any order" semantics for IDE and agent use.
//!
//! ## Lifecycle-Aware Sorting
//!
//! The `topological_sort_with_lifecycle` function extends basic binding-based
//! sorting with lifecycle state tracking:
//!
//! - Verbs that `transitions_to` a state create state edges
//! - Verbs with `requires_states` add dependencies on prior state transitions
//! - Lifecycle violations are reported as diagnostics (warnings in non-strict mode)

use std::collections::{HashMap, HashSet, VecDeque};

use super::ast::{AstNode, Literal, Program, Statement, VerbCall};
use super::binding_context::BindingContext;
use super::execution_plan::{PlannerDiagnostic, PlanningContext};
use super::runtime_registry::{RuntimeVerb, RuntimeVerbRegistry};

/// Errors from topological sort
#[derive(Debug, Clone)]
pub enum TopoSortError {
    /// Circular dependency detected
    CyclicDependency {
        /// Statements involved in the cycle
        cycle: Vec<String>,
    },
}

impl std::fmt::Display for TopoSortError {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            TopoSortError::CyclicDependency { cycle } => {
                write!(f, "Cyclic dependency detected: {}", cycle.join(" → "))
            }
        }
    }
}

impl std::error::Error for TopoSortError {}

/// Result of topological sort
#[derive(Debug)]
pub struct TopoSortResult {
    /// The sorted program
    pub program: Program,
    /// Whether any reordering occurred
    pub reordered: bool,
    /// Original indices in new order (for mapping diagnostics)
    pub index_map: Vec<usize>,
    /// Lifecycle-related diagnostics (violations, reorderings)
    pub lifecycle_diagnostics: Vec<PlannerDiagnostic>,
    /// Dependency graph (statement index → set of indices it depends on)
    /// Available for phase computation
    pub deps: HashMap<usize, HashSet<usize>>,
}

/// An execution phase - group of statements at the same DAG depth
#[derive(Debug, Clone)]
pub struct ExecutionPhase {
    /// Phase depth (0 = no dependencies, 1 = depends on depth 0, etc.)
    pub depth: usize,
    /// Statement indices in this phase (in sorted order)
    pub statement_indices: Vec<usize>,
}

impl TopoSortResult {
    /// Compute execution phases from dependency graph.
    ///
    /// Groups statements by their DAG depth:
    /// - Depth 0: No dependencies (can execute first)
    /// - Depth 1: Depends only on depth 0 statements
    /// - Depth N: Depends on statements up to depth N-1
    ///
    /// Statements within a phase can theoretically execute in parallel.
    pub fn compute_phases(&self) -> Vec<ExecutionPhase> {
        if self.program.statements.is_empty() {
            return vec![];
        }

        let n = self.program.statements.len();
        let mut depths: HashMap<usize, usize> = HashMap::new();

        // Compute depth for each statement using memoized recursion
        for idx in 0..n {
            compute_depth_recursive(idx, &self.deps, &mut depths);
        }

        // Find max depth
        let max_depth = depths.values().copied().max().unwrap_or(0);

        // Group statements by depth
        let mut phases: Vec<ExecutionPhase> = (0..=max_depth)
            .map(|d| ExecutionPhase {
                depth: d,
                statement_indices: vec![],
            })
            .collect();

        // Use index_map to get the sorted order
        for (sorted_pos, &original_idx) in self.index_map.iter().enumerate() {
            let depth = depths.get(&original_idx).copied().unwrap_or(0);
            // Store the sorted position, not the original index
            phases[depth].statement_indices.push(sorted_pos);
        }

        phases
    }

    /// Get the DAG depth for a specific statement index
    pub fn get_depth(&self, stmt_index: usize) -> usize {
        let mut depths: HashMap<usize, usize> = HashMap::new();
        compute_depth_recursive(stmt_index, &self.deps, &mut depths);
        depths.get(&stmt_index).copied().unwrap_or(0)
    }
}

/// Recursively compute depth for a statement
fn compute_depth_recursive(
    idx: usize,
    deps: &HashMap<usize, HashSet<usize>>,
    depths: &mut HashMap<usize, usize>,
) -> usize {
    // Already computed
    if let Some(&d) = depths.get(&idx) {
        return d;
    }

    // Get dependencies for this statement
    let my_deps = deps.get(&idx);

    let depth = if let Some(dep_set) = my_deps {
        if dep_set.is_empty() {
            0
        } else {
            // Depth is 1 + max depth of dependencies
            dep_set
                .iter()
                .map(|&dep_idx| compute_depth_recursive(dep_idx, deps, depths) + 1)
                .max()
                .unwrap_or(0)
        }
    } else {
        0
    };

    depths.insert(idx, depth);
    depth
}

/// Topologically sort pending statements respecting dataflow dependencies
///
/// # Arguments
/// * `pending` - The program to sort (pending/new statements)
/// * `executed_context` - Bindings from previously executed statements
/// * `registry` - Verb registry for produces/consumes lookup
///
/// # Returns
/// * `Ok(TopoSortResult)` - Sorted program with metadata
/// * `Err(TopoSortError)` - If cyclic dependency detected
pub fn topological_sort(
    pending: &Program,
    executed_context: &BindingContext,
    _registry: &RuntimeVerbRegistry,
) -> Result<TopoSortResult, TopoSortError> {
    let statements = &pending.statements;

    if statements.is_empty() {
        return Ok(TopoSortResult {
            program: pending.clone(),
            reordered: false,
            index_map: vec![],
            lifecycle_diagnostics: vec![],
            deps: HashMap::new(),
        });
    }

    // Build dependency graph
    // Key: statement index, Value: set of statement indices this depends on
    let mut deps: HashMap<usize, HashSet<usize>> = HashMap::new();

    // Map binding names to statement indices (for pending statements only)
    let mut binding_to_stmt: HashMap<String, usize> = HashMap::new();

    // Map PK values (UUID strings) to statement indices for implicit dependency tracking
    let mut pk_to_stmt: HashMap<String, usize> = HashMap::new();

    // First pass: record what each statement produces (bindings and implicit PKs)
    for (idx, stmt) in statements.iter().enumerate() {
        deps.insert(idx, HashSet::new());

        if let Statement::VerbCall(vc) = stmt {
            if let Some(ref binding) = vc.binding {
                binding_to_stmt.insert(binding.clone(), idx);
            }

            // Heuristic for implicit PK production:
            // If it's a creation verb (create-* or ensure), check for ID args
            if vc.verb.starts_with("create") || vc.verb == "ensure" {
                // Check for generic :id or domain-specific id (e.g. :cbu-id for cbu domain)
                let domain_id_arg = format!("{}-id", vc.domain);

                for arg in &vc.arguments {
                    if arg.key == "id" || arg.key == domain_id_arg {
                        if let Some(val_str) = extract_literal_string_or_uuid(&arg.value) {
                            pk_to_stmt.insert(val_str, idx);
                        }
                    }
                }
            }
        }
    }

    // Second pass: record dependencies based on symbol references AND implicit keys
    for (idx, stmt) in statements.iter().enumerate() {
        if let Statement::VerbCall(vc) = stmt {
            // Check all arguments for symbol references
            for arg in &vc.arguments {
                collect_symbol_refs(
                    &arg.value,
                    &binding_to_stmt,
                    executed_context,
                    idx,
                    &mut deps,
                );

                // Check for implicit key references (literals that match a produced PK)
                // Only if NOT a creation verb (or if it is, ensure we don't depend on ourselves)
                // Actually, even creators might depend on other entities (e.g. create-entity :cbu-id "...")
                collect_implicit_refs(&arg.value, &pk_to_stmt, idx, &mut deps);
            }
        }
    }

    // Kahn's algorithm for topological sort
    let mut in_degree: HashMap<usize, usize> = HashMap::new();
    for idx in 0..statements.len() {
        in_degree.insert(idx, deps[&idx].len());
    }

    // Start with nodes that have no dependencies
    let mut queue: VecDeque<usize> = in_degree
        .iter()
        .filter(|(_, &deg)| deg == 0)
        .map(|(&idx, _)| idx)
        .collect();

    // Sort initial queue by original index for stable ordering
    let mut queue_vec: Vec<usize> = queue.drain(..).collect();
    queue_vec.sort();
    queue = queue_vec.into_iter().collect();

    let mut sorted_indices = vec![];

    while let Some(idx) = queue.pop_front() {
        sorted_indices.push(idx);

        // For each statement that depends on this one, reduce its in-degree
        let mut next_ready = vec![];
        for (other_idx, other_deps) in &deps {
            if other_deps.contains(&idx) {
                if let Some(deg) = in_degree.get_mut(other_idx) {
                    *deg -= 1;
                    if *deg == 0 {
                        next_ready.push(*other_idx);
                    }
                }
            }
        }

        // Add newly ready nodes in sorted order for stability
        next_ready.sort();
        for ready_idx in next_ready {
            queue.push_back(ready_idx);
        }
    }

    // Check for cycles
    if sorted_indices.len() != statements.len() {
        let remaining: Vec<String> = (0..statements.len())
            .filter(|i| !sorted_indices.contains(i))
            .filter_map(|i| {
                if let Statement::VerbCall(vc) = &statements[i] {
                    Some(format!("{}.{}", vc.domain, vc.verb))
                } else {
                    None
                }
            })
            .collect();
        return Err(TopoSortError::CyclicDependency { cycle: remaining });
    }

    // Check if reordering occurred
    let reordered = sorted_indices
        .iter()
        .enumerate()
        .any(|(new, &old)| new != old);

    // Build sorted program
    let sorted_statements: Vec<Statement> = sorted_indices
        .iter()
        .map(|&idx| statements[idx].clone())
        .collect();

    Ok(TopoSortResult {
        program: Program {
            statements: sorted_statements,
        },
        reordered,
        index_map: sorted_indices,
        lifecycle_diagnostics: vec![],
        deps,
    })
}

/// Topologically sort statements with lifecycle state tracking
///
/// This extends basic dataflow sorting with lifecycle awareness:
/// - Verbs with `transitions_to` update binding state
/// - Verbs with `requires_states` add dependencies on prior transitions
/// - Lifecycle violations are reported as diagnostics
///
/// # Arguments
/// * `pending` - The program to sort
/// * `executed_context` - Bindings from previously executed statements
/// * `planning_context` - Planning context with binding lifecycle info
/// * `registry` - Verb registry for lifecycle and produces/consumes lookup
///
/// # Returns
/// * `Ok(TopoSortResult)` - Sorted program with lifecycle diagnostics
/// * `Err(TopoSortError)` - If cyclic dependency detected
pub fn topological_sort_with_lifecycle(
    pending: &Program,
    executed_context: &BindingContext,
    planning_context: &PlanningContext,
    registry: &RuntimeVerbRegistry,
) -> Result<TopoSortResult, TopoSortError> {
    let statements = &pending.statements;

    if statements.is_empty() {
        return Ok(TopoSortResult {
            program: pending.clone(),
            reordered: false,
            index_map: vec![],
            lifecycle_diagnostics: vec![],
            deps: HashMap::new(),
        });
    }

    // Build dependency graph with lifecycle edges
    let mut deps: HashMap<usize, HashSet<usize>> = HashMap::new();
    let mut binding_to_stmt: HashMap<String, usize> = HashMap::new();
    let mut pk_to_stmt: HashMap<String, usize> = HashMap::new();
    let mut lifecycle_diagnostics: Vec<PlannerDiagnostic> = vec![];

    // Track which statement transitions each binding to which state
    // Key: (binding_name, target_state), Value: statement index
    let mut state_transitions: HashMap<(String, String), usize> = HashMap::new();

    // Track which statements write to which tables (for table-level dependency ordering)
    // Key: table name (e.g., "custody.cbu_ssi"), Value: statement indices that write to it
    let mut table_writers: HashMap<String, Vec<usize>> = HashMap::new();

    // First pass: record what each statement produces and its state transitions
    for (idx, stmt) in statements.iter().enumerate() {
        deps.insert(idx, HashSet::new());

        if let Statement::VerbCall(vc) = stmt {
            // Record binding production
            if let Some(ref binding) = vc.binding {
                binding_to_stmt.insert(binding.clone(), idx);
            }

            // Heuristic for implicit PK production
            if vc.verb.starts_with("create") || vc.verb == "ensure" {
                let domain_id_arg = format!("{}-id", vc.domain);
                for arg in &vc.arguments {
                    if arg.key == "id" || arg.key == domain_id_arg {
                        if let Some(val_str) = extract_literal_string_or_uuid(&arg.value) {
                            pk_to_stmt.insert(val_str, idx);
                        }
                    }
                }
            }

            // Look up verb config for lifecycle info
            if let Some(runtime_verb) = registry.get(&vc.domain, &vc.verb) {
                if let Some(ref lifecycle) = runtime_verb.lifecycle {
                    // Record state transition if this verb transitions_to a state
                    if let Some(ref target_state) = lifecycle.transitions_to {
                        // Find which binding this verb operates on
                        if let Some(binding_name) = get_target_binding(vc, runtime_verb) {
                            state_transitions.insert((binding_name, target_state.clone()), idx);
                        }
                    }

                    // Record table writes for table-level dependency ordering
                    for table in &lifecycle.writes_tables {
                        tracing::debug!(
                            "Statement {} ({}.{}) writes to table: {}",
                            idx,
                            vc.domain,
                            vc.verb,
                            table
                        );
                        table_writers.entry(table.clone()).or_default().push(idx);
                    }
                }
            }
        }
    }

    // Second pass: record dependencies (dataflow + lifecycle + table reads/writes)
    for (idx, stmt) in statements.iter().enumerate() {
        if let Statement::VerbCall(vc) = stmt {
            // Standard dataflow dependencies (symbol references)
            for arg in &vc.arguments {
                collect_symbol_refs(
                    &arg.value,
                    &binding_to_stmt,
                    executed_context,
                    idx,
                    &mut deps,
                );

                // Check for implicit key references (literals that match a produced PK)
                collect_implicit_refs(&arg.value, &pk_to_stmt, idx, &mut deps);
            }

            // Lifecycle dependencies: requires_states
            let verb_key = format!("{}.{}", vc.domain, vc.verb);
            if let Some(runtime_verb) = registry.get(&vc.domain, &vc.verb) {
                if let Some(ref lifecycle) = runtime_verb.lifecycle {
                    if !lifecycle.requires_states.is_empty() {
                        // Find which binding this verb operates on
                        if let Some(binding_name) = get_target_binding(vc, runtime_verb) {
                            // Check if this binding comes from executed context
                            let from_executed = executed_context.get(&binding_name).is_some();
                            let from_planning =
                                planning_context.get_binding(&binding_name).is_some();

                            if from_executed || from_planning {
                                // Binding exists in prior context - check state
                                let current_state = planning_context
                                    .get_binding(&binding_name)
                                    .and_then(|b| b.state.clone());

                                if let Some(ref state) = current_state {
                                    if !lifecycle.requires_states.contains(state) {
                                        // State violation from prior context
                                        lifecycle_diagnostics.push(
                                            PlannerDiagnostic::LifecycleViolation {
                                                binding: binding_name.clone(),
                                                verb: verb_key.clone(),
                                                current_state: state.clone(),
                                                required_states: lifecycle.requires_states.clone(),
                                                stmt_index: idx,
                                            },
                                        );
                                    }
                                }
                            } else {
                                // Look for state transition in pending statements
                                let mut found_transition = false;
                                for required_state in &lifecycle.requires_states {
                                    let key = (binding_name.clone(), required_state.clone());
                                    if let Some(&transition_idx) = state_transitions.get(&key) {
                                        if transition_idx != idx {
                                            // Add lifecycle dependency edge
                                            if let Some(dep_set) = deps.get_mut(&idx) {
                                                dep_set.insert(transition_idx);
                                            }
                                            found_transition = true;
                                        }
                                    }
                                }

                                if !found_transition && !lifecycle.requires_states.is_empty() {
                                    // No transition found - this may be a problem
                                    // Check if the producer verb itself sets the required state
                                    if let Some(&producer_idx) = binding_to_stmt.get(&binding_name)
                                    {
                                        if let Statement::VerbCall(producer_vc) =
                                            &statements[producer_idx]
                                        {
                                            if let Some(producer_verb) =
                                                registry.get(&producer_vc.domain, &producer_vc.verb)
                                            {
                                                if let Some(ref producer_lifecycle) =
                                                    producer_verb.lifecycle
                                                {
                                                    if let Some(ref initial_state) =
                                                        producer_lifecycle.transitions_to
                                                    {
                                                        if lifecycle
                                                            .requires_states
                                                            .contains(initial_state)
                                                        {
                                                            // Producer sets the required state, add dataflow dep
                                                            if let Some(dep_set) =
                                                                deps.get_mut(&idx)
                                                            {
                                                                dep_set.insert(producer_idx);
                                                            }
                                                            found_transition = true;
                                                        }
                                                    }
                                                }
                                            }
                                        }
                                    }

                                    if !found_transition {
                                        // Report lifecycle violation warning
                                        lifecycle_diagnostics.push(
                                            PlannerDiagnostic::LifecycleViolation {
                                                binding: binding_name.clone(),
                                                verb: verb_key.clone(),
                                                current_state: "unknown".to_string(),
                                                required_states: lifecycle.requires_states.clone(),
                                                stmt_index: idx,
                                            },
                                        );
                                    }
                                }
                            }
                        }
                    }

                    // Table-level dependencies: reads_tables
                    // If this verb reads from tables, it must come AFTER any verb that writes to those tables
                    for table in &lifecycle.reads_tables {
                        tracing::debug!(
                            "Statement {} ({}.{}) reads from table: {}",
                            idx,
                            vc.domain,
                            vc.verb,
                            table
                        );
                        if let Some(writer_indices) = table_writers.get(table) {
                            for &writer_idx in writer_indices {
                                if writer_idx != idx {
                                    tracing::debug!(
                                        "Adding table dependency edge: {} -> {} (table: {})",
                                        idx,
                                        writer_idx,
                                        table
                                    );
                                    // Add dependency edge: this statement depends on the writer
                                    if let Some(dep_set) = deps.get_mut(&idx) {
                                        dep_set.insert(writer_idx);
                                    }
                                }
                            }
                        }
                    }
                }
            }
        }
    }

    // Kahn's algorithm for topological sort (same as basic version)
    let mut in_degree: HashMap<usize, usize> = HashMap::new();
    for idx in 0..statements.len() {
        in_degree.insert(idx, deps[&idx].len());
    }

    let mut queue: VecDeque<usize> = in_degree
        .iter()
        .filter(|(_, &deg)| deg == 0)
        .map(|(&idx, _)| idx)
        .collect();

    // Sort initial queue by original index for stable ordering
    let mut queue_vec: Vec<usize> = queue.drain(..).collect();
    queue_vec.sort();
    queue = queue_vec.into_iter().collect();

    let mut sorted_indices = vec![];

    while let Some(idx) = queue.pop_front() {
        sorted_indices.push(idx);

        let mut next_ready = vec![];
        for (other_idx, other_deps) in &deps {
            if other_deps.contains(&idx) {
                if let Some(deg) = in_degree.get_mut(other_idx) {
                    *deg -= 1;
                    if *deg == 0 {
                        next_ready.push(*other_idx);
                    }
                }
            }
        }

        next_ready.sort();
        for ready_idx in next_ready {
            queue.push_back(ready_idx);
        }
    }

    // Check for cycles
    if sorted_indices.len() != statements.len() {
        let remaining: Vec<String> = (0..statements.len())
            .filter(|i| !sorted_indices.contains(i))
            .filter_map(|i| {
                if let Statement::VerbCall(vc) = &statements[i] {
                    Some(format!("{}.{}", vc.domain, vc.verb))
                } else {
                    None
                }
            })
            .collect();
        return Err(TopoSortError::CyclicDependency { cycle: remaining });
    }

    // Check if reordering occurred
    let reordered = sorted_indices
        .iter()
        .enumerate()
        .any(|(new, &old)| new != old);

    // Record reordering diagnostic if applicable
    if reordered {
        let original_order: Vec<usize> = (0..statements.len()).collect();
        lifecycle_diagnostics.push(PlannerDiagnostic::StatementsReordered {
            original_order,
            new_order: sorted_indices.clone(),
            reason: "Dataflow and lifecycle dependencies".to_string(),
        });
    }

    // Build sorted program
    let sorted_statements: Vec<Statement> = sorted_indices
        .iter()
        .map(|&idx| statements[idx].clone())
        .collect();

    Ok(TopoSortResult {
        program: Program {
            statements: sorted_statements,
        },
        reordered,
        index_map: sorted_indices,
        lifecycle_diagnostics,
        deps,
    })
}

/// Extract the target binding that a verb operates on
///
/// For verbs that transition or require state, we need to know which binding
/// they affect. This looks at the verb's primary_target_arg or common patterns.
fn get_target_binding(vc: &VerbCall, runtime_verb: &RuntimeVerb) -> Option<String> {
    // Check if verb config specifies a primary target argument via entity_arg
    if let Some(ref lifecycle) = runtime_verb.lifecycle {
        if let Some(ref entity_arg) = lifecycle.entity_arg {
            // Look for this argument in the verb call
            for arg in &vc.arguments {
                if &arg.key == entity_arg {
                    if let AstNode::SymbolRef { name, .. } = &arg.value {
                        return Some(name.clone());
                    }
                }
            }
        }
    }

    // Fallback: check common argument names that indicate target binding
    let target_arg_names = [
        "instance-id",
        "entity-id",
        "cbu-id",
        "ssi-id",
        "case-id",
        "workstream-id",
    ];
    for arg_name in &target_arg_names {
        for arg in &vc.arguments {
            if arg.key == *arg_name {
                if let AstNode::SymbolRef { name, .. } = &arg.value {
                    return Some(name.clone());
                }
            }
        }
    }

    // If verb produces a binding, that's also a target
    vc.binding.clone()
}

/// Recursively collect symbol references from an AST node
fn collect_symbol_refs(
    node: &AstNode,
    binding_to_stmt: &HashMap<String, usize>,
    _executed_context: &BindingContext,
    current_idx: usize,
    deps: &mut HashMap<usize, HashSet<usize>>,
) {
    match node {
        AstNode::SymbolRef { name: ref_name, .. } => {
            // Is this ref from another pending statement?
            if let Some(&producer_idx) = binding_to_stmt.get(ref_name) {
                if producer_idx != current_idx {
                    // This statement depends on producer_idx
                    if let Some(dep_set) = deps.get_mut(&current_idx) {
                        dep_set.insert(producer_idx);
                    }
                }
            }
            // If from executed context, no dependency to track (already satisfied)
        }
        AstNode::List { items, .. } => {
            for item in items {
                collect_symbol_refs(item, binding_to_stmt, _executed_context, current_idx, deps);
            }
        }
        AstNode::Map { entries, .. } => {
            for (_, v) in entries {
                collect_symbol_refs(v, binding_to_stmt, _executed_context, current_idx, deps);
            }
        }
        // Literals, EntityRefs, and Nested don't contain symbol references we need to track
        AstNode::Literal(_) | AstNode::EntityRef { .. } | AstNode::Nested(_) => {}
    }
}

/// Extract string representation from a literal (String or UUID)
fn extract_literal_string_or_uuid(node: &AstNode) -> Option<String> {
    match node {
        AstNode::Literal(Literal::String(s)) => Some(s.clone()),
        AstNode::Literal(Literal::Uuid(u)) => Some(u.to_string()),
        _ => None,
    }
}

/// Collect implicit key references from an AST node
fn collect_implicit_refs(
    node: &AstNode,
    pk_to_stmt: &HashMap<String, usize>,
    current_idx: usize,
    deps: &mut HashMap<usize, HashSet<usize>>,
) {
    match node {
        AstNode::Literal(Literal::String(s)) => {
            if let Some(&producer_idx) = pk_to_stmt.get(s) {
                if producer_idx != current_idx {
                    if let Some(dep_set) = deps.get_mut(&current_idx) {
                        dep_set.insert(producer_idx);
                    }
                }
            }
        }
        AstNode::Literal(Literal::Uuid(u)) => {
            let s = u.to_string();
            if let Some(&producer_idx) = pk_to_stmt.get(&s) {
                if producer_idx != current_idx {
                    if let Some(dep_set) = deps.get_mut(&current_idx) {
                        dep_set.insert(producer_idx);
                    }
                }
            }
        }
        AstNode::List { items, .. } => {
            for item in items {
                collect_implicit_refs(item, pk_to_stmt, current_idx, deps);
            }
        }
        AstNode::Map { entries, .. } => {
            for (_, v) in entries {
                collect_implicit_refs(v, pk_to_stmt, current_idx, deps);
            }
        }
        AstNode::Nested(vc) => {
            for arg in &vc.arguments {
                collect_implicit_refs(&arg.value, pk_to_stmt, current_idx, deps);
            }
        }
        _ => {}
    }
}

/// Emit DSL source from a sorted program
///
/// Reconstructs the DSL text from the AST, preserving formatting where possible.
pub fn emit_dsl(program: &Program) -> String {
    let mut lines = vec![];

    for stmt in &program.statements {
        match stmt {
            Statement::VerbCall(vc) => {
                let mut parts = vec![format!("({}.{}", vc.domain, vc.verb)];

                for arg in &vc.arguments {
                    let value_str = format_ast_node(&arg.value);
                    parts.push(format!(":{} {}", arg.key, value_str));
                }

                if let Some(ref binding) = vc.binding {
                    parts.push(format!(":as @{}", binding));
                }

                parts.push(")".to_string());
                lines.push(parts.join(" "));
            }
            Statement::Comment(text) => {
                lines.push(format!(";; {}", text));
            }
        }
    }

    lines.join("\n")
}

/// Format an AST node back to DSL text
fn format_ast_node(node: &AstNode) -> String {
    match node {
        AstNode::Literal(lit) => match lit {
            Literal::String(s) => format!("\"{}\"", s.replace('\"', "\\\"")),
            Literal::Integer(n) => n.to_string(),
            Literal::Decimal(d) => d.to_string(),
            Literal::Boolean(b) => if *b { "true" } else { "false" }.to_string(),
            Literal::Null => "nil".to_string(),
            Literal::Uuid(u) => format!("\"{}\"", u),
        },
        AstNode::SymbolRef { name, .. } => format!("@{}", name),
        AstNode::EntityRef {
            resolved_key,
            value,
            ..
        } => {
            // EntityRef with resolved key - emit the display or search key
            if let Some(ref pk) = resolved_key {
                format!("\"{}\"", pk)
            } else {
                format!("\"{}\"", value)
            }
        }
        AstNode::List { items, .. } => {
            let inner: Vec<String> = items.iter().map(format_ast_node).collect();
            format!("[{}]", inner.join(" "))
        }
        AstNode::Map { entries, .. } => {
            let inner: Vec<String> = entries
                .iter()
                .map(|(k, v)| format!(":{} {}", k, format_ast_node(v)))
                .collect();
            format!("{{{}}}", inner.join(" "))
        }
        AstNode::Nested(vc) => {
            // Format nested verb call
            let mut parts = vec![format!("({}.{}", vc.domain, vc.verb)];
            for arg in &vc.arguments {
                let value_str = format_ast_node(&arg.value);
                parts.push(format!(":{} {}", arg.key, value_str));
            }
            if let Some(ref binding) = vc.binding {
                parts.push(format!(":as @{}", binding));
            }
            parts.push(")".to_string());
            parts.join(" ")
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::dsl_v2::{parse_program, runtime_registry::runtime_registry};

    #[test]
    fn test_already_sorted_no_reorder() {
        let source = r#"
            (cbu.ensure :name "Fund" :jurisdiction "LU" :as @fund)
            (entity.create-proper-person :first-name "John" :last-name "Smith" :as @john)
            (cbu.assign-role :cbu-id @fund :entity-id @john :role "DIRECTOR")
        "#;

        let ast = parse_program(source).expect("parse");
        let registry = runtime_registry();
        let ctx = BindingContext::new();

        let result = topological_sort(&ast, &ctx, registry).expect("sort");

        // Already in order, should not reorder
        assert!(
            !result.reordered,
            "Should not reorder already-sorted program"
        );
    }

    #[test]
    fn test_out_of_order_reorders() {
        let source = r#"
            (cbu.assign-role :cbu-id @fund :entity-id @john :role "DIRECTOR")
            (entity.create-proper-person :first-name "John" :last-name "Smith" :as @john)
            (cbu.ensure :name "Fund" :jurisdiction "LU" :as @fund)
        "#;

        let ast = parse_program(source).expect("parse");
        let registry = runtime_registry();
        let ctx = BindingContext::new();

        let result = topological_sort(&ast, &ctx, registry).expect("sort");

        assert!(result.reordered, "Should reorder out-of-order program");

        // Last statement should be cbu.assign-role (consumes both @fund and @john)
        if let Statement::VerbCall(vc) = &result.program.statements[2] {
            assert_eq!(vc.verb, "assign-role", "Last should be cbu.assign-role");
        }

        // First two statements should be the producers (order between them doesn't matter)
        // They produce @fund and @john which are consumed by assign-role
        let first_two_verbs: Vec<&str> = result.program.statements[0..2]
            .iter()
            .filter_map(|s| {
                if let Statement::VerbCall(vc) = s {
                    Some(vc.verb.as_str())
                } else {
                    None
                }
            })
            .collect();

        assert!(
            first_two_verbs.contains(&"ensure")
                && first_two_verbs.contains(&"create-proper-person"),
            "First two should be cbu.ensure and entity.create-proper-person (any order)"
        );
    }

    #[test]
    fn test_with_executed_context() {
        // @fund already exists in executed context
        let mut ctx = BindingContext::new();
        ctx.insert(super::super::binding_context::BindingInfo {
            name: "fund".to_string(),
            produced_type: "cbu".to_string(),
            subtype: None,
            entity_pk: uuid::Uuid::new_v4(),
            resolved: false,
            source_sheet_id: None,
        });

        // New statement references existing @fund
        let source = r#"
            (cbu.assign-role :cbu-id @fund :entity-id @john :role "DIRECTOR")
            (entity.create-proper-person :first-name "John" :last-name "Smith" :as @john)
        "#;

        let ast = parse_program(source).expect("parse");
        let registry = runtime_registry();

        let result = topological_sort(&ast, &ctx, registry).expect("sort");

        // Should reorder: entity.create before assign-role
        // @fund from executed context is already satisfied
        assert!(result.reordered);

        if let Statement::VerbCall(vc) = &result.program.statements[0] {
            assert_eq!(
                vc.verb, "create-proper-person",
                "First should create entity"
            );
        }
    }

    #[test]
    fn test_emit_dsl_roundtrip() {
        let source = r#"(cbu.ensure :name "Test Fund" :jurisdiction "LU" :as @fund)"#;
        let ast = parse_program(source).expect("parse");

        let emitted = emit_dsl(&ast);

        // Should be parseable
        let reparsed = parse_program(&emitted).expect("reparse");
        assert_eq!(reparsed.statements.len(), 1);
    }

    #[test]
    fn test_empty_program() {
        let ast = Program { statements: vec![] };
        let registry = runtime_registry();
        let ctx = BindingContext::new();

        let result = topological_sort(&ast, &ctx, registry).expect("sort");

        assert!(!result.reordered);
        assert!(result.index_map.is_empty());
    }

    #[test]
    fn test_independent_statements_stable_order() {
        // Two independent statements should maintain original order
        let source = r#"
            (cbu.ensure :name "Fund A" :jurisdiction "LU" :as @funda)
            (cbu.ensure :name "Fund B" :jurisdiction "US" :as @fundb)
        "#;

        let ast = parse_program(source).expect("parse");
        let registry = runtime_registry();
        let ctx = BindingContext::new();

        let result = topological_sort(&ast, &ctx, registry).expect("sort");

        // Should not reorder - both are independent
        assert!(
            !result.reordered,
            "Independent statements should maintain order"
        );
        assert_eq!(result.index_map, vec![0, 1]);
    }

    // =========================================================================
    // Lifecycle-aware topological sort tests
    // =========================================================================

    #[test]
    fn test_lifecycle_sort_empty_program() {
        let ast = Program { statements: vec![] };
        let registry = runtime_registry();
        let ctx = BindingContext::new();
        let planning_ctx = PlanningContext::new();

        let result =
            topological_sort_with_lifecycle(&ast, &ctx, &planning_ctx, registry).expect("sort");

        assert!(!result.reordered);
        assert!(result.index_map.is_empty());
        assert!(result.lifecycle_diagnostics.is_empty());
    }

    #[test]
    fn test_lifecycle_sort_basic_dataflow() {
        // Basic dataflow sorting should still work
        let source = r#"
            (cbu.assign-role :cbu-id @fund :entity-id @john :role "DIRECTOR")
            (entity.create-proper-person :first-name "John" :last-name "Smith" :as @john)
            (cbu.ensure :name "Fund" :jurisdiction "LU" :as @fund)
        "#;

        let ast = parse_program(source).expect("parse");
        let registry = runtime_registry();
        let ctx = BindingContext::new();
        let planning_ctx = PlanningContext::new();

        let result =
            topological_sort_with_lifecycle(&ast, &ctx, &planning_ctx, registry).expect("sort");

        assert!(result.reordered, "Should reorder out-of-order program");

        // Last statement should be cbu.assign-role (consumes both @fund and @john)
        if let Statement::VerbCall(vc) = &result.program.statements[2] {
            assert_eq!(vc.verb, "assign-role", "Last should be cbu.assign-role");
        }
    }

    #[test]
    fn test_lifecycle_sort_records_reordering_diagnostic() {
        let source = r#"
            (cbu.assign-role :cbu-id @fund :entity-id @john :role "DIRECTOR")
            (cbu.ensure :name "Fund" :jurisdiction "LU" :as @fund)
            (entity.create-proper-person :first-name "John" :last-name "Smith" :as @john)
        "#;

        let ast = parse_program(source).expect("parse");
        let registry = runtime_registry();
        let ctx = BindingContext::new();
        let planning_ctx = PlanningContext::new();

        let result =
            topological_sort_with_lifecycle(&ast, &ctx, &planning_ctx, registry).expect("sort");

        assert!(result.reordered);

        // Should have a reordering diagnostic
        let has_reorder_diag = result
            .lifecycle_diagnostics
            .iter()
            .any(|d| matches!(d, PlannerDiagnostic::StatementsReordered { .. }));
        assert!(
            has_reorder_diag,
            "Should record reordering diagnostic when statements are reordered"
        );
    }

    #[test]
    fn test_lifecycle_sort_no_reorder_already_sorted() {
        let source = r#"
            (cbu.ensure :name "Fund" :jurisdiction "LU" :as @fund)
            (entity.create-proper-person :first-name "John" :last-name "Smith" :as @john)
            (cbu.assign-role :cbu-id @fund :entity-id @john :role "DIRECTOR")
        "#;

        let ast = parse_program(source).expect("parse");
        let registry = runtime_registry();
        let ctx = BindingContext::new();
        let planning_ctx = PlanningContext::new();

        let result =
            topological_sort_with_lifecycle(&ast, &ctx, &planning_ctx, registry).expect("sort");

        assert!(
            !result.reordered,
            "Should not reorder already-sorted program"
        );

        // No reordering diagnostic when already in order
        let has_reorder_diag = result
            .lifecycle_diagnostics
            .iter()
            .any(|d| matches!(d, PlannerDiagnostic::StatementsReordered { .. }));
        assert!(
            !has_reorder_diag,
            "Should not have reordering diagnostic when already sorted"
        );
    }

    #[test]
    fn test_lifecycle_sort_with_executed_context() {
        // @fund already exists in executed context
        let mut ctx = BindingContext::new();
        ctx.insert(super::super::binding_context::BindingInfo {
            name: "fund".to_string(),
            produced_type: "cbu".to_string(),
            subtype: None,
            entity_pk: uuid::Uuid::new_v4(),
            resolved: false,
            source_sheet_id: None,
        });

        let source = r#"
            (cbu.assign-role :cbu-id @fund :entity-id @john :role "DIRECTOR")
            (entity.create-proper-person :first-name "John" :last-name "Smith" :as @john)
        "#;

        let ast = parse_program(source).expect("parse");
        let registry = runtime_registry();
        let planning_ctx = PlanningContext::new();

        let result =
            topological_sort_with_lifecycle(&ast, &ctx, &planning_ctx, registry).expect("sort");

        // Should reorder: entity.create before assign-role
        assert!(result.reordered);

        if let Statement::VerbCall(vc) = &result.program.statements[0] {
            assert_eq!(
                vc.verb, "create-proper-person",
                "First should create entity"
            );
        }
    }

    // =========================================================================
    // Phase computation tests
    // =========================================================================

    #[test]
    fn test_compute_phases_empty() {
        let ast = Program { statements: vec![] };
        let registry = runtime_registry();
        let ctx = BindingContext::new();

        let result = topological_sort(&ast, &ctx, registry).expect("sort");
        let phases = result.compute_phases();

        assert!(phases.is_empty(), "Empty program should have no phases");
    }

    #[test]
    fn test_compute_phases_independent_statements() {
        // Two independent statements should be in phase 0
        let source = r#"
            (cbu.ensure :name "Fund A" :jurisdiction "LU" :as @funda)
            (cbu.ensure :name "Fund B" :jurisdiction "US" :as @fundb)
        "#;

        let ast = parse_program(source).expect("parse");
        let registry = runtime_registry();
        let ctx = BindingContext::new();

        let result = topological_sort(&ast, &ctx, registry).expect("sort");
        let phases = result.compute_phases();

        assert_eq!(
            phases.len(),
            1,
            "Independent statements should be in one phase"
        );
        assert_eq!(phases[0].depth, 0, "Phase should be depth 0");
        assert_eq!(
            phases[0].statement_indices.len(),
            2,
            "Both statements in phase 0"
        );
    }

    #[test]
    fn test_compute_phases_dependency_chain() {
        // A chain: create fund → create person → assign role
        let source = r#"
            (cbu.ensure :name "Fund" :jurisdiction "LU" :as @fund)
            (entity.create-proper-person :first-name "John" :last-name "Smith" :as @john)
            (cbu.assign-role :cbu-id @fund :entity-id @john :role "DIRECTOR")
        "#;

        let ast = parse_program(source).expect("parse");
        let registry = runtime_registry();
        let ctx = BindingContext::new();

        let result = topological_sort(&ast, &ctx, registry).expect("sort");
        let phases = result.compute_phases();

        // fund and john are independent (phase 0)
        // assign-role depends on both (phase 1)
        assert_eq!(phases.len(), 2, "Should have 2 phases");
        assert_eq!(phases[0].depth, 0);
        assert_eq!(
            phases[0].statement_indices.len(),
            2,
            "Two producers in phase 0"
        );
        assert_eq!(phases[1].depth, 1);
        assert_eq!(
            phases[1].statement_indices.len(),
            1,
            "One consumer in phase 1"
        );
    }

    #[test]
    fn test_compute_phases_deep_chain() {
        // Deep chain: A → B → C (each depends on previous)
        let source = r#"
            (cbu.ensure :name "Fund" :jurisdiction "LU" :as @fund)
            (entity.create-proper-person :first-name "John" :last-name "Smith" :as @john)
            (cbu.assign-role :cbu-id @fund :entity-id @john :role "DIRECTOR" :as @role)
        "#;

        let ast = parse_program(source).expect("parse");
        let registry = runtime_registry();
        let ctx = BindingContext::new();

        let result = topological_sort(&ast, &ctx, registry).expect("sort");
        let phases = result.compute_phases();

        // Phase 0: fund, john (independent)
        // Phase 1: assign-role (depends on fund and john)
        assert!(phases.len() >= 2, "Should have at least 2 phases");
        assert_eq!(phases[0].depth, 0);
    }
}
