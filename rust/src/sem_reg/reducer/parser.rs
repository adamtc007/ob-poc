use nom::{
    branch::alt,
    bytes::complete::{escaped_transform, is_not, tag, take_while, take_while1},
    character::complete::{char, digit1, multispace0, multispace1, one_of},
    combinator::{all_consuming, map, opt, recognize, value},
    multi::{many0, separated_list0, separated_list1},
    sequence::{delimited, pair, preceded, tuple},
    IResult,
};

use super::ast::{
    AggFn, CompareOp, ConditionBody, Expr, Literal, Predicate, SlotField, SlotPredicate, Value,
};
use super::error::{ReducerError, ReducerResult};

/// Parse a full reducer condition body.
///
/// # Examples
/// ```rust
/// use ob_poc::sem_reg::reducer::parse_condition_body;
///
/// let body = parse_condition_body("screening.status = 'CLEAR'").unwrap();
/// assert!(matches!(body, ob_poc::sem_reg::reducer::ConditionBody::Leaf { .. }));
/// ```
pub fn parse_condition_body(input: &str) -> ReducerResult<ConditionBody> {
    let trimmed = input.trim();
    if trimmed.is_empty() {
        return Err(ReducerError::Parse("empty input".into()));
    }
    let (_, body) = all_consuming(ws(parse_or_composition))(trimmed)
        .map_err(|err| ReducerError::Parse(err.to_string()))?;
    Ok(body)
}

/// Parse a comparison value.
///
/// # Examples
/// ```rust
/// use ob_poc::sem_reg::reducer::{parse_value, Value};
///
/// assert_eq!(parse_value("$2").unwrap(), Value::Param(2));
/// ```
pub fn parse_value(input: &str) -> ReducerResult<Value> {
    let (_, value) = all_consuming(ws(parse_value_inner))(input)
        .map_err(|err| ReducerError::Parse(err.to_string()))?;
    Ok(value)
}

/// Parse a literal value.
///
/// # Examples
/// ```rust
/// use ob_poc::sem_reg::reducer::{parse_literal, Literal};
///
/// assert_eq!(parse_literal("'ok'").unwrap(), Literal::Str("ok".into()));
/// ```
pub fn parse_literal(input: &str) -> ReducerResult<Literal> {
    let (_, value) = all_consuming(ws(parse_literal_inner))(input)
        .map_err(|err| ReducerError::Parse(err.to_string()))?;
    Ok(value)
}

fn parse_or_composition(input: &str) -> IResult<&str, ConditionBody> {
    let (input, first) = parse_and_composition(input)?;
    let (input, rest) = many0(preceded(ws(tag("OR")), parse_and_composition))(input)?;
    if rest.is_empty() {
        Ok((input, first))
    } else {
        let mut terms = vec![first];
        terms.extend(rest);
        Ok((input, ConditionBody::Or(terms)))
    }
}

fn parse_and_composition(input: &str) -> IResult<&str, ConditionBody> {
    let (input, first) = parse_unary(input)?;
    let (input, rest) = many0(preceded(ws(tag("AND")), parse_unary))(input)?;
    if rest.is_empty() {
        Ok((input, first))
    } else {
        let mut terms = vec![first];
        terms.extend(rest);
        Ok((input, ConditionBody::And(terms)))
    }
}

fn parse_unary(input: &str) -> IResult<&str, ConditionBody> {
    ws(alt((
        map(
            preceded(pair(tag("NOT"), multispace1), parse_unary),
            |body| ConditionBody::Not(Box::new(body)),
        ),
        delimited(char('('), parse_or_composition, char(')')),
        parse_scope_slot_aggregate,
        parse_scope_comparison,
        parse_condition_call,
        parse_aggregate_leaf,
        parse_overlay_leaf,
        parse_condition_ref,
    )))(input)
}

fn parse_scope_comparison(input: &str) -> IResult<&str, ConditionBody> {
    let (input, _) = tag("scope.")(input)?;
    let (input, path) = separated_list1(char('.'), identifier)(input)?;
    let (input, op) = ws(parse_compare_op)(input)?;
    let (input, value) = ws(parse_value_inner)(input)?;
    Ok((
        input,
        ConditionBody::Leaf {
            expr: Expr::ScopeComparison {
                path: path.into_iter().map(ToString::to_string).collect(),
                op,
                value,
            },
            compare: None,
        },
    ))
}

