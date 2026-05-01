//! Valid verb set computation for SemOS-scoped verb resolution.
//!
//! Given a session context (client group, constellation template, entity states),
//! computes the set of verbs that are LEGAL at this moment. This is entirely
//! deterministic — no NLP, no embeddings, no LLM calls.

use std::{
    collections::{HashMap, HashSet},
    path::PathBuf,
    sync::OnceLock,
};

use anyhow::Result;
use chrono::{DateTime, Utc};
use dsl_core::resolver::{resolve_template, ResolverInputs};
use sem_os_core::{
    constellation_map_def::{
        Cardinality, ConstellationMapDefBody, DependencyEntry as CoreDependencyEntry,
        JoinDef as CoreJoinDef, SlotDef as CoreSlotDef, SlotType as CoreSlotType,
        VerbAvailability as CoreVerbAvailability, VerbPaletteEntry as CoreVerbPaletteEntry,
    },
    grounding::{compute_slot_action_surface, ConstellationModel},
    state_machine_def::{ReducerDef, StateMachineDefBody, TransitionDef},
};
use uuid::Uuid;

use crate::sem_os_runtime::constellation_runtime::{
    ConstellationMapDef, DependencyEntry as RuntimeDependencyEntry, SlotDef as RuntimeSlotDef,
    SlotType as RuntimeSlotType, VerbAvailability as RuntimeVerbAvailability,
    VerbPaletteEntry as RuntimeVerbPaletteEntry,
};

use super::session_context::EntityState;

// ---------------------------------------------------------------------------
// Types
// ---------------------------------------------------------------------------

/// A verb that is legal in the current session context.
#[derive(Debug, Clone)]
pub struct VerbCandidate {
    pub verb_fqn: String,
    pub entity_id: Option<Uuid>,
    pub entity_type: String,
    pub source: VerbSource,
    pub priority: u32,
    pub keywords: Vec<String>,
}

/// How this verb became part of the valid set.
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum VerbSource {
    /// Outgoing FSM transition from current entity state.
    FsmTransition,
    /// Creation verb for an entity that doesn't exist yet.
    CreationVerb,
    /// Always available (observation verbs: read, list, show).
    AlwaysAvailable,
}

/// The computed set of valid verbs for a session context.
#[derive(Debug, Clone)]
pub struct ValidVerbSet {
    pub verbs: Vec<VerbCandidate>,
    pub client_group_id: Uuid,
    pub constellation_id: String,
    pub computed_at: DateTime<Utc>,
}

impl ValidVerbSet {
    /// Get all verb FQNs in the set.
    pub fn verb_fqns(&self) -> Vec<&str> {
        self.verbs.iter().map(|v| v.verb_fqn.as_str()).collect()
    }

    pub fn is_empty(&self) -> bool {
        self.verbs.is_empty()
    }

    pub fn len(&self) -> usize {
        self.verbs.len()
    }

    /// Check if a specific verb FQN is in the valid set.
    pub fn contains_verb(&self, fqn: &str) -> bool {
        self.verbs.iter().any(|v| v.verb_fqn == fqn)
    }

    /// Convert to a HashSet for passing to the constrained embedding search.
    pub fn to_allowed_set(&self) -> HashSet<String> {
        self.verbs.iter().map(|v| v.verb_fqn.clone()).collect()
    }
}

static VERB_KEYWORDS: OnceLock<HashMap<String, Vec<String>>> = OnceLock::new();

// ---------------------------------------------------------------------------
// State machine loading
// ---------------------------------------------------------------------------

/// Raw state machine seed loaded from YAML before SemOS-core normalization.
#[derive(Debug, Clone, serde::Deserialize)]
struct StateMachineSeed {
    state_machine: String,
    #[allow(dead_code)]
    description: Option<String>,
    states: Vec<String>,
    initial: String,
    #[serde(default)]
    transitions: Vec<TransitionDef>,
    #[serde(default)]
    reducer: Option<ReducerDef>,
}

fn constellation_maps_dir() -> PathBuf {
    PathBuf::from(env!("CARGO_MANIFEST_DIR")).join("config/sem_os_seeds/constellation_maps")
}

/// Load a raw constellation definition by constellation ID.
///
/// # Examples
/// ```rust
/// use ob_poc::sage::valid_verb_set::load_constellation_by_id;
///
/// let map = load_constellation_by_id("group.ownership").unwrap();
/// assert_eq!(map.constellation, "group.ownership");
/// ```
pub fn load_constellation_by_id(id: &str) -> Result<ConstellationMapDefBody> {
    for entry in std::fs::read_dir(constellation_maps_dir())? {
        let path = entry?.path();
        let is_yaml = path
            .extension()
            .and_then(|ext| ext.to_str())
            .is_some_and(|ext| matches!(ext, "yaml" | "yml"));
        if !is_yaml {
            continue;
        }

        let content = std::fs::read_to_string(&path)?;
        let map: ConstellationMapDef = serde_yaml::from_str(&content)?;
        if map.constellation == id {
            return Ok(to_core_constellation_map(map));
        }
    }

    anyhow::bail!("Constellation not found: {}", id)
}

/// Load the composed constellation stack for a session-facing constellation ID.
///
/// # Examples
/// ```rust
/// use ob_poc::sage::valid_verb_set::load_constellation_stack;
///
/// let stack = load_constellation_stack("struct.lux.ucits.sicav").unwrap();
/// assert!(stack.iter().any(|map| map.constellation == "group.ownership"));
/// assert!(stack.iter().any(|map| map.constellation == "kyc.onboarding"));
/// ```
#[deprecated(note = "transitional; see D-008")]
pub fn load_constellation_stack(id: &str) -> Result<Vec<ConstellationMapDefBody>> {
    let inputs = ResolverInputs::default_from_cargo_manifest()?;
    let template = resolve_template(id.to_string(), "cbu".to_string(), &inputs)?;
    Ok(template.generated_from.legacy_constellation_stack)
}

// ---------------------------------------------------------------------------
// Core computation
// ---------------------------------------------------------------------------

/// Compute the set of verbs that are LEGAL right now, given the session context.
///
/// This is entirely deterministic — no NLP, no embeddings, no LLM calls.
///
/// Steps:
/// 1. For each existing entity, find its constellation slot and compute legal FSM transitions
/// 2. For constellation slots with no entity, add creation verbs if dependencies are met
/// 3. Add observation verbs (read, list) for all existing entities
/// 4. Sort by priority, dedup
pub fn compute_valid_verb_set(
    entity_states: &[EntityState],
    constellation: &ConstellationMapDefBody,
    client_group_id: Uuid,
) -> ValidVerbSet {
    compute_valid_verb_set_for_constellations(
        entity_states,
        std::slice::from_ref(constellation),
        client_group_id,
    )
}

/// Compute the union of legal verbs across a composed constellation stack.
///
/// # Examples
/// ```rust
/// use uuid::Uuid;
/// use ob_poc::sage::session_context::EntityState;
/// use ob_poc::sage::valid_verb_set::{compute_valid_verb_set_for_constellations, load_constellation_stack};
///
/// let stack = load_constellation_stack("group.ownership").unwrap();
/// let valid = compute_valid_verb_set_for_constellations(&Vec::<EntityState>::new(), &stack, Uuid::nil());
/// assert_eq!(valid.client_group_id, Uuid::nil());
/// ```
pub fn compute_valid_verb_set_for_constellations(
    entity_states: &[EntityState],
    constellations: &[ConstellationMapDefBody],
    client_group_id: Uuid,
) -> ValidVerbSet {
    let mut verbs = Vec::new();

    for constellation in constellations {
        verbs.extend(compute_valid_verb_set_for_single_constellation(
            entity_states,
            constellation,
        ));
    }

    // Sort by priority (lower = higher priority), dedup by verb_fqn
    verbs.sort_by(|a, b| {
        a.verb_fqn
            .cmp(&b.verb_fqn)
            .then_with(|| a.priority.cmp(&b.priority))
    });
    verbs.dedup_by(|a, b| a.verb_fqn == b.verb_fqn);
    verbs.sort_by(|a, b| {
        a.priority
            .cmp(&b.priority)
            .then_with(|| a.verb_fqn.cmp(&b.verb_fqn))
    });

    let constellation_id = constellations
        .iter()
        .map(|constellation| constellation.constellation.as_str())
        .collect::<Vec<_>>()
        .join(" + ");

    ValidVerbSet {
        verbs,
        client_group_id,
        constellation_id,
        computed_at: Utc::now(),
    }
}

