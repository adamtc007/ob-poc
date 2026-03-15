use serde::{Deserialize, Serialize};

use super::hydrated::{HydratedConstellation, HydratedSlot};

/// High-level summary for a hydrated constellation.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ConstellationSummary {
    pub total_slots: usize,
    pub slots_filled: usize,
    pub slots_empty_mandatory: usize,
    pub slots_empty_optional: usize,
    pub slots_placeholder: usize,
    pub slots_complete: usize,
    pub slots_in_progress: usize,
    pub slots_blocked: usize,
    pub blocking_slots: usize,
    pub overall_progress: u8,
    pub completion_pct: u8,
    pub ownership_chain: Option<OwnershipSummary>,
}

/// Ownership chain summary extracted from hydrated graph slots.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct OwnershipSummary {
    pub total_entities: usize,
    pub total_edges: usize,
    pub chain_complete: bool,
}

/// Compute a compact summary for a hydrated constellation.
///
/// # Examples
/// ```rust
/// use ob_poc::sem_reg::constellation::{compute_summary, load_builtin_constellation_map, normalize_slots, RawHydrationData};
/// use uuid::Uuid;
///
/// let map = load_builtin_constellation_map("struct.lux.ucits.sicav").unwrap();
/// let hydrated = normalize_slots(&map, Uuid::nil(), None, RawHydrationData::default());
/// let summary = compute_summary(&hydrated);
/// assert!(summary.total_slots >= 1);
/// ```
pub fn compute_summary(hydrated: &HydratedConstellation) -> ConstellationSummary {
    let slots = flatten(&hydrated.slots);
    let total_slots = slots.len();
    let slots_filled = slots.iter().filter(|slot| slot.effective_state != "empty").count();
    let slots_empty_mandatory = slots
        .iter()
        .filter(|slot| {
            slot.cardinality == super::map_def::Cardinality::Mandatory
                && slot.effective_state == "empty"
        })
        .count();
    let slots_empty_optional = slots
        .iter()
        .filter(|slot| {
            slot.cardinality == super::map_def::Cardinality::Optional
                && slot.effective_state == "empty"
        })
        .count();
    let slots_placeholder = slots
        .iter()
        .filter(|slot| slot.effective_state == "placeholder")
        .count();
    let slots_complete = slots.iter().filter(|slot| slot.progress == 100).count();
    let slots_in_progress = slots
        .iter()
        .filter(|slot| slot.progress > 0 && slot.progress < 100)
        .count();
    let slots_blocked = slots.iter().filter(|slot| !slot.blocked_verbs.is_empty()).count();
    let blocking_slots = slots.iter().filter(|slot| slot.blocking).count();
    let overall_progress = if total_slots == 0 {
        0
    } else {
        (slots
            .iter()
            .map(|slot| slot.progress as usize)
            .sum::<usize>()
            / total_slots) as u8
    };
    let completion_pct = if total_slots == 0 {
        0
    } else {
        ((slots_complete * 100) / total_slots) as u8
    };
    let ownership_chain = slots
        .iter()
        .find(|slot| slot.slot_type == super::map_def::SlotType::EntityGraph)
        .map(|slot| OwnershipSummary {
            total_entities: slot.graph_node_count.unwrap_or(0),
            total_edges: slot.graph_edge_count.unwrap_or(0),
            chain_complete: slot.progress == 100 && !slot.blocking,
        });
    ConstellationSummary {
        total_slots,
        slots_filled,
        slots_empty_mandatory,
        slots_empty_optional,
        slots_placeholder,
        slots_complete,
        slots_in_progress,
        slots_blocked,
        blocking_slots,
        overall_progress,
        completion_pct,
        ownership_chain,
    }
}

fn flatten(slots: &[HydratedSlot]) -> Vec<&HydratedSlot> {
    let mut out = Vec::new();
    for slot in slots {
        out.push(slot);
        out.extend(flatten(&slot.children));
    }
    out
}
