use dsl_core::config::{
    dag::{ClosureType, EligibilityConstraint},
    validate_constellation_map_schema_coordination, Dag, LoadedDag,
};
use std::{
    collections::{BTreeMap, HashMap},
    path::PathBuf,
};

fn seed_path(relative: &str) -> PathBuf {
    PathBuf::from(env!("CARGO_MANIFEST_DIR"))
        .join("../../config/sem_os_seeds")
        .join(relative)
}

fn cbu_dag() -> Dag {
    let path = seed_path("dag_taxonomies/cbu_dag.yaml");
    let yaml = std::fs::read_to_string(&path).expect("cbu DAG readable");
    serde_yaml::from_str(&yaml).unwrap_or_else(|err| panic!("{path:?}: {err}"))
}

fn lux_sicav_yaml() -> String {
    let path = seed_path("constellation_maps/struct_lux_ucits_sicav.yaml");
    std::fs::read_to_string(&path).expect("Lux SICAV constellation readable")
}

fn lux_aif_raif_yaml() -> String {
    let path = seed_path("constellation_maps/struct_lux_aif_raif.yaml");
    std::fs::read_to_string(&path).expect("Lux AIF RAIF constellation readable")
}

#[derive(Debug, serde::Deserialize)]
struct RawConstellationMap {
    slots: BTreeMap<String, RawConstellationSlot>,
}

#[derive(Debug, serde::Deserialize)]
struct RawConstellationSlot {
    #[serde(default)]
    closure: Option<ClosureType>,
    #[serde(default)]
    eligibility: Option<EligibilityConstraint>,
    #[serde(default)]
    cardinality_max: Option<u64>,
    #[serde(default)]
    entry_state: Option<String>,
}

fn slot_without_gate_metadata(yaml: &str, slot_id: &str) -> serde_yaml::Value {
    let mut value: serde_yaml::Value = serde_yaml::from_str(yaml).expect("constellation parses");
    let slots = value
        .as_mapping_mut()
        .and_then(|map| map.get_mut(serde_yaml::Value::String("slots".to_string())))
        .and_then(|slots| slots.as_mapping_mut())
        .expect("slots mapping present");
    let slot = slots
        .get_mut(serde_yaml::Value::String(slot_id.to_string()))
        .unwrap_or_else(|| panic!("{slot_id} slot present"))
        .as_mapping_mut()
        .unwrap_or_else(|| panic!("{slot_id} slot is a mapping"));
    for key in ["closure", "eligibility", "cardinality_max", "entry_state"] {
        slot.remove(serde_yaml::Value::String(key.to_string()));
    }
    serde_yaml::Value::Mapping(slot.clone())
}

#[test]
fn cbu_dag_pilot_slots_have_gate_metadata() {
    let dag = cbu_dag();
    let slots = dag
        .slots
        .iter()
        .map(|slot| (slot.id.as_str(), slot))
        .collect::<HashMap<_, _>>();

    let cbu = slots.get("cbu").expect("cbu slot present");
    assert_eq!(cbu.closure, Some(ClosureType::ClosedBounded));
    assert_eq!(cbu.cardinality_max, Some(1));
    assert_eq!(cbu.entry_state.as_deref(), Some("DISCOVERED"));
    assert_eq!(
        cbu.eligibility,
        Some(EligibilityConstraint::EntityKinds {
            entity_kinds: vec!["cbu".to_string()]
        })
    );

    for (slot_id, expected_kind, expected_entry_state) in [
        ("entity_proper_person", Some("proper_person"), "GHOST"),
        (
            "entity_limited_company_ubo",
            Some("limited_company"),
            "PENDING",
        ),
        ("cbu_evidence", None, "UPLOADED"),
        ("share_class", None, "DRAFT"),
    ] {
        let slot = slots
            .get(slot_id)
            .unwrap_or_else(|| panic!("{slot_id} present"));
        assert_eq!(
            slot.closure,
            Some(ClosureType::ClosedUnbounded),
            "{slot_id}"
        );
        assert_eq!(
            slot.entry_state.as_deref(),
            Some(expected_entry_state),
            "{slot_id}"
        );
        if let Some(kind) = expected_kind {
            assert_eq!(
                slot.eligibility,
                Some(EligibilityConstraint::EntityKinds {
                    entity_kinds: vec![kind.to_string()]
                }),
                "{slot_id}"
            );
        }
    }

    let manco = slots.get("manco").expect("manco slot present");
    assert_eq!(manco.closure, Some(ClosureType::ClosedBounded));
    assert_eq!(manco.cardinality_max, Some(1));
    assert_eq!(manco.entry_state.as_deref(), Some("UNDER_REVIEW"));
}

#[test]
fn lux_sicav_constellation_pilot_slots_have_gate_metadata() {
    let yaml = lux_sicav_yaml();
    let map: RawConstellationMap = serde_yaml::from_str(&yaml).expect("Lux SICAV parses");

    for slot_id in [
        "management_company",
        "depositary",
        "investment_manager",
        "mandate",
        "administrator",
        "auditor",
    ] {
        assert!(
            map.slots.contains_key(slot_id),
            "expected pilot slot {slot_id}"
        );
    }
    assert!(
        !map.slots.contains_key("domiciliation_agent"),
        "domiciliation_agent is explicitly out of 1.5C scope"
    );

    for slot_id in [
        "management_company",
        "depositary",
        "administrator",
        "auditor",
    ] {
        let slot = &map.slots[slot_id];
        assert_eq!(slot.closure, Some(ClosureType::ClosedBounded), "{slot_id}");
        assert_eq!(slot.cardinality_max, Some(1), "{slot_id}");
        assert_eq!(slot.entry_state.as_deref(), Some("empty"), "{slot_id}");
        assert_eq!(
            slot.eligibility,
            Some(EligibilityConstraint::EntityKinds {
                entity_kinds: vec!["company".to_string()]
            }),
            "{slot_id}"
        );
    }

    let investment_manager = &map.slots["investment_manager"];
    assert_eq!(
        investment_manager.closure,
        Some(ClosureType::ClosedUnbounded)
    );
    assert_eq!(investment_manager.cardinality_max, None);
    assert_eq!(investment_manager.entry_state.as_deref(), Some("empty"));

    let mandate = &map.slots["mandate"];
    assert_eq!(mandate.closure, Some(ClosureType::ClosedBounded));
    assert_eq!(mandate.cardinality_max, Some(1));
}

#[test]
fn lux_sicav_administrator_and_auditor_match_aif_raif_structural_blocks() {
    let sicav = lux_sicav_yaml();
    let aif_raif = lux_aif_raif_yaml();

    for slot_id in ["administrator", "auditor"] {
        assert_eq!(
            slot_without_gate_metadata(&sicav, slot_id),
            slot_without_gate_metadata(&aif_raif, slot_id),
            "{slot_id} structural block drifted from Lux AIF RAIF baseline"
        );
    }
}

#[test]
fn pilot_pair_has_no_schema_coordination_errors() {
    let dag = cbu_dag();
    let loaded = BTreeMap::from([(
        "cbu".to_string(),
        LoadedDag {
            source_path: seed_path("dag_taxonomies/cbu_dag.yaml"),
            dag,
        },
    )]);

    let report = validate_constellation_map_schema_coordination(
        &loaded,
        "struct_lux_ucits_sicav.yaml",
        &lux_sicav_yaml(),
    );

    assert!(
        report.errors.is_empty(),
        "schema-coordination errors: {:#?}",
        report.errors
    );
}
