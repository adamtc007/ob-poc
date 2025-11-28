//! Nom-based parser for DSL v2
//!
//! Parses the unified S-expression grammar into AST types.

use nom::{
    branch::alt,
    bytes::complete::{escaped_transform, tag, take_while, take_while1},
    character::complete::{alpha1, alphanumeric1, char, digit1, multispace0, multispace1, none_of},
    combinator::{all_consuming, cut, map, opt, recognize, value},
    error::{context, ContextError, ParseError as NomParseError},
    multi::{many0, many1},
    sequence::{delimited, pair, preceded, tuple},
    IResult,
};
use rust_decimal::Decimal;
use std::collections::HashMap;
use std::str::FromStr;
use uuid::Uuid;

use super::ast::*;

// ============================================================================
// Public API
// ============================================================================

/// Parse a complete DSL program from source text
///
/// Returns a structured Program AST or a human-readable error message.
///
/// # Example
/// ```ignore
/// let program = parse_program(r#"
///     (cbu.create :name "Test Fund" :jurisdiction "LU")
///     (cbu.link :cbu-id @cbu :entity-id @manager :role "InvestmentManager")
/// "#)?;
/// ```
pub fn parse_program(input: &str) -> Result<Program, String> {
    match all_consuming(program::<nom::error::VerboseError<&str>>)(input) {
        Ok((_, prog)) => Ok(prog),
        Err(nom::Err::Error(e)) | Err(nom::Err::Failure(e)) => {
            Err(nom::error::convert_error(input, e))
        }
        Err(nom::Err::Incomplete(_)) => Err("Incomplete input".to_string()),
    }
}

/// Parse a single verb call (for REPL/interactive use)
pub fn parse_single_verb(input: &str) -> Result<VerbCall, String> {
    let input = input.trim();
    match all_consuming(delimited(
        multispace0::<_, nom::error::VerboseError<&str>>,
        verb_call,
        multispace0,
    ))(input)
    {
        Ok((_, vc)) => Ok(vc),
        Err(nom::Err::Error(e)) | Err(nom::Err::Failure(e)) => {
            Err(nom::error::convert_error(input, e))
        }
        Err(nom::Err::Incomplete(_)) => Err("Incomplete input".to_string()),
    }
}

// ============================================================================
// Internal Parsers
// ============================================================================

fn program<'a, E: NomParseError<&'a str> + ContextError<&'a str>>(
    input: &'a str,
) -> IResult<&'a str, Program, E> {
    let (input, _) = multispace0(input)?;
    let (input, statements) = many0(statement)(input)?;
    let (input, _) = multispace0(input)?;
    Ok((input, Program { statements }))
}

fn statement<'a, E: NomParseError<&'a str> + ContextError<&'a str>>(
    input: &'a str,
) -> IResult<&'a str, Statement, E> {
    let (input, _) = multispace0(input)?;
    alt((
        map(comment, Statement::Comment),
        map(verb_call, Statement::VerbCall),
    ))(input)
}

// ============================================================================
// Comments
// ============================================================================

fn comment<'a, E: NomParseError<&'a str>>(input: &'a str) -> IResult<&'a str, String, E> {
    let (input, _) = tag(";;")(input)?;
    let (input, text) = take_while(|c| c != '\n')(input)?;
    let (input, _) = opt(char('\n'))(input)?;
    Ok((input, text.trim().to_string()))
}

// ============================================================================
// Verb Calls
// ============================================================================

fn verb_call<'a, E: NomParseError<&'a str> + ContextError<&'a str>>(
    input: &'a str,
) -> IResult<&'a str, VerbCall, E> {
    let start_pos = input.len();

    let (input, _) = char('(')(input)?;
    let (input, _) = multispace0(input)?;
    let (input, (domain, verb)) = word(input)?;
    let (input, arguments) = many0(argument)(input)?;
    let (input, _) = multispace0(input)?;
    let (input, _) = cut(context("closing parenthesis", char(')')))(input)?;

    let end_pos = input.len();

    Ok((
        input,
        VerbCall {
            domain,
            verb,
            arguments,
            span: Span::new(start_pos, end_pos),
        },
    ))
}

fn word<'a, E: NomParseError<&'a str>>(input: &'a str) -> IResult<&'a str, (String, String), E> {
    let (input, domain) = identifier(input)?;
    let (input, _) = char('.')(input)?;
    let (input, verb) = kebab_identifier(input)?;
    Ok((input, (domain.to_string(), verb)))
}

// ============================================================================
// Arguments
// ============================================================================

fn argument<'a, E: NomParseError<&'a str> + ContextError<&'a str>>(
    input: &'a str,
) -> IResult<&'a str, Argument, E> {
    let (input, _) = multispace0(input)?;
    let (input, key) = keyword(input)?;
    let (input, _) = multispace1(input)?;
    let (input, val) = context("value", value_parser)(input)?;
    Ok((input, Argument { key, value: val }))
}

