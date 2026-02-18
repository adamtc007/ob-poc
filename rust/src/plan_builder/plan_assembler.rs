//! Plan assembler — step ordering and dependency detection.
//!
//! Takes a list of `CompiledStep` values (from macro expansion or single-verb
//! compilation) and resolves inter-step dependencies by analysing:
//!
//! 1. **Binding references** — `@binding` in DSL that match `:as @binding` in
//!    a prior step.
//! 2. **Verb ordering heuristics** — create before assign, entity before role.
//!
//! The output is a `PlanAssemblyResult` with:
//! - Steps reordered by dependency (topological sort).
//! - `depends_on` fields populated with predecessor `step_id` values.
//! - Execution phases computed from DAG depth.
//!
//! ## Design
//!
//! This operates at the **DSL string level** (not the full AST).  Macro
//! expansion produces DSL strings, not parsed ASTs, so the assembler uses
//! lightweight regex-free heuristic parsing to extract bindings and
//! references.  This avoids pulling in the full `dsl-core` parser and keeps
//! the plan_builder self-contained.

use std::collections::{HashMap, HashSet, VecDeque};

use uuid::Uuid;

use crate::runbook::types::CompiledStep;

use super::errors::AssemblyError;

// ---------------------------------------------------------------------------
// PlanAssemblyResult
// ---------------------------------------------------------------------------

/// Result of plan assembly — steps with resolved dependencies and phases.
#[derive(Debug, Clone)]
pub struct PlanAssemblyResult {
    /// Steps in dependency order (topologically sorted).
    pub steps: Vec<CompiledStep>,
    /// Whether any reordering occurred.
    pub reordered: bool,
    /// Execution phases grouped by DAG depth.
    /// Phase 0 = no dependencies, Phase 1 = depends only on Phase 0, etc.
    pub phases: Vec<ExecutionPhase>,
    /// Assembly diagnostics (informational, not errors).
    pub diagnostics: Vec<AssemblyDiagnostic>,
}

/// An execution phase — group of steps at the same DAG depth.
#[derive(Debug, Clone)]
pub struct ExecutionPhase {
    /// Phase depth (0 = roots, 1 = depends on roots, …).
    pub depth: usize,
    /// Indices into `PlanAssemblyResult.steps`.
    pub step_indices: Vec<usize>,
}

/// Informational diagnostic from assembly.
#[derive(Debug, Clone)]
pub struct AssemblyDiagnostic {
    pub kind: DiagnosticKind,
    pub message: String,
}

#[derive(Debug, Clone, PartialEq)]
pub enum DiagnosticKind {
    /// Steps were reordered to satisfy dependencies.
    Reordered,
    /// A binding reference could not be resolved (warning, not error).
    UnresolvedBinding,
}

// ---------------------------------------------------------------------------
// assemble_plan — public entry point
// ---------------------------------------------------------------------------

