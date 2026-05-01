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
            id: "cbu_evidence".to_string(),
            state_machine: Some("cbu_evidence_lifecycle".to_string()),
            predicate_bindings: ["evidence_blob", "evidence_review", "evidence_expiry"]
                .into_iter()
                .map(binding)
                .collect(),
            table: None,
            pk: None,
            join: None,
            entity_kinds: vec!["cbu_evidence".to_string()],
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
            entry_state: Some("UPLOADED".to_string()),
            attachment_predicates: Vec::new(),
            addition_predicates: Vec::new(),
            aggregate_breach_checks: Vec::new(),
            role_guard: None,
            justification_required: None,
            audit_class: None,
            completeness_assertion: None,
            provenance: SlotProvenance::default(),
        }],
        transitions: vec![
            ResolvedTransition {
                slot_id: "cbu_evidence".to_string(),
                from: "UPLOADED".to_string(),
                to: "REVIEWED".to_string(),
                via: Some("cbu.review-evidence".to_string()),
                destination_green_when: Some(
                    "evidence_review exists AND evidence_review.state = COMPLETE".to_string(),
                ),
            },
            ResolvedTransition {
                slot_id: "cbu_evidence".to_string(),
                from: "REVIEWED".to_string(),
                to: "APPROVED".to_string(),
                via: Some("cbu.approve-evidence".to_string()),
                destination_green_when: Some(
                    "evidence_review exists AND evidence_review.state = APPROVED AND evidence_expiry.status = CURRENT"
                        .to_string(),
                ),
            },
            ResolvedTransition {
                slot_id: "cbu_evidence".to_string(),
                from: "APPROVED".to_string(),
                to: "EXPIRED".to_string(),
                via: Some("evidence.expire".to_string()),
                destination_green_when: Some("evidence_expiry.status = EXPIRED".to_string()),
            },
        ],
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

fn evidence(current_state: &str, facts: BTreeMap<String, Vec<FrontierFact>>) -> EntityRef {
    EntityRef {
        slot_id: "cbu_evidence".to_string(),
        entity_id: "evidence-1".to_string(),
        current_state: current_state.to_string(),
        facts,
    }
}

fn state_fact(state: &str) -> FrontierFact {
    FrontierFact {
        state: Some(state.to_string()),
        attrs: BTreeMap::new(),
    }
}

fn attr_fact(attr: &str, value: &str) -> FrontierFact {
    FrontierFact {
        state: None,
        attrs: BTreeMap::from([(attr.to_string(), value.to_string())]),
    }
}

#[test]
fn uploaded_evidence_can_reach_reviewed_when_review_is_complete() {
    let facts = BTreeMap::from([("evidence_review".to_string(), vec![state_fact("COMPLETE")])]);

    let frontier = hydrate_frontier(evidence("UPLOADED", facts), &template()).expect("hydrates");

    assert_eq!(frontier.reachable.len(), 1);
    assert_eq!(frontier.reachable[0].destination_state, "REVIEWED");
    assert_eq!(frontier.reachable[0].status, GreenWhenStatus::Green);
}

#[test]
fn reviewed_evidence_can_reach_approved_when_review_and_expiry_are_green() {
    let facts = BTreeMap::from([
        ("evidence_review".to_string(), vec![state_fact("APPROVED")]),
        (
            "evidence_expiry".to_string(),
            vec![attr_fact("status", "CURRENT")],
        ),
    ]);

    let frontier = hydrate_frontier(evidence("REVIEWED", facts), &template()).expect("hydrates");

    assert_eq!(frontier.reachable.len(), 1);
    assert_eq!(frontier.reachable[0].destination_state, "APPROVED");
    assert_eq!(frontier.reachable[0].status, GreenWhenStatus::Green);
}

#[test]
fn reviewed_evidence_is_red_when_expiry_is_not_current() {
    let facts = BTreeMap::from([
        ("evidence_review".to_string(), vec![state_fact("APPROVED")]),
        (
            "evidence_expiry".to_string(),
            vec![attr_fact("status", "EXPIRED")],
        ),
    ]);

    let frontier = hydrate_frontier(evidence("REVIEWED", facts), &template()).expect("hydrates");

    let GreenWhenStatus::Red { missing, invalid } = &frontier.reachable[0].status else {
        panic!("expected red destination");
    };
    assert!(missing.is_empty());
    assert!(invalid.iter().any(|fact| fact.entity == "evidence_expiry"));
}
