//! GraphSceneModel projection — transforms HydratedConstellation into render-ready scene.
//!
//! Pure projection: no DB calls, no new data sources.
//! HydratedConstellation → GraphSceneModel (every field derivable).

use ob_poc_types::galaxy::ViewLevel;
use ob_poc_types::graph_scene::*;

/// Project a GraphSceneModel from a HydratedConstellation at a given view level.
///
/// This function lives in sem_os_core (server-only) because it needs access to
/// the HydratedConstellation type. The output (GraphSceneModel) lives in
/// ob-poc-types so the WASM crate can consume it.
pub fn project_graph_scene(
    constellation: &str,
    jurisdiction: &str,
    cbu_id: &str,
    slots: &[SlotProjection],
    level: ViewLevel,
    generation: u64,
) -> GraphSceneModel {
    let layout_strategy = match level {
        ViewLevel::Universe => LayoutStrategy::ForceDirected,
        ViewLevel::Cluster => LayoutStrategy::ForceWithinBoundary,
        ViewLevel::System => LayoutStrategy::DeterministicOrbital,
        ViewLevel::Planet => LayoutStrategy::HierarchicalGraph,
        ViewLevel::Surface => LayoutStrategy::StructuredPanels,
        ViewLevel::Core => LayoutStrategy::TreeDag,
    };

    let mut nodes = Vec::new();
    let mut edges = Vec::new();
    let mut drill_targets = Vec::new();
    let mut max_depth: usize = 0;

    // Root CBU node
    nodes.push(SceneNode {
        id: cbu_id.to_string(),
        label: constellation.to_string(),
        node_type: SceneNodeType::Cbu,
        state: Some("filled".into()),
        progress: 0,
        blocking: false,
        depth: 0,
        position_hint: Some((0.0, 0.0)),
        badges: vec![SceneBadge {
            badge_type: "jurisdiction".into(),
            label: jurisdiction.to_string(),
            color: None,
        }],
        child_count: slots.len(),
        group_id: None,
    });

    // Project each slot as a node
    for slot in slots {
        let node_type = match slot.slot_type.as_str() {
            "cbu" => SceneNodeType::Cbu,
            "entity" => SceneNodeType::Entity,
            "entity_graph" => SceneNodeType::EntityGraph,
            "case" => SceneNodeType::Case,
            "tollgate" => SceneNodeType::Tollgate,
            "mandate" => SceneNodeType::Mandate,
            _ => SceneNodeType::Entity,
        };

        let depth = slot.depth;
        if depth > max_depth {
            max_depth = depth;
        }

        nodes.push(SceneNode {
            id: slot.path.clone(),
            label: slot.name.clone(),
            node_type,
            state: Some(slot.computed_state.clone()),
            progress: slot.progress,
            blocking: slot.blocking,
            depth,
            position_hint: None, // Layout engine computes positions
            badges: vec![],
            child_count: slot.child_count,
            group_id: None,
        });

        // Edge from parent to this slot
        let parent = if depth == 0 {
            cbu_id.to_string()
        } else {
            slot.parent_path
                .clone()
                .unwrap_or_else(|| cbu_id.to_string())
        };

        edges.push(SceneEdge {
            source: parent,
            target: slot.path.clone(),
            edge_type: if !slot.depends_on.is_empty() {
                SceneEdgeType::Dependency
            } else {
                SceneEdgeType::ParentChild
            },
            label: None,
            weight: 1.0,
        });

        // Drill target if has children or is an entity graph
        if slot.child_count > 0 || node_type == SceneNodeType::EntityGraph {
            let target_level = match level {
                ViewLevel::System => ViewLevel::Planet,
                ViewLevel::Planet => ViewLevel::Surface,
                _ => ViewLevel::Planet,
            };
            drill_targets.push(DrillTarget {
                node_id: slot.path.clone(),
                target_level,
                drill_label: format!("View {}", slot.name),
            });
        }

        // Dependency edges
        for dep in &slot.depends_on {
            edges.push(SceneEdge {
                source: dep.clone(),
                target: slot.path.clone(),
                edge_type: SceneEdgeType::Dependency,
                label: Some("depends on".into()),
                weight: 0.5,
            });
        }

        // Ownership/graph edges
        for graph_edge in &slot.graph_edges {
            edges.push(SceneEdge {
                source: graph_edge.from_id.clone(),
                target: graph_edge.to_id.clone(),
                edge_type: match graph_edge.edge_type.as_str() {
                    "ownership" => SceneEdgeType::Ownership,
                    "control" => SceneEdgeType::Control,
                    _ => SceneEdgeType::SharedEntity,
                },
                label: graph_edge.label.clone(),
                weight: graph_edge.weight,
            });
        }
    }

    GraphSceneModel {
        generation,
        level,
        layout_strategy,
        nodes,
        edges,
        groups: vec![],
        drill_targets,
        max_depth,
    }
}