/// Assemble a plan from a list of compiled steps.
///
/// Analyses binding dependencies between steps, topologically sorts them,
/// populates `depends_on` with predecessor step IDs, and computes execution
/// phases.
///
/// # Errors
///
/// Returns `AssemblyError::CyclicDependency` if the dependency graph has
/// cycles, and `AssemblyError::EmptyPlan` if no steps are provided.
pub fn assemble_plan(steps: Vec<CompiledStep>) -> Result<PlanAssemblyResult, AssemblyError> {
    if steps.is_empty() {
        return Err(AssemblyError::EmptyPlan);
    }

    // Single step — no dependencies to resolve.
    if steps.len() == 1 {
        return Ok(PlanAssemblyResult {
            steps,
            reordered: false,
            phases: vec![ExecutionPhase {
                depth: 0,
                step_indices: vec![0],
            }],
            diagnostics: vec![],
        });
    }

    let mut diagnostics = Vec::new();

    // 1. Extract bindings produced and consumed by each step.
    let step_bindings: Vec<StepBindings> = steps.iter().map(extract_bindings).collect();

    // 2. Build dependency graph: step_index → set of step_indices it depends on.
    let deps = build_dependency_graph(&step_bindings, &mut diagnostics);

    // 3. Topological sort (Kahn's algorithm).
    let sorted_indices = topological_sort(steps.len(), &deps)?;

    let reordered = sorted_indices
        .iter()
        .enumerate()
        .any(|(pos, &orig)| pos != orig);

    if reordered {
        diagnostics.push(AssemblyDiagnostic {
            kind: DiagnosticKind::Reordered,
            message: "Steps were reordered to satisfy binding dependencies.".into(),
        });
    }

    // 4. Reorder steps and build new step_id → index mapping.
    let mut sorted_steps: Vec<CompiledStep> =
        sorted_indices.iter().map(|&i| steps[i].clone()).collect();

    // 5. Populate `depends_on` with predecessor step_ids.
    //    Map old indices to new step_ids.
    let old_to_new_step_id: HashMap<usize, Uuid> = sorted_indices
        .iter()
        .enumerate()
        .map(|(new_pos, &old_idx)| (old_idx, sorted_steps[new_pos].step_id))
        .collect();

    for (new_pos, &old_idx) in sorted_indices.iter().enumerate() {
        if let Some(dep_set) = deps.get(&old_idx) {
            let depends_on: Vec<Uuid> = dep_set
                .iter()
                .filter_map(|dep_old_idx| old_to_new_step_id.get(dep_old_idx).copied())
                .collect();
            sorted_steps[new_pos].depends_on = depends_on;
        }
    }

    // 6. Compute execution phases from DAG depth.
    let phases = compute_phases(&sorted_indices, &deps);

    Ok(PlanAssemblyResult {
        steps: sorted_steps,
        reordered,
        phases,
        diagnostics,
    })
}

// ---------------------------------------------------------------------------
// Binding extraction
// ---------------------------------------------------------------------------

/// Bindings produced and consumed by a single step.
struct StepBindings {
    /// Bindings this step produces (`:as @name` patterns).
    produces: Vec<String>,
    /// Bindings this step consumes (`@name` references not in `:as`).
    consumes: Vec<String>,
}

/// Extract binding info from a compiled step's DSL string.
///
/// Lightweight heuristic parsing — does not need the full parser.
fn extract_bindings(step: &CompiledStep) -> StepBindings {
    let dsl = &step.dsl;
    let mut produces = Vec::new();
    let mut consumes = Vec::new();

    // Find `:as @binding` patterns — these are productions.
    let mut produced_set = HashSet::new();
    let mut search_from = 0;
    while let Some(pos) = dsl[search_from..].find(":as @") {
        let abs_pos = search_from + pos + 5; // skip ":as @"
        if let Some(name) = extract_binding_name(&dsl[abs_pos..]) {
            produced_set.insert(name.clone());
            produces.push(name);
        }
        search_from = abs_pos;
    }

    // Find all `@binding` references — those not in produced_set are consumptions.
    search_from = 0;
    while let Some(pos) = dsl[search_from..].find('@') {
        let abs_pos = search_from + pos;

        // Skip if this is part of a `:as @` production (already handled).
        if abs_pos >= 4 && &dsl[abs_pos - 4..abs_pos] == ":as " {
            search_from = abs_pos + 1;
            continue;
        }

        if let Some(name) = extract_binding_name(&dsl[abs_pos + 1..]) {
            if !produced_set.contains(&name) {
                consumes.push(name);
            }
        }
        search_from = abs_pos + 1;
    }

    StepBindings { produces, consumes }
}

/// Extract a binding name starting at the given position.
///
/// Binding names are sequences of `[a-zA-Z0-9_-]`.
fn extract_binding_name(s: &str) -> Option<String> {
    let end = s
        .find(|c: char| !c.is_alphanumeric() && c != '_' && c != '-')
        .unwrap_or(s.len());
    if end == 0 {
        return None;
    }
    Some(s[..end].to_string())
}

