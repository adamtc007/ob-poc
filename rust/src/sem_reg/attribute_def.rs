//! Attribute definition body — the typed JSONB content for `ObjectType::AttributeDef`.

use serde::{Deserialize, Serialize};

/// The JSONB body stored in `definition` for attribute definitions.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct AttributeDefBody {
    /// Fully qualified name, e.g. "cbu.jurisdiction_code"
    pub fqn: String,
    /// Human-readable name
    pub name: String,
    /// Description of the attribute
    pub description: String,
    /// Domain this attribute belongs to (e.g. "cbu", "entity", "kyc")
    pub domain: String,
    /// Data type
    pub data_type: AttributeDataType,
    /// Source specification — where the attribute value comes from
    #[serde(default)]
    pub source: Option<AttributeSource>,
    /// Constraints on values
    #[serde(default)]
    pub constraints: Option<AttributeConstraints>,
    /// Where this attribute is consumed (sinks)
    #[serde(default, skip_serializing_if = "Vec::is_empty")]
    pub sinks: Vec<AttributeSink>,
}

/// Data type for an attribute.
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
    Enum(Vec<String>),
}

/// Source specification for an attribute.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct AttributeSource {
    /// The verb that produces this attribute (e.g. "cbu.create")
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
    /// Whether this is a derived attribute (requires DerivationSpec)
    #[serde(default)]
    pub derived: bool,
}

/// Value constraints for an attribute.
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

/// Sink specification — where an attribute is consumed.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct AttributeSink {
    /// The verb that consumes this attribute
    pub consuming_verb: String,
    /// The argument name in the consuming verb
    pub arg_name: String,
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_attribute_def_serde_round_trip() {
        let body = AttributeDefBody {
            fqn: "cbu.jurisdiction_code".into(),
            name: "Jurisdiction Code".into(),
            description: "ISO country code for the CBU".into(),
            domain: "cbu".into(),
            data_type: AttributeDataType::String,
            source: Some(AttributeSource {
                producing_verb: Some("cbu.create".into()),
                schema: Some("ob-poc".into()),
                table: Some("cbus".into()),
                column: Some("jurisdiction".into()),
                derived: false,
            }),
            constraints: Some(AttributeConstraints {
                required: true,
                unique: false,
                min_length: Some(2),
                max_length: Some(3),
                pattern: Some("^[A-Z]{2,3}$".into()),
                valid_values: None,
            }),
            sinks: vec![AttributeSink {
                consuming_verb: "session.load-jurisdiction".into(),
                arg_name: "code".into(),
            }],
        };

        let json = serde_json::to_value(&body).unwrap();
        let back: AttributeDefBody = serde_json::from_value(json).unwrap();
        assert_eq!(back.fqn, "cbu.jurisdiction_code");
        assert_eq!(back.sinks.len(), 1);
    }

    #[test]
    fn test_enum_data_type() {
        let body = AttributeDefBody {
            fqn: "cbu.client_type".into(),
            name: "Client Type".into(),
            description: "Type of client".into(),
            domain: "cbu".into(),
            data_type: AttributeDataType::Enum(vec![
                "FUND".into(),
                "CORPORATE".into(),
                "BANK".into(),
            ]),
            source: None,
            constraints: None,
            sinks: vec![],
        };

        let json = serde_json::to_value(&body).unwrap();
        let back: AttributeDefBody = serde_json::from_value(json).unwrap();
        match &back.data_type {
            AttributeDataType::Enum(vals) => assert_eq!(vals.len(), 3),
            _ => panic!("Expected Enum data type"),
        }
    }
}
