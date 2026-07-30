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

use crate::graph::types::GraphScope;

/// Session scope with stats and windowing info
#[derive(Debug, Clone, Serialize, Deserialize)]
pub(crate) struct SessionScope {
    /// How scope was defined
    pub definition: GraphScope,

    /// Summary statistics
    pub stats: ScopeSummary,

    /// Whether full data is loaded or windowed
    pub load_status: LoadStatus,
}

/// Summary statistics for a loaded scope
#[derive(Debug, Clone, Default, Serialize, Deserialize)]
pub(crate) struct ScopeSummary {
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
#[derive(Debug, Clone, Serialize, Deserialize, Default)]
pub(crate) enum LoadStatus {
    /// All data loaded in memory
    #[default]
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

/// A node that can be expanded to load more data
#[derive(Debug, Clone, Serialize, Deserialize)]
pub(crate) struct ExpandableNode {
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
    /// Create scope from a GraphScope definition (without graph data)
    ///
    /// This is used when session.* verbs change the scope definition but
    /// graph data hasn't been loaded yet. Stats will be populated later
    /// when the graph is loaded.
    pub(crate) fn from_graph_scope(definition: GraphScope) -> Self {
        Self {
            definition,
            stats: ScopeSummary::default(),
            load_status: LoadStatus::SummaryOnly {
                expandable_nodes: vec![],
            },
        }
    }

    /// Check if scope is fully loaded
    pub(crate) fn is_fully_loaded(&self) -> bool {
        matches!(self.load_status, LoadStatus::Full)
    }
}
