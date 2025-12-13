//! Search Expression - S-expression based query language for entity resolution
//!
//! The search engine is a mini DSL interpreter. Queries are s-expressions
//! with filled-in values (which can be partial).
//!
//! ## Schema Definition (in verb YAML)
//!
//! ```yaml
//! lookup:
//!   entity_type: proper_person
//!   search_key: "(search_name (date_of_birth :selectivity 0.95) (nationality :selectivity 0.7))"
//! ```
//!
//! ## Query Execution (at runtime)
//!
//! ```text
//! # Full query - all fields provided
//! (search_name "John Smith" (date_of_birth "1980-01-15") (nationality "US"))
//!
//! # Partial query - only name and DOB
//! (search_name "John Smith" (date_of_birth "1980-01-15"))
//!
//! # Minimal query - just the primary field
//! (search_name "John Smith")
//! ```
//!
//! ## Semantics
//!
//! - First symbol is the primary search field (required)
//! - Subsequent elements are discriminators (optional)
//! - Each discriminator narrows the result set
//! - Selectivity scores determine ranking boost

use std::collections::HashMap;

// =============================================================================
// S-EXPRESSION AST
// =============================================================================

/// A parsed search expression
#[derive(Debug, Clone, PartialEq)]
pub enum SearchExpr {
    /// A symbol (field name or keyword)
    Symbol(String),
    /// A string literal value
    String(String),
    /// A number literal
    Number(f64),
    /// A list of expressions
    List(Vec<SearchExpr>),
}

impl SearchExpr {
    /// Parse an s-expression string into a SearchExpr
    pub fn parse(input: &str) -> Result<Self, ParseError> {
        let mut chars = input.trim().chars().peekable();
        parse_expr(&mut chars)
    }

    /// Check if this is a list
    pub fn is_list(&self) -> bool {
        matches!(self, SearchExpr::List(_))
    }

    /// Get as symbol string
    pub fn as_symbol(&self) -> Option<&str> {
        match self {
            SearchExpr::Symbol(s) => Some(s),
            _ => None,
        }
    }

    /// Get as string value
    pub fn as_string(&self) -> Option<&str> {
        match self {
            SearchExpr::String(s) => Some(s),
            _ => None,
        }
    }

    /// Get as number
    pub fn as_number(&self) -> Option<f64> {
        match self {
            SearchExpr::Number(n) => Some(*n),
            _ => None,
        }
    }

    /// Get as list
    pub fn as_list(&self) -> Option<&[SearchExpr]> {
        match self {
            SearchExpr::List(l) => Some(l),
            _ => None,
        }
    }
}

#[derive(Debug, Clone, PartialEq)]
pub struct ParseError {
    pub message: String,
    pub position: usize,
}

impl std::fmt::Display for ParseError {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        write!(f, "Parse error at {}: {}", self.position, self.message)
    }
}

impl std::error::Error for ParseError {}

fn parse_expr(chars: &mut std::iter::Peekable<std::str::Chars>) -> Result<SearchExpr, ParseError> {
    skip_whitespace(chars);

    match chars.peek() {
        None => Err(ParseError {
            message: "Unexpected end of input".to_string(),
            position: 0,
        }),
        Some('(') => parse_list(chars),
        Some('"') => parse_string(chars),
        Some(c) if c.is_ascii_digit() || *c == '-' => parse_number_or_symbol(chars),
        Some(_) => parse_symbol(chars),
    }
}

fn parse_list(chars: &mut std::iter::Peekable<std::str::Chars>) -> Result<SearchExpr, ParseError> {
    chars.next(); // consume '('
    let mut elements = Vec::new();

    loop {
        skip_whitespace(chars);
        match chars.peek() {
            None => {
                return Err(ParseError {
                    message: "Unclosed list".to_string(),
                    position: 0,
                })
            }
            Some(')') => {
                chars.next();
                return Ok(SearchExpr::List(elements));
            }
            Some(_) => {
                elements.push(parse_expr(chars)?);
            }
        }
    }
}

