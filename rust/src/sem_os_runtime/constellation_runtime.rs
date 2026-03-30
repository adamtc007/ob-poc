//! Transitional Sem OS-backed constellation runtime surface.
//!
//! This module concentrates the remaining public hydration/summary runtime
//! entrypoints behind a Sem OS-oriented namespace while the internal
//! implementation is still being migrated off the legacy constellation module.

use anyhow::Result;
use chrono::{DateTime, Utc};
use sem_os_core::constellation_map_def as core_map;
use sem_os_core::grounding::{compute_slot_action_surface, ConstellationModel};
use sem_os_core::state_machine_def as core_sm;

use crate::sem_os_runtime::reducer_runtime::load_runtime_state_machine;
use serde::{Deserialize, Serialize};
use sha2::{Digest, Sha256};
use sqlx::PgPool;
use std::collections::BTreeMap;
use std::collections::HashMap;
use std::collections::HashSet;
use std::path::PathBuf;
use thiserror::Error;
use uuid::Uuid;

/// Constellation runtime error type.
#[derive(Debug, Error)]
pub enum ConstellationError {
    #[error("parse error: {0}")]
    Parse(String),
    #[error("validation error: {0}")]
    Validation(String),
    #[error("execution error: {0}")]
    Execution(String),
    #[error(transparent)]
    Other(#[from] anyhow::Error),
}

/// Standard result type for constellation runtime operations.
pub type ConstellationResult<T> = Result<T, ConstellationError>;

const SUPPORTED_BULK_MACROS: &[&str] = &["role_slots"];

/// Raw constellation map definition loaded from YAML.
#[derive(Debug, Clone, Deserialize, Serialize)]
pub struct ConstellationMapDef {
    pub constellation: String,
    pub description: Option<String>,
    pub jurisdiction: String,
    pub slots: BTreeMap<String, SlotDef>,
    #[serde(default)]
    pub bulk_macros: Vec<String>,
}

/// Raw slot definition loaded from YAML.
#[derive(Debug, Clone, Deserialize, Serialize)]
pub struct SlotDef {
    #[serde(rename = "type")]
    pub slot_type: SlotType,
    #[serde(default)]
    pub entity_kinds: Vec<String>,
    pub table: Option<String>,
    pub pk: Option<String>,
    pub join: Option<JoinDef>,
    pub occurrence: Option<usize>,
    pub cardinality: Cardinality,
    #[serde(default)]
    pub depends_on: Vec<DependencyEntry>,
    #[serde(default, deserialize_with = "deserialize_placeholder")]
    pub placeholder: Option<String>,
    #[serde(default = "default_placeholder_detection")]
    pub placeholder_detection: String,
    pub state_machine: Option<String>,
    #[serde(default)]
    pub overlays: Vec<String>,
    #[serde(default)]
    pub edge_overlays: Vec<String>,
    #[serde(default)]
    pub verbs: BTreeMap<String, VerbPaletteEntry>,
    #[serde(default)]
    pub children: BTreeMap<String, SlotDef>,
    pub max_depth: Option<usize>,
}

fn default_placeholder_detection() -> String {
    String::from("name_match")
}

fn deserialize_placeholder<'de, D>(deserializer: D) -> Result<Option<String>, D::Error>
where
    D: serde::Deserializer<'de>,
{
    #[derive(Deserialize)]
    #[serde(untagged)]
    enum PlaceholderValue {
        Bool(bool),
        Text(String),
    }

    let value = Option::<PlaceholderValue>::deserialize(deserializer)?;
    Ok(match value {
        Some(PlaceholderValue::Bool(true)) => Some(String::from("placeholder")),
        Some(PlaceholderValue::Bool(false)) | None => None,
        Some(PlaceholderValue::Text(value)) => Some(value),
    })
}

/// Supported slot classes.
#[derive(Debug, Clone, Copy, Deserialize, Serialize, PartialEq, Eq)]
#[serde(rename_all = "snake_case")]
pub enum SlotType {
    Cbu,
    Entity,
    EntityGraph,
    Case,
    Tollgate,
    Mandate,
}

/// Supported cardinality semantics.
#[derive(Debug, Clone, Copy, Deserialize, Serialize, PartialEq, Eq)]
#[serde(rename_all = "snake_case")]
pub enum Cardinality {
    Root,
    Mandatory,
    Optional,
    Recursive,
}

/// Join definition for non-root slots.
#[derive(Debug, Clone, Deserialize, Serialize, PartialEq, Eq)]
pub struct JoinDef {
    pub via: String,
    pub parent_fk: String,
    pub child_fk: String,
    pub filter_column: Option<String>,
    pub filter_value: Option<String>,
}

/// Dependency declaration for a slot.
#[derive(Debug, Clone, Deserialize, Serialize, PartialEq, Eq)]
#[serde(untagged)]
pub enum DependencyEntry {
    Simple(String),
    Explicit { slot: String, min_state: String },
}

impl DependencyEntry {
    /// Return the referenced slot name.
    ///
    /// # Examples
    /// ```rust
    /// use ob_poc::sem_os_runtime::constellation_runtime::DependencyEntry;
    ///
    /// let dep = DependencyEntry::Simple(String::from("cbu"));
    /// assert_eq!(dep.slot_name(), "cbu");
    /// ```
    pub fn slot_name(&self) -> &str {
        match self {
            Self::Simple(slot) => slot,
            Self::Explicit { slot, .. } => slot,
        }
    }

    /// Return the minimum required state for the dependency.
    ///
    /// # Examples
    /// ```rust
    /// use ob_poc::sem_os_runtime::constellation_runtime::DependencyEntry;
    ///
    /// let dep = DependencyEntry::Simple(String::from("cbu"));
    /// assert_eq!(dep.min_state(), "filled");
    /// ```
    pub fn min_state(&self) -> &str {
        match self {
            Self::Simple(_) => "filled",
            Self::Explicit { min_state, .. } => min_state,
        }
    }
}

/// Verb palette entry in simple or gated form.
#[derive(Debug, Clone, Deserialize, Serialize, PartialEq, Eq)]
#[serde(untagged)]
pub enum VerbPaletteEntry {
    Simple(String),
    Gated {
        verb: String,
        when: VerbAvailability,
    },
}

impl VerbPaletteEntry {
    /// Return the fully qualified verb name.
    ///
    /// # Examples
    /// ```rust
    /// use ob_poc::sem_os_runtime::constellation_runtime::VerbPaletteEntry;
    ///
    /// let entry = VerbPaletteEntry::Simple(String::from("cbu.read"));
    /// assert_eq!(entry.verb_fqn(), "cbu.read");
    /// ```
    pub fn verb_fqn(&self) -> &str {
        match self {
            Self::Simple(verb) => verb,
            Self::Gated { verb, .. } => verb,
        }
    }

