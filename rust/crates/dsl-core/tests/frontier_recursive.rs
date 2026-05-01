use std::collections::BTreeMap;

use dsl_core::{
    config::dag::PredicateBinding,
    frontier::{hydrate_frontier, EntityRef, FrontierFact, GreenWhenStatus},
    resolver::{
        ResolvedSlot, ResolvedTemplate, ResolvedTransition, ResolverProvenance, SlotProvenance,
        VersionHash,
    },
};

fn template() -> ResolvedTemplate {
    ResolvedTemplate {
        workspace: "cbu".to_string(),
        composite_shape: "struct.test".to_string(),
        structural_facts: Default::default(),
        slots: vec![ResolvedSlot {
            id: "ubo_registry".to_string(),
            state_machine: Some("ubo_registry_lifecycle".to_string()),
            predicate_bindings: vec![PredicateBinding {
                entity: "ubo".to_string(),
                source_kind: Default::default(),
                source_entity: None,
                state_column: None,
                value_column: None,
                id_column: None,
                scope: None,
                parent_key: None,
                child_key: None,
                required_universe: None,
                replaceable_by_shape: false,
                extra: BTreeMap::new(),
            }],
            table: None,
            pk: None,
            join: None,
            entity_kinds: vec!["ubo_registry".to_string()],
            cardinality: None,
            depends_on: Vec::new(),
            placeholder: None,
            overlays: Vec::new(),
            edge_overlays: Vec::new(),
            verbs: BTreeMap::new(),
            children: BTreeMap::new(),
            max_depth: Some(10),
            closure: None,
            eligibility: None,
            cardinality_max: None,
            entry_state: Some("PENDING".to_string()),
            attachment_predicates: Vec::new(),
            addition_predicates: Vec::new(),
            aggregate_breach_checks: Vec::new(),
            role_guard: None,
            justification_required: None,
            audit_class: None,
            completeness_assertion: None,
            provenance: SlotProvenance::default(),
        }],
        transitions: vec![ResolvedTransition {
            slot_id: "ubo_registry".to_string(),
            from: "PENDING".to_string(),
            to: "PROVABLE".to_string(),
            via: Some("ubo_registry.prove".to_string()),
            destination_green_when: Some("every ubo.state = VERIFIED".to_string()),
        }],
        version: VersionHash("test-version".to_string()),
        generated_at: "test".to_string(),
        generated_from: ResolverProvenance {
            dag_paths: Vec::new(),
            constellation_paths: Vec::new(),
            shape_rule_paths: Vec::new(),
            legacy_constellation_stack: Vec::new(),
        },
    }
}

fn entity(facts: Vec<FrontierFact>) -> EntityRef {
    EntityRef {
        slot_id: "ubo_registry".to_string(),
        entity_id: "root-cbu".to_string(),
        current_state: "PENDING".to_string(),
        facts: BTreeMap::from([("ubo".to_string(), facts)]),
    }
}

fn ubo(id: &str, parent_id: &str, state: &str) -> FrontierFact {
    FrontierFact {
        state: Some(state.to_string()),
        attrs: BTreeMap::from([
            ("id".to_string(), id.to_string()),
            ("parent_id".to_string(), parent_id.to_string()),
        ]),
    }
}

#[test]
fn acyclic_ubo_chain_evaluates_green_when_all_descendants_satisfy_condition() {
    let frontier = hydrate_frontier(
        entity(vec![
            ubo("ubo-1", "root-cbu", "VERIFIED"),
            ubo("ubo-2", "ubo-1", "VERIFIED"),
            ubo("ubo-3", "ubo-2", "VERIFIED"),
        ]),
        &template(),
    )
    .expect("hydrates");

    assert_eq!(frontier.reachable.len(), 1);
    assert_eq!(frontier.reachable[0].status, GreenWhenStatus::Green);
}

#[test]
fn acyclic_ubo_chain_evaluates_red_when_a_descendant_fails_condition() {
    let frontier = hydrate_frontier(
        entity(vec![
            ubo("ubo-1", "root-cbu", "VERIFIED"),
            ubo("ubo-2", "ubo-1", "DRAFT"),
        ]),
        &template(),
    )
    .expect("hydrates");

    let GreenWhenStatus::Red { missing, invalid } = &frontier.reachable[0].status else {
        panic!("expected red destination");
    };
    assert!(missing.is_empty());
    assert_eq!(invalid.len(), 1);
    assert_eq!(invalid[0].entity, "this");
}

#[test]
fn cyclic_ubo_chain_is_detected_and_reported_as_invalid_fact() {
    let frontier = hydrate_frontier(
        entity(vec![
            ubo("ubo-1", "root-cbu", "VERIFIED"),
            ubo("ubo-2", "ubo-1", "VERIFIED"),
            ubo("ubo-1", "ubo-2", "VERIFIED"),
        ]),
        &template(),
    )
    .expect("hydrates");

    let GreenWhenStatus::Red { missing, invalid } = &frontier.reachable[0].status else {
        panic!("expected red destination");
    };
    assert!(missing.is_empty());
    assert_eq!(invalid.len(), 1);
    assert_eq!(invalid[0].entity, "ubo");
    assert!(invalid[0].reason.starts_with("CycleDetected"));
}
