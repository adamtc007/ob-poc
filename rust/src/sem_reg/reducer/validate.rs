use std::collections::{HashMap, HashSet};

use anyhow::anyhow;

use super::ast::{AggFn, ConditionBody, Expr, Predicate, SlotPredicate, Value};
use super::error::{ReducerError, ReducerResult};
use super::parser::parse_condition_body;
use super::state_machine::{OverlaySourceDef, StateMachineDefinition, ValidatedStateMachine};

/// Parse and validate a reducer state machine definition.
///
/// # Examples
/// ```rust
/// use std::collections::HashMap;
/// use ob_poc::sem_reg::reducer::{validate_state_machine, ConditionDef, ReducerDef, RuleDef, StateMachineDefinition};
///
/// let definition = StateMachineDefinition {
///     state_machine: String::from("demo"),
///     description: None,
///     states: vec![String::from("empty")],
///     initial: String::from("empty"),
///     transitions: vec![],
///     reducer: ReducerDef {
///         overlay_sources: HashMap::new(),
///         conditions: HashMap::from([(String::from("ok"), ConditionDef { expr: String::from("scope.ready = true"), description: None, parameterized: false })]),
///         rules: vec![RuleDef { state: String::from("empty"), requires: vec![], excludes: vec![], consistency_check: None }],
///     },
/// };
/// let validated = validate_state_machine(&definition).unwrap();
/// assert_eq!(validated.name, "demo");
/// ```
pub fn validate_state_machine(
    definition: &StateMachineDefinition,
) -> ReducerResult<ValidatedStateMachine> {
    let mut conditions = HashMap::new();
    for (name, def) in &definition.reducer.conditions {
        let parsed = parse_condition_body(&def.expr)?;
        conditions.insert(name.clone(), parsed);
    }

    validate_condition_graph(&conditions, &definition.reducer.overlay_sources, definition)?;
    validate_rules(definition)?;
    let eval_order = topological_order(&conditions)?;

    Ok(ValidatedStateMachine {
        name: definition.state_machine.clone(),
        states: definition.states.clone(),
        initial: definition.initial.clone(),
        transitions: definition.transitions.clone(),
        conditions,
        eval_order,
        rules: definition.reducer.rules.clone(),
        overlay_sources: definition.reducer.overlay_sources.clone(),
        reducer_revision: String::new(),
    })
}

fn validate_condition_graph(
    conditions: &HashMap<String, ConditionBody>,
    overlay_sources: &HashMap<String, OverlaySourceDef>,
    definition: &StateMachineDefinition,
) -> ReducerResult<()> {
    for (name, body) in conditions {
        validate_body(name, body, conditions, overlay_sources)?;
        let max_param = max_param_index(body);
        let declared_parameterized = definition
            .reducer
            .conditions
            .get(name)
            .map(|condition| condition.parameterized)
            .unwrap_or(false);
        if max_param > 0 && !declared_parameterized {
            return Err(ReducerError::Validation(format!(
                "condition '{name}' uses parameters but is not marked parameterized"
            )));
        }
        if max_param == 0 && declared_parameterized {
            return Err(ReducerError::Validation(format!(
                "condition '{name}' is marked parameterized but has no parameter references"
            )));
        }
    }
    detect_cycles(conditions)?;
    Ok(())
}

fn validate_body(
    condition_name: &str,
    body: &ConditionBody,
    conditions: &HashMap<String, ConditionBody>,
    overlay_sources: &HashMap<String, OverlaySourceDef>,
) -> ReducerResult<()> {
    match body {
        ConditionBody::Leaf { expr, compare } => {
            validate_expr(condition_name, expr, compare.as_ref(), overlay_sources)?;
        }
        ConditionBody::Ref { name } => {
            if !conditions.contains_key(name) {
                return Err(ReducerError::Validation(format!(
                    "condition '{condition_name}' references unknown condition '{name}'"
                )));
            }
        }
        ConditionBody::Call { name, args } => {
            let Some(target) = conditions.get(name) else {
                return Err(ReducerError::Validation(format!(
                    "condition '{condition_name}' calls unknown condition '{name}'"
                )));
            };
            let expected = max_param_index(target);
            if expected != args.len() {
                return Err(ReducerError::Validation(format!(
                    "condition '{condition_name}' calls '{name}' with {} args but expected {expected}",
                    args.len()
                )));
            }
            for arg in args {
                validate_value(arg, expected)?;
            }
        }
        ConditionBody::And(items) | ConditionBody::Or(items) => {
            for item in items {
                validate_body(condition_name, item, conditions, overlay_sources)?;
            }
        }
        ConditionBody::Not(item) => {
            validate_body(condition_name, item, conditions, overlay_sources)?;
        }
    }
    Ok(())
}

