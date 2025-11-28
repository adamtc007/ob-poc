# DSL v2 Refactoring Specification

## Implementation Status

| Component | Status | Location |
|-----------|--------|----------|
| AST Types | **COMPLETE** | `src/dsl_v2/ast.rs` |
| Nom Parser | **COMPLETE** | `src/dsl_v2/parser.rs` |
| Verb Definitions (50 verbs) | **COMPLETE** | `src/dsl_v2/verbs.rs` |
| Column Mappings (22 tables) | **COMPLETE** | `src/dsl_v2/mappings.rs` |
| DslExecutor | **COMPLETE** | `src/dsl_v2/executor.rs` |
| Custom Operations (5 ops) | **COMPLETE** | `src/dsl_v2/custom_ops/mod.rs` |
| Module Integration | **COMPLETE** | `src/lib.rs` |

**Note**: The dsl_v2 module compiles successfully. Pre-existing database schema mismatches in other parts of the codebase require SQL migrations to be run before full integration testing.

---

## Overview

This document specifies a complete refactoring of the ob-poc DSL system from the current dual-grammar, vocab-sprawl architecture to a unified, data-driven design.

### Goals

1. **Single Grammar** - One S-expression syntax: `(domain.verb :key value ...)`
2. **Data-Driven Execution** - 90% of verbs defined as static data, not code
3. **Explicit Custom Operations** - 10% truly custom operations with mandatory rationale
4. **Document↔Attribute Integration** - Bidirectional mapping via `document_type_attributes`
5. **Maintainability** - ~3000 lines vs current ~4000+, clear separation of concerns

### Current Problems Being Solved

- Two grammars (EBNF workflow DSL + Forth-style vocab) don't reconcile
- 100+ vocabulary words with identical implementations
- 2300-line `crud_executor.rs` with extensive match arms
- Weak document-attribute linkage
- No document-type-to-attribute mapping
- Hardcoded DSL keyword synonyms scattered through code

---

## Architecture

```
┌─────────────────────────────────────────────────────────────────────────┐
│                           DSL Source Text                                │
│       (cbu.link :cbu-id @cbu :entity-id @aviva :role "InvestmentManager")│
└─────────────────────────────────────────────────────────────────────────┘
                                    │
                                    ▼
┌─────────────────────────────────────────────────────────────────────────┐
│                     Unified S-Expression Parser (Nom)                    │
│                       src/dsl_v2/parser.rs                              │
└─────────────────────────────────────────────────────────────────────────┘
                                    │
                                    ▼
┌─────────────────────────────────────────────────────────────────────────┐
│                              AST Types                                   │
│                        src/dsl_v2/ast.rs                                │
│              Program → Statement → VerbCall → Arguments                  │
└─────────────────────────────────────────────────────────────────────────┘
                                    │
                                    ▼
┌─────────────────────────────────────────────────────────────────────────┐
│                            DslExecutor                                   │
│                       src/dsl_v2/executor.rs                            │
│                                                                          │
│   ┌─────────────────────────────┐    ┌─────────────────────────────┐   │
│   │     Tier 1: Standard        │    │    Tier 2: Custom Ops       │   │
│   │   (Data-Driven, ~50 verbs)  │    │  (Trait-based, ~5-10 ops)   │   │
│   │                             │    │                             │   │
│   │  find_verb() → VerbDef      │    │  CustomOperationRegistry    │   │
│   │         ↓                   │    │         ↓                   │   │
│   │  match behavior {           │    │  op.execute(args, ctx)      │   │
│   │    Insert → generic_insert  │    │                             │   │
│   │    Link → generic_link      │    │  - ubo.calculate            │   │
│   │    ...                      │    │  - document.extract         │   │
│   │  }                          │    │  - document.catalog         │   │
│   └─────────────────────────────┘    └─────────────────────────────┘   │
└─────────────────────────────────────────────────────────────────────────┘
                                    │
                                    ▼
┌─────────────────────────────────────────────────────────────────────────┐
│                           PostgreSQL                                     │
│    entities, cbus, cbu_entity_roles, document_catalog,                  │
│    document_type_attributes, dictionary, attribute_values_typed...       │
└─────────────────────────────────────────────────────────────────────────┘
```

---

## File Structure

```
src/dsl_v2/
├── mod.rs                    # Module root, re-exports
├── ast.rs                    # AST type definitions
├── parser.rs                 # Nom parser implementation
├── verbs.rs                  # STANDARD_VERBS static array (Tier 1)
├── mappings.rs               # Column mappings (DSL key → DB column)
├── executor.rs               # DslExecutor + generic CRUD functions
└── custom_ops/
    ├── mod.rs                # CustomOperation trait + registry
    ├── ubo_calculate.rs      # UBO graph traversal
    ├── document_catalog.rs   # Document cataloging with type lookup
    ├── document_extract.rs   # AI/OCR extraction
    ├── screening_pep.rs      # PEP screening (external API)
    └── screening_sanctions.rs # Sanctions screening (external API)
```

---

## Part 1: Grammar (EBNF)

File: `docs/dsl-grammar-v2.ebnf`

```ebnf
(* ============================================================================
   ob-poc DSL v2.0 - Unified Grammar
   ============================================================================
   
   Design Principles:
   - Single syntax: (domain.verb :key value ...)
   - S-expression based for homoiconicity
   - Keywords use kebab-case with colon prefix
   - References use @ prefix for late binding
   - Comments use ;; prefix
   
   ============================================================================ *)

(* Top-level: A program is zero or more statements *)
program = { statement } ;

(* Statement: Either a verb call or a comment *)
statement = verb_call | comment ;

(* Verb Call: The core construct - (domain.verb args...) *)
verb_call = '(' , word , { argument } , ')' ;

(* Word: domain.verb pattern *)
word = domain , '.' , verb ;

domain = identifier ;
verb = identifier | kebab_identifier ;

(* Arguments: keyword-value pairs *)
argument = keyword , value ;

(* Keywords: :keyword or :dotted.keyword *)
keyword = ':' , ( dotted_identifier | identifier | kebab_identifier ) ;

dotted_identifier = identifier , { '.' , identifier } ;
kebab_identifier = identifier , { '-' , identifier } ;

(* Values: The types we support *)
value = string_literal
      | number_literal
      | boolean_literal
      | reference
      | attribute_ref
      | document_ref
      | list_literal
      | map_literal
      | null_literal ;

(* References: @name for late-bound identifiers *)
reference = '@' , identifier ;

(* Typed References: @attr{uuid} and @doc{uuid} *)
attribute_ref = '@attr{' , uuid , '}' ;
document_ref = '@doc{' , uuid , '}' ;

(* Literals *)
string_literal = '"' , { string_char } , '"' ;
string_char = ? any character except '"' or '\' ?
            | '\' , escape_char ;
escape_char = 'n' | 'r' | 't' | '\' | '"' ;

number_literal = integer_literal | decimal_literal ;
integer_literal = [ '-' ] , digit , { digit } ;
decimal_literal = [ '-' ] , digit , { digit } , '.' , digit , { digit } ;

boolean_literal = 'true' | 'false' ;

null_literal = 'nil' ;

(* Collections *)
list_literal = '[' , [ value , { ( ',' | whitespace ) , value } ] , ']' ;
map_literal = '{' , [ map_entry , { map_entry } ] , '}' ;
map_entry = keyword , value ;

(* Comments: ;; to end of line *)
comment = ';;' , { ? any character except newline ? } , newline ;

(* Primitives *)
identifier = letter , { letter | digit | '_' } ;
uuid = hex{8} , '-' , hex{4} , '-' , hex{4} , '-' , hex{4} , '-' , hex{12} ;
hex = digit | 'a'..'f' | 'A'..'F' ;
letter = 'a'..'z' | 'A'..'Z' ;
digit = '0'..'9' ;
whitespace = ' ' | '\t' | '\n' | '\r' ;
newline = '\n' | '\r\n' ;
```

---

## Part 2: AST Types

File: `src/dsl_v2/ast.rs`

