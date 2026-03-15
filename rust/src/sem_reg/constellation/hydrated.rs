use serde::{Deserialize, Serialize};
use uuid::Uuid;

use crate::sem_reg::reducer::BlockedVerb;

use super::hydration::RawOverlayRow;
use super::map_def::{Cardinality, SlotType};

/// Hydrated constellation payload returned by the plugin verb.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct HydratedConstellation {
    pub constellation: String,
    pub description: Option<String>,
    pub jurisdiction: String,
    pub map_revision: String,
    pub cbu_id: Uuid,
    pub case_id: Option<Uuid>,
    pub slots: Vec<HydratedSlot>,
}

/// Hydrated ownership graph node used by entity-graph slots.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct HydratedGraphNode {
    pub entity_id: Uuid,
    pub name: Option<String>,
    pub entity_type: Option<String>,
}

/// Hydrated ownership graph edge used by entity-graph slots.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct HydratedGraphEdge {
    pub from_entity_id: Uuid,
    pub to_entity_id: Uuid,
    pub percentage: Option<f64>,
    pub ownership_type: Option<String>,
    pub depth: usize,
}

/// Hydrated slot in normalized tree form.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct HydratedSlot {
    pub name: String,
    pub path: String,
    pub slot_type: SlotType,
    pub cardinality: Cardinality,
    pub entity_id: Option<Uuid>,
    pub record_id: Option<Uuid>,
    pub computed_state: String,
    pub effective_state: String,
    pub progress: u8,
    pub blocking: bool,
    pub warnings: Vec<String>,
    pub overlays: Vec<RawOverlayRow>,
    pub graph_node_count: Option<usize>,
    pub graph_edge_count: Option<usize>,
    pub graph_nodes: Vec<HydratedGraphNode>,
    pub graph_edges: Vec<HydratedGraphEdge>,
    pub available_verbs: Vec<String>,
    pub blocked_verbs: Vec<BlockedVerb>,
    pub children: Vec<HydratedSlot>,
}
