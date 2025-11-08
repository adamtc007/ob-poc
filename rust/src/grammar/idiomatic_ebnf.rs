//! Idiomatic EBNF grammar parser using nom combinators
#![allow(clippy::only_used_in_recursion)]
//!
//! This module provides a clean, stateless EBNF parser that follows Rust idioms
//! and works properly with nom's combinator system without borrow checker issues.

use std::collections::HashMap;

use nom::{
    branch::alt,
    bytes::complete::{tag, take_until},
    character::complete::{alpha1, alphanumeric1, char, multispace0, one_of, space0, space1},
    combinator::{cut, map, opt, recognize},
    error::{context, ParseError, VerboseError},
    multi::{many0, many1, separated_list1},
    sequence::{delimited, pair, preceded, terminated, tuple},
    Finish, IResult,
};

use serde::{Deserialize, Serialize};

/// EBNF parser error type with context information
pub type EBNFError<'a> = VerboseError<&'a str>;
pub type EBNFResult<'a, T> = IResult<&'a str, T, EBNFError<'a>>;

/// EBNF Grammar representation
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct EBNFGrammar {
    pub rules: HashMap<String, EBNFRule>,
    pub start_rule: Option<String>,
}

/// Individual EBNF rule
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct EBNFRule {
    pub name: String,
    pub expression: EBNFExpression,
    pub comment: Option<String>,
}

/// EBNF expressions - recursive structure
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub enum EBNFExpression {
    /// Terminal string literal "text" or 'text'
    Terminal(String),
    /// Non-terminal reference to another rule
    NonTerminal(String),
    /// Character class [abc] or [a-z]
    CharacterClass(String),
    /// Character range a-z
    CharacterRange(char, char),
    /// Sequence of expressions: A B C
    Sequence(Vec<EBNFExpression>),
    /// Choice between expressions: A | B | C
    Choice(Vec<EBNFExpression>),
    /// Optional expression: A?
    Optional(Box<EBNFExpression>),
    /// Zero or more: A*
    ZeroOrMore(Box<EBNFExpression>),
    /// One or more: A+
    OneOrMore(Box<EBNFExpression>),
    /// Grouped expression: (A B | C)
    Group(Box<EBNFExpression>),
}

/// Stateless EBNF parser
#[derive(Debug, Default)]
pub struct EBNFParser;

impl EBNFParser {
    /// Create a new EBNF parser
    pub fn new() -> Self {
        Self
    }

    /// Parse a complete EBNF grammar
    pub fn parse_grammar(&self, input: &str) -> Result<EBNFGrammar, String> {
        let (remaining, rules) = grammar(input)
            .finish()
            .map_err(|e| format!("Parse error: {:?}", e))?;

        if !remaining.trim().is_empty() {
            return Err(format!("Unexpected remaining input: {}", remaining.trim()));
        }

        let rule_map: HashMap<String, EBNFRule> = rules
            .into_iter()
            .map(|rule| (rule.name.clone(), rule))
            .collect();

        // Find the first rule as start rule (or could be specified)
        let start_rule = rule_map.keys().next().cloned();

        Ok(EBNFGrammar {
            rules: rule_map,
            start_rule,
        })
    }
}

// Standalone parser functions to avoid lifetime issues

/// Parse complete grammar: multiple rules
fn grammar(input: &str) -> EBNFResult<'_, Vec<EBNFRule>> {
    context(
        "grammar",
        preceded(multispace0, many1(terminated(rule, multispace0))),
    )(input)
}

/// Parse a single EBNF rule: rule_name ::= expression ;
fn rule(input: &str) -> EBNFResult<'_, EBNFRule> {
    context(
        "rule",
        map(
            tuple((
                terminated(identifier, tuple((space0, tag("::="), space0))),
                cut(terminated(expression, tuple((space0, char(';'))))),
                opt(preceded(space1, comment)),
            )),
            |(name, expression, comment)| EBNFRule {
                name,
                expression,
                comment,
            },
        ),
    )(input)
}

/// Parse EBNF expressions with proper precedence
fn expression(input: &str) -> EBNFResult<'_, EBNFExpression> {
    context("expression", choice)(input)
}

/// Parse choice expressions: A | B | C
fn choice(input: &str) -> EBNFResult<'_, EBNFExpression> {
    context(
        "choice",
        map(
            separated_list1(delimited(space0, char('|'), space0), sequence),
            |choices| {
                if choices.len() == 1 {
                    choices.into_iter().next().unwrap()
                } else {
                    EBNFExpression::Choice(choices)
                }
            },
        ),
    )(input)
}

/// Parse sequence expressions: A B C
fn sequence(input: &str) -> EBNFResult<'_, EBNFExpression> {
    context(
        "sequence",
        map(many1(terminated(postfix, space0)), |terms| {
            if terms.len() == 1 {
                terms.into_iter().next().unwrap()
            } else {
                EBNFExpression::Sequence(terms)
            }
        }),
    )(input)
}

