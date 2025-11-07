//! Nom-based parser for the UBO/KYC DSL
//!
//! This parser transforms S-expression DSL text into a typed AST.
//! It handles:
//! - S-expression syntax with balanced parentheses
//! - Keywords with ':' prefix
//! - Multi-value properties with source attribution
//! - Nested structures (maps, lists, blocks)

use nom::{
    branch::alt,
    bytes::complete::{tag, take_until, take_while, take_while1},
    character::complete::{char, line_ending, multispace1},
    combinator::{map, value},
    multi::{many0, separated_list0},
    number::complete::double,
    sequence::{delimited, pair, preceded, tuple},
    IResult,
};

use crate::ast::*;
use chrono::NaiveDate;

pub mod combinators;
pub mod primitives;
pub mod statements;

/// Parse comments and whitespace
fn ws_and_comments(input: &str) -> IResult<&str, ()> {
    use nom::character::complete::multispace0;
    let (input, _) = multispace0(input)?;
    let (input, _) = many0(alt((map(comment, |_| ()), map(multispace1, |_| ()))))(input)?;
    Ok((input, ()))
}

/// Parse a line comment starting with ;;
fn comment(input: &str) -> IResult<&str, &str> {
    let (input, _) = tag(";;")(input)?;
    let (input, content) = take_until("\n")(input)?;
    let (input, _) = line_ending(input)?;
    Ok((input, content))
}

/// Main entry point: parse a complete DSL program
pub fn parse_program(input: &str) -> IResult<&str, Program> {
    let (input, _) = ws_and_comments(input)?;
    let (input, workflows) = many0(preceded(ws_and_comments, parse_workflow))(input)?;
    let (input, _) = ws_and_comments(input)?;
    Ok((input, Program { workflows }))
}

/// Parse a workflow definition
pub fn parse_workflow(input: &str) -> IResult<&str, Workflow> {
    let (input, _) = char('(')(input)?;
    let (input, _) = ws_and_comments(input)?;
    let (input, _) = tag("define-kyc-investigation")(input)?;
    let (input, _) = ws_and_comments(input)?;

    let (input, id) = parse_string(input)?;
    let (input, _) = ws_and_comments(input)?;

    let (input, properties) = parse_property_list(input)?;
    let (input, _) = ws_and_comments(input)?;

    let (input, statements) = many0(preceded(ws_and_comments, parse_statement))(input)?;
    let (input, _) = ws_and_comments(input)?;
    let (input, _) = char(')')(input)?;

    Ok((
        input,
        Workflow {
            id,
            properties,
            statements,
        },
    ))
}

/// Parse any statement type
fn parse_statement(input: &str) -> IResult<&str, Statement> {
    alt((
        map(parse_declare_entity, Statement::DeclareEntity),
        map(parse_parallel_obtain, Statement::ParallelObtain),
        map(parse_obtain_document, Statement::ObtainDocument),
        map(parse_create_edge, Statement::CreateEdge),
        map(parse_solicit_attribute, Statement::SolicitAttribute),
        map(parse_calculate_ubo, Statement::CalculateUbo),
        map(parse_resolve_conflict, Statement::ResolveConflict),
        map(parse_generate_report, Statement::GenerateReport),
        map(parse_schedule_monitoring, Statement::ScheduleMonitoring),
        map(parse_parallel_block, Statement::Parallel),
    ))(input)
}

/// Parse declare-entity statement
fn parse_declare_entity(input: &str) -> IResult<&str, DeclareEntity> {
    let (input, _) = char('(')(input)?;
    let (input, _) = ws_and_comments(input)?;
    let (input, _) = tag("declare-entity")(input)?;
    let (input, _) = ws_and_comments(input)?;

    let (input, node_id) =
        preceded(tuple((tag(":node-id"), ws_and_comments)), parse_string)(input)?;
    let (input, _) = ws_and_comments(input)?;

    let (input, label) =
        preceded(tuple((tag(":label"), ws_and_comments)), parse_entity_label)(input)?;
    let (input, _) = ws_and_comments(input)?;

    let (input, properties) = preceded(
        tuple((tag(":properties"), ws_and_comments)),
        parse_property_map,
    )(input)?;
    let (input, _) = ws_and_comments(input)?;

    let (input, _) = char(')')(input)?;

    Ok((
        input,
        DeclareEntity {
            node_id,
            label,
            properties,
        },
    ))
}