fn keyword<'a, E: NomParseError<&'a str>>(input: &'a str) -> IResult<&'a str, Key, E> {
    let (input, _) = char(':')(input)?;

    // Try dotted identifier first (must have at least one dot)
    if let Ok((remaining, parts)) = dotted_identifier::<E>(input) {
        return Ok((remaining, Key::Dotted(parts)));
    }

    // Fall back to simple kebab identifier
    let (input, name) = kebab_identifier(input)?;
    Ok((input, Key::Simple(name)))
}

fn dotted_identifier<'a, E: NomParseError<&'a str>>(
    input: &'a str,
) -> IResult<&'a str, Vec<String>, E> {
    let (input, first) = simple_identifier(input)?;
    let (input, rest) = many1(preceded(char('.'), simple_identifier))(input)?;

    let mut parts = vec![first.to_string()];
    parts.extend(rest.into_iter().map(|s| s.to_string()));
    Ok((input, parts))
}

fn kebab_identifier<'a, E: NomParseError<&'a str>>(input: &'a str) -> IResult<&'a str, String, E> {
    recognize(pair(
        alt((alpha1, tag("_"))),
        many0(alt((alphanumeric1, tag("_"), tag("-")))),
    ))(input)
    .map(|(rest, matched)| (rest, matched.to_string()))
}

fn simple_identifier<'a, E: NomParseError<&'a str>>(
    input: &'a str,
) -> IResult<&'a str, &'a str, E> {
    recognize(pair(
        alt((alpha1, tag("_"))),
        many0(alt((alphanumeric1, tag("_")))),
    ))(input)
}

fn identifier<'a, E: NomParseError<&'a str>>(input: &'a str) -> IResult<&'a str, &'a str, E> {
    recognize(pair(
        alt((alpha1, tag("_"))),
        many0(alt((alphanumeric1, tag("_")))),
    ))(input)
}

// ============================================================================
// Values
// ============================================================================

fn value_parser<'a, E: NomParseError<&'a str> + ContextError<&'a str>>(
    input: &'a str,
) -> IResult<&'a str, Value, E> {
    alt((
        // Order matters: try specific patterns before generic ones
        map(boolean_literal, Value::Boolean),
        map(null_literal, |_| Value::Null),
        map(attribute_ref, Value::AttributeRef),
        map(document_ref, Value::DocumentRef),
        map(reference, Value::Reference),
        map(string_literal, Value::String),
        number_literal, // Returns Value directly (Integer or Decimal)
        map(list_literal, Value::List),
        map(map_literal, Value::Map),
    ))(input)
}