/// Parse postfix operators: ?, *, +
fn postfix(input: &str) -> EBNFResult<'_, EBNFExpression> {
    context("postfix", pair(primary, opt(one_of("?*+"))))(input).map(|(remaining, (expr, op))| {
        let result = match op {
            Some('?') => EBNFExpression::Optional(Box::new(expr)),
            Some('*') => EBNFExpression::ZeroOrMore(Box::new(expr)),
            Some('+') => EBNFExpression::OneOrMore(Box::new(expr)),
            _ => expr,
        };
        (remaining, result)
    })
}

/// Parse primary expressions: terminals, non-terminals, groups
fn primary(input: &str) -> EBNFResult<'_, EBNFExpression> {
    context(
        "primary",
        alt((terminal, character_class, group, non_terminal)),
    )(input)
}

/// Parse grouped expressions: ( expression )
fn group(input: &str) -> EBNFResult<'_, EBNFExpression> {
    context(
        "group",
        map(
            delimited(char('('), delimited(space0, expression, space0), char(')')),
            |expr| EBNFExpression::Group(Box::new(expr)),
        ),
    )(input)
}

/// Parse terminal strings: "string" or 'string'
fn terminal(input: &str) -> EBNFResult<'_, EBNFExpression> {
    context(
        "terminal",
        alt((double_quoted_terminal, single_quoted_terminal)),
    )(input)
}

/// Parse double-quoted terminal: "string"
fn double_quoted_terminal(input: &str) -> EBNFResult<'_, EBNFExpression> {
    context(
        "double_quoted_terminal",
        map(
            delimited(char('"'), take_until("\""), char('"')),
            |s: &str| EBNFExpression::Terminal(s.to_string()),
        ),
    )(input)
}

/// Parse single-quoted terminal: 'string'
fn single_quoted_terminal(input: &str) -> EBNFResult<'_, EBNFExpression> {
    context(
        "single_quoted_terminal",
        map(
            delimited(char('\''), take_until("'"), char('\'')),
            |s: &str| EBNFExpression::Terminal(s.to_string()),
        ),
    )(input)
}

/// Parse character class: [abc] or [a-z]
fn character_class(input: &str) -> EBNFResult<'_, EBNFExpression> {
    context(
        "character_class",
        map(
            delimited(char('['), take_until("]"), char(']')),
            |s: &str| EBNFExpression::CharacterClass(s.to_string()),
        ),
    )(input)
}

/// Parse character range: a-z
#[allow(dead_code)]
fn character_range(input: &str) -> EBNFResult<'_, EBNFExpression> {
    context(
        "character_range",
        map(
            tuple((none_of("-"), char('-'), none_of("-"))),
            |(start, _, end)| EBNFExpression::CharacterRange(start, end),
        ),
    )(input)
}

/// Parse non-terminal reference: identifier
fn non_terminal(input: &str) -> EBNFResult<'_, EBNFExpression> {
    context(
        "non_terminal",
        map(identifier, EBNFExpression::NonTerminal),
    )(input)
}

/// Parse identifier: letter (letter | digit | '_')*
fn identifier(input: &str) -> EBNFResult<'_, String> {
    context(
        "identifier",
        map(
            recognize(pair(
                alt((alpha1, tag("_"))),
                many0(alt((alphanumeric1, tag("_")))),
            )),
            |s: &str| s.to_string(),
        ),
    )(input)
}

/// Parse comment: // text
fn comment(input: &str) -> EBNFResult<'_, String> {
    context(
        "comment",
        map(preceded(tag("//"), take_until("\n")), |s: &str| {
            s.trim().to_string()
        }),
    )(input)
}

impl EBNFGrammar {
    /// Validate the grammar for consistency
    pub fn validate(&self) -> Result<(), Vec<String>> {
        let mut errors = Vec::new();

        // Check for undefined rule references
        if let Err(mut undefined_errors) = self.check_undefined_references() {
            errors.append(&mut undefined_errors);
        }

        // Check for unreachable rules (if there's a start rule)
        if let Some(start_rule) = &self.start_rule {
            let reachable = self.find_reachable_rules(start_rule);
            for rule_name in self.rules.keys() {
                if !reachable.contains(rule_name) && rule_name != start_rule {
                    errors.push(format!("Rule '{}' is unreachable", rule_name));
                }
            }
        }

        if errors.is_empty() {
            Ok(())
        } else {
            Err(errors)
        }
    }

    fn check_undefined_references(&self) -> Result<(), Vec<String>> {
        let mut errors = Vec::new();
        let rule_names: std::collections::HashSet<&String> = self.rules.keys().collect();

        for rule in self.rules.values() {
            self.check_expr_references(&rule.expression, &rule_names, &mut errors);
        }

        if errors.is_empty() {
            Ok(())
        } else {
            Err(errors)
        }
    }

    fn check_expr_references(
        &self,
        expr: &EBNFExpression,
        rule_names: &std::collections::HashSet<&String>,
        errors: &mut Vec<String>,
    ) {
        match expr {
            EBNFExpression::NonTerminal(name) => {
                if !rule_names.contains(name) {
                    errors.push(format!("Undefined rule reference: '{}'", name));
                }
            }
            EBNFExpression::Sequence(exprs) | EBNFExpression::Choice(exprs) => {
                for sub_expr in exprs {
                    self.check_expr_references(sub_expr, rule_names, errors);
                }
            }
            EBNFExpression::Optional(expr)
            | EBNFExpression::ZeroOrMore(expr)
            | EBNFExpression::OneOrMore(expr)
            | EBNFExpression::Group(expr) => {
                self.check_expr_references(expr, rule_names, errors);
            }
            _ => {} // Terminal expressions don't reference other rules
        }
    }