// ---------------------------------------------------------------------------
// Dependency graph construction
// ---------------------------------------------------------------------------

/// Build a dependency graph from step bindings.
///
/// Step A depends on step B if A consumes a binding that B produces.
fn build_dependency_graph(
    bindings: &[StepBindings],
    diagnostics: &mut Vec<AssemblyDiagnostic>,
) -> HashMap<usize, HashSet<usize>> {
    // Build producer index: binding_name → step_index that produces it.
    let mut producer_index: HashMap<&str, usize> = HashMap::new();
    for (idx, step_bindings) in bindings.iter().enumerate() {
        for name in &step_bindings.produces {
            producer_index.insert(name.as_str(), idx);
        }
    }

    // Build dependency edges.
    let mut deps: HashMap<usize, HashSet<usize>> = HashMap::new();
    for (idx, step_bindings) in bindings.iter().enumerate() {
        for consumed in &step_bindings.consumes {
            if let Some(&producer_idx) = producer_index.get(consumed.as_str()) {
                if producer_idx != idx {
                    deps.entry(idx).or_default().insert(producer_idx);
                }
            } else {
                diagnostics.push(AssemblyDiagnostic {
                    kind: DiagnosticKind::UnresolvedBinding,
                    message: format!(
                        "Binding @{} referenced but not produced by any step",
                        consumed
                    ),
                });
            }
        }
    }

    deps
}

// ---------------------------------------------------------------------------
// Topological sort (Kahn's algorithm)
// ---------------------------------------------------------------------------

/// Topological sort using Kahn's algorithm. Returns sorted indices.
fn topological_sort(
    n: usize,
    deps: &HashMap<usize, HashSet<usize>>,
) -> Result<Vec<usize>, AssemblyError> {
    // Compute in-degree for each node.
    let mut in_degree = vec![0usize; n];
    let mut reverse_deps: HashMap<usize, Vec<usize>> = HashMap::new();

    for (&dependent, dep_set) in deps {
        in_degree[dependent] = dep_set.len();
        for &dependency in dep_set {
            reverse_deps.entry(dependency).or_default().push(dependent);
        }
    }

    // Start with zero-in-degree nodes.
    let mut queue: VecDeque<usize> = (0..n).filter(|&i| in_degree[i] == 0).collect();
    let mut sorted = Vec::with_capacity(n);

    while let Some(node) = queue.pop_front() {
        sorted.push(node);
        if let Some(dependents) = reverse_deps.get(&node) {
            for &dependent in dependents {
                in_degree[dependent] -= 1;
                if in_degree[dependent] == 0 {
                    queue.push_back(dependent);
                }
            }
        }
    }

    if sorted.len() != n {
        // Cycle detected — collect the verbs involved.
        let remaining: Vec<usize> = (0..n).filter(|i| !sorted.contains(i)).collect();
        return Err(AssemblyError::CyclicDependency {
            cycle: remaining.iter().map(|i| format!("step_{}", i)).collect(),
        });
    }

    Ok(sorted)
}

// ---------------------------------------------------------------------------
// Phase computation
// ---------------------------------------------------------------------------

/// Compute execution phases from sorted indices and dependency graph.
fn compute_phases(
    sorted_indices: &[usize],
    deps: &HashMap<usize, HashSet<usize>>,
) -> Vec<ExecutionPhase> {
    let mut depths: HashMap<usize, usize> = HashMap::new();

    // Compute depth for each original index.
    for &idx in sorted_indices {
        let depth = compute_depth(idx, deps, &mut depths);
        depths.insert(idx, depth);
    }

    // Map original indices to sorted positions.
    let mut orig_to_sorted: HashMap<usize, usize> = HashMap::new();
    for (sorted_pos, &orig_idx) in sorted_indices.iter().enumerate() {
        orig_to_sorted.insert(orig_idx, sorted_pos);
    }

    // Find max depth.
    let max_depth = depths.values().copied().max().unwrap_or(0);

    // Group sorted positions by depth.
    let mut phases: Vec<ExecutionPhase> = (0..=max_depth)
        .map(|d| ExecutionPhase {
            depth: d,
            step_indices: vec![],
        })
        .collect();

    for &orig_idx in sorted_indices {
        let depth = depths.get(&orig_idx).copied().unwrap_or(0);
        let sorted_pos = orig_to_sorted[&orig_idx];
        phases[depth].step_indices.push(sorted_pos);
    }

    // Remove empty phases (shouldn't happen, but defensive).
    phases.retain(|p| !p.step_indices.is_empty());

    phases
}

