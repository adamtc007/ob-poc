use std::collections::BTreeMap;

use crate::{
    config::predicate::{
        parse_green_when, AttrValue, CmpOp, EntityRef as PredicateEntityRef, EntitySetRef,
        Predicate, Validity,
    },
    resolver::{ResolvedSlot, ResolvedTemplate},
};

use super::{
    EntityRef, GreenWhenStatus, InstanceFrontier, InvalidFact, MissingFact, ReachableDestination,
};

/// Synthetic fact set used by the Phase 3 skeleton hydrator.
pub type FrontierFacts = BTreeMap<String, Vec<FrontierFact>>;

/// One bound predicate fact for a synthetic substrate entity.
#[derive(Debug, Clone, PartialEq, Eq, Default)]
pub struct FrontierFact {
    pub state: Option<String>,
    pub attrs: BTreeMap<String, String>,
}

#[derive(Debug, thiserror::Error)]
pub enum HydrateFrontierError {
    #[error("slot not found in resolved template: {0}")]
    SlotNotFound(String),
}

pub fn hydrate_frontier(
    entity_ref: EntityRef,
    resolved_template: &ResolvedTemplate,
) -> Result<InstanceFrontier, HydrateFrontierError> {
    let slot = resolved_template
        .slot(&entity_ref.slot_id)
        .ok_or_else(|| HydrateFrontierError::SlotNotFound(entity_ref.slot_id.clone()))?;

    let reachable = resolved_template
        .transitions
        .iter()
        .filter(|transition| {
            transition.slot_id == slot.id && transition.from == entity_ref.current_state
        })
        .map(|transition| ReachableDestination {
            destination_state: transition.to.clone(),
            via_verb: transition.via.clone(),
            status: evaluate_destination(
                slot,
                &entity_ref,
                transition.destination_green_when.as_deref(),
            ),
        })
        .collect();

    Ok(InstanceFrontier {
        current_state: entity_ref.current_state.clone(),
        entity_ref,
        reachable,
    })
}

fn evaluate_destination(
    slot: &ResolvedSlot,
    entity_ref: &EntityRef,
    green_when: Option<&str>,
) -> GreenWhenStatus {
    let Some(green_when) = green_when.filter(|value| !value.trim().is_empty()) else {
        return GreenWhenStatus::Green;
    };

    let bound_entities = slot
        .predicate_bindings
        .iter()
        .map(|binding| binding.entity.as_str())
        .collect::<Vec<_>>();

    match parse_green_when(green_when) {
        Ok(predicate) => {
            let mut ctx = EvalContext {
                root_entity_id: &entity_ref.entity_id,
                current_state: &entity_ref.current_state,
                facts: &entity_ref.facts,
                missing: Vec::new(),
                invalid: Vec::new(),
            };
            if eval_predicate(&predicate, &mut ctx, None) {
                GreenWhenStatus::Green
            } else {
                if ctx.missing.is_empty() && ctx.invalid.is_empty() {
                    ctx.invalid.push(InvalidFact {
                        entity: "predicate".to_string(),
                        reason: "predicate evaluated false".to_string(),
                    });
                }
                GreenWhenStatus::Red {
                    missing: ctx.missing,
                    invalid: ctx.invalid,
                }
            }
        }
        Err(err) => GreenWhenStatus::Red {
            missing: Vec::new(),
            invalid: vec![InvalidFact {
                entity: bound_entities.join(","),
                reason: err.to_string(),
            }],
        },
    }
}

struct EvalContext<'a> {
    root_entity_id: &'a str,
    current_state: &'a str,
    facts: &'a FrontierFacts,
    missing: Vec<MissingFact>,
    invalid: Vec<InvalidFact>,
}