    /// Return the reducer states in which the verb is available.
    ///
    /// # Examples
    /// ```rust
    /// use ob_poc::sem_os_runtime::constellation_runtime::{VerbAvailability, VerbPaletteEntry};
    ///
    /// let entry = VerbPaletteEntry::Gated {
    ///     verb: String::from("entity.read"),
    ///     when: VerbAvailability::Many(vec![String::from("filled")]),
    /// };
    /// assert_eq!(entry.available_in(), vec![String::from("filled")]);
    /// ```
    pub fn available_in(&self) -> Vec<String> {
        match self {
            Self::Simple(_) => Vec::new(),
            Self::Gated { when, .. } => when.to_vec(),
        }
    }
}

/// Availability expression for gated verbs.
#[derive(Debug, Clone, Deserialize, Serialize, PartialEq, Eq)]
#[serde(untagged)]
pub enum VerbAvailability {
    One(String),
    Many(Vec<String>),
}

impl VerbAvailability {
    pub(crate) fn to_vec(&self) -> Vec<String> {
        match self {
            Self::One(value) => vec![value.clone()],
            Self::Many(values) => values.clone(),
        }
    }
}

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

/// Flattened slot with path and parent metadata.
#[derive(Debug, Clone)]
pub struct ResolvedSlot {
    pub name: String,
    pub path: String,
    pub def: SlotDef,
    pub depth: usize,
    pub parent: Option<String>,
}

/// Runtime state-machine metadata retained on validated constellation maps.
#[derive(Debug, Clone)]
pub struct RuntimeStateMachine {
    pub name: String,
    pub states: Vec<String>,
    pub initial: String,
    pub transitions: Vec<RuntimeStateTransition>,
    pub overlay_sources: HashMap<String, RuntimeOverlaySource>,
    pub(crate) reducer_backing: crate::sem_os_runtime::reducer_runtime::ReducerMachineBacking,
}

/// Runtime transition metadata needed for Sem OS grounding and diagnostics.
#[derive(Debug, Clone)]
pub struct RuntimeStateTransition {
    pub from: String,
    pub to: String,
    pub verbs: Vec<String>,
}

/// Runtime overlay-source metadata needed for validation and reducer wiring.
#[derive(Debug, Clone)]
pub struct RuntimeOverlaySource {
    pub table: String,
    pub join: String,
    pub provides: Vec<String>,
    pub cardinality: Option<String>,
}

/// Runtime block reason surfaced from grounded or reducer-backed legality.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct RuntimeBlockReason {
    pub message: String,
}

/// Runtime blocked-verb payload stored on hydrated slots.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct RuntimeBlockedVerb {
    pub verb: String,
    pub reasons: Vec<RuntimeBlockReason>,
}

/// Runtime reducer result retained in raw hydration traces.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct RuntimeSlotReduceResult {
    pub slot_path: String,
    pub computed_state: String,
    pub effective_state: String,
    pub available_verbs: Vec<String>,
    pub blocked_verbs: Vec<RuntimeBlockedVerb>,
    pub consistency_warnings: Vec<String>,
    pub reducer_revision: String,
}

/// Validated constellation map with flattened slot index.
#[derive(Debug, Clone)]
pub struct ValidatedConstellationMap {
    pub constellation: String,
    pub description: Option<String>,
    pub jurisdiction: String,
    pub slots_ordered: Vec<ResolvedSlot>,
    pub slot_index: HashMap<String, ResolvedSlot>,
    pub state_machines: HashMap<String, RuntimeStateMachine>,
    pub map_revision: String,
    pub bulk_macros: Vec<String>,
}

/// Raw hydration bundle collected before normalization.
#[derive(Debug, Clone, Default, Serialize, Deserialize)]
pub struct RawHydrationData {
    pub root: Option<Uuid>,
    pub slot_rows: HashMap<String, Vec<RawSlotRow>>,
    pub overlay_rows: HashMap<String, Vec<RawOverlayRow>>,
    pub overrides: HashMap<String, serde_json::Value>,
    pub entity_details: HashMap<Uuid, serde_json::Value>,
    pub graph_edges: HashMap<String, Vec<RawGraphEdge>>,
    pub warnings: HashMap<String, Vec<String>>,
    pub reducer_results: HashMap<String, RuntimeSlotReduceResult>,
}

/// Raw slot row before singular/graph normalization.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct RawSlotRow {
    pub entity_id: Option<Uuid>,
    pub record_id: Option<Uuid>,
    pub filter_value: Option<String>,
    pub created_at: Option<DateTime<Utc>>,
}

/// Raw overlay row before reducer binding.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct RawOverlayRow {
    pub entity_id: Option<Uuid>,
    pub source_name: String,
    pub fields: serde_json::Value,
}

/// Raw graph edge for recursive slots.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct RawGraphEdge {
    pub from_entity_id: Uuid,
    pub to_entity_id: Uuid,
    pub percentage: Option<f64>,
    pub ownership_type: Option<String>,
    pub depth: usize,
}

/// Reducer-relevant slot context discovered from a constellation map.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ConstellationSlotContext {
    pub slot_path: String,
    pub entity_id: Uuid,
    pub slot_type: String,
    pub cardinality: String,
}

/// Hydrated constellation payload returned by the runtime.
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

/// Hydrated slot-type classification exposed by the public runtime.
#[derive(Debug, Clone, Copy, Serialize, Deserialize, PartialEq, Eq)]
#[serde(rename_all = "snake_case")]
pub enum HydratedSlotType {
    Cbu,
    Entity,
    EntityGraph,
    Case,
    Tollgate,
    Mandate,
}

