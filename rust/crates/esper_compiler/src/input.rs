//! Graph input abstraction for the snapshot compiler.

use serde::{Deserialize, Serialize};
use std::collections::HashMap;

/// Abstract interface for graph data sources.
///
/// This trait allows the compiler to work with different data sources
/// (database queries, in-memory graphs, test fixtures).
pub trait GraphInput {
    /// Get all entity IDs.
    fn entity_ids(&self) -> Vec<u64>;

    /// Get entity by ID.
    fn get_entity(&self, id: u64) -> Option<EntityInput>;

    /// Get all edges.
    fn edges(&self) -> Vec<EdgeInput>;

    /// Get edges originating from an entity.
    fn edges_from(&self, entity_id: u64) -> Vec<EdgeInput>;

    /// Get edges pointing to an entity.
    fn edges_to(&self, entity_id: u64) -> Vec<EdgeInput>;

    /// Get root entities (no incoming parent edges).
    fn roots(&self) -> Vec<u64>;

    /// Get children of an entity.
    fn children(&self, entity_id: u64) -> Vec<u64>;

    /// Get parent of an entity.
    fn parent(&self, entity_id: u64) -> Option<u64>;

    /// Total entity count.
    fn entity_count(&self) -> usize {
        self.entity_ids().len()
    }

    /// Total edge count.
    fn edge_count(&self) -> usize {
        self.edges().len()
    }
}

/// Entity data for compilation.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct EntityInput {
    /// Unique entity ID.
    pub id: u64,
    /// Display name.
    pub name: String,
    /// Entity kind ID (maps to kind metadata).
    pub kind_id: u32,
    /// Optional pre-computed position.
    pub position: Option<(f32, f32)>,
    /// Detail reference (for drill-down).
    pub detail_ref: Option<u64>,
    /// Additional metadata (JSON).
    pub metadata: Option<serde_json::Value>,
}

impl EntityInput {
    /// Create a minimal entity.
    pub fn new(id: u64, name: impl Into<String>, kind_id: u32) -> Self {
        Self {
            id,
            name: name.into(),
            kind_id,
            position: None,
            detail_ref: None,
            metadata: None,
        }
    }

    /// Set position.
    pub fn with_position(mut self, x: f32, y: f32) -> Self {
        self.position = Some((x, y));
        self
    }

    /// Set detail reference.
    pub fn with_detail_ref(mut self, detail_ref: u64) -> Self {
        self.detail_ref = Some(detail_ref);
        self
    }
}

/// Edge connecting two entities.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, Serialize, Deserialize)]
pub struct EdgeInput {
    /// Source entity ID.
    pub from: u64,
    /// Target entity ID.
    pub to: u64,
    /// Edge kind.
    pub kind: EdgeKind,
}

impl EdgeInput {
    /// Create a new edge.
    pub fn new(from: u64, to: u64, kind: EdgeKind) -> Self {
        Self { from, to, kind }
    }

    /// Create a parent edge (from child to parent).
    pub fn parent(child: u64, parent: u64) -> Self {
        Self::new(child, parent, EdgeKind::Parent)
    }

    /// Create a sibling edge.
    pub fn sibling(a: u64, b: u64) -> Self {
        Self::new(a, b, EdgeKind::Sibling)
    }
}

/// Edge relationship types.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, Default, Serialize, Deserialize)]
pub enum EdgeKind {
    /// Parent-child relationship.
    #[default]
    Parent,
    /// Sibling relationship (same level).
    Sibling,
    /// Control/ownership relationship.
    Control,
    /// Reference/link relationship.
    Reference,
    /// Door (cross-chamber navigation).
    Door,
}

/// In-memory graph implementation for testing and simple use cases.
#[derive(Debug, Default)]
pub struct MemoryGraphInput {
    entities: HashMap<u64, EntityInput>,
    edges: Vec<EdgeInput>,
    // Cached indices
    edges_from: HashMap<u64, Vec<EdgeInput>>,
    edges_to: HashMap<u64, Vec<EdgeInput>>,
    children_of: HashMap<u64, Vec<u64>>,
    parent_of: HashMap<u64, u64>,
}