```rust
//! AST type definitions for DSL v2
//!
//! These types represent the parsed structure of a DSL program.
//! They are intentionally simple and contain no execution logic.

use rust_decimal::Decimal;
use uuid::Uuid;
use std::collections::HashMap;

/// A complete DSL program
#[derive(Debug, Clone, PartialEq)]
pub struct Program {
    pub statements: Vec<Statement>,
}

/// A single statement in the DSL
#[derive(Debug, Clone, PartialEq)]
pub enum Statement {
    VerbCall(VerbCall),
    Comment(String),
}

/// A verb call: (domain.verb :key value ...)
#[derive(Debug, Clone, PartialEq)]
pub struct VerbCall {
    pub domain: String,
    pub verb: String,
    pub arguments: Vec<Argument>,
    /// Source location for error reporting
    pub span: Span,
}

/// A keyword-value argument
#[derive(Debug, Clone, PartialEq)]
pub struct Argument {
    pub key: Key,
    pub value: Value,
}

/// Keyword types
#[derive(Debug, Clone, PartialEq, Eq, Hash)]
pub enum Key {
    /// Simple keyword: :name
    Simple(String),
    /// Dotted keyword: :address.city
    Dotted(Vec<String>),
}

impl Key {
    /// Get the canonical string form (for lookups)
    pub fn canonical(&self) -> String {
        match self {
            Key::Simple(s) => s.clone(),
            Key::Dotted(parts) => parts.join("."),
        }
    }
    
    /// Check if this key matches a simple name (handles aliases via canonical form)
    pub fn matches(&self, name: &str) -> bool {
        match self {
            Key::Simple(s) => s == name,
            Key::Dotted(parts) => parts.len() == 1 && parts[0] == name,
        }
    }
}

/// Value types in the DSL
#[derive(Debug, Clone, PartialEq)]
pub enum Value {
    /// String: "hello"
    String(String),
    
    /// Integer: 42, -17
    Integer(i64),
    
    /// Decimal: 3.14, -0.5
    Decimal(Decimal),
    
    /// Boolean: true, false
    Boolean(bool),
    
    /// Null: nil
    Null,
    
    /// Reference: @cbu, @entity
    /// Late-bound identifier resolved at execution time
    Reference(String),
    
    /// Attribute reference: @attr{uuid}
    AttributeRef(Uuid),
    
    /// Document reference: @doc{uuid}
    DocumentRef(Uuid),
    
    /// List: [1, 2, 3] or ["a" "b" "c"]
    List(Vec<Value>),
    
    /// Map: {:key value :key2 value2}
    Map(HashMap<String, Value>),
}

impl Value {
    /// Try to extract as string
    pub fn as_string(&self) -> Option<&str> {
        match self {
            Value::String(s) => Some(s),
            _ => None,
        }
    }
    
    /// Try to extract as UUID (from string, reference result, or typed ref)
    pub fn as_uuid(&self) -> Option<Uuid> {
        match self {
            Value::String(s) => Uuid::parse_str(s).ok(),
            Value::AttributeRef(u) | Value::DocumentRef(u) => Some(*u),
            _ => None,
        }
    }
    
    /// Try to extract as integer
    pub fn as_integer(&self) -> Option<i64> {
        match self {
            Value::Integer(i) => Some(*i),
            _ => None,
        }
    }
    
    /// Try to extract as decimal (integers promoted)
    pub fn as_decimal(&self) -> Option<Decimal> {
        match self {
            Value::Decimal(d) => Some(*d),
            Value::Integer(i) => Some(Decimal::from(*i)),
            _ => None,
        }
    }
    
    /// Try to extract as boolean
    pub fn as_boolean(&self) -> Option<bool> {
        match self {
            Value::Boolean(b) => Some(*b),
            _ => None,
        }
    }
    
    /// Check if this is a reference
    pub fn is_reference(&self) -> bool {
        matches!(self, Value::Reference(_))
    }
    
    /// Get reference name if this is a reference
    pub fn as_reference(&self) -> Option<&str> {
        match self {
            Value::Reference(name) => Some(name),
            _ => None,
        }
    }
}

/// Source span for error reporting
#[derive(Debug, Clone, Copy, PartialEq, Eq, Default)]
pub struct Span {
    pub start: usize,
    pub end: usize,
}

impl Span {
    pub fn new(start: usize, end: usize) -> Self {
        Self { start, end }
    }
    
    /// Create a span covering two spans
    pub fn merge(a: Span, b: Span) -> Span {
        Span {
            start: a.start.min(b.start),
            end: a.end.max(b.end),
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_key_canonical() {
        assert_eq!(Key::Simple("name".into()).canonical(), "name");
        assert_eq!(
            Key::Dotted(vec!["address".into(), "city".into()]).canonical(),
            "address.city"
        );
    }

    #[test]
    fn test_value_conversions() {
        assert_eq!(Value::Integer(42).as_decimal(), Some(Decimal::from(42)));
        assert_eq!(Value::String("hello".into()).as_string(), Some("hello"));
        assert!(Value::Reference("cbu".into()).is_reference());
    }
}
```

---

## Part 3: Nom Parser

File: `src/dsl_v2/parser.rs`

```rust
//! Nom-based parser for DSL v2
//!
//! Parses the unified S-expression grammar into AST types.

use nom::{
    IResult,
    branch::alt,
    bytes::complete::{tag, take_while, take_while1, escaped_transform},
    character::complete::{char, multispace0, multispace1, none_of, digit1, alpha1, alphanumeric1},
    combinator::{map, opt, recognize, value, all_consuming, cut, peek},
    multi::{many0, many1, separated_list0},
    sequence::{delimited, pair, preceded, tuple, terminated},
    error::{context, VerboseError, convert_error},
};
use rust_decimal::Decimal;
use uuid::Uuid;
use std::collections::HashMap;
use std::str::FromStr;

use super::ast::*;

type ParseResult<'a, T> = IResult<&'a str, T, VerboseError<&'a str>>;

// ============================================================================
// Public API
// ============================================================================

/// Parse a complete DSL program from source text
/// 
/// Returns a structured Program AST or a human-readable error message.
/// 
/// # Example
/// ```
/// let program = parse_program(r#"
///     (cbu.create :name "Test Fund" :jurisdiction "LU")
///     (cbu.link :cbu-id @cbu :entity-id @manager :role "InvestmentManager")
/// "#)?;
/// ```
pub fn parse_program(input: &str) -> Result<Program, String> {
    match all_consuming(program)(input) {
        Ok((_, prog)) => Ok(prog),
        Err(nom::Err::Error(e)) | Err(nom::Err::Failure(e)) => {
            Err(convert_error(input, e))
        }
        Err(nom::Err::Incomplete(_)) => {
            Err("Incomplete input".to_string())
        }
    }
}

/// Parse a single verb call (for REPL/interactive use)
pub fn parse_single_verb(input: &str) -> Result<VerbCall, String> {
    let input = input.trim();
    match all_consuming(delimited(multispace0, verb_call, multispace0))(input) {
        Ok((_, vc)) => Ok(vc),
        Err(nom::Err::Error(e)) | Err(nom::Err::Failure(e)) => {
            Err(convert_error(input, e))
        }
        Err(nom::Err::Incomplete(_)) => {
            Err("Incomplete input".to_string())
        }
    }
}

// ============================================================================
// Internal Parsers
// ============================================================================

fn program(input: &str) -> ParseResult<Program> {
    let (input, _) = multispace0(input)?;
    let (input, statements) = many0(statement)(input)?;
    let (input, _) = multispace0(input)?;
    Ok((input, Program { statements }))
}

fn statement(input: &str) -> ParseResult<Statement> {
    let (input, _) = multispace0(input)?;
    alt((
        map(comment, Statement::Comment),
        map(verb_call, Statement::VerbCall),
    ))(input)
}

// ============================================================================
// Comments
// ============================================================================

fn comment(input: &str) -> ParseResult<String> {
    let (input, _) = tag(";;")(input)?;
    let (input, text) = take_while(|c| c != '\n')(input)?;
    let (input, _) = opt(char('\n'))(input)?;
    Ok((input, text.trim().to_string()))
}

// ============================================================================
// Verb Calls
// ============================================================================

fn verb_call(input: &str) -> ParseResult<VerbCall> {
    let start_pos = input.len();
    
    let (input, _) = char('(')(input)?;
    let (input, _) = multispace0(input)?;
    let (input, (domain, verb)) = context("word (domain.verb)", word)(input)?;
    let (input, arguments) = many0(argument)(input)?;
    let (input, _) = multispace0(input)?;
    let (input, _) = cut(context("closing parenthesis", char(')')))(input)?;
    
    let end_pos = input.len();
    
    Ok((input, VerbCall {
        domain,
        verb,
        arguments,
        span: Span::new(start_pos, end_pos),
    }))
}

fn word(input: &str) -> ParseResult<(String, String)> {
    let (input, domain) = identifier(input)?;
    let (input, _) = char('.')(input)?;
    let (input, verb) = kebab_identifier(input)?;
    Ok((input, (domain.to_string(), verb)))
}

// ============================================================================
// Arguments
// ============================================================================

fn argument(input: &str) -> ParseResult<Argument> {
    let (input, _) = multispace0(input)?;
    let (input, key) = keyword(input)?;
    let (input, _) = multispace1(input)?;
    let (input, val) = context("value", value_parser)(input)?;
    Ok((input, Argument { key, value: val }))
}

fn keyword(input: &str) -> ParseResult<Key> {
    let (input, _) = char(':')(input)?;
    
    // Try dotted identifier first (must have at least one dot)
    if let Ok((remaining, parts)) = dotted_identifier(input) {
        return Ok((remaining, Key::Dotted(parts)));
    }
    
    // Fall back to simple kebab identifier
    let (input, name) = kebab_identifier(input)?;
    Ok((input, Key::Simple(name)))
}

