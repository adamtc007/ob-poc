use std::collections::HashMap;

use crate::state_reducer::BlockedVerb;

use super::hydrated::{HydratedConstellation, HydratedSlot};
use super::map_def::{Cardinality, DependencyEntry, VerbPaletteEntry};
use super::validate::ValidatedConstellationMap;

/// Compute action-surface verbs and progress for a hydrated constellation.
///
/// # Examples
/// ```rust
/// use ob_poc::constellation::{compute_action_surface, load_builtin_constellation_map, normalize_slots, RawHydrationData};
/// use uuid::Uuid;
///
/// let map = load_builtin_constellation_map("struct.lux.ucits.sicav").unwrap();
/// let hydrated = normalize_slots(&map, Uuid::nil(), None, RawHydrationData::default());
/// let surfaced = compute_action_surface(&map, hydrated);
/// assert_eq!(surfaced.constellation, "struct.lux.ucits.sicav");
/// ```
pub fn compute_action_surface(
    map: &ValidatedConstellationMap,
    mut hydrated: HydratedConstellation,
) -> HydratedConstellation {
    let states = flatten_states(&hydrated.slots);
    for slot in &mut hydrated.slots {
        decorate_slot(map, &states, slot);
    }
    hydrated
}

fn decorate_slot(
    map: &ValidatedConstellationMap,
    states: &HashMap<String, String>,
    slot: &mut HydratedSlot,
) {
    if let Some(resolved) = map.slot_index.get(&slot.name) {
        slot.progress = compute_progress(slot.cardinality, &slot.effective_state);
        slot.blocking = slot.cardinality == Cardinality::Mandatory && slot.progress < 100;
        let mut seen_available = slot
            .available_verbs
            .iter()
            .cloned()
            .collect::<std::collections::HashSet<_>>();
        for entry in resolved.def.verbs.values() {
            match entry {
                VerbPaletteEntry::Simple(verb) => {
                    let unmet_dependencies = resolved
                        .def
                        .depends_on
                        .iter()
                        .filter_map(|dep| dependency_block_reason(dep, states, map))
                        .collect::<Vec<_>>();
                    if unmet_dependencies.is_empty() {
                        if seen_available.insert(verb.clone()) {
                            slot.available_verbs.push(verb.clone());
                        }
                    } else {
                        slot.blocked_verbs.push(BlockedVerb {
                            verb: verb.clone(),
                            reasons: unmet_dependencies,
                        });
                    }
                }
                VerbPaletteEntry::Gated { verb, when } => {
                    let allowed_states = when.to_vec();
                    let state_allowed = allowed_states
                        .iter()
                        .any(|state| state_at_least(map, &slot.name, &slot.effective_state, state));
                    let unmet_dependencies = resolved
                        .def
                        .depends_on
                        .iter()
                        .filter_map(|dep| dependency_block_reason(dep, states, map))
                        .collect::<Vec<_>>();
                    if state_allowed && unmet_dependencies.is_empty() {
                        slot.available_verbs.push(verb.clone());
                        seen_available.insert(verb.clone());
                    } else {
                        let mut reasons = unmet_dependencies;
                        if !state_allowed {
                            reasons.push(crate::state_reducer::BlockReason {
                                message: format!(
                                    "slot '{}' is in state '{}' but '{}' requires one of [{}]",
                                    slot.name,
                                    slot.effective_state,
                                    verb,
                                    allowed_states.join(", ")
                                ),
                            });
                        }
                        slot.blocked_verbs.push(BlockedVerb {
                            verb: verb.clone(),
                            reasons,
                        });
                    }
                }
            }
        }
    }
    for child in &mut slot.children {
        decorate_slot(map, states, child);
    }
}

fn flatten_states(slots: &[HydratedSlot]) -> HashMap<String, String> {
    let mut out = HashMap::new();
    for slot in slots {
        out.insert(slot.name.clone(), slot.effective_state.clone());
        out.extend(flatten_states(&slot.children));
    }
    out
}

fn dependency_block_reason(
    dep: &DependencyEntry,
    states: &HashMap<String, String>,
    map: &ValidatedConstellationMap,
) -> Option<crate::state_reducer::BlockReason> {
    match states.get(dep.slot_name()) {
        Some(state) if state_at_least(map, dep.slot_name(), state, dep.min_state()) => None,
        Some(state) => Some(crate::state_reducer::BlockReason {
            message: format!(
                "dependency '{}' is in state '{}' but requires '{}'",
                dep.slot_name(),
                state,
                dep.min_state()
            ),
        }),
        None => Some(crate::state_reducer::BlockReason {
            message: format!(
                "dependency '{}' is missing; requires '{}'",
                dep.slot_name(),
                dep.min_state()
            ),
        }),
    }
}

fn state_rank_fallback(state: &str) -> usize {
    match state {
        "empty" => 0,
        "placeholder" => 1,
        "filled" => 2,
        "workstream_open" => 3,
        "screening_complete" | "evidence_collected" => 4,
        "verified" | "proved" => 5,
        "approved" => 6,
        "alleged" => 2,
        "provable" => 3,
        _ => 0,
    }
}

fn state_at_least(
    map: &ValidatedConstellationMap,
    slot_name: &str,
    current: &str,
    min_state: &str,
) -> bool {
    if let Some(machine_states) = map
        .slot_index
        .get(slot_name)
        .and_then(|slot| slot.def.state_machine.as_ref())
        .and_then(|name| map.state_machines.get(name))
        .map(|machine| &machine.states)
    {
        let current_rank = machine_states.iter().position(|state| state == current);
        let min_rank = machine_states.iter().position(|state| state == min_state);
        if let (Some(current_rank), Some(min_rank)) = (current_rank, min_rank) {
            return current_rank >= min_rank;
        }
        tracing::warn!(
            slot = %slot_name,
            current_state = %current,
            min_state = %min_state,
            "falling back to generic state ranking for constellation action surface"
        );
    }
    state_at_least_fallback(current, min_state)
}

fn state_at_least_fallback(current: &str, min_state: &str) -> bool {
    state_rank_fallback(current) >= state_rank_fallback(min_state)
}

fn compute_progress(cardinality: Cardinality, state: &str) -> u8 {
    if cardinality == Cardinality::Optional && state == "empty" {
        return 100;
    }
    match state {
        "empty" => 0,
        "placeholder" => 25,
        "filled" => 50,
        "workstream_open" | "alleged" => 65,
        "screening_complete" | "evidence_collected" | "provable" => 80,
        "verified" | "proved" => 95,
        "approved" => 100,
        _ => 0,
    }
}