impl MemoryGraphInput {
    /// Create a new empty graph.
    pub fn new() -> Self {
        Self::default()
    }

    /// Add an entity.
    pub fn add_entity(mut self, entity: EntityInput) -> Self {
        self.entities.insert(entity.id, entity);
        self
    }

    /// Add an edge.
    pub fn add_edge(mut self, from: u64, to: u64, kind: EdgeKind) -> Self {
        let edge = EdgeInput { from, to, kind };
        self.edges.push(edge);

        // Update indices
        self.edges_from.entry(from).or_default().push(edge);
        self.edges_to.entry(to).or_default().push(edge);

        // Track parent-child for hierarchy
        if kind == EdgeKind::Parent {
            self.parent_of.insert(from, to);
            self.children_of.entry(to).or_default().push(from);
        }

        self
    }

    /// Build a simple tree from entity IDs.
    ///
    /// First entity is root, others are children.
    pub fn simple_tree(entities: Vec<(u64, &str, u32)>) -> Self {
        let mut graph = Self::new();

        for (id, name, kind_id) in &entities {
            graph = graph.add_entity(EntityInput::new(*id, *name, *kind_id));
        }

        // First entity is root, link others as children
        if entities.len() > 1 {
            let root_id = entities[0].0;
            for (id, _, _) in entities.iter().skip(1) {
                graph = graph.add_edge(*id, root_id, EdgeKind::Parent);
            }
        }

        graph
    }
}

impl GraphInput for MemoryGraphInput {
    fn entity_ids(&self) -> Vec<u64> {
        self.entities.keys().copied().collect()
    }

    fn get_entity(&self, id: u64) -> Option<EntityInput> {
        self.entities.get(&id).cloned()
    }

    fn edges(&self) -> Vec<EdgeInput> {
        self.edges.clone()
    }

    fn edges_from(&self, entity_id: u64) -> Vec<EdgeInput> {
        self.edges_from.get(&entity_id).cloned().unwrap_or_default()
    }

    fn edges_to(&self, entity_id: u64) -> Vec<EdgeInput> {
        self.edges_to.get(&entity_id).cloned().unwrap_or_default()
    }

    fn roots(&self) -> Vec<u64> {
        self.entities
            .keys()
            .filter(|id| !self.parent_of.contains_key(id))
            .copied()
            .collect()
    }

    fn children(&self, entity_id: u64) -> Vec<u64> {
        self.children_of
            .get(&entity_id)
            .cloned()
            .unwrap_or_default()
    }

    fn parent(&self, entity_id: u64) -> Option<u64> {
        self.parent_of.get(&entity_id).copied()
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn memory_graph_basic() {
        let graph = MemoryGraphInput::new()
            .add_entity(EntityInput::new(1, "Root", 0))
            .add_entity(EntityInput::new(2, "Child A", 1))
            .add_entity(EntityInput::new(3, "Child B", 1))
            .add_edge(2, 1, EdgeKind::Parent)
            .add_edge(3, 1, EdgeKind::Parent);

        assert_eq!(graph.entity_count(), 3);
        assert_eq!(graph.edge_count(), 2);
        assert_eq!(graph.roots(), vec![1]);
        assert_eq!(graph.parent(2), Some(1));
        assert!(graph.children(1).contains(&2));
        assert!(graph.children(1).contains(&3));
    }

    #[test]
    fn simple_tree_builder() {
        let graph = MemoryGraphInput::simple_tree(vec![
            (1, "Root", 0),
            (2, "A", 1),
            (3, "B", 1),
            (4, "C", 1),
        ]);

        assert_eq!(graph.entity_count(), 4);
        assert_eq!(graph.roots(), vec![1]);
        assert_eq!(graph.children(1).len(), 3);
    }

    #[test]
    fn entity_builder() {
        let entity = EntityInput::new(42, "Test Entity", 1)
            .with_position(100.0, 200.0)
            .with_detail_ref(99);

        assert_eq!(entity.id, 42);
        assert_eq!(entity.position, Some((100.0, 200.0)));
        assert_eq!(entity.detail_ref, Some(99));
    }
}
