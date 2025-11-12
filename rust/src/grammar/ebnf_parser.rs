//! EBNF parser for processing grammar rule definitions
//!
//! This module provides comprehensive EBNF (Extended Backus-Naur Form) parsing
//! capabilities for the DSL grammar system. It handles parsing, validation,
//! and compilation of grammar rules stored in the database.

use crate::ast::types::{ErrorSeverity, SourceLocation, ValidationError, ValidationState};
use nom::{
    branch::alt,
    bytes::complete::{tag, take_until},
    character::complete::{alpha1, alphanumeric1, char, multispace0},
    combinator::{opt, recognize},
    multi::many0,
    sequence::pair,
    IResult,
};
use serde::{Deserialize, Serialize};
use std::collections::{HashMap, HashSet};
use thiserror::Error;

/// EBNF parser for grammar rules
pub struct EBNFParser {
    /// Current parsing context
    context: ParsingContext,
    /// Symbol table for rule references
    symbol_table: HashMap<String, EBNFSymbol>,
    /// Dependency tracker
    dependencies: HashSet<String>,
}

/// Parsing context for EBNF rules
#[derive(Debug, Clone)]
pub(crate) struct ParsingContext {
    pub current_rule: Option<String>,
    pub current_line: usize,
    pub current_column: usize,
    pub in_terminal: bool,
    pub in_production: bool,
}

/// Parsed EBNF rule representation
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct EBNFRule {
    pub name: String,
    pub definition: EBNFExpression,
    pub rule_type: EBNFRuleType,
    pub dependencies: Vec<String>,
    pub metadata: HashMap<String, String>,
}

/// EBNF rule types
#[derive(Debug, Clone, Serialize, Deserialize, PartialEq)]
pub(crate) enum EBNFRuleType {
    Production,
    Terminal,
    Lexical,
}

/// EBNF expression types
#[derive(Debug, Clone, Serialize, Deserialize)]
pub enum EBNFExpression {
    /// Non-terminal reference
    NonTerminal(String),
    /// Terminal string
    Terminal(String),
    /// Sequence of expressions
    Sequence(Vec<EBNFExpression>),
    /// Choice between expressions
    Choice(Vec<EBNFExpression>),
    /// Optional expression
    Optional(Box<EBNFExpression>),
    /// Zero or more repetitions
    ZeroOrMore(Box<EBNFExpression>),
    /// One or more repetitions
    OneOrMore(Box<EBNFExpression>),
    /// Grouped expression
    Group(Box<EBNFExpression>),
    /// Character class
    CharacterClass(String),
    /// Range of characters
    CharacterRange(char, char),
    /// Except expression (A - B)
    Except(Box<EBNFExpression>, Box<EBNFExpression>),
}

/// Symbol in the grammar
#[derive(Debug, Clone, Serialize, Deserialize)]
pub(crate) struct EBNFSymbol {
    pub name: String,
    pub symbol_type: SymbolType,
    pub definition: Option<EBNFExpression>,
    pub first_set: HashSet<String>,
    pub follow_set: HashSet<String>,
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq)]
pub(crate) enum SymbolType {
    Terminal,
    NonTerminal,
    StartSymbol,
}

/// Compilation target for EBNF rules
#[derive(Debug, Clone)]
pub(crate) struct CompilationTarget {
    pub target_type: TargetType,
    pub options: HashMap<String, String>,
}

#[derive(Debug, Clone)]
pub(crate) enum TargetType {
    Nom,   // Rust nom parser combinators
    Regex, // Regular expressions
    PEG,   // Parsing Expression Grammar
    LALR,  // LALR parser table
}

#[derive(Debug, Error)]
pub enum EBNFError {
    #[error("Parse error at line {line}, column {column}: {message}")]
    ParseError {
        line: usize,
        column: usize,
        message: String,
    },

    #[error("Undefined symbol: {symbol}")]
    UndefinedSymbol { symbol: String },

    #[error("Circular dependency detected: {cycle:?}")]
    CircularDependency { cycle: Vec<String> },

    #[error("Invalid rule definition: {message}")]
    InvalidRule { message: String },