// String literals with escape sequences
fn string_literal<'a, E: NomParseError<&'a str>>(input: &'a str) -> IResult<&'a str, String, E> {
    delimited(
        char('"'),
        escaped_transform(
            none_of("\"\\"),
            '\\',
            alt((
                value('\n', char('n')),
                value('\r', char('r')),
                value('\t', char('t')),
                value('\\', char('\\')),
                value('"', char('"')),
            )),
        ),
        char('"'),
    )(input)
}

// Number literals (integer or decimal)
fn number_literal<'a, E: NomParseError<&'a str>>(input: &'a str) -> IResult<&'a str, Value, E> {
    let (remaining, num_str) = recognize(tuple((
        opt(char('-')),
        digit1,
        opt(pair(char('.'), digit1)),
    )))(input)?;

    if num_str.contains('.') {
        match Decimal::from_str(num_str) {
            Ok(d) => Ok((remaining, Value::Decimal(d))),
            Err(_) => Err(nom::Err::Error(E::from_error_kind(
                input,
                nom::error::ErrorKind::Float,
            ))),
        }
    } else {
        match num_str.parse::<i64>() {
            Ok(i) => Ok((remaining, Value::Integer(i))),
            Err(_) => Err(nom::Err::Error(E::from_error_kind(
                input,
                nom::error::ErrorKind::Digit,
            ))),
        }
    }
}

// Boolean literals
fn boolean_literal<'a, E: NomParseError<&'a str>>(input: &'a str) -> IResult<&'a str, bool, E> {
    alt((value(true, tag("true")), value(false, tag("false"))))(input)
}

// Null literal
fn null_literal<'a, E: NomParseError<&'a str>>(input: &'a str) -> IResult<&'a str, (), E> {
    value((), tag("nil"))(input)
}

// Reference: @identifier
fn reference<'a, E: NomParseError<&'a str>>(input: &'a str) -> IResult<&'a str, String, E> {
    let (input, _) = char('@')(input)?;
    // Don't match if followed by "attr{" or "doc{" (those are typed refs)
    if input.starts_with("attr{") || input.starts_with("doc{") {
        return Err(nom::Err::Error(E::from_error_kind(
            input,
            nom::error::ErrorKind::Verify,
        )));
    }
    let (input, name) = identifier(input)?;
    Ok((input, name.to_string()))
}

// Attribute reference: @attr{uuid}
fn attribute_ref<'a, E: NomParseError<&'a str>>(input: &'a str) -> IResult<&'a str, Uuid, E> {
    delimited(tag("@attr{"), uuid_parser, char('}'))(input)
}

// Document reference: @doc{uuid}
fn document_ref<'a, E: NomParseError<&'a str>>(input: &'a str) -> IResult<&'a str, Uuid, E> {
    delimited(tag("@doc{"), uuid_parser, char('}'))(input)
}

// UUID parser
fn uuid_parser<'a, E: NomParseError<&'a str>>(input: &'a str) -> IResult<&'a str, Uuid, E> {
    let (remaining, uuid_str) = recognize(tuple((
        take_while1(|c: char| c.is_ascii_hexdigit()),
        char('-'),
        take_while1(|c: char| c.is_ascii_hexdigit()),
        char('-'),
        take_while1(|c: char| c.is_ascii_hexdigit()),
        char('-'),
        take_while1(|c: char| c.is_ascii_hexdigit()),
        char('-'),
        take_while1(|c: char| c.is_ascii_hexdigit()),
    )))(input)?;

    match Uuid::parse_str(uuid_str) {
        Ok(uuid) => Ok((remaining, uuid)),
        Err(_) => Err(nom::Err::Error(E::from_error_kind(
            input,
            nom::error::ErrorKind::Verify,
        ))),
    }
}

// List literal: [value, value, ...] or [value value ...]
fn list_literal<'a, E: NomParseError<&'a str> + ContextError<&'a str>>(
    input: &'a str,
) -> IResult<&'a str, Vec<Value>, E> {
    let (input, _) = char('[')(input)?;
    let (input, _) = multispace0(input)?;

    // Parse values separated by comma or whitespace
    let mut values = Vec::new();
    let mut remaining = input;

    loop {
        // Try to parse a value
        match value_parser::<E>(remaining) {
            Ok((rest, val)) => {
                values.push(val);
                remaining = rest;

                // Skip whitespace
                let (rest, _) = multispace0::<_, E>(remaining)?;
                remaining = rest;

                // Check for comma separator (optional)
                if let Ok((rest, _)) = char::<_, E>(',')(remaining) {
                    let (rest, _) = multispace0::<_, E>(rest)?;
                    remaining = rest;
                }
            }
            Err(_) => break,
        }
    }

    let (input, _) = multispace0(remaining)?;
    let (input, _) = char(']')(input)?;

    Ok((input, values))
}

// Map literal: {:key value :key2 value2}
fn map_literal<'a, E: NomParseError<&'a str> + ContextError<&'a str>>(
    input: &'a str,
) -> IResult<&'a str, HashMap<String, Value>, E> {
    delimited(
        char('{'),
        map(
            many0(delimited(
                multispace0,
                pair(map_key, preceded(multispace1, value_parser)),
                multispace0,
            )),
            |pairs| pairs.into_iter().collect(),
        ),
        char('}'),
    )(input)
}

fn map_key<'a, E: NomParseError<&'a str>>(input: &'a str) -> IResult<&'a str, String, E> {
    preceded(char(':'), kebab_identifier)(input)
}

// ============================================================================
// Tests
// ============================================================================

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_simple_verb_call() {
        let input = r#"(entity.create-limited-company :name "Acme Corp")"#;
        let result = parse_program(input).unwrap();

        assert_eq!(result.statements.len(), 1);
        if let Statement::VerbCall(vc) = &result.statements[0] {
            assert_eq!(vc.domain, "entity");
            assert_eq!(vc.verb, "create-limited-company");
            assert_eq!(vc.arguments.len(), 1);
            assert!(vc.arguments[0].key.matches("name"));
            assert_eq!(vc.arguments[0].value.as_string(), Some("Acme Corp"));
        } else {
            panic!("Expected VerbCall");
        }
    }

    #[test]
    fn test_cbu_link_with_reference() {
        let input = r#"(cbu.link :cbu-id @cbu :entity-id @aviva :role "InvestmentManager")"#;
        let result = parse_program(input).unwrap();

        if let Statement::VerbCall(vc) = &result.statements[0] {
            assert_eq!(vc.domain, "cbu");
            assert_eq!(vc.verb, "link");
            assert_eq!(vc.arguments.len(), 3);

            // Check reference
            assert!(vc.arguments[0].value.is_reference());
            assert_eq!(vc.arguments[0].value.as_reference(), Some("cbu"));
        } else {
            panic!("Expected VerbCall");
        }
    }

    #[test]
    fn test_attribute_ref() {
        let input =
            r#"(attr.set :attr-id @attr{550e8400-e29b-41d4-a716-446655440000} :value "test")"#;
        let result = parse_program(input).unwrap();

        if let Statement::VerbCall(vc) = &result.statements[0] {
            if let Value::AttributeRef(uuid) = &vc.arguments[0].value {
                assert_eq!(uuid.to_string(), "550e8400-e29b-41d4-a716-446655440000");
            } else {
                panic!("Expected AttributeRef");
            }
        } else {
            panic!("Expected VerbCall");
        }
    }

    #[test]
    fn test_number_literals() {
        let input = r#"(test.verb :int 42 :neg -17 :dec 3.14 :neg-dec -0.5)"#;
        let result = parse_program(input).unwrap();

        if let Statement::VerbCall(vc) = &result.statements[0] {
            assert!(matches!(&vc.arguments[0].value, Value::Integer(42)));
            assert!(matches!(&vc.arguments[1].value, Value::Integer(-17)));
            assert!(matches!(&vc.arguments[2].value, Value::Decimal(_)));
            assert!(matches!(&vc.arguments[3].value, Value::Decimal(_)));
        } else {
            panic!("Expected VerbCall");
        }
    }

    #[test]
    fn test_list_and_map() {
        let input = r#"(test.verb :list [1, 2, 3] :map {:a 1 :b 2})"#;
        let result = parse_program(input).unwrap();

        if let Statement::VerbCall(vc) = &result.statements[0] {
            if let Value::List(items) = &vc.arguments[0].value {
                assert_eq!(items.len(), 3);
            } else {
                panic!("Expected List");
            }

            if let Value::Map(m) = &vc.arguments[1].value {
                assert_eq!(m.len(), 2);
                assert!(m.contains_key("a"));
                assert!(m.contains_key("b"));
            } else {
                panic!("Expected Map");
            }
        } else {
            panic!("Expected VerbCall");
        }
    }

    #[test]
    fn test_list_without_commas() {
        let input = r#"(test.verb :list ["a" "b" "c"])"#;
        let result = parse_program(input).unwrap();

        if let Statement::VerbCall(vc) = &result.statements[0] {
            if let Value::List(items) = &vc.arguments[0].value {
                assert_eq!(items.len(), 3);
            } else {
                panic!("Expected List");
            }
        }
    }

    #[test]
    fn test_comment() {
        let input = r#";; This is a comment
(entity.create :name "Test")"#;
        let result = parse_program(input).unwrap();

        assert_eq!(result.statements.len(), 2);
        assert!(matches!(&result.statements[0], Statement::Comment(c) if c == "This is a comment"));
        assert!(matches!(&result.statements[1], Statement::VerbCall(_)));
    }

    #[test]
    fn test_multiline_program() {
        let input = r#"
;; Create a CBU
(cbu.create :name "Test Fund" :jurisdiction "LU")

;; Link an entity
(cbu.link :cbu-id @cbu :entity-id @manager :role "InvestmentManager")
"#;
        let result = parse_program(input).unwrap();
        assert_eq!(result.statements.len(), 4); // 2 comments + 2 verb calls
    }

    #[test]
    fn test_dotted_keyword() {
        let input = r#"(test.verb :address.city "London")"#;
        let result = parse_program(input).unwrap();

        if let Statement::VerbCall(vc) = &result.statements[0] {
            assert!(
                matches!(&vc.arguments[0].key, Key::Dotted(parts) if parts == &["address", "city"])
            );
        }
    }

    #[test]
    fn test_escape_sequences() {
        let input = r#"(test.verb :text "line1\nline2\ttab")"#;
        let result = parse_program(input).unwrap();

        if let Statement::VerbCall(vc) = &result.statements[0] {
            assert_eq!(vc.arguments[0].value.as_string(), Some("line1\nline2\ttab"));
        }
    }

    #[test]
    fn test_empty_program() {
        let input = "";
        let result = parse_program(input).unwrap();
        assert!(result.statements.is_empty());
    }

    #[test]
    fn test_whitespace_only() {
        let input = "   \n\n   \t  ";
        let result = parse_program(input).unwrap();
        assert!(result.statements.is_empty());
    }

    #[test]
    fn test_parse_error_unclosed_paren() {
        let input = r#"(entity.create :name "Test""#;
        let result = parse_program(input);
        assert!(result.is_err());
    }

    #[test]
    fn test_parse_single_verb() {
        let input = r#"(cbu.create :name "Fund")"#;
        let vc = parse_single_verb(input).unwrap();
        assert_eq!(vc.domain, "cbu");
        assert_eq!(vc.verb, "create");
    }
}
