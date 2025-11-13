//! Idiomatic nom-based DSL parser for Ultimate Beneficial Ownership workflows
//!
//! This module provides efficient standalone parser functions using nom combinators
//! with proper error handling and zero-copy parsing where possible.

use nom::{
    branch::alt,
    bytes::complete::{tag, take_until},
    character::complete::{alpha1, alphanumeric1, char, digit1, multispace0},
    combinator::{map, opt, recognize, value},
    error::{ParseError, VerboseError},
    multi::{many0, separated_list1},
    sequence::{delimited, pair, preceded, terminated, tuple},
    Finish, IResult,
};

use crate::parser_ast::{Form, Key, Literal, Program, PropertyMap, Value, VerbForm};

/// Parser error type with context information
pub(crate) type NomParseError<'a> = VerboseError<&'a str>;
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

/// Parse program: multiple forms
fn program_internal(input: &str) -> ParseResult<'_, Program> {
    let (input, _) = multispace0(input)?;
    let (input, forms) = many0(terminated(parse_form, multispace0))(input)?;
    Ok((input, forms))
}

/// Parse a form: (verb :key value ...) or a comment
pub fn parse_form(input: &str) -> ParseResult<'_, Form> {
    alt((
        map(parse_comment, Form::Comment),
        map(parse_verb_form, Form::Verb),
    ))(input)
}

/// Parse a comment (;; ...)
fn parse_comment(input: &str) -> ParseResult<'_, String> {
    let (input, _) = tag(";;")(input)?;
    let (input, comment_text) = take_until("\n")(input)?;
    let (input, _) = opt(char('\n'))(input)?; // Consume newline if present
    Ok((input, comment_text.to_string()))
}

/// Parse a verb form: (verb :key value ...)
pub fn parse_verb_form(input: &str) -> ParseResult<'_, VerbForm> {
    let (input, _) = char('(')(input)?;
    let (input, _) = multispace0(input)?; // Allow whitespace after '('
    let (input, verb) = parse_identifier(input)?; // Verb is an identifier
    let (input, _) = multispace0(input)?; // Allow whitespace after verb

    let (input, pairs) = many0(pair(
        preceded(multispace0, parse_key),
        preceded(multispace0, parse_value),
    ))(input)?;
    let (input, _) = multispace0(input)?; // Allow whitespace before ')'
    let (input, _) = char(')')(input)?;

    Ok((
        input,
        VerbForm {
            verb,
            pairs: pairs.into_iter().collect(), // Collect into PropertyMap (HashMap<Key, Value>)
        },
    ))
}

/// Parse a key: :identifier or :identifier.sub_identifier
fn parse_key(input: &str) -> ParseResult<'_, Key> {
    let (input, _) = char(':')(input)?;
    let (input, id_parts) = separated_list1(char('.'), parse_key_part)(input)?;
    Ok((input, Key { parts: id_parts }))
}

/// Parse a value based on v3.0 EBNF: literal | identifier | list | map | attr-ref
pub fn parse_value(input: &str) -> ParseResult<'_, Value> {
    alt((
        map(parse_literal, Value::Literal),
        map(parse_identifier, Value::Identifier),
        parse_list_value, // returns Value::List
        parse_map_value,  // returns Value::Map
        parse_attr_ref,   // returns Value::AttrRef
    ))(input)
}

/// Parse a literal value: string | number | boolean | date | uuid
fn parse_literal(input: &str) -> ParseResult<'_, Literal> {
    alt((
        map(parse_string_literal, Literal::String),
        parse_number_literal,
        parse_boolean_literal,
        map(parse_string_literal, Literal::Date), // Dates are strings in ISO 8601 format
        map(parse_string_literal, Literal::Uuid), // UUIDs are strings
    ))(input)
}

/// Parse string literals with proper escaping.
pub fn parse_string_literal(input: &str) -> ParseResult<'_, String> {
    delimited(
        char('\"'),
        map(
            many0(alt((
                value('\n', tag("\\n")),
                value('\r', tag("\\r")),
                value('\t', tag("\\t")),
                value('\\', tag("\\\\")),
                value('\"', tag("\\\"")),
                none_of("\"\\"),
            ))),
            |chars| chars.into_iter().collect(),
        ),
        char('\"'),
    )(input)
}

