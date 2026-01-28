//! Schema-guided s-expression parser
//!
//! Parses DSL input using verb schemas to:
//! - Resolve aliases to canonical verb names
//! - Handle positional sugar (convert to keyword form)
//! - Validate argument types
//! - Generate helpful error messages

use super::registry::{HeadResolution, VerbRegistry};
use super::tokenizer::{Span, Token, TokenError, TokenKind, Tokenizer};
use super::types::{ArgDef, ArgShape, VerbSpec};
use std::collections::HashMap;

/// Parsed expression (before canonicalization)
#[derive(Debug, Clone)]
pub struct ParsedExpr {
    /// Resolved verb FQN
    pub verb_fqn: String,
    /// Verb spec used for parsing
    pub spec: VerbSpec,
    /// Arguments (keyword → value)
    pub args: HashMap<String, ParsedValue>,
    /// Source span of entire expression
    pub span: Span,
    /// Parsing feedback (warnings, suggestions)
    pub feedback: Vec<ParseFeedback>,
}

/// Parsed argument value
#[derive(Debug, Clone)]
pub enum ParsedValue {
    String(String, Span),
    Integer(i64, Span),
    Float(f64, Span),
    Bool(bool, Span),
    EntityRef(String, Span),
    BindingRef(String, Span),
    List(Vec<ParsedValue>, Span),
    Null(Span),
}

impl ParsedValue {
    pub fn span(&self) -> Span {
        match self {
            ParsedValue::String(_, s) => *s,
            ParsedValue::Integer(_, s) => *s,
            ParsedValue::Float(_, s) => *s,
            ParsedValue::Bool(_, s) => *s,
            ParsedValue::EntityRef(_, s) => *s,
            ParsedValue::BindingRef(_, s) => *s,
            ParsedValue::List(_, s) => *s,
            ParsedValue::Null(s) => *s,
        }
    }
}

/// Parse feedback (non-fatal issues)
#[derive(Debug, Clone)]
pub struct ParseFeedback {
    pub kind: FeedbackKind,
    pub message: String,
    pub span: Option<Span>,
}

#[derive(Debug, Clone, PartialEq)]
pub enum FeedbackKind {
    AliasUsed,
    PositionalSugar,
    KeywordAlias,
    TypeCoercion,
    UnknownArg,
}

/// Parse error
#[derive(Debug, Clone)]
pub struct ParseError {
    pub message: String,
    pub span: Span,
    pub expected: Vec<String>,
    pub suggestions: Vec<String>,
}

impl std::fmt::Display for ParseError {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        write!(f, "{} at {}", self.message, self.span)
    }
}

impl std::error::Error for ParseError {}

impl From<TokenError> for ParseError {
    fn from(e: TokenError) -> Self {
        ParseError {
            message: e.message,
            span: e.span,
            expected: vec![],
            suggestions: vec![],
        }
    }
}

/// Parse result
pub type ParseResult = Result<ParsedExpr, ParseError>;

/// Schema-guided parser
pub struct Parser<'a> {
    tokens: Vec<Token>,
    pos: usize,
    registry: &'a VerbRegistry,
    feedback: Vec<ParseFeedback>,
}

impl<'a> Parser<'a> {
    /// Parse an s-expression
    pub fn parse(input: &str, registry: &'a VerbRegistry) -> ParseResult {
        let tokens = Tokenizer::tokenize(input)?;
        let mut parser = Parser {
            tokens,
            pos: 0,
            registry,
            feedback: Vec::new(),
        };
        parser.parse_expr()
    }

    fn parse_expr(&mut self) -> ParseResult {
        let start_span = self.current_span();

        // Expect opening paren
        self.expect(TokenKind::LParen)?;

        // Parse verb head
        let (verb_fqn, spec) = self.parse_verb_head()?;

        // Parse arguments
        let args = self.parse_args(&spec)?;

        // Expect closing paren
        let end_span = self.current_span();
        self.expect(TokenKind::RParen)?;

        Ok(ParsedExpr {
            verb_fqn,
            spec,
            args,
            span: start_span.merge(&end_span),
            feedback: std::mem::take(&mut self.feedback),
        })
    }

    fn parse_verb_head(&mut self) -> Result<(String, VerbSpec), ParseError> {
        let token = self.advance()?;
        let span = token.span;

        let head = match &token.kind {
            TokenKind::Symbol(s) => s.clone(),
            _ => {
                return Err(ParseError {
                    message: format!("Expected verb name, got {:?}", token.kind),
                    span,
                    expected: vec!["verb".to_string()],
                    suggestions: vec![],
                });
            }
        };

        match self.registry.resolve_head(&head) {
            HeadResolution::Exact(spec) => Ok((spec.name.clone(), spec)),
            HeadResolution::Alias { alias, spec } => {
                self.feedback.push(ParseFeedback {
                    kind: FeedbackKind::AliasUsed,
                    message: format!("'{}' resolved to '{}'", alias, spec.name),
                    span: Some(span),
                });
                Ok((spec.name.clone(), spec))
            }
            HeadResolution::Ambiguous { alias, candidates } => {
                let suggestions: Vec<String> = candidates.iter().map(|c| c.name.clone()).collect();
                Err(ParseError {
                    message: format!(
                        "Ambiguous verb '{}': could be {}",
                        alias,
                        suggestions.join(" or ")
                    ),
                    span,
                    expected: suggestions.clone(),
                    suggestions,
                })
            }
            HeadResolution::NotFound { input, suggestions } => Err(ParseError {
                message: format!("Unknown verb: '{}'", input),
                span,
                expected: vec!["verb".to_string()],
                suggestions,
            }),
        }
    }