fn validate_expr(
    condition_name: &str,
    expr: &Expr,
    compare: Option<&(super::ast::CompareOp, Value)>,
    overlay_sources: &HashMap<String, OverlaySourceDef>,
) -> ReducerResult<()> {
    match expr {
        Expr::Existence { source, field, .. } => {
            validate_overlay_field(condition_name, source, field, overlay_sources)?;
        }
        Expr::Comparison {
            source,
            field,
            value,
            ..
        } => {
            validate_overlay_field(condition_name, source, field, overlay_sources)?;
            validate_value(value, usize::MAX)?;
        }
        Expr::Aggregate {
            function,
            source,
            filter,
        } => {
            validate_overlay_source(condition_name, source, overlay_sources)?;
            if let Some(predicate) = filter {
                validate_predicate(condition_name, source, predicate, overlay_sources)?;
            }
            match function {
                AggFn::Count if compare.is_none() => {
                    return Err(ReducerError::Validation(format!(
                        "condition '{condition_name}' uses COUNT without a comparison"
                    )));
                }
                AggFn::Any | AggFn::All if compare.is_some() => {
                    return Err(ReducerError::Validation(format!(
                        "condition '{condition_name}' uses {:?} with an explicit comparison",
                        function
                    )));
                }
                AggFn::Count | AggFn::Any | AggFn::All => {}
            }
            if let Some((_, value)) = compare {
                validate_value(value, usize::MAX)?;
            }
        }
        Expr::ScopeComparison { value, .. } => validate_value(value, usize::MAX)?,
        Expr::SlotAggregate { filter, .. } => validate_slot_predicate(filter, usize::MAX)?,
    }
    Ok(())
}

fn validate_overlay_source(
    condition_name: &str,
    source: &str,
    overlay_sources: &HashMap<String, OverlaySourceDef>,
) -> ReducerResult<()> {
    if overlay_sources.contains_key(source) {
        Ok(())
    } else {
        Err(ReducerError::Validation(format!(
            "condition '{condition_name}' references unknown overlay source '{source}'"
        )))
    }
}

fn validate_overlay_field(
    condition_name: &str,
    source: &str,
    field: &str,
    overlay_sources: &HashMap<String, OverlaySourceDef>,
) -> ReducerResult<()> {
    validate_overlay_source(condition_name, source, overlay_sources)?;
    if field.is_empty() {
        return Ok(());
    }
    let provides = &overlay_sources
        .get(source)
        .ok_or_else(|| anyhow!("missing source after validation"))?
        .provides;
    if provides.iter().any(|provided| provided == field) {
        Ok(())
    } else {
        Err(ReducerError::Validation(format!(
            "condition '{condition_name}' references unknown field '{source}.{field}'"
        )))
    }
}

fn validate_predicate(
    condition_name: &str,
    source: &str,
    predicate: &Predicate,
    overlay_sources: &HashMap<String, OverlaySourceDef>,
) -> ReducerResult<()> {
    match predicate {
        Predicate::Atom { field, value, .. } => {
            validate_overlay_field(condition_name, source, field, overlay_sources)?;
            validate_value(value, usize::MAX)?;
        }
        Predicate::IsNull { field, .. } => {
            validate_overlay_field(condition_name, source, field, overlay_sources)?;
        }
        Predicate::And(items) | Predicate::Or(items) => {
            for item in items {
                validate_predicate(condition_name, source, item, overlay_sources)?;
            }
        }
        Predicate::Not(item) => validate_predicate(condition_name, source, item, overlay_sources)?,
    }
    Ok(())
}

fn validate_slot_predicate(predicate: &SlotPredicate, parameter_count: usize) -> ReducerResult<()> {
    match predicate {
        SlotPredicate::Atom { value, .. } => validate_value(value, parameter_count)?,
        SlotPredicate::And(items) | SlotPredicate::Or(items) => {
            for item in items {
                validate_slot_predicate(item, parameter_count)?;
            }
        }
        SlotPredicate::Not(item) => validate_slot_predicate(item, parameter_count)?,
    }
    Ok(())
}

fn validate_value(value: &Value, parameter_count: usize) -> ReducerResult<()> {
    if let Value::Param(index) = value {
        if *index == 0 || *index > parameter_count {
            return Err(ReducerError::Validation(format!(
                "parameter ${index} exceeds declared parameter count {parameter_count}"
            )));
        }
    }
    Ok(())
}

