//! Structured-intent DTOs — hoisted from `ob-poc`'s `mcp::intent_pipeline`
//! (T11.1b, 2026-07-12).
//!
//! Pure data shapes: serde derives, no IO, no crate-internal deps. Needed by
//! `ob-poc-agent`'s Sage arg-assembly/drafting engines and by `ob-poc`'s own
//! `mcp::intent_pipeline` (the LLM extraction pipeline), which re-exports
//! these from here rather than defining them twice.
//!
//! Separate from DSL's `ArgumentValue` to avoid ripple effects across
//! serde/DB/UI boundaries. Converted to DSL syntax during assembly.

use serde::{Deserialize, Serialize};
use std::collections::BTreeMap;

#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(tag = "type", content = "data")]
pub enum IntentArgValue {
    /// Plain string literal (no lookup config)
    String(String),
    /// Numeric value
    Number(f64),
    /// Boolean value
    Boolean(bool),
    /// @symbol reference
    Reference(String),
    /// Resolved UUID
    Uuid(String),
    /// Needs entity resolution (has lookup config in YAML)
    Unresolved {
        value: String,
        entity_type: Option<String>,
    },
    /// Required arg not extracted by LLM
    Missing { arg_name: String },
    /// List of values
    List(Vec<IntentArgValue>),
    /// Map of key-value pairs (BTreeMap for stable ordering)
    Map(BTreeMap<String, IntentArgValue>),
}

/// A single argument extracted from user intent
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct IntentArgument {
    pub name: String,
    pub value: IntentArgValue,
    pub resolved: bool,
}

/// Extracted structured intent
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct StructuredIntent {
    /// The verb to execute
    pub verb: String,
    /// Extracted argument values
    pub arguments: Vec<IntentArgument>,
    /// Confidence in extraction
    pub confidence: f32,
    /// Any extraction notes/warnings
    pub notes: Vec<String>,
}

impl StructuredIntent {
    /// Create empty intent (for early exit cases)
    pub fn empty() -> Self {
        Self {
            verb: String::new(),
            arguments: vec![],
            confidence: 0.0,
            notes: vec![],
        }
    }
}

/// Format IntentArgValue to DSL string only (Fix C - no synthetic refs)
///
/// Unresolved refs are extracted from the enriched AST after parsing,
/// which gives us real span-based ref_ids and search_column metadata.
pub fn format_intent_value_string_only(value: &IntentArgValue) -> String {
    match value {
        IntentArgValue::String(s) => format!("\"{}\"", s.replace('"', "\\\"")),
        IntentArgValue::Number(n) => n.to_string(),
        IntentArgValue::Boolean(b) => b.to_string(),
        IntentArgValue::Reference(r) => format!("@{}", r),
        IntentArgValue::Uuid(u) => format!("\"{}\"", u),
        IntentArgValue::Unresolved { value, .. } => {
            // Emit as quoted string - enrichment pass will convert to EntityRef
            // based on verb arg's lookup config
            format!("\"{}\"", value.replace('"', "\\\""))
        }
        IntentArgValue::Missing { .. } => "nil".to_string(),
        IntentArgValue::List(items) => {
            let formatted: Vec<String> =
                items.iter().map(format_intent_value_string_only).collect();
            format!("[{}]", formatted.join(" "))
        }
        IntentArgValue::Map(entries) => {
            let formatted: Vec<String> = entries
                .iter()
                .map(|(k, v)| format!(":{} {}", k, format_intent_value_string_only(v)))
                .collect();
            format!("{{{}}}", formatted.join(" "))
        }
    }
}

/// Assemble a DSL string from a structured intent.
///
/// T11.1b (2026-07-12): hoisted from `ob-poc::mcp::intent_pipeline`
/// alongside `StructuredIntent`/`IntentArgValue` — needed by
/// `ob-poc-agent`'s Sage arg-assembly/drafting engines, which cannot
/// depend on `ob-poc` (L1). Pure formatting, no IO.
pub fn assemble_dsl_string(intent: &StructuredIntent) -> anyhow::Result<String> {
    let mut dsl = format!("({}", intent.verb);

    for arg in &intent.arguments {
        if matches!(arg.value, IntentArgValue::Missing { .. }) {
            continue;
        }

        let value_str = format_intent_value_string_only(&arg.value);
        dsl.push_str(&format!(" :{} {}", arg.name, value_str));
    }

    dsl.push(')');
    Ok(dsl)
}
