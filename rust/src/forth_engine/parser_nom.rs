//! Nom-based parser for the DSL Forth Engine.
//! 
//! Unified parser supporting all DSL dialects with a single EBNF grammar.

use nom::{
    branch::alt,
    bytes::complete::{escaped_transform, tag, take_until},
    character::complete::{alpha1, alphanumeric1, char, multispace0, none_of, digit1},
    combinator::{map, opt, recognize},
    multi::{many0, separated_list1},
    sequence::{delimited, pair, preceded,  tuple},
    IResult,
};

use crate::forth_engine::ast::{DslParser, Expr};
use crate::forth_engine::errors::EngineError;

pub struct NomDslParser;

impl Default for NomDslParser {
    fn default() -> Self {
        Self::new()
    }
}

impl NomDslParser {
    pub fn new() -> Self {
        NomDslParser
    }
}

impl DslParser for NomDslParser {
    fn parse(&self, input: &str) -> Result<Vec<Expr>, EngineError> {
        let trimmed = input.trim();
        if trimmed.is_empty() {
            return Ok(vec![]);
        }

        match many0(parse_expr)(trimmed) {
            Ok((remaining, exprs)) if remaining.trim().is_empty() => Ok(exprs),
            Ok((remaining, _)) => Err(EngineError::Parse(format!(
                "Input string has trailing characters: {}",
                remaining
            ))),
            Err(e) => Err(EngineError::Parse(e.to_string())),
        }
    }
}

fn parse_expr(input: &str) -> IResult<&str, Expr> {
    delimited(
        multispace0,
        alt((parse_comment, parse_s_expr, parse_atom)),
        multispace0,
    )(input)
}

fn parse_s_expr(input: &str) -> IResult<&str, Expr> {
    delimited(char('('), parse_word_call, char(')'))(input)
}

fn parse_word_call(input: &str) -> IResult<&str, Expr> {
    let (input, _) = multispace0(input)?;
    let (input, name) = map(parse_symbol, |s| s.to_string())(input)?;
    let (input, args) = many0(parse_expr)(input)?;
    Ok((input, Expr::WordCall { name, args }))
}

fn parse_atom(input: &str) -> IResult<&str, Expr> {
    alt((
        parse_number_literal,
        map(parse_bool_literal, Expr::BoolLiteral),
        map(parse_string_literal, Expr::StringLiteral),
        parse_dotted_keyword,
        map(parse_keyword, Expr::Keyword),
        map(parse_attribute_ref, Expr::AttributeRef),
        map(parse_document_ref, Expr::DocumentRef),
        parse_list_literal,
        parse_map_literal,
    ))(input)
}

// Parse comment: ;; text until newline
fn parse_comment(input: &str) -> IResult<&str, Expr> {
    let (input, _) = tag(";;")(input)?;
    let (input, text) = alt((
        take_until("\n"),
        // Handle comment at end of file (no newline)
        recognize(many0(none_of("\n"))),
    ))(input)?;
    let (input, _) = opt(char('\n'))(input)?;
    Ok((input, Expr::Comment(text.to_string())))
}

// Parse dotted keyword: :customer.id -> DottedKeyword(["customer", "id"])
fn parse_dotted_keyword(input: &str) -> IResult<&str, Expr> {
    let (input, _) = char(':')(input)?;
    let (input, parts) = separated_list1(char('.'), parse_key_part)(input)?;
    
    if parts.len() > 1 {
        Ok((input, Expr::DottedKeyword(parts.into_iter().map(|s| s.to_string()).collect())))
    } else {
        // Single part, return as regular Keyword
        Err(nom::Err::Error(nom::error::Error::new(input, nom::error::ErrorKind::Verify)))
    }
}

fn parse_keyword(input: &str) -> IResult<&str, String> {
    let (input, _) = char(':')(input)?;
    let (input, name) = parse_symbol(input)?;
    Ok((input, format!(":{}", name)))
}