fn dotted_identifier(input: &str) -> ParseResult<Vec<String>> {
    let (input, first) = simple_identifier(input)?;
    let (input, rest) = many1(preceded(char('.'), simple_identifier))(input)?;
    
    let mut parts = vec![first.to_string()];
    parts.extend(rest.into_iter().map(|s| s.to_string()));
    Ok((input, parts))
}

fn kebab_identifier(input: &str) -> ParseResult<String> {
    recognize(pair(
        alt((alpha1, tag("_"))),
        many0(alt((alphanumeric1, tag("_"), tag("-")))),
    ))(input)
    .map(|(rest, matched)| (rest, matched.to_string()))
}

fn simple_identifier(input: &str) -> ParseResult<&str> {
    recognize(pair(
        alt((alpha1, tag("_"))),
        many0(alt((alphanumeric1, tag("_")))),
    ))(input)
}

fn identifier(input: &str) -> ParseResult<&str> {
    recognize(pair(
        alt((alpha1, tag("_"))),
        many0(alt((alphanumeric1, tag("_")))),
    ))(input)
}

// ============================================================================
// Values
// ============================================================================

fn value_parser(input: &str) -> ParseResult<Value> {
    alt((
        // Order matters: try specific patterns before generic ones
        map(boolean_literal, Value::Boolean),
        map(null_literal, |_| Value::Null),
        map(attribute_ref, Value::AttributeRef),
        map(document_ref, Value::DocumentRef),
        map(reference, Value::Reference),
        map(string_literal, Value::String),
        number_literal,  // Returns Value directly (Integer or Decimal)
        map(list_literal, Value::List),
        map(map_literal, Value::Map),
    ))(input)
}

// String literals with escape sequences
fn string_literal(input: &str) -> ParseResult<String> {
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
fn number_literal(input: &str) -> ParseResult<Value> {
    let (input, num_str) = recognize(tuple((
        opt(char('-')),
        digit1,
        opt(pair(char('.'), digit1)),
    )))(input)?;
    
    if num_str.contains('.') {
        let d = Decimal::from_str(num_str)
            .map_err(|_| nom::Err::Error(VerboseError::from_error_kind(
                input, nom::error::ErrorKind::Float
            )))?;
        Ok((input, Value::Decimal(d)))
    } else {
        let i = num_str.parse::<i64>()
            .map_err(|_| nom::Err::Error(VerboseError::from_error_kind(
                input, nom::error::ErrorKind::Digit
            )))?;
        Ok((input, Value::Integer(i)))
    }
}

// Boolean literals
fn boolean_literal(input: &str) -> ParseResult<bool> {
    alt((
        value(true, tag("true")),
        value(false, tag("false")),
    ))(input)
}

// Null literal
fn null_literal(input: &str) -> ParseResult<()> {
    value((), tag("nil"))(input)
}

// Reference: @identifier
fn reference(input: &str) -> ParseResult<String> {
    preceded(
        char('@'),
        // Don't match if followed by "attr{" or "doc{" (those are typed refs)
        |input: &str| {
            if input.starts_with("attr{") || input.starts_with("doc{") {
                Err(nom::Err::Error(VerboseError::from_error_kind(
                    input, nom::error::ErrorKind::Verify
                )))
            } else {
                map(identifier, |s: &str| s.to_string())(input)
            }
        }
    )(input)
}

// Attribute reference: @attr{uuid}
fn attribute_ref(input: &str) -> ParseResult<Uuid> {
    delimited(
        tag("@attr{"),
        uuid_parser,
        char('}'),
    )(input)
}

// Document reference: @doc{uuid}
fn document_ref(input: &str) -> ParseResult<Uuid> {
    delimited(
        tag("@doc{"),
        uuid_parser,
        char('}'),
    )(input)
}

// UUID parser
fn uuid_parser(input: &str) -> ParseResult<Uuid> {
    let (input, uuid_str) = recognize(tuple((
        take_hex(8),
        char('-'),
        take_hex(4),
        char('-'),
        take_hex(4),
        char('-'),
        take_hex(4),
        char('-'),
        take_hex(12),
    )))(input)?;
    
    let uuid = Uuid::parse_str(uuid_str)
        .map_err(|_| nom::Err::Error(VerboseError::from_error_kind(
            input, nom::error::ErrorKind::Verify
        )))?;
    
    Ok((input, uuid))
}

fn take_hex(count: usize) -> impl Fn(&str) -> ParseResult<&str> {
    move |input: &str| {
        let mut chars = input.chars();
        let mut end = 0;
        
        for _ in 0..count {
            match chars.next() {
                Some(c) if c.is_ascii_hexdigit() => end += c.len_utf8(),
                _ => return Err(nom::Err::Error(VerboseError::from_error_kind(
                    input, nom::error::ErrorKind::HexDigit
                ))),
            }
        }
        
        Ok((&input[end..], &input[..end]))
    }
}

// List literal: [value, value, ...] or [value value ...]
fn list_literal(input: &str) -> ParseResult<Vec<Value>> {
    delimited(
        char('['),
        delimited(
            multispace0,
            separated_list0(
                alt((
                    // Comma separator (with optional whitespace)
                    delimited(multispace0, char(','), multispace0),
                    // Or just whitespace
                    multispace1,
                )),
                value_parser,
            ),
            multispace0,
        ),
        char(']'),
    )(input)
}

// Map literal: {:key value :key2 value2}
fn map_literal(input: &str) -> ParseResult<HashMap<String, Value>> {
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

fn map_key(input: &str) -> ParseResult<String> {
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
        let input = r#"(attr.set :attr-id @attr{550e8400-e29b-41d4-a716-446655440000} :value "test")"#;
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
            assert!(matches!(&vc.arguments[0].key, Key::Dotted(parts) if parts == &["address", "city"]));
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
```

---

## Part 4: Verb Definitions (Tier 1)

File: `src/dsl_v2/verbs.rs`

```rust
//! Standard verb definitions (Tier 1)
//!
//! This module contains the static definitions for all data-driven verbs.
//! These verbs are executed by the generic CRUD functions in the executor.
//!
//! IMPORTANT: If a verb cannot be expressed using the Behavior enum,
//! it belongs in custom_ops/, not here.

use crate::dsl_v2::executor::ReturnType;

/// Behavior patterns for data-driven execution
/// 
/// Each variant maps to a generic executor function.
/// If your operation doesn't fit these patterns, use CustomOperation instead.
#[derive(Debug, Clone)]
pub enum Behavior {
    /// INSERT into a single table
    /// Generates: INSERT INTO table (cols...) VALUES (vals...) RETURNING pk
    Insert { 
        table: &'static str,
    },
    
    /// SELECT from a single table
    /// Generates: SELECT * FROM table WHERE conditions...
    Select { 
        table: &'static str,
    },
    
    /// UPDATE a single table
    /// Generates: UPDATE table SET cols... WHERE pk = $1
    Update { 
        table: &'static str,
    },
    
    /// DELETE from a single table
    /// Generates: DELETE FROM table WHERE pk = $1
    Delete { 
        table: &'static str,
    },
    
    /// INSERT with ON CONFLICT DO UPDATE (upsert)
    /// Generates: INSERT ... ON CONFLICT (keys) DO UPDATE SET ...
    Upsert { 
        table: &'static str, 
        conflict_keys: &'static [&'static str],
    },
    
    /// INSERT into junction table (taxonomy link)
    /// For linking entities with roles: CBU ↔ Entity, Document ↔ Entity, etc.
    Link { 
        junction: &'static str, 
        from_col: &'static str, 
        to_col: &'static str, 
        role_col: Option<&'static str>,
    },
    
    /// DELETE from junction table (taxonomy unlink)
    Unlink { 
        junction: &'static str, 
        from_col: &'static str, 
        to_col: &'static str,
    },
    
    /// SELECT filtered by foreign key
    /// Generates: SELECT * FROM table WHERE fk_col = $1
    ListByFk { 
        table: &'static str, 
        fk_col: &'static str,
    },
    
    /// SELECT with JOIN for related data
    SelectWithJoin {
        primary_table: &'static str,
        join_table: &'static str,
        join_col: &'static str,
    },
}

/// Complete verb definition
#[derive(Debug, Clone)]
pub struct VerbDef {
    /// Domain name (e.g., "entity", "cbu", "document")
    pub domain: &'static str,
    
    /// Verb name (e.g., "create-limited-company", "link")
    pub verb: &'static str,
    
    /// Execution behavior pattern
    pub behavior: Behavior,
    
    /// Required arguments (error if missing)
    pub required_args: &'static [&'static str],
    
    /// Optional arguments
    pub optional_args: &'static [&'static str],
    
    /// Return type specification
    pub returns: ReturnType,
    
    /// Human-readable description (for docs/help)
    pub description: &'static str,
}