fn compute_valid_verb_set_for_single_constellation(
    entity_states: &[EntityState],
    constellation: &ConstellationMapDefBody,
) -> Vec<VerbCandidate> {
    let model = ConstellationModel::from_parts(
        constellation.clone(),
        load_state_machine_bodies().unwrap_or_default(),
    );
    let slot_states = build_slot_states(entity_states, &model);

    let mut verbs = Vec::new();

    for slot_name in model.slots.keys() {
        let surface =
            compute_slot_action_surface(&model, &slot_states, slot_name).unwrap_or_default();
        for action in surface.valid_actions {
            verbs.push(VerbCandidate {
                verb_fqn: action.action_id.clone(),
                entity_id: entity_id_for_slot(slot_name, entity_states, &model),
                entity_type: entity_type_for_slot(slot_name, &model),
                source: VerbSource::FsmTransition,
                priority: 10,
                keywords: extract_keywords_for_verb(&action.action_id),
            });
        }
    }

    for (slot_name, slot) in &model.slots {
        if !slot_states.contains_key(slot_name)
            && slot_dependencies_met(&model, &slot_states, slot_name)
        {
            let entity_type = slot
                .def
                .entity_kinds
                .first()
                .cloned()
                .unwrap_or_else(|| entity_type_for_slot(slot_name, &model));
            for create_verb in empty_slot_verbs(&slot.def) {
                verbs.push(VerbCandidate {
                    verb_fqn: create_verb.clone(),
                    entity_id: None,
                    entity_type: entity_type.clone(),
                    source: VerbSource::CreationVerb,
                    priority: 5,
                    keywords: extract_keywords_for_verb(&create_verb),
                });
            }
        }
    }
    verbs
}

// ---------------------------------------------------------------------------
// Helper functions
// ---------------------------------------------------------------------------

/// Find the constellation slot matching an entity type.
fn load_state_machine_bodies() -> Result<HashMap<String, StateMachineDefBody>> {
    let mut machines = HashMap::new();
    let dir = PathBuf::from(env!("CARGO_MANIFEST_DIR")).join("config/sem_os_seeds/state_machines");
    for entry in std::fs::read_dir(dir)? {
        let path = entry?.path();
        let is_yaml = path
            .extension()
            .and_then(|ext| ext.to_str())
            .is_some_and(|ext| matches!(ext, "yaml" | "yml"));
        if !is_yaml {
            continue;
        }

        let content = std::fs::read_to_string(&path)?;
        let seed: StateMachineSeed = serde_yaml::from_str(&content)?;
        let state_machine = StateMachineDefBody {
            fqn: seed.state_machine.clone(),
            state_machine: seed.state_machine.clone(),
            description: seed.description,
            states: seed.states,
            initial: seed.initial,
            transitions: seed.transitions,
            reducer: seed.reducer,
        };
        machines.insert(state_machine.state_machine.clone(), state_machine);
    }
    Ok(machines)
}

fn build_slot_states(
    entity_states: &[EntityState],
    model: &ConstellationModel,
) -> HashMap<String, String> {
    let mut slot_states = HashMap::new();
    for entity_state in entity_states {
        if let Some(slot_name) = infer_slot_name(entity_state, model) {
            let normalized_state = model
                .slots
                .get(&slot_name)
                .map(|slot| normalize_slot_state(&entity_state.current_state, slot))
                .unwrap_or_else(|| entity_state.current_state.to_lowercase());
            slot_states.insert(slot_name, normalized_state);
        }
    }
    slot_states
}

fn infer_slot_name(entity_state: &EntityState, model: &ConstellationModel) -> Option<String> {
    if let Some(slot_name) = entity_state.slot_name.as_deref() {
        if model.slots.contains_key(slot_name) {
            return Some(slot_name.to_string());
        }
        if let Some((name, _)) = model
            .slots
            .iter()
            .find(|(name, _)| name.rsplit('.').next() == Some(slot_name))
        {
            return Some(name.clone());
        }
    }

    model
        .slots
        .iter()
        .find(|(name, slot)| {
            slot.def
                .entity_kinds
                .iter()
                .any(|kind| kind == &entity_state.entity_type)
                || name.rsplit('.').next() == Some(entity_state.entity_type.as_str())
        })
        .map(|(name, _)| name.clone())
}

fn entity_id_for_slot(
    slot_name: &str,
    entity_states: &[EntityState],
    model: &ConstellationModel,
) -> Option<Uuid> {
    entity_states
        .iter()
        .find(|entity_state| infer_slot_name(entity_state, model).as_deref() == Some(slot_name))
        .map(|entity_state| entity_state.entity_id)
}

fn entity_type_for_slot(slot_name: &str, model: &ConstellationModel) -> String {
    model
        .slots
        .get(slot_name)
        .and_then(|slot| slot.def.entity_kinds.first().cloned())
        .unwrap_or_else(|| {
            slot_name
                .rsplit('.')
                .next()
                .unwrap_or(slot_name)
                .to_string()
        })
}

fn normalize_slot_state(
    current_state: &str,
    slot: &sem_os_core::grounding::ResolvedSlot,
) -> String {
    let normalized = current_state.to_lowercase();
    if slot.def.state_machine.is_none() {
        if normalized == "placeholder" {
            return normalized;
        }
        return "filled".to_string();
    }
    normalized
}

fn slot_dependencies_met(
    model: &ConstellationModel,
    slot_states: &HashMap<String, String>,
    slot_name: &str,
) -> bool {
    model
        .slots
        .get(slot_name)
        .map(|slot| {
            slot.def.depends_on.iter().all(|dependency| {
                slot_states
                    .get(dependency.slot_name())
                    .is_some_and(|state| {
                        state_at_least(model, dependency.slot_name(), state, dependency.min_state())
                    })
            })
        })
        .unwrap_or(false)
}

fn state_at_least(
    model: &ConstellationModel,
    slot_name: &str,
    current: &str,
    min_state: &str,
) -> bool {
    if let Some(order) = model
        .slots
        .get(slot_name)
        .and_then(|slot| slot.def.state_machine.as_ref())
        .and_then(|name| model.state_machines.get(name))
        .map(|machine| &machine.states)
    {
        let current_rank = order.iter().position(|state| state == current);
        let min_rank = order.iter().position(|state| state == min_state);
        if let (Some(current_rank), Some(min_rank)) = (current_rank, min_rank) {
            return current_rank >= min_rank;
        }
    }

    state_rank_fallback(current) >= state_rank_fallback(min_state)
}

fn state_rank_fallback(state: &str) -> usize {
    match state {
        "empty" => 0,
        "placeholder" => 1,
        "filled" | "intake" => 2,
        "prospect" => 2,
        "researching" => 3,
        "workstream_open" | "discovery" | "alleged" => 3,
        "ubo_mapped" => 4,
        "control_mapped" => 5,
        "cbus_identified" => 5,
        "onboarding" => 6,
        "screening_complete" | "evidence_collected" | "assessment" | "provable" => 4,
        "review" | "verified" | "proved" => 5,
        "active" | "approved" => 7,
        _ => 0,
    }
}

const KEYWORD_STOP_WORDS: &[&str] = &[
    "the", "a", "an", "for", "this", "that", "my", "me", "it", "is", "to", "of", "in", "and", "or",
    "with", "from", "on", "at", "by", "i", "we", "you", "can", "please", "want", "need", "would",
    "like", "just", "do", "does", "did", "be", "been", "being", "have", "has", "had", "will",
    "shall", "should", "could", "may", "might", "all",
];

fn normalize_token(token: &str) -> String {
    let token = token.to_lowercase();
    if token.ends_with("ments") && token.len() > 6 {
        return token[..token.len() - 1].to_string();
    }
    if token.ends_with("ions") && token.len() > 5 {
        return token[..token.len() - 1].to_string();
    }
    if token.ends_with("ing") && token.len() > 5 {
        return token[..token.len() - 3].to_string();
    }
    if token.ends_with("ies") && token.len() > 4 {
        return format!("{}y", &token[..token.len() - 3]);
    }
    if token.ends_with("ses")
        && token.len() > 5
        && !matches!(token.as_str(), "cases" | "bases" | "phases")
    {
        return token[..token.len() - 2].to_string();
    }
    if token.ends_with("es") && token.len() > 4 {
        return token[..token.len() - 2].to_string();
    }
    if token.ends_with('s')
        && !token.ends_with("ss")
        && token.len() > 4
        && !KEYWORD_STOP_WORDS.contains(&token.as_str())
    {
        return token[..token.len() - 1].to_string();
    }
    token
}

