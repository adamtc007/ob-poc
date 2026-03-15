#[cfg(feature = "database")]
mod tests {
    use uuid::Uuid;

    use ob_poc::sem_reg::constellation::{
        compute_action_surface, load_builtin_constellation_map, normalize_slots, RawHydrationData,
        RawSlotRow,
    };

    #[test]
    fn deps_unmet_blocks_verb() {
        let map = load_builtin_constellation_map("struct.lux.ucits.sicav").unwrap();
        let hydrated = normalize_slots(&map, Uuid::nil(), None, RawHydrationData::default());
        let surfaced = compute_action_surface(&map, hydrated);
        let manco = surfaced.slots[0]
            .children
            .iter()
            .find(|slot| slot.name == "management_company")
            .unwrap();
        assert!(!manco.blocked_verbs.is_empty());
    }

    #[test]
    fn progress_empty_optional() {
        let map = load_builtin_constellation_map("struct.lux.ucits.sicav").unwrap();
        let surfaced = compute_action_surface(
            &map,
            normalize_slots(&map, Uuid::nil(), None, RawHydrationData::default()),
        );
        let im = surfaced.slots[0]
            .children
            .iter()
            .find(|slot| slot.name == "investment_manager")
            .unwrap();
        assert_eq!(im.progress, 100);
    }

    #[test]
    fn blocked_reason_names_missing_dependency() {
        let map = load_builtin_constellation_map("struct.lux.ucits.sicav").unwrap();
        let surfaced = compute_action_surface(
            &map,
            normalize_slots(&map, Uuid::nil(), None, RawHydrationData::default()),
        );
        let manco = surfaced.slots[0]
            .children
            .iter()
            .find(|slot| slot.name == "management_company")
            .unwrap();
        let blocked = manco
            .blocked_verbs
            .iter()
            .find(|verb| verb.verb == "entity.ensure-or-placeholder")
            .unwrap();
        assert!(
            blocked
                .reasons
                .iter()
                .any(|reason| reason.message.contains("dependency 'cbu'"))
        );
    }

    #[test]
    fn gated_verb_available_when_state_matches() {
        let map = load_builtin_constellation_map("struct.lux.ucits.sicav").unwrap();
        let mut raw = RawHydrationData::default();
        raw.slot_rows.insert(
            "cbu".into(),
            vec![RawSlotRow {
                entity_id: Some(Uuid::nil()),
                record_id: Some(Uuid::nil()),
                filter_value: None,
                created_at: None,
            }],
        );
        raw.slot_rows.insert(
            "management_company".into(),
            vec![RawSlotRow {
                entity_id: Some(Uuid::new_v4()),
                record_id: Some(Uuid::new_v4()),
                filter_value: None,
                created_at: None,
            }],
        );
        let surfaced = compute_action_surface(&map, normalize_slots(&map, Uuid::nil(), None, raw));
        let manco = surfaced.slots[0]
            .children
            .iter()
            .find(|slot| slot.name == "management_company")
            .unwrap();
        assert!(manco.available_verbs.iter().any(|verb| verb == "entity.read"));
    }

    #[test]
    fn deps_placeholder_satisfies_min_state_placeholder() {
        let map = load_builtin_constellation_map("struct.lux.ucits.sicav").unwrap();
        let mut raw = RawHydrationData::default();
        raw.slot_rows.insert(
            "cbu".into(),
            vec![RawSlotRow {
                entity_id: Some(Uuid::nil()),
                record_id: Some(Uuid::nil()),
                filter_value: None,
                created_at: None,
            }],
        );
        raw.slot_rows.insert(
            "management_company".into(),
            vec![RawSlotRow {
                entity_id: Some(Uuid::new_v4()),
                record_id: Some(Uuid::new_v4()),
                filter_value: None,
                created_at: None,
            }],
        );
        raw.slot_rows.insert(
            "case".into(),
            vec![RawSlotRow {
                entity_id: None,
                record_id: Some(Uuid::new_v4()),
                filter_value: Some(String::from("placeholder")),
                created_at: None,
            }],
        );
        let surfaced = compute_action_surface(&map, normalize_slots(&map, Uuid::nil(), None, raw));
        let tollgate = surfaced.slots[0]
            .children
            .iter()
            .find(|slot| slot.name == "case")
            .and_then(|slot| slot.children.iter().find(|slot| slot.name == "case.tollgate"))
            .unwrap();
        assert!(tollgate.available_verbs.iter().any(|verb| verb == "tollgate.evaluate"));
    }

    #[test]
    fn simple_verb_blocked_when_deps_unmet() {
        let map = load_builtin_constellation_map("struct.lux.ucits.sicav").unwrap();
        let surfaced = compute_action_surface(
            &map,
            normalize_slots(&map, Uuid::nil(), None, RawHydrationData::default()),
        );
        let tollgate = surfaced.slots[0]
            .children
            .iter()
            .find(|slot| slot.name == "case")
            .and_then(|slot| slot.children.iter().find(|slot| slot.name == "case.tollgate"))
            .unwrap();
        assert!(tollgate.available_verbs.is_empty());
        assert!(
            tollgate
                .blocked_verbs
                .iter()
                .any(|verb| verb.verb == "tollgate.evaluate")
        );
    }
}