fn parse_string(
    chars: &mut std::iter::Peekable<std::str::Chars>,
) -> Result<SearchExpr, ParseError> {
    chars.next(); // consume opening '"'
    let mut s = String::new();

    loop {
        match chars.next() {
            None => {
                return Err(ParseError {
                    message: "Unclosed string".to_string(),
                    position: 0,
                })
            }
            Some('\\') => {
                // Escape sequence
                match chars.next() {
                    Some('n') => s.push('\n'),
                    Some('t') => s.push('\t'),
                    Some('\\') => s.push('\\'),
                    Some('"') => s.push('"'),
                    Some(c) => s.push(c),
                    None => {
                        return Err(ParseError {
                            message: "Incomplete escape sequence".to_string(),
                            position: 0,
                        })
                    }
                }
            }
            Some('"') => return Ok(SearchExpr::String(s)),
            Some(c) => s.push(c),
        }
    }
}

fn parse_symbol(
    chars: &mut std::iter::Peekable<std::str::Chars>,
) -> Result<SearchExpr, ParseError> {
    let mut s = String::new();

    while let Some(&c) = chars.peek() {
        if c.is_whitespace() || c == '(' || c == ')' || c == '"' {
            break;
        }
        s.push(c);
        chars.next();
    }

    if s.is_empty() {
        return Err(ParseError {
            message: "Empty symbol".to_string(),
            position: 0,
        });
    }

    Ok(SearchExpr::Symbol(s))
}

fn parse_number_or_symbol(
    chars: &mut std::iter::Peekable<std::str::Chars>,
) -> Result<SearchExpr, ParseError> {
    let mut s = String::new();

    while let Some(&c) = chars.peek() {
        if c.is_whitespace() || c == '(' || c == ')' || c == '"' {
            break;
        }
        s.push(c);
        chars.next();
    }

    // Try to parse as number
    if let Ok(n) = s.parse::<f64>() {
        Ok(SearchExpr::Number(n))
    } else {
        Ok(SearchExpr::Symbol(s))
    }
}

fn skip_whitespace(chars: &mut std::iter::Peekable<std::str::Chars>) {
    while let Some(&c) = chars.peek() {
        if c.is_whitespace() {
            chars.next();
        } else {
            break;
        }
    }
}

// =============================================================================
// SEARCH SCHEMA (parsed from verb YAML search_key)
// =============================================================================

/// Schema definition for a search expression
///
/// Parsed from verb YAML `search_key` field.
#[derive(Debug, Clone)]
pub struct SearchSchema {
    /// Primary search field (required in all queries)
    pub primary_field: String,
    /// Discriminator fields with their selectivity scores
    pub discriminators: Vec<DiscriminatorDef>,
    /// Minimum confidence for auto-resolution
    pub min_confidence: f32,
}

/// A discriminator field definition
#[derive(Debug, Clone)]
pub struct DiscriminatorDef {
    /// Database column name
    pub field: String,
    /// Selectivity score (0.0-1.0, higher = more unique)
    pub selectivity: f32,
    /// Is this field required for resolution?
    pub required: bool,
}

impl SearchSchema {
    /// Parse a schema from s-expression
    ///
    /// ```text
    /// (search_name (date_of_birth :selectivity 0.95) (nationality :selectivity 0.7))
    /// ```
    pub fn parse(input: &str) -> Result<Self, ParseError> {
        let expr = SearchExpr::parse(input)?;
        Self::from_expr(&expr)
    }

