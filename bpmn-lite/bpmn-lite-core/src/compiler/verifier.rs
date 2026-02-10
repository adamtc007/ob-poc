use super::ir::*;
use crate::types::{Addr, CompiledProgram, Instr};
use anyhow::{anyhow, Result};
use petgraph::visit::Dfs;
use std::collections::HashMap;

/// Verification errors.
#[derive(Debug, Clone)]
pub struct VerifyError {
    pub message: String,
    pub element_id: Option<String>,
}

impl std::fmt::Display for VerifyError {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        if let Some(id) = &self.element_id {
            write!(f, "[{}] {}", id, self.message)
        } else {
            write!(f, "{}", self.message)
        }
    }
}

/// Verify structural invariants of the IR graph.
///
/// Returns a list of errors. Empty list means the graph is valid.
pub fn verify(graph: &IRGraph) -> Vec<VerifyError> {
    let mut errors = Vec::new();

    // 1. Exactly one StartEvent
    let starts: Vec<_> = graph
        .node_indices()
        .filter(|&idx| matches!(&graph[idx], IRNode::Start { .. }))
        .collect();

    if starts.is_empty() {
        errors.push(VerifyError {
            message: "No StartEvent found".to_string(),
            element_id: None,
        });
    } else if starts.len() > 1 {
        errors.push(VerifyError {
            message: format!("Multiple StartEvents found ({})", starts.len()),
            element_id: None,
        });
    }

    // 2. At least one EndEvent
    let ends: Vec<_> = graph
        .node_indices()
        .filter(|&idx| matches!(&graph[idx], IRNode::End { .. }))
        .collect();

    if ends.is_empty() {
        errors.push(VerifyError {
            message: "No EndEvent found".to_string(),
            element_id: None,
        });
    }

    // 3. All nodes reachable from Start (or from BoundaryTimer nodes,
    //    which are alternative entry points for escalation paths)
    if let Some(start_idx) = starts.first() {
        let mut reachable = std::collections::HashSet::new();

        // DFS from Start
        let mut dfs = Dfs::new(graph, *start_idx);
        while let Some(nx) = dfs.next(graph) {
            reachable.insert(nx);
        }

        // Also DFS from each BoundaryTimer/BoundaryError node (escalation/error paths)
        for idx in graph.node_indices() {
            let is_boundary = matches!(
                &graph[idx],
                IRNode::BoundaryTimer { .. } | IRNode::BoundaryError { .. }
            );
            if is_boundary && !reachable.contains(&idx) {
                reachable.insert(idx);
                let mut bdfs = Dfs::new(graph, idx);
                while let Some(nx) = bdfs.next(graph) {
                    reachable.insert(nx);
                }
            }
        }

        for idx in graph.node_indices() {
            if !reachable.contains(&idx) {
                errors.push(VerifyError {
                    message: format!("Unreachable node: {}", graph[idx].id()),
                    element_id: Some(graph[idx].id().to_string()),
                });
            }
        }
    }

    // 4. Parallel gateways: check fork/join pairs
    let forks: Vec<_> = graph
        .node_indices()
        .filter(|&idx| {
            matches!(
                &graph[idx],
                IRNode::GatewayAnd {
                    direction: GatewayDirection::Diverging,
                    ..
                }
            )
        })
        .collect();

    let joins: Vec<_> = graph
        .node_indices()
        .filter(|&idx| {
            matches!(
                &graph[idx],
                IRNode::GatewayAnd {
                    direction: GatewayDirection::Converging,
                    ..
                }
            )
        })
        .collect();

    if forks.len() != joins.len() {
        errors.push(VerifyError {
            message: format!(
                "Mismatched parallel gateways: {} forks, {} joins",
                forks.len(),
                joins.len()
            ),
            element_id: None,
        });
    }

    // 5. All task_type references are non-empty (ServiceTask)
    for idx in graph.node_indices() {
        if let IRNode::ServiceTask { id, task_type, .. } = &graph[idx] {
            if task_type.is_empty() {
                errors.push(VerifyError {
                    message: "ServiceTask has empty task_type".to_string(),
                    element_id: Some(id.clone()),
                });
            }
        }
    }

    // 6. XOR diverging gateways should have at least one outgoing edge with a condition
    //    and exactly one default (no condition)
    for idx in graph.node_indices() {
        if matches!(&graph[idx], IRNode::GatewayXor { .. }) {
            let outgoing: Vec<_> = graph
                .edges_directed(idx, petgraph::Direction::Outgoing)
                .collect();

            if outgoing.len() > 1 {
                let with_condition = outgoing
                    .iter()
                    .filter(|e| e.weight().condition.is_some())
                    .count();
                let without_condition = outgoing.len() - with_condition;

                if without_condition != 1 {
                    errors.push(VerifyError {
                        message: format!(
                            "XOR gateway should have exactly 1 default edge, found {}",
                            without_condition
                        ),
                        element_id: Some(graph[idx].id().to_string()),
                    });
                }
            }
        }
    }

    // 7. Boundary event validation
    {
        let mut host_boundary_count: HashMap<String, Vec<String>> = HashMap::new();

        for idx in graph.node_indices() {
            if let IRNode::BoundaryTimer {
                id,
                attached_to,
                interrupting,
                spec,
            } = &graph[idx]
            {
                // 7a. attached_to must reference an existing ServiceTask or HumanWait
                let host_exists = graph.node_indices().any(|other| {
                    matches!(&graph[other],
                        IRNode::ServiceTask { id: host_id, .. } | IRNode::HumanWait { id: host_id, .. }
                        if host_id == attached_to
                    )
                });
                if !host_exists {
                    errors.push(VerifyError {
                        message: format!(
                            "BoundaryTimer '{}' attachedToRef '{}' does not reference a task",
                            id, attached_to
                        ),
                        element_id: Some(id.clone()),
                    });
                }

                // 7b. Cycle timers MUST be non-interrupting (cycle + interrupting is invalid)
                if let TimerSpec::Cycle { .. } = &spec {
                    if *interrupting {
                        errors.push(VerifyError {
                            message: format!(
                                "BoundaryTimer '{}': cycle timers must be non-interrupting (cancelActivity=\"false\")",
                                id
                            ),
                            element_id: Some(id.clone()),
                        });
                    }
                }

                // 7c. Must have at least one outgoing edge
                let outgoing = graph
                    .edges_directed(idx, petgraph::Direction::Outgoing)
                    .count();
                if outgoing == 0 {
                    errors.push(VerifyError {
                        message: format!("BoundaryTimer '{}' has no outgoing sequence flow", id),
                        element_id: Some(id.clone()),
                    });
                }

                host_boundary_count
                    .entry(attached_to.clone())
                    .or_default()
                    .push(id.clone());
            }
        }

        // 7d. Phase 2: max 1 boundary timer per host task
        for (host_id, boundary_ids) in &host_boundary_count {
            if boundary_ids.len() > 1 {
                errors.push(VerifyError {
                    message: format!(
                        "Task '{}' has {} boundary timers (max 1 supported in this version): [{}]",
                        host_id,
                        boundary_ids.len(),
                        boundary_ids.join(", ")
                    ),
                    element_id: Some(host_id.clone()),
                });
            }
        }
    }

    // 8. Boundary error event validation
    {
        // Track catch-all count per host task
        let mut host_catch_all_count: HashMap<String, Vec<String>> = HashMap::new();

        for idx in graph.node_indices() {
            if let IRNode::BoundaryError {
                id,
                attached_to,
                error_code,
            } = &graph[idx]
            {
                // 8a. attached_to must reference an existing ServiceTask
                let host_exists = graph.node_indices().any(|other| {
                    matches!(&graph[other],
                        IRNode::ServiceTask { id: host_id, .. }
                        if host_id == attached_to
                    )
                });
                if !host_exists {
                    errors.push(VerifyError {
                        message: format!(
                            "BoundaryError '{}' attachedToRef '{}' does not reference a ServiceTask",
                            id, attached_to
                        ),
                        element_id: Some(id.clone()),
                    });
                }

                // 8b. Must have exactly 1 outgoing edge
                let outgoing = graph
                    .edges_directed(idx, petgraph::Direction::Outgoing)
                    .count();
                if outgoing != 1 {
                    errors.push(VerifyError {
                        message: format!(
                            "BoundaryError '{}' must have exactly 1 outgoing edge, found {}",
                            id, outgoing
                        ),
                        element_id: Some(id.clone()),
                    });
                }

                // 8c. Track catch-all (error_code: None) per host
                if error_code.is_none() {
                    host_catch_all_count
                        .entry(attached_to.clone())
                        .or_default()
                        .push(id.clone());
                }
            }
        }

        // 8d. At most 1 catch-all BoundaryError per host task
        for (host_id, catch_all_ids) in &host_catch_all_count {
            if catch_all_ids.len() > 1 {
                errors.push(VerifyError {
                    message: format!(
                        "Task '{}' has {} catch-all error boundaries (max 1): [{}]",
                        host_id,
                        catch_all_ids.len(),
                        catch_all_ids.join(", ")
                    ),
                    element_id: Some(host_id.clone()),
                });
            }
        }
    }

    // 9. Inclusive gateway validation
    {
        let mut diverging_count = 0u32;
        let mut converging_count = 0u32;

        for idx in graph.node_indices() {
            match &graph[idx] {
                IRNode::GatewayInclusive {
                    id,
                    direction: GatewayDirection::Diverging,
                    ..
                } => {
                    diverging_count += 1;
                    let outgoing = graph
                        .edges_directed(idx, petgraph::Direction::Outgoing)
                        .count();
                    if outgoing < 2 {
                        errors.push(VerifyError {
                            message: format!(
                                "Inclusive gateway (diverging) must have ≥2 outgoing edges, found {}",
                                outgoing
                            ),
                            element_id: Some(id.clone()),
                        });
                    }
                }
                IRNode::GatewayInclusive {
                    id,
                    direction: GatewayDirection::Converging,
                    ..
                } => {
                    converging_count += 1;
                    let incoming = graph
                        .edges_directed(idx, petgraph::Direction::Incoming)
                        .count();
                    if incoming < 2 {
                        errors.push(VerifyError {
                            message: format!(
                                "Inclusive gateway (converging) must have ≥2 incoming edges, found {}",
                                incoming
                            ),
                            element_id: Some(id.clone()),
                        });
                    }
                    let outgoing = graph
                        .edges_directed(idx, petgraph::Direction::Outgoing)
                        .count();
                    if outgoing != 1 {
                        errors.push(VerifyError {
                            message: format!(
                                "Inclusive gateway (converging) must have exactly 1 outgoing edge, found {}",
                                outgoing
                            ),
                            element_id: Some(id.clone()),
                        });
                    }
                }
                _ => {}
            }
        }

        // v1 constraint: single inclusive pair per process
        if diverging_count > 1 {
            errors.push(VerifyError {
                message: format!(
                    "Multiple diverging inclusive gateways ({}) not supported in v1",
                    diverging_count
                ),
                element_id: None,
            });
        }
        if converging_count > 1 {
            errors.push(VerifyError {
                message: format!(
                    "Multiple converging inclusive gateways ({}) not supported in v1",
                    converging_count
                ),
                element_id: None,
            });
        }
    }

    errors
}

