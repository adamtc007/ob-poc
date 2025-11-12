//! Property Graph model for UBO network representation

use serde::{Deserialize, Serialize};
use std::collections::HashMap;

pub mod prong;
pub mod traversal;

use crate::ast::{EdgeType, EntityLabel};

#[derive(Debug, Clone, Serialize, Deserialize)]
pub(crate) struct PropertyGraph {
    pub nodes: HashMap<String, Node>,
    pub edges: Vec<Edge>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub(crate) struct Node {
    pub node_id: String,
    pub label: EntityLabel,
    pub properties: HashMap<String, serde_json::Value>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct Edge {
    pub edge_id: String,
    pub from: String,
    pub to: String,
    pub edge_type: EdgeType,
    pub properties: HashMap<String, serde_json::Value>,
    pub evidenced_by: Vec<String>,
}

impl PropertyGraph {
    pub fn new() -> Self {
        PropertyGraph {
            nodes: HashMap::new(),
            edges: Vec::new(),
        }
    }

    pub(crate) fn add_node(&mut self, node: Node) {
        self.nodes.insert(node.node_id.clone(), node);
    }

    pub(crate) fn add_edge(&mut self, edge: Edge) {
        self.edges.push(edge);
    }

    pub(crate) fn get_node(&self, node_id: &str) -> Option<&Node> {
        self.nodes.get(node_id)
    }

    pub(crate) fn get_outgoing_edges(&self, node_id: &str) -> Vec<&Edge> {
        self.edges.iter().filter(|e| e.from == node_id).collect()
    }

    pub(crate) fn get_incoming_edges(&self, node_id: &str) -> Vec<&Edge> {
        self.edges.iter().filter(|e| e.to == node_id).collect()
    }
}

impl Default for PropertyGraph {
    fn default() -> Self {
        Self::new()
    }
}