    #[error("Compilation error: {message}")]
    CompilationError { message: String },

    #[error("Left recursion detected in rule: {rule}")]
    LeftRecursion { rule: String },
}

impl EBNFParser {
    /// Create a new EBNF parser
    pub fn new() -> Self {
        Self {
            context: ParsingContext::new(),
            symbol_table: HashMap::new(),
            dependencies: HashSet::new(),
        }
    }

    /// Parse an EBNF rule definition
    pub(crate) fn parse_rule(&mut self, input: &str) -> Result<EBNFRule, EBNFError> {
        self.context.reset();

        let (remaining, (rule_name, definition)) =
            self.parse_rule_definition(input)
                .map_err(|e| EBNFError::ParseError {
                    line: self.context.current_line,
                    column: self.context.current_column,
                    message: format!("Failed to parse rule: {:?}", e),
                })?;

        if !remaining.trim().is_empty() {
            return Err(EBNFError::ParseError {
                line: self.context.current_line,
                column: self.context.current_column,
                message: format!("Unexpected input after rule definition: '{}'", remaining),
            });
        }

        let rule_type = self.infer_rule_type(&definition);
        let dependencies = self.extract_dependencies_from_expression(&definition);

        Ok(EBNFRule {
            name: rule_name,
            definition,
            rule_type,
            dependencies,
            metadata: HashMap::new(),
        })
    }

    /// Parse a complete rule definition (name ::= expression)
    fn parse_rule_definition(&mut self, input: &str) -> IResult<&str, (String, EBNFExpression)> {
        let (input, _) = multispace0(input)?;
        let (input, rule_name) = self.parse_identifier(input)?;
        let (input, _) = multispace0(input)?;
        let (input, _) = tag("::=")(input)?;
        let (input, _) = multispace0(input)?;
        let (input, expression) = self.parse_expression(input)?;
        let (input, _) = multispace0(input)?;
        let (input, _) = opt(char(';'))(input)?; // Optional semicolon terminator

        self.context.current_rule = Some(rule_name.clone());
        Ok((input, (rule_name, expression)))
    }

    /// Parse an EBNF expression
    fn parse_expression(&mut self, input: &str) -> IResult<&str, EBNFExpression> {
        self.parse_choice(input)
    }

    /// Parse choice expressions (A | B | C)
    fn parse_choice(&mut self, input: &str) -> IResult<&str, EBNFExpression> {
        let (input, first) = self.parse_sequence(input)?;
        let (input, rest) = many0(preceded(
            tuple((multispace0, char('|'), multispace0)),
            |i| self.parse_sequence(i),
        ))(input)?;

        if rest.is_empty() {
            Ok((input, first))
        } else {
            let mut choices = vec![first];
            choices.extend(rest);
            Ok((input, EBNFExpression::Choice(choices)))
        }
    }

    /// Parse sequence expressions (A B C)
    fn parse_sequence(&mut self, input: &str) -> IResult<&str, EBNFExpression> {
        // Parse factors manually to avoid borrow checker issues
        let mut items = Vec::new();
        let mut input = input;

        // Parse at least one factor
        let (remaining, factor) = self.parse_factor(input)?;
        items.push(factor);
        input = remaining;
        let (mut input, _) = multispace0(input)?;

        // Parse additional factors
        loop {
            match self.parse_factor(input) {
                Ok((remaining, factor)) => {
                    items.push(factor);
                    let (remaining, _) = multispace0::<&str, nom::error::Error<&str>>(remaining)?;
                    input = remaining;
                }
                Err(_) => break,
            }
        }

        if items.len() == 1 {
            Ok((input, items.into_iter().next().unwrap()))
        } else {
            Ok((input, EBNFExpression::Sequence(items)))
        }
    }

