//! Topological sorting for DSL statements
//!
//! Reorders statements so that producers come before consumers.
//! This enables "write in any order" semantics for IDE and agent use.

use std::collections::{HashMap, HashSet, VecDeque};

use super::ast::{AstNode, Literal, Program, Statement};
use super::binding_context::BindingContext;
use super::runtime_registry::RuntimeVerbRegistry;

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
        });
    }

    // Build dependency graph
    // Key: statement index, Value: set of statement indices this depends on
    let mut deps: HashMap<usize, HashSet<usize>> = HashMap::new();

    // Map binding names to statement indices (for pending statements only)
    let mut binding_to_stmt: HashMap<String, usize> = HashMap::new();

    // First pass: record what each statement produces
    for (idx, stmt) in statements.iter().enumerate() {
        deps.insert(idx, HashSet::new());

        if let Statement::VerbCall(vc) = stmt {
            if let Some(ref binding) = vc.binding {
                binding_to_stmt.insert(binding.clone(), idx);
            }
        }
    }

    // Second pass: record dependencies based on symbol references in arguments
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
                let deg = in_degree.get_mut(other_idx).unwrap();
                *deg -= 1;
                if *deg == 0 {
                    next_ready.push(*other_idx);
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
    })
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
                    deps.get_mut(&current_idx).unwrap().insert(producer_idx);
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
}