/// Find the creation verb in a slot's verb palette.
fn find_creation_verb(slot: &CoreSlotDef) -> Option<String> {
    // Look for verbs with "create" or "open" in the key
    for (key, entry) in &slot.verbs {
        if key.contains("create") || key.contains("open") || key.contains("add") {
            return Some(entry.verb_fqn().to_string());
        }
    }
    // Fallback: first verb in palette
    slot.verbs.values().next().map(|e| e.verb_fqn().to_string())
}

fn empty_slot_verbs(slot: &CoreSlotDef) -> Vec<String> {
    let mut verbs = Vec::new();
    for (key, entry) in &slot.verbs {
        let include = match entry {
            CoreVerbPaletteEntry::Simple(_) => {
                key.contains("create")
                    || key.contains("open")
                    || key.contains("add")
                    || key.contains("ensure")
                    || key.contains("import")
                    || key.contains("discover")
                    || key.contains("search")
                    || key.contains("lookup")
            }
            CoreVerbPaletteEntry::Gated { when, .. } => when
                .to_vec()
                .iter()
                .any(|state| matches!(state.as_str(), "empty" | "placeholder")),
        };
        if include {
            verbs.push(entry.verb_fqn().to_string());
        }
    }

    if verbs.is_empty() {
        if let Some(create_verb) = find_creation_verb(slot) {
            verbs.push(create_verb);
        }
    }

    verbs.sort();
    verbs.dedup();
    verbs
}

/// Check if a verb is an observation (read-only) verb.
#[cfg_attr(not(test), allow(dead_code))]
fn is_observation_verb(verb_fqn: &str) -> bool {
    let action = verb_fqn.rsplit('.').next().unwrap_or("");
    matches!(
        action,
        "read"
            | "list"
            | "show"
            | "inspect"
            | "get"
            | "search"
            | "find"
            | "for-entity"
            | "missing-for-entity"
            | "compute-requirements"
            | "state"
            | "summary"
            | "list-by-cbu"
            | "list-by-case"
            | "list-by-type"
            | "list-by-severity"
            | "list-owners"
            | "list-ubos"
            | "get-metrics"
            | "get-decision-readiness"
            | "list-evaluations"
            | "list-thresholds"
            | "list-overrides"
            | "unsatisfied"
    )
}

/// Extract keywords from a verb FQN for keyword matching.
///
/// Splits on dots and hyphens. e.g., "document.solicit" → ["document", "solicit"]
/// Public accessor for tests.
pub fn extract_keywords_for_verb_pub(verb_fqn: &str) -> Vec<String> {
    extract_keywords_for_verb(verb_fqn)
}

fn extract_keywords_for_verb(verb_fqn: &str) -> Vec<String> {
    let index = VERB_KEYWORDS.get_or_init(load_verb_keyword_index);
    if let Some(keywords) = index.get(verb_fqn) {
        return keywords.clone();
    }

    fqn_keywords(verb_fqn)
}

fn fqn_keywords(verb_fqn: &str) -> Vec<String> {
    verb_fqn
        .split(['.', '-'])
        .filter(|s| !s.is_empty() && s.len() > 2)
        .map(normalize_token)
        .collect()
}

fn load_verb_keyword_index() -> HashMap<String, Vec<String>> {
    let mut index = HashMap::new();
    let verbs_dir = PathBuf::from(env!("CARGO_MANIFEST_DIR")).join("config/verbs");
    walk_verb_dir(&verbs_dir, &mut index);
    index
}

fn walk_verb_dir(dir: &PathBuf, index: &mut HashMap<String, Vec<String>>) {
    let Ok(entries) = std::fs::read_dir(dir) else {
        return;
    };

    for entry in entries.flatten() {
        let path = entry.path();
        if path.is_dir() {
            walk_verb_dir(&path, index);
            continue;
        }

        let is_yaml = path
            .extension()
            .and_then(|ext| ext.to_str())
            .is_some_and(|ext| matches!(ext, "yaml" | "yml"));
        if !is_yaml {
            continue;
        }

        if let Ok(content) = std::fs::read_to_string(&path) {
            parse_verb_yaml(&content, index);
        }
    }
}

fn parse_verb_yaml(content: &str, index: &mut HashMap<String, Vec<String>>) {
    let Ok(value) = serde_yaml::from_str::<serde_yaml::Value>(content) else {
        return;
    };

    let Some(domains) = value.get("domains").and_then(serde_yaml::Value::as_mapping) else {
        return;
    };

    for (domain_name, domain_value) in domains {
        let Some(domain_name) = domain_name.as_str() else {
            continue;
        };
        let Some(verbs) = domain_value
            .get("verbs")
            .and_then(serde_yaml::Value::as_mapping)
        else {
            continue;
        };

        for (verb_key, verb_value) in verbs {
            let Some(verb_key) = verb_key.as_str() else {
                continue;
            };
            let Some(verb_mapping) = verb_value.as_mapping() else {
                continue;
            };

            let phrases = verb_mapping
                .get("invocation_phrases")
                .and_then(serde_yaml::Value::as_sequence)
                .map(|items| {
                    items
                        .iter()
                        .filter_map(|item| item.as_str().map(str::to_string))
                        .collect::<Vec<_>>()
                })
                .unwrap_or_default();

            let fqn = verb_mapping
                .get("verb")
                .and_then(serde_yaml::Value::as_str)
                .map(str::to_string)
                .unwrap_or_else(|| format!("{domain_name}.{verb_key}"));

            let keywords = extract_keywords_from_phrases(&fqn, &phrases);
            if !keywords.is_empty() {
                index.insert(fqn, keywords);
            }
        }
    }
}

fn extract_keywords_from_phrases(verb_fqn: &str, phrases: &[String]) -> Vec<String> {
    let mut token_counts: HashMap<String, usize> = HashMap::new();

    for phrase in phrases {
        let unique: HashSet<String> = phrase
            .to_lowercase()
            .split(|c: char| !c.is_alphanumeric())
            .filter(|token| !token.is_empty() && token.len() > 2)
            .map(normalize_token)
            .filter(|token| !KEYWORD_STOP_WORDS.contains(&token.as_str()))
            .collect();

        for token in unique {
            *token_counts.entry(token).or_insert(0) += 1;
        }
    }

    let mut sorted: Vec<(String, usize)> = token_counts.into_iter().collect();
    sorted.sort_by(|a, b| b.1.cmp(&a.1).then_with(|| a.0.cmp(&b.0)));

    let mut keywords: Vec<String> = sorted
        .into_iter()
        .take(10)
        .map(|(token, _)| token)
        .collect();

    for token in fqn_keywords(verb_fqn) {
        if !keywords.contains(&token) {
            keywords.push(token);
        }
    }

    keywords
}

/// Add verbs from child slots for an existing entity.
fn to_core_constellation_map(map: ConstellationMapDef) -> ConstellationMapDefBody {
    ConstellationMapDefBody {
        fqn: map.constellation.clone(),
        constellation: map.constellation,
        description: map.description,
        jurisdiction: map.jurisdiction,
        slots: map
            .slots
            .into_iter()
            .map(|(name, slot)| (name, to_core_slot(slot)))
            .collect(),
    }
}