    /// Parse factor expressions (terminals, non-terminals, groups, etc.)
    fn parse_factor(&mut self, input: &str) -> IResult<&str, EBNFExpression> {
        let (input, base) = self
            .parse_grouped(input)
            .or_else(|_| self.parse_optional(input))
            .or_else(|_| self.parse_zero_or_more(input))
            .or_else(|_| self.parse_one_or_more(input))
            .or_else(|_| self.parse_terminal(input))
            .or_else(|_| self.parse_character_class(input))
            .or_else(|_| self.parse_character_range(input))
            .or_else(|_| self.parse_non_terminal(input))?;

        // Handle postfix operators
        let (input, result) =
            if let Ok((input, _)) = char::<&str, nom::error::Error<&str>>('?')(input) {
                (input, EBNFExpression::Optional(Box::new(base.clone())))
            } else if let Ok((input, _)) = char::<&str, nom::error::Error<&str>>('*')(input) {
                (input, EBNFExpression::ZeroOrMore(Box::new(base.clone())))
            } else if let Ok((input, _)) = char::<&str, nom::error::Error<&str>>('+')(input) {
                (input, EBNFExpression::OneOrMore(Box::new(base.clone())))
            } else {
                (input, base)
            };

        Ok((input, result))
    }

    /// Parse grouped expression ( ... )
    fn parse_grouped(&mut self, input: &str) -> IResult<&str, EBNFExpression> {
        let (input, expr) = delimited(
            char('('),
            |i| {
                let (i, _) = multispace0(i)?;
                let (i, expr) = self.parse_expression(i)?;
                let (i, _) = multispace0(i)?;
                Ok((i, expr))
            },
            char(')'),
        )(input)?;

        Ok((input, EBNFExpression::Group(Box::new(expr))))
    }

    /// Parse optional expression [ ... ]
    fn parse_optional(&mut self, input: &str) -> IResult<&str, EBNFExpression> {
        let (input, expr) = delimited(
            char('['),
            |i| {
                let (i, _) = multispace0(i)?;
                let (i, expr) = self.parse_expression(i)?;
                let (i, _) = multispace0(i)?;
                Ok((i, expr))
            },
            char(']'),
        )(input)?;

        Ok((input, EBNFExpression::Optional(Box::new(expr))))
    }

    /// Parse zero or more repetition { ... }
    fn parse_zero_or_more(&mut self, input: &str) -> IResult<&str, EBNFExpression> {
        let (input, expr) = delimited(
            char('{'),
            |i| {
                let (i, _) = multispace0(i)?;
                let (i, expr) = self.parse_expression(i)?;
                let (i, _) = multispace0(i)?;
                Ok((i, expr))
            },
            char('}'),
        )(input)?;

        Ok((input, EBNFExpression::ZeroOrMore(Box::new(expr))))
    }

    /// Parse one or more repetition { ... }+
    fn parse_one_or_more(&mut self, input: &str) -> IResult<&str, EBNFExpression> {
        let (input, _) = char('{')(input)?;
        let (input, _) = multispace0(input)?;
        let (input, expr) = self.parse_expression(input)?;
        let (input, _) = multispace0(input)?;
        let (input, _) = char('}')(input)?;
        let (input, _) = char('+')(input)?;

        Ok((input, EBNFExpression::OneOrMore(Box::new(expr))))
    }

    /// Parse terminal string "..."
    fn parse_terminal(&mut self, input: &str) -> IResult<&str, EBNFExpression> {
        self.parse_double_quoted_terminal(input)
            .or_else(|_| self.parse_single_quoted_terminal(input))
    }

    fn parse_double_quoted_terminal(&mut self, input: &str) -> IResult<&str, EBNFExpression> {
        let (input, _) = char('"')(input)?;
        let (input, content) = take_until("\"")(input)?;
        let (input, _) = char('"')(input)?;

        Ok((input, EBNFExpression::Terminal(content.to_string())))
    }

    fn parse_single_quoted_terminal(&mut self, input: &str) -> IResult<&str, EBNFExpression> {
        let (input, _) = char('\'')(input)?;
        let (input, content) = take_until("'")(input)?;
        let (input, _) = char('\'')(input)?;

        Ok((input, EBNFExpression::Terminal(content.to_string())))
    }

    /// Parse character class [a-zA-Z]
    fn parse_character_class(&mut self, input: &str) -> IResult<&str, EBNFExpression> {
        let (input, _) = char('[')(input)?;
        let (input, content) = take_until("]")(input)?;
        let (input, _) = char(']')(input)?;

        Ok((input, EBNFExpression::CharacterClass(content.to_string())))
    }

