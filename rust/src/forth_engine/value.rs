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
    DataUpsert(DataUpsert),
}

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize, Default)]
pub struct DataCreate {
    pub asset: String,
    pub values: HashMap<String, Value>,
    /// If set, capture the returned ID into RuntimeEnv under this key.
    /// Supported keys: "cbu_id", "entity_id", "investigation_id", "decision_id"
    #[serde(default)]
    pub capture_result: Option<String>,
}

impl DataCreate {
    /// Create a new DataCreate with no result capture
    pub fn new(asset: impl Into<String>, values: HashMap<String, Value>) -> Self {
        Self {
            asset: asset.into(),
            values,
            capture_result: None,
        }
    }

    /// Create a new DataCreate that captures the result into the given key
    pub fn with_capture(
        asset: impl Into<String>,
        values: HashMap<String, Value>,
        capture_key: impl Into<String>,
    ) -> Self {
        Self {
            asset: asset.into(),
            values,
            capture_result: Some(capture_key.into()),
        }
    }
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

/// UPSERT operation - create if not exists, update if changed.
/// Uses natural keys (conflict_keys) to determine identity.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize, Default)]
pub struct DataUpsert {
    pub asset: String,
    pub values: HashMap<String, Value>,
    /// Natural key fields used to detect conflicts (e.g., ["cbu-name", "jurisdiction"] for CBU)
    pub conflict_keys: Vec<String>,
    /// If set, capture the returned ID into RuntimeEnv under this key.
    /// Supported keys: "cbu_id", "entity_id", "investigation_id", "decision_id"
    #[serde(default)]
    pub capture_result: Option<String>,
}

impl DataUpsert {
    /// Create a new DataUpsert with no result capture
    pub fn new(
        asset: impl Into<String>,
        values: HashMap<String, Value>,
        conflict_keys: Vec<String>,
    ) -> Self {
        Self {
            asset: asset.into(),
            values,
            conflict_keys,
            capture_result: None,
        }
    }

    /// Create a new DataUpsert that captures the result into the given key
    pub fn with_capture(
        asset: impl Into<String>,
        values: HashMap<String, Value>,
        conflict_keys: Vec<String>,
        capture_key: impl Into<String>,
    ) -> Self {
        Self {
            asset: asset.into(),
            values,
            conflict_keys,
            capture_result: Some(capture_key.into()),
        }
    }
}