fn to_core_slot(slot: RuntimeSlotDef) -> CoreSlotDef {
    CoreSlotDef {
        slot_type: match slot.slot_type {
            RuntimeSlotType::Cbu => CoreSlotType::Cbu,
            RuntimeSlotType::Entity => CoreSlotType::Entity,
            RuntimeSlotType::EntityGraph => CoreSlotType::EntityGraph,
            RuntimeSlotType::Case => CoreSlotType::Case,
            RuntimeSlotType::Tollgate => CoreSlotType::Tollgate,
            RuntimeSlotType::Mandate => CoreSlotType::Mandate,
        },
        entity_kinds: slot.entity_kinds,
        table: slot.table,
        pk: slot.pk,
        join: slot.join.map(|join| CoreJoinDef {
            via: join.via,
            parent_fk: join.parent_fk,
            child_fk: join.child_fk,
            filter_column: join.filter_column,
            filter_value: join.filter_value,
        }),
        occurrence: slot.occurrence,
        cardinality: match slot.cardinality {
            crate::sem_os_runtime::constellation_runtime::Cardinality::Root => Cardinality::Root,
            crate::sem_os_runtime::constellation_runtime::Cardinality::Mandatory => {
                Cardinality::Mandatory
            }
            crate::sem_os_runtime::constellation_runtime::Cardinality::Optional => {
                Cardinality::Optional
            }
            crate::sem_os_runtime::constellation_runtime::Cardinality::Recursive => {
                Cardinality::Recursive
            }
        },
        depends_on: slot
            .depends_on
            .into_iter()
            .map(|dependency| match dependency {
                RuntimeDependencyEntry::Simple(slot) => CoreDependencyEntry::Simple(slot),
                RuntimeDependencyEntry::Explicit { slot, min_state } => {
                    CoreDependencyEntry::Explicit { slot, min_state }
                }
            })
            .collect(),
        placeholder: slot.placeholder,
        state_machine: slot.state_machine,
        overlays: slot.overlays,
        edge_overlays: slot.edge_overlays,
        verbs: slot
            .verbs
            .into_iter()
            .map(|(name, entry)| {
                let entry = match entry {
                    RuntimeVerbPaletteEntry::Simple(verb) => CoreVerbPaletteEntry::Simple(verb),
                    RuntimeVerbPaletteEntry::Gated { verb, when } => CoreVerbPaletteEntry::Gated {
                        verb,
                        when: match when {
                            RuntimeVerbAvailability::One(value) => CoreVerbAvailability::One(value),
                            RuntimeVerbAvailability::Many(values) => {
                                CoreVerbAvailability::Many(values)
                            }
                        },
                    },
                };
                (name, entry)
            })
            .collect(),
        children: slot
            .children
            .into_iter()
            .map(|(name, child)| (name, to_core_slot(child)))
            .collect(),
        max_depth: slot.max_depth,
        closure: None,
        eligibility: None,
        cardinality_max: None,
        entry_state: None,
        attachment_predicates: Vec::new(),
        addition_predicates: Vec::new(),
        aggregate_breach_checks: Vec::new(),
        additive_attachment_predicates: Vec::new(),
        additive_addition_predicates: Vec::new(),
        additive_aggregate_breach_checks: Vec::new(),
        role_guard: None,
        justification_required: None,
        audit_class: None,
        completeness_assertion: None,
    }
}

// ---------------------------------------------------------------------------
// Tests
// ---------------------------------------------------------------------------

#[cfg(test)]
mod tests {
    use super::*;

    #[cfg(feature = "database")]
    use anyhow::Result;
    #[cfg(feature = "database")]
    use sqlx::PgPool;

    #[test]
    fn test_compute_valid_verb_set_basic() {
        let map = match load_constellation_by_id("group.ownership") {
            Ok(map) => map,
            Err(e) => {
                println!("Skipping test — could not load constellation: {}", e);
                return;
            }
        };

        // Mock entity states: empty — no entities exist yet.
        // The valid verb set should contain creation verbs for required slots.
        let entity_states: Vec<EntityState> = vec![];

        let valid = compute_valid_verb_set(&entity_states, &map, Uuid::new_v4());

        println!(
            "Constellation: {} ({} top-level slots)",
            map.constellation,
            map.slots.len()
        );
        println!("Valid verb set ({} verbs):", valid.len());
        for v in &valid.verbs {
            println!(
                "  {} [{}] {:?} — keywords: {:?}",
                v.verb_fqn, v.entity_type, v.source, v.keywords
            );
        }

        assert!(!valid.is_empty(), "Valid verb set should not be empty");
        // With no entities, we should get creation verbs for root/mandatory slots
        assert!(
            !valid.is_empty(),
            "Expected at least 1 creation verb, got 0",
        );
    }

    #[test]
    fn test_valid_verb_set_utility_methods() {
        let valid = ValidVerbSet {
            verbs: vec![
                VerbCandidate {
                    verb_fqn: "cbu.update".to_string(),
                    entity_id: Some(Uuid::new_v4()),
                    entity_type: "cbu".to_string(),
                    source: VerbSource::FsmTransition,
                    priority: 10,
                    keywords: vec!["cbu".to_string(), "update".to_string()],
                },
                VerbCandidate {
                    verb_fqn: "document.solicit".to_string(),
                    entity_id: None,
                    entity_type: "document".to_string(),
                    source: VerbSource::CreationVerb,
                    priority: 5,
                    keywords: vec!["document".to_string(), "solicit".to_string()],
                },
            ],
            client_group_id: Uuid::new_v4(),
            constellation_id: "test".to_string(),
            computed_at: Utc::now(),
        };

        assert_eq!(valid.len(), 2);
        assert!(!valid.is_empty());
        assert!(valid.contains_verb("cbu.update"));
        assert!(valid.contains_verb("document.solicit"));
        assert!(!valid.contains_verb("nonexistent.verb"));

        let fqns = valid.verb_fqns();
        assert_eq!(fqns.len(), 2);

        let allowed = valid.to_allowed_set();
        assert!(allowed.contains("cbu.update"));
        assert!(allowed.contains("document.solicit"));
    }

    #[test]
    fn test_extract_keywords() {
        let document_keywords = extract_keywords_for_verb("document.solicit");
        assert!(document_keywords.contains(&"document".to_string()));
        assert!(document_keywords.contains(&"solicit".to_string()));
        assert!(document_keywords.contains(&"identity".to_string()));

        let case_keywords = extract_keywords_for_verb("kyc-case.create");
        assert!(case_keywords.contains(&"kyc".to_string()));
        assert!(case_keywords.contains(&"case".to_string()));
        assert!(case_keywords.contains(&"create".to_string()));

        let cbu_keywords = extract_keywords_for_verb("cbu.assign-role");
        assert!(cbu_keywords.contains(&"cbu".to_string()));
        assert!(cbu_keywords.contains(&"assign".to_string()));
        assert!(cbu_keywords.contains(&"role".to_string()));
    }

    #[test]
    fn test_is_observation_verb() {
        assert!(is_observation_verb("cbu.read"));
        assert!(is_observation_verb("entity.list"));
        assert!(is_observation_verb("document.for-entity"));
        assert!(!is_observation_verb("cbu.create"));
        assert!(!is_observation_verb("document.solicit"));
    }

