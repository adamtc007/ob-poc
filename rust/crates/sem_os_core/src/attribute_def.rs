//! Attribute definition body types — pure value types, no DB dependency.

use serde::{Deserialize, Serialize};

/// Body of an `attribute_def` registry snapshot.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct AttributeDefBody {
    pub fqn: String,
    pub name: String,
    pub description: String,
    pub domain: String,
    pub data_type: AttributeDataType,
    #[serde(default)]
    pub source: Option<AttributeSource>,
    #[serde(default)]
    pub constraints: Option<AttributeConstraints>,
    #[serde(default, skip_serializing_if = "Vec::is_empty")]
    pub sinks: Vec<AttributeSink>,
}

/// Supported attribute data types.
#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum AttributeDataType {
    String,
    Integer,
    Decimal,
    Boolean,
    Uuid,
    Date,
    Timestamp,
    Json,
    Enum(Vec<std::string::String>),
}

/// Where an attribute value comes from.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct AttributeSource {
    #[serde(default)]
    pub producing_verb: Option<String>,
    #[serde(default)]
    pub table: Option<String>,
    #[serde(default)]
    pub column: Option<String>,
    #[serde(default)]
    pub derived: bool,
}

/// Validation constraints on attribute values.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct AttributeConstraints {
    #[serde(default)]
    pub required: bool,
    #[serde(default)]
    pub unique: bool,
    #[serde(default)]
    pub min_length: Option<usize>,
    #[serde(default)]
    pub max_length: Option<usize>,
    #[serde(default)]
    pub pattern: Option<String>,
    #[serde(default)]
    pub valid_values: Option<Vec<String>>,
}

/// A consuming verb that reads this attribute as an argument.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct AttributeSink {
    pub consuming_verb: String,
    pub arg_name: String,
}