fn eval_predicate(
    predicate: &Predicate,
    ctx: &mut EvalContext<'_>,
    this_fact: Option<&FrontierFact>,
) -> bool {
    match predicate {
        Predicate::And(items) => items
            .iter()
            .all(|item| eval_predicate(item, ctx, this_fact)),
        Predicate::Exists { entity } => eval_exists(entity, ctx, this_fact),
        Predicate::StateIn { entity, state_set } => {
            eval_state_in(entity, state_set, ctx, this_fact)
        }
        Predicate::AttrCmp {
            entity,
            attr,
            op,
            value,
        } => eval_attr_cmp(entity, attr, *op, value, ctx, this_fact),
        Predicate::Every { set, condition } => {
            let facts = set_facts(set, ctx);
            if has_cycle(ctx, set) {
                return false;
            }
            if facts.is_empty() {
                ctx.missing.push(MissingFact {
                    entity: set.kind.clone(),
                    reason: "set is empty".to_string(),
                });
                return false;
            }
            facts
                .iter()
                .all(|fact| eval_predicate(condition, ctx, Some(fact)))
        }
        Predicate::NoneExists { set, condition } => {
            let facts = set_facts(set, ctx);
            !has_cycle(ctx, set)
                && facts
                    .iter()
                    .all(|fact| !eval_predicate(condition, ctx, Some(fact)))
        }
        Predicate::AtLeastOne { set, condition } => {
            let facts = set_facts(set, ctx);
            if has_cycle(ctx, set) {
                return false;
            }
            let matched = facts
                .iter()
                .any(|fact| eval_predicate(condition, ctx, Some(fact)));
            if !matched {
                ctx.missing.push(MissingFact {
                    entity: set.kind.clone(),
                    reason: "no set member satisfied condition".to_string(),
                });
            }
            matched
        }
        Predicate::Count {
            set,
            condition,
            op,
            threshold,
        } => {
            let facts = set_facts(set, ctx);
            if has_cycle(ctx, set) {
                return false;
            }
            let count = facts
                .iter()
                .filter(|fact| {
                    condition
                        .as_ref()
                        .is_none_or(|condition| eval_predicate(condition, ctx, Some(fact)))
                })
                .count() as u64;
            cmp_u64(count, *op, *threshold)
        }
        Predicate::Obtained { entity, validity } => match validity {
            Validity::StateIn(state_set) => eval_state_in(entity, state_set, ctx, this_fact),
            Validity::DelegatedToEntityDag => eval_exists(entity, ctx, this_fact),
        },
    }
}

fn eval_exists(
    entity: &PredicateEntityRef,
    ctx: &mut EvalContext<'_>,
    this_fact: Option<&FrontierFact>,
) -> bool {
    match entity {
        PredicateEntityRef::This => this_fact.is_some() || !ctx.current_state.is_empty(),
        PredicateEntityRef::Named(kind)
        | PredicateEntityRef::Parent(kind)
        | PredicateEntityRef::Scoped { kind, .. } => {
            let exists = ctx.facts.get(kind).is_some_and(|facts| !facts.is_empty());
            if !exists {
                ctx.missing.push(MissingFact {
                    entity: kind.clone(),
                    reason: "no bound fact found".to_string(),
                });
            }
            exists
        }
    }
}

fn eval_state_in(
    entity: &PredicateEntityRef,
    state_set: &[String],
    ctx: &mut EvalContext<'_>,
    this_fact: Option<&FrontierFact>,
) -> bool {
    match entity {
        PredicateEntityRef::This => {
            let state = this_fact
                .and_then(|fact| fact.state.as_deref())
                .unwrap_or(ctx.current_state);
            let ok = state_set.iter().any(|allowed| allowed == state);
            if !ok {
                ctx.invalid.push(InvalidFact {
                    entity: "this".to_string(),
                    reason: format!("state {state} not in {state_set:?}"),
                });
            }
            ok
        }
        PredicateEntityRef::Named(kind)
        | PredicateEntityRef::Parent(kind)
        | PredicateEntityRef::Scoped { kind, .. } => {
            let facts = ctx.facts.get(kind).cloned().unwrap_or_default();
            if facts.is_empty() {
                ctx.missing.push(MissingFact {
                    entity: kind.clone(),
                    reason: "no bound fact found".to_string(),
                });
                return false;
            }
            let ok = facts.iter().any(|fact| {
                fact.state
                    .as_ref()
                    .is_some_and(|state| state_set.iter().any(|allowed| allowed == state))
            });
            if !ok {
                ctx.invalid.push(InvalidFact {
                    entity: kind.clone(),
                    reason: format!("no fact state in {state_set:?}"),
                });
            }
            ok
        }
    }
}

