//! Idiomatic nom-based DSL parser for Ultimate Beneficial Ownership workflows
//!
//! This module provides efficient standalone parser functions using nom combinators
//! with proper error handling and zero-copy parsing where possible.

use nom::{
    branch::alt,
    bytes::complete::{tag, take_until},
    character::complete::{alpha1, alphanumeric1, char, digit1, multispace0, space0, space1},
    combinator::{map, opt, recognize, value},
    error::{ParseError, VerboseError},
    multi::{many0, many1, separated_list0},
    sequence::{delimited, pair, preceded, terminated, tuple},
    Finish, IResult,
};

use crate::ast::{Program, PropertyMap, Statement, Value, Workflow};

/// Parser error type with context information
pub type NomParseError<'a> = VerboseError<&'a str>;
pub type ParseResult<'a, T> = IResult<&'a str, T, NomParseError<'a>>;

/// Parse a complete DSL program
pub fn parse_program(input: &str) -> Result<Program, NomParseError<'_>> {
    let (remaining, program) = program_internal(input).finish()?;

    if !remaining.trim().is_empty() {
        return Err(VerboseError::from_error_kind(
            remaining,
            nom::error::ErrorKind::Eof,
        ));
    }

    Ok(program)
}

/// Parse program: multiple workflows
fn program_internal(input: &str) -> ParseResult<'_, Program> {
    let (input, _) = multispace0(input)?;
    let (input, workflows) = many1(terminated(parse_workflow, multispace0))(input)?;
    Ok((input, Program { workflows }))
}

/// Parse workflow: (workflow "id" properties statements...)
pub fn parse_workflow(input: &str) -> ParseResult<'_, Workflow> {
    let (input, _) = char('(')(input)?;
    let (input, _) = tag("workflow")(input)?;
    let (input, _) = space1(input)?;
    let (input, id) = parse_string_literal(input)?;
    let (input, _) = space0(input)?;
    let (input, properties) = opt(parse_properties)(input)?;
    let (input, _) = space0(input)?;
    let (input, statements) = many0(terminated(parse_statement, space0))(input)?;
    let (input, _) = char(')')(input)?;

    Ok((
        input,
        Workflow {
            id,
            properties: properties.unwrap_or_default(),
            statements,
        },
    ))
}

/// Parse properties: (properties key1 value1 key2 value2...)
pub fn parse_properties(input: &str) -> ParseResult<'_, PropertyMap> {
    let (input, _) = char('(')(input)?;
    let (input, _) = tag("properties")(input)?;
    let (input, _) = space0(input)?;
    let (input, pairs) = separated_list0(space1, parse_property_pair)(input)?;
    let (input, _) = char(')')(input)?;
    Ok((input, pairs.into_iter().collect()))
}

/// Parse a key-value property pair
fn parse_property_pair(input: &str) -> ParseResult<'_, (String, Value)> {
    let (input, key) = parse_identifier(input)?;
    let (input, _) = space1(input)?;
    let (input, value) = parse_value(input)?;
    Ok((input, (key, value)))
}

/// Parse statement - the main DSL constructs
pub fn parse_statement(input: &str) -> ParseResult<'_, Statement> {
    let (input, _) = char('(')(input)?;
    let (input, statement) = alt((
        parse_declare_entity,
        parse_obtain_document,
        parse_create_edge,
        parse_calculate_ubo,
        parse_placeholder_statement,
    ))(input)?;
    let (input, _) = char(')')(input)?;
    Ok((input, statement))
}

/// Parse declare-entity statement
fn parse_declare_entity(input: &str) -> ParseResult<'_, Statement> {
    let (input, _) = tag("declare-entity")(input)?;
    let (input, _) = space1(input)?;
    let (input, id) = parse_string_literal(input)?;
    let (input, _) = space1(input)?;
    let (input, entity_type) = parse_string_literal(input)?;
    let (input, _) = space0(input)?;
    let (input, properties) = opt(parse_properties)(input)?;

    Ok((
        input,
        Statement::DeclareEntity {
            id,
            entity_type,
            properties: properties.unwrap_or_default(),
        },
    ))
}

/// Parse obtain-document statement
fn parse_obtain_document(input: &str) -> ParseResult<'_, Statement> {
    let (input, _) = tag("obtain-document")(input)?;
    let (input, _) = space1(input)?;
    let (input, document_type) = parse_string_literal(input)?;
    let (input, _) = space1(input)?;
    let (input, source) = parse_string_literal(input)?;
    let (input, _) = space0(input)?;
    let (input, properties) = opt(parse_properties)(input)?;

    Ok((
        input,
        Statement::ObtainDocument {
            document_type,
            source,
            properties: properties.unwrap_or_default(),
        },
    ))
}

