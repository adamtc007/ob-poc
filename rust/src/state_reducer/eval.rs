use std::collections::HashMap;

use super::aggregate::{compare_field, resolve_value};
use super::ast::{
    AggFn, ConditionBody, Expr, FieldValue, Literal, ScopeData, SlotField, SlotOverlayData,
    SlotPredicate, Value,
};
use super::error::{ReducerError, ReducerResult};
use super::state_machine::{RuleDef, ValidatedStateMachine};

/// Evaluator for parsed reducer conditions.
#[derive(Debug)]
pub struct ConditionEvaluator<'a> {
    pub eval_order: &'a [String],
    pub asts: &'a HashMap<String, ConditionBody>,
    pub results: HashMap<String, bool>,
}

impl<'a> ConditionEvaluator<'a> {
    /// Create a new evaluator from validated condition ASTs.
    ///
    /// # Examples
    /// ```rust
    /// use std::collections::HashMap;
    /// use ob_poc::state_reducer::{ConditionBody, ConditionEvaluator};
    ///
    /// let asts = HashMap::<String, ConditionBody>::new();
    /// let order = Vec::<String>::new();
    /// let evaluator = ConditionEvaluator::new(&order, &asts);
    /// assert!(evaluator.results.is_empty());
    /// ```
    pub fn new(eval_order: &'a [String], asts: &'a HashMap<String, ConditionBody>) -> Self {
        Self {
            eval_order,
            asts,
            results: HashMap::new(),
        }
    }

    /// Evaluate all named conditions against the provided data.
    ///
    /// # Examples
    /// ```rust
    /// use std::collections::HashMap;
    /// use ob_poc::state_reducer::{ConditionBody, ConditionEvaluator, ScopeData, SlotOverlayData};
    ///
    /// let asts = HashMap::<String, ConditionBody>::new();
    /// let order = Vec::<String>::new();
    /// let mut evaluator = ConditionEvaluator::new(&order, &asts);
    /// let data = SlotOverlayData { sources: HashMap::new(), scope: ScopeData { fields: serde_json::json!({}) }, slots: vec![] };
    /// evaluator.evaluate_all(&data).unwrap();
    /// ```
    pub fn evaluate_all(&mut self, data: &SlotOverlayData) -> ReducerResult<HashMap<String, bool>> {
        for name in self.eval_order {
            let value = self.evaluate_named(name, data, &[])?;
            self.results.insert(name.clone(), value);
        }
        Ok(self.results.clone())
    }

    fn evaluate_named(
        &mut self,
        name: &str,
        data: &SlotOverlayData,
        bindings: &[Literal],
    ) -> ReducerResult<bool> {
        if let Some(value) = self.results.get(name) {
            return Ok(*value);
        }

        let body = self
            .asts
            .get(name)
            .ok_or_else(|| ReducerError::Evaluation(format!("unknown condition '{name}'")))?;
        let value = self.evaluate_body(body, data, bindings)?;
        if bindings.is_empty() {
            self.results.insert(name.to_string(), value);
        }
        Ok(value)
    }

    fn evaluate_body(
        &mut self,
        body: &ConditionBody,
        data: &SlotOverlayData,
        bindings: &[Literal],
    ) -> ReducerResult<bool> {
        match body {
            ConditionBody::Leaf { expr, compare } => {
                self.evaluate_leaf(expr, compare.as_ref(), data, bindings)
            }
            ConditionBody::Ref { name } => self.evaluate_named(name, data, bindings),
            ConditionBody::Call { name, args } => {
                let resolved = args
                    .iter()
                    .map(|arg| {
                        resolve_value(arg, bindings).ok_or_else(|| {
                            ReducerError::Evaluation(format!("unbound parameter in call '{name}'"))
                        })
                    })
                    .collect::<ReducerResult<Vec<_>>>()?;
                let body = self.asts.get(name).ok_or_else(|| {
                    ReducerError::Evaluation(format!("unknown condition '{name}'"))
                })?;
                self.evaluate_body(body, data, &resolved)
            }
            ConditionBody::And(items) => items.iter().try_fold(true, |acc, item| {
                if !acc {
                    return Ok(false);
                }
                self.evaluate_body(item, data, bindings)
            }),
            ConditionBody::Or(items) => items.iter().try_fold(false, |acc, item| {
                if acc {
                    return Ok(true);
                }
                self.evaluate_body(item, data, bindings)
            }),
            ConditionBody::Not(item) => Ok(!self.evaluate_body(item, data, bindings)?),
        }
    }

