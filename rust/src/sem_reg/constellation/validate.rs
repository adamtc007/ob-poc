use std::collections::{HashMap, HashSet};
use std::path::PathBuf;

use super::error::{ConstellationError, ConstellationResult};
use super::map_def::{Cardinality, ConstellationMapDef, SlotDef, SlotType};
use crate::sem_reg::reducer::{load_builtin_state_machine, ValidatedStateMachine};

/// Validated constellation map with flattened slot index.
#[derive(Debug, Clone)]
pub struct ValidatedConstellationMap {
    pub constellation: String,
    pub description: Option<String>,
    pub jurisdiction: String,
    pub slots_ordered: Vec<ResolvedSlot>,
    pub slot_index: HashMap<String, ResolvedSlot>,
    pub state_machines: HashMap<String, ValidatedStateMachine>,
    pub map_revision: String,
    pub bulk_macros: Vec<String>,
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

/// Validate and flatten a parsed constellation map definition.
///
/// # Examples
/// ```rust
/// use ob_poc::sem_reg::constellation::{load_constellation_map, validate_constellation_map};
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
/// let validated = validate_constellation_map(&map).unwrap();
/// assert_eq!(validated.constellation, "demo");
/// ```
pub fn validate_constellation_map(
    definition: &ConstellationMapDef,
) -> ConstellationResult<ValidatedConstellationMap> {
    let mut flattened = Vec::new();
    flatten_slots(&definition.slots, &mut flattened, None, Vec::new(), 0);

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
        validate_slot(slot, &names)?;
        if let Some(machine_name) = &slot.def.state_machine {
            state_machines.entry(machine_name.clone()).or_insert(
                load_builtin_state_machine(machine_name)
                    .map_err(|err| ConstellationError::Other(anyhow::anyhow!(err.to_string())))?,
            );
        }
    }

    let ordered_names = topo_sort(&flattened)?;
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

fn flatten_slots(
    slots: &std::collections::BTreeMap<String, SlotDef>,
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
            flatten_slots(&def.children, out, Some(path.join(".")), path, depth + 1);
        }
    }
}

fn validate_slot(slot: &ResolvedSlot, names: &HashSet<String>) -> ConstellationResult<()> {
    if slot.def.cardinality != Cardinality::Root && slot.def.join.is_none() {
        return Err(ConstellationError::Validation(format!(
            "slot '{}' requires a join definition",
            slot.name
        )));
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
    let known_verbs = load_known_verbs()?;
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
        let machine = load_builtin_state_machine(machine_name)
            .map_err(|err| ConstellationError::Other(anyhow::anyhow!(err.to_string())))?;
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

fn load_known_verbs() -> ConstellationResult<HashSet<String>> {
    let root = PathBuf::from(env!("CARGO_MANIFEST_DIR")).join("config/verbs");
    let mut known = HashSet::new();
    load_known_verbs_from_dir(&root, &mut known)?;
    let macros_root = PathBuf::from(env!("CARGO_MANIFEST_DIR")).join("config/verb_schemas/macros");
    load_known_macro_verbs_from_dir(&macros_root, &mut known)?;
    Ok(known)
}

fn load_known_verbs_from_dir(path: &PathBuf, known: &mut HashSet<String>) -> ConstellationResult<()> {
    for entry in std::fs::read_dir(path).map_err(|err| ConstellationError::Other(err.into()))? {
        let entry = entry.map_err(|err| ConstellationError::Other(err.into()))?;
        let path = entry.path();
        if path.is_dir() {
            load_known_verbs_from_dir(&path, known)?;
            continue;
        }
        if path.extension().and_then(|ext| ext.to_str()) != Some("yaml") {
            continue;
        }
        let contents =
            std::fs::read_to_string(&path).map_err(|err| ConstellationError::Other(err.into()))?;
        let value: serde_yaml::Value =
            serde_yaml::from_str(&contents).map_err(|err| ConstellationError::Other(err.into()))?;
        collect_verb_fqns(&value, known);
    }
    Ok(())
}

fn collect_verb_fqns(value: &serde_yaml::Value, known: &mut HashSet<String>) {
    let Some(mapping) = value.as_mapping() else {
        return;
    };
    if let Some(domains) = mapping.get(serde_yaml::Value::String(String::from("domains"))) {
        collect_domain_verbs(domains, known);
        return;
    }
    if let Some(verbs) = mapping.get(serde_yaml::Value::String(String::from("verbs"))) {
        collect_domain_verbs(verbs, known);
    } else {
        collect_domain_verbs(value, known);
    }
}

fn load_known_macro_verbs_from_dir(
    path: &PathBuf,
    known: &mut HashSet<String>,
) -> ConstellationResult<()> {
    for entry in std::fs::read_dir(path).map_err(|err| ConstellationError::Other(err.into()))? {
        let entry = entry.map_err(|err| ConstellationError::Other(err.into()))?;
        let path = entry.path();
        if path.is_dir() {
            load_known_macro_verbs_from_dir(&path, known)?;
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

fn collect_domain_verbs(value: &serde_yaml::Value, known: &mut HashSet<String>) {
    let Some(mapping) = value.as_mapping() else {
        return;
    };
    for (domain_key, domain_value) in mapping {
        let Some(domain) = domain_key.as_str() else {
            continue;
        };
        let Some(domain_map) = domain_value.as_mapping() else {
            continue;
        };
        let source = domain_map
            .get(serde_yaml::Value::String(String::from("verbs")))
            .and_then(serde_yaml::Value::as_mapping)
            .unwrap_or(domain_map);
        for (verb_key, _) in source {
            if let Some(verb) = verb_key.as_str() {
                known.insert(format!("{domain}.{verb}"));
            }
        }
    }
}

fn topo_sort(slots: &[ResolvedSlot]) -> ConstellationResult<Vec<String>> {
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
        visit(&slot.name, &deps, &mut visiting, &mut visited, &mut ordered)?;
    }
    Ok(ordered)
}

fn visit(
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
            visit(dep, deps, visiting, visited, ordered)?;
        }
    }
    visiting.remove(name);
    visited.insert(name.to_string());
    ordered.push(name.to_string());
    Ok(())
}