    fn parse_args(&mut self, spec: &VerbSpec) -> Result<HashMap<String, ParsedValue>, ParseError> {
        let mut args: HashMap<String, ParsedValue> = HashMap::new();
        let mut positional_index = 0;

        while !self.check(&TokenKind::RParen) && !self.is_at_end() {
            let token = self.peek()?.clone();

            match &token.kind {
                TokenKind::Keyword(key) => {
                    // Keyword argument
                    let key_span = token.span;
                    let key = key.clone(); // Clone key before advancing
                    self.advance()?;

                    // Resolve keyword aliases
                    let resolved_key = spec
                        .keyword_aliases
                        .get(&key)
                        .cloned()
                        .unwrap_or_else(|| key.clone());

                    if key != resolved_key {
                        self.feedback.push(ParseFeedback {
                            kind: FeedbackKind::KeywordAlias,
                            message: format!("'{}' expanded to '{}'", key, resolved_key),
                            span: Some(key_span),
                        });
                    }

                    // Find arg def
                    let arg_def = spec.args.get(&resolved_key);

                    // Parse value
                    let value = self.parse_value(arg_def)?;

                    if arg_def.is_none() {
                        self.feedback.push(ParseFeedback {
                            kind: FeedbackKind::UnknownArg,
                            message: format!("Unknown argument: '{}'", resolved_key),
                            span: Some(key_span),
                        });
                    }

                    args.insert(resolved_key, value);
                }
                _ => {
                    // Positional argument
                    if positional_index < spec.positional_sugar.len() {
                        let key = spec.positional_sugar[positional_index].clone();
                        let arg_def = spec.args.get(&key);

                        let value_span = token.span;
                        let value = self.parse_value(arg_def)?;

                        self.feedback.push(ParseFeedback {
                            kind: FeedbackKind::PositionalSugar,
                            message: format!("Positional arg {} → :{}", positional_index + 1, key),
                            span: Some(value_span),
                        });

                        args.insert(key, value);
                        positional_index += 1;
                    } else {
                        return Err(ParseError {
                            message: format!(
                                "Unexpected positional argument (max {} allowed for this verb)",
                                spec.positional_sugar.len()
                            ),
                            span: token.span,
                            expected: vec![":keyword".to_string()],
                            suggestions: spec.args.all().map(|a| format!(":{}", a.name)).collect(),
                        });
                    }
                }
            }
        }

        Ok(args)
    }

    fn parse_value(&mut self, arg_def: Option<&ArgDef>) -> Result<ParsedValue, ParseError> {
        let token = self.advance()?;
        let span = token.span;

        match token.kind {
            TokenKind::String(s) => Ok(ParsedValue::String(s, span)),
            TokenKind::Integer(i) => {
                // Check if we should coerce to decimal/string
                if let Some(def) = arg_def {
                    if let ArgShape::Decimal = &def.shape {
                        self.feedback.push(ParseFeedback {
                            kind: FeedbackKind::TypeCoercion,
                            message: format!("Integer {} coerced to decimal", i),
                            span: Some(span),
                        });
                    }
                }
                Ok(ParsedValue::Integer(i, span))
            }
            TokenKind::Float(f) => Ok(ParsedValue::Float(f, span)),
            TokenKind::Bool(b) => Ok(ParsedValue::Bool(b, span)),
            TokenKind::EntityRef(e) => Ok(ParsedValue::EntityRef(e, span)),
            TokenKind::BindingRef(b) => Ok(ParsedValue::BindingRef(b, span)),
            TokenKind::Symbol(s) => {
                // Symbol as value - could be enum, uuid, or unquoted string
                if let Some(def) = arg_def {
                    match &def.shape {
                        ArgShape::Enum { values } => {
                            // Validate enum value
                            let s_upper = s.to_uppercase();
                            if !values.iter().any(|v| v.to_uppercase() == s_upper) {
                                return Err(ParseError {
                                    message: format!(
                                        "Invalid enum value '{}'. Expected one of: {}",
                                        s,
                                        values.join(", ")
                                    ),
                                    span,
                                    expected: values.clone(),
                                    suggestions: values.clone(),
                                });
                            }
                        }
                        ArgShape::Bool => {
                            // Try to parse as boolean
                            return match s.to_lowercase().as_str() {
                                "true" | "yes" | "1" => Ok(ParsedValue::Bool(true, span)),
                                "false" | "no" | "0" => Ok(ParsedValue::Bool(false, span)),
                                _ => Err(ParseError {
                                    message: format!("Invalid boolean: '{}'", s),
                                    span,
                                    expected: vec!["true".to_string(), "false".to_string()],
                                    suggestions: vec!["true".to_string(), "false".to_string()],
                                }),
                            };
                        }
                        _ => {}
                    }
                }
                // Treat as unquoted string
                Ok(ParsedValue::String(s, span))
            }
            TokenKind::LParen => {
                // Nested list
                let mut items = Vec::new();
                let list_start = span;

                while !self.check(&TokenKind::RParen) && !self.is_at_end() {
                    items.push(self.parse_value(None)?);
                }

                let end_span = self.current_span();
                self.expect(TokenKind::RParen)?;

                Ok(ParsedValue::List(items, list_start.merge(&end_span)))
            }
            _ => Err(ParseError {
                message: format!("Unexpected token in value position: {:?}", token.kind),
                span,
                expected: vec!["value".to_string()],
                suggestions: vec![],
            }),
        }
    }

