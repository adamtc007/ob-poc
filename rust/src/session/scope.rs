//! Session Scope Management
//!
//! Handles scope definitions and windowing for large datasets.
//!
//! # Load Strategies
//!
//! - `Full`: All data loaded in memory (small scopes < 1000 entities)
//! - `SummaryOnly`: Only summary loaded, expand nodes on demand
//! - `Windowed`: Data loaded around a focal point with configurable depth

use serde::{Deserialize, Serialize};
use std::collections::HashMap;
use uuid::Uuid;

use crate::graph::{EntityGraph, GraphScope};

/// Session scope with stats and windowing info
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct SessionScope {
    /// How scope was defined
    pub definition: GraphScope,

    /// Summary statistics
    pub stats: ScopeSummary,

    /// Whether full data is loaded or windowed
    pub load_status: LoadStatus,
}

/// Summary statistics for a loaded scope
#[derive(Debug, Clone, Default, Serialize, Deserialize)]
pub struct ScopeSummary {
    /// Total number of entities in scope
    pub total_entities: usize,

    /// Total number of CBUs in scope
    pub total_cbus: usize,

    /// Total number of edges (ownership + control + fund structure)
    pub total_edges: usize,

    /// Entity count by jurisdiction code
    pub by_jurisdiction: HashMap<String, usize>,

    /// Entity count by entity type
    pub by_entity_type: HashMap<String, usize>,

    /// Number of terminus entities (natural persons, public companies)
    pub terminus_count: usize,

    /// Maximum depth in ownership chains
    pub max_depth: u32,
}

/// Load status indicating how much data is in memory
#[derive(Debug, Clone, Serialize, Deserialize)]
pub enum LoadStatus {
    /// All data loaded in memory
    Full,

    /// Only summary loaded, expand on demand
    SummaryOnly {
        /// Nodes that can be expanded to load children
        expandable_nodes: Vec<ExpandableNode>,
    },

    /// Windowed around a focal point
    Windowed {
        /// Entity at center of window
        center_entity_id: Uuid,
        /// How many hops are loaded
        loaded_hops: u32,
        /// Total entities reachable (but not loaded)
        total_reachable: usize,
    },
}

impl Default for LoadStatus {
    fn default() -> Self {
        Self::Full
    }
}

/// A node that can be expanded to load more data
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ExpandableNode {
    /// Entity ID that can be expanded
    pub entity_id: Uuid,

    /// Display name
    pub name: String,

    /// Number of children that would be loaded
    pub collapsed_child_count: usize,

    /// Hint about what's collapsed (e.g., "47 subfunds")
    pub child_type_hint: String,
}

impl SessionScope {
    /// Create an empty scope
    pub fn empty() -> Self {
        Self {
            definition: GraphScope::Empty,
            stats: ScopeSummary::default(),
            load_status: LoadStatus::Full,
        }
    }

    /// Create scope from a loaded graph
    pub fn from_graph(graph: &EntityGraph, definition: GraphScope) -> Self {
        let max_depth = graph
            .nodes
            .values()
            .filter_map(|n| n.depth_from_terminus)
            .max()
            .unwrap_or(0);

        Self {
            definition,
            stats: ScopeSummary {
                total_entities: graph.nodes.len(),
                total_cbus: graph.cbus.len(),
                total_edges: graph.ownership_edges.len()
                    + graph.control_edges.len()
                    + graph.fund_edges.len(),
                by_jurisdiction: Self::count_by_jurisdiction(graph),
                by_entity_type: Self::count_by_type(graph),
                terminus_count: graph.termini.len(),
                max_depth,
            },
            load_status: LoadStatus::Full,
        }
    }

    /// Create a windowed scope centered on an entity
    pub fn windowed(
        definition: GraphScope,
        center_entity_id: Uuid,
        loaded_hops: u32,
        total_reachable: usize,
        partial_stats: ScopeSummary,
    ) -> Self {
        Self {
            definition,
            stats: partial_stats,
            load_status: LoadStatus::Windowed {
                center_entity_id,
                loaded_hops,
                total_reachable,
            },
        }
    }

    /// Create a summary-only scope with expandable nodes
    pub fn summary_only(
        definition: GraphScope,
        stats: ScopeSummary,
        expandable_nodes: Vec<ExpandableNode>,
    ) -> Self {
        Self {
            definition,
            stats,
            load_status: LoadStatus::SummaryOnly { expandable_nodes },
        }
    }

    /// Count entities by jurisdiction
    fn count_by_jurisdiction(graph: &EntityGraph) -> HashMap<String, usize> {
        let mut counts = HashMap::new();
        for node in graph.nodes.values() {
            if let Some(j) = &node.jurisdiction {
                *counts.entry(j.clone()).or_insert(0) += 1;
            }
        }
        counts
    }

    /// Count entities by type
    fn count_by_type(graph: &EntityGraph) -> HashMap<String, usize> {
        let mut counts = HashMap::new();
        for node in graph.nodes.values() {
            let type_str = format!("{:?}", node.entity_type);
            *counts.entry(type_str).or_insert(0) += 1;
        }
        counts
    }

    /// Check if scope is empty
    pub fn is_empty(&self) -> bool {
        matches!(self.definition, GraphScope::Empty)
    }

    /// Check if scope is fully loaded
    pub fn is_fully_loaded(&self) -> bool {
        matches!(self.load_status, LoadStatus::Full)
    }

    /// Check if scope is windowed
    pub fn is_windowed(&self) -> bool {
        matches!(self.load_status, LoadStatus::Windowed { .. })
    }