    /// Build schema from parsed expression
    pub fn from_expr(expr: &SearchExpr) -> Result<Self, ParseError> {
        let list = expr.as_list().ok_or_else(|| ParseError {
            message: "Schema must be a list".to_string(),
            position: 0,
        })?;

        if list.is_empty() {
            return Err(ParseError {
                message: "Empty schema".to_string(),
                position: 0,
            });
        }

        // First element is primary field
        let primary_field = list[0]
            .as_symbol()
            .ok_or_else(|| ParseError {
                message: "Primary field must be a symbol".to_string(),
                position: 0,
            })?
            .to_string();

        let mut discriminators = Vec::new();
        let mut min_confidence = 0.8f32;

        let mut i = 1;
        while i < list.len() {
            match &list[i] {
                SearchExpr::Symbol(s) if s.starts_with(':') => {
                    // Schema-level option
                    let key = &s[1..];
                    i += 1;
                    if i >= list.len() {
                        break;
                    }
                    if key == "min-confidence" {
                        if let Some(n) = list[i].as_number() {
                            min_confidence = n as f32;
                        }
                    }
                    i += 1;
                }
                SearchExpr::Symbol(s) => {
                    // Simple discriminator
                    discriminators.push(DiscriminatorDef {
                        field: s.clone(),
                        selectivity: 0.5,
                        required: false,
                    });
                    i += 1;
                }
                SearchExpr::List(inner) => {
                    // Discriminator with options
                    let disc = Self::parse_discriminator(inner)?;
                    discriminators.push(disc);
                    i += 1;
                }
                _ => i += 1,
            }
        }

        Ok(SearchSchema {
            primary_field,
            discriminators,
            min_confidence,
        })
    }

    fn parse_discriminator(list: &[SearchExpr]) -> Result<DiscriminatorDef, ParseError> {
        if list.is_empty() {
            return Err(ParseError {
                message: "Empty discriminator".to_string(),
                position: 0,
            });
        }

        let field = list[0]
            .as_symbol()
            .ok_or_else(|| ParseError {
                message: "Discriminator field must be a symbol".to_string(),
                position: 0,
            })?
            .to_string();

        let mut selectivity = 0.5f32;
        let mut required = false;

        let mut i = 1;
        while i < list.len() {
            if let Some(key) = list[i].as_symbol() {
                if let Some(key_name) = key.strip_prefix(':') {
                    i += 1;
                    if i >= list.len() {
                        break;
                    }
                    match key_name {
                        "selectivity" => {
                            if let Some(n) = list[i].as_number() {
                                selectivity = n as f32;
                            }
                        }
                        "required" => {
                            if let Some(s) = list[i].as_symbol() {
                                required = s == "true";
                            }
                        }
                        _ => {}
                    }
                }
            }
            i += 1;
        }

        Ok(DiscriminatorDef {
            field,
            selectivity,
            required,
        })
    }

    /// Get all column names needed for this schema
    pub fn all_columns(&self) -> Vec<&str> {
        let mut cols = vec![self.primary_field.as_str()];
        for d in &self.discriminators {
            cols.push(d.field.as_str());
        }
        cols
    }
}

// =============================================================================
// SEARCH QUERY (runtime query with values)
// =============================================================================

/// A search query with filled-in values
///
/// Parsed from runtime s-expression like:
/// ```text
/// (search_name "John Smith" (date_of_birth "1980-01-15"))
/// ```
#[derive(Debug, Clone)]
pub struct SearchQuery {
    /// Primary field value (required)
    pub primary_value: String,
    /// Discriminator values (field -> value)
    pub discriminators: HashMap<String, String>,
}

impl SearchQuery {
    /// Parse a query from s-expression
    pub fn parse(input: &str) -> Result<Self, ParseError> {
        let expr = SearchExpr::parse(input)?;
        Self::from_expr(&expr)
    }

    /// Build query from parsed expression
    pub fn from_expr(expr: &SearchExpr) -> Result<Self, ParseError> {
        let list = expr.as_list().ok_or_else(|| ParseError {
            message: "Query must be a list".to_string(),
            position: 0,
        })?;

        if list.len() < 2 {
            return Err(ParseError {
                message: "Query must have at least primary field and value".to_string(),
                position: 0,
            });
        }

        // First element is field name (ignored, schema defines it)
        // Second element is primary value
        let primary_value = list[1]
            .as_string()
            .ok_or_else(|| ParseError {
                message: "Primary value must be a string".to_string(),
                position: 0,
            })?
            .to_string();

        let mut discriminators = HashMap::new();

        // Rest are discriminator (field value) pairs
        for item in list.iter().skip(2) {
            if let Some(inner) = item.as_list() {
                if inner.len() >= 2 {
                    if let (Some(field), Some(value)) = (inner[0].as_symbol(), inner[1].as_string())
                    {
                        discriminators.insert(field.to_string(), value.to_string());
                    }
                }
            }
        }

        Ok(SearchQuery {
            primary_value,
            discriminators,
        })
    }