/// Parse parallel-obtain statement
fn parse_parallel_obtain(input: &str) -> IResult<&str, ParallelObtain> {
    let (input, _) = char('(')(input)?;
    let (input, _) = ws_and_comments(input)?;
    let (input, _) = tag("parallel-obtain")(input)?;
    let (input, _) = ws_and_comments(input)?;

    let (input, documents) = many0(preceded(ws_and_comments, parse_obtain_document))(input)?;
    let (input, _) = ws_and_comments(input)?;
    let (input, _) = char(')')(input)?;

    Ok((input, ParallelObtain { documents }))
}

/// Parse obtain-document statement
fn parse_obtain_document(input: &str) -> IResult<&str, ObtainDocument> {
    let (input, _) = char('(')(input)?;
    let (input, _) = ws_and_comments(input)?;
    let (input, _) = tag("obtain-document")(input)?;
    let (input, _) = ws_and_comments(input)?;

    let (input, doc_id) = preceded(tuple((tag(":doc-id"), ws_and_comments)), parse_string)(input)?;
    let (input, _) = ws_and_comments(input)?;

    let (input, doc_type) =
        preceded(tuple((tag(":doc-type"), ws_and_comments)), parse_string)(input)?;
    let (input, _) = ws_and_comments(input)?;

    let (input, issuer) = preceded(tuple((tag(":issuer"), ws_and_comments)), parse_string)(input)?;
    let (input, _) = ws_and_comments(input)?;

    let (input, issue_date) =
        preceded(tuple((tag(":issue-date"), ws_and_comments)), parse_date)(input)?;
    let (input, _) = ws_and_comments(input)?;

    let (input, confidence) =
        preceded(tuple((tag(":confidence"), ws_and_comments)), double)(input)?;
    let (input, _) = ws_and_comments(input)?;

    let (input, additional_props) = parse_property_list(input)?;
    let (input, _) = ws_and_comments(input)?;
    let (input, _) = char(')')(input)?;

    Ok((
        input,
        ObtainDocument {
            doc_id,
            doc_type,
            issuer,
            issue_date,
            confidence,
            additional_props,
        },
    ))
}

/// Parse create-edge statement
fn parse_create_edge(input: &str) -> IResult<&str, CreateEdge> {
    let (input, _) = char('(')(input)?;
    let (input, _) = ws_and_comments(input)?;
    let (input, _) = tag("create-edge")(input)?;
    let (input, _) = ws_and_comments(input)?;

    let (input, from) = preceded(tuple((tag(":from"), ws_and_comments)), parse_string)(input)?;
    let (input, _) = ws_and_comments(input)?;

    let (input, to) = preceded(tuple((tag(":to"), ws_and_comments)), parse_string)(input)?;
    let (input, _) = ws_and_comments(input)?;

    let (input, edge_type) =
        preceded(tuple((tag(":type"), ws_and_comments)), parse_edge_type)(input)?;
    let (input, _) = ws_and_comments(input)?;

    let (input, properties) = preceded(
        tuple((tag(":properties"), ws_and_comments)),
        parse_property_map,
    )(input)?;
    let (input, _) = ws_and_comments(input)?;

    let (input, evidenced_by) = preceded(
        tuple((tag(":evidenced-by"), ws_and_comments)),
        parse_string_list,
    )(input)?;
    let (input, _) = ws_and_comments(input)?;

    let (input, _) = char(')')(input)?;

    Ok((
        input,
        CreateEdge {
            from,
            to,
            edge_type,
            properties,
            evidenced_by,
        },
    ))
}

/// Parse solicit-attribute statement
fn parse_solicit_attribute(input: &str) -> IResult<&str, SolicitAttribute> {
    let (input, _) = char('(')(input)?;
    let (input, _) = ws_and_comments(input)?;
    let (input, _) = tag("solicit-attribute")(input)?;
    let (input, _) = ws_and_comments(input)?;

    let (input, attr_id) =
        preceded(tuple((tag(":attr-id"), ws_and_comments)), parse_string)(input)?;
    let (input, _) = ws_and_comments(input)?;

    let (input, from) = preceded(tuple((tag(":from"), ws_and_comments)), parse_string)(input)?;
    let (input, _) = ws_and_comments(input)?;

    let (input, value_type) =
        preceded(tuple((tag(":value-type"), ws_and_comments)), parse_string)(input)?;
    let (input, _) = ws_and_comments(input)?;

    let (input, additional_props) = parse_property_list(input)?;
    let (input, _) = ws_and_comments(input)?;
    let (input, _) = char(')')(input)?;

    Ok((
        input,
        SolicitAttribute {
            attr_id,
            from,
            value_type,
            additional_props,
        },
    ))
}

