use std::collections::{BTreeMap, HashMap, HashSet};

use uuid::Uuid;

use super::hydrated::{
    HydratedConstellation, HydratedGraphEdge, HydratedGraphNode, HydratedSlot,
};
use super::hydration::{RawGraphEdge, RawHydrationData, RawOverlayRow, RawSlotRow};
use super::map_def::Cardinality;
use super::validate::ValidatedConstellationMap;

/// Normalize raw hydration data into a tree of hydrated slots.
///
/// # Examples
/// ```rust
/// use ob_poc::sem_reg::constellation::{load_builtin_constellation_map, normalize_slots, RawHydrationData};
/// use uuid::Uuid;
///
/// let map = load_builtin_constellation_map("struct.lux.ucits.sicav").unwrap();
/// let normalized = normalize_slots(
///     &map,
///     Uuid::nil(),
///     None,
///     RawHydrationData::default(),
/// );
/// assert_eq!(normalized.constellation, "struct.lux.ucits.sicav");
/// ```
pub fn normalize_slots(
    map: &ValidatedConstellationMap,
    cbu_id: Uuid,
    case_id: Option<Uuid>,
    raw: RawHydrationData,
) -> HydratedConstellation {
    let mut nodes = BTreeMap::<String, HydratedSlot>::new();
    for slot in &map.slots_ordered {
        let rows = raw.slot_rows.get(&slot.name).cloned().unwrap_or_default();
        let overlays = raw
            .overlay_rows
            .get(&slot.name)
            .cloned()
            .unwrap_or_default();
        let warnings = raw.warnings.get(&slot.name).cloned().unwrap_or_default();
        let reducer_result = raw.reducer_results.get(&slot.name);
        let hydrated = match slot.def.cardinality {
            Cardinality::Recursive => normalize_graph(
                slot,
                &rows,
                raw.graph_edges.get(&slot.name).cloned().unwrap_or_default(),
                &raw.entity_details,
                warnings,
                overlays,
            ),
            Cardinality::Root | Cardinality::Mandatory | Cardinality::Optional => {
                normalize_singular(slot, &rows, &overlays, warnings, reducer_result)
            }
        };
        nodes.insert(slot.name.clone(), hydrated);
    }

    let mut children = HashMap::<String, Vec<String>>::new();
    for slot in &map.slots_ordered {
        if let Some(parent) = &slot.parent {
            children
                .entry(parent.clone())
                .or_default()
                .push(slot.name.clone());
        }
    }

    let root = map
        .slots_ordered
        .iter()
        .find(|slot| slot.def.cardinality == Cardinality::Root)
        .map(|slot| slot.name.clone())
        .unwrap_or_default();

    for slot in &map.slots_ordered {
        if slot.name != root && slot.parent.is_none() {
            children
                .entry(root.clone())
                .or_default()
                .push(slot.name.clone());
        }
    }

    let root_slots = build_tree(&root, &children, &nodes);

    HydratedConstellation {
        constellation: map.constellation.clone(),
        description: map.description.clone(),
        jurisdiction: map.jurisdiction.clone(),
        map_revision: map.map_revision.clone(),
        cbu_id,
        case_id,
        slots: root_slots,
    }
}

fn build_tree(
    root: &str,
    children: &HashMap<String, Vec<String>>,
    nodes: &BTreeMap<String, HydratedSlot>,
) -> Vec<HydratedSlot> {
    let mut node = nodes.get(root).cloned().into_iter().collect::<Vec<_>>();
    if let Some(item) = node.first_mut() {
        item.children = children
            .get(root)
            .into_iter()
            .flat_map(|names| names.iter())
            .flat_map(|name| build_tree(name, children, nodes))
            .collect();
    }
    node
}

fn normalize_singular(
    slot: &super::validate::ResolvedSlot,
    rows: &[RawSlotRow],
    overlays: &[RawOverlayRow],
    mut warnings: Vec<String>,
    reducer_result: Option<&crate::sem_reg::reducer::SlotReduceResult>,
) -> HydratedSlot {
    let chosen = pick_deterministic(rows);
    if rows.len() > 1 && slot.def.cardinality != Cardinality::Recursive {
        warnings.push(format!(
            "slot '{}' had {} rows; picked a deterministic representative",
            slot.name,
            rows.len()
        ));
    }
    let placeholder = slot.def.placeholder.is_some()
        && overlays.iter().any(|overlay| {
            overlay
                .fields
                .get("name")
                .and_then(|value| value.as_str())
                .map(|name| name.to_lowercase().contains("placeholder"))
                .unwrap_or(false)
        });
    let (computed_state, effective_state, available_verbs, blocked_verbs, reducer_warnings) =
        if let Some(result) = reducer_result {
            (
                result.computed_state.clone(),
                result.effective_state.clone(),
                result.available_verbs.clone(),
                result.blocked_verbs.clone(),
                result.consistency_warnings.clone(),
            )
        } else if chosen.is_none() {
            (
                "empty".to_string(),
                "empty".to_string(),
                Vec::new(),
                Vec::new(),
                Vec::new(),
            )
        } else if placeholder {
            (
                "placeholder".to_string(),
                "placeholder".to_string(),
                Vec::new(),
                Vec::new(),
                Vec::new(),
            )
        } else {
            (
                "filled".to_string(),
                "filled".to_string(),
                Vec::new(),
                Vec::new(),
                Vec::new(),
            )
        };
    warnings.extend(reducer_warnings);
    HydratedSlot {
        name: slot.name.clone(),
        path: slot.path.clone(),
        slot_type: slot.def.slot_type,
        cardinality: slot.def.cardinality,
        entity_id: chosen.and_then(|row| row.entity_id),
        record_id: chosen.and_then(|row| row.record_id),
        computed_state,
        effective_state,
        progress: 0,
        blocking: false,
        warnings,
        overlays: overlays.to_vec(),
        graph_node_count: None,
        graph_edge_count: None,
        graph_nodes: Vec::new(),
        graph_edges: Vec::new(),
        available_verbs,
        blocked_verbs,
        children: Vec::new(),
    }
}