fn parse_string_literal(input: &str) -> IResult<&str, String> {
    map(
        delimited(
            char('\"'),
            escaped_transform(
                none_of("\"\\"),
                '\\',
                alt((
                    map(char('n'), |_| "\n"),
                    map(char('r'), |_| "\r"),
                    map(char('t'), |_| "\t"),
                    map(char('\\'), |_| "\\"),
                    map(char('\"'), |_| "\""),
                )),
            ),
            char('\"'),
        ),
        |s| s.to_string(),
    )(input)
}

// Parse number: integer or float
fn parse_number_literal(input: &str) -> IResult<&str, Expr> {
    let (input, num_str) = recognize(tuple((
        opt(char('-')),
        digit1,
        opt(preceded(char('.'), digit1)),
    )))(input)?;
    
    if num_str.contains('.') {
        let num = num_str.parse::<f64>().map_err(|_| {
            nom::Err::Error(nom::error::Error::new(input, nom::error::ErrorKind::Float))
        })?;
        Ok((input, Expr::FloatLiteral(num)))
    } else {
        let num = num_str.parse::<i64>().map_err(|_| {
            nom::Err::Error(nom::error::Error::new(input, nom::error::ErrorKind::Digit))
        })?;
        Ok((input, Expr::IntegerLiteral(num)))
    }
}

fn parse_bool_literal(input: &str) -> IResult<&str, bool> {
    alt((map(tag("true"), |_| true), map(tag("false"), |_| false)))(input)
}

fn parse_symbol(input: &str) -> IResult<&str, &str> {
    recognize(pair(
        alt((alpha1, tag("_"), tag("-"), tag("."), tag(">"))),
        many0(alt((alphanumeric1, tag("_"), tag("-"), tag("."), tag(">")))),
    ))(input)
}

// Parse key part (for dotted keywords): alphanumeric with underscore, dash (not dot)
fn parse_key_part(input: &str) -> IResult<&str, &str> {
    recognize(pair(
        alt((alpha1, tag("_"))),
        many0(alt((alphanumeric1, tag("_"), tag("-")))),
    ))(input)
}

// Parse attribute reference: @attr{uuid} or @attr("uuid")
fn parse_attribute_ref(input: &str) -> IResult<&str, String> {
    alt((
        // New format: @attr{uuid}
        delimited(
            tag("@attr{"),
            map(
                recognize(pair(alphanumeric1, many0(alt((alphanumeric1, tag("-")))))),
                |s: &str| s.to_string(),
            ),
            char('}'),
        ),
        // Legacy format: @attr("uuid")
        delimited(tag("@attr("), parse_string_literal, char(')')),
    ))(input)
}

// Parse document reference: @doc{uuid} or @doc("uuid")
fn parse_document_ref(input: &str) -> IResult<&str, String> {
    alt((
        // New format: @doc{uuid}
        delimited(
            tag("@doc{"),
            map(
                recognize(pair(alphanumeric1, many0(alt((alphanumeric1, tag("-")))))),
                |s: &str| s.to_string(),
            ),
            char('}'),
        ),
        // Legacy format: @doc("uuid")
        delimited(tag("@doc("), parse_string_literal, char(')')),
    ))(input)
}

// Parse list: [item1 item2] or [item1, item2]
fn parse_list_literal(input: &str) -> IResult<&str, Expr> {
    let (input, _) = char('[')(input)?;
    let (input, _) = multispace0(input)?;
    
    // Handle empty list
    if let Ok((remaining, _)) = char::<_, nom::error::Error<&str>>(']')(input) {
        return Ok((remaining, Expr::ListLiteral(vec![])));
    }
    
    // Parse first value
    let (input, first) = parse_list_item(input)?;
    let (input, _) = multispace0(input)?;
    
    let mut items = vec![first];
    let mut current = input;
    
    loop {
        if current.starts_with(']') {
            break;
        }
        
        // Try comma separator
        if let Ok((remaining, _)) = char::<_, nom::error::Error<&str>>(',')(current) {
            let (remaining, _) = multispace0(remaining)?;
            let (remaining, item) = parse_list_item(remaining)?;
            items.push(item);
            let (remaining, _) = multispace0(remaining)?;
            current = remaining;
        } else if let Ok((remaining, item)) = parse_list_item(current) {
            // Space separator
            items.push(item);
            let (remaining, _) = multispace0(remaining)?;
            current = remaining;
        } else {
            break;
        }
    }
    
    let (input, _) = char(']')(current)?;
    Ok((input, Expr::ListLiteral(items)))
}