fn parse_scope_slot_aggregate(input: &str) -> IResult<&str, ConditionBody> {
    let (input, function) = parse_agg_fn(input)?;
    let (input, _) = ws(char('('))(input)?;
    let (input, _) = tag("scope.slots")(input)?;
    let (input, _) = ws(tag("WHERE"))(input)?;
    let (input, filter) = parse_slot_predicate(input)?;
    let (input, _) = ws(char(')'))(input)?;
    Ok((
        input,
        ConditionBody::Leaf {
            expr: Expr::SlotAggregate { function, filter },
            compare: None,
        },
    ))
}

fn parse_condition_call(input: &str) -> IResult<&str, ConditionBody> {
    let (input, name) = identifier(input)?;
    let (input, args) = delimited(
        ws(char('(')),
        separated_list0(ws(char(',')), parse_value_inner),
        ws(char(')')),
    )(input)?;
    Ok((
        input,
        ConditionBody::Call {
            name: name.to_string(),
            args,
        },
    ))
}

fn parse_aggregate_leaf(input: &str) -> IResult<&str, ConditionBody> {
    let (input, function) = parse_agg_fn(input)?;
    let (input, _) = ws(char('('))(input)?;
    let (input, source) = identifier(input)?;
    let (input, filter) = opt(preceded(ws(tag("WHERE")), parse_predicate))(input)?;
    let (input, _) = ws(char(')'))(input)?;
    let (input, compare) = opt(pair(ws(parse_compare_op), ws(parse_value_inner)))(input)?;
    Ok((
        input,
        ConditionBody::Leaf {
            expr: Expr::Aggregate {
                function,
                source: source.to_string(),
                filter,
            },
            compare,
        },
    ))
}

fn parse_overlay_leaf(input: &str) -> IResult<&str, ConditionBody> {
    let (input, source) = identifier(input)?;
    let (input, field) = opt(preceded(char('.'), separated_list1(char('.'), identifier)))(input)?;
    let field_name = field.map(|parts| parts.join(".")).unwrap_or_default();
    let (input, _) = multispace1(input)?;
    let (input, is_kw) = alt((
        tag("IS"),
        tag("="),
        tag("!="),
        tag(">="),
        tag("<="),
        tag(">"),
        tag("<"),
        tag("NOT"),
        tag("IN"),
        tag("LIKE"),
    ))(input)?;
    if is_kw == "IS" {
        let (input, _) = multispace1(input)?;
        let (input, negated) = map(opt(terminated_tag("NOT")), |value| value.is_none())(input)?;
        let (input, _) = ws(tag("NULL"))(input)?;
        return Ok((
            input,
            ConditionBody::Leaf {
                expr: Expr::Existence {
                    source: source.to_string(),
                    field: field_name,
                    negated,
                },
                compare: None,
            },
        ));
    }

    let op = match is_kw {
        "=" => CompareOp::Eq,
        "!=" => CompareOp::Neq,
        ">" => CompareOp::Gt,
        ">=" => CompareOp::Gte,
        "<" => CompareOp::Lt,
        "<=" => CompareOp::Lte,
        "LIKE" => CompareOp::Like,
        "IN" => CompareOp::In,
        "NOT" => {
            let (input, _) = multispace1(input)?;
            let (input, _) = tag("IN")(input)?;
            let (input, _) = multispace1(input)?;
            let (input, value) = parse_value_inner(input)?;
            return Ok((
                input,
                ConditionBody::Leaf {
                    expr: Expr::Comparison {
                        source: source.to_string(),
                        field: field_name,
                        op: CompareOp::NotIn,
                        value,
                    },
                    compare: None,
                },
            ));
        }
        _ => unreachable!(),
    };
    let (input, value) = ws(parse_value_inner)(input)?;
    Ok((
        input,
        ConditionBody::Leaf {
            expr: Expr::Comparison {
                source: source.to_string(),
                field: field_name,
                op,
                value,
            },
            compare: None,
        },
    ))
}

fn parse_condition_ref(input: &str) -> IResult<&str, ConditionBody> {
    let (input, name) = identifier(input)?;
    if is_reserved(name) {
        return Err(nom::Err::Error(nom::error::Error::new(
            input,
            nom::error::ErrorKind::Tag,
        )));
    }
    Ok((
        input,
        ConditionBody::Ref {
            name: name.to_string(),
        },
    ))
}

fn parse_predicate(input: &str) -> IResult<&str, Predicate> {
    parse_pred_or(input)
}

