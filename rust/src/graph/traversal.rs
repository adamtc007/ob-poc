//! Graph traversal algorithms for UBO discovery

use super::*;
use crate::ast::EdgeType;

pub(crate) struct GraphTraversal<'a> {
    graph: &'a PropertyGraph,
}

impl<'a> GraphTraversal<'a> {
    pub fn new(graph: &'a PropertyGraph) -> Self {
        GraphTraversal { graph }
    }

    /// Find all paths from target to terminal nodes (e.g., Person nodes)
    pub(crate) fn find_ownership_paths(&self, start_node: &str, max_depth: usize) -> Vec<OwnershipPath> {
        let mut paths = Vec::new();
        let mut current_path = Vec::new();

        self.dfs_ownership(start_node, &mut current_path, &mut paths, 0, max_depth);

        paths
    }

    fn dfs_ownership(
        &self,
        node_id: &str,
        current_path: &mut Vec<String>,
        all_paths: &mut Vec<OwnershipPath>,
        depth: usize,
        max_depth: usize,
    ) {
        if depth >= max_depth {
            return;
        }

        current_path.push(node_id.to_string());

        // Check if this is a terminal node (Person)
        if let Some(node) = self.graph.get_node(node_id) {
            if matches!(node.label, EntityLabel::Person) {
                all_paths.push(OwnershipPath {
                    nodes: current_path.clone(),
                    effective_ownership: self.calculate_path_ownership(current_path),
                });
            }
        }

        // Traverse incoming ownership edges
        for edge in self.graph.get_incoming_edges(node_id) {
            if matches!(edge.edge_type, EdgeType::HasOwnership) {
                self.dfs_ownership(&edge.from, current_path, all_paths, depth + 1, max_depth);
            }
        }

        current_path.pop();
    }

    fn calculate_path_ownership(&self, path: &[String]) -> f64 {
        let mut ownership = 1.0;

        for i in 0..path.len() - 1 {
            let from = &path[i];
            let to = &path[i + 1];

            // Find edge between these nodes
            for edge in &self.graph.edges {
                if edge.from == *from && edge.to == *to {
                    if let Some(percent) = edge.properties.get("percent") {
                        if let Some(pct) = percent.as_f64() {
                            ownership *= pct / 100.0;
                        }
                    }
                }
            }
        }

        ownership * 100.0
    }
}

#[derive(Debug, Clone)]
pub(crate) struct OwnershipPath {
    pub nodes: Vec<String>,
    pub effective_ownership: f64,
}