    /// Parse character range 'a'..'z'
    fn parse_character_range(&mut self, input: &str) -> IResult<&str, EBNFExpression> {
        let (input, _) = char('\'')(input)?;
        let (input, start_char) = nom::character::complete::anychar(input)?;
        let (input, _) = char('\'')(input)?;
        let (input, _) = tag("..")(input)?;
        let (input, _) = char('\'')(input)?;
        let (input, end_char) = nom::character::complete::anychar(input)?;
        let (input, _) = char('\'')(input)?;

        Ok((input, EBNFExpression::CharacterRange(start_char, end_char)))
    }

    /// Parse non-terminal identifier
    fn parse_non_terminal(&mut self, input: &str) -> IResult<&str, EBNFExpression> {
        let (input, identifier) = self.parse_identifier(input)?;
        self.dependencies.insert(identifier.clone());
        Ok((input, EBNFExpression::NonTerminal(identifier)))
    }

    /// Parse identifier (rule name or non-terminal reference)
    fn parse_identifier(&mut self, input: &str) -> IResult<&str, String> {
        let (input, id) = recognize(pair(
            alt((alpha1, tag("_"))),
            many0(alt((alphanumeric1, tag("_"), tag("-")))),
        ))(input)?;

        Ok((input, id.to_string()))
    }

    /// Infer the type of rule based on its definition
    fn infer_rule_type(&self, expression: &EBNFExpression) -> EBNFRuleType {
        match expression {
            EBNFExpression::Terminal(_) => EBNFRuleType::Terminal,
            EBNFExpression::CharacterClass(_) => EBNFRuleType::Lexical,
            EBNFExpression::CharacterRange(_, _) => EBNFRuleType::Lexical,
            _ => EBNFRuleType::Production,
        }
    }

    /// Extract dependencies from an expression
    fn extract_dependencies_from_expression(&self, expression: &EBNFExpression) -> Vec<String> {
        let mut deps = Vec::new();
        self.collect_dependencies(expression, &mut deps);
        deps.sort();
        deps.dedup();
        deps
    }

    /// Recursively collect dependencies from an expression
    fn collect_dependencies(&self, expression: &EBNFExpression, deps: &mut Vec<String>) {
        match expression {
            EBNFExpression::NonTerminal(name) => {
                deps.push(name.clone());
            }
            EBNFExpression::Sequence(items) | EBNFExpression::Choice(items) => {
                for item in items {
                    self.collect_dependencies(item, deps);
                }
            }
            EBNFExpression::Optional(expr)
            | EBNFExpression::ZeroOrMore(expr)
            | EBNFExpression::OneOrMore(expr)
            | EBNFExpression::Group(expr) => {
                self.collect_dependencies(expr, deps);
            }
            EBNFExpression::Except(left, right) => {
                self.collect_dependencies(left, deps);
                self.collect_dependencies(right, deps);
            }
            _ => {} // Terminals don't have dependencies
        }
    }

    /// Extract dependencies from a rule definition string
    pub(crate) fn extract_dependencies(
        &mut self,
        rule_definition: &str,
    ) -> Result<Vec<String>, EBNFError> {
        self.dependencies.clear();
        let parsed_rule = self.parse_rule(rule_definition)?;
        Ok(parsed_rule.dependencies)
    }

    /// Compile rule to executable form
    pub(crate) fn compile_to_executable(&self, rule: &EBNFRule) -> Result<String, EBNFError> {
        match &rule.rule_type {
            EBNFRuleType::Production => self.compile_to_nom_parser(rule),
            EBNFRuleType::Terminal => self.compile_to_regex(rule),
            EBNFRuleType::Lexical => self.compile_to_regex(rule),
        }
    }

    /// Compile rule to Nom parser combinator code
    fn compile_to_nom_parser(&self, rule: &EBNFRule) -> Result<String, EBNFError> {
        let parser_code = self.expression_to_nom(&rule.definition)?;

        Ok(format!(
            "pub fn parse_{}(input: &str) -> IResult<&str, &str> {{\n    {}\n}}",
            rule.name.replace("-", "_"),
            parser_code
        ))
    }