/// Lightweight slot projection — extracted from HydratedSlot for the projection function.
/// Avoids importing the full constellation_runtime types into sem_os_core.
#[derive(Debug, Clone)]
pub struct SlotProjection {
    pub name: String,
    pub path: String,
    pub slot_type: String,
    pub computed_state: String,
    pub progress: u8,
    pub blocking: bool,
    pub depth: usize,
    pub parent_path: Option<String>,
    pub child_count: usize,
    pub depends_on: Vec<String>,
    pub graph_edges: Vec<GraphEdgeProjection>,
}

/// Lightweight graph edge projection.
#[derive(Debug, Clone)]
pub struct GraphEdgeProjection {
    pub from_id: String,
    pub to_id: String,
    pub edge_type: String,
    pub label: Option<String>,
    pub weight: f32,
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_project_empty_constellation() {
        let scene = project_graph_scene("test", "LU", "cbu-1", &[], ViewLevel::System, 1);
        assert_eq!(scene.nodes.len(), 1); // Just the CBU root
        assert_eq!(scene.level, ViewLevel::System);
        assert_eq!(scene.layout_strategy, LayoutStrategy::DeterministicOrbital);
    }

    #[test]
    fn test_project_with_slots() {
        let slots = vec![
            SlotProjection {
                name: "Depositary".into(),
                path: "depositary".into(),
                slot_type: "entity".into(),
                computed_state: "filled".into(),
                progress: 100,
                blocking: false,
                depth: 1,
                parent_path: None,
                child_count: 0,
                depends_on: vec![],
                graph_edges: vec![],
            },
            SlotProjection {
                name: "Investment Manager".into(),
                path: "investment_manager".into(),
                slot_type: "entity".into(),
                computed_state: "empty".into(),
                progress: 0,
                blocking: true,
                depth: 1,
                parent_path: None,
                child_count: 0,
                depends_on: vec!["depositary".into()],
                graph_edges: vec![],
            },
        ];

        let scene = project_graph_scene("Allianz SICAV", "LU", "cbu-1", &slots, ViewLevel::System, 2);
        assert_eq!(scene.nodes.len(), 3); // CBU + 2 slots
        assert_eq!(scene.generation, 2);
        // Dependency edge + 2 parent-child edges
        assert!(scene.edges.len() >= 3);
        // Investment Manager depends on Depositary
        let dep_edge = scene.edges.iter().find(|e| {
            e.source == "depositary"
                && e.target == "investment_manager"
                && matches!(e.edge_type, SceneEdgeType::Dependency)
        });
        assert!(dep_edge.is_some());
    }

    #[test]
    fn test_layout_strategy_by_level() {
        assert_eq!(
            project_graph_scene("t", "LU", "c", &[], ViewLevel::Universe, 1).layout_strategy,
            LayoutStrategy::ForceDirected
        );
        assert_eq!(
            project_graph_scene("t", "LU", "c", &[], ViewLevel::Core, 1).layout_strategy,
            LayoutStrategy::TreeDag
        );
    }
}