fn parse_pred_or(input: &str) -> IResult<&str, Predicate> {
    let (input, first) = parse_pred_and(input)?;
    let (input, rest) = many0(preceded(ws(tag("OR")), parse_pred_and))(input)?;
    if rest.is_empty() {
        Ok((input, first))
    } else {
        let mut terms = vec![first];
        terms.extend(rest);
        Ok((input, Predicate::Or(terms)))
    }
}

fn parse_pred_and(input: &str) -> IResult<&str, Predicate> {
    let (input, first) = parse_pred_unary(input)?;
    let (input, rest) = many0(preceded(ws(tag("AND")), parse_pred_unary))(input)?;
    if rest.is_empty() {
        Ok((input, first))
    } else {
        let mut terms = vec![first];
        terms.extend(rest);
        Ok((input, Predicate::And(terms)))
    }
}

fn parse_pred_unary(input: &str) -> IResult<&str, Predicate> {
    ws(alt((
        map(
            preceded(pair(tag("NOT"), multispace1), parse_pred_unary),
            |pred| Predicate::Not(Box::new(pred)),
        ),
        delimited(char('('), parse_predicate, char(')')),
        parse_pred_atom,
    )))(input)
}

fn parse_pred_atom(input: &str) -> IResult<&str, Predicate> {
    let (input, field) = identifier(input)?;
    let (input, _) = multispace1(input)?;
    let (input, keyword) = alt((
        tag("IS"),
        tag("NOT"),
        tag("IN"),
        tag("LIKE"),
        tag("="),
        tag("!="),
        tag(">="),
        tag("<="),
        tag(">"),
        tag("<"),
    ))(input)?;
    if keyword == "IS" {
        let (input, _) = multispace1(input)?;
        let (input, negated) = map(opt(terminated_tag("NOT")), |value| value.is_some())(input)?;
        let (input, _) = ws(tag("NULL"))(input)?;
        return Ok((
            input,
            Predicate::IsNull {
                field: field.to_string(),
                negated,
            },
        ));
    }
    if keyword == "NOT" {
        let (input, _) = multispace1(input)?;
        let (input, _) = tag("IN")(input)?;
        let (input, _) = multispace1(input)?;
        let (input, value) = parse_value_inner(input)?;
        return Ok((
            input,
            Predicate::Atom {
                field: field.to_string(),
                op: CompareOp::NotIn,
                value,
            },
        ));
    }
    let op = keyword_to_compare_op(keyword);
    let (input, value) = ws(parse_value_inner)(input)?;
    Ok((
        input,
        Predicate::Atom {
            field: field.to_string(),
            op,
            value,
        },
    ))
}

fn parse_slot_predicate(input: &str) -> IResult<&str, SlotPredicate> {
    parse_slot_pred_or(input)
}

fn parse_slot_pred_or(input: &str) -> IResult<&str, SlotPredicate> {
    let (input, first) = parse_slot_pred_and(input)?;
    let (input, rest) = many0(preceded(ws(tag("OR")), parse_slot_pred_and))(input)?;
    if rest.is_empty() {
        Ok((input, first))
    } else {
        let mut terms = vec![first];
        terms.extend(rest);
        Ok((input, SlotPredicate::Or(terms)))
    }
}

fn parse_slot_pred_and(input: &str) -> IResult<&str, SlotPredicate> {
    let (input, first) = parse_slot_pred_unary(input)?;
    let (input, rest) = many0(preceded(ws(tag("AND")), parse_slot_pred_unary))(input)?;
    if rest.is_empty() {
        Ok((input, first))
    } else {
        let mut terms = vec![first];
        terms.extend(rest);
        Ok((input, SlotPredicate::And(terms)))
    }
}

fn parse_slot_pred_unary(input: &str) -> IResult<&str, SlotPredicate> {
    ws(alt((
        map(
            preceded(pair(tag("NOT"), multispace1), parse_slot_pred_unary),
            |pred| SlotPredicate::Not(Box::new(pred)),
        ),
        delimited(char('('), parse_slot_predicate, char(')')),
        parse_slot_pred_atom,
    )))(input)
}

fn parse_slot_pred_atom(input: &str) -> IResult<&str, SlotPredicate> {
    let (input, field) = parse_slot_field(input)?;
    let (input, op) = ws(parse_compare_op)(input)?;
    let (input, value) = ws(parse_value_inner)(input)?;
    Ok((input, SlotPredicate::Atom { field, op, value }))
}

fn parse_slot_field(input: &str) -> IResult<&str, SlotField> {
    alt((
        value(SlotField::Type, tag("type")),
        value(SlotField::Cardinality, tag("cardinality")),
        value(SlotField::EffectiveState, tag("effective_state")),
        value(SlotField::ComputedState, tag("computed_state")),
    ))(input)
}

