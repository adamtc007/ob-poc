//! Nom-based parser for the DSL Forth Engine.

use nom::{
    branch::alt,
    bytes::complete::{escaped_transform, tag},
    character::complete::{alpha1, alphanumeric1, char, i64, multispace0, none_of},
    combinator::{map, recognize},
    multi::many0,
    sequence::{delimited, pair},
    IResult,
};

use crate::forth_engine::ast::{DslParser, Expr};
use crate::forth_engine::errors::EngineError;

pub struct NomKycParser;

impl Default for NomKycParser {
    fn default() -> Self {
        Self::new()
    }
}

impl NomKycParser {
    pub fn new() -> Self {
        NomKycParser
    }
}

impl DslParser for NomKycParser {
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
    delimited(multispace0, alt((parse_s_expr, parse_atom)), multispace0)(input)
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
        map(parse_integer_literal, Expr::IntegerLiteral),
        map(parse_bool_literal, Expr::BoolLiteral),
        map(parse_string_literal, Expr::StringLiteral),
        map(parse_keyword, Expr::Keyword),
        map(parse_attribute_ref, Expr::AttributeRef),
        map(parse_document_ref, Expr::DocumentRef),
    ))(input)
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

fn parse_integer_literal(input: &str) -> IResult<&str, i64> {
    i64(input)
}

fn parse_bool_literal(input: &str) -> IResult<&str, bool> {
    alt((map(tag("true"), |_| true), map(tag("false"), |_| false)))(input)
}

fn parse_symbol(input: &str) -> IResult<&str, &str> {
    recognize(pair(
        alt((alpha1, tag("_"), tag("-"), tag("."))),
        many0(alt((alphanumeric1, tag("_"), tag("-"), tag(".")))),
    ))(input)
}

fn parse_attribute_ref(input: &str) -> IResult<&str, String> {
    map(
        delimited(tag("@attr("), parse_string_literal, char(')')),
        |s| s.to_string(),
    )(input)
}

fn parse_document_ref(input: &str) -> IResult<&str, String> {
    map(
        delimited(tag("@doc("), parse_string_literal, char(')')),
        |s| s.to_string(),
    )(input)
}