    // Helper methods

    fn peek(&self) -> Result<&Token, ParseError> {
        self.tokens.get(self.pos).ok_or_else(|| ParseError {
            message: "Unexpected end of input".to_string(),
            span: self.tokens.last().map(|t| t.span).unwrap_or_default(),
            expected: vec![],
            suggestions: vec![],
        })
    }

    fn advance(&mut self) -> Result<Token, ParseError> {
        let token = self.peek()?.clone();
        self.pos += 1;
        Ok(token)
    }

    fn check(&self, kind: &TokenKind) -> bool {
        self.tokens
            .get(self.pos)
            .map(|t| std::mem::discriminant(&t.kind) == std::mem::discriminant(kind))
            .unwrap_or(false)
    }

    fn expect(&mut self, expected: TokenKind) -> Result<Token, ParseError> {
        let token = self.advance()?;
        if std::mem::discriminant(&token.kind) != std::mem::discriminant(&expected) {
            return Err(ParseError {
                message: format!("Expected {:?}, got {:?}", expected, token.kind),
                span: token.span,
                expected: vec![format!("{}", expected)],
                suggestions: vec![],
            });
        }
        Ok(token)
    }

    fn is_at_end(&self) -> bool {
        self.pos >= self.tokens.len()
            || matches!(
                self.tokens.get(self.pos).map(|t| &t.kind),
                Some(TokenKind::Eof)
            )
    }

    fn current_span(&self) -> Span {
        self.tokens
            .get(self.pos)
            .map(|t| t.span)
            .unwrap_or_default()
    }
}

/// Convenience function
#[allow(dead_code)]
pub fn parse(input: &str, registry: &VerbRegistry) -> ParseResult {
    Parser::parse(input, registry)
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::mcp::schema::types::*;

    fn test_registry() -> VerbRegistry {
        let mut registry = VerbRegistry::new();

        // Add test verbs
        registry.register(VerbSpec {
            name: "view.drill".to_string(),
            domain: "view".to_string(),
            action: "drill".to_string(),
            aliases: vec!["drill".to_string(), "dive".to_string()],
            args: ArgSchema {
                style: "keyworded".to_string(),
                required: vec![ArgDef {
                    name: "entity".to_string(),
                    shape: ArgShape::EntityRef {
                        allowed_kinds: vec![],
                    },
                    default: None,
                    doc: "Entity to drill into".to_string(),
                    maps_to: None,
                    lookup: None,
                }],
                optional: vec![ArgDef {
                    name: "depth".to_string(),
                    shape: ArgShape::Int,
                    default: Some(serde_json::json!(1)),
                    doc: "Drill depth".to_string(),
                    maps_to: None,
                    lookup: None,
                }],
            },
            positional_sugar: vec!["entity".to_string()],
            keyword_aliases: HashMap::new(),
            doc: "Drill into entity".to_string(),
            tier: "intent".to_string(),
            tags: vec!["navigation".to_string()],
            ..Default::default()
        });

        registry
    }

    #[test]
    fn test_parse_keyword_form() {
        let registry = test_registry();
        let result = parse("(view.drill :entity \"Allianz\")", &registry).unwrap();

        assert_eq!(result.verb_fqn, "view.drill");
        assert!(result.args.contains_key("entity"));
    }

    #[test]
    fn test_parse_positional_sugar() {
        let registry = test_registry();
        let result = parse("(drill \"Allianz\")", &registry).unwrap();

        assert_eq!(result.verb_fqn, "view.drill");
        assert!(result.args.contains_key("entity"));
        assert!(result
            .feedback
            .iter()
            .any(|f| f.kind == FeedbackKind::PositionalSugar));
    }

    #[test]
    fn test_parse_alias_resolution() {
        let registry = test_registry();
        let result = parse("(dive :entity \"X\")", &registry).unwrap();

        assert_eq!(result.verb_fqn, "view.drill");
        assert!(result
            .feedback
            .iter()
            .any(|f| f.kind == FeedbackKind::AliasUsed));
    }

    #[test]
    fn test_parse_entity_ref() {
        let registry = test_registry();
        let result = parse("(drill <Allianz SE>)", &registry).unwrap();

        assert!(matches!(
            result.args.get("entity"),
            Some(ParsedValue::EntityRef(e, _)) if e == "Allianz SE"
        ));
    }
}