    /// Get expandable nodes if in summary-only mode
    pub fn expandable_nodes(&self) -> Option<&[ExpandableNode]> {
        match &self.load_status {
            LoadStatus::SummaryOnly { expandable_nodes } => Some(expandable_nodes),
            _ => None,
        }
    }

    /// Get window center if in windowed mode
    pub fn window_center(&self) -> Option<Uuid> {
        match &self.load_status {
            LoadStatus::Windowed {
                center_entity_id, ..
            } => Some(*center_entity_id),
            _ => None,
        }
    }

    /// Get top jurisdictions by entity count
    pub fn top_jurisdictions(&self, limit: usize) -> Vec<(&str, usize)> {
        let mut sorted: Vec<_> = self
            .stats
            .by_jurisdiction
            .iter()
            .map(|(k, v)| (k.as_str(), *v))
            .collect();
        sorted.sort_by(|a, b| b.1.cmp(&a.1));
        sorted.truncate(limit);
        sorted
    }

    /// Get top entity types by count
    pub fn top_entity_types(&self, limit: usize) -> Vec<(&str, usize)> {
        let mut sorted: Vec<_> = self
            .stats
            .by_entity_type
            .iter()
            .map(|(k, v)| (k.as_str(), *v))
            .collect();
        sorted.sort_by(|a, b| b.1.cmp(&a.1));
        sorted.truncate(limit);
        sorted
    }

    /// Format scope for display
    pub fn display_summary(&self) -> String {
        let scope_type = match &self.definition {
            GraphScope::SingleCbu { cbu_name, .. } => format!("CBU: {}", cbu_name),
            GraphScope::Book { apex_name, .. } => format!("Book: {}", apex_name),
            GraphScope::Jurisdiction { code } => format!("Jurisdiction: {}", code),
            GraphScope::EntityNeighborhood { entity_id, hops } => {
                format!("Neighborhood: {} ({} hops)", entity_id, hops)
            }
            GraphScope::Empty => "Empty".to_string(),
            GraphScope::Custom { description } => format!("Custom: {}", description),
        };

        let load_hint = match &self.load_status {
            LoadStatus::Full => "fully loaded".to_string(),
            LoadStatus::SummaryOnly { expandable_nodes } => {
                format!("{} expandable nodes", expandable_nodes.len())
            }
            LoadStatus::Windowed {
                loaded_hops,
                total_reachable,
                ..
            } => format!("{} hops loaded, {} reachable", loaded_hops, total_reachable),
        };

        format!(
            "{} | {} entities, {} CBUs, {} termini | {}",
            scope_type,
            self.stats.total_entities,
            self.stats.total_cbus,
            self.stats.terminus_count,
            load_hint
        )
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_empty_scope() {
        let scope = SessionScope::empty();
        assert!(scope.is_empty());
        assert!(scope.is_fully_loaded());
        assert_eq!(scope.stats.total_entities, 0);
    }

    #[test]
    fn test_windowed_scope() {
        let center = Uuid::new_v4();
        let scope = SessionScope::windowed(
            GraphScope::SingleCbu {
                cbu_id: Uuid::new_v4(),
                cbu_name: "Test Fund".to_string(),
            },
            center,
            3,
            500,
            ScopeSummary {
                total_entities: 50,
                ..Default::default()
            },
        );

        assert!(scope.is_windowed());
        assert!(!scope.is_fully_loaded());
        assert_eq!(scope.window_center(), Some(center));
    }

    #[test]
    fn test_summary_only_scope() {
        let expandable = vec![ExpandableNode {
            entity_id: Uuid::new_v4(),
            name: "Allianz SE".to_string(),
            collapsed_child_count: 47,
            child_type_hint: "47 subfunds".to_string(),
        }];

        let scope = SessionScope::summary_only(
            GraphScope::Book {
                apex_entity_id: Uuid::new_v4(),
                apex_name: "Allianz".to_string(),
            },
            ScopeSummary::default(),
            expandable,
        );

        assert!(!scope.is_fully_loaded());
        assert!(scope.expandable_nodes().is_some());
        assert_eq!(scope.expandable_nodes().unwrap().len(), 1);
    }

    #[test]
    fn test_top_jurisdictions() {
        let mut stats = ScopeSummary::default();
        stats.by_jurisdiction.insert("LU".to_string(), 100);
        stats.by_jurisdiction.insert("IE".to_string(), 50);
        stats.by_jurisdiction.insert("DE".to_string(), 25);
        stats.by_jurisdiction.insert("US".to_string(), 10);

        let scope = SessionScope {
            definition: GraphScope::Empty,
            stats,
            load_status: LoadStatus::Full,
        };

        let top = scope.top_jurisdictions(2);
        assert_eq!(top.len(), 2);
        assert_eq!(top[0], ("LU", 100));
        assert_eq!(top[1], ("IE", 50));
    }

    #[test]
    fn test_display_summary() {
        let scope = SessionScope {
            definition: GraphScope::SingleCbu {
                cbu_id: Uuid::new_v4(),
                cbu_name: "Alpha Fund".to_string(),
            },
            stats: ScopeSummary {
                total_entities: 25,
                total_cbus: 1,
                terminus_count: 3,
                ..Default::default()
            },
            load_status: LoadStatus::Full,
        };

        let summary = scope.display_summary();
        assert!(summary.contains("Alpha Fund"));
        assert!(summary.contains("25 entities"));
        assert!(summary.contains("fully loaded"));
    }
}