    /// Convert EBNF expression to Nom parser code
    fn expression_to_nom(&self, expr: &EBNFExpression) -> Result<String, EBNFError> {
        match expr {
            EBNFExpression::Terminal(s) => Ok(format!("tag(\"{}\")", s)),
            EBNFExpression::NonTerminal(name) => Ok(format!("parse_{}", name.replace("-", "_"))),
            EBNFExpression::Sequence(items) => {
                let item_codes: Result<Vec<_>, _> = items
                    .iter()
                    .map(|item| self.expression_to_nom(item))
                    .collect();
                let item_codes = item_codes?;
                Ok(format!("tuple(({}))", item_codes.join(", ")))
            }
            EBNFExpression::Choice(items) => {
                let item_codes: Result<Vec<_>, _> = items
                    .iter()
                    .map(|item| self.expression_to_nom(item))
                    .collect();
                let item_codes = item_codes?;
                Ok(format!("alt(({}))", item_codes.join(", ")))
            }
            EBNFExpression::Optional(expr) => {
                let expr_code = self.expression_to_nom(expr)?;
                Ok(format!("opt({})", expr_code))
            }
            EBNFExpression::ZeroOrMore(expr) => {
                let expr_code = self.expression_to_nom(expr)?;
                Ok(format!("many0({})", expr_code))
            }
            EBNFExpression::OneOrMore(expr) => {
                let expr_code = self.expression_to_nom(expr)?;
                Ok(format!("many1({})", expr_code))
            }
            EBNFExpression::Group(expr) => self.expression_to_nom(expr),
            EBNFExpression::CharacterClass(class) => {
                Ok(format!("take_while1(|c: char| \"{}\" .contains(c))", class))
            }
            EBNFExpression::CharacterRange(start, end) => Ok(format!(
                "take_while1(|c: char| c >= '{}' && c <= '{}')",
                start, end
            )),
            EBNFExpression::Except(_, _) => Err(EBNFError::CompilationError {
                message: "Except expressions not yet supported in Nom compilation".to_string(),
            }),
        }
    }

    /// Compile rule to regular expression
    fn compile_to_regex(&self, rule: &EBNFRule) -> Result<String, EBNFError> {
        let regex_pattern = self.expression_to_regex(&rule.definition)?;
        Ok(format!("^{}$", regex_pattern))
    }

    /// Convert EBNF expression to regex pattern
    fn expression_to_regex(&self, expr: &EBNFExpression) -> Result<String, EBNFError> {
        match expr {
            EBNFExpression::Terminal(s) => {
                // Escape regex special characters
                let escaped = regex::escape(s);
                Ok(escaped)
            }
            EBNFExpression::Sequence(items) => {
                let item_patterns: Result<Vec<_>, _> = items
                    .iter()
                    .map(|item| self.expression_to_regex(item))
                    .collect();
                let item_patterns = item_patterns?;
                Ok(item_patterns.join(""))
            }
            EBNFExpression::Choice(items) => {
                let item_patterns: Result<Vec<_>, _> = items
                    .iter()
                    .map(|item| self.expression_to_regex(item))
                    .collect();
                let item_patterns = item_patterns?;
                Ok(format!("({})", item_patterns.join("|")))
            }
            EBNFExpression::Optional(expr) => {
                let pattern = self.expression_to_regex(expr)?;
                Ok(format!("({})?", pattern))
            }
            EBNFExpression::ZeroOrMore(expr) => {
                let pattern = self.expression_to_regex(expr)?;
                Ok(format!("({})*", pattern))
            }
            EBNFExpression::OneOrMore(expr) => {
                let pattern = self.expression_to_regex(expr)?;
                Ok(format!("({})+", pattern))
            }
            EBNFExpression::Group(expr) => {
                let pattern = self.expression_to_regex(expr)?;
                Ok(format!("({})", pattern))
            }
            EBNFExpression::CharacterClass(class) => Ok(format!("[{}]", class)),
            EBNFExpression::CharacterRange(start, end) => Ok(format!("[{}-{}]", start, end)),
            EBNFExpression::NonTerminal(_) => Err(EBNFError::CompilationError {
                message: "Non-terminals cannot be compiled to regex".to_string(),
            }),
            EBNFExpression::Except(_, _) => Err(EBNFError::CompilationError {
                message: "Except expressions not supported in regex compilation".to_string(),
            }),
        }
    }