    #[test]
    #[allow(deprecated)]
    fn test_semos_scope_hit_rate() {
        use crate::sage::constrained_match::resolve_constrained;

        // Load test utterances from TOML fixture
        let path = PathBuf::from(env!("CARGO_MANIFEST_DIR"))
            .join("tests/fixtures/intent_test_utterances.toml");
        let content = std::fs::read_to_string(&path).expect("Failed to read fixture");

        #[derive(serde::Deserialize)]
        struct Fixture {
            test: Vec<TC>,
        }
        #[derive(serde::Deserialize)]
        struct TC {
            utterance: String,
            expected_verb: String,
            #[serde(default)]
            alt_verbs: Vec<String>,
            #[serde(default)]
            category: String,
        }
        let fixture: Fixture = toml::from_str(&content).expect("Failed to parse TOML");

        // Build a realistic valid verb set from a single session-scoped constellation.
        let group_id = Uuid::new_v4();
        let constellation_id = "struct.lux.ucits.sicav";
        let entity_states = vec![
            EntityState {
                entity_id: Uuid::new_v4(),
                entity_type: "cbu".to_string(),
                current_state: "DISCOVERED".to_string(),
                slot_name: Some("cbu".to_string()),
            },
            EntityState {
                entity_id: Uuid::new_v4(),
                entity_type: "kyc_case".to_string(),
                current_state: "intake".to_string(),
                slot_name: Some("kyc_case".to_string()),
            },
        ];

        let stack =
            load_constellation_stack(constellation_id).expect("failed to load constellation");
        let valid = compute_valid_verb_set_for_constellations(&entity_states, &stack, group_id);

        // Run all utterances through constrained match
        let total = fixture.test.len();
        let mut resolved = 0usize;
        let mut correct = 0usize;
        let mut wrong = 0usize;
        let mut fallthrough = 0usize;
        let mut wrong_cases: Vec<(String, String, String, String)> = Vec::new();
        let mut correct_cases: Vec<(String, String)> = Vec::new();

        for tc in &fixture.test {
            let result = resolve_constrained(&tc.utterance, &valid);
            if result.resolved() {
                resolved += 1;
                let got = result.verb_fqn.as_deref().unwrap_or("");
                if got == tc.expected_verb || tc.alt_verbs.iter().any(|a| a == got) {
                    correct += 1;
                    correct_cases.push((tc.utterance.clone(), got.to_string()));
                } else {
                    wrong += 1;
                    wrong_cases.push((
                        tc.utterance.clone(),
                        tc.expected_verb.clone(),
                        got.to_string(),
                        tc.category.clone(),
                    ));
                }
            } else {
                fallthrough += 1;
            }
        }

        println!("\n============================================================");
        println!("  SemOS-SCOPED RESOLUTION HIT RATE (simulated session)");
        println!("============================================================");
        println!("  Constellation:         {}", valid.constellation_id);
        println!("  Valid verb set:        {} verbs", valid.len());
        println!("  Valid verb contents:   {:?}", valid.verb_fqns());
        println!("  Total utterances:      {}", total);
        println!(
            "  Resolved (constrained): {} ({:.1}%)",
            resolved,
            resolved as f64 / total as f64 * 100.0
        );
        println!(
            "  ├─ Correct:            {} ({:.1}% of total)",
            correct,
            correct as f64 / total as f64 * 100.0
        );
        println!(
            "  └─ Wrong:              {} ({:.1}% of total)",
            wrong,
            wrong as f64 / total as f64 * 100.0
        );
        println!(
            "  Fallthrough to open:   {} ({:.1}%)",
            fallthrough,
            fallthrough as f64 / total as f64 * 100.0
        );
        println!(
            "  Constrained accuracy:  {:.1}%",
            if resolved > 0 {
                correct as f64 / resolved as f64 * 100.0
            } else {
                0.0
            }
        );

        if !wrong_cases.is_empty() {
            println!("\n  WRONG ({}):", wrong_cases.len());
            for (utt, expected, got, cat) in &wrong_cases {
                let short = if utt.len() > 50 { &utt[..50] } else { utt };
                println!(
                    "    ✗ [{}] '{}' → {} (expected {})",
                    cat, short, got, expected
                );
            }
        }

        println!("\n  CORRECT SAMPLE (first 15):");
        for (utt, got) in correct_cases.iter().take(15) {
            let short = if utt.len() > 45 { &utt[..45] } else { utt };
            println!("    ✓ '{}' → {}", short, got);
        }
        println!("============================================================\n");
    }

    #[cfg(feature = "database")]
    #[derive(Debug, Clone)]
    struct SeededScenario {
        name: &'static str,
        group_id: Uuid,
        cbu_id: Option<Uuid>,
        case_id: Option<Uuid>,
        entity_ids: Vec<Uuid>,
        workstream_ids: Vec<Uuid>,
        request_ids: Vec<Uuid>,
        agreement_ids: Vec<Uuid>,
        screening_ids: Vec<Uuid>,
        tollgate_ids: Vec<Uuid>,
        relationship_ids: Vec<Uuid>,
    }

    #[cfg(feature = "database")]
    async fn test_pool() -> Result<PgPool> {
        let url =
            std::env::var("DATABASE_URL").unwrap_or_else(|_| "postgresql:///data_designer".into());
        Ok(PgPool::connect(&url).await?)
    }

    #[cfg(feature = "database")]
    async fn resolve_entity_type_id(pool: &PgPool, kind: &str) -> Result<Uuid> {
        let candidates: &[&str] = match kind {
            "company" => &[
                "LIMITED_COMPANY_PRIVATE",
                "LIMITED_COMPANY",
                "LEGAL_ENTITY",
                "COMPANY",
            ],
            "person" => &[
                "PROPER_PERSON_NATURAL",
                "NATURAL_PERSON",
                "INDIVIDUAL",
                "PERSON",
            ],
            _ => &["LEGAL_ENTITY"],
        };

        let id = sqlx::query_scalar(
            r#"
            SELECT entity_type_id
            FROM "ob-poc".entity_types
            WHERE UPPER(COALESCE(type_code, '')) = ANY($1)
               OR UPPER(name) = ANY($1)
            ORDER BY entity_type_id
            LIMIT 1
            "#,
        )
        .bind(
            candidates
                .iter()
                .map(|candidate| candidate.to_string())
                .collect::<Vec<_>>(),
        )
        .fetch_one(pool)
        .await?;

        Ok(id)
    }

    #[cfg(feature = "database")]
    async fn resolve_role_id(pool: &PgPool, candidates: &[&str]) -> Result<Option<Uuid>> {
        let role_id = sqlx::query_scalar(
            r#"
            SELECT role_id
            FROM "ob-poc".roles
            WHERE UPPER(name) = ANY($1)
            ORDER BY display_priority DESC, sort_order ASC
            LIMIT 1
            "#,
        )
        .bind(
            candidates
                .iter()
                .map(|candidate| candidate.to_string())
                .collect::<Vec<_>>(),
        )
        .fetch_optional(pool)
        .await?;

        Ok(role_id)
    }

    #[cfg(feature = "database")]
    async fn insert_client_group(pool: &PgPool, group_id: Uuid, name: &str) -> Result<()> {
        sqlx::query(
            r#"
            INSERT INTO "ob-poc".client_group (id, canonical_name, discovery_status)
            VALUES ($1, $2, 'complete')
            "#,
        )
        .bind(group_id)
        .bind(name)
        .execute(pool)
        .await?;

        sqlx::query(
            r#"
            INSERT INTO "ob-poc".client_group_alias (group_id, alias, alias_norm, source, is_primary)
            VALUES ($1, $2, LOWER($2), 'test', true)
            "#,
        )
        .bind(group_id)
        .bind(name)
        .execute(pool)
        .await?;

        Ok(())
    }

    #[cfg(feature = "database")]
    async fn insert_entity(
        pool: &PgPool,
        entity_id: Uuid,
        entity_type_id: Uuid,
        name: &str,
    ) -> Result<()> {
        sqlx::query(
            r#"
            INSERT INTO "ob-poc".entities (entity_id, entity_type_id, name, name_norm)
            VALUES ($1, $2, $3, LOWER($3))
            "#,
        )
        .bind(entity_id)
        .bind(entity_type_id)
        .bind(name)
        .execute(pool)
        .await?;
        Ok(())
    }

    #[cfg(feature = "database")]
    async fn insert_group_membership(
        pool: &PgPool,
        group_id: Uuid,
        entity_id: Uuid,
        cbu_id: Option<Uuid>,
    ) -> Result<()> {
        sqlx::query(
            r#"
            INSERT INTO "ob-poc".client_group_entity
                (group_id, entity_id, membership_type, added_by, review_status, cbu_id)
            VALUES ($1, $2, 'confirmed', 'test', 'confirmed', $3)
            "#,
        )
        .bind(group_id)
        .bind(entity_id)
        .bind(cbu_id)
        .execute(pool)
        .await?;
        Ok(())
    }