/// All standard verbs - THE source of truth for Tier 1
/// 
/// To add a new verb:
/// 1. First, try to express it using existing Behavior variants
/// 2. If you need a new Behavior variant, add it and implement in executor
/// 3. If it truly can't be data-driven, add to custom_ops/ instead
pub static STANDARD_VERBS: &[VerbDef] = &[
    // =========================================================================
    // ENTITY DOMAIN - Singleton CRUD for different entity types
    // =========================================================================
    
    VerbDef {
        domain: "entity",
        verb: "create-limited-company",
        behavior: Behavior::Insert { table: "limited_companies" },
        required_args: &["name"],
        optional_args: &["jurisdiction", "company-number", "incorporation-date", "registered-address", "business-nature"],
        returns: ReturnType::Uuid { name: "entity_id", capture: true },
        description: "Create a new limited company entity",
    },
    VerbDef {
        domain: "entity",
        verb: "create-proper-person",
        behavior: Behavior::Insert { table: "proper_persons" },
        required_args: &["first-name", "last-name"],
        optional_args: &["middle-names", "date-of-birth", "nationality", "tax-id", "residence-address"],
        returns: ReturnType::Uuid { name: "entity_id", capture: true },
        description: "Create a new natural person entity",
    },
    VerbDef {
        domain: "entity",
        verb: "create-partnership",
        behavior: Behavior::Insert { table: "partnerships" },
        required_args: &["name"],
        optional_args: &["jurisdiction", "partnership-type", "formation-date", "principal-place-business"],
        returns: ReturnType::Uuid { name: "entity_id", capture: true },
        description: "Create a new partnership entity (LP, LLP, GP)",
    },
    VerbDef {
        domain: "entity",
        verb: "create-trust",
        behavior: Behavior::Insert { table: "trusts" },
        required_args: &["name", "jurisdiction"],
        optional_args: &["trust-type", "establishment-date", "governing-law", "trust-purpose"],
        returns: ReturnType::Uuid { name: "entity_id", capture: true },
        description: "Create a new trust entity",
    },
    VerbDef {
        domain: "entity",
        verb: "read",
        behavior: Behavior::Select { table: "entities" },
        required_args: &["entity-id"],
        optional_args: &[],
        returns: ReturnType::Record,
        description: "Read an entity by ID",
    },
    VerbDef {
        domain: "entity",
        verb: "update",
        behavior: Behavior::Update { table: "entities" },
        required_args: &["entity-id"],
        optional_args: &["name", "status", "jurisdiction"],
        returns: ReturnType::Affected,
        description: "Update an entity's base fields",
    },
    VerbDef {
        domain: "entity",
        verb: "delete",
        behavior: Behavior::Delete { table: "entities" },
        required_args: &["entity-id"],
        optional_args: &[],
        returns: ReturnType::Affected,
        description: "Delete an entity (cascades to type extension)",
    },
    VerbDef {
        domain: "entity",
        verb: "list",
        behavior: Behavior::Select { table: "entities" },
        required_args: &[],
        optional_args: &["entity-type", "jurisdiction", "status", "limit", "offset"],
        returns: ReturnType::RecordSet,
        description: "List entities with optional filters",
    },
    
    // Upsert variants for idempotent operations
    VerbDef {
        domain: "entity",
        verb: "ensure-limited-company",
        behavior: Behavior::Upsert { 
            table: "limited_companies", 
            conflict_keys: &["company-number", "jurisdiction"],
        },
        required_args: &["name"],
        optional_args: &["jurisdiction", "company-number", "incorporation-date"],
        returns: ReturnType::Uuid { name: "entity_id", capture: true },
        description: "Create or update a limited company by natural key (company-number + jurisdiction)",
    },
    VerbDef {
        domain: "entity",
        verb: "ensure-proper-person",
        behavior: Behavior::Upsert { 
            table: "proper_persons", 
            conflict_keys: &["tax-id"],
        },
        required_args: &["first-name", "last-name"],
        optional_args: &["nationality", "tax-id", "date-of-birth"],
        returns: ReturnType::Uuid { name: "entity_id", capture: true },
        description: "Create or update a proper person by tax ID",
    },
    
    // =========================================================================
    // CBU DOMAIN - Singleton + Taxonomy operations
    // =========================================================================
    
    VerbDef {
        domain: "cbu",
        verb: "create",
        behavior: Behavior::Insert { table: "cbus" },
        required_args: &["name"],
        optional_args: &["jurisdiction", "client-type", "nature-purpose", "description"],
        returns: ReturnType::Uuid { name: "cbu_id", capture: true },
        description: "Create a new Client Business Unit",
    },
    VerbDef {
        domain: "cbu",
        verb: "read",
        behavior: Behavior::Select { table: "cbus" },
        required_args: &["cbu-id"],
        optional_args: &[],
        returns: ReturnType::Record,
        description: "Read a CBU by ID",
    },
    VerbDef {
        domain: "cbu",
        verb: "update",
        behavior: Behavior::Update { table: "cbus" },
        required_args: &["cbu-id"],
        optional_args: &["name", "status", "client-type", "jurisdiction"],
        returns: ReturnType::Affected,
        description: "Update a CBU",
    },
    VerbDef {
        domain: "cbu",
        verb: "delete",
        behavior: Behavior::Delete { table: "cbus" },
        required_args: &["cbu-id"],
        optional_args: &[],
        returns: ReturnType::Affected,
        description: "Delete a CBU",
    },
    VerbDef {
        domain: "cbu",
        verb: "list",
        behavior: Behavior::Select { table: "cbus" },
        required_args: &[],
        optional_args: &["status", "client-type", "jurisdiction", "limit", "offset"],
        returns: ReturnType::RecordSet,
        description: "List CBUs with optional filters",
    },
    VerbDef {
        domain: "cbu",
        verb: "ensure",
        behavior: Behavior::Upsert { 
            table: "cbus", 
            conflict_keys: &["name", "jurisdiction"],
        },
        required_args: &["name"],
        optional_args: &["jurisdiction", "client-type", "nature-purpose"],
        returns: ReturnType::Uuid { name: "cbu_id", capture: true },
        description: "Create or update a CBU by natural key (name + jurisdiction)",
    },
    
    // Taxonomy: Link entities to CBU with roles
    VerbDef {
        domain: "cbu",
        verb: "link",
        behavior: Behavior::Link {
            junction: "cbu_entity_roles",
            from_col: "cbu_id",
            to_col: "entity_id",
            role_col: Some("role"),
        },
        required_args: &["cbu-id", "entity-id", "role"],
        optional_args: &["ownership-percent", "effective-date", "notes"],
        returns: ReturnType::Uuid { name: "cbu_entity_role_id", capture: false },
        description: "Link an entity to a CBU with a role",
    },
    VerbDef {
        domain: "cbu",
        verb: "unlink",
        behavior: Behavior::Unlink {
            junction: "cbu_entity_roles",
            from_col: "cbu_id",
            to_col: "entity_id",
        },
        required_args: &["cbu-id", "entity-id"],
        optional_args: &["role"],
        returns: ReturnType::Affected,
        description: "Unlink an entity from a CBU (optionally by specific role)",
    },
    VerbDef {
        domain: "cbu",
        verb: "entities",
        behavior: Behavior::ListByFk { 
            table: "cbu_entity_roles", 
            fk_col: "cbu_id",
        },
        required_args: &["cbu-id"],
        optional_args: &["role"],
        returns: ReturnType::RecordSet,
        description: "List all entities linked to a CBU",
    },
    
    // =========================================================================
    // DOCUMENT DOMAIN - Note: catalog and extract are CUSTOM
    // =========================================================================
    
    VerbDef {
        domain: "document",
        verb: "read",
        behavior: Behavior::Select { table: "document_catalog" },
        required_args: &["document-id"],
        optional_args: &[],
        returns: ReturnType::Record,
        description: "Read a document by ID",
    },
    VerbDef {
        domain: "document",
        verb: "update",
        behavior: Behavior::Update { table: "document_catalog" },
        required_args: &["document-id"],
        optional_args: &["title", "status", "verification-status"],
        returns: ReturnType::Affected,
        description: "Update document metadata",
    },
    VerbDef {
        domain: "document",
        verb: "delete",
        behavior: Behavior::Delete { table: "document_catalog" },
        required_args: &["document-id"],
        optional_args: &[],
        returns: ReturnType::Affected,
        description: "Delete a document",
    },
    VerbDef {
        domain: "document",
        verb: "link-cbu",
        behavior: Behavior::Link {
            junction: "document_cbu_links",
            from_col: "document_id",
            to_col: "cbu_id",
            role_col: Some("relationship_type"),
        },
        required_args: &["document-id", "cbu-id"],
        optional_args: &["relationship-type"],
        returns: ReturnType::Uuid { name: "link_id", capture: false },
        description: "Link a document to a CBU",
    },
    VerbDef {
        domain: "document",
        verb: "link-entity",
        behavior: Behavior::Link {
            junction: "document_entity_links",
            from_col: "document_id",
            to_col: "entity_id",
            role_col: Some("relationship_type"),
        },
        required_args: &["document-id", "entity-id"],
        optional_args: &["relationship-type"],
        returns: ReturnType::Uuid { name: "link_id", capture: false },
        description: "Link a document to an entity",
    },
    VerbDef {
        domain: "document",
        verb: "unlink-cbu",
        behavior: Behavior::Unlink {
            junction: "document_cbu_links",
            from_col: "document_id",
            to_col: "cbu_id",
        },
        required_args: &["document-id", "cbu-id"],
        optional_args: &[],
        returns: ReturnType::Affected,
        description: "Unlink a document from a CBU",
    },
    VerbDef {
        domain: "document",
        verb: "unlink-entity",
        behavior: Behavior::Unlink {
            junction: "document_entity_links",
            from_col: "document_id",
            to_col: "entity_id",
        },
        required_args: &["document-id", "entity-id"],
        optional_args: &[],
        returns: ReturnType::Affected,
        description: "Unlink a document from an entity",
    },
    VerbDef {
        domain: "document",
        verb: "list-by-cbu",
        behavior: Behavior::ListByFk { 
            table: "document_catalog", 
            fk_col: "cbu_id",
        },
        required_args: &["cbu-id"],
        optional_args: &["doc-type", "status"],
        returns: ReturnType::RecordSet,
        description: "List documents for a CBU",
    },
    VerbDef {
        domain: "document",
        verb: "list-by-entity",
        behavior: Behavior::SelectWithJoin {
            primary_table: "document_catalog",
            join_table: "document_entity_links",
            join_col: "document_id",
        },
        required_args: &["entity-id"],
        optional_args: &["doc-type", "relationship-type"],
        returns: ReturnType::RecordSet,
        description: "List documents linked to an entity",
    },
    
    // =========================================================================
    // PRODUCT DOMAIN
    // =========================================================================
    
    VerbDef {
        domain: "product",
        verb: "create",
        behavior: Behavior::Insert { table: "products" },
        required_args: &["name", "product-code"],
        optional_args: &["description", "product-category", "regulatory-framework"],
        returns: ReturnType::Uuid { name: "product_id", capture: true },
        description: "Create a new product",
    },
    VerbDef {
        domain: "product",
        verb: "read",
        behavior: Behavior::Select { table: "products" },
        required_args: &[],
        optional_args: &["product-id", "product-code"],
        returns: ReturnType::Record,
        description: "Read a product by ID or code",
    },
    VerbDef {
        domain: "product",
        verb: "update",
        behavior: Behavior::Update { table: "products" },
        required_args: &["product-id"],
        optional_args: &["name", "description", "is-active"],
        returns: ReturnType::Affected,
        description: "Update a product",
    },
    VerbDef {
        domain: "product",
        verb: "delete",
        behavior: Behavior::Delete { table: "products" },
        required_args: &["product-id"],
        optional_args: &[],
        returns: ReturnType::Affected,
        description: "Delete a product",
    },
    VerbDef {
        domain: "product",
        verb: "list",
        behavior: Behavior::Select { table: "products" },
        required_args: &[],
        optional_args: &["product-category", "is-active", "limit", "offset"],
        returns: ReturnType::RecordSet,
        description: "List products with optional filters",
    },
    
    // =========================================================================
    // SERVICE DOMAIN
    // =========================================================================
    
    VerbDef {
        domain: "service",
        verb: "create",
        behavior: Behavior::Insert { table: "services" },
        required_args: &["name", "service-code"],
        optional_args: &["description", "service-category"],
        returns: ReturnType::Uuid { name: "service_id", capture: true },
        description: "Create a new service",
    },
    VerbDef {
        domain: "service",
        verb: "read",
        behavior: Behavior::Select { table: "services" },
        required_args: &[],
        optional_args: &["service-id", "service-code"],
        returns: ReturnType::Record,
        description: "Read a service by ID or code",
    },
    VerbDef {
        domain: "service",
        verb: "link-product",
        behavior: Behavior::Link {
            junction: "product_services",
            from_col: "service_id",
            to_col: "product_id",
            role_col: None,
        },
        required_args: &["service-id", "product-id"],
        optional_args: &["is-required"],
        returns: ReturnType::Uuid { name: "link_id", capture: false },
        description: "Link a service to a product",
    },
    VerbDef {
        domain: "service",
        verb: "unlink-product",
        behavior: Behavior::Unlink {
            junction: "product_services",
            from_col: "service_id",
            to_col: "product_id",
        },
        required_args: &["service-id", "product-id"],
        optional_args: &[],
        returns: ReturnType::Affected,
        description: "Unlink a service from a product",
    },
    
    // =========================================================================
    // INVESTIGATION DOMAIN
    // =========================================================================
    
    VerbDef {
        domain: "investigation",
        verb: "create",
        behavior: Behavior::Insert { table: "investigations" },
        required_args: &["cbu-id", "investigation-type"],
        optional_args: &["risk-rating", "ubo-threshold", "deadline", "assigned-to"],
        returns: ReturnType::Uuid { name: "investigation_id", capture: true },
        description: "Create a KYC investigation",
    },
    VerbDef {
        domain: "investigation",
        verb: "read",
        behavior: Behavior::Select { table: "investigations" },
        required_args: &["investigation-id"],
        optional_args: &[],
        returns: ReturnType::Record,
        description: "Read an investigation by ID",
    },
    VerbDef {
        domain: "investigation",
        verb: "update-status",
        behavior: Behavior::Update { table: "investigations" },
        required_args: &["investigation-id", "status"],
        optional_args: &["notes"],
        returns: ReturnType::Affected,
        description: "Update investigation status",
    },
    VerbDef {
        domain: "investigation",
        verb: "complete",
        behavior: Behavior::Update { table: "investigations" },
        required_args: &["investigation-id", "outcome"],
        optional_args: &["notes", "completed-by"],
        returns: ReturnType::Affected,
        description: "Complete an investigation with outcome",
    },
    VerbDef {
        domain: "investigation",
        verb: "list-by-cbu",
        behavior: Behavior::ListByFk {
            table: "investigations",
            fk_col: "cbu_id",
        },
        required_args: &["cbu-id"],
        optional_args: &["status", "investigation-type"],
        returns: ReturnType::RecordSet,
        description: "List investigations for a CBU",
    },
    
    // =========================================================================
    // SCREENING DOMAIN - Note: pep and sanctions are CUSTOM (external APIs)
    // =========================================================================
    
    VerbDef {
        domain: "screening",
        verb: "record-result",
        behavior: Behavior::Insert { table: "screening_results" },
        required_args: &["screening-id", "result"],
        optional_args: &["match-details", "reviewed-by"],
        returns: ReturnType::Uuid { name: "result_id", capture: false },
        description: "Record a screening result",
    },
    VerbDef {
        domain: "screening",
        verb: "resolve",
        behavior: Behavior::Update { table: "screening_results" },
        required_args: &["screening-id", "resolution"],
        optional_args: &["rationale", "resolved-by"],
        returns: ReturnType::Affected,
        description: "Resolve a screening match",
    },
    
    // =========================================================================
    // RISK DOMAIN
    // =========================================================================
    
    VerbDef {
        domain: "risk",
        verb: "set-rating",
        behavior: Behavior::Upsert { 
            table: "risk_ratings", 
            conflict_keys: &["cbu-id"],
        },
        required_args: &["rating"],
        optional_args: &["cbu-id", "entity-id", "factors", "rationale", "assessed-by"],
        returns: ReturnType::Uuid { name: "rating_id", capture: false },
        description: "Set risk rating for CBU or entity (upserts by cbu-id)",
    },
    VerbDef {
        domain: "risk",
        verb: "add-flag",
        behavior: Behavior::Insert { table: "risk_flags" },
        required_args: &["flag-type", "description"],
        optional_args: &["cbu-id", "entity-id", "flagged-by", "severity"],
        returns: ReturnType::Uuid { name: "flag_id", capture: false },
        description: "Add a risk flag",
    },
    VerbDef {
        domain: "risk",
        verb: "remove-flag",
        behavior: Behavior::Delete { table: "risk_flags" },
        required_args: &["flag-id"],
        optional_args: &[],
        returns: ReturnType::Affected,
        description: "Remove a risk flag",
    },
    VerbDef {
        domain: "risk",
        verb: "list-flags",
        behavior: Behavior::ListByFk {
            table: "risk_flags",
            fk_col: "cbu_id",
        },
        required_args: &["cbu-id"],
        optional_args: &["flag-type", "severity"],
        returns: ReturnType::RecordSet,
        description: "List risk flags for a CBU",
    },
    
    // =========================================================================
    // DECISION DOMAIN
    // =========================================================================
    
    VerbDef {
        domain: "decision",
        verb: "record",
        behavior: Behavior::Insert { table: "decisions" },
        required_args: &["cbu-id", "decision"],
        optional_args: &["investigation-id", "decision-authority", "rationale", "decided-by"],
        returns: ReturnType::Uuid { name: "decision_id", capture: true },
        description: "Record an onboarding decision",
    },
    VerbDef {
        domain: "decision",
        verb: "read",
        behavior: Behavior::Select { table: "decisions" },
        required_args: &["decision-id"],
        optional_args: &[],
        returns: ReturnType::Record,
        description: "Read a decision by ID",
    },
    VerbDef {
        domain: "decision",
        verb: "add-condition",
        behavior: Behavior::Insert { table: "decision_conditions" },
        required_args: &["decision-id", "condition-type"],
        optional_args: &["description", "frequency", "due-date", "threshold"],
        returns: ReturnType::Uuid { name: "condition_id", capture: false },
        description: "Add a condition to a decision",
    },
    VerbDef {
        domain: "decision",
        verb: "satisfy-condition",
        behavior: Behavior::Update { table: "decision_conditions" },
        required_args: &["condition-id"],
        optional_args: &["satisfied-by", "evidence", "satisfied-date"],
        returns: ReturnType::Affected,
        description: "Mark a condition as satisfied",
    },
    
    // =========================================================================
    // MONITORING DOMAIN
    // =========================================================================
    
    VerbDef {
        domain: "monitoring",
        verb: "setup",
        behavior: Behavior::Insert { table: "monitoring_configs" },
        required_args: &["cbu-id", "monitoring-level"],
        optional_args: &["components", "frequency"],
        returns: ReturnType::Uuid { name: "config_id", capture: false },
        description: "Setup ongoing monitoring for a CBU",
    },
    VerbDef {
        domain: "monitoring",
        verb: "record-event",
        behavior: Behavior::Insert { table: "monitoring_events" },
        required_args: &["cbu-id", "event-type"],
        optional_args: &["description", "severity", "requires-review"],
        returns: ReturnType::Uuid { name: "event_id", capture: false },
        description: "Record a monitoring event",
    },
    VerbDef {
        domain: "monitoring",
        verb: "schedule-review",
        behavior: Behavior::Insert { table: "scheduled_reviews" },
        required_args: &["cbu-id", "review-type", "due-date"],
        optional_args: &["assigned-to"],
        returns: ReturnType::Uuid { name: "review_id", capture: false },
        description: "Schedule a periodic review",
    },
    VerbDef {
        domain: "monitoring",
        verb: "complete-review",
        behavior: Behavior::Update { table: "scheduled_reviews" },
        required_args: &["review-id"],
        optional_args: &["completed-by", "notes", "next-review-date"],
        returns: ReturnType::Affected,
        description: "Complete a scheduled review",
    },
];