/// Hydrated cardinality classification exposed by the public runtime.
#[derive(Debug, Clone, Copy, Serialize, Deserialize, PartialEq, Eq)]
#[serde(rename_all = "snake_case")]
pub enum HydratedCardinality {
    Root,
    Mandatory,
    Optional,
    Recursive,
}

/// Hydrated slot in normalized tree form.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct HydratedSlot {
    pub name: String,
    pub path: String,
    pub slot_type: HydratedSlotType,
    pub cardinality: HydratedCardinality,
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
    pub blocked_verbs: Vec<RuntimeBlockedVerb>,
    pub children: Vec<HydratedSlot>,
}

/// Batch hydration plan grouped by slot depth.
#[derive(Debug, Clone)]
pub struct HydrationQueryPlan {
    pub levels: Vec<QueryLevel>,
}

/// Queries to execute for a specific depth.
#[derive(Debug, Clone)]
pub struct QueryLevel {
    pub depth: usize,
    pub queries: Vec<SlotQuery>,
}

/// Single compiled query for a slot.
#[derive(Debug, Clone)]
pub struct SlotQuery {
    pub slot_name: String,
    pub query_type: QueryType,
    pub sql: String,
}

/// Query shape used to hydrate a slot.
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum QueryType {
    FetchOne,
    JoinLookup,
    BatchFetch,
    RecursiveCte { max_depth: usize },
    OverlayBatch,
}

/// Load and validate a constellation map from YAML.
///
/// # Examples
/// ```rust
/// use ob_poc::sem_os_runtime::constellation_runtime::load_constellation_map;
///
/// let yaml = r#"
/// constellation: demo
/// jurisdiction: LU
/// slots:
///   cbu:
///     type: cbu
///     table: cbus
///     pk: cbu_id
///     cardinality: root
/// "#;
/// let map = load_constellation_map(yaml).unwrap();
/// assert_eq!(map.constellation, "demo");
/// ```
pub fn load_constellation_map(yaml: &str) -> ConstellationResult<ValidatedConstellationMap> {
    let definition: ConstellationMapDef =
        serde_yaml::from_str(yaml).map_err(|err| ConstellationError::Other(err.into()))?;
    let mut validated = validate_constellation_map(&definition)?;
    validated.map_revision = compute_map_revision(yaml);
    Ok(validated)
}

/// Compute the stable map revision for a YAML definition.
///
/// # Examples
/// ```rust
/// use ob_poc::sem_os_runtime::constellation_runtime::compute_map_revision;
///
/// assert_eq!(compute_map_revision("demo").len(), 16);
/// ```
pub fn compute_map_revision(yaml: &str) -> String {
    let hash = Sha256::digest(yaml.as_bytes());
    hex::encode(&hash[..8])
}

/// Load one of the baked-in constellation maps from Sem OS seed config.
///
/// # Examples
/// ```rust
/// use ob_poc::sem_os_runtime::constellation_runtime::load_builtin_constellation_map;
///
/// let map = load_builtin_constellation_map("struct.lux.ucits.sicav").unwrap();
/// assert_eq!(map.constellation, "struct.lux.ucits.sicav");
/// ```
pub fn load_builtin_constellation_map(
    name: &str,
) -> ConstellationResult<ValidatedConstellationMap> {
    if !name
        .chars()
        .all(|ch| ch.is_ascii_alphanumeric() || ch == '.' || ch == '_' || ch == '-')
    {
        return Err(ConstellationError::Validation(format!(
            "invalid built-in constellation map '{}'",
            name
        )));
    }

    let filename = format!("{}.yaml", name.replace(['.', '-'], "_"));
    let path = PathBuf::from(env!("CARGO_MANIFEST_DIR"))
        .join("config/sem_os_seeds/constellation_maps")
        .join(filename);
    let yaml = std::fs::read_to_string(&path).map_err(|_| {
        ConstellationError::Validation(format!("unknown built-in constellation map '{}'", name))
    })?;
    load_constellation_map(&yaml)
}

/// Validate and flatten a parsed constellation map definition.
///
/// # Examples
/// ```rust
/// use ob_poc::sem_os_runtime::constellation_runtime::{validate_constellation_map, ConstellationMapDef};
///
/// let yaml = r#"
/// constellation: demo
/// jurisdiction: LU
/// slots:
///   cbu:
///     type: cbu
///     table: cbus
///     pk: cbu_id
///     cardinality: root
/// "#;
/// let definition: ConstellationMapDef = serde_yaml::from_str(yaml).unwrap();
/// let validated = validate_constellation_map(&definition).unwrap();
/// assert_eq!(validated.constellation, "demo");
/// ```
pub fn validate_constellation_map(
    definition: &ConstellationMapDef,
) -> ConstellationResult<ValidatedConstellationMap> {
    let mut flattened = Vec::new();
    flatten_validated_slots(&definition.slots, &mut flattened, None, Vec::new(), 0);
    let known_verbs = load_known_verbs_for_validation()?;
    validate_bulk_macros(definition)?;

    let root_count = flattened
        .iter()
        .filter(|slot| slot.def.cardinality == Cardinality::Root)
        .count();
    if root_count != 1 {
        return Err(ConstellationError::Validation(
            "constellation map must define exactly one root slot".into(),
        ));
    }

    let root = flattened
        .iter()
        .find(|slot| slot.def.cardinality == Cardinality::Root)
        .ok_or_else(|| ConstellationError::Validation("missing root slot".into()))?;
    if root.def.slot_type != SlotType::Cbu {
        return Err(ConstellationError::Validation(
            "root slot must have type 'cbu'".into(),
        ));
    }

    let names: HashSet<_> = flattened.iter().map(|slot| slot.name.clone()).collect();
    let mut state_machines = HashMap::new();
    for slot in &flattened {
        validate_flattened_slot(slot, &names, &known_verbs)?;
        if let Some(machine_name) = &slot.def.state_machine {
            state_machines.entry(machine_name.clone()).or_insert(
                load_runtime_state_machine(machine_name).map_err(ConstellationError::Other)?,
            );
        }
    }

    let ordered_names = topo_sort_slots(&flattened)?;
    let slot_index = flattened
        .iter()
        .cloned()
        .map(|slot| (slot.name.clone(), slot))
        .collect::<HashMap<_, _>>();
    let slots_ordered = ordered_names
        .iter()
        .filter_map(|name| slot_index.get(name).cloned())
        .collect::<Vec<_>>();

    Ok(ValidatedConstellationMap {
        constellation: definition.constellation.clone(),
        description: definition.description.clone(),
        jurisdiction: definition.jurisdiction.clone(),
        slots_ordered,
        slot_index,
        state_machines,
        map_revision: String::new(),
        bulk_macros: definition.bulk_macros.clone(),
    })
}