    #[cfg(feature = "database")]
    async fn seed_kyc_onboarding_scenarios(pool: &PgPool) -> Result<Vec<SeededScenario>> {
        let company_type_id = resolve_entity_type_id(pool, "company").await?;
        let person_type_id = resolve_entity_type_id(pool, "person").await?;
        let management_role_id =
            resolve_role_id(pool, &["MANAGEMENT_COMPANY", "MANAGEMENT-COMPANY"]).await?;
        let depositary_role_id = resolve_role_id(pool, &["DEPOSITARY"]).await?;

        let cold = SeededScenario {
            name: "cold",
            group_id: Uuid::new_v4(),
            cbu_id: None,
            case_id: None,
            entity_ids: Vec::new(),
            workstream_ids: Vec::new(),
            request_ids: Vec::new(),
            agreement_ids: Vec::new(),
            screening_ids: Vec::new(),
            tollgate_ids: Vec::new(),
            relationship_ids: Vec::new(),
        };
        insert_client_group(
            pool,
            cold.group_id,
            &format!(
                "test-group-cold-{}",
                &cold.group_id.simple().to_string()[..8]
            ),
        )
        .await?;

        let mid_group_id = Uuid::new_v4();
        let mid_cbu_id = Uuid::new_v4();
        let mid_case_id = Uuid::new_v4();
        let mid_manco_id = Uuid::new_v4();
        let mid_depositary_id = Uuid::new_v4();
        let mid_ubo_id = Uuid::new_v4();
        let mid_workstream_id = Uuid::new_v4();
        let mid_verified_workstream_id = Uuid::new_v4();
        let mid_request_id = Uuid::new_v4();
        let mid_agreement_id = Uuid::new_v4();
        let mid_identifier_id = Uuid::new_v4();
        let mid_relationship_id = Uuid::new_v4();

        insert_client_group(
            pool,
            mid_group_id,
            &format!("test-group-mid-{}", &mid_group_id.simple().to_string()[..8]),
        )
        .await?;
        insert_entity(
            pool,
            mid_manco_id,
            company_type_id,
            &format!("Test Mid ManCo {}", &mid_manco_id.simple().to_string()[..8]),
        )
        .await?;
        insert_entity(
            pool,
            mid_depositary_id,
            company_type_id,
            &format!(
                "Test Mid Depositary {}",
                &mid_depositary_id.simple().to_string()[..8]
            ),
        )
        .await?;
        insert_entity(
            pool,
            mid_ubo_id,
            person_type_id,
            &format!("John Mid UBO {}", &mid_ubo_id.simple().to_string()[..8]),
        )
        .await?;

        sqlx::query(
            r#"
            INSERT INTO "ob-poc".cbus
                (cbu_id, name, jurisdiction, client_type, commercial_client_entity_id, status)
            VALUES ($1, $2, 'LU', 'FUND', $3, 'VALIDATED')
            "#,
        )
        .bind(mid_cbu_id)
        .bind(format!(
            "Test Mid-Onboarding SICAV {}",
            &mid_cbu_id.simple().to_string()[..8]
        ))
        .bind(mid_manco_id)
        .execute(pool)
        .await?;

        insert_group_membership(pool, mid_group_id, mid_manco_id, Some(mid_cbu_id)).await?;
        insert_group_membership(pool, mid_group_id, mid_depositary_id, Some(mid_cbu_id)).await?;
        insert_group_membership(pool, mid_group_id, mid_ubo_id, Some(mid_cbu_id)).await?;

        if let Some(role_id) = management_role_id {
            sqlx::query(
                r#"
                INSERT INTO "ob-poc".cbu_entity_roles (cbu_id, entity_id, role_id)
                VALUES ($1, $2, $3)
                "#,
            )
            .bind(mid_cbu_id)
            .bind(mid_manco_id)
            .bind(role_id)
            .execute(pool)
            .await?;
        }
        if let Some(role_id) = depositary_role_id {
            sqlx::query(
                r#"
                INSERT INTO "ob-poc".cbu_entity_roles (cbu_id, entity_id, role_id)
                VALUES ($1, $2, $3)
                "#,
            )
            .bind(mid_cbu_id)
            .bind(mid_depositary_id)
            .bind(role_id)
            .execute(pool)
            .await?;
        }

        sqlx::query(
            r#"
            INSERT INTO "ob-poc".cases
                (case_id, cbu_id, case_ref, client_group_id, subject_entity_id, status, case_type)
            VALUES ($1, $2, $3, $4, $5, 'DISCOVERY', 'NEW_CLIENT')
            "#,
        )
        .bind(mid_case_id)
        .bind(mid_cbu_id)
        .bind(format!("MID-{}", &mid_case_id.simple().to_string()[..8]))
        .bind(mid_group_id)
        .bind(mid_manco_id)
        .execute(pool)
        .await?;

        sqlx::query(
            r#"
            INSERT INTO "ob-poc".entity_workstreams
                (workstream_id, case_id, entity_id, status, blocker_type, blocker_message)
            VALUES ($1, $2, $3, 'COLLECT', 'AWAITING_DOCUMENT', 'Pending onboarding documents')
            "#,
        )
        .bind(mid_workstream_id)
        .bind(mid_case_id)
        .bind(mid_manco_id)
        .execute(pool)
        .await?;

        sqlx::query(
            r#"
            INSERT INTO "ob-poc".entity_workstreams
                (workstream_id, case_id, entity_id, status, risk_rating, screening_cleared, evidence_complete)
            VALUES ($1, $2, $3, 'COMPLETE', 'LOW', true, true)
            "#,
        )
        .bind(mid_verified_workstream_id)
        .bind(mid_case_id)
        .bind(mid_depositary_id)
        .execute(pool)
        .await?;

        sqlx::query(
            r#"
            INSERT INTO "ob-poc".outstanding_requests
                (request_id, subject_type, subject_id, workstream_id, case_id, cbu_id, entity_id,
                 request_type, request_subtype, status, created_by_verb, reason_for_request)
            VALUES
                ($1, 'WORKSTREAM', $2, $2, $3, $4, $5, 'DOCUMENT', 'KYC_EVIDENCE', 'PENDING',
                 'document.solicit', 'Need onboarding document pack')
            "#,
        )
        .bind(mid_request_id)
        .bind(mid_workstream_id)
        .bind(mid_case_id)
        .bind(mid_cbu_id)
        .bind(mid_manco_id)
        .execute(pool)
        .await?;

        sqlx::query(
            r#"
            INSERT INTO "ob-poc".kyc_service_agreements
                (agreement_id, sponsor_cbu_id, sponsor_entity_id, agreement_reference, effective_date, status)
            VALUES ($1, $2, $3, $4, CURRENT_DATE, 'DRAFT')
            "#,
        )
        .bind(mid_agreement_id)
        .bind(mid_cbu_id)
        .bind(mid_manco_id)
        .bind("MID-KYC-AGR")
        .execute(pool)
        .await?;

        sqlx::query(
            r#"
            UPDATE "ob-poc".cases
            SET service_agreement_id = $2
            WHERE case_id = $1
            "#,
        )
        .bind(mid_case_id)
        .bind(mid_agreement_id)
        .execute(pool)
        .await?;

        sqlx::query(
            r#"
            INSERT INTO "ob-poc".entity_identifiers
                (identifier_id, entity_id, identifier_type, identifier_value, source, is_primary, is_validated)
            VALUES ($1, $2, 'LEI', $3, 'test', true, false)
            "#,
        )
        .bind(mid_identifier_id)
        .bind(mid_manco_id)
        .bind(format!("LEI-{}", &mid_manco_id.simple().to_string()[..12]))
        .execute(pool)
        .await?;

        sqlx::query(
            r#"
            INSERT INTO "ob-poc".entity_relationships
                (relationship_id, from_entity_id, to_entity_id, relationship_type,
                 percentage, source, confidence)
            VALUES ($1, $2, $3, 'ownership', 62.50, 'test', 'high')
            "#,
        )
        .bind(mid_relationship_id)
        .bind(mid_ubo_id)
        .bind(mid_manco_id)
        .execute(pool)
        .await?;

        let mid = SeededScenario {
            name: "mid",
            group_id: mid_group_id,
            cbu_id: Some(mid_cbu_id),
            case_id: Some(mid_case_id),
            entity_ids: vec![mid_manco_id, mid_depositary_id, mid_ubo_id],
            workstream_ids: vec![mid_workstream_id, mid_verified_workstream_id],
            request_ids: vec![mid_request_id],
            agreement_ids: vec![mid_agreement_id],
            screening_ids: Vec::new(),
            tollgate_ids: Vec::new(),
            relationship_ids: vec![mid_relationship_id],
        };

        let late_group_id = Uuid::new_v4();
        let late_cbu_id = Uuid::new_v4();
        let late_case_id = Uuid::new_v4();
        let late_manco_id = Uuid::new_v4();
        let late_depositary_id = Uuid::new_v4();
        let late_workstream_one = Uuid::new_v4();
        let late_workstream_two = Uuid::new_v4();
        let late_agreement_id = Uuid::new_v4();
        let late_tollgate_id = Uuid::new_v4();
        let late_identifier_id = Uuid::new_v4();
        let late_request_id = Uuid::new_v4();
        let late_screening_ids = vec![Uuid::new_v4(), Uuid::new_v4(), Uuid::new_v4()];

        insert_client_group(
            pool,
            late_group_id,
            &format!(
                "test-group-late-{}",
                &late_group_id.simple().to_string()[..8]
            ),
        )
        .await?;
        insert_entity(
            pool,
            late_manco_id,
            company_type_id,
            &format!(
                "Test Late ManCo {}",
                &late_manco_id.simple().to_string()[..8]
            ),
        )
        .await?;
        insert_entity(
            pool,
            late_depositary_id,
            company_type_id,
            &format!(
                "Test Late Depositary {}",
                &late_depositary_id.simple().to_string()[..8]
            ),
        )
        .await?;

        sqlx::query(
            r#"
            INSERT INTO "ob-poc".cbus
                (cbu_id, name, jurisdiction, client_type, commercial_client_entity_id, status)
            VALUES ($1, $2, 'LU', 'FUND', $3, 'VALIDATED')
            "#,
        )
        .bind(late_cbu_id)
        .bind(format!(
            "Test Late-Onboarding SICAV {}",
            &late_cbu_id.simple().to_string()[..8]
        ))
        .bind(late_manco_id)
        .execute(pool)
        .await?;

        insert_group_membership(pool, late_group_id, late_manco_id, Some(late_cbu_id)).await?;
        insert_group_membership(pool, late_group_id, late_depositary_id, Some(late_cbu_id)).await?;

        sqlx::query(
            r#"
            INSERT INTO "ob-poc".cases
                (case_id, cbu_id, case_ref, client_group_id, subject_entity_id, status, case_type, risk_rating)
            VALUES ($1, $2, $3, $4, $5, 'REVIEW', 'NEW_CLIENT', 'LOW')
            "#
        )
        .bind(late_case_id)
        .bind(late_cbu_id)
        .bind(format!("LATE-{}", &late_case_id.simple().to_string()[..8]))
        .bind(late_group_id)
        .bind(late_manco_id)
        .execute(pool)
        .await?;

        for (workstream_id, entity_id) in [
            (late_workstream_one, late_manco_id),
            (late_workstream_two, late_depositary_id),
        ] {
            sqlx::query(
                r#"
                INSERT INTO "ob-poc".entity_workstreams
                    (workstream_id, case_id, entity_id, status, risk_rating,
                     identity_verified, ownership_proved, screening_cleared, evidence_complete)
                VALUES ($1, $2, $3, 'COMPLETE', 'LOW', true, true, true, true)
                "#,
            )
            .bind(workstream_id)
            .bind(late_case_id)
            .bind(entity_id)
            .execute(pool)
            .await?;
        }

        for (screening_id, screening_type) in
            late_screening_ids
                .iter()
                .zip(["SANCTIONS", "PEP", "ADVERSE_MEDIA"])
        {
            sqlx::query(
                r#"
                INSERT INTO "ob-poc".screenings
                    (screening_id, workstream_id, screening_type, status, match_count)
                VALUES ($1, $2, $3, 'CLEAR', 0)
                "#,
            )
            .bind(screening_id)
            .bind(late_workstream_one)
            .bind(screening_type)
            .execute(pool)
            .await?;
        }

        sqlx::query(
            r#"
            INSERT INTO "ob-poc".kyc_service_agreements
                (agreement_id, sponsor_cbu_id, sponsor_entity_id, agreement_reference, effective_date, status)
            VALUES ($1, $2, $3, $4, CURRENT_DATE, 'SIGNED')
            "#
        )
        .bind(late_agreement_id)
        .bind(late_cbu_id)
        .bind(late_manco_id)
        .bind("LATE-KYC-AGR")
        .execute(pool)
        .await?;

        sqlx::query(
            r#"
            UPDATE "ob-poc".cases
            SET service_agreement_id = $2
            WHERE case_id = $1
            "#,
        )
        .bind(late_case_id)
        .bind(late_agreement_id)
        .execute(pool)
        .await?;

        sqlx::query(
            r#"
            INSERT INTO "ob-poc".entity_identifiers
                (identifier_id, entity_id, identifier_type, identifier_value, source, is_primary, is_validated)
            VALUES ($1, $2, 'LEI', $3, 'test', true, true)
            "#,
        )
        .bind(late_identifier_id)
        .bind(late_manco_id)
        .bind(format!("LEI-{}", &late_manco_id.simple().to_string()[..12]))
        .execute(pool)
        .await?;

        sqlx::query(
            r#"
            INSERT INTO "ob-poc".outstanding_requests
                (request_id, subject_type, subject_id, workstream_id, case_id, cbu_id, entity_id,
                 request_type, request_subtype, status, created_by_verb, reason_for_request)
            VALUES
                ($1, 'WORKSTREAM', $2, $2, $3, $4, $5, 'DOCUMENT', 'KYC_EVIDENCE', 'FULFILLED',
                 'document.solicit', 'Previously completed onboarding evidence request')
            "#,
        )
        .bind(late_request_id)
        .bind(late_workstream_one)
        .bind(late_case_id)
        .bind(late_cbu_id)
        .bind(late_manco_id)
        .execute(pool)
        .await?;

        sqlx::query(
            r#"
            INSERT INTO "ob-poc".tollgate_evaluations
                (evaluation_id, case_id, tollgate_id, passed, evaluation_detail, config_version)
            VALUES ($1, $2, 'REVIEW_COMPLETE', true, '{}'::jsonb, 'test')
            "#,
        )
        .bind(late_tollgate_id)
        .bind(late_case_id)
        .execute(pool)
        .await?;

        let late = SeededScenario {
            name: "late",
            group_id: late_group_id,
            cbu_id: Some(late_cbu_id),
            case_id: Some(late_case_id),
            entity_ids: vec![late_manco_id, late_depositary_id],
            workstream_ids: vec![late_workstream_one, late_workstream_two],
            request_ids: vec![late_request_id],
            agreement_ids: vec![late_agreement_id],
            screening_ids: late_screening_ids,
            tollgate_ids: vec![late_tollgate_id],
            relationship_ids: Vec::new(),
        };

        Ok(vec![cold, mid, late])
    }