/// Parse calculate-ubo-prongs statement
fn parse_calculate_ubo(input: &str) -> IResult<&str, CalculateUbo> {
    let (input, _) = char('(')(input)?;
    let (input, _) = ws_and_comments(input)?;
    let (input, _) = tag("calculate-ubo-prongs")(input)?;
    let (input, _) = ws_and_comments(input)?;

    let (input, target) = preceded(tuple((tag(":target"), ws_and_comments)), parse_string)(input)?;
    let (input, _) = ws_and_comments(input)?;

    let (input, algorithm) =
        preceded(tuple((tag(":algorithm"), ws_and_comments)), parse_string)(input)?;
    let (input, _) = ws_and_comments(input)?;

    let (input, max_depth) = preceded(
        tuple((tag(":max-depth"), ws_and_comments)),
        nom::character::complete::u64,
    )(input)?;
    let (input, _) = ws_and_comments(input)?;

    let (input, threshold) = preceded(tuple((tag(":threshold"), ws_and_comments)), double)(input)?;
    let (input, _) = ws_and_comments(input)?;

    let (input, traversal_rules) = preceded(
        tuple((tag(":traversal-rules"), ws_and_comments)),
        parse_property_map,
    )(input)?;
    let (input, _) = ws_and_comments(input)?;

    let (input, output) =
        preceded(tuple((tag(":output"), ws_and_comments)), parse_property_map)(input)?;
    let (input, _) = ws_and_comments(input)?;

    let (input, _) = char(')')(input)?;

    Ok((
        input,
        CalculateUbo {
            target,
            algorithm,
            max_depth: max_depth as usize,
            threshold,
            traversal_rules,
            output,
        },
    ))
}

/// Parse resolve-conflicts statement (simplified)
fn parse_resolve_conflict(input: &str) -> IResult<&str, ResolveConflict> {
    let (input, _) = char('(')(input)?;
    let (input, _) = ws_and_comments(input)?;
    let (input, _) = tag("resolve-conflicts")(input)?;
    let (input, _) = ws_and_comments(input)?;

    let (input, node) = preceded(tuple((tag(":node"), ws_and_comments)), parse_string)(input)?;
    let (input, _) = ws_and_comments(input)?;

    let (input, property) =
        preceded(tuple((tag(":property"), ws_and_comments)), parse_string)(input)?;
    let (input, _) = ws_and_comments(input)?;

    // Simplified: just consume the rest as property map
    let (input, _) = preceded(
        tuple((tag(":strategy"), ws_and_comments)),
        take_until_balanced_paren,
    )(input)?;
    let (input, _) = ws_and_comments(input)?;

    let (input, resolution) = preceded(
        tuple((tag(":resolution"), ws_and_comments)),
        parse_property_map,
    )(input)?;
    let (input, _) = ws_and_comments(input)?;

    let (input, _) = char(')')(input)?;

    Ok((
        input,
        ResolveConflict {
            node,
            property,
            strategy: WaterfallStrategy { priorities: vec![] },
            resolution,
        },
    ))
}

/// Parse generate-ubo-report statement
fn parse_generate_report(input: &str) -> IResult<&str, GenerateReport> {
    let (input, _) = char('(')(input)?;
    let (input, _) = ws_and_comments(input)?;
    let (input, _) = tag("generate-ubo-report")(input)?;
    let (input, _) = ws_and_comments(input)?;

    let (input, target) = preceded(tuple((tag(":target"), ws_and_comments)), parse_string)(input)?;
    let (input, _) = ws_and_comments(input)?;

    let (input, status) = preceded(tuple((tag(":status"), ws_and_comments)), parse_string)(input)?;
    let (input, _) = ws_and_comments(input)?;

    let (input, identified_ubos) = preceded(
        tuple((tag(":identified-ubos"), ws_and_comments)),
        parse_map_list,
    )(input)?;
    let (input, _) = ws_and_comments(input)?;

    let (input, unresolved_prongs) = preceded(
        tuple((tag(":unresolved-prongs"), ws_and_comments)),
        parse_map_list,
    )(input)?;
    let (input, _) = ws_and_comments(input)?;

    let (input, additional_props) = parse_property_list(input)?;
    let (input, _) = ws_and_comments(input)?;
    let (input, _) = char(')')(input)?;

    Ok((
        input,
        GenerateReport {
            target,
            status,
            identified_ubos,
            unresolved_prongs,
            additional_props,
        },
    ))
}

