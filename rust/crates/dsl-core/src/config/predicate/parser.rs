//! Parser for the v1.4 `green_when` free-text convention.

use thiserror::Error;

use super::ast::{
    AttrValue, CmpOp, EntityQualifier, EntityRef, EntitySetRef, Predicate, RelationScope, StateSet,
};

/// Error returned when a `green_when` predicate cannot be parsed.
#[derive(Debug, Clone, PartialEq, Eq, Error)]
#[error("failed to parse green_when clause {clause_index}: {message}: {clause}")]
pub struct ParseError {
    /// One-based clause index after top-level `AND` splitting.
    pub clause_index: usize,
    /// Clause text that failed.
    pub clause: String,
    /// Human-readable parse failure.
    pub message: String,
}

/// Parse a DAG `green_when` predicate into a typed AST.
///
/// # Examples
///
/// ```
/// use dsl_core::config::predicate::{parse_green_when, Predicate};
///
/// let ast = parse_green_when(
///     "board_review exists AND board_review.state = COMPLETE",
/// )
/// .expect("predicate parses");
///
/// assert!(matches!(ast, Predicate::And(_)));
/// ```
pub fn parse_green_when(input: &str) -> Result<Predicate, ParseError> {
    let clauses = split_conjuncts(input);
    let mut predicates = Vec::with_capacity(clauses.len());
    for (idx, clause) in clauses.iter().enumerate() {
        predicates.push(parse_clause(clause).map_err(|message| ParseError {
            clause_index: idx + 1,
            clause: clause.clone(),
            message,
        })?);
    }

    match predicates.len() {
        0 => Err(ParseError {
            clause_index: 0,
            clause: normalize(input),
            message: "predicate is empty".to_string(),
        }),
        1 => Ok(predicates.remove(0)),
        _ => Ok(Predicate::And(predicates)),
    }
}

fn split_conjuncts(input: &str) -> Vec<String> {
    let normalized = normalize(input);
    if normalized.is_empty() {
        return Vec::new();
    }

    let mut clauses: Vec<String> = Vec::new();
    for part in normalized.split(" AND ").map(str::trim) {
        if is_relation_scope_tail(part) {
            if let Some(previous) = clauses.last_mut() {
                previous.push_str(" AND ");
                previous.push_str(part);
            } else {
                clauses.push(part.to_string());
            }
        } else {
            clauses.push(part.to_string());
        }
    }
    clauses
}

fn normalize(input: &str) -> String {
    input.split_whitespace().collect::<Vec<_>>().join(" ")
}

fn is_relation_scope_tail(part: &str) -> bool {
    part.starts_with("attached_to ")
}

fn parse_clause(clause: &str) -> Result<Predicate, String> {
    if let Some(rest) = clause.strip_prefix("every ") {
        return parse_every(rest);
    }
    if let Some(rest) = clause.strip_prefix("at least one ") {
        return parse_at_least_one(rest);
    }
    if let Some(rest) = clause.strip_prefix("no ") {
        return parse_none_exists(rest);
    }
    if let Some(predicate) = parse_exists(clause)? {
        return Ok(predicate);
    }
    parse_comparison(clause)
}

fn parse_every(rest: &str) -> Result<Predicate, String> {
    if let Some((subject, scope)) = split_exists_subject(rest) {
        let set = parse_set_ref(subject, scope)?;
        return Ok(Predicate::Every {
            set,
            condition: Box::new(Predicate::Exists {
                entity: EntityRef::This,
            }),
        });
    }

    if let Ok((subject, state_set)) = parse_state_condition(rest) {
        let set = parse_set_ref(subject, None)?;
        return Ok(Predicate::Every {
            set,
            condition: Box::new(Predicate::StateIn {
                entity: EntityRef::This,
                state_set,
            }),
        });
    }

    let (subject, attr, op, value) = parse_quantified_attr_condition(rest)?;
    let set = parse_set_ref(subject, None)?;
    Ok(Predicate::Every {
        set,
        condition: Box::new(Predicate::AttrCmp {
            entity: EntityRef::This,
            attr,
            op,
            value,
        }),
    })
}

fn parse_at_least_one(rest: &str) -> Result<Predicate, String> {
    let (subject, state_set) = parse_state_condition(rest)?;
    let set = parse_set_ref(subject, None)?;
    Ok(Predicate::AtLeastOne {
        set,
        condition: Box::new(Predicate::StateIn {
            entity: EntityRef::This,
            state_set,
        }),
    })
}

