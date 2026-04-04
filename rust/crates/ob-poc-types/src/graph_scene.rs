//! GraphSceneModel — server-prepared visual projection of the semantic struct.
//!
//! Projected from `HydratedConstellation`. Defines what is drawable, selectable,
//! drillable. Server-authoritative in meaning. Client renders and animates but
//! may not reinterpret.
//!
//! Lives in ob-poc-types (not sem_os_core) so the WASM observatory crate can
//! consume it without pulling in tokio/prost/async dependencies.

use serde::{Deserialize, Serialize};

use crate::galaxy::ViewLevel;

// ── GraphSceneModel ──────────────────────────────────────────

/// The render scene — server-prepared projection of a constellation.
///
/// Every field is derivable from `HydratedConstellation` + `HydratedSlot`.
/// The projection function lives in `sem_os_core::observatory::graph_scene_projection`.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct GraphSceneModel {
    /// Scene generation counter — incremented on each server update.
    /// Used by the client to detect changes and invalidate caches.
    pub generation: u64,
    /// Which view level this scene represents.
    pub level: ViewLevel,
    /// Layout strategy the client should apply.
    pub layout_strategy: LayoutStrategy,
    /// Nodes in the scene (slots, entities, clusters, etc.).
    pub nodes: Vec<SceneNode>,
    /// Edges between nodes (dependencies, parent-child, ownership, etc.).
    pub edges: Vec<SceneEdge>,
    /// Visual groups (clusters, slot-type groupings).
    #[serde(default, skip_serializing_if = "Vec::is_empty")]
    pub groups: Vec<SceneGroup>,
    /// Nodes that support drill-down (clicking opens a deeper level).
    #[serde(default, skip_serializing_if = "Vec::is_empty")]
    pub drill_targets: Vec<DrillTarget>,
    /// Depth encoding hint (max depth in the scene for color mapping).
    #[serde(default)]
    pub max_depth: usize,
}

// ── SceneNode ────────────────────────────────────────────────

/// A node in the render scene.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct SceneNode {
    /// Unique node ID (slot path, entity UUID, cluster ID).
    pub id: String,
    /// Display label.
    pub label: String,
    /// Node type for rendering dispatch.
    pub node_type: SceneNodeType,
    /// Computed state (e.g., "filled", "empty", "complete", "blocked").
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub state: Option<String>,
    /// Progress percentage (0–100).
    #[serde(default)]
    pub progress: u8,
    /// Whether this node is blocking downstream progress.
    #[serde(default)]
    pub blocking: bool,
    /// Depth in the constellation tree (for depth encoding).
    #[serde(default)]
    pub depth: usize,
    /// Server-suggested position hint (optional — client may override via layout).
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub position_hint: Option<(f32, f32)>,
    /// Badges for quick visual indicators.
    #[serde(default, skip_serializing_if = "Vec::is_empty")]
    pub badges: Vec<SceneBadge>,
    /// Number of child nodes (for size/weight rendering).
    #[serde(default)]
    pub child_count: usize,
    /// Group ID this node belongs to (references SceneGroup.id).
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub group_id: Option<String>,
}

/// Node type classification for rendering dispatch.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum SceneNodeType {
    Cbu,
    Entity,
    EntityGraph,
    Case,
    Tollgate,
    Mandate,
    Cluster,
    /// Aggregated summary node (e.g., "14 CBUs" at Universe level).
    Aggregate,
}

/// A badge on a scene node.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct SceneBadge {
    pub badge_type: String,
    pub label: String,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub color: Option<String>,
}

// ── SceneEdge ────────────────────────────────────────────────

/// An edge between two scene nodes.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct SceneEdge {
    /// Source node ID.
    pub source: String,
    /// Target node ID.
    pub target: String,
    /// Edge type for rendering (dependency, parent-child, ownership, control, etc.).
    pub edge_type: SceneEdgeType,
    /// Optional label.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub label: Option<String>,
    /// Edge weight (e.g., ownership percentage).
    #[serde(default)]
    pub weight: f32,
}