fn validate_rules(definition: &StateMachineDefinition) -> ReducerResult<()> {
    let mut seen = HashSet::new();
    for rule in &definition.reducer.rules {
        if !seen.insert(rule.state.clone()) {
            return Err(ReducerError::Validation(format!(
                "duplicate reducer rule for state '{}'",
                rule.state
            )));
        }
        for condition in &rule.requires {
            if !definition.reducer.conditions.contains_key(condition) {
                return Err(ReducerError::Validation(format!(
                    "rule for state '{}' references unknown required condition '{}'",
                    rule.state, condition
                )));
            }
        }
        for condition in &rule.excludes {
            if !definition.reducer.conditions.contains_key(condition) {
                return Err(ReducerError::Validation(format!(
                    "rule for state '{}' references unknown excluded condition '{}'",
                    rule.state, condition
                )));
            }
        }
        if let Some(check) = &rule.consistency_check {
            if !definition
                .reducer
                .conditions
                .contains_key(&check.warn_unless)
            {
                return Err(ReducerError::Validation(format!(
                    "rule for state '{}' references unknown consistency condition '{}'",
                    rule.state, check.warn_unless
                )));
            }
        }
    }

    let states: HashSet<_> = definition.states.iter().cloned().collect();
    if seen != states {
        let missing: Vec<_> = states.difference(&seen).cloned().collect();
        if !missing.is_empty() {
            return Err(ReducerError::Validation(format!(
                "missing reducer rules for states: {}",
                missing.join(", ")
            )));
        }
    }

    let last = definition
        .reducer
        .rules
        .last()
        .ok_or_else(|| ReducerError::Validation("reducer.rules must not be empty".into()))?;
    if !last.requires.is_empty() {
        return Err(ReducerError::Validation(
            "last reducer rule must be the unconditional fallback".into(),
        ));
    }
    Ok(())
}

fn topological_order(conditions: &HashMap<String, ConditionBody>) -> ReducerResult<Vec<String>> {
    let mut visited = HashSet::new();
    let mut visiting = HashSet::new();
    let mut order = Vec::new();
    for name in conditions.keys() {
        visit(name, conditions, &mut visited, &mut visiting, &mut order)?;
    }
    Ok(order)
}

fn visit(
    name: &str,
    conditions: &HashMap<String, ConditionBody>,
    visited: &mut HashSet<String>,
    visiting: &mut HashSet<String>,
    order: &mut Vec<String>,
) -> ReducerResult<()> {
    if visited.contains(name) {
        return Ok(());
    }
    if !visiting.insert(name.to_string()) {
        return Err(ReducerError::Validation(format!(
            "condition cycle detected at '{name}'"
        )));
    }
    if let Some(body) = conditions.get(name) {
        for dep in dependencies(body) {
            visit(dep, conditions, visited, visiting, order)?;
        }
    }
    visiting.remove(name);
    visited.insert(name.to_string());
    order.push(name.to_string());
    Ok(())
}

fn detect_cycles(conditions: &HashMap<String, ConditionBody>) -> ReducerResult<()> {
    let _ = topological_order(conditions)?;
    Ok(())
}

fn dependencies(body: &ConditionBody) -> Vec<&str> {
    match body {
        ConditionBody::Leaf { .. } => Vec::new(),
        ConditionBody::Ref { name } => vec![name.as_str()],
        ConditionBody::Call { name, .. } => vec![name.as_str()],
        ConditionBody::And(items) | ConditionBody::Or(items) => {
            items.iter().flat_map(dependencies).collect()
        }
        ConditionBody::Not(item) => dependencies(item),
    }
}

fn max_param_index(body: &ConditionBody) -> usize {
    match body {
        ConditionBody::Leaf { expr, compare } => {
            let expr_max = match expr {
                Expr::Existence { .. } => 0,
                Expr::Comparison { value, .. } | Expr::ScopeComparison { value, .. } => {
                    value_max(value)
                }
                Expr::Aggregate { filter, .. } => filter.as_ref().map(predicate_max).unwrap_or(0),
                Expr::SlotAggregate { filter, .. } => slot_predicate_max(filter),
            };
            let compare_max = compare
                .as_ref()
                .map(|(_, value)| value_max(value))
                .unwrap_or(0);
            expr_max.max(compare_max)
        }
        ConditionBody::Ref { .. } => 0,
        ConditionBody::Call { args, .. } => args.iter().map(value_max).max().unwrap_or(0),
        ConditionBody::And(items) | ConditionBody::Or(items) => {
            items.iter().map(max_param_index).max().unwrap_or(0)
        }
        ConditionBody::Not(item) => max_param_index(item),
    }
}

fn predicate_max(predicate: &Predicate) -> usize {
    match predicate {
        Predicate::Atom { value, .. } => value_max(value),
        Predicate::IsNull { .. } => 0,
        Predicate::And(items) | Predicate::Or(items) => {
            items.iter().map(predicate_max).max().unwrap_or(0)
        }
        Predicate::Not(item) => predicate_max(item),
    }
}

fn slot_predicate_max(predicate: &SlotPredicate) -> usize {
    match predicate {
        SlotPredicate::Atom { value, .. } => value_max(value),
        SlotPredicate::And(items) | SlotPredicate::Or(items) => {
            items.iter().map(slot_predicate_max).max().unwrap_or(0)
        }
        SlotPredicate::Not(item) => slot_predicate_max(item),
    }
}

fn value_max(value: &Value) -> usize {
    match value {
        Value::Literal(_) => 0,
        Value::Param(index) => *index,
    }
}