/// Parse schedule-monitoring statement
fn parse_schedule_monitoring(input: &str) -> IResult<&str, ScheduleMonitoring> {
    let (input, _) = char('(')(input)?;
    let (input, _) = ws_and_comments(input)?;
    let (input, _) = tag("schedule-monitoring")(input)?;
    let (input, _) = ws_and_comments(input)?;

    let (input, target) = preceded(tuple((tag(":target"), ws_and_comments)), parse_string)(input)?;
    let (input, _) = ws_and_comments(input)?;

    let (input, frequency) =
        preceded(tuple((tag(":frequency"), ws_and_comments)), parse_string)(input)?;
    let (input, _) = ws_and_comments(input)?;

    let (input, triggers) =
        preceded(tuple((tag(":triggers"), ws_and_comments)), parse_map_list)(input)?;
    let (input, _) = ws_and_comments(input)?;

    let (input, additional_props) = parse_property_list(input)?;
    let (input, _) = ws_and_comments(input)?;
    let (input, _) = char(')')(input)?;

    Ok((
        input,
        ScheduleMonitoring {
            target,
            frequency,
            triggers,
            additional_props,
        },
    ))
}

/// Parse parallel block
fn parse_parallel_block(input: &str) -> IResult<&str, Vec<Statement>> {
    let (input, _) = char('(')(input)?;
    let (input, _) = ws_and_comments(input)?;
    let (input, _) = tag("parallel")(input)?;
    let (input, _) = ws_and_comments(input)?;

    let (input, statements) = many0(preceded(ws_and_comments, parse_statement))(input)?;
    let (input, _) = ws_and_comments(input)?;
    let (input, _) = char(')')(input)?;

    Ok((input, statements))
}

// ============================================================================
// Primitive Parsers
// ============================================================================

fn parse_string(input: &str) -> IResult<&str, String> {
    let (input, s) = delimited(char('"'), take_while(|c| c != '"'), char('"'))(input)?;
    Ok((input, s.to_string()))
}

fn parse_identifier(input: &str) -> IResult<&str, String> {
    let (input, id) = take_while1(|c: char| c.is_alphanumeric() || c == '-' || c == '_')(input)?;
    Ok((input, id.to_string()))
}

fn parse_keyword(input: &str) -> IResult<&str, String> {
    let (input, _) = char(':')(input)?;
    let (input, kw) = parse_identifier(input)?;
    Ok((input, kw))
}

fn parse_date(input: &str) -> IResult<&str, NaiveDate> {
    let (input, date_str) = parse_string(input)?;
    let date = NaiveDate::parse_from_str(&date_str, "%Y-%m-%d")
        .unwrap_or_else(|_| NaiveDate::from_ymd_opt(2024, 1, 1).unwrap());
    Ok((input, date))
}

fn parse_entity_label(input: &str) -> IResult<&str, EntityLabel> {
    alt((
        value(EntityLabel::Company, tag("Company")),
        value(EntityLabel::Person, tag("Person")),
        value(EntityLabel::Trust, tag("Trust")),
        value(EntityLabel::Address, tag("Address")),
        value(EntityLabel::Document, tag("Document")),
        value(EntityLabel::Officer, tag("Officer")),
    ))(input)
}

fn parse_edge_type(input: &str) -> IResult<&str, EdgeType> {
    alt((
        value(EdgeType::HasOwnership, tag("HAS_OWNERSHIP")),
        value(EdgeType::HasControl, tag("HAS_CONTROL")),
        value(EdgeType::IsDirectorOf, tag("IS_DIRECTOR_OF")),
        value(EdgeType::IsSecretaryOf, tag("IS_SECRETARY_OF")),
        value(EdgeType::HasShareholder, tag("HAS_SHAREHOLDER")),
        value(EdgeType::ResidesAt, tag("RESIDES_AT")),
        value(EdgeType::HasRegisteredOffice, tag("HAS_REGISTERED_OFFICE")),
        value(EdgeType::EvidencedBy, tag("EVIDENCED_BY")),
    ))(input)
}

