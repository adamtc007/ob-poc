#[cfg(feature = "database")]
mod tests {
    use ob_poc::sem_reg::constellation::{
        compile_query_plan, compute_map_revision, load_builtin_constellation_map,
        load_constellation_map, QueryType,
    };

    #[test]
    fn loads_lux_ucits_sicav_yaml() {
        let map = load_builtin_constellation_map("struct.lux.ucits.sicav").unwrap();
        assert_eq!(map.constellation, "struct.lux.ucits.sicav");
    }

    #[test]
    fn validates_lux_ucits_sicav() {
        let map = load_builtin_constellation_map("struct.lux.ucits.sicav").unwrap();
        assert!(!map.slots_ordered.is_empty());
    }

    #[test]
    fn slots_in_dependency_order() {
        let map = load_builtin_constellation_map("struct.lux.ucits.sicav").unwrap();
        let cbu = map
            .slots_ordered
            .iter()
            .position(|slot| slot.name == "cbu")
            .unwrap();
        let manco = map
            .slots_ordered
            .iter()
            .position(|slot| slot.name == "management_company")
            .unwrap();
        assert!(cbu < manco);
    }

    #[test]
    fn root_slot_is_cbu() {
        let map = load_builtin_constellation_map("struct.lux.ucits.sicav").unwrap();
        assert_eq!(map.slots_ordered[0].name, "cbu");
    }

    #[test]
    fn recursive_slot_has_max_depth() {
        let map = load_builtin_constellation_map("struct.lux.ucits.sicav").unwrap();
        let slot = map.slot_index.get("ownership_chain").unwrap();
        assert_eq!(slot.def.max_depth, Some(5));
    }

    #[test]
    fn rejects_missing_root() {
        let yaml = r#"
constellation: bad
jurisdiction: LU
slots:
  child:
    type: entity
    cardinality: mandatory
    join:
      via: cbu_entity_roles
      parent_fk: cbu_id
      child_fk: entity_id
"#;
        assert!(load_constellation_map(yaml).is_err());
    }

    #[test]
    fn rejects_cyclic_dependencies() {
        let yaml = r#"
constellation: bad
jurisdiction: LU
slots:
  cbu:
    type: cbu
    table: cbus
    pk: cbu_id
    cardinality: root
    depends_on: [a]
  a:
    type: entity
    cardinality: mandatory
    join:
      via: cbu_entity_roles
      parent_fk: cbu_id
      child_fk: entity_id
    depends_on: [cbu]
"#;
        assert!(load_constellation_map(yaml).is_err());
    }

    #[test]
    fn map_revision_is_stable() {
        let value = compute_map_revision("demo");
        assert_eq!(value, compute_map_revision("demo"));
    }

    #[test]
    fn plan_includes_overlay_batch_queries() {
        let map = load_builtin_constellation_map("struct.lux.ucits.sicav").unwrap();
        let plan = compile_query_plan(&map);
        assert!(plan.levels.iter().any(|level| {
            level.queries.iter().any(|query| {
                query.slot_name == "management_company.overlays"
                    && query.query_type == QueryType::OverlayBatch
            })
        }));
        assert!(plan.levels.iter().any(|level| {
            level.queries.iter().any(|query| {
                query.slot_name == "ownership_chain.overlays"
                    && query.query_type == QueryType::OverlayBatch
                    && query.sql.contains("edge:ownership")
            })
        }));
    }
}
