#[cfg(feature = "database")]
mod tests {
    use uuid::Uuid;

    use ob_poc::sem_os_runtime::constellation_runtime::{
        compute_summary, load_builtin_constellation_map, normalize_slots,
        HydratedCardinality, HydratedSlot, HydratedSlotType, RawGraphEdge, RawHydrationData,
        RawSlotRow,
    };

    #[test]
    fn singular_slot_empty() {
        let map = load_builtin_constellation_map("struct.lux.ucits.sicav").unwrap();
        let hydrated = normalize_slots(&map, Uuid::nil(), None, RawHydrationData::default());
        assert_eq!(hydrated.slots[0].computed_state, "empty");
    }

    #[test]
    fn graph_truncation_at_max_depth() {
        let map = load_builtin_constellation_map("struct.lux.ucits.sicav").unwrap();
        let mut raw = RawHydrationData::default();
        raw.slot_rows.insert(
            "ownership_chain".into(),
            vec![
                RawSlotRow {
                    entity_id: Some(Uuid::new_v4()),
                    record_id: None,
                    filter_value: None,
                    created_at: None,
                },
                RawSlotRow {
                    entity_id: Some(Uuid::new_v4()),
                    record_id: None,
                    filter_value: None,
                    created_at: None,
                },
            ],
        );
        raw.graph_edges.insert(
            "ownership_chain".into(),
            vec![RawGraphEdge {
                from_entity_id: Uuid::new_v4(),
                to_entity_id: Uuid::new_v4(),
                percentage: Some(75.0),
                ownership_type: Some(String::from("direct")),
                depth: 3,
            }],
        );
        let hydrated = normalize_slots(&map, Uuid::nil(), None, raw);
        let chain = hydrated
            .slots
            .iter()
            .flat_map(|slot| slot.children.iter())
            .find(|slot| slot.name == "ownership_chain")
            .unwrap();
        assert!(!chain.warnings.is_empty());
        assert_eq!(chain.graph_node_count, Some(2));
        assert_eq!(chain.graph_edge_count, Some(1));
        assert_eq!(chain.graph_nodes.len(), 2);
        assert_eq!(chain.graph_edges.len(), 1);
    }

    #[test]
    fn summary_counts_blocking_slots() {
        let summary = compute_summary(
            &ob_poc::sem_os_runtime::constellation_runtime::HydratedConstellation {
                constellation: String::from("demo"),
                description: None,
                jurisdiction: String::from("LU"),
                map_revision: String::from("0000000000000000"),
                cbu_id: Uuid::nil(),
                case_id: None,
                slots: vec![HydratedSlot {
                    name: String::from("management_company"),
                    path: String::from("management_company"),
                    slot_type: HydratedSlotType::Entity,
                    cardinality: HydratedCardinality::Mandatory,
                    entity_id: None,
                    record_id: None,
                    computed_state: String::from("empty"),
                    effective_state: String::from("empty"),
                    progress: 0,
                    blocking: true,
                    warnings: vec![],
                    overlays: vec![],
                    graph_node_count: None,
                    graph_edge_count: None,
                    graph_nodes: vec![],
                    graph_edges: vec![],
                    available_verbs: vec![],
                    blocked_verbs: vec![],
                    children: vec![],
                }],
            },
        );
        assert_eq!(summary.blocking_slots, 1);
        assert_eq!(summary.slots_empty_mandatory, 1);
    }

    #[test]
    fn singular_slot_multiplicity_warning() {
        let map = load_builtin_constellation_map("struct.lux.ucits.sicav").unwrap();
        let mut raw = RawHydrationData::default();
        raw.slot_rows.insert(
            "management_company".into(),
            vec![
                RawSlotRow {
                    entity_id: Some(Uuid::new_v4()),
                    record_id: Some(Uuid::new_v4()),
                    filter_value: None,
                    created_at: None,
                },
                RawSlotRow {
                    entity_id: Some(Uuid::new_v4()),
                    record_id: Some(Uuid::new_v4()),
                    filter_value: None,
                    created_at: None,
                },
            ],
        );
        let hydrated = normalize_slots(&map, Uuid::nil(), None, raw);
        let manco = hydrated.slots[0]
            .children
            .iter()
            .find(|slot| slot.name == "management_company")
            .unwrap();
        assert!(manco
            .warnings
            .iter()
            .any(|warning| warning.contains("deterministic representative")));
    }

    #[test]
    fn child_cbu_slot_normalizes_as_filled_when_linked() {
        let map = load_builtin_constellation_map("struct.hedge.cross-border").unwrap();
        let feeder_cbu_id = Uuid::new_v4();
        let mut raw = RawHydrationData::default();
        raw.slot_rows.insert(
            "cbu.us_feeder".into(),
            vec![RawSlotRow {
                entity_id: Some(feeder_cbu_id),
                record_id: Some(feeder_cbu_id),
                filter_value: Some(String::from("feeder:us")),
                created_at: None,
            }],
        );

        let hydrated = normalize_slots(&map, Uuid::nil(), None, raw);
        let feeder = hydrated.slots[0]
            .children
            .iter()
            .find(|slot| slot.name == "cbu.us_feeder")
            .unwrap();
        assert_eq!(feeder.effective_state, "filled");
        assert_eq!(feeder.record_id, Some(feeder_cbu_id));
    }
}