    /// Validate that all rule dependencies exist
    pub(crate) fn validate_rule_dependencies(
        &self,
        rule: &EBNFRule,
        available_rules: &HashSet<String>,
    ) -> ValidationState {
        let mut errors = Vec::new();

        for dep in &rule.dependencies {
            if !available_rules.contains(dep) {
                errors.push(ValidationError {
                    code: "MISSING_DEPENDENCY".to_string(),
                    message: format!("Rule '{}' depends on undefined rule '{}'", rule.name, dep),
                    severity: ErrorSeverity::Error,
                    location: None,
                    suggestions: vec![
                        format!("Define rule '{}'", dep),
                        format!("Remove dependency on '{}'", dep),
                    ],
                });
            }
        }

        if errors.is_empty() {
            ValidationState::Valid
        } else {
            ValidationState::Invalid { errors }
        }
    }

    /// Check for left recursion in a rule
    pub(crate) fn check_left_recursion(&self, rule: &EBNFRule) -> Result<(), EBNFError> {
        let mut visited = HashSet::new();
        self.check_left_recursion_recursive(&rule.definition, &rule.name, &mut visited)
    }

    fn check_left_recursion_recursive(
        &self,
        expr: &EBNFExpression,
        target_rule: &str,
        visited: &mut HashSet<String>,
    ) -> Result<(), EBNFError> {
        match expr {
            EBNFExpression::NonTerminal(name) if name == target_rule => {
                if visited.contains(name) {
                    return Err(EBNFError::LeftRecursion {
                        rule: target_rule.to_string(),
                    });
                }
            }
            EBNFExpression::Sequence(items) => {
                // Only check the first item for left recursion
                if let Some(first) = items.first() {
                    self.check_left_recursion_recursive(first, target_rule, visited)?;
                }
            }
            EBNFExpression::Choice(items) => {
                // Check all alternatives for left recursion
                for item in items {
                    self.check_left_recursion_recursive(item, target_rule, visited)?;
                }
            }
            EBNFExpression::Group(expr) => {
                self.check_left_recursion_recursive(expr, target_rule, visited)?;
            }
            _ => {} // Other expressions don't cause left recursion
        }
        Ok(())
    }
}

impl ParsingContext {
    fn new() -> Self {
        Self {
            current_rule: None,
            current_line: 1,
            current_column: 1,
            in_terminal: false,
            in_production: false,
        }
    }

    fn reset(&mut self) {
        self.current_rule = None;
        self.current_line = 1;
        self.current_column = 1;
        self.in_terminal = false;
        self.in_production = false;
    }
}

