use super::ast::{AggFn, AggResult, CompareOp, FieldValue, Literal, OverlayRow, Predicate, Value};

/// Evaluate an aggregate over overlay rows.
///
/// # Examples
/// ```rust
/// use std::collections::HashMap;
/// use ob_poc::state_reducer::{
///     evaluate_aggregate, AggFn, AggResult, FieldValue, OverlayRow
/// };
///
/// let row = OverlayRow { fields: HashMap::from([(String::from("status"), FieldValue::Str(String::from("CLEAR")))]) };
/// assert_eq!(evaluate_aggregate(AggFn::Count, &[row], None, &[]), AggResult::Count(1));
/// ```
pub fn evaluate_aggregate(
    function: AggFn,
    rows: &[OverlayRow],
    filter: Option<&Predicate>,
    bindings: &[Literal],
) -> AggResult {
    let filtered: Vec<&OverlayRow> = rows
        .iter()
        .filter(|row| filter.is_none_or(|predicate| eval_predicate(predicate, row, bindings)))
        .collect();

    match function {
        AggFn::Count => AggResult::Count(filtered.len()),
        AggFn::Any => AggResult::Boolean(!filtered.is_empty()),
        AggFn::All => AggResult::Boolean(rows.is_empty() || filtered.len() == rows.len()),
    }
}

fn eval_predicate(predicate: &Predicate, row: &OverlayRow, bindings: &[Literal]) -> bool {
    match predicate {
        Predicate::Atom { field, op, value } => row
            .fields
            .get(field)
            .is_some_and(|lhs| compare_field(lhs, *op, resolve_value(value, bindings))),
        Predicate::IsNull { field, negated } => row
            .fields
            .get(field)
            .map(|lhs| matches!(lhs, FieldValue::Null) ^ *negated)
            .unwrap_or(!negated),
        Predicate::And(predicates) => predicates
            .iter()
            .all(|predicate| eval_predicate(predicate, row, bindings)),
        Predicate::Or(predicates) => predicates
            .iter()
            .any(|predicate| eval_predicate(predicate, row, bindings)),
        Predicate::Not(predicate) => !eval_predicate(predicate, row, bindings),
    }
}

pub(crate) fn compare_field(lhs: &FieldValue, op: CompareOp, rhs: Option<Literal>) -> bool {
    let Some(rhs) = rhs else {
        return false;
    };

    match (lhs, rhs) {
        (FieldValue::Str(lhs), Literal::Str(rhs)) => compare_string(lhs, op, &rhs),
        (FieldValue::Num(lhs), Literal::Num(rhs)) => compare_number(*lhs, op, rhs),
        (FieldValue::Bool(lhs), Literal::Bool(rhs)) => compare_bool(*lhs, op, rhs),
        (FieldValue::Null, Literal::Null) => matches!(op, CompareOp::Eq),
        (FieldValue::Str(lhs), Literal::List(items)) => compare_membership(lhs, op, &items),
        _ => false,
    }
}

pub(crate) fn resolve_value(value: &Value, bindings: &[Literal]) -> Option<Literal> {
    match value {
        Value::Literal(literal) => Some(literal.clone()),
        Value::Param(index) => bindings.get(index - 1).cloned(),
    }
}

fn compare_string(lhs: &str, op: CompareOp, rhs: &str) -> bool {
    match op {
        CompareOp::Eq => lhs == rhs,
        CompareOp::Neq => lhs != rhs,
        CompareOp::Like => lhs.contains(rhs),
        CompareOp::Gt => lhs > rhs,
        CompareOp::Gte => lhs >= rhs,
        CompareOp::Lt => lhs < rhs,
        CompareOp::Lte => lhs <= rhs,
        CompareOp::In | CompareOp::NotIn => false,
    }
}

fn compare_number(lhs: f64, op: CompareOp, rhs: f64) -> bool {
    match op {
        CompareOp::Eq => lhs == rhs,
        CompareOp::Neq => lhs != rhs,
        CompareOp::Gt => lhs > rhs,
        CompareOp::Gte => lhs >= rhs,
        CompareOp::Lt => lhs < rhs,
        CompareOp::Lte => lhs <= rhs,
        CompareOp::In | CompareOp::NotIn | CompareOp::Like => false,
    }
}

fn compare_bool(lhs: bool, op: CompareOp, rhs: bool) -> bool {
    match op {
        CompareOp::Eq => lhs == rhs,
        CompareOp::Neq => lhs != rhs,
        CompareOp::Gt
        | CompareOp::Gte
        | CompareOp::Lt
        | CompareOp::Lte
        | CompareOp::In
        | CompareOp::NotIn
        | CompareOp::Like => false,
    }
}

fn compare_membership(lhs: &str, op: CompareOp, items: &[Literal]) -> bool {
    let contains = items
        .iter()
        .any(|item| matches!(item, Literal::Str(value) if value == lhs));
    match op {
        CompareOp::In => contains,
        CompareOp::NotIn => !contains,
        CompareOp::Eq
        | CompareOp::Neq
        | CompareOp::Gt
        | CompareOp::Gte
        | CompareOp::Lt
        | CompareOp::Lte
        | CompareOp::Like => false,
    }
}
