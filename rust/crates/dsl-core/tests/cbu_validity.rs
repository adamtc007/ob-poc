use std::collections::BTreeMap;

use dsl_core::{
    config::dag::PredicateBinding,
    frontier::{hydrate_frontier, EntityRef, FrontierFact, GreenWhenStatus, InvalidFactDetail},
    resolver::{
        ResolvedSlot, ResolvedTemplate, ResolvedTransition, ResolverProvenance, SlotProvenance,
        VersionHash,
    },
};

const CBU_VALIDATED_GREEN_WHEN: &str = r#"
every entity_proper_person.state = VERIFIED
AND every entity_limited_company_ubo.state in {DISCOVERED, PUBLIC_FLOAT, EXEMPT}
AND every mandate.state in {approved, active}
AND every cbu_evidence.state = APPROVED
AND no investor_disqualifying_flag exists
AND investment_managers.completeness = green
"#;

fn template() -> ResolvedTemplate {
    ResolvedTemplate {
        workspace: "cbu".to_string(),
        composite_shape: "struct.lux.ucits.sicav".to_string(),
        structural_facts: Default::default(),
        slots: vec![ResolvedSlot {
            id: "cbu".to_string(),
            state_machine: Some("cbu_discovery_lifecycle".to_string()),
            predicate_bindings: [
                "entity_proper_person",
                "entity_limited_company_ubo",
                "mandate",
                "cbu_evidence",
                "investor_disqualifying_flag",
                "investment_managers",
            ]
            .into_iter()
            .map(binding)
            .collect(),
            table: None,
            pk: None,
            join: None,
            entity_kinds: vec!["cbu".to_string()],
            cardinality: None,
            depends_on: Vec::new(),
            placeholder: None,
            overlays: Vec::new(),
            edge_overlays: Vec::new(),
            verbs: BTreeMap::new(),
            children: BTreeMap::new(),
            max_depth: None,
            closure: None,
            eligibility: None,
            cardinality_max: None,
            entry_state: Some("DISCOVERED".to_string()),
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
            slot_id: "cbu".to_string(),
            from: "VALIDATION_PENDING".to_string(),
            to: "VALIDATED".to_string(),
            via: Some("cbu.decide".to_string()),
            destination_green_when: Some(CBU_VALIDATED_GREEN_WHEN.to_string()),
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

fn binding(entity: &str) -> PredicateBinding {
    PredicateBinding {
        entity: entity.to_string(),
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
    }
}

fn happy_facts() -> BTreeMap<String, Vec<FrontierFact>> {
    BTreeMap::from([
        (
            "entity_proper_person".to_string(),
            vec![state_fact("VERIFIED")],
        ),
        (
            "entity_limited_company_ubo".to_string(),
            vec![
                recursive_state_fact("ubo-1", "cbu-1", "DISCOVERED"),
                recursive_state_fact("ubo-2", "ubo-1", "PUBLIC_FLOAT"),
            ],
        ),
        ("mandate".to_string(), vec![state_fact("approved")]),
        ("cbu_evidence".to_string(), vec![state_fact("APPROVED")]),
        (
            "investment_managers".to_string(),
            vec![attr_fact("completeness", "green")],
        ),
    ])
}

fn state_fact(state: &str) -> FrontierFact {
    FrontierFact {
        state: Some(state.to_string()),
        attrs: BTreeMap::new(),
    }
}

fn recursive_state_fact(id: &str, parent_id: &str, state: &str) -> FrontierFact {
    FrontierFact {
        state: Some(state.to_string()),
        attrs: BTreeMap::from([
            ("id".to_string(), id.to_string()),
            ("parent_id".to_string(), parent_id.to_string()),
        ]),
    }
}

fn attr_fact(attr: &str, value: &str) -> FrontierFact {
    FrontierFact {
        state: None,
        attrs: BTreeMap::from([(attr.to_string(), value.to_string())]),
    }
}

fn cbu(facts: BTreeMap<String, Vec<FrontierFact>>) -> EntityRef {
    EntityRef {
        slot_id: "cbu".to_string(),
        entity_id: "cbu-1".to_string(),
        current_state: "VALIDATION_PENDING".to_string(),
        facts,
    }
}

fn validated_status(facts: BTreeMap<String, Vec<FrontierFact>>) -> GreenWhenStatus {
    hydrate_frontier(cbu(facts), &template())
        .expect("hydrates")
        .reachable
        .into_iter()
        .next()
        .expect("VALIDATED reachable")
        .status
}

#[test]
fn cbu_validated_green_when_happy_path_is_green() {
    assert_eq!(validated_status(happy_facts()), GreenWhenStatus::Green);
}

#[test]
fn cbu_validated_fails_when_person_is_not_verified() {
    let mut facts = happy_facts();
    facts.insert(
        "entity_proper_person".to_string(),
        vec![state_fact("IDENTIFIED")],
    );

    assert_red_invalid_entity(validated_status(facts), "this");
}

#[test]
fn cbu_validated_fails_when_ubo_chain_has_unapproved_descendant() {
    let mut facts = happy_facts();
    facts.insert(
        "entity_limited_company_ubo".to_string(),
        vec![
            recursive_state_fact("ubo-1", "cbu-1", "DISCOVERED"),
            recursive_state_fact("ubo-2", "ubo-1", "PUBLIC_FLOAT"),
            recursive_state_fact("ubo-3", "ubo-2", "MANUAL_REQUIRED"),
        ],
    );

    assert_red_invalid_entity(validated_status(facts), "this");
}

#[test]
fn cbu_validated_fails_when_mandate_is_not_approved_or_active() {
    let mut facts = happy_facts();
    facts.insert("mandate".to_string(), vec![state_fact("draft")]);

    assert_red_invalid_entity(validated_status(facts), "this");
}

#[test]
fn cbu_validated_fails_when_evidence_is_not_approved() {
    let mut facts = happy_facts();
    facts.insert("cbu_evidence".to_string(), vec![state_fact("REVIEWED")]);

    assert_red_invalid_entity(validated_status(facts), "this");
}

#[test]
fn cbu_validated_fails_when_evidence_population_has_mixed_states() {
    let mut facts = happy_facts();
    facts.insert(
        "cbu_evidence".to_string(),
        vec![state_fact("APPROVED"), state_fact("REVIEWED")],
    );

    assert_red_invalid_entity(validated_status(facts), "this");
}

#[test]
fn cbu_validated_fails_when_disqualifying_flag_exists() {
    let mut facts = happy_facts();
    facts.insert(
        "investor_disqualifying_flag".to_string(),
        vec![state_fact("OPEN")],
    );

    assert_red_forbidden_member(validated_status(facts), "investor_disqualifying_flag");
}

#[test]
fn cbu_validated_fails_when_completeness_assertion_is_not_green() {
    let mut facts = happy_facts();
    facts.insert(
        "investment_managers".to_string(),
        vec![attr_fact("completeness", "red")],
    );

    assert_red_invalid_entity(validated_status(facts), "investment_managers");
}

fn assert_red_invalid_entity(status: GreenWhenStatus, entity: &str) {
    let GreenWhenStatus::Red { missing, invalid } = status else {
        panic!("expected red status");
    };
    assert!(missing.is_empty(), "unexpected missing facts: {missing:?}");
    assert!(
        invalid.iter().any(|fact| fact.entity == entity),
        "expected invalid entity {entity}; invalid={invalid:?}"
    );
}

fn assert_red_forbidden_member(status: GreenWhenStatus, entity: &str) {
    let GreenWhenStatus::Red { missing, invalid } = status else {
        panic!("expected red status");
    };
    assert!(missing.is_empty(), "unexpected missing facts: {missing:?}");
    assert_eq!(invalid.len(), 1);
    assert_eq!(invalid[0].entity, entity);
    assert!(matches!(
        &invalid[0].detail,
        InvalidFactDetail::ForbiddenMemberPresent { kind, .. } if kind == entity
    ));
}
