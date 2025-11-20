//! Value types for the DSL Forth Engine.

use serde::{Deserialize, Serialize};
use std::fmt;

#[derive(Debug, Clone, PartialEq, Eq, Hash, Serialize, Deserialize)]
pub struct AttributeId(pub String);

#[derive(Debug, Clone, PartialEq, Eq, Hash, Serialize, Deserialize)]
pub struct DocumentId(pub String);

#[derive(Debug, Clone, Serialize, Deserialize)]
pub enum Value {
    Int(i64),
    Str(String),
    Bool(bool),
    Keyword(String), // :case-id, :case-type, etc.
    Attr(AttributeId),
    Doc(DocumentId),
}

impl fmt::Display for Value {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            Value::Int(i) => write!(f, "{}", i),
            Value::Str(s) => write!(f, "\"{}\"", s),
            Value::Bool(b) => write!(f, "{}", b),
            Value::Keyword(k) => write!(f, "{}", k),
            Value::Attr(id) => write!(f, "Attr({})", id.0),
            Value::Doc(id) => write!(f, "Doc({})", id.0),
        }
    }
}