/// Parse create-edge statement
fn parse_create_edge(input: &str) -> ParseResult<'_, Statement> {
    let (input, _) = tag("create-edge")(input)?;
    let (input, _) = space1(input)?;
    let (input, from) = parse_string_literal(input)?;
    let (input, _) = space1(input)?;
    let (input, to) = parse_string_literal(input)?;
    let (input, _) = space1(input)?;
    let (input, edge_type) = parse_string_literal(input)?;
    let (input, _) = space0(input)?;
    let (input, properties) = opt(parse_properties)(input)?;

    Ok((
        input,
        Statement::CreateEdge {
            from,
            to,
            edge_type,
            properties: properties.unwrap_or_default(),
        },
    ))
}

/// Parse calculate-ubo statement
fn parse_calculate_ubo(input: &str) -> ParseResult<'_, Statement> {
    let (input, _) = tag("calculate-ubo")(input)?;
    let (input, _) = space1(input)?;
    let (input, entity_id) = parse_string_literal(input)?;
    let (input, _) = space0(input)?;
    let (input, properties) = opt(parse_properties)(input)?;

    Ok((
        input,
        Statement::CalculateUbo {
            entity_id,
            properties: properties.unwrap_or_default(),
        },
    ))
}

/// Placeholder for unimplemented statement types
fn parse_placeholder_statement(input: &str) -> ParseResult<'_, Statement> {
    let (input, command) = parse_identifier(input)?;
    let (input, args) = many0(preceded(space1, parse_value))(input)?;
    Ok((input, Statement::Placeholder { command, args }))
}

/// Parse values - strings, numbers, booleans, lists, maps
pub fn parse_value(input: &str) -> ParseResult<'_, Value> {
    alt((
        parse_string_value,
        parse_number_value,
        parse_boolean_value,
        parse_list_value,
        parse_map_value,
        parse_null_value,
    ))(input)
}

/// Parse string values: "quoted strings"
fn parse_string_value(input: &str) -> ParseResult<'_, Value> {
    let (input, s) = parse_string_literal(input)?;
    Ok((input, Value::String(s)))
}

/// Parse string literals with proper escaping
pub fn parse_string_literal(input: &str) -> ParseResult<'_, String> {
    alt((
        delimited(
            char('"'),
            map(
                many0(alt((
                    value('\n', tag("\\n")),
                    value('\r', tag("\\r")),
                    value('\t', tag("\\t")),
                    value('\\', tag("\\\\")),
                    value('"', tag("\\\"")),
                    none_of("\"\\"),
                ))),
                |chars| chars.into_iter().collect(),
            ),
            char('"'),
        ),
        delimited(
            char('\''),
            map(take_until("'"), |s: &str| s.to_string()),
            char('\''),
        ),
    ))(input)
}

/// Parse numeric values with proper error handling
fn parse_number_value(input: &str) -> ParseResult<'_, Value> {
    alt((parse_float, parse_integer))(input)
}

/// Parse float values with proper error handling
fn parse_float(input: &str) -> ParseResult<'_, Value> {
    let (input, num_str) = recognize(tuple((opt(char('-')), digit1, char('.'), digit1)))(input)?;
    let num = num_str.parse::<f64>().map_err(|_| {
        nom::Err::Error(VerboseError::from_error_kind(
            input,
            nom::error::ErrorKind::Float,
        ))
    })?;
    Ok((input, Value::Number(num)))
}

/// Parse integer values with proper error handling
fn parse_integer(input: &str) -> ParseResult<'_, Value> {
    let (input, num_str) = recognize(tuple((opt(char('-')), digit1)))(input)?;
    let num = num_str.parse::<i64>().map_err(|_| {
        nom::Err::Error(VerboseError::from_error_kind(
            input,
            nom::error::ErrorKind::Digit,
        ))
    })?;
    Ok((input, Value::Number(num as f64)))
}

/// Parse boolean values
fn parse_boolean_value(input: &str) -> ParseResult<'_, Value> {
    alt((
        value(Value::Boolean(true), tag("true")),
        value(Value::Boolean(false), tag("false")),
    ))(input)
}

/// Parse null values
fn parse_null_value(input: &str) -> ParseResult<'_, Value> {
    value(Value::Null, tag("null"))(input)
}