/// Compile a coarse query plan for a validated map.
///
/// # Examples
/// ```rust
/// use ob_poc::sem_os_runtime::constellation_runtime::{compile_query_plan, load_builtin_constellation_map};
///
/// let map = load_builtin_constellation_map("struct.lux.ucits.sicav").unwrap();
/// let plan = compile_query_plan(&map);
/// assert!(!plan.levels.is_empty());
/// ```
pub fn compile_query_plan(map: &ValidatedConstellationMap) -> HydrationQueryPlan {
    let mut levels = BTreeMap::<usize, Vec<SlotQuery>>::new();
    let role_batch = compile_role_batch_query(map);

    for slot in &map.slots_ordered {
        let sql = if let Some(sql) = role_batch.get(&slot.name) {
            sql.clone()
        } else {
            compile_slot_sql(slot)
        };
        levels.entry(slot.depth).or_default().push(SlotQuery {
            slot_name: slot.name.clone(),
            query_type: query_type_for_slot(slot),
            sql,
        });
        if !slot.def.overlays.is_empty() || !slot.def.edge_overlays.is_empty() {
            levels.entry(slot.depth).or_default().push(SlotQuery {
                slot_name: format!("{}.overlays", slot.name),
                query_type: QueryType::OverlayBatch,
                sql: compile_overlay_sql(slot),
            });
        }
    }

    HydrationQueryPlan {
        levels: levels
            .into_iter()
            .map(|(depth, queries)| QueryLevel { depth, queries })
            .collect(),
    }
}

/// Normalize raw hydration data into a tree of hydrated slots.
///
/// # Examples
/// ```rust
/// use ob_poc::sem_os_runtime::constellation_runtime::{load_builtin_constellation_map, normalize_slots, RawHydrationData};
/// use uuid::Uuid;
///
/// let map = load_builtin_constellation_map("struct.lux.ucits.sicav").unwrap();
/// let normalized = normalize_slots(&map, Uuid::nil(), None, RawHydrationData::default());
/// assert_eq!(normalized.constellation, "struct.lux.ucits.sicav");
/// ```
pub fn normalize_slots(
    map: &ValidatedConstellationMap,
    cbu_id: Uuid,
    case_id: Option<Uuid>,
    raw: RawHydrationData,
) -> HydratedConstellation {
    crate::sem_os_runtime::normalize_impl::normalize_slots(map, cbu_id, case_id, raw)
}