    fn evaluate_leaf(
        &self,
        expr: &Expr,
        compare: Option<&(super::ast::CompareOp, Value)>,
        data: &SlotOverlayData,
        bindings: &[Literal],
    ) -> ReducerResult<bool> {
        match expr {
            Expr::Existence {
                source,
                field,
                negated,
            } => {
                let empty_rows = Vec::new();
                let rows = data.sources.get(source).unwrap_or(&empty_rows);
                let exists = if field.is_empty() {
                    !rows.is_empty()
                } else {
                    rows.iter().any(|row| {
                        row.fields
                            .get(field)
                            .is_some_and(|value| !matches!(value, FieldValue::Null))
                    })
                };
                Ok(if *negated { !exists } else { exists })
            }
            Expr::Comparison {
                source,
                field,
                op,
                value,
            } => {
                let empty_rows = Vec::new();
                let rows = data.sources.get(source).unwrap_or(&empty_rows);
                Ok(rows.iter().any(|row| {
                    row.fields
                        .get(field)
                        .is_some_and(|lhs| compare_field(lhs, *op, resolve_value(value, bindings)))
                }))
            }
            Expr::ScopeComparison { path, op, value } => {
                let Some(lhs) = resolve_scope_path(&data.scope, path) else {
                    return Ok(false);
                };
                Ok(compare_field(&lhs, *op, resolve_value(value, bindings)))
            }
            Expr::Aggregate {
                function,
                source,
                filter,
            } => {
                let empty_rows = Vec::new();
                let rows = data.sources.get(source).unwrap_or(&empty_rows);
                let result = super::aggregate::evaluate_aggregate(
                    *function,
                    rows,
                    filter.as_ref(),
                    bindings,
                );
                Ok(match compare {
                    Some((op, value)) => {
                        let literal = resolve_value(value, bindings).ok_or_else(|| {
                            ReducerError::Evaluation(
                                "unbound aggregate comparison parameter".into(),
                            )
                        })?;
                        result.compare(*op, &literal)
                    }
                    None => result.as_bool(),
                })
            }
            Expr::SlotAggregate { function, filter } => {
                let matched = data
                    .slots
                    .iter()
                    .filter(|slot| eval_slot_predicate(filter, slot, bindings))
                    .count();
                Ok(match function {
                    AggFn::Count => matched > 0,
                    AggFn::Any => matched > 0,
                    AggFn::All => data.slots.is_empty() || matched == data.slots.len(),
                })
            }
        }
    }
}

/// Evaluate reducer rules using first-match-wins semantics.
///
/// # Examples
/// ```rust
/// use std::collections::HashMap;
/// use ob_poc::state_reducer::{evaluate_rules, RuleDef};
///
/// let rules = vec![RuleDef { state: String::from("ready"), requires: vec![String::from("ok")], excludes: vec![], consistency_check: None }];
/// let results = HashMap::from([(String::from("ok"), true)]);
/// assert_eq!(evaluate_rules(&rules, &results).unwrap(), "ready");
/// ```
pub fn evaluate_rules(rules: &[RuleDef], results: &HashMap<String, bool>) -> ReducerResult<String> {
    for rule in rules {
        let requires_ok = rule
            .requires
            .iter()
            .all(|name| results.get(name).copied().unwrap_or(false));
        let excludes_ok = rule
            .excludes
            .iter()
            .all(|name| !results.get(name).copied().unwrap_or(false));
        if requires_ok && excludes_ok {
            return Ok(rule.state.clone());
        }
    }
    Err(ReducerError::Evaluation(
        "no reducer rule matched the evaluated condition set".into(),
    ))
}

impl ValidatedStateMachine {
    /// Evaluate the validated state machine against a data bundle.
    ///
    /// # Examples
    /// ```rust
    /// use std::collections::HashMap;
    /// use ob_poc::state_reducer::{ConditionBody, ScopeData, SlotOverlayData, ValidatedStateMachine};
    ///
    /// let machine = ValidatedStateMachine {
    ///     name: String::from("demo"),
    ///     states: vec![String::from("empty")],
    ///     initial: String::from("empty"),
    ///     transitions: vec![],
    ///     conditions: HashMap::<String, ConditionBody>::new(),
    ///     eval_order: vec![],
    ///     rules: vec![],
    ///     overlay_sources: HashMap::new(),
    ///     reducer_revision: String::from("0000000000000000"),
    /// };
    /// let data = SlotOverlayData { sources: HashMap::new(), scope: ScopeData { fields: serde_json::json!({}) }, slots: vec![] };
    /// let _ = machine.evaluate(&data);
    /// ```
    pub fn evaluate(&self, data: &SlotOverlayData) -> ReducerResult<String> {
        let mut evaluator = ConditionEvaluator::new(&self.eval_order, &self.conditions);
        let results = evaluator.evaluate_all(data)?;
        evaluate_rules(&self.rules, &results)
    }
}

fn resolve_scope_path(scope: &ScopeData, path: &[String]) -> Option<FieldValue> {
    let mut current = &scope.fields;
    for segment in path {
        current = current.get(segment)?;
    }

    match current {
        serde_json::Value::String(value) => Some(FieldValue::Str(value.clone())),
        serde_json::Value::Number(value) => value.as_f64().map(FieldValue::Num),
        serde_json::Value::Bool(value) => Some(FieldValue::Bool(*value)),
        serde_json::Value::Null => Some(FieldValue::Null),
        _ => None,
    }
}

fn eval_slot_predicate(
    predicate: &SlotPredicate,
    slot: &super::ast::SlotRecord,
    bindings: &[Literal],
) -> bool {
    match predicate {
        SlotPredicate::Atom { field, op, value } => {
            let lhs = match field {
                SlotField::Type => FieldValue::Str(slot.slot_type.clone()),
                SlotField::Cardinality => FieldValue::Str(slot.cardinality.clone()),
                SlotField::EffectiveState => FieldValue::Str(slot.effective_state.clone()),
                SlotField::ComputedState => FieldValue::Str(slot.computed_state.clone()),
            };
            compare_field(&lhs, *op, resolve_value(value, bindings))
        }
        SlotPredicate::And(items) => items
            .iter()
            .all(|item| eval_slot_predicate(item, slot, bindings)),
        SlotPredicate::Or(items) => items
            .iter()
            .any(|item| eval_slot_predicate(item, slot, bindings)),
        SlotPredicate::Not(item) => !eval_slot_predicate(item, slot, bindings),
    }
}