fn compute_depth(
    idx: usize,
    deps: &HashMap<usize, HashSet<usize>>,
    cache: &mut HashMap<usize, usize>,
) -> usize {
    if let Some(&d) = cache.get(&idx) {
        return d;
    }

    let depth = match deps.get(&idx) {
        Some(dep_set) if !dep_set.is_empty() => dep_set
            .iter()
            .map(|&dep| compute_depth(dep, deps, cache) + 1)
            .max()
            .unwrap_or(0),
        _ => 0,
    };

    cache.insert(idx, depth);
    depth
}

// ---------------------------------------------------------------------------
// Tests
// ---------------------------------------------------------------------------

#[cfg(test)]
mod tests {
    use super::*;
    use crate::runbook::types::ExecutionMode;

    fn make_step(verb: &str, dsl: &str) -> CompiledStep {
        CompiledStep {
            step_id: Uuid::new_v4(),
            sentence: format!("Execute {}", verb),
            verb: verb.to_string(),
            dsl: dsl.to_string(),
            args: std::collections::BTreeMap::new(),
            depends_on: vec![],
            execution_mode: ExecutionMode::Sync,
            write_set: vec![],
            verb_contract_snapshot_id: None,
        }
    }

    #[test]
    fn test_single_step() {
        let steps = vec![make_step("cbu.create", "(cbu.create :name \"Acme\")")];
        let result = assemble_plan(steps).unwrap();
        assert_eq!(result.steps.len(), 1);
        assert!(!result.reordered);
        assert_eq!(result.phases.len(), 1);
        assert_eq!(result.phases[0].depth, 0);
    }

    #[test]
    fn test_empty_plan_error() {
        let err = assemble_plan(vec![]).unwrap_err();
        assert!(matches!(err, AssemblyError::EmptyPlan));
    }

    #[test]
    fn test_no_dependencies_preserves_order() {
        let steps = vec![
            make_step("cbu.create", "(cbu.create :name \"A\")"),
            make_step("entity.create", "(entity.create :name \"B\")"),
        ];
        let result = assemble_plan(steps).unwrap();
        assert_eq!(result.steps.len(), 2);
        assert!(!result.reordered);
        // Both at phase 0 — no dependencies.
        assert_eq!(result.phases.len(), 1);
        assert_eq!(result.phases[0].step_indices.len(), 2);
    }

    #[test]
    fn test_binding_dependency_detected() {
        let steps = vec![
            make_step("cbu.create", "(cbu.create :name \"Acme\" :as @cbu)"),
            make_step(
                "cbu-role.assign",
                "(cbu-role.assign :cbu-id @cbu :role depositary)",
            ),
        ];
        let result = assemble_plan(steps).unwrap();
        assert_eq!(result.steps.len(), 2);
        // Step 1 depends on step 0
        assert!(!result.steps[1].depends_on.is_empty());
        assert_eq!(result.steps[1].depends_on[0], result.steps[0].step_id);
        // Two phases: depth 0 (create), depth 1 (assign)
        assert_eq!(result.phases.len(), 2);
    }