fn parse_none_exists(rest: &str) -> Result<Predicate, String> {
    if let Some(subject) = rest.strip_suffix(" exists") {
        return Ok(Predicate::NoneExists {
            set: parse_set_ref(subject, None)?,
            condition: Box::new(Predicate::Exists {
                entity: EntityRef::This,
            }),
        });
    }

    let (subject, condition) = rest
        .split_once(" exists with ")
        .ok_or_else(|| "expected `exists with` in negative existence predicate".to_string())?;
    let (condition, scope) = split_attached_scope(condition)?;
    let set = parse_set_ref(subject, scope)?;
    let state_set = parse_state_rhs(condition)?;
    Ok(Predicate::NoneExists {
        set,
        condition: Box::new(Predicate::StateIn {
            entity: EntityRef::This,
            state_set,
        }),
    })
}

fn parse_exists(clause: &str) -> Result<Option<Predicate>, String> {
    let Some((subject, scope)) = split_exists_subject(clause) else {
        return Ok(None);
    };
    Ok(Some(Predicate::Exists {
        entity: parse_entity_ref(subject, scope)?,
    }))
}

fn split_exists_subject(clause: &str) -> Option<(&str, Option<RelationScope>)> {
    if let Some((subject, scope)) = clause.split_once(" exists for ") {
        return parse_for_scope(scope).map(|parsed| (subject, Some(parsed)));
    }
    clause
        .strip_suffix(" exists")
        .map(|subject| (subject.trim(), None))
}

fn parse_comparison(clause: &str) -> Result<Predicate, String> {
    if let Some((subject, rhs)) = clause.split_once(".state in ") {
        let entity = parse_entity_ref(subject, None)?;
        return Ok(Predicate::StateIn {
            entity,
            state_set: parse_state_set(rhs)?,
        });
    }

    let (left, op, right) = split_comparison(clause)?;
    let (entity, attr) = parse_qualified_field(left)?;
    if attr == "state" {
        if !matches!(op, CmpOp::Eq) {
            return Err("state comparisons only support `=` or `in`".to_string());
        }
        return Ok(Predicate::StateIn {
            entity,
            state_set: parse_state_set(right)?,
        });
    }

    Ok(Predicate::AttrCmp {
        entity,
        attr,
        op,
        value: parse_attr_value(right),
    })
}

fn parse_state_condition(clause: &str) -> Result<(&str, StateSet), String> {
    if let Some((subject, rhs)) = clause.split_once(".state in ") {
        return Ok((subject.trim(), parse_state_set(rhs)?));
    }
    if let Some((subject, rhs)) = clause.split_once(".state = ") {
        return Ok((subject.trim(), parse_state_set(rhs)?));
    }
    Err("expected `.state =` or `.state in` condition".to_string())
}

fn parse_quantified_attr_condition(
    clause: &str,
) -> Result<(&str, String, CmpOp, AttrValue), String> {
    let (left, op, right) = split_comparison(clause)?;
    let (subject, attr) = split_field(left)?;
    if attr == "state" {
        return Err("state condition must use state parser".to_string());
    }
    Ok((subject, attr.to_string(), op, parse_attr_value(right)))
}

fn parse_state_rhs(clause: &str) -> Result<StateSet, String> {
    let Some(rhs) = clause.strip_prefix("state = ") else {
        return Err("expected `state =` condition".to_string());
    };
    parse_state_set(rhs)
}

fn parse_state_set(rhs: &str) -> Result<StateSet, String> {
    let rhs = rhs.trim();
    if rhs.is_empty() {
        return Err("state set is empty".to_string());
    }
    if rhs.starts_with('{') {
        let inner = rhs
            .strip_prefix('{')
            .and_then(|s| s.strip_suffix('}'))
            .ok_or_else(|| "state set must close with `}`".to_string())?;
        let states = inner
            .split(',')
            .map(str::trim)
            .filter(|s| !s.is_empty())
            .map(ToOwned::to_owned)
            .collect::<Vec<_>>();
        if states.is_empty() {
            return Err("state set is empty".to_string());
        }
        return Ok(states);
    }
    Ok(vec![rhs.to_string()])
}