/// Parse list values: [item1, item2, item3]
fn parse_list_value(input: &str) -> ParseResult<'_, Value> {
    let (input, _) = char('[')(input)?;
    let (input, values) = separated_list0(tuple((space0, char(','), space0)), parse_value)(input)?;
    let (input, _) = space0(input)?;
    let (input, _) = char(']')(input)?;
    Ok((input, Value::List(values)))
}

/// Parse map values: {key1: value1, key2: value2}
fn parse_map_value(input: &str) -> ParseResult<'_, Value> {
    let (input, _) = char('{')(input)?;
    let (input, pairs) = separated_list0(tuple((space0, char(','), space0)), |input| {
        let (input, key) = parse_string_literal(input)?;
        let (input, _) = space0(input)?;
        let (input, _) = char(':')(input)?;
        let (input, _) = space0(input)?;
        let (input, value) = parse_value(input)?;
        Ok((input, (key, value)))
    })(input)?;
    let (input, _) = space0(input)?;
    let (input, _) = char('}')(input)?;
    Ok((input, Value::Map(pairs.into_iter().collect())))
}

/// Parse identifiers: alphanumeric with underscore, dash, dot
pub fn parse_identifier(input: &str) -> ParseResult<'_, String> {
    let (input, id) = recognize(pair(
        alt((alpha1, tag("_"))),
        many0(alt((alphanumeric1, tag("_"), tag("-"), tag(".")))),
    ))(input)?;
    Ok((input, id.to_string()))
}

/// Helper function to parse single characters not in the given set
fn none_of(chars: &'static str) -> impl Fn(&str) -> ParseResult<char> {
    move |input| {
        if let Some(c) = input.chars().next() {
            if !chars.contains(c) {
                Ok((&input[c.len_utf8()..], c))
            } else {
                Err(nom::Err::Error(VerboseError::from_error_kind(
                    input,
                    nom::error::ErrorKind::OneOf,
                )))
            }
        } else {
            Err(nom::Err::Error(VerboseError::from_error_kind(
                input,
                nom::error::ErrorKind::Eof,
            )))
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_string_literal() {
        // Test double quotes
        assert_eq!(
            parse_string_literal("\"hello world\"").unwrap(),
            ("", "hello world".to_string())
        );

        // Test single quotes
        assert_eq!(
            parse_string_literal("'hello world'").unwrap(),
            ("", "hello world".to_string())
        );

        // Test escapes
        assert_eq!(
            parse_string_literal("\"hello\\nworld\"").unwrap(),
            ("", "hello\nworld".to_string())
        );
    }

    #[test]
    fn test_number_value() {
        // Test integer
        assert_eq!(parse_number_value("42").unwrap(), ("", Value::Number(42.0)));

        // Test float
        assert_eq!(
            parse_number_value("3.14159265").unwrap(),
            ("", Value::Number(3.14159265))
        );

        // Test negative
        assert_eq!(
            parse_number_value("-123").unwrap(),
            ("", Value::Number(-123.0))
        );
    }

    #[test]
    fn test_simple_workflow() {
        let input = r#"(workflow "test-workflow" (declare-entity "entity1" "person"))"#;

        let result = parse_program(input);
        assert!(result.is_ok());

        let program = result.unwrap();
        assert_eq!(program.workflows.len(), 1);
        assert_eq!(program.workflows[0].id, "test-workflow");
        assert_eq!(program.workflows[0].statements.len(), 1);
    }

    #[test]
    fn test_properties() {
        let input = r#"(properties name "John" age 30 active true)"#;

        let result = parse_properties(input);
        assert!(result.is_ok());

        let (_, props) = result.unwrap();
        assert_eq!(props.len(), 3);
        assert_eq!(props.get("name"), Some(&Value::String("John".to_string())));
        assert_eq!(props.get("age"), Some(&Value::Number(30.0)));
        assert_eq!(props.get("active"), Some(&Value::Boolean(true)));
    }

    #[test]
    fn test_list_value() {
        let input = r#"["item1", 42, true, null]"#;

        let result = parse_list_value(input);
        assert!(result.is_ok());

        let (_, value) = result.unwrap();
        if let Value::List(items) = value {
            assert_eq!(items.len(), 4);
            assert_eq!(items[0], Value::String("item1".to_string()));
            assert_eq!(items[1], Value::Number(42.0));
            assert_eq!(items[2], Value::Boolean(true));
            assert_eq!(items[3], Value::Null);
        } else {
            panic!("Expected list value");
        }
    }
}