// ============================================================================
// Lookup Functions
// ============================================================================

/// Find a verb definition by domain and verb name
pub fn find_verb(domain: &str, verb: &str) -> Option<&'static VerbDef> {
    STANDARD_VERBS.iter().find(|v| v.domain == domain && v.verb == verb)
}

/// Get all verbs for a specific domain
pub fn verbs_for_domain(domain: &str) -> impl Iterator<Item = &'static VerbDef> {
    STANDARD_VERBS.iter().filter(move |v| v.domain == domain)
}

/// Get all unique domain names
pub fn domains() -> impl Iterator<Item = &'static str> {
    let mut seen = std::collections::HashSet::new();
    STANDARD_VERBS.iter().filter_map(move |v| {
        if seen.insert(v.domain) {
            Some(v.domain)
        } else {
            None
        }
    })
}

/// Count of standard verbs (for metrics)
pub fn verb_count() -> usize {
    STANDARD_VERBS.len()
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_find_verb() {
        let verb = find_verb("entity", "create-limited-company");
        assert!(verb.is_some());
        let v = verb.unwrap();
        assert_eq!(v.domain, "entity");
        assert_eq!(v.verb, "create-limited-company");
    }

    #[test]
    fn test_find_verb_not_found() {
        assert!(find_verb("nonexistent", "verb").is_none());
    }

    #[test]
    fn test_verbs_for_domain() {
        let entity_verbs: Vec<_> = verbs_for_domain("entity").collect();
        assert!(entity_verbs.len() > 5); // We have multiple entity verbs
        assert!(entity_verbs.iter().all(|v| v.domain == "entity"));
    }

    #[test]
    fn test_domains() {
        let all_domains: Vec<_> = domains().collect();
        assert!(all_domains.contains(&"entity"));
        assert!(all_domains.contains(&"cbu"));
        assert!(all_domains.contains(&"document"));
    }

    #[test]
    fn test_all_verbs_have_required_fields() {
        for verb in STANDARD_VERBS {
            assert!(!verb.domain.is_empty(), "Verb has empty domain");
            assert!(!verb.verb.is_empty(), "Verb has empty verb name");
            assert!(!verb.description.is_empty(), "Verb {} has empty description", verb.verb);
        }
    }

    #[test]
    fn test_no_duplicate_verbs() {
        let mut seen = std::collections::HashSet::new();
        for verb in STANDARD_VERBS {
            let key = (verb.domain, verb.verb);
            assert!(seen.insert(key), "Duplicate verb: {}.{}", verb.domain, verb.verb);
        }
    }
}
```

---

## Part 5: Column Mappings

File: `src/dsl_v2/mappings.rs`

```rust
//! Column mappings for DSL key → DB column translation
//!
//! This module provides the configuration for mapping DSL keyword arguments
//! to database column names. This decouples the DSL surface syntax from
//! the database schema.