fn split_attached_scope(clause: &str) -> Result<(&str, Option<RelationScope>), String> {
    if let Some((condition, scope)) = clause.split_once(" AND attached_to ") {
        return parse_attached_scope(scope).map(|parsed| (condition.trim(), Some(parsed)));
    }
    Ok((clause.trim(), None))
}

fn parse_for_scope(scope: &str) -> Option<RelationScope> {
    parse_this_scope(scope)
        .or_else(|| parse_parent_scope(scope))
        .or_else(|| parse_attached_scope(scope).ok())
}

fn parse_this_scope(scope: &str) -> Option<RelationScope> {
    scope
        .strip_prefix("this ")
        .map(|kind| RelationScope::This(kind.trim().to_string()))
}

fn parse_parent_scope(scope: &str) -> Option<RelationScope> {
    scope
        .strip_prefix("parent ")
        .map(|kind| RelationScope::Parent(kind.trim().to_string()))
}

fn parse_attached_scope(scope: &str) -> Result<RelationScope, String> {
    let target = scope
        .strip_prefix("this ")
        .ok_or_else(|| "attached_to scope must be `this <kind>`".to_string())?;
    Ok(RelationScope::AttachedTo(target.trim().to_string()))
}

fn parse_set_ref(subject: &str, scope: Option<RelationScope>) -> Result<EntitySetRef, String> {
    let subject = subject.trim();
    if subject.is_empty() {
        return Err("entity set kind is empty".to_string());
    }
    let (qualifier, kind) = if let Some(kind) = subject.strip_prefix("required ") {
        (Some(EntityQualifier::Required), kind.trim())
    } else {
        (None, subject)
    };
    if kind.is_empty() {
        return Err("entity set kind is empty".to_string());
    }
    Ok(EntitySetRef {
        kind: kind.to_string(),
        qualifier,
        scope,
    })
}

fn parse_entity_ref(subject: &str, scope: Option<RelationScope>) -> Result<EntityRef, String> {
    let subject = subject.trim();
    if subject.is_empty() {
        return Err("entity kind is empty".to_string());
    }
    if let Some(kind) = subject.strip_prefix("parent ") {
        return Ok(EntityRef::Parent(kind.trim().to_string()));
    }
    if let Some(scope) = scope {
        return Ok(EntityRef::Scoped {
            kind: subject.to_string(),
            scope,
        });
    }
    Ok(EntityRef::Named(subject.to_string()))
}

fn parse_qualified_field(left: &str) -> Result<(EntityRef, String), String> {
    if let Some(rest) = left.strip_prefix("parent ") {
        let (kind, attr) = split_field(rest)?;
        return Ok((EntityRef::Parent(kind.to_string()), attr.to_string()));
    }

    let (kind, attr) = split_field(left)?;
    Ok((EntityRef::Named(kind.to_string()), attr.to_string()))
}

fn split_field(input: &str) -> Result<(&str, &str), String> {
    let (kind, attr) = input
        .trim()
        .rsplit_once('.')
        .ok_or_else(|| "expected qualified field `<entity>.<attr>`".to_string())?;
    if kind.trim().is_empty() || attr.trim().is_empty() {
        return Err("qualified field has an empty entity or attribute".to_string());
    }
    Ok((kind.trim(), attr.trim()))
}

fn split_comparison(clause: &str) -> Result<(&str, CmpOp, &str), String> {
    for (token, op) in [
        (" >= ", CmpOp::Ge),
        (" <= ", CmpOp::Le),
        (" != ", CmpOp::Ne),
        (" = ", CmpOp::Eq),
        (" > ", CmpOp::Gt),
        (" < ", CmpOp::Lt),
    ] {
        if let Some((left, right)) = clause.split_once(token) {
            return Ok((left.trim(), op, right.trim()));
        }
    }

    Err("expected comparison operator".to_string())
}

fn parse_attr_value(value: &str) -> AttrValue {
    let value = value.trim();
    if matches!(value, "true" | "TRUE") {
        return AttrValue::Bool(true);
    }
    if matches!(value, "false" | "FALSE") {
        return AttrValue::Bool(false);
    }
    if value.parse::<i64>().is_ok() || value.parse::<f64>().is_ok() {
        return AttrValue::Number(value.to_string());
    }
    if (value.starts_with('"') && value.ends_with('"'))
        || (value.starts_with('\'') && value.ends_with('\''))
    {
        return AttrValue::String(value[1..value.len() - 1].to_string());
    }
    AttrValue::Symbol(value.to_string())
}