    #[cfg(feature = "database")]
    async fn cleanup_scenario(pool: &PgPool, scenario: &SeededScenario) {
        let _ = sqlx::query(
            r#"DELETE FROM "ob-poc".tollgate_evaluations WHERE evaluation_id = ANY($1)"#,
        )
        .bind(&scenario.tollgate_ids)
        .execute(pool)
        .await;
        let _ =
            sqlx::query(r#"DELETE FROM "ob-poc".outstanding_requests WHERE request_id = ANY($1)"#)
                .bind(&scenario.request_ids)
                .execute(pool)
                .await;
        let _ = sqlx::query(r#"DELETE FROM "ob-poc".screenings WHERE screening_id = ANY($1)"#)
            .bind(&scenario.screening_ids)
            .execute(pool)
            .await;
        let _ = sqlx::query(r#"DELETE FROM "ob-poc".entity_identifiers WHERE entity_id = ANY($1)"#)
            .bind(&scenario.entity_ids)
            .execute(pool)
            .await;
        let _ =
            sqlx::query(r#"DELETE FROM "ob-poc".entity_workstreams WHERE workstream_id = ANY($1)"#)
                .bind(&scenario.workstream_ids)
                .execute(pool)
                .await;
        if let Some(case_id) = scenario.case_id {
            let _ = sqlx::query(r#"DELETE FROM "ob-poc".cases WHERE case_id = $1"#)
                .bind(case_id)
                .execute(pool)
                .await;
        }
        let _ = sqlx::query(
            r#"DELETE FROM "ob-poc".entity_relationships WHERE relationship_id = ANY($1)"#,
        )
        .bind(&scenario.relationship_ids)
        .execute(pool)
        .await;
        let _ = sqlx::query(r#"DELETE FROM "ob-poc".cbu_entity_roles WHERE cbu_id = $1"#)
            .bind(scenario.cbu_id)
            .execute(pool)
            .await;
        let _ = sqlx::query(r#"DELETE FROM "ob-poc".client_group_entity WHERE group_id = $1"#)
            .bind(scenario.group_id)
            .execute(pool)
            .await;
        let _ = sqlx::query(
            r#"DELETE FROM "ob-poc".kyc_service_agreements WHERE agreement_id = ANY($1)"#,
        )
        .bind(&scenario.agreement_ids)
        .execute(pool)
        .await;
        if let Some(cbu_id) = scenario.cbu_id {
            let _ = sqlx::query(r#"DELETE FROM "ob-poc".cbus WHERE cbu_id = $1"#)
                .bind(cbu_id)
                .execute(pool)
                .await;
        }
        let _ = sqlx::query(r#"DELETE FROM "ob-poc".entities WHERE entity_id = ANY($1)"#)
            .bind(&scenario.entity_ids)
            .execute(pool)
            .await;
        let _ = sqlx::query(r#"DELETE FROM "ob-poc".client_group_alias WHERE group_id = $1"#)
            .bind(scenario.group_id)
            .execute(pool)
            .await;
        let _ = sqlx::query(r#"DELETE FROM "ob-poc".client_group WHERE id = $1"#)
            .bind(scenario.group_id)
            .execute(pool)
            .await;
    }

    #[cfg(feature = "database")]
    #[tokio::test]
    #[ignore]
    #[allow(deprecated)]
    async fn test_semos_scope_hit_rate_seeded_db() -> Result<()> {
        use std::{sync::Arc, time::Instant};

        use crate::agent::learning::embedder::{CandleEmbedder, Embedder};
        use crate::database::verb_service::VerbService;
        use crate::mcp::verb_search::HybridVerbSearcher;
        use crate::sage::constrained_match::{resolve_constrained_hybrid, MatchStrategy};
        use crate::sage::session_context::{load_entity_states_for_group, SageSession};

        let pool = test_pool().await?;
        let scenarios = seed_kyc_onboarding_scenarios(&pool).await?;
        let stack = load_constellation_stack("struct.lux.ucits.sicav")?;
        let kyc_map = stack
            .iter()
            .find(|map| map.constellation == "kyc.onboarding")
            .cloned()
            .expect("kyc.onboarding must be present in composed stack");
        let model = ConstellationModel::from_parts(kyc_map.clone(), load_state_machine_bodies()?);
        let embedder = tokio::task::spawn_blocking(CandleEmbedder::new).await??;
        let shared_embedder: Arc<dyn Embedder> = Arc::new(embedder);
        let searcher = HybridVerbSearcher::new(Arc::new(VerbService::new(pool.clone())), None)
            .with_embedder(shared_embedder);

        for scenario in &scenarios {
            let entity_states = load_entity_states_for_group(&pool, scenario.group_id).await?;
            let slot_states = build_slot_states(&entity_states, &model);
            let valid = compute_valid_verb_set_for_constellations(
                &entity_states,
                &stack,
                scenario.group_id,
            );
            println!("\nScenario {}:", scenario.name);
            println!(
                "  session: {:?}",
                SageSession::from_ui_context(
                    scenario.group_id,
                    "kyc.onboarding".to_string(),
                    scenario.cbu_id
                )
            );
            println!("  loaded entity states: {:?}", entity_states);
            for slot_name in model.slots.keys() {
                let surface = compute_slot_action_surface(&model, &slot_states, slot_name)
                    .unwrap_or_default();
                let transition_count = surface
                    .valid_actions
                    .iter()
                    .filter(|action| !is_observation_verb(&action.action_id))
                    .count();
                let observation_count =
                    surface.valid_actions.len().saturating_sub(transition_count);
                println!(
                    "  slot {:<18} state={:<18} valid={} blocked={} transitions={} observations={}",
                    slot_name,
                    slot_states
                        .get(slot_name)
                        .map(String::as_str)
                        .unwrap_or("<empty>"),
                    surface.valid_actions.len(),
                    surface.blocked_actions.len(),
                    transition_count,
                    observation_count
                );
                for action in &surface.valid_actions {
                    println!("    ✓ {}", action.action_id);
                }
            }
            println!("  total unique verbs: {}", valid.len());
            println!("  valid verbs: {:?}", valid.verb_fqns());
        }

        let path = PathBuf::from(env!("CARGO_MANIFEST_DIR"))
            .join("tests/fixtures/intent_test_utterances.toml");
        let content = std::fs::read_to_string(&path)?;

        #[derive(serde::Deserialize)]
        struct Fixture {
            test: Vec<TC>,
        }
        #[derive(serde::Deserialize)]
        struct TC {
            utterance: String,
            expected_verb: String,
            #[serde(default)]
            alt_verbs: Vec<String>,
            #[serde(default)]
            category: String,
        }

        let fixture: Fixture = toml::from_str(&content)?;
        let mid = scenarios
            .iter()
            .find(|scenario| scenario.name == "mid")
            .expect("mid scenario must exist");
        let entity_states = load_entity_states_for_group(&pool, mid.group_id).await?;
        let valid = compute_valid_verb_set_for_constellations(&entity_states, &stack, mid.group_id);

        for verb_fqn in [
            "document.solicit",
            "kyc-case.create",
            "screening.run",
            "entity.read",
            "request.create",
        ] {
            let keywords = extract_keywords_for_verb(verb_fqn);
            println!("{verb_fqn} keywords: {:?}", keywords);
        }

        let litmus = "request identity documents for due diligence";
        let litmus_result = resolve_constrained_hybrid(litmus, &valid, &searcher, None).await?;
        println!(
            "litmus: '{}' -> {:?} (confidence {:.2}, strategy {:?})",
            litmus, litmus_result.verb_fqn, litmus_result.confidence, litmus_result.strategy
        );

        let total = fixture.test.len();
        let mut resolved = 0usize;
        let mut correct = 0usize;
        let mut wrong = 0usize;
        let mut fallthrough = 0usize;
        let mut keyword_resolved = 0usize;
        let mut scoped_embedding_resolved = 0usize;
        let mut total_latency_ms = 0.0f64;
        let mut near_miss_count = 0usize;

        for tc in &fixture.test {
            let started = Instant::now();
            let result = resolve_constrained_hybrid(&tc.utterance, &valid, &searcher, None).await?;
            total_latency_ms += started.elapsed().as_secs_f64() * 1000.0;
            if result.resolved() {
                resolved += 1;
                match result.strategy {
                    MatchStrategy::Keyword => keyword_resolved += 1,
                    MatchStrategy::ScopedEmbedding => scoped_embedding_resolved += 1,
                    MatchStrategy::Fallthrough => {}
                }
                let got = result.verb_fqn.as_deref().unwrap_or("");
                if got == tc.expected_verb || tc.alt_verbs.iter().any(|alt| alt == got) {
                    correct += 1;
                    println!(
                        "  resolved: [{}] '{}' -> {} via {:?} ({:.3}) [correct]",
                        tc.category, tc.utterance, got, result.strategy, result.confidence
                    );
                } else {
                    wrong += 1;
                    println!(
                        "  resolved: [{}] '{}' -> {} via {:?} ({:.3}) [wrong; expected {}]",
                        tc.category,
                        tc.utterance,
                        got,
                        result.strategy,
                        result.confidence,
                        tc.expected_verb
                    );
                }
            } else {
                fallthrough += 1;
                let scoped_results = searcher
                    .search_embeddings_only(&tc.utterance, 3, Some(&valid.to_allowed_set()))
                    .await?;
                if let Some(top) = scoped_results.first() {
                    let margin = top.score
                        - scoped_results
                            .get(1)
                            .map(|candidate| candidate.score)
                            .unwrap_or(0.0);
                    if (0.70..0.80).contains(&top.score) {
                        near_miss_count += 1;
                        println!(
                            "  near-miss: '{}' -> {} ({:.3}) [margin: {:.3}]",
                            tc.utterance, top.verb, top.score, margin
                        );
                    }
                }
            }
        }

        println!("\n============================================================");
        println!("  SemOS-SCOPED RESOLUTION HIT RATE (seeded DB session)");
        println!("============================================================");
        println!("  Constellation:         {}", valid.constellation_id);
        println!("  Valid verb set:        {} verbs", valid.len());
        println!("  Valid verb contents:   {:?}", valid.verb_fqns());
        println!("  Total utterances:      {}", total);
        println!("  Keyword resolved:      {}", keyword_resolved);
        println!("  Scoped embedding:      {}", scoped_embedding_resolved);
        println!("  Resolved (constrained): {}", resolved);
        println!("  Correct:               {}", correct);
        println!("  Wrong:                 {}", wrong);
        println!("  Fallthrough to open:   {}", fallthrough);
        println!(
            "  Avg constrained ms:    {:.2}",
            if total > 0 {
                total_latency_ms / total as f64
            } else {
                0.0
            }
        );
        println!(
            "  Constrained accuracy:  {:.1}%",
            if resolved > 0 {
                correct as f64 / resolved as f64 * 100.0
            } else {
                0.0
            }
        );
        println!("  Near-misses (0.70-0.80): {}", near_miss_count);
        println!("============================================================\n");

        for scenario in &scenarios {
            cleanup_scenario(&pool, scenario).await;
        }

        Ok(())
    }
}
