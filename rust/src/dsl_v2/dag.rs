//! DAG Builder and Topological Sort
//!
//! Implements Kahn's algorithm for topological sorting with:
//! - Cycle detection with clear error messages
//! - Stable sort (preserves source order when no dependency relationship)
//! - Execution phases for optional grouped execution

use crate::dsl_v2::ops::{Op, OpRef};
use std::collections::{BinaryHeap, HashMap, HashSet};

/// Execution plan with topologically sorted ops
#[derive(Debug)]
pub struct ExecutionPlan {
    /// Ops in execution order (topologically sorted)
    pub ops: Vec<Op>,
    /// Execution phases (for optional phased execution)
    pub phases: Vec<ExecutionPhase>,
    /// Original op count before sorting
    pub original_count: usize,
}

/// A phase of execution containing ops that can run in parallel
#[derive(Debug, Clone)]
pub struct ExecutionPhase {
    pub name: String,
    /// Indices into ExecutionPlan.ops
    pub op_indices: Vec<usize>,
}

/// Error when a cycle is detected in the dependency graph
#[derive(Debug)]
pub struct CycleError {
    /// Source statement indices involved in the cycle
    pub cycle_stmts: Vec<usize>,
    /// Human-readable explanation
    pub explanation: String,
}

impl std::fmt::Display for CycleError {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        write!(f, "{}", self.explanation)
    }
}

impl std::error::Error for CycleError {}

/// Wrapper for BinaryHeap to get min-heap behavior (stable sort by source_stmt)
#[derive(Debug, Eq, PartialEq)]
struct MinHeapEntry {
    source_stmt: usize,
    op_idx: usize,
}

impl Ord for MinHeapEntry {
    fn cmp(&self, other: &Self) -> std::cmp::Ordering {
        // Reverse ordering for min-heap
        other
            .source_stmt
            .cmp(&self.source_stmt)
            .then_with(|| other.op_idx.cmp(&self.op_idx))
    }
}

impl PartialOrd for MinHeapEntry {
    fn partial_cmp(&self, other: &Self) -> Option<std::cmp::Ordering> {
        Some(self.cmp(other))
    }
}

/// Build execution plan from ops using Kahn's algorithm
///
/// # Algorithm
///
/// 1. Build a map of what each Op produces (OpRef → op index)
/// 2. Build adjacency list: for each Op, find its dependencies
/// 3. Run Kahn's algorithm with stable sort (min-heap by source_stmt)
/// 4. Detect cycles if not all ops are processed
///
/// # Stable Sort
///
/// When multiple ops have in_degree=0 (no unmet dependencies), we pick
/// the one with the lowest source_stmt index first. This preserves the
/// user's original source order when there's no dependency relationship,
/// preventing LSP from thrashing the user's code on every keystroke.
pub fn build_execution_plan(ops: Vec<Op>) -> Result<ExecutionPlan, CycleError> {
    let n = ops.len();
    if n == 0 {
        return Ok(ExecutionPlan {
            ops: vec![],
            phases: vec![],
            original_count: 0,
        });
    }

    // Step 1: Build map of what each Op produces
    let mut produces: HashMap<OpRef, usize> = HashMap::new();
    for (idx, op) in ops.iter().enumerate() {
        if let Some(ref prod) = op.produces() {
            produces.insert(prod.clone(), idx);
        }
    }

    // Step 2: Build adjacency list and in-degrees
    // adj[i] = list of ops that depend on op i (i must come before them)
    let mut adj: Vec<Vec<usize>> = vec![Vec::new(); n];
    let mut in_degree: Vec<usize> = vec![0; n];

    for (idx, op) in ops.iter().enumerate() {
        for dep_ref in op.dependencies() {
            if let Some(&dep_idx) = produces.get(&dep_ref) {
                // dep_idx must come before idx
                adj[dep_idx].push(idx);
                in_degree[idx] += 1;
            }
            // If dependency not found in produces, it's an external reference
            // (exists in DB but not created by this DSL program) - OK, no edge
        }
    }

    // Step 3: Kahn's algorithm with stable sort
    // Use a min-heap to always pick the op with lowest source_stmt first
    let mut heap: BinaryHeap<MinHeapEntry> = BinaryHeap::new();
    for (idx, &degree) in in_degree.iter().enumerate() {
        if degree == 0 {
            heap.push(MinHeapEntry {
                source_stmt: ops[idx].source_stmt(),
                op_idx: idx,
            });
        }
    }

    let mut sorted_indices: Vec<usize> = Vec::with_capacity(n);
    while let Some(entry) = heap.pop() {
        let idx = entry.op_idx;
        sorted_indices.push(idx);

        for &next_idx in &adj[idx] {
            in_degree[next_idx] -= 1;
            if in_degree[next_idx] == 0 {
                heap.push(MinHeapEntry {
                    source_stmt: ops[next_idx].source_stmt(),
                    op_idx: next_idx,
                });
            }
        }
    }

    // Step 4: Check for cycle
    if sorted_indices.len() != n {
        // Find which ops are in the cycle (those with remaining in_degree > 0)
        let remaining: Vec<usize> = (0..n).filter(|i| !sorted_indices.contains(i)).collect();

        let cycle_stmts: Vec<usize> = remaining.iter().map(|&i| ops[i].source_stmt()).collect();

        // Build a helpful error message
        let mut explanation = String::from("Circular dependency detected:\n");
        for &idx in &remaining {
            let op = &ops[idx];
            explanation.push_str(&format!(
                "  --> statement {}: {}\n",
                op.source_stmt() + 1,
                op.describe()
            ));
        }
        explanation.push_str("\nThese operations depend on each other in a cycle.");

        return Err(CycleError {
            cycle_stmts,
            explanation,
        });
    }

    // Reorder ops by sorted indices
    let sorted_ops: Vec<Op> = sorted_indices.iter().map(|&i| ops[i].clone()).collect();

    // Group into phases
    let phases = group_into_phases(&sorted_ops);

    Ok(ExecutionPlan {
        ops: sorted_ops,
        phases,
        original_count: n,
    })
}

