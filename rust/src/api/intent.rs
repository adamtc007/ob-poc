//! Intent-based DSL generation types
//!
//! The LLM outputs structured intents, not DSL code.
//! Rust validates and assembles DSL deterministically.

use serde::{Deserialize, Serialize};
use std::collections::HashMap;
use uuid::Uuid;

/// A single verb intent extracted from natural language
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct VerbIntent {
    /// The verb to execute, e.g., "cbu.ensure", "entity.create-proper-person"
    pub verb: String,

    /// Parameters with literal values
    /// e.g., {"cbu-name": "Acme Corp", "client-type": "COMPANY"}
    #[serde(default)]
    pub params: HashMap<String, ParamValue>,

    /// References to previous results (optional)
    /// e.g., {"cbu-id": "@last_cbu", "entity-id": "@last_entity"}
    #[serde(default)]
    pub refs: HashMap<String, String>,

    /// Optional ordering hint for complex sequences
    #[serde(default)]
    pub sequence: Option<u32>,
}

/// Parameter value types that can appear in intents
#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(untagged)]
pub enum ParamValue {
    String(String),
    Number(f64),
    Integer(i64),
    Boolean(bool),
    Uuid(Uuid),
    List(Vec<ParamValue>),
    Object(HashMap<String, ParamValue>),
}

impl ParamValue {
    /// Convert to DSL string representation
    pub fn to_dsl_string(&self) -> String {
        match self {
            ParamValue::String(s) => {
                // If the string starts with @, it's a reference - don't quote it
                if s.starts_with('@') {
                    s.clone()
                } else {
                    format!("\"{}\"", s.replace('\"', "\\\""))
                }
            }
            ParamValue::Number(n) => n.to_string(),
            ParamValue::Integer(i) => i.to_string(),
            ParamValue::Boolean(b) => if *b { "true" } else { "false" }.to_string(),
            ParamValue::Uuid(u) => format!("\"{}\"", u),
            ParamValue::List(items) => {
                let inner: Vec<String> = items.iter().map(|v| v.to_dsl_string()).collect();
                format!("[{}]", inner.join(" "))
            }
            ParamValue::Object(map) => {
                let pairs: Vec<String> = map
                    .iter()
                    .map(|(k, v)| format!(":{} {}", k, v.to_dsl_string()))
                    .collect();
                format!("{{{}}}", pairs.join(" "))
            }
        }
    }
}

/// Sequence of intents extracted from a single user message
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct IntentSequence {
    pub intents: Vec<VerbIntent>,
    /// LLM's reasoning about the extraction
    pub reasoning: Option<String>,
    /// Confidence score (0.0-1.0)
    pub confidence: Option<f64>,
}

/// Result of validating an intent against the verb registry
#[derive(Debug, Clone, Serialize)]
pub struct IntentValidation {
    pub valid: bool,
    pub intent: VerbIntent,
    pub errors: Vec<IntentError>,
    pub warnings: Vec<String>,
}

#[derive(Debug, Clone, Serialize)]
pub struct IntentError {
    pub code: String,
    pub message: String,
    pub param: Option<String>,
}

/// Result of assembling DSL from validated intents
#[derive(Debug, Clone, Serialize)]
pub struct AssembledDsl {
    pub statements: Vec<String>,
    pub combined: String,
    pub intent_count: usize,
}

// ============================================================================
// Tests
// ============================================================================

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_param_value_string() {
        let pv = ParamValue::String("Hello World".to_string());
        assert_eq!(pv.to_dsl_string(), "\"Hello World\"");
    }

    #[test]
    fn test_param_value_string_with_quotes() {
        let pv = ParamValue::String("Say \"hello\"".to_string());
        assert_eq!(pv.to_dsl_string(), "\"Say \\\"hello\\\"\"");
    }

    #[test]
    fn test_param_value_number() {
        let pv = ParamValue::Number(42.5);
        assert_eq!(pv.to_dsl_string(), "42.5");
    }

    #[test]
    fn test_param_value_integer() {
        let pv = ParamValue::Integer(100);
        assert_eq!(pv.to_dsl_string(), "100");
    }

    #[test]
    fn test_param_value_boolean() {
        assert_eq!(ParamValue::Boolean(true).to_dsl_string(), "true");
        assert_eq!(ParamValue::Boolean(false).to_dsl_string(), "false");
    }

    #[test]
    fn test_param_value_uuid() {
        let id = Uuid::parse_str("550e8400-e29b-41d4-a716-446655440000").unwrap();
        let pv = ParamValue::Uuid(id);
        assert_eq!(
            pv.to_dsl_string(),
            "\"550e8400-e29b-41d4-a716-446655440000\""
        );
    }

    #[test]
    fn test_param_value_list() {
        let pv = ParamValue::List(vec![
            ParamValue::String("a".to_string()),
            ParamValue::String("b".to_string()),
        ]);
        assert_eq!(pv.to_dsl_string(), "[\"a\" \"b\"]");
    }

    #[test]
    fn test_intent_sequence_deserialize() {
        let json = r#"{
            "intents": [
                {
                    "verb": "cbu.ensure",
                    "params": {"cbu-name": "Test Corp"},
                    "refs": {}
                }
            ],
            "reasoning": "Creating a CBU",
            "confidence": 0.95
        }"#;

        let seq: IntentSequence = serde_json::from_str(json).unwrap();
        assert_eq!(seq.intents.len(), 1);
        assert_eq!(seq.intents[0].verb, "cbu.ensure");
        assert_eq!(seq.confidence, Some(0.95));
    }

    #[test]
    fn test_verb_intent_with_refs() {
        let json = r#"{
            "verb": "cbu.attach-entity",
            "params": {"role": "DIRECTOR"},
            "refs": {"cbu-id": "@last_cbu", "entity-id": "@last_entity"}
        }"#;

        let intent: VerbIntent = serde_json::from_str(json).unwrap();
        assert_eq!(intent.verb, "cbu.attach-entity");
        assert_eq!(intent.refs.get("cbu-id"), Some(&"@last_cbu".to_string()));
    }
}