/// Compute a compact summary for a hydrated constellation.
///
/// # Examples
/// ```rust
/// use ob_poc::sem_os_runtime::constellation_runtime::{compute_summary, load_builtin_constellation_map, normalize_slots, RawHydrationData};
/// use uuid::Uuid;
///
/// let map = load_builtin_constellation_map("struct.lux.ucits.sicav").unwrap();
/// let hydrated = normalize_slots(&map, Uuid::nil(), None, RawHydrationData::default());
/// let summary = compute_summary(&hydrated);
/// assert!(summary.total_slots >= 1);
/// ```
pub fn compute_summary(hydrated: &HydratedConstellation) -> ConstellationSummary {
    let slots = flatten_slots(&hydrated.slots);
    let total_slots = slots.len();
    let slots_filled = slots
        .iter()
        .filter(|slot| slot.effective_state != "empty")
        .count();
    let slots_empty_mandatory = slots
        .iter()
        .filter(|slot| {
            slot.cardinality == HydratedCardinality::Mandatory && slot.effective_state == "empty"
        })
        .count();
    let slots_empty_optional = slots
        .iter()
        .filter(|slot| {
            slot.cardinality == HydratedCardinality::Optional && slot.effective_state == "empty"
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
    let slots_blocked = slots
        .iter()
        .filter(|slot| !slot.blocked_verbs.is_empty())
        .count();
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
        .find(|slot| slot.slot_type == HydratedSlotType::EntityGraph)
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

/// Compute action-surface verbs and progress for a hydrated constellation.
///
/// # Examples
/// ```rust
/// use ob_poc::sem_os_runtime::constellation_runtime::{compute_action_surface, load_builtin_constellation_map, normalize_slots, RawHydrationData};
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
    let states = flatten_states_for_action_surface(&hydrated.slots);
    let core_model = build_core_grounding_model(map);
    for slot in &mut hydrated.slots {
        decorate_slot_for_action_surface(map, &core_model, &states, slot);
    }
    hydrated
}

/// Hydrate a constellation for a CBU and optional case.
///
/// # Examples
/// ```rust,no_run
/// # async fn demo(pool: &sqlx::PgPool) -> anyhow::Result<()> {
/// use uuid::Uuid;
/// use ob_poc::sem_os_runtime::constellation_runtime::handle_constellation_hydrate;
///
/// let _ = handle_constellation_hydrate(
///     pool,
///     Uuid::new_v4(),
///     None,
///     "struct.lux.ucits.sicav",
/// )
/// .await?;
/// # Ok(())
/// # }
/// ```
pub async fn handle_constellation_hydrate(
    pool: &PgPool,
    cbu_id: Uuid,
    case_id: Option<Uuid>,
    map_name: &str,
) -> Result<HydratedConstellation> {
    let map =
        load_builtin_constellation_map(map_name).map_err(|err| anyhow::anyhow!(err.to_string()))?;
    hydrate_constellation(pool, cbu_id, case_id, &map)
        .await
        .map_err(|err| anyhow::anyhow!(err.to_string()))
}

/// Compute a compact summary for a hydrated constellation.
///
/// # Examples
/// ```rust,no_run
/// # async fn demo(pool: &sqlx::PgPool) -> anyhow::Result<()> {
/// use uuid::Uuid;
/// use ob_poc::sem_os_runtime::constellation_runtime::handle_constellation_summary;
///
/// let _ = handle_constellation_summary(
///     pool,
///     Uuid::new_v4(),
///     None,
///     "struct.lux.ucits.sicav",
/// )
/// .await?;
/// # Ok(())
/// # }
/// ```
pub async fn handle_constellation_summary(
    pool: &PgPool,
    cbu_id: Uuid,
    case_id: Option<Uuid>,
    map_name: &str,
) -> Result<ConstellationSummary> {
    let map =
        load_builtin_constellation_map(map_name).map_err(|err| anyhow::anyhow!(err.to_string()))?;
    hydrate_constellation_summary(pool, cbu_id, case_id, &map)
        .await
        .map_err(|err| anyhow::anyhow!(err.to_string()))
}

/// Hydrate a constellation using a validated Sem OS runtime map.
///
/// # Examples
/// ```rust,no_run
/// # async fn demo(pool: &sqlx::PgPool) -> anyhow::Result<()> {
/// use uuid::Uuid;
/// use ob_poc::sem_os_runtime::constellation_runtime::{hydrate_constellation, load_builtin_constellation_map};
///
/// let map = load_builtin_constellation_map("struct.lux.ucits.sicav").unwrap();
/// let _ = hydrate_constellation(pool, Uuid::new_v4(), None, &map).await?;
/// # Ok(())
/// # }
/// ```
pub async fn hydrate_constellation(
    pool: &PgPool,
    cbu_id: Uuid,
    case_id: Option<Uuid>,
    map: &ValidatedConstellationMap,
) -> ConstellationResult<HydratedConstellation> {
    crate::sem_os_runtime::hydration_impl::hydrate_constellation(pool, cbu_id, case_id, map).await
}

/// Compute just the high-level summary for a constellation.
///
/// # Examples
/// ```rust,no_run
/// # async fn demo(pool: &sqlx::PgPool) -> anyhow::Result<()> {
/// use uuid::Uuid;
/// use ob_poc::sem_os_runtime::constellation_runtime::{hydrate_constellation_summary, load_builtin_constellation_map};
///
/// let map = load_builtin_constellation_map("struct.lux.ucits.sicav").unwrap();
/// let _ = hydrate_constellation_summary(pool, Uuid::new_v4(), None, &map).await?;
/// # Ok(())
/// # }
/// ```
pub async fn hydrate_constellation_summary(
    pool: &PgPool,
    cbu_id: Uuid,
    case_id: Option<Uuid>,
    map: &ValidatedConstellationMap,
) -> ConstellationResult<ConstellationSummary> {
    crate::sem_os_runtime::hydration_impl::hydrate_constellation_summary(pool, cbu_id, case_id, map)
        .await
}

/// Discover slot contexts in a constellation that use the given reducer machine.
///
/// # Examples
/// ```rust,no_run
/// # async fn demo(pool: &sqlx::PgPool) -> anyhow::Result<()> {
/// use uuid::Uuid;
/// use ob_poc::sem_os_runtime::constellation_runtime::{discover_state_machine_slot_contexts, load_builtin_constellation_map};
///
/// let map = load_builtin_constellation_map("struct.lux.ucits.sicav").unwrap();
/// let _ = discover_state_machine_slot_contexts(
///     pool,
///     Uuid::new_v4(),
///     None,
///     &map,
///     "entity_kyc_lifecycle",
/// ).await?;
/// # Ok(())
/// # }
/// ```
pub async fn discover_state_machine_slot_contexts(
    pool: &PgPool,
    cbu_id: Uuid,
    case_id: Option<Uuid>,
    map: &ValidatedConstellationMap,
    state_machine_name: &str,
) -> ConstellationResult<Vec<ConstellationSlotContext>> {
    crate::sem_os_runtime::hydration_impl::discover_state_machine_slot_contexts(
        pool,
        cbu_id,
        case_id,
        map,
        state_machine_name,
    )
    .await
    .map(|contexts| contexts.into_iter().collect())
}

fn flatten_slots(slots: &[HydratedSlot]) -> Vec<&HydratedSlot> {
    let mut out = Vec::new();
    for slot in slots {
        out.push(slot);
        out.extend(flatten_slots(&slot.children));
    }
    out
}

fn flatten_validated_slots(
    slots: &BTreeMap<String, SlotDef>,
    out: &mut Vec<ResolvedSlot>,
    parent: Option<String>,
    prefix: Vec<String>,
    depth: usize,
) {
    for (name, def) in slots {
        let mut path = prefix.clone();
        path.push(name.clone());
        out.push(ResolvedSlot {
            name: path.join("."),
            path: path.join("."),
            def: def.clone(),
            depth,
            parent: parent.clone(),
        });
        if !def.children.is_empty() {
            flatten_validated_slots(&def.children, out, Some(path.join(".")), path, depth + 1);
        }
    }
}

/// Known join tables from the schema. Used to validate constellation map
/// `join.via` names at load time. Prevents silent empty-slot hydration
/// when a YAML declares a table name that doesn't exist.
///
/// This set is maintained manually — add entries when new join tables
/// are created via migration.
fn known_join_tables() -> HashSet<&'static str> {
    [
        "access_reviews",
        "bods_entity_statements",
        "bods_ownership_statements",
        "bods_person_statements",
        "booking_locations",
        "booking_principals",
        "capital_events",
        "cases",
        "cash_sweep_configs",
        "cbu_custody_profiles",
        "cbu_entity_roles",
        "cbu_structure_links",
        "cbu_trading_profiles",
        "client_group_entity",
        "contract_packs",
        "deal_contracts",
        "deal_onboarding_requests",
        "deal_participants",
        "deal_products",
        "deal_rate_cards",
        "deal_slas",
        "deals",
        "delegations",
        "delivery_channels",
        "doc_requests",
        "entity_funds",
        "entity_identifiers",
        "entity_relationships",
        "entity_workstreams",
        "fee_billing_profiles",
        "fund_feeder_links",
        "fund_investments",
        "fund_share_classes",
        "kyc_service_agreements",
        "legal_contracts",
        "legal_entities",
        "manco_groups",
        "ownership_snapshots",
        "partnership_structures",
        "pricing_configs",
        "product_services",
        "products",
        "regulatory_filings",
        "rule_fields",
        "rules",
        "rulesets",
        "screenings",
        "service_intents",
        "service_resource_capabilities",
        "service_resources",
        "team_members",
        "tollgate_evaluations",
        "trust_structures",
        "ubo_registry",
    ]
    .into_iter()
    .collect()
}

fn validate_flattened_slot(
    slot: &ResolvedSlot,
    names: &HashSet<String>,
    known_verbs: &HashSet<String>,
) -> ConstellationResult<()> {
    if slot.def.cardinality != Cardinality::Root
        && slot.def.slot_type != SlotType::Cbu
        && slot.def.join.is_none()
    {
        return Err(ConstellationError::Validation(format!(
            "slot '{}' requires a join definition",
            slot.name
        )));
    }

    // Validate join.via against known schema tables
    if let Some(ref join) = slot.def.join {
        let known = known_join_tables();
        if !known.contains(join.via.as_str()) {
            tracing::warn!(
                slot = %slot.name,
                via = %join.via,
                "Constellation slot declares join via unknown table — hydration will return empty"
            );
        }
    }
    if slot.def.cardinality == Cardinality::Recursive && slot.def.max_depth.is_none() {
        return Err(ConstellationError::Validation(format!(
            "recursive slot '{}' requires max_depth",
            slot.name
        )));
    }
    if slot.def.cardinality == Cardinality::Recursive && slot.def.slot_type != SlotType::EntityGraph
    {
        return Err(ConstellationError::Validation(format!(
            "recursive slot '{}' must have type entity_graph",
            slot.name
        )));
    }
    for dep in &slot.def.depends_on {
        if !names.contains(dep.slot_name()) {
            return Err(ConstellationError::Validation(format!(
                "slot '{}' depends on unknown slot '{}'",
                slot.name,
                dep.slot_name()
            )));
        }
    }
    for entry in slot.def.verbs.values() {
        let verb = entry.verb_fqn();
        if !known_verbs.contains(verb) {
            return Err(ConstellationError::Validation(format!(
                "slot '{}' references unknown verb '{}'",
                slot.name, verb
            )));
        }
    }
    if let Some(machine_name) = &slot.def.state_machine {
        let machine =
            load_runtime_state_machine(machine_name).map_err(ConstellationError::Other)?;
        for overlay in &slot.def.overlays {
            if !machine.overlay_sources.contains_key(overlay) {
                return Err(ConstellationError::Validation(format!(
                    "slot '{}' references unknown overlay '{}' for state machine '{}'",
                    slot.name, overlay, machine_name
                )));
            }
        }
    }
    Ok(())
}

fn validate_bulk_macros(definition: &ConstellationMapDef) -> ConstellationResult<()> {
    for bulk_macro in &definition.bulk_macros {
        if !SUPPORTED_BULK_MACROS.contains(&bulk_macro.as_str()) {
            return Err(ConstellationError::Validation(format!(
                "constellation '{}' references unsupported bulk macro '{}'",
                definition.constellation, bulk_macro
            )));
        }
    }
    Ok(())
}

fn load_known_verbs_for_validation() -> ConstellationResult<HashSet<String>> {
    let mut known = HashSet::new();
    for verb in crate::dsl_v2::runtime_registry::runtime_registry().all_verbs() {
        known.insert(verb.full_name.clone());
    }
    load_known_macro_verbs_for_validation(
        &PathBuf::from(env!("CARGO_MANIFEST_DIR")).join("config/verb_schemas/macros"),
        &mut known,
    )?;
    Ok(known)
}

fn load_known_macro_verbs_for_validation(
    path: &std::path::Path,
    known: &mut HashSet<String>,
) -> ConstellationResult<()> {
    for entry in std::fs::read_dir(path).map_err(|err| ConstellationError::Other(err.into()))? {
        let entry = entry.map_err(|err| ConstellationError::Other(err.into()))?;
        let path = entry.path();
        if path.is_dir() {
            load_known_macro_verbs_for_validation(&path, known)?;
            continue;
        }
        if path.extension().and_then(|ext| ext.to_str()) != Some("yaml") {
            continue;
        }
        let contents =
            std::fs::read_to_string(&path).map_err(|err| ConstellationError::Other(err.into()))?;
        let value: serde_yaml::Value =
            serde_yaml::from_str(&contents).map_err(|err| ConstellationError::Other(err.into()))?;
        let Some(mapping) = value.as_mapping() else {
            continue;
        };
        for (key, _) in mapping {
            if let Some(verb_fqn) = key.as_str() {
                known.insert(verb_fqn.to_string());
            }
        }
    }
    Ok(())
}

fn topo_sort_slots(slots: &[ResolvedSlot]) -> ConstellationResult<Vec<String>> {
    let deps = slots
        .iter()
        .map(|slot| {
            let mut values = slot
                .def
                .depends_on
                .iter()
                .map(|dep| dep.slot_name().to_string())
                .collect::<Vec<_>>();
            if let Some(parent) = &slot.parent {
                values.push(parent.clone());
            }
            (slot.name.clone(), values)
        })
        .collect::<HashMap<_, _>>();

    let mut ordered = Vec::new();
    let mut visiting = HashSet::new();
    let mut visited = HashSet::new();
    for slot in slots {
        visit_slot_dependency(&slot.name, &deps, &mut visiting, &mut visited, &mut ordered)?;
    }
    Ok(ordered)
}

fn visit_slot_dependency(
    name: &str,
    deps: &HashMap<String, Vec<String>>,
    visiting: &mut HashSet<String>,
    visited: &mut HashSet<String>,
    ordered: &mut Vec<String>,
) -> ConstellationResult<()> {
    if visited.contains(name) {
        return Ok(());
    }
    if !visiting.insert(name.to_string()) {
        return Err(ConstellationError::Validation(format!(
            "cyclic slot dependency detected at '{}'",
            name
        )));
    }
    if let Some(children) = deps.get(name) {
        for dep in children {
            visit_slot_dependency(dep, deps, visiting, visited, ordered)?;
        }
    }
    visiting.remove(name);
    visited.insert(name.to_string());
    ordered.push(name.to_string());
    Ok(())
}

fn flatten_states_for_action_surface(slots: &[HydratedSlot]) -> HashMap<String, String> {
    let mut out = HashMap::new();
    for slot in slots {
        out.insert(slot.name.clone(), slot.effective_state.clone());
        out.extend(flatten_states_for_action_surface(&slot.children));
    }
    out
}

fn decorate_slot_for_action_surface(
    map: &ValidatedConstellationMap,
    core_model: &ConstellationModel,
    states: &HashMap<String, String>,
    slot: &mut HydratedSlot,
) {
    if let Some(resolved) = map.slot_index.get(&slot.name) {
        slot.progress =
            compute_progress_for_action_surface(slot.cardinality, &slot.effective_state);
        slot.blocking = slot.cardinality == HydratedCardinality::Mandatory && slot.progress < 100;
        if let Ok(surface) = compute_slot_action_surface(core_model, states, &slot.name) {
            let mut seen_available = slot
                .available_verbs
                .iter()
                .cloned()
                .collect::<std::collections::HashSet<_>>();
            for valid in surface.valid_actions {
                if seen_available.insert(valid.action_id.clone()) {
                    slot.available_verbs.push(valid.action_id);
                }
            }
            slot.blocked_verbs = surface
                .blocked_actions
                .into_iter()
                .map(|blocked| RuntimeBlockedVerb {
                    verb: blocked.action_id,
                    reasons: blocked
                        .reasons
                        .into_iter()
                        .map(|message| RuntimeBlockReason { message })
                        .collect(),
                })
                .collect();
        } else {
            let mut seen_available = slot
                .available_verbs
                .iter()
                .cloned()
                .collect::<std::collections::HashSet<_>>();
            for entry in resolved.def.verbs.values() {
                let verb = entry.verb_fqn().to_string();
                if seen_available.insert(verb.clone()) {
                    slot.available_verbs.push(verb);
                }
            }
        }
    }
    for child in &mut slot.children {
        decorate_slot_for_action_surface(map, core_model, states, child);
    }
}

fn compute_progress_for_action_surface(cardinality: HydratedCardinality, state: &str) -> u8 {
    if cardinality == HydratedCardinality::Optional && state == "empty" {
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

fn build_core_grounding_model(map: &ValidatedConstellationMap) -> ConstellationModel {
    let authored_map = core_map::ConstellationMapDefBody {
        fqn: map.constellation.clone(),
        constellation: map.constellation.clone(),
        description: map.description.clone(),
        jurisdiction: map.jurisdiction.clone(),
        slots: rebuild_authored_slot_tree(map),
        bulk_macros: map.bulk_macros.clone(),
    };
    let state_machines = map
        .state_machines
        .iter()
        .map(|(name, machine)| {
            (
                name.clone(),
                core_sm::StateMachineDefBody {
                    fqn: name.clone(),
                    state_machine: name.clone(),
                    description: None,
                    states: machine.states.clone(),
                    initial: machine.initial.clone(),
                    transitions: machine
                        .transitions
                        .iter()
                        .map(|transition| core_sm::TransitionDef {
                            from: transition.from.clone(),
                            to: transition.to.clone(),
                            verbs: transition.verbs.clone(),
                        })
                        .collect(),
                    reducer: None,
                },
            )
        })
        .collect();
    ConstellationModel::from_parts(authored_map, state_machines)
}

fn rebuild_authored_slot_tree(
    map: &ValidatedConstellationMap,
) -> BTreeMap<String, core_map::SlotDef> {
    fn build_children(
        map: &ValidatedConstellationMap,
        parent: Option<&str>,
        prefix: &[&str],
    ) -> BTreeMap<String, core_map::SlotDef> {
        map.slots_ordered
            .iter()
            .filter(|slot| slot.parent.as_deref() == parent)
            .filter_map(|slot| {
                let segments = slot.path.split('.').collect::<Vec<_>>();
                if segments.len() != prefix.len() + 1 || !segments.starts_with(prefix) {
                    return None;
                }
                let child_name = segments.last()?.to_string();
                Some((
                    child_name.clone(),
                    core_slot_def_from_public(
                        &slot.def,
                        build_children(map, Some(&slot.path), &segments),
                    ),
                ))
            })
            .collect()
    }

    build_children(map, None, &[])
}

impl From<&JoinDef> for core_map::JoinDef {
    fn from(value: &JoinDef) -> Self {
        Self {
            via: value.via.clone(),
            parent_fk: value.parent_fk.clone(),
            child_fk: value.child_fk.clone(),
            filter_column: value.filter_column.clone(),
            filter_value: value.filter_value.clone(),
        }
    }
}

impl From<&DependencyEntry> for core_map::DependencyEntry {
    fn from(value: &DependencyEntry) -> Self {
        match value {
            DependencyEntry::Simple(slot) => Self::Simple(slot.clone()),
            DependencyEntry::Explicit { slot, min_state } => Self::Explicit {
                slot: slot.clone(),
                min_state: min_state.clone(),
            },
        }
    }
}

impl From<&VerbAvailability> for core_map::VerbAvailability {
    fn from(value: &VerbAvailability) -> Self {
        match value {
            VerbAvailability::One(value) => Self::One(value.clone()),
            VerbAvailability::Many(values) => Self::Many(values.clone()),
        }
    }
}

impl From<&VerbPaletteEntry> for core_map::VerbPaletteEntry {
    fn from(value: &VerbPaletteEntry) -> Self {
        match value {
            VerbPaletteEntry::Simple(verb) => Self::Simple(verb.clone()),
            VerbPaletteEntry::Gated { verb, when } => Self::Gated {
                verb: verb.clone(),
                when: core_map::VerbAvailability::from(when),
            },
        }
    }
}

fn core_slot_def_from_public(
    definition: &SlotDef,
    children: BTreeMap<String, core_map::SlotDef>,
) -> core_map::SlotDef {
    core_map::SlotDef {
        slot_type: match definition.slot_type {
            SlotType::Cbu => core_map::SlotType::Cbu,
            SlotType::Entity => core_map::SlotType::Entity,
            SlotType::EntityGraph => core_map::SlotType::EntityGraph,
            SlotType::Case => core_map::SlotType::Case,
            SlotType::Tollgate => core_map::SlotType::Tollgate,
            SlotType::Mandate => core_map::SlotType::Mandate,
        },
        entity_kinds: definition.entity_kinds.clone(),
        table: definition.table.clone(),
        pk: definition.pk.clone(),
        join: definition.join.as_ref().map(core_map::JoinDef::from),
        occurrence: definition.occurrence,
        cardinality: match definition.cardinality {
            Cardinality::Root => core_map::Cardinality::Root,
            Cardinality::Mandatory => core_map::Cardinality::Mandatory,
            Cardinality::Optional => core_map::Cardinality::Optional,
            Cardinality::Recursive => core_map::Cardinality::Recursive,
        },
        depends_on: definition
            .depends_on
            .iter()
            .map(core_map::DependencyEntry::from)
            .collect(),
        placeholder: definition.placeholder.clone(),
        state_machine: definition.state_machine.clone(),
        overlays: definition.overlays.clone(),
        edge_overlays: definition.edge_overlays.clone(),
        verbs: definition
            .verbs
            .iter()
            .map(|(key, entry)| (key.clone(), core_map::VerbPaletteEntry::from(entry)))
            .collect(),
        children,
        max_depth: definition.max_depth,
    }
}

fn compile_slot_sql(slot: &ResolvedSlot) -> String {
    match slot.def.cardinality {
        Cardinality::Root => format!(
            "SELECT {pk} FROM {table} WHERE {pk} = $1",
            pk = slot.def.pk.as_deref().unwrap_or("id"),
            table = slot.def.table.as_deref().unwrap_or("unknown")
        ),
        Cardinality::Recursive => format!(
            "WITH RECURSIVE slot_graph AS (...) SELECT * FROM {table} /* max_depth={depth} */",
            table = slot
                .def
                .join
                .as_ref()
                .map(|join| join.via.as_str())
                .unwrap_or("unknown"),
            depth = slot.def.max_depth.unwrap_or(0)
        ),
        Cardinality::Mandatory | Cardinality::Optional => format!(
            "SELECT * FROM {table} /* via {via} */",
            table = slot
                .def
                .table
                .as_deref()
                .or_else(|| slot.def.join.as_ref().map(|join| join.via.as_str()))
                .unwrap_or("unknown"),
            via = slot
                .def
                .join
                .as_ref()
                .map(|join| join.via.as_str())
                .unwrap_or("none")
        ),
    }
}

fn query_type_for_slot(slot: &ResolvedSlot) -> QueryType {
    match slot.def.cardinality {
        Cardinality::Root => QueryType::FetchOne,
        Cardinality::Recursive => QueryType::RecursiveCte {
            max_depth: slot.def.max_depth.unwrap_or(1),
        },
        Cardinality::Mandatory | Cardinality::Optional => {
            if slot.def.slot_type == SlotType::Entity && slot.def.join.is_some() {
                QueryType::BatchFetch
            } else {
                QueryType::JoinLookup
            }
        }
    }
}

fn compile_overlay_sql(slot: &ResolvedSlot) -> String {
    let mut sources = slot.def.overlays.clone();
    if !slot.def.edge_overlays.is_empty() {
        sources.extend(
            slot.def
                .edge_overlays
                .iter()
                .map(|source| format!("edge:{source}")),
        );
    }
    format!(
        "/* overlay batch for slot {} */ SELECT {}",
        slot.name,
        sources.join(", ")
    )
}

fn compile_role_batch_query(map: &ValidatedConstellationMap) -> BTreeMap<String, String> {
    let role_slots = map
        .slots_ordered
        .iter()
        .filter(|slot| {
            slot.def.slot_type == SlotType::Entity
                && slot
                    .def
                    .join
                    .as_ref()
                    .map(|join| join.via == "cbu_entity_roles")
                    .unwrap_or(false)
        })
        .collect::<Vec<_>>();

    if role_slots.len() < 2 {
        return BTreeMap::new();
    }

    let filters = role_slots
        .iter()
        .filter_map(|slot| {
            slot.def.join.as_ref().and_then(|join| {
                join.filter_value
                    .as_ref()
                    .map(|value| format!("'{}' AS {}", value, slot.name.replace('.', "_")))
            })
        })
        .collect::<Vec<_>>()
        .join(", ");

    role_slots
        .into_iter()
        .map(|slot| {
            (
                slot.name.clone(),
                format!(
                    "SELECT cer.cbu_id, cer.entity_id, r.name AS role_name, {} \
                     FROM \"ob-poc\".cbu_entity_roles cer \
                     JOIN \"ob-poc\".roles r ON r.role_id = cer.role_id \
                     WHERE cer.cbu_id = $1",
                    filters
                ),
            )
        })
        .collect()
}

#[cfg(test)]
mod tests {
    use super::*;

    fn parse_map(yaml: &str) -> ConstellationMapDef {
        serde_yaml::from_str(yaml).expect("constellation yaml should parse")
    }

    #[test]
    fn test_validate_constellation_map_rejects_unknown_slot_verb() {
        let definition = parse_map(
            r#"
constellation: demo
jurisdiction: LU
slots:
  cbu:
    type: cbu
    table: cbus
    pk: cbu_id
    cardinality: root
    verbs:
      create: nonexistent.verb
"#,
        );

        let error = validate_constellation_map(&definition).expect_err("should reject dead verb");
        assert!(error
            .to_string()
            .contains("unknown verb 'nonexistent.verb'"));
    }

    #[test]
    fn test_validate_constellation_map_rejects_unknown_bulk_macro() {
        let definition = parse_map(
            r#"
constellation: demo
jurisdiction: LU
slots:
  cbu:
    type: cbu
    table: cbus
    pk: cbu_id
    cardinality: root
bulk_macros: [unknown_bulk]
"#,
        );

        let error =
            validate_constellation_map(&definition).expect_err("should reject dead bulk macro");
        assert!(error
            .to_string()
            .contains("unsupported bulk macro 'unknown_bulk'"));
    }
}
