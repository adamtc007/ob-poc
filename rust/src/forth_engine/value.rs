//! Value types for the DSL Forth Engine.

use serde::{Deserialize, Serialize};
use std::fmt;

#[derive(Debug, Clone, PartialEq, Eq, Hash, Serialize, Deserialize)]
pub struct AttributeId(pub String);

#[derive(Debug, Clone, PartialEq, Eq, Hash, Serialize, Deserialize)]
pub struct DocumentId(pub String);

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub enum Value {
    Int(i64),
    Float(f64),
    Str(String),
    Bool(bool),
    Keyword(String),            // :case-id, :case-type, etc.
    DottedKeyword(Vec<String>), // :customer.id -> ["customer", "id"]
    Attr(AttributeId),
    Doc(DocumentId),
    List(Vec<Value>),
    Map(Vec<(String, Value)>),
}

impl fmt::Display for Value {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            Value::Int(i) => write!(f, "{}", i),
            Value::Float(n) => write!(f, "{}", n),
            Value::Str(s) => write!(f, "\"{}\"", s),
            Value::Bool(b) => write!(f, "{}", b),
            Value::Keyword(k) => write!(f, "{}", k),
            Value::DottedKeyword(parts) => write!(f, ":{}", parts.join(".")),
            Value::Attr(id) => write!(f, "@attr{{{}}}", id.0),
            Value::Doc(id) => write!(f, "@doc{{{}}}", id.0),
            Value::List(items) => {
                write!(f, "[")?;
                for (i, item) in items.iter().enumerate() {
                    if i > 0 {
                        write!(f, " ")?;
                    }
                    write!(f, "{}", item)?;
                }
                write!(f, "]")
            }
            Value::Map(pairs) => {
                write!(f, "{{")?;
                for (i, (key, value)) in pairs.iter().enumerate() {
                    if i > 0 {
                        write!(f, " ")?;
                    }
                    write!(f, ":{} {}", key, value)?;
                }
                write!(f, "}}")
            }
        }
    }
}

// CRUD Statement types - database operation representations

use std::collections::HashMap;

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub enum CrudStatement {
    DataCreate(DataCreate),
    DataRead(DataRead),
    DataUpdate(DataUpdate),
    DataDelete(DataDelete),
}

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct DataCreate {
    pub asset: String,
    pub values: HashMap<String, Value>,
}

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct DataRead {
    pub asset: String,
    pub where_clause: HashMap<String, Value>,
    pub select: Vec<String>,
    pub limit: Option<u32>,
}

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct DataUpdate {
    pub asset: String,
    pub where_clause: HashMap<String, Value>,
    pub values: HashMap<String, Value>,
}

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct DataDelete {
    pub asset: String,
    pub where_clause: HashMap<String, Value>,
}