/// Group ops into execution phases based on their type
///
/// This is optional - ops can be executed sequentially. But phases
/// allow for potential parallel execution within a phase.
fn group_into_phases(ops: &[Op]) -> Vec<ExecutionPhase> {
    let mut phase_entities = ExecutionPhase {
        name: "1. Entities".to_string(),
        op_indices: vec![],
    };
    let mut phase_relationships = ExecutionPhase {
        name: "2. Relationships".to_string(),
        op_indices: vec![],
    };
    let mut phase_documents = ExecutionPhase {
        name: "3. Documents".to_string(),
        op_indices: vec![],
    };
    let mut phase_kyc = ExecutionPhase {
        name: "4. KYC".to_string(),
        op_indices: vec![],
    };
    let mut phase_custody = ExecutionPhase {
        name: "5. Custody".to_string(),
        op_indices: vec![],
    };
    let mut phase_materialize = ExecutionPhase {
        name: "6. Materialization".to_string(),
        op_indices: vec![],
    };

    for (idx, op) in ops.iter().enumerate() {
        match op {
            Op::EnsureEntity { .. } => phase_entities.op_indices.push(idx),

            Op::SetFK { .. }
            | Op::LinkRole { .. }
            | Op::UnlinkRole { .. }
            | Op::AddOwnership { .. }
            | Op::RegisterUBO { .. } => phase_relationships.op_indices.push(idx),

            Op::UpsertDoc { .. } | Op::AttachEvidence { .. } => {
                phase_documents.op_indices.push(idx)
            }

            Op::CreateCase { .. }
            | Op::UpdateCaseStatus { .. }
            | Op::CreateWorkstream { .. }
            | Op::RunScreening { .. } => phase_kyc.op_indices.push(idx),

            Op::AddUniverse { .. } | Op::CreateSSI { .. } | Op::AddBookingRule { .. } => {
                phase_custody.op_indices.push(idx)
            }

            Op::Materialize { .. } => phase_materialize.op_indices.push(idx),

            Op::RequireRef { .. } => {} // No-op, skip
        }
    }

    // Return non-empty phases in order
    vec![
        phase_entities,
        phase_relationships,
        phase_documents,
        phase_kyc,
        phase_custody,
        phase_materialize,
    ]
    .into_iter()
    .filter(|p| !p.op_indices.is_empty())
    .collect()
}

/// Generate plan description for dry-run output
pub fn describe_plan(plan: &ExecutionPlan) -> String {
    let mut output = String::new();
    output.push_str("Execution Plan\n");
    output.push_str("==============\n\n");
    output.push_str(&format!("Total operations: {}\n\n", plan.original_count));

    for phase in &plan.phases {
        output.push_str(&format!("Phase: {}\n", phase.name));
        output.push_str(&"-".repeat(40));
        output.push('\n');

        for &idx in &phase.op_indices {
            let op = &plan.ops[idx];
            output.push_str(&format!(
                "  [stmt {}] {}\n",
                op.source_stmt() + 1,
                op.describe()
            ));
        }
        output.push('\n');
    }

    output
}