/// Edge type classification.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum SceneEdgeType {
    Dependency,
    ParentChild,
    Ownership,
    Control,
    SharedEntity,
    ServiceProvider,
}

// ── SceneGroup ───────────────────────────────────────────────

/// A visual group of nodes (e.g., jurisdiction cluster, slot-type group).
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct SceneGroup {
    /// Unique group ID.
    pub id: String,
    /// Display label.
    pub label: String,
    /// IDs of nodes in this group.
    pub node_ids: Vec<String>,
    /// Whether the group is visually collapsed.
    #[serde(default)]
    pub collapsed: bool,
    /// Optional boundary hint for layout.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub boundary_hint: Option<GroupBoundary>,
}

/// Boundary hint for a group.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct GroupBoundary {
    pub center_x: f32,
    pub center_y: f32,
    pub radius: f32,
}

// ── DrillTarget ──────────────────────────────────────────────

/// A node that supports drill-down navigation.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct DrillTarget {
    /// Node ID that is drillable.
    pub node_id: String,
    /// What level drilling into this node goes to.
    pub target_level: ViewLevel,
    /// Label for the drill action.
    pub drill_label: String,
}

// ── LayoutStrategy ───────────────────────────────────────────

/// Server-authored layout instruction (not a client preference).
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, Serialize, Deserialize, Default)]
#[serde(rename_all = "snake_case")]
pub enum LayoutStrategy {
    /// Force-directed with cluster grouping (Universe level).
    #[default]
    ForceDirected,
    /// Force within fixed boundary (Cluster level).
    ForceWithinBoundary,
    /// Deterministic orbital positions (System level).
    DeterministicOrbital,
    /// Relationship graph with hierarchical hints (Planet level).
    HierarchicalGraph,
    /// Tree / DAG layout (Core level).
    TreeDag,
    /// No canvas layout — structured panels only (Surface level).
    StructuredPanels,
}

// ── Tests ────────────────────────────────────────────────────

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_graph_scene_model_serde() {
        let scene = GraphSceneModel {
            generation: 1,
            level: ViewLevel::System,
            layout_strategy: LayoutStrategy::DeterministicOrbital,
            nodes: vec![SceneNode {
                id: "cbu".into(),
                label: "Allianz SICAV".into(),
                node_type: SceneNodeType::Cbu,
                state: Some("filled".into()),
                progress: 75,
                blocking: false,
                depth: 0,
                position_hint: None,
                badges: vec![],
                child_count: 5,
                group_id: None,
            }],
            edges: vec![],
            groups: vec![],
            drill_targets: vec![DrillTarget {
                node_id: "cbu".into(),
                target_level: ViewLevel::Planet,
                drill_label: "View entities".into(),
            }],
            max_depth: 3,
        };

        let json = serde_json::to_string(&scene).unwrap();
        assert!(json.contains("deterministic_orbital"));
        assert!(json.contains("Allianz SICAV"));

        let back: GraphSceneModel = serde_json::from_str(&json).unwrap();
        assert_eq!(back.generation, 1);
        assert_eq!(back.nodes.len(), 1);
        assert_eq!(back.drill_targets.len(), 1);
    }

    #[test]
    fn test_layout_strategy_default() {
        assert_eq!(LayoutStrategy::default(), LayoutStrategy::ForceDirected);
    }

    #[test]
    fn test_scene_node_type_serde() {
        let node_type = SceneNodeType::EntityGraph;
        let json = serde_json::to_string(&node_type).unwrap();
        assert_eq!(json, "\"entity_graph\"");

        let back: SceneNodeType = serde_json::from_str(&json).unwrap();
        assert_eq!(back, SceneNodeType::EntityGraph);
    }

    #[test]
    fn test_scene_edge_serde() {
        let edge = SceneEdge {
            source: "cbu".into(),
            target: "depositary".into(),
            edge_type: SceneEdgeType::Dependency,
            label: Some("depends on".into()),
            weight: 1.0,
        };

        let json = serde_json::to_string(&edge).unwrap();
        assert!(json.contains("dependency"));

        let back: SceneEdge = serde_json::from_str(&json).unwrap();
        assert_eq!(back.source, "cbu");
    }
}