/// Parse numeric literal values (f64 for both integers and floats)
fn parse_number_literal(input: &str) -> ParseResult<'_, Literal> {
    let (input, num_str) = recognize(tuple((
        opt(char('-')),
        digit1,
        opt(preceded(char('.'), digit1)),
    )))(input)?;
    let num = num_str.parse::<f64>().map_err(|_| {
        nom::Err::Error(VerboseError::from_error_kind(
            input,
            nom::error::ErrorKind::Float, // Use Float for any numeric parsing error
        ))
    })?;
    Ok((input, Literal::Number(num)))
}

/// Parse boolean literal values
fn parse_boolean_literal(input: &str) -> ParseResult<'_, Literal> {
    alt((
        value(Literal::Boolean(true), tag("true")),
        value(Literal::Boolean(false), tag("false")),
    ))(input)
}

/// Parse list values: [item1 item2 item3] or [item1, item2, item3] (both formats supported)
/// Implements V3.1 EBNF: "[" (value (("," | whitespace) value)*)? "]"
pub fn parse_list_value(input: &str) -> ParseResult<'_, Value> {
    let (input, _) = char('[')(input)?;
    let (input, _) = multispace0(input)?;

    // Handle empty list
    if let Ok((remaining, _)) = char::<_, VerboseError<&str>>(']')(input) {
        return Ok((remaining, Value::List(vec![])));
    }

    // Parse first value
    let (input, first_value) = parse_value(input)?;
    let (input, _) = multispace0(input)?;

    let mut values = vec![first_value];
    let mut current_input = input;

    // Parse remaining values with comma or space separators
    loop {
        // If we see a closing bracket, we're done
        if current_input.starts_with(']') {
            break;
        }

        // Try comma separator first
        if let Ok((remaining, _)) = char::<_, VerboseError<&str>>(',')(current_input) {
            let (remaining, _) = multispace0(remaining)?;
            let (remaining, value) = parse_value(remaining)?;
            values.push(value);
            let (remaining, _) = multispace0(remaining)?;
            current_input = remaining;
        } else {
            // Try space separator (whitespace already consumed)
            if let Ok((remaining, value)) = parse_value(current_input) {
                values.push(value);
                let (remaining, _) = multispace0(remaining)?;
                current_input = remaining;
            } else {
                // No more values to parse
                break;
            }
        }
    }

    let (input, _) = char(']')(current_input)?;
    Ok((input, Value::List(values)))
}

/// Parse map values: {key1: value1, key2: value2}
pub(crate) fn parse_map_value(input: &str) -> ParseResult<'_, Value> {
    let (input, _) = char('{')(input)?;
    let (input, _) = multispace0(input)?;
    let (input, pairs) = many0(|input| {
        let (input, _) = multispace0(input)?;
        let (input, key) = parse_key(input)?;
        let (input, _) = multispace0(input)?;
        let (input, value) = parse_value(input)?;
        let (input, _) = multispace0(input)?;
        Ok((input, (key, value)))
    })(input)?;
    let (input, _) = multispace0(input)?;
    let (input, _) = char('}')(input)?;
    Ok((input, Value::Map(pairs.into_iter().collect())))
}

/// Parse an attribute reference: @attr{uuid}
fn parse_attr_ref(input: &str) -> ParseResult<'_, Value> {
    let (input, uuid_str) = delimited(tag("@attr{"), parse_string_literal, char('}'))(input)?;
    Ok((input, Value::AttrRef(uuid_str)))
}

/// Parse identifiers: alphanumeric with underscore, dash, dot
pub fn parse_identifier(input: &str) -> ParseResult<'_, String> {
    let (input, id) = recognize(pair(
        alt((alpha1, tag("_"))),
        many0(alt((alphanumeric1, tag("_"), tag("-"), tag(".")))),
    ))(input)?;
    Ok((input, id.to_string()))
}

/// Parse key parts: alphanumeric with underscore, dash (but not dot for key splitting)
fn parse_key_part(input: &str) -> ParseResult<'_, String> {
    let (input, id) = recognize(pair(
        alt((alpha1, tag("_"))),
        many0(alt((alphanumeric1, tag("_"), tag("-")))),
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