fn parse_value(input: &str) -> IResult<&str, Value> {
    alt((
        map(parse_string, Value::String),
        map(nom::character::complete::i64, Value::Integer),
        map(double, Value::Number),
        value(Value::Boolean(true), tag("true")),
        value(Value::Boolean(false), tag("false")),
        map(parse_property_map, Value::Map),
        map(parse_value_list, Value::List),
        value(Value::Null, tag("null")),
        map(parse_bare_symbol, Value::String),
    ))(input)
}

fn parse_bare_symbol(input: &str) -> IResult<&str, String> {
    let (input, symbol) =
        take_while1(|c: char| c.is_alphanumeric() || c == '_' || c == '-')(input)?;
    // Don't consume keywords or reserved words
    if symbol.starts_with(':') || symbol == "null" || symbol == "true" || symbol == "false" {
        return Err(nom::Err::Error(nom::error::Error::new(
            input,
            nom::error::ErrorKind::Tag,
        )));
    }
    Ok((input, symbol.to_string()))
}

fn parse_value_list(input: &str) -> IResult<&str, Vec<Value>> {
    delimited(
        pair(char('['), ws_and_comments),
        separated_list0(
            tuple((ws_and_comments, char(','), ws_and_comments)),
            parse_value,
        ),
        pair(ws_and_comments, char(']')),
    )(input)
}

fn parse_string_list(input: &str) -> IResult<&str, Vec<String>> {
    delimited(
        pair(char('['), ws_and_comments),
        separated_list0(
            tuple((ws_and_comments, char(','), ws_and_comments)),
            parse_string,
        ),
        pair(ws_and_comments, char(']')),
    )(input)
}

fn parse_map_list(input: &str) -> IResult<&str, Vec<PropertyMap>> {
    delimited(
        pair(char('['), ws_and_comments),
        separated_list0(
            tuple((ws_and_comments, char(','), ws_and_comments)),
            parse_property_map,
        ),
        pair(ws_and_comments, char(']')),
    )(input)
}

fn parse_property_map(input: &str) -> IResult<&str, PropertyMap> {
    let (input, _) = char('{')(input)?;
    let (input, _) = ws_and_comments(input)?;

    let (input, entries) = separated_list0(
        tuple((ws_and_comments, char(','), ws_and_comments)),
        parse_map_entry,
    )(input)?;

    let (input, _) = ws_and_comments(input)?;
    let (input, _) = char('}')(input)?;

    let mut map = PropertyMap::new();
    for (key, value) in entries {
        map.insert(key, value);
    }

    Ok((input, map))
}

fn parse_map_entry(input: &str) -> IResult<&str, (String, Value)> {
    let (input, key) = parse_keyword(input)?;
    let (input, _) = ws_and_comments(input)?;
    let (input, value) = parse_value(input)?;
    Ok((input, (key, value)))
}

fn parse_property_list(input: &str) -> IResult<&str, PropertyMap> {
    let (input, pairs) = many0(preceded(ws_and_comments, parse_property_pair))(input)?;

    let mut map = PropertyMap::new();
    for (key, value) in pairs {
        map.insert(key, value);
    }
    Ok((input, map))
}

fn parse_property_pair(input: &str) -> IResult<&str, (String, Value)> {
    let (input, key) = parse_keyword(input)?;
    let (input, _) = ws_and_comments(input)?;
    let (input, value) = parse_value(input)?;
    Ok((input, (key, value)))
}

// Helper to consume nested structures
fn take_until_balanced_paren(input: &str) -> IResult<&str, &str> {
    let mut depth = 0;
    let mut pos = 0;

    for (i, c) in input.char_indices() {
        match c {
            '(' => depth += 1,
            ')' => {
                depth -= 1;
                if depth == 0 {
                    pos = i + 1;
                    break;
                }
            }
            _ => {}
        }
    }

    if depth == 0 && pos > 0 {
        Ok((&input[pos..], &input[..pos]))
    } else {
        Err(nom::Err::Error(nom::error::Error::new(
            input,
            nom::error::ErrorKind::TakeUntil,
        )))
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_parse_string() {
        let input = r#""hello world""#;
        let (_, result) = parse_string(input).unwrap();
        assert_eq!(result, "hello world");
    }

    #[test]
    fn test_parse_entity_label() {
        let input = "Company";
        let (_, result) = parse_entity_label(input).unwrap();
        assert_eq!(result, EntityLabel::Company);
    }

    #[test]
    fn test_parse_property_map() {
        let input = r#"{ :name "Test", :value 42 }"#;
        let (_, result) = parse_property_map(input).unwrap();
        assert!(result.contains_key("name"));
        assert!(result.contains_key("value"));
    }
}