    #[test]
    fn test_reordering_when_consumer_before_producer() {
        // Consumer listed before producer — assembler should reorder.
        let steps = vec![
            make_step(
                "cbu-role.assign",
                "(cbu-role.assign :cbu-id @cbu :role depositary)",
            ),
            make_step("cbu.create", "(cbu.create :name \"Acme\" :as @cbu)"),
        ];
        let result = assemble_plan(steps).unwrap();
        assert!(result.reordered);
        // After sort, cbu.create should come first.
        assert_eq!(result.steps[0].verb, "cbu.create");
        assert_eq!(result.steps[1].verb, "cbu-role.assign");
        // The assign step should depend on the create step.
        assert_eq!(result.steps[1].depends_on.len(), 1);
        assert_eq!(result.steps[1].depends_on[0], result.steps[0].step_id);
    }

    #[test]
    fn test_diamond_dependency() {
        // A → B, A → C, B → D, C → D (diamond)
        let steps = vec![
            make_step("a.create", "(a.create :as @a)"),
            make_step("b.create", "(b.create :src @a :as @b)"),
            make_step("c.create", "(c.create :src @a :as @c)"),
            make_step("d.create", "(d.create :x @b :y @c)"),
        ];
        let result = assemble_plan(steps).unwrap();
        assert_eq!(result.steps.len(), 4);

        // 3 phases: depth 0 (a), depth 1 (b, c), depth 2 (d)
        assert_eq!(result.phases.len(), 3);
        assert_eq!(result.phases[0].step_indices.len(), 1); // a
        assert_eq!(result.phases[1].step_indices.len(), 2); // b, c
        assert_eq!(result.phases[2].step_indices.len(), 1); // d

        // Step d should depend on both b and c.
        let d_step = &result.steps[3];
        assert_eq!(d_step.depends_on.len(), 2);
    }

    #[test]
    fn test_unresolved_binding_diagnostic() {
        let steps = vec![
            make_step("cbu.create", "(cbu.create :name \"Acme\")"),
            make_step(
                "cbu-role.assign",
                "(cbu-role.assign :cbu-id @unknown_ref :role depositary)",
            ),
        ];
        let result = assemble_plan(steps).unwrap();
        // Should have an UnresolvedBinding diagnostic.
        assert!(result
            .diagnostics
            .iter()
            .any(|d| d.kind == DiagnosticKind::UnresolvedBinding));
    }

    #[test]
    fn test_extract_binding_produces() {
        let step = make_step("cbu.create", "(cbu.create :name \"Acme\" :as @my-cbu)");
        let bindings = extract_bindings(&step);
        assert_eq!(bindings.produces, vec!["my-cbu"]);
        assert!(bindings.consumes.is_empty());
    }

    #[test]
    fn test_extract_binding_consumes() {
        let step = make_step(
            "cbu-role.assign",
            "(cbu-role.assign :cbu-id @my-cbu :entity-id @person)",
        );
        let bindings = extract_bindings(&step);
        assert!(bindings.produces.is_empty());
        assert_eq!(bindings.consumes.len(), 2);
        assert!(bindings.consumes.contains(&"my-cbu".to_string()));
        assert!(bindings.consumes.contains(&"person".to_string()));
    }

    #[test]
    fn test_self_reference_not_dependency() {
        // Step produces and references same binding — should not self-depend.
        let step = make_step(
            "cbu.create",
            "(cbu.create :name \"Acme\" :as @cbu :parent @cbu)",
        );
        let bindings = extract_bindings(&step);
        assert_eq!(bindings.produces, vec!["cbu"]);
        // @cbu after :as is a production, @cbu as parent is also in produced_set,
        // so it should NOT appear in consumes.
        assert!(bindings.consumes.is_empty());
    }

    #[test]
    fn test_reordered_diagnostic_present() {
        let steps = vec![
            make_step("b.use", "(b.use :ref @a)"),
            make_step("a.create", "(a.create :as @a)"),
        ];
        let result = assemble_plan(steps).unwrap();
        assert!(result.reordered);
        assert!(result
            .diagnostics
            .iter()
            .any(|d| d.kind == DiagnosticKind::Reordered));
    }
}