fn pick_deterministic(rows: &[RawSlotRow]) -> Option<&RawSlotRow> {
    rows.iter().max_by(|lhs, rhs| {
        lhs.created_at
            .cmp(&rhs.created_at)
            .then(lhs.filter_value.cmp(&rhs.filter_value))
            .then(lhs.entity_id.cmp(&rhs.entity_id))
    })
}

fn normalize_graph(
    slot: &super::validate::ResolvedSlot,
    rows: &[RawSlotRow],
    edges: Vec<RawGraphEdge>,
    entity_details: &HashMap<Uuid, serde_json::Value>,
    warnings: Vec<String>,
    overlays: Vec<RawOverlayRow>,
) -> HydratedSlot {
    let mut sorted_nodes = rows
        .iter()
        .filter_map(|row| row.entity_id)
        .collect::<HashSet<_>>()
        .into_iter()
        .collect::<Vec<_>>();
    sorted_nodes.sort();
    let mut sorted_edges = edges
        .iter()
        .map(|edge| (edge.from_entity_id, edge.to_entity_id, edge.depth))
        .collect::<HashSet<_>>()
        .into_iter()
        .collect::<Vec<_>>();
    sorted_edges.sort();
    let unique_nodes = sorted_nodes.len();
    let unique_edges = sorted_edges.len();
    let graph_nodes = sorted_nodes
        .iter()
        .map(|entity_id| {
            let detail = entity_details.get(entity_id);
            HydratedGraphNode {
                entity_id: *entity_id,
                name: detail
                    .and_then(|value| value.get("name"))
                    .and_then(|value| value.as_str())
                    .map(ToOwned::to_owned),
                entity_type: detail
                    .and_then(|value| value.get("entity_type"))
                    .and_then(|value| value.as_str())
                    .map(ToOwned::to_owned),
            }
        })
        .collect::<Vec<_>>();
    let mut graph_edges = edges;
    graph_edges.sort_by(|lhs, rhs| {
        lhs.depth
            .cmp(&rhs.depth)
            .then(lhs.from_entity_id.cmp(&rhs.from_entity_id))
            .then(lhs.to_entity_id.cmp(&rhs.to_entity_id))
            .then(lhs.ownership_type.cmp(&rhs.ownership_type))
    });
    let mut normalized_warnings = warnings;
    if let Some(max_depth) = slot.def.max_depth.filter(|_| {
        graph_edges.iter()
            .map(|edge| edge.depth)
            .max()
            .map(|depth| depth >= slot.def.max_depth.unwrap_or(0))
            .unwrap_or(false)
    }) {
        normalized_warnings.push(format!(
            "graph reached configured max depth {}",
            max_depth
        ));
    }
    HydratedSlot {
        name: slot.name.clone(),
        path: slot.path.clone(),
        slot_type: slot.def.slot_type,
        cardinality: slot.def.cardinality,
        entity_id: None,
        record_id: None,
        computed_state: if unique_nodes == 0 { "empty" } else { "filled" }.to_string(),
        effective_state: if unique_nodes == 0 { "empty" } else { "filled" }.to_string(),
        progress: 0,
        blocking: false,
        warnings: normalized_warnings
            .into_iter()
            .chain(std::iter::once(format!(
                "graph normalized to {} nodes and {} edges",
                unique_nodes, unique_edges
            )))
            .collect(),
        overlays,
        graph_node_count: Some(unique_nodes),
        graph_edge_count: Some(unique_edges),
        graph_nodes,
        graph_edges: graph_edges
            .into_iter()
            .map(|edge| HydratedGraphEdge {
                from_entity_id: edge.from_entity_id,
                to_entity_id: edge.to_entity_id,
                percentage: edge.percentage,
                ownership_type: edge.ownership_type,
                depth: edge.depth,
            })
            .collect(),
        available_verbs: Vec::new(),
        blocked_verbs: Vec::new(),
        children: Vec::new(),
    }
}
