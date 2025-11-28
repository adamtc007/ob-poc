//! AST type definitions for DSL v2
//!
//! These types represent the parsed structure of a DSL program.
//! They are intentionally simple and contain no execution logic.

use rust_decimal::Decimal;
use std::collections::HashMap;
use uuid::Uuid;

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
