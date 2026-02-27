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
    /// DB schema where the canonical value lives (e.g. "ob-poc", "sem_reg")
    #[serde(default)]
    pub schema: Option<String>,
    /// DB table where the canonical value lives (e.g. "cbus", "entities")
    #[serde(default)]
    pub table: Option<String>,
    /// DB column name (e.g. "jurisdiction_code")
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

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn serde_round_trip() {
        let val = AttributeDefBody {
            fqn: "cbu.jurisdiction_code".into(),
            name: "jurisdiction_code".into(),
            description: "ISO jurisdiction".into(),
            domain: "cbu".into(),
            data_type: AttributeDataType::Enum(vec!["LU".into(), "IE".into(), "DE".into()]),
            source: Some(AttributeSource {
                producing_verb: Some("cbu.create".into()),
                schema: Some("ob-poc".into()),
                table: Some("cbus".into()),
                column: Some("jurisdiction_code".into()),
                derived: false,
            }),
            constraints: Some(AttributeConstraints {
                required: true,
                unique: false,
                min_length: Some(2),
                max_length: Some(2),
                pattern: Some("^[A-Z]{2}$".into()),
                valid_values: Some(vec!["LU".into(), "IE".into()]),
            }),
            sinks: vec![AttributeSink {
                consuming_verb: "session.load-jurisdiction".into(),
                arg_name: "code".into(),
            }],
        };
        let json = serde_json::to_value(&val).unwrap();
        // Check Enum variant serialization
        assert!(json["data_type"]["enum"].is_array());
        // Check #[serde(default)] on optional fields: omitting them deserializes fine
        let minimal: AttributeDefBody = serde_json::from_str(
            r#"{"fqn":"x","name":"x","description":"x","domain":"x","data_type":"string"}"#,
        ).unwrap();
        assert!(minimal.source.is_none());
        assert!(minimal.sinks.is_empty());
        // Round-trip
        let back: AttributeDefBody = serde_json::from_value(json.clone()).unwrap();
        let json2 = serde_json::to_value(&back).unwrap();
        assert_eq!(json, json2);
    }
}