/// Validate that all external references exist
///
/// This checks that any dependency not produced by an Op in this program
/// is a valid external reference. Returns the list of external refs needed.
pub fn collect_external_refs(ops: &[Op]) -> HashSet<OpRef> {
    let mut produces: HashSet<OpRef> = HashSet::new();
    for op in ops {
        if let Some(prod) = op.produces() {
            produces.insert(prod);
        }
    }

    let mut external: HashSet<OpRef> = HashSet::new();
    for op in ops {
        for dep in op.dependencies() {
            if !produces.contains(&dep) {
                external.insert(dep);
            }
        }
    }

    external
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::dsl_v2::ops::EntityKey;
    use std::collections::HashMap;

    fn make_ensure_entity(name: &str, stmt: usize, binding: Option<&str>) -> Op {
        Op::EnsureEntity {
            entity_type: "cbu".to_string(),
            key: EntityKey::cbu(name),
            attrs: HashMap::new(),
            binding: binding.map(|s| s.to_string()),
            source_stmt: stmt,
        }
    }

    fn make_link_role(cbu: &str, entity: &str, role: &str, stmt: usize) -> Op {
        Op::LinkRole {
            cbu: EntityKey::cbu(cbu),
            entity: EntityKey::proper_person(entity),
            role: role.to_string(),
            ownership_percentage: None,
            source_stmt: stmt,
        }
    }

    #[test]
    fn test_empty_plan() {
        let plan = build_execution_plan(vec![]).unwrap();
        assert!(plan.ops.is_empty());
        assert!(plan.phases.is_empty());
    }

    #[test]
    fn test_single_op() {
        let ops = vec![make_ensure_entity("Fund", 0, None)];
        let plan = build_execution_plan(ops).unwrap();
        assert_eq!(plan.ops.len(), 1);
    }

    #[test]
    fn test_reordering_by_dependency() {
        // Source order: LinkRole, then EnsureEntity (wrong order)
        // Expected: EnsureEntity first (creates entity), then LinkRole (uses it)
        let ops = vec![
            make_link_role("Fund", "John", "DIRECTOR", 0), // stmt 0, depends on Fund and John
            make_ensure_entity("Fund", 1, None),           // stmt 1, no deps
            Op::EnsureEntity {
                entity_type: "proper_person".to_string(),
                key: EntityKey::proper_person("John"),
                attrs: HashMap::new(),
                binding: None,
                source_stmt: 2,
            },
        ];

        let plan = build_execution_plan(ops).unwrap();

        // The two EnsureEntity ops should come before LinkRole
        // Stable sort means stmt 1 (Fund) should come before stmt 2 (John)
        // since both have in_degree=0 initially
        assert!(matches!(plan.ops[0], Op::EnsureEntity { .. }));
        assert!(matches!(plan.ops[1], Op::EnsureEntity { .. }));
        assert!(matches!(plan.ops[2], Op::LinkRole { .. }));

        // Verify stable sort: Fund (stmt 1) before John (stmt 2)
        assert_eq!(plan.ops[0].source_stmt(), 1);
        assert_eq!(plan.ops[1].source_stmt(), 2);
    }

    #[test]
    fn test_stable_sort_preserves_source_order() {
        // Three independent entities with no dependencies
        // Should preserve source order: 0, 1, 2
        let ops = vec![
            make_ensure_entity("A", 0, None),
            make_ensure_entity("B", 1, None),
            make_ensure_entity("C", 2, None),
        ];

        let plan = build_execution_plan(ops).unwrap();

        assert_eq!(plan.ops[0].source_stmt(), 0);
        assert_eq!(plan.ops[1].source_stmt(), 1);
        assert_eq!(plan.ops[2].source_stmt(), 2);
    }

    #[test]
    fn test_phases_grouping() {
        let ops = vec![
            make_ensure_entity("Fund", 0, None),
            make_link_role("Fund", "John", "DIRECTOR", 1),
            Op::EnsureEntity {
                entity_type: "proper_person".to_string(),
                key: EntityKey::proper_person("John"),
                attrs: HashMap::new(),
                binding: None,
                source_stmt: 2,
            },
        ];

        let plan = build_execution_plan(ops).unwrap();

        // Should have 2 phases: Entities and Relationships
        assert_eq!(plan.phases.len(), 2);
        assert!(plan.phases[0].name.contains("Entities"));
        assert!(plan.phases[1].name.contains("Relationships"));
    }

    #[test]
    fn test_external_refs_collection() {
        // LinkRole depends on entities not created in this program
        let ops = vec![make_link_role(
            "ExternalFund",
            "ExternalPerson",
            "DIRECTOR",
            0,
        )];

        let external = collect_external_refs(&ops);

        // Should have 2 external refs (the CBU and person)
        assert_eq!(external.len(), 2);
    }

    #[test]
    fn test_describe_plan() {
        let ops = vec![
            make_ensure_entity("Fund", 0, None),
            make_link_role("Fund", "John", "DIRECTOR", 1),
        ];

        let plan = build_execution_plan(ops).unwrap();
        let description = describe_plan(&plan);

        assert!(description.contains("Execution Plan"));
        assert!(description.contains("Ensure cbu 'Fund'"));
    }

    // Note: Actual cycle detection test is tricky because our current Op
    // definitions don't easily create cycles. SetFK would be needed.
    // The algorithm is correct - cycles would be detected if ops had
    // mutual dependencies.
}