    /// Create a simple query (primary value only)
    pub fn simple(value: &str) -> Self {
        SearchQuery {
            primary_value: value.to_string(),
            discriminators: HashMap::new(),
        }
    }

    /// Create a query from a map of field -> value
    pub fn from_map(primary_field: &str, values: HashMap<String, String>) -> Option<Self> {
        let primary_value = values.get(primary_field)?.clone();
        let mut discriminators = values;
        discriminators.remove(primary_field);
        Some(SearchQuery {
            primary_value,
            discriminators,
        })
    }
}

// =============================================================================
// TESTS
// =============================================================================

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_parse_simple_symbol() {
        let expr = SearchExpr::parse("name").unwrap();
        assert_eq!(expr, SearchExpr::Symbol("name".to_string()));
    }

    #[test]
    fn test_parse_string() {
        let expr = SearchExpr::parse("\"hello world\"").unwrap();
        assert_eq!(expr, SearchExpr::String("hello world".to_string()));
    }

    #[test]
    fn test_parse_number() {
        let expr = SearchExpr::parse("0.95").unwrap();
        assert_eq!(expr, SearchExpr::Number(0.95));
    }

    #[test]
    fn test_parse_simple_list() {
        let expr = SearchExpr::parse("(name date_of_birth)").unwrap();
        assert_eq!(
            expr,
            SearchExpr::List(vec![
                SearchExpr::Symbol("name".to_string()),
                SearchExpr::Symbol("date_of_birth".to_string()),
            ])
        );
    }

    #[test]
    fn test_parse_nested_list() {
        let expr = SearchExpr::parse("(name (dob :selectivity 0.95))").unwrap();
        assert_eq!(
            expr,
            SearchExpr::List(vec![
                SearchExpr::Symbol("name".to_string()),
                SearchExpr::List(vec![
                    SearchExpr::Symbol("dob".to_string()),
                    SearchExpr::Symbol(":selectivity".to_string()),
                    SearchExpr::Number(0.95),
                ]),
            ])
        );
    }

    #[test]
    fn test_parse_schema_simple() {
        let schema = SearchSchema::parse("(search_name date_of_birth nationality)").unwrap();
        assert_eq!(schema.primary_field, "search_name");
        assert_eq!(schema.discriminators.len(), 2);
        assert_eq!(schema.discriminators[0].field, "date_of_birth");
        assert_eq!(schema.discriminators[1].field, "nationality");
    }

    #[test]
    fn test_parse_schema_with_options() {
        let schema = SearchSchema::parse(
            "(search_name (date_of_birth :selectivity 0.95) (nationality :selectivity 0.7))",
        )
        .unwrap();
        assert_eq!(schema.primary_field, "search_name");
        assert_eq!(schema.discriminators.len(), 2);
        assert_eq!(schema.discriminators[0].field, "date_of_birth");
        assert!((schema.discriminators[0].selectivity - 0.95).abs() < 0.01);
        assert_eq!(schema.discriminators[1].field, "nationality");
        assert!((schema.discriminators[1].selectivity - 0.7).abs() < 0.01);
    }

    #[test]
    fn test_parse_query() {
        let query =
            SearchQuery::parse("(search_name \"John Smith\" (date_of_birth \"1980-01-15\"))")
                .unwrap();
        assert_eq!(query.primary_value, "John Smith");
        assert_eq!(
            query.discriminators.get("date_of_birth"),
            Some(&"1980-01-15".to_string())
        );
    }

    #[test]
    fn test_parse_query_minimal() {
        let query = SearchQuery::parse("(search_name \"John Smith\")").unwrap();
        assert_eq!(query.primary_value, "John Smith");
        assert!(query.discriminators.is_empty());
    }
}