use std::collections::HashMap;
use once_cell::sync::Lazy;

/// Database column type (for proper value binding)
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum DbType {
    Uuid,
    Text,
    Integer,
    Decimal,
    Boolean,
    Date,
    Timestamp,
    Jsonb,
}

/// Single column mapping entry
#[derive(Debug, Clone)]
pub struct ColumnMapping {
    /// DSL key (canonical form, e.g., "company-number")
    pub dsl_key: &'static str,
    /// Database column name (e.g., "registration_number")
    pub db_column: &'static str,
    /// Database type for proper binding
    pub db_type: DbType,
    /// Aliases (alternative DSL keys that map to same column)
    pub aliases: &'static [&'static str],
}

/// Table-level mapping configuration
#[derive(Debug, Clone)]
pub struct TableMappings {
    /// Table name (without schema)
    pub table: &'static str,
    /// Primary key column name
    pub pk_column: &'static str,
    /// Column mappings for this table
    pub columns: &'static [ColumnMapping],
}

// ============================================================================
// Entity Mappings
// ============================================================================

pub static ENTITIES_MAPPINGS: TableMappings = TableMappings {
    table: "entities",
    pk_column: "entity_id",
    columns: &[
        ColumnMapping { dsl_key: "entity-id", db_column: "entity_id", db_type: DbType::Uuid, aliases: &["id"] },
        ColumnMapping { dsl_key: "name", db_column: "name", db_type: DbType::Text, aliases: &["entity-name", "legal-name"] },
        ColumnMapping { dsl_key: "entity-type", db_column: "entity_type", db_type: DbType::Text, aliases: &["type"] },
        ColumnMapping { dsl_key: "jurisdiction", db_column: "jurisdiction", db_type: DbType::Text, aliases: &["country", "domicile"] },
        ColumnMapping { dsl_key: "status", db_column: "status", db_type: DbType::Text, aliases: &[] },
        ColumnMapping { dsl_key: "external-id", db_column: "external_id", db_type: DbType::Text, aliases: &[] },
    ],
};

pub static LIMITED_COMPANIES_MAPPINGS: TableMappings = TableMappings {
    table: "limited_companies",
    pk_column: "entity_id",
    columns: &[
        ColumnMapping { dsl_key: "entity-id", db_column: "entity_id", db_type: DbType::Uuid, aliases: &["id"] },
        ColumnMapping { dsl_key: "name", db_column: "name", db_type: DbType::Text, aliases: &["company-name", "legal-name"] },
        ColumnMapping { dsl_key: "jurisdiction", db_column: "jurisdiction", db_type: DbType::Text, aliases: &["country"] },
        ColumnMapping { dsl_key: "company-number", db_column: "registration_number", db_type: DbType::Text, aliases: &["registration-number", "reg-no"] },
        ColumnMapping { dsl_key: "incorporation-date", db_column: "incorporation_date", db_type: DbType::Date, aliases: &["formation-date"] },
        ColumnMapping { dsl_key: "registered-address", db_column: "registered_address", db_type: DbType::Text, aliases: &["registered-office"] },
        ColumnMapping { dsl_key: "business-nature", db_column: "business_nature", db_type: DbType::Text, aliases: &["nature-of-business"] },
    ],
};