/// Verify bytecode for bounded-loop safety.
///
/// Rejects backward `Jump`/`BrIf`/`BrIfNot` (infinite loop risk).
/// Allows backward `BrCounterLt` (bounded by counter limit).
pub fn verify_bytecode(program: &CompiledProgram) -> Vec<VerifyError> {
    let mut errors = Vec::new();
    for (addr, instr) in program.program.iter().enumerate() {
        let addr = addr as Addr;
        match instr {
            Instr::Jump { target } | Instr::BrIf { target } | Instr::BrIfNot { target } => {
                if *target < addr {
                    errors.push(VerifyError {
                        message: format!(
                            "Backward jump at addr {} to {} — only BrCounterLt may jump backward",
                            addr, target
                        ),
                        element_id: program.debug_map.get(&addr).cloned(),
                    });
                }
            }
            Instr::BrCounterLt { .. } => {
                // BrCounterLt is allowed to jump backward (it's bounded by limit)
            }
            _ => {}
        }
    }
    errors
}

/// Verify and return Result — convenience wrapper.
pub fn verify_or_err(graph: &IRGraph) -> Result<()> {
    let errors = verify(graph);
    if errors.is_empty() {
        Ok(())
    } else {
        let msgs: Vec<String> = errors.iter().map(|e| e.to_string()).collect();
        Err(anyhow!("Verification failed:\n{}", msgs.join("\n")))
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    /// A4.T5: Verifier rejects graph with no StartEvent
    #[test]
    fn test_no_start_event() {
        let mut graph = IRGraph::new();
        graph.add_node(IRNode::End {
            id: "end1".to_string(),
            terminate: false,
        });

        let errors = verify(&graph);
        assert!(errors.iter().any(|e| e.message.contains("No StartEvent")));
    }

    /// A4.T6: Verifier rejects unstructured parallel gateway
    #[test]
    fn test_unmatched_parallel_gateways() {
        let mut graph = IRGraph::new();
        let start = graph.add_node(IRNode::Start {
            id: "start".to_string(),
        });
        let fork = graph.add_node(IRNode::GatewayAnd {
            id: "fork1".to_string(),
            name: "Fork".to_string(),
            direction: GatewayDirection::Diverging,
        });
        let end = graph.add_node(IRNode::End {
            id: "end1".to_string(),
            terminate: false,
        });

        graph.add_edge(
            start,
            fork,
            IREdge {
                id: "f1".to_string(),
                condition: None,
            },
        );
        graph.add_edge(
            fork,
            end,
            IREdge {
                id: "f2".to_string(),
                condition: None,
            },
        );

        let errors = verify(&graph);
        assert!(errors
            .iter()
            .any(|e| e.message.contains("Mismatched parallel gateways")));
    }
}