    fn find_reachable_rules(&self, start_rule: &str) -> std::collections::HashSet<String> {
        let mut reachable = std::collections::HashSet::new();
        let mut to_visit = vec![start_rule.to_string()];

        while let Some(rule_name) = to_visit.pop() {
            if reachable.contains(&rule_name) {
                continue;
            }
            reachable.insert(rule_name.clone());

            if let Some(rule) = self.rules.get(&rule_name) {
                self.find_reachable_in_expr(&rule.expression, &mut to_visit);
            }
        }

        reachable
    }

    fn find_reachable_in_expr(&self, expr: &EBNFExpression, to_visit: &mut Vec<String>) {
        match expr {
            EBNFExpression::NonTerminal(name) => {
                to_visit.push(name.clone());
            }
            EBNFExpression::Sequence(exprs) | EBNFExpression::Choice(exprs) => {
                for sub_expr in exprs {
                    self.find_reachable_in_expr(sub_expr, to_visit);
                }
            }
            EBNFExpression::Optional(expr)
            | EBNFExpression::ZeroOrMore(expr)
            | EBNFExpression::OneOrMore(expr)
            | EBNFExpression::Group(expr) => {
                self.find_reachable_in_expr(expr, to_visit);
            }
            _ => {} // Terminal expressions don't reference other rules
        }
    }
}

/// Helper function to parse single characters not in the given set
#[allow(dead_code)]
fn none_of(chars: &'static str) -> impl Fn(&str) -> EBNFResult<char> {
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
    fn test_simple_rule() {
        let input = "rule ::= \"terminal\" ;";
        let result = rule(input);
        assert!(result.is_ok());

        let (remaining, rule) = result.unwrap();
        assert_eq!(remaining, "");
        assert_eq!(rule.name, "rule");
        assert_eq!(
            rule.expression,
            EBNFExpression::Terminal("terminal".to_string())
        );
    }

    #[test]
    fn test_complex_grammar() {
        let parser = EBNFParser::new();
        let input = r#"
            expression ::= term (('+' | '-') term)* ;
            term ::= factor (('*' | '/') factor)* ;
            factor ::= number | '(' expression ')' ;
            number ::= [0-9]+ ;
        "#;

        let result = parser.parse_grammar(input);
        assert!(result.is_ok());

        let grammar = result.unwrap();
        assert_eq!(grammar.rules.len(), 4);
        assert!(grammar.rules.contains_key("expression"));
        assert!(grammar.rules.contains_key("term"));
        assert!(grammar.rules.contains_key("factor"));
        assert!(grammar.rules.contains_key("number"));
    }

    #[test]
    fn test_terminal_parsing() {
        let result = terminal("\"hello\"");
        assert!(result.is_ok());
        let (_, expr) = result.unwrap();
        assert_eq!(expr, EBNFExpression::Terminal("hello".to_string()));

        let result = terminal("'world'");
        assert!(result.is_ok());
        let (_, expr) = result.unwrap();
        assert_eq!(expr, EBNFExpression::Terminal("world".to_string()));
    }

    #[test]
    fn test_postfix_operators() {
        // Optional
        let result = postfix("\"test\"?");
        assert!(result.is_ok());
        let (_, expr) = result.unwrap();
        match expr {
            EBNFExpression::Optional(inner) => {
                assert_eq!(*inner, EBNFExpression::Terminal("test".to_string()));
            }
            _ => panic!("Expected optional expression"),
        }

        // Zero or more
        let result = postfix("identifier*");
        assert!(result.is_ok());
        let (_, expr) = result.unwrap();
        match expr {
            EBNFExpression::ZeroOrMore(inner) => {
                assert_eq!(
                    *inner,
                    EBNFExpression::NonTerminal("identifier".to_string())
                );
            }
            _ => panic!("Expected zero-or-more expression"),
        }
    }

    #[test]
    fn test_character_class() {
        let result = character_class("[a-z]");
        assert!(result.is_ok());
        let (_, expr) = result.unwrap();
        assert_eq!(expr, EBNFExpression::CharacterClass("a-z".to_string()));
    }

    #[test]
    fn test_validation_undefined_reference() {
        let mut grammar = EBNFGrammar {
            rules: HashMap::new(),
            start_rule: Some("start".to_string()),
        };

        grammar.rules.insert(
            "start".to_string(),
            EBNFRule {
                name: "start".to_string(),
                expression: EBNFExpression::NonTerminal("undefined".to_string()),
                comment: None,
            },
        );

        let result = grammar.validate();
        assert!(result.is_err());
        let errors = result.unwrap_err();
        assert_eq!(errors.len(), 1);
        assert!(errors[0].contains("undefined"));
    }
}
