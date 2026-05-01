use std::collections::BTreeMap;

use dsl_core::{
    config::dag::{ClosureType, PredicateBinding},
    frontier::{hydrate_frontier, EntityRef, FrontierFact, GreenWhenStatus, HydrateFrontierError},
    resolver::{
        ResolvedSlot, ResolvedSource, ResolvedTemplate, ResolvedTransition, ResolverProvenance,
        SlotProvenance, VersionHash,
    },
};
use sem_os_core::constellation_map_def::CompletenessAssertionConfig;

fn template() -> ResolvedTemplate {
    ResolvedTemplate {
        workspace: "cbu".to_string(),
        composite_shape: "struct.test".to_string(),
        structural_facts: Default::default(),
        slots: vec![ResolvedSlot {
            id: "review".to_string(),
            state_machine: Some("review_lifecycle".to_string()),
            predicate_bindings: vec![PredicateBinding {
                entity: "evidence".to_string(),
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
            entity_kinds: vec!["review".to_string()],
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
            entry_state: Some("PENDING".to_string()),
            attachment_predicates: Vec::new(),
            addition_predicates: Vec::new(),
            aggregate_breach_checks: Vec::new(),
            role_guard: None,
            justification_required: None,
            audit_class: None,
            completeness_assertion: None,
            provenance: SlotProvenance {
                field_sources: BTreeMap::from([(
                    "predicate_bindings".to_string(),
                    ResolvedSource::DagTaxonomy,
                )]),
            },
        }],
        transitions: vec![
            ResolvedTransition {
                slot_id: "review".to_string(),
                from: "PENDING".to_string(),
                to: "READY".to_string(),
                via: Some("review.mark-ready".to_string()),
                destination_green_when: Some(
                    "evidence exists AND evidence.state = COMPLETE".to_string(),
                ),
            },
            ResolvedTransition {
                slot_id: "review".to_string(),
                from: "READY".to_string(),
                to: "ARCHIVED".to_string(),
                via: Some("review.archive".to_string()),
                destination_green_when: None,
            },
            ResolvedTransition {
                slot_id: "review".to_string(),
                from: "READY".to_string(),
                to: "SYSTEM_EXPIRED".to_string(),
                via: Some("(backend: retention timer)".to_string()),
                destination_green_when: None,
            },
            ResolvedTransition {
                slot_id: "review".to_string(),
                from: "READY".to_string(),
                to: "AUTO_CLOSED".to_string(),
                via: None,
                destination_green_when: None,
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

fn template_with_slot(mut slot: ResolvedSlot) -> ResolvedTemplate {
    let mut template = template();
    slot.provenance = template.slots[0].provenance.clone();
    template.slots = vec![slot];
    template
}

fn entity(current_state: &str, facts: BTreeMap<String, Vec<FrontierFact>>) -> EntityRef {
    EntityRef {
        slot_id: "review".to_string(),
        entity_id: "review-1".to_string(),
        current_state: current_state.to_string(),
        facts,
    }
}

#[test]
fn hydrate_frontier_returns_green_reachable_destination_for_satisfied_postcondition() {
    let facts = BTreeMap::from([(
        "evidence".to_string(),
        vec![FrontierFact {
            state: Some("COMPLETE".to_string()),
            attrs: BTreeMap::new(),
        }],
    )]);

    let frontier = hydrate_frontier(entity("PENDING", facts), &template()).expect("hydrates");

    assert_eq!(frontier.current_state, "PENDING");
    assert_eq!(frontier.reachable.len(), 1);
    assert_eq!(frontier.reachable[0].destination_state, "READY");
    assert_eq!(
        frontier.reachable[0].via_verb.as_deref(),
        Some("review.mark-ready")
    );
    assert_eq!(frontier.reachable[0].status, GreenWhenStatus::Green);
}

#[test]
fn hydrate_frontier_returns_awaiting_completeness_for_open_slot_with_stale_assertion() {
    let mut slot = template().slots.remove(0);
    slot.closure = Some(ClosureType::Open);
    slot.completeness_assertion = Some(CompletenessAssertionConfig {
        predicate: Some("evidence.state = COMPLETE".to_string()),
        description: Some("evidence population is complete".to_string()),
        extra: BTreeMap::new(),
    });
    let mut template = template_with_slot(slot);
    template.transitions[0].destination_green_when = None;
    let facts = BTreeMap::from([(
        "evidence".to_string(),
        vec![FrontierFact {
            state: Some("PENDING".to_string()),
            attrs: BTreeMap::new(),
        }],
    )]);

    let frontier = hydrate_frontier(entity("PENDING", facts), &template).expect("hydrates");

    assert_eq!(
        frontier.reachable[0].status,
        GreenWhenStatus::AwaitingCompleteness(dsl_core::frontier::CompletenessAssertionStatus {
            assertion: "evidence.state = COMPLETE".to_string(),
            satisfied: false,
        })
    );
}

#[test]
fn hydrate_frontier_returns_discretionary_for_justification_required_slot() {
    let mut slot = template().slots.remove(0);
    slot.justification_required = Some(true);
    let mut template = template_with_slot(slot);
    template.transitions[0].destination_green_when = None;

    let frontier =
        hydrate_frontier(entity("PENDING", BTreeMap::new()), &template).expect("hydrates");

    assert!(matches!(
        &frontier.reachable[0].status,
        GreenWhenStatus::Discretionary(reason)
            if reason.reason == "slot review requires justification"
    ));
}

#[test]
fn hydrate_frontier_returns_red_destination_for_unsatisfied_postcondition() {
    let facts = BTreeMap::from([(
        "evidence".to_string(),
        vec![FrontierFact {
            state: Some("DRAFT".to_string()),
            attrs: BTreeMap::new(),
        }],
    )]);

    let frontier = hydrate_frontier(entity("PENDING", facts), &template()).expect("hydrates");

    let GreenWhenStatus::Red { missing, invalid } = &frontier.reachable[0].status else {
        panic!("expected red destination");
    };
    assert!(missing.is_empty());
    assert_eq!(invalid.len(), 1);
    assert_eq!(invalid[0].entity, "evidence");
}

#[test]
fn hydrate_frontier_filters_transitions_by_current_state() {
    let frontier =
        hydrate_frontier(entity("READY", BTreeMap::new()), &template()).expect("hydrates");

    assert_eq!(frontier.reachable.len(), 1);
    assert_eq!(frontier.reachable[0].destination_state, "ARCHIVED");
    assert_eq!(frontier.reachable[0].status, GreenWhenStatus::Green);
}

#[test]
fn hydrate_frontier_errors_for_unknown_slot() {
    let mut entity_ref = entity("PENDING", BTreeMap::new());
    entity_ref.slot_id = "missing".to_string();

    let err = hydrate_frontier(entity_ref, &template()).expect_err("unknown slot fails");

    assert!(matches!(err, HydrateFrontierError::SlotNotFound(slot) if slot == "missing"));
}