// Parse list item (recursive into parse_atom or parse_s_expr)
fn parse_list_item(input: &str) -> IResult<&str, Expr> {
    delimited(multispace0, alt((parse_s_expr, parse_atom)), multispace0)(input)
}

// Parse map: {:key value :key2 value2}
fn parse_map_literal(input: &str) -> IResult<&str, Expr> {
    let (input, _) = char('{')(input)?;
    let (input, _) = multispace0(input)?;
    
    let (input, pairs) = many0(|input| {
        let (input, _) = multispace0(input)?;
        let (input, key) = parse_map_key(input)?;
        let (input, _) = multispace0(input)?;
        let (input, value) = alt((parse_s_expr, parse_atom))(input)?;
        let (input, _) = multispace0(input)?;
        Ok((input, (key, value)))
    })(input)?;
    
    let (input, _) = multispace0(input)?;
    let (input, _) = char('}')(input)?;
    
    Ok((input, Expr::MapLiteral(pairs)))
}

// Parse map key: :keyword -> "keyword"
fn parse_map_key(input: &str) -> IResult<&str, String> {
    let (input, _) = char(':')(input)?;
    let (input, name) = parse_symbol(input)?;
    Ok((input, name.to_string()))
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_parse_comment() {
        let parser = NomDslParser::new();
        let result = parser.parse(";; This is a comment\n").unwrap();
        assert_eq!(result.len(), 1);
        match &result[0] {
            Expr::Comment(text) => assert_eq!(text, " This is a comment"),
            _ => panic!("Expected Comment"),
        }
    }

    #[test]
    fn test_parse_float() {
        let parser = NomDslParser::new();
        let result = parser.parse("3.14").unwrap();
        assert_eq!(result.len(), 1);
        match &result[0] {
            Expr::FloatLiteral(n) => assert!((n - 3.14).abs() < 0.001),
            _ => panic!("Expected FloatLiteral"),
        }
    }

    #[test]
    fn test_parse_list() {
        let parser = NomDslParser::new();
        let result = parser.parse("[1 2 3]").unwrap();
        assert_eq!(result.len(), 1);
        match &result[0] {
            Expr::ListLiteral(items) => assert_eq!(items.len(), 3),
            _ => panic!("Expected ListLiteral"),
        }
    }

    #[test]
    fn test_parse_map() {
        let parser = NomDslParser::new();
        let result = parser.parse("{:name \"test\" :count 42}").unwrap();
        assert_eq!(result.len(), 1);
        match &result[0] {
            Expr::MapLiteral(pairs) => assert_eq!(pairs.len(), 2),
            _ => panic!("Expected MapLiteral"),
        }
    }

    #[test]
    fn test_parse_dotted_keyword() {
        let parser = NomDslParser::new();
        let result = parser.parse(":customer.address.city").unwrap();
        assert_eq!(result.len(), 1);
        match &result[0] {
            Expr::DottedKeyword(parts) => {
                assert_eq!(parts, &vec!["customer", "address", "city"]);
            }
            _ => panic!("Expected DottedKeyword"),
        }
    }

    #[test]
    fn test_parse_attr_ref_braces() {
        let parser = NomDslParser::new();
        let result = parser.parse("@attr{uuid-001}").unwrap();
        assert_eq!(result.len(), 1);
        match &result[0] {
            Expr::AttributeRef(uuid) => assert_eq!(uuid, "uuid-001"),
            _ => panic!("Expected AttributeRef"),
        }
    }

    #[test]
    fn test_parse_kebab_case_word() {
        let parser = NomDslParser::new();
        let result = parser.parse("(cbu.attach-entity :id \"123\")").unwrap();
        assert_eq!(result.len(), 1);
        match &result[0] {
            Expr::WordCall { name, args } => {
                assert_eq!(name, "cbu.attach-entity");
                assert_eq!(args.len(), 2);
            }
            _ => panic!("Expected WordCall"),
        }
    }
}