pub static PROPER_PERSONS_MAPPINGS: TableMappings = TableMappings {
    table: "proper_persons",
    pk_column: "entity_id",
    columns: &[
        ColumnMapping { dsl_key: "entity-id", db_column: "entity_id", db_type: DbType::Uuid, aliases: &["id"] },
        ColumnMapping { dsl_key: "first-name", db_column: "first_name", db_type: DbType::Text, aliases: &["given-name"] },
        ColumnMapping { dsl_key: "last-name", db_column: "last_name", db_type: DbType::Text, aliases: &["family-name", "surname"] },
        ColumnMapping { dsl_key: "middle-names", db_column: "middle_names", db_type: DbType::Text, aliases: &["middle-name"] },
        ColumnMapping { dsl_key: "date-of-birth", db_column: "date_of_birth", db_type: DbType::Date, aliases: &["dob", "birth-date"] },
        ColumnMapping { dsl_key: "nationality", db_column: "nationality", db_type: DbType::Text, aliases: &[] },
        ColumnMapping { dsl_key: "tax-id", db_column: "tax_id", db_type: DbType::Text, aliases: &["tin", "ssn", "national-id"] },
        ColumnMapping { dsl_key: "residence-address", db_column: "residence_address", db_type: DbType::Text, aliases: &["address"] },
    ],
};

pub static PARTNERSHIPS_MAPPINGS: TableMappings = TableMappings {
    table: "partnerships",
    pk_column: "entity_id",
    columns: &[
        ColumnMapping { dsl_key: "entity-id", db_column: "entity_id", db_type: DbType::Uuid, aliases: &["id"] },
        ColumnMapping { dsl_key: "name", db_column: "name", db_type: DbType::Text, aliases: &["partnership-name"] },
        ColumnMapping { dsl_key: "jurisdiction", db_column: "jurisdiction", db_type: DbType::Text, aliases: &[] },
        ColumnMapping { dsl_key: "partnership-type", db_column: "partnership_type", db_type: DbType::Text, aliases: &["type"] },
        ColumnMapping { dsl_key: "formation-date", db_column: "formation_date", db_type: DbType::Date, aliases: &[] },
        ColumnMapping { dsl_key: "principal-place-business", db_column: "principal_place_of_business", db_type: DbType::Text, aliases: &[] },
    ],
};

pub static TRUSTS_MAPPINGS: TableMappings = TableMappings {
    table: "trusts",
    pk_column: "entity_id",
    columns: &[
        ColumnMapping { dsl_key: "entity-id", db_column: "entity_id", db_type: DbType::Uuid, aliases: &["id"] },
        ColumnMapping { dsl_key: "name", db_column: "name", db_type: DbType::Text, aliases: &["trust-name"] },
        ColumnMapping { dsl_key: "jurisdiction", db_column: "jurisdiction", db_type: DbType::Text, aliases: &[] },
        ColumnMapping { dsl_key: "trust-type", db_column: "trust_type", db_type: DbType::Text, aliases: &["type"] },
        ColumnMapping { dsl_key: "establishment-date", db_column: "establishment_date", db_type: DbType::Date, aliases: &["formation-date"] },
        ColumnMapping { dsl_key: "governing-law", db_column: "governing_law", db_type: DbType::Text, aliases: &[] },
        ColumnMapping { dsl_key: "trust-purpose", db_column: "trust_purpose", db_type: DbType::Text, aliases: &["purpose"] },
    ],
};

// ============================================================================
// CBU Mappings
// ============================================================================

pub static CBUS_MAPPINGS: TableMappings = TableMappings {
    table: "cbus",
    pk_column: "cbu_id",
    columns: &[
        ColumnMapping { dsl_key: "cbu-id", db_column: "cbu_id", db_type: DbType::Uuid, aliases: &["id"] },
        ColumnMapping { dsl_key: "name", db_column: "name", db_type: DbType::Text, aliases: &["cbu-name"] },
        ColumnMapping { dsl_key: "jurisdiction", db_column: "jurisdiction", db_type: DbType::Text, aliases: &[] },
        ColumnMapping { dsl_key: "client-type", db_column: "client_type", db_type: DbType::Text, aliases: &["type"] },
        ColumnMapping { dsl_key: "nature-purpose", db_column: "nature_purpose", db_type: DbType::Text, aliases: &["nature-and-purpose"] },
        ColumnMapping { dsl_key: "description", db_column: "description", db_type: DbType::Text, aliases: &[] },
        ColumnMapping { dsl_key: "status", db_column: "status", db_type: DbType::Text, aliases: &[] },
    ],
};

pub static CBU_ENTITY_ROLES_MAPPINGS: TableMappings = TableMappings {
    table: "cbu_entity_roles",
    pk_column: "cbu_entity_role_id",
    columns: &[
        ColumnMapping { dsl_key: "cbu-entity-role-id", db_column: "cbu_entity_role_id", db_type: DbType::Uuid, aliases: &["id"] },
        ColumnMapping { dsl_key: "cbu-id", db_column: "cbu_id", db_type: DbType::Uuid, aliases: &[] },
        ColumnMapping { dsl_key: "entity-id", db_column: "entity_id", db_type: DbType::Uuid, aliases: &[] },
        ColumnMapping { dsl_key: "role", db_column: "role", db_type: DbType::Text, aliases: &[] },
        ColumnMapping { dsl_key: "ownership-percent", db_column: "ownership_percent", db_type: DbType::Decimal, aliases: &["ownership-percentage", "ownership"] },
        ColumnMapping { dsl_key: "effective-date", db_column: "effective_date", db_type: DbType::Date, aliases: &["start-date"] },
        ColumnMapping { dsl_key: "notes", db_column: "notes", db_type: DbType::Text, aliases: &[] },
    ],
};

// ============================================================================
// Document Mappings
// ============================================================================

pub static DOCUMENT_CATALOG_MAPPINGS: TableMappings = TableMappings {
    table: "document_catalog",
    pk_column: "document_id",
    columns: &[
        ColumnMapping { dsl_key: "document-id", db_column: "document_id", db_type: DbType::Uuid, aliases: &["doc-id", "id"] },
        ColumnMapping { dsl_key: "document-type-id", db_column: "document_type_id", db_type: DbType::Uuid, aliases: &[] },
        ColumnMapping { dsl_key: "cbu-id", db_column: "cbu_id", db_type: DbType::Uuid, aliases: &[] },
        ColumnMapping { dsl_key: "title", db_column: "title", db_type: DbType::Text, aliases: &[] },
        ColumnMapping { dsl_key: "description", db_column: "description", db_type: DbType::Text, aliases: &[] },
        ColumnMapping { dsl_key: "file-path", db_column: "file_path", db_type: DbType::Text, aliases: &[] },
        ColumnMapping { dsl_key: "file-hash", db_column: "file_hash", db_type: DbType::Text, aliases: &[] },
        ColumnMapping { dsl_key: "mime-type", db_column: "mime_type", db_type: DbType::Text, aliases: &["content-type"] },
        ColumnMapping { dsl_key: "status", db_column: "status", db_type: DbType::Text, aliases: &[] },
        ColumnMapping { dsl_key: "verification-status", db_column: "verification_status", db_type: DbType::Text, aliases: &[] },
        ColumnMapping { dsl_key: "extraction-status", db_column: "extraction_status", db_type: DbType::Text, aliases: &[] },
    ],
};

pub static DOCUMENT_CBU_LINKS_MAPPINGS: TableMappings = TableMappings {
    table: "document_cbu_links",
    pk_column: "link_id",
    columns: &[
        ColumnMapping { dsl_key: "link-id", db_column: "link_id", db_type: DbType::Uuid, aliases: &["id"] },
        ColumnMapping { dsl_key: "document-id", db_column: "document_id", db_type: DbType::Uuid, aliases: &["doc-id"] },
        ColumnMapping { dsl_key: "cbu-id", db_column: "cbu_id", db_type: DbType::Uuid, aliases: &[] },
        ColumnMapping { dsl_key: "relationship-type", db_column: "relationship_type", db_type: DbType::Text, aliases: &["rel-type"] },
    ],
};

pub static DOCUMENT_ENTITY_LINKS_MAPPINGS: TableMappings = TableMappings {
    table: "document_entity_links",
    pk_column: "link_id",
    columns: &[
        ColumnMapping { dsl_key: "link-id", db_column: "link_id", db_type: DbType::Uuid, aliases: &["id"] },
        ColumnMapping { dsl_key: "document-id", db_column: "document_id", db_type: DbType::Uuid, aliases: &["doc-id"] },
        ColumnMapping { dsl_key: "entity-id", db_column: "entity_id", db_type: DbType::Uuid, aliases: &[] },
        ColumnMapping { dsl_key: "relationship-type", db_column: "relationship_type", db_type: DbType::Text, aliases: &["rel-type"] },
    ],
};

// ============================================================================
// Product/Service Mappings
// ============================================================================