fn eval_attr_cmp(
    entity: &PredicateEntityRef,
    attr: &str,
    op: CmpOp,
    value: &AttrValue,
    ctx: &mut EvalContext<'_>,
    this_fact: Option<&FrontierFact>,
) -> bool {
    let expected = attr_value_to_string(value);
    let values = match entity {
        PredicateEntityRef::This => this_fact
            .and_then(|fact| fact.attrs.get(attr))
            .into_iter()
            .cloned()
            .collect::<Vec<_>>(),
        PredicateEntityRef::Named(kind)
        | PredicateEntityRef::Parent(kind)
        | PredicateEntityRef::Scoped { kind, .. } => ctx
            .facts
            .get(kind)
            .into_iter()
            .flatten()
            .filter_map(|fact| fact.attrs.get(attr).cloned())
            .collect(),
    };
    let ok = values
        .iter()
        .any(|actual| cmp_string_or_number(actual, op, &expected));
    if !ok {
        ctx.invalid.push(InvalidFact {
            entity: entity_name(entity),
            reason: format!("attribute {attr} did not satisfy comparison"),
        });
    }
    ok
}

fn set_facts(set: &EntitySetRef, ctx: &mut EvalContext<'_>) -> Vec<FrontierFact> {
    let Some(facts) = ctx.facts.get(&set.kind) else {
        return Vec::new();
    };
    if !facts
        .iter()
        .any(|fact| fact.attrs.contains_key("parent_id"))
    {
        return facts.clone();
    }

    let mut out = Vec::new();
    let mut path = Vec::new();
    collect_recursive_set(
        &set.kind,
        ctx.root_entity_id,
        facts,
        &mut path,
        &mut out,
        &mut ctx.invalid,
    );
    out
}

fn has_cycle(ctx: &EvalContext<'_>, set: &EntitySetRef) -> bool {
    ctx.invalid
        .iter()
        .any(|invalid| invalid.entity == set.kind && invalid.reason.starts_with("CycleDetected"))
}

fn collect_recursive_set(
    kind: &str,
    parent_id: &str,
    facts: &[FrontierFact],
    path: &mut Vec<String>,
    out: &mut Vec<FrontierFact>,
    invalid: &mut Vec<InvalidFact>,
) {
    for fact in facts.iter().filter(|fact| {
        fact.attrs
            .get("parent_id")
            .is_some_and(|value| value == parent_id)
    }) {
        let Some(id) = fact.attrs.get("id").cloned() else {
            invalid.push(InvalidFact {
                entity: kind.to_string(),
                reason: "recursive fact missing id".to_string(),
            });
            continue;
        };
        if path.contains(&id) {
            let mut cycle = path.clone();
            cycle.push(id);
            invalid.push(InvalidFact {
                entity: kind.to_string(),
                reason: format!("CycleDetected{{path:{cycle:?}}}"),
            });
            continue;
        }

        out.push(fact.clone());
        path.push(id.clone());
        collect_recursive_set(kind, &id, facts, path, out, invalid);
        path.pop();
    }
}

fn cmp_u64(left: u64, op: CmpOp, right: u64) -> bool {
    match op {
        CmpOp::Eq => left == right,
        CmpOp::Ne => left != right,
        CmpOp::Lt => left < right,
        CmpOp::Le => left <= right,
        CmpOp::Gt => left > right,
        CmpOp::Ge => left >= right,
    }
}

fn cmp_string_or_number(left: &str, op: CmpOp, right: &str) -> bool {
    match (left.parse::<f64>(), right.parse::<f64>()) {
        (Ok(left), Ok(right)) => match op {
            CmpOp::Eq => left == right,
            CmpOp::Ne => left != right,
            CmpOp::Lt => left < right,
            CmpOp::Le => left <= right,
            CmpOp::Gt => left > right,
            CmpOp::Ge => left >= right,
        },
        _ => match op {
            CmpOp::Eq => left == right,
            CmpOp::Ne => left != right,
            CmpOp::Lt => left < right,
            CmpOp::Le => left <= right,
            CmpOp::Gt => left > right,
            CmpOp::Ge => left >= right,
        },
    }
}

fn attr_value_to_string(value: &AttrValue) -> String {
    match value {
        AttrValue::String(value) | AttrValue::Number(value) | AttrValue::Symbol(value) => {
            value.clone()
        }
        AttrValue::Bool(value) => value.to_string(),
    }
}

fn entity_name(entity: &PredicateEntityRef) -> String {
    match entity {
        PredicateEntityRef::This => "this".to_string(),
        PredicateEntityRef::Named(kind)
        | PredicateEntityRef::Parent(kind)
        | PredicateEntityRef::Scoped { kind, .. } => kind.clone(),
    }
}