fn parse_agg_fn(input: &str) -> IResult<&str, AggFn> {
    alt((
        value(AggFn::Count, tag("COUNT")),
        value(AggFn::Any, tag("ANY")),
        value(AggFn::All, tag("ALL")),
    ))(input)
}

fn parse_compare_op(input: &str) -> IResult<&str, CompareOp> {
    alt((
        value(
            CompareOp::NotIn,
            tuple((tag("NOT"), multispace1, tag("IN"))),
        ),
        value(CompareOp::Gte, tag(">=")),
        value(CompareOp::Lte, tag("<=")),
        value(CompareOp::Neq, tag("!=")),
        value(CompareOp::Eq, tag("=")),
        value(CompareOp::Gt, tag(">")),
        value(CompareOp::Lt, tag("<")),
        value(CompareOp::In, tag("IN")),
        value(CompareOp::Like, tag("LIKE")),
    ))(input)
}

fn parse_value_inner(input: &str) -> IResult<&str, Value> {
    alt((parse_param_ref, map(parse_literal_inner, Value::Literal)))(input)
}

fn parse_param_ref(input: &str) -> IResult<&str, Value> {
    let (input, _) = char('$')(input)?;
    let (input, digits) = digit1(input)?;
    let index = digits.parse::<usize>().map_err(|_| {
        nom::Err::Error(nom::error::Error::new(input, nom::error::ErrorKind::Digit))
    })?;
    Ok((input, Value::Param(index)))
}

fn parse_literal_inner(input: &str) -> IResult<&str, Literal> {
    alt((
        parse_string_lit,
        parse_number_lit,
        value(Literal::Bool(true), tag("true")),
        value(Literal::Bool(false), tag("false")),
        value(Literal::Null, tag("null")),
        parse_list_lit,
    ))(input)
}

fn parse_string_lit(input: &str) -> IResult<&str, Literal> {
    let single = delimited(
        char('\''),
        escaped_transform(is_not("\\'"), '\\', one_of("\\'nrt")),
        char('\''),
    );
    let double = delimited(
        char('"'),
        escaped_transform(is_not("\\\""), '\\', one_of("\\\"nrt")),
        char('"'),
    );
    map(alt((single, double)), Literal::Str)(input)
}

fn parse_number_lit(input: &str) -> IResult<&str, Literal> {
    map(
        recognize(tuple((
            opt(char('-')),
            digit1,
            opt(tuple((char('.'), digit1))),
        ))),
        |digits: &str| Literal::Num(digits.parse::<f64>().unwrap_or_default()),
    )(input)
}

fn parse_list_lit(input: &str) -> IResult<&str, Literal> {
    map(
        delimited(
            char('('),
            separated_list0(ws(char(',')), parse_literal_inner),
            ws(char(')')),
        ),
        Literal::List,
    )(input)
}

fn identifier(input: &str) -> IResult<&str, &str> {
    recognize(pair(
        take_while1(|c: char| c.is_ascii_alphabetic() || c == '_'),
        take_while(|c: char| c.is_ascii_alphanumeric() || c == '_'),
    ))(input)
}

fn keyword_to_compare_op(keyword: &str) -> CompareOp {
    match keyword {
        "=" => CompareOp::Eq,
        "!=" => CompareOp::Neq,
        ">" => CompareOp::Gt,
        ">=" => CompareOp::Gte,
        "<" => CompareOp::Lt,
        "<=" => CompareOp::Lte,
        "IN" => CompareOp::In,
        "LIKE" => CompareOp::Like,
        _ => unreachable!(),
    }
}

fn is_reserved(input: &str) -> bool {
    matches!(
        input,
        "AND" | "OR" | "NOT" | "IN" | "IS" | "LIKE" | "NULL" | "WHERE"
    )
}

fn terminated_tag<'a>(keyword: &'static str) -> impl Fn(&'a str) -> IResult<&'a str, &'a str> {
    move |input| {
        let (input, value) = tag(keyword)(input)?;
        let (input, _) = multispace1(input)?;
        Ok((input, value))
    }
}

fn ws<'a, O, F>(mut parser: F) -> impl FnMut(&'a str) -> IResult<&'a str, O>
where
    F: FnMut(&'a str) -> IResult<&'a str, O>,
{
    move |input| delimited(multispace0, &mut parser, multispace0)(input)
}
