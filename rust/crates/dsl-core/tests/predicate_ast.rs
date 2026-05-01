use dsl_core::config::dag::{load_dags_from_dir, SlotStateMachine};
use dsl_core::config::predicate::{
    parse_green_when, CmpOp, EntityQualifier, EntityRef, EntitySetRef, Predicate, RelationScope,
    Validity,
};
use std::collections::BTreeSet;
use std::path::PathBuf;

#[derive(Debug)]
struct Fixture {
    file: String,
    dag: String,
    slot: String,
    state: String,
    predicate: String,
    bindings: BTreeSet<String>,
}

fn dag_dir() -> PathBuf {
    PathBuf::from(env!("CARGO_MANIFEST_DIR"))
        .join("../..")
        .join("config/sem_os_seeds/dag_taxonomies")
}

fn green_when_fixtures() -> Vec<Fixture> {
    let dags = load_dags_from_dir(&dag_dir()).expect("DAG taxonomies load");
    let mut fixtures = Vec::new();
    for loaded in dags.values() {
        for slot in &loaded.dag.slots {
            let Some(SlotStateMachine::Structured(machine)) = &slot.state_machine else {
                continue;
            };
            for state in &machine.states {
                let Some(predicate) = &state.green_when else {
                    continue;
                };
                if predicate.trim().is_empty() {
                    continue;
                }
                fixtures.push(Fixture {
                    file: loaded
                        .source_path
                        .file_name()
                        .expect("DAG file name")
                        .to_string_lossy()
                        .to_string(),
                    dag: loaded.dag.dag_id.clone(),
                    slot: slot.id.clone(),
                    state: state.id.clone(),
                    predicate: predicate.clone(),
                    bindings: machine
                        .predicate_bindings
                        .iter()
                        .map(|binding| binding.entity.clone())
                        .collect(),
                });
            }
        }
    }
    fixtures.sort_by(|left, right| {
        (&left.file, &left.slot, &left.state).cmp(&(&right.file, &right.slot, &right.state))
    });
    fixtures
}

fn collect_referenced_entities(predicate: &Predicate, out: &mut BTreeSet<String>) {
    match predicate {
        Predicate::And(items) => {
            for item in items {
                collect_referenced_entities(item, out);
            }
        }
        Predicate::Exists { entity }
        | Predicate::StateIn { entity, .. }
        | Predicate::AttrCmp { entity, .. }
        | Predicate::Obtained { entity, .. } => collect_entity_ref(entity, out),
        Predicate::Every { set, condition }
        | Predicate::NoneExists { set, condition }
        | Predicate::AtLeastOne { set, condition } => {
            collect_entity_set_ref(set, out);
            collect_referenced_entities(condition, out);
        }
        Predicate::Count { set, condition, .. } => {
            collect_entity_set_ref(set, out);
            if let Some(condition) = condition {
                collect_referenced_entities(condition, out);
            }
        }
    }
}

fn collect_entity_ref(entity: &EntityRef, out: &mut BTreeSet<String>) {
    match entity {
        EntityRef::This => {}
        EntityRef::Named(kind) | EntityRef::Parent(kind) => {
            out.insert(kind.clone());
        }
        EntityRef::Scoped { kind, .. } => {
            out.insert(kind.clone());
        }
    }
}

fn collect_entity_set_ref(set: &EntitySetRef, out: &mut BTreeSet<String>) {
    out.insert(set.kind.clone());
}

#[test]
fn confirmed_green_when_fixture_count_is_eighteen() {
    let fixtures = green_when_fixtures();
    assert_eq!(
        fixtures.len(),
        18,
        "confirmed Phase 1 fixture set drifted: {fixtures:#?}"
    );
}

#[test]
fn confirmed_green_when_fixtures_parse() {
    for fixture in green_when_fixtures() {
        parse_green_when(&fixture.predicate).unwrap_or_else(|err| {
            panic!(
                "{} | dag={} | slot={} | state={} did not parse: {err}",
                fixture.file, fixture.dag, fixture.slot, fixture.state
            )
        });
    }
}

#[test]
fn confirmed_green_when_entities_have_dag_bindings() {
    for fixture in green_when_fixtures() {
        let ast = parse_green_when(&fixture.predicate).expect("fixture parses");
        let mut referenced = BTreeSet::new();
        collect_referenced_entities(&ast, &mut referenced);

        for entity in referenced {
            assert!(
                fixture.bindings.contains(&entity),
                "{} | dag={} | slot={} | state={} missing predicate_binding for `{}`; bindings={:?}",
                fixture.file,
                fixture.dag,
                fixture.slot,
                fixture.state,
                entity,
                fixture.bindings
            );
        }
    }
}