pub static PRODUCTS_MAPPINGS: TableMappings = TableMappings {
    table: "products",
    pk_column: "product_id",
    columns: &[
        ColumnMapping { dsl_key: "product-id", db_column: "product_id", db_type: DbType::Uuid, aliases: &["id"] },
        ColumnMapping { dsl_key: "name", db_column: "name", db_type: DbType::Text, aliases: &["product-name"] },
        ColumnMapping { dsl_key: "product-code", db_column: "product_code", db_type: DbType::Text, aliases: &["code"] },
        ColumnMapping { dsl_key: "description", db_column: "description", db_type: DbType::Text, aliases: &[] },
        ColumnMapping { dsl_key: "product-category", db_column: "product_category", db_type: DbType::Text, aliases: &["category"] },
        ColumnMapping { dsl_key: "regulatory-framework", db_column: "regulatory_framework", db_type: DbType::Text, aliases: &[] },
        ColumnMapping { dsl_key: "is-active", db_column: "is_active", db_type: DbType::Boolean, aliases: &["active"] },
    ],
};

pub static SERVICES_MAPPINGS: TableMappings = TableMappings {
    table: "services",
    pk_column: "service_id",
    columns: &[
        ColumnMapping { dsl_key: "service-id", db_column: "service_id", db_type: DbType::Uuid, aliases: &["id"] },
        ColumnMapping { dsl_key: "name", db_column: "name", db_type: DbType::Text, aliases: &["service-name"] },
        ColumnMapping { dsl_key: "service-code", db_column: "service_code", db_type: DbType::Text, aliases: &["code"] },
        ColumnMapping { dsl_key: "description", db_column: "description", db_type: DbType::Text, aliases: &[] },
        ColumnMapping { dsl_key: "service-category", db_column: "service_category", db_type: DbType::Text, aliases: &["category"] },
        ColumnMapping { dsl_key: "is-active", db_column: "is_active", db_type: DbType::Boolean, aliases: &["active"] },
    ],
};

pub static PRODUCT_SERVICES_MAPPINGS: TableMappings = TableMappings {
    table: "product_services",
    pk_column: "product_service_id",
    columns: &[
        ColumnMapping { dsl_key: "product-service-id", db_column: "product_service_id", db_type: DbType::Uuid, aliases: &["id", "link-id"] },
        ColumnMapping { dsl_key: "product-id", db_column: "product_id", db_type: DbType::Uuid, aliases: &[] },
        ColumnMapping { dsl_key: "service-id", db_column: "service_id", db_type: DbType::Uuid, aliases: &[] },
        ColumnMapping { dsl_key: "is-required", db_column: "is_required", db_type: DbType::Boolean, aliases: &["required"] },
    ],
};

// ============================================================================
// Investigation/Screening/Risk/Decision/Monitoring Mappings
// ============================================================================

pub static INVESTIGATIONS_MAPPINGS: TableMappings = TableMappings {
    table: "investigations",
    pk_column: "investigation_id",
    columns: &[
        ColumnMapping { dsl_key: "investigation-id", db_column: "investigation_id", db_type: DbType::Uuid, aliases: &["id"] },
        ColumnMapping { dsl_key: "cbu-id", db_column: "cbu_id", db_type: DbType::Uuid, aliases: &[] },
        ColumnMapping { dsl_key: "investigation-type", db_column: "investigation_type", db_type: DbType::Text, aliases: &["type"] },
        ColumnMapping { dsl_key: "status", db_column: "status", db_type: DbType::Text, aliases: &[] },
        ColumnMapping { dsl_key: "outcome", db_column: "outcome", db_type: DbType::Text, aliases: &[] },
        ColumnMapping { dsl_key: "risk-rating", db_column: "risk_rating", db_type: DbType::Text, aliases: &[] },
        ColumnMapping { dsl_key: "ubo-threshold", db_column: "ubo_threshold", db_type: DbType::Decimal, aliases: &["threshold"] },
        ColumnMapping { dsl_key: "deadline", db_column: "deadline", db_type: DbType::Date, aliases: &["due-date"] },
        ColumnMapping { dsl_key: "assigned-to", db_column: "assigned_to", db_type: DbType::Text, aliases: &["assignee"] },
        ColumnMapping { dsl_key: "completed-by", db_column: "completed_by", db_type: DbType::Text, aliases: &[] },
        ColumnMapping { dsl_key: "notes", db_column: "notes", db_type: DbType::Text, aliases: &[] },
    ],
};

pub static SCREENING_RESULTS_MAPPINGS: TableMappings = TableMappings {
    table: "screening_results",
    pk_column: "result_id",
    columns: &[
        ColumnMapping { dsl_key: "result-id", db_column: "result_id", db_type: DbType::Uuid, aliases: &["id"] },
        ColumnMapping { dsl_key: "screening-id", db_column: "screening_id", db_type: DbType::Uuid, aliases: &[] },
        ColumnMapping { dsl_key: "result", db_column: "result", db_type: DbType::Text, aliases: &[] },
        ColumnMapping { dsl_key: "resolution", db_column: "resolution", db_type: DbType::Text, aliases: &[] },
        ColumnMapping { dsl_key: "match-details", db_column: "match_details", db_type: DbType::Jsonb, aliases: &[] },
        ColumnMapping { dsl_key: "rationale", db_column: "rationale", db_type: DbType::Text, aliases: &[] },
        ColumnMapping { dsl_key: "reviewed-by", db_column: "reviewed_by", db_type: DbType::Text, aliases: &[] },
        ColumnMapping { dsl_key: "resolved-by", db_column: "resolved_by", db_type: DbType::Text, aliases: &[] },
    ],
};

pub static RISK_RATINGS_MAPPINGS: TableMappings = TableMappings {
    table: "risk_ratings",
    pk_column: "rating_id",
    columns: &[
        ColumnMapping { dsl_key: "rating-id", db_column: "rating_id", db_type: DbType::Uuid, aliases: &["id"] },
        ColumnMapping { dsl_key: "cbu-id", db_column: "cbu_id", db_type: DbType::Uuid, aliases: &[] },
        ColumnMapping { dsl_key: "entity-id", db_column: "entity_id", db_type: DbType::Uuid, aliases: &[] },
        ColumnMapping { dsl_key: "rating", db_column: "rating", db_type: DbType::Text, aliases: &[] },
        ColumnMapping { dsl_key: "factors", db_column: "factors", db_type: DbType::Jsonb, aliases: &[] },
        ColumnMapping { dsl_key: "rationale", db_column: "rationale", db_type: DbType::Text, aliases: &[] },
        ColumnMapping { dsl_key: "assessed-by", db_column: "assessed_by", db_type: DbType::Text, aliases: &[] },
    ],
};

pub static RISK_FLAGS_MAPPINGS: TableMappings = TableMappings {
    table: "risk_flags",
    pk_column: "flag_id",
    columns: &[
        ColumnMapping { dsl_key: "flag-id", db_column: "flag_id", db_type: DbType::Uuid, aliases: &["id"] },
        ColumnMapping { dsl_key: "cbu-id", db_column: "cbu_id", db_type: DbType::Uuid, aliases: &[] },
        ColumnMapping { dsl_key: "entity-id", db_column: "entity_id", db_type: DbType::Uuid, aliases: &[] },
        ColumnMapping { dsl_key: "flag-type", db_column: "flag_type", db_type: DbType::Text, aliases: &["type"] },
        ColumnMapping { dsl_key: "description", db_column: "description", db_type: DbType::Text, aliases: &[] },
        ColumnMapping { dsl_key: "severity", db_column: "severity", db_type: DbType::Text, aliases: &[] },
        ColumnMapping { dsl_key: "flagged-by", db_column: "flagged_by", db_type: DbType::Text, aliases: &[] },
    ],
};

pub static DECISIONS_MAPPINGS: TableMappings = TableMappings {
    table: "decisions",
    pk_column: "decision_id",
    columns: &[
        ColumnMapping { dsl_key: "decision-id", db_column: "decision_id", db_type: DbType::Uuid, aliases: &["id"] },
        ColumnMapping { dsl_key: "cbu-id", db_column: "cbu_id", db_type: DbType::Uuid, aliases: &[] },
        ColumnMapping { dsl_key: "investigation-id", db_column: "investigation_id", db_type: DbType::Uuid, aliases: &[] },
        ColumnMapping { dsl_key: "decision", db_column: "decision", db_type: DbType::Text, aliases: &[] },
        ColumnMapping { dsl_key: "decision-authority", db_column: "decision_authority", db_type: DbType::Text, aliases: &[] },
        ColumnMapping { dsl_key: "rationale", db_column: "rationale", db_type: DbType::Text, aliases: &[] },
        ColumnMapping { dsl_key: "decided-by", db_column: "decided_by", db_type: DbType::Text, aliases: &[] },
    ],
};

pub static DECISION_CONDITIONS_MAPPINGS: TableMappings = TableMappings {
    table: "decision_conditions",
    pk_column: "condition_id",
    columns: &[