impl Default for EBNFParser {
    fn default() -> Self {
        Self::new()
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_simple_terminal_rule() {
        let mut parser = EBNFParser::new();
        let input = r#"identifier ::= "test""#;

        let result = parser.parse_rule(input);
        assert!(result.is_ok());

        let rule = result.unwrap();
        assert_eq!(rule.name, "identifier");
        assert_eq!(rule.rule_type, EBNFRuleType::Terminal);
    }

    #[test]
    fn test_sequence_rule() {
        let mut parser = EBNFParser::new();
        let input = r#"statement ::= "BEGIN" expression "END""#;

        let result = parser.parse_rule(input);
        assert!(result.is_ok());

        let rule = result.unwrap();
        assert_eq!(rule.name, "statement");
        match rule.definition {
            EBNFExpression::Sequence(items) => assert_eq!(items.len(), 3),
            _ => panic!("Expected sequence"),
        }
    }

    #[test]
    fn test_choice_rule() {
        let mut parser = EBNFParser::new();
        let input = r#"operator ::= "+" | "-" | "*" | "/""#;

        let result = parser.parse_rule(input);
        assert!(result.is_ok());

        let rule = result.unwrap();
        match rule.definition {
            EBNFExpression::Choice(items) => assert_eq!(items.len(), 4),
            _ => panic!("Expected choice"),
        }
    }

    #[test]
    fn test_optional_rule() {
        let mut parser = EBNFParser::new();
        let input = r#"maybe_sign ::= ["+"|"-"]"#;

        let result = parser.parse_rule(input);
        assert!(result.is_ok());

        let rule = result.unwrap();
        match rule.definition {
            EBNFExpression::Optional(_) => assert!(true),
            _ => panic!("Expected optional"),
        }
    }

    #[test]
    fn test_repetition_rules() {
        let mut parser = EBNFParser::new();

        // Zero or more
        let input1 = r#"digits ::= {digit}"#;
        let result1 = parser.parse_rule(input1);
        assert!(result1.is_ok());
        match result1.unwrap().definition {
            EBNFExpression::ZeroOrMore(_) => assert!(true),
            _ => panic!("Expected zero or more"),
        }

        // One or more
        let input2 = r#"digits ::= {digit}+"#;
        let result2 = parser.parse_rule(input2);
        assert!(result2.is_ok());
        match result2.unwrap().definition {
            EBNFExpression::OneOrMore(_) => assert!(true),
            _ => panic!("Expected one or more"),
        }
    }

    #[test]
    fn test_dependency_extraction() {
        let mut parser = EBNFParser::new();
        let input = r#"expression ::= term "+" expression | term"#;

        let result = parser.parse_rule(input);
        assert!(result.is_ok());

        let rule = result.unwrap();
        assert!(rule.dependencies.contains(&"term".to_string()));
        assert!(rule.dependencies.contains(&"expression".to_string()));
    }

    #[test]
    fn test_nom_compilation() {
        let mut parser = EBNFParser::new();
        let input = r#"keyword ::= "if""#;

        let rule = parser.parse_rule(input).unwrap();
        let compiled = parser.compile_to_executable(&rule).unwrap();

        assert!(compiled.contains("tag(\"if\")"));
        assert!(compiled.contains("pub fn parse_keyword"));
    }

    #[test]
    fn test_regex_compilation() {
        let mut parser = EBNFParser::new();
        let input = r#"digit ::= [0-9]"#;

        let rule = parser.parse_rule(input).unwrap();
        let compiled = parser.compile_to_executable(&rule).unwrap();

        assert!(compiled.contains("[0-9]"));
    }

    #[test]
    fn test_character_range() {
        let mut parser = EBNFParser::new();
        let input = r#"letter ::= 'a'..'z'"#;

        let result = parser.parse_rule(input);
        assert!(result.is_ok());

        let rule = result.unwrap();
        match rule.definition {
            EBNFExpression::CharacterRange('a', 'z') => assert!(true),
            _ => panic!("Expected character range"),
        }
    }

    #[test]
    fn test_left_recursion_detection() {
        let mut parser = EBNFParser::new();
        let input = r#"expr ::= expr "+" term"#;

        let rule = parser.parse_rule(input).unwrap();
        let result = parser.check_left_recursion(&rule);

        assert!(result.is_err());
        match result {
            Err(EBNFError::LeftRecursion { rule: rule_name }) => {
                assert_eq!(rule_name, "expr");
            }
            _ => panic!("Expected left recursion error"),
        }
    }

    #[test]
    fn test_dependency_validation() {
        let parser = EBNFParser::new();
        let rule = EBNFRule {
            name: "test".to_string(),
            definition: EBNFExpression::NonTerminal("missing_rule".to_string()),
            rule_type: EBNFRuleType::Production,
            dependencies: vec!["missing_rule".to_string()],
            metadata: HashMap::new(),
        };

        let available_rules = HashSet::new();
        let validation_result = parser.validate_rule_dependencies(&rule, &available_rules);

        match validation_result {
            ValidationState::Invalid { errors } => {
                assert!(!errors.is_empty());
                assert_eq!(errors[0].code, "MISSING_DEPENDENCY");
            }
            _ => panic!("Expected validation errors"),
        }
    }
}