#[test]
fn required_scoped_evidence_fixture_is_structured() {
    let fixture = green_when_fixtures()
        .into_iter()
        .find(|fixture| fixture.slot == "kyc_ubo_registry" && fixture.state == "PROVABLE")
        .expect("PROVABLE fixture exists");
    let ast = parse_green_when(&fixture.predicate).expect("fixture parses");

    let Predicate::Every { set, condition } = ast else {
        panic!("expected Every predicate");
    };
    assert_eq!(set.kind, "evidence_requirement");
    assert_eq!(set.qualifier, Some(EntityQualifier::Required));
    assert_eq!(set.scope, Some(RelationScope::This("UBO".to_string())));
    assert!(matches!(
        *condition,
        Predicate::Exists {
            entity: EntityRef::This
        }
    ));
}

#[test]
fn attached_negative_existence_fixture_is_structured() {
    let fixture = green_when_fixtures()
        .into_iter()
        .find(|fixture| fixture.slot == "clearance" && fixture.state == "APPROVED")
        .expect("booking principal APPROVED fixture exists");
    let ast = parse_green_when(&fixture.predicate).expect("fixture parses");

    let Predicate::And(items) = ast else {
        panic!("expected And predicate");
    };
    let red_flag = items
        .iter()
        .find_map(|item| match item {
            Predicate::NoneExists { set, condition } if set.kind == "red_flag" => {
                Some((set, condition))
            }
            Predicate::And(_)
            | Predicate::Exists { .. }
            | Predicate::StateIn { .. }
            | Predicate::AttrCmp { .. }
            | Predicate::Every { .. }
            | Predicate::AtLeastOne { .. }
            | Predicate::Count { .. }
            | Predicate::Obtained { .. }
            | Predicate::NoneExists { .. } => None,
        })
        .expect("red_flag negative existence predicate");

    assert_eq!(
        red_flag.0.scope,
        Some(RelationScope::AttachedTo("clearance".to_string()))
    );
    assert!(matches!(
        red_flag.1.as_ref(),
        Predicate::StateIn {
            entity: EntityRef::This,
            state_set
        } if state_set == &vec!["OPEN".to_string()]
    ));
}

#[test]
fn symbolic_attribute_threshold_fixture_is_structured() {
    let fixture = green_when_fixtures()
        .into_iter()
        .find(|fixture| fixture.slot == "kyc_ubo_registry" && fixture.state == "IDENTIFIED")
        .expect("IDENTIFIED fixture exists");
    let ast = parse_green_when(&fixture.predicate).expect("fixture parses");

    let Predicate::And(items) = ast else {
        panic!("expected And predicate");
    };
    assert!(items.iter().any(|item| {
        matches!(
            item,
            Predicate::AttrCmp {
                entity: EntityRef::Named(entity),
                attr,
                value,
                ..
            } if entity == "ownership_percentage"
                && attr == "value"
                && matches!(
                    value,
                    dsl_core::config::predicate::AttrValue::Symbol(symbol)
                        if symbol == "registration_threshold"
                )
        )
    }));
}

#[test]
fn count_predicate_fixture_is_structured() {
    let ast = parse_green_when("count(cbu_evidence where state = APPROVED) >= 2")
        .expect("count predicate parses");

    let Predicate::Count {
        set,
        condition,
        op,
        threshold,
    } = ast
    else {
        panic!("expected Count predicate");
    };

    assert_eq!(set.kind, "cbu_evidence");
    assert_eq!(op, CmpOp::Ge);
    assert_eq!(threshold, 2);
    assert!(matches!(
        condition.as_deref(),
        Some(Predicate::StateIn {
            entity: EntityRef::This,
            state_set,
        }) if state_set == &vec!["APPROVED".to_string()]
    ));
}

#[test]
fn obtained_predicate_fixture_is_structured() {
    let ast = parse_green_when("obtained(kyc_case.state in {APPROVED, ACTIVE})")
        .expect("obtained predicate parses");

    assert!(matches!(
        ast,
        Predicate::Obtained {
            entity: EntityRef::Named(ref entity),
            validity: Validity::StateIn(ref states),
        } if entity == "kyc_case"
            && states == &vec!["APPROVED".to_string(), "ACTIVE".to_string()]
    ));
}

#[test]
fn orphaned_attached_to_scope_has_specific_parse_error() {
    let err = parse_green_when("attached_to this clearance").expect_err("orphaned scope fails");

    assert!(err.message.contains("orphaned `attached_to`"), "{err:?}");
}
