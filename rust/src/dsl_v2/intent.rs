//! Structured Intent for DSL Generation
//!
//! This module defines a general-purpose intent schema that can represent
//! any DSL operation. The key insight is:
//!
//! 1. AI extracts a STRUCTURED intent (not DSL text)
//! 2. Rust code performs DETERMINISTIC lookups via EntityGateway
//! 3. Rust code assembles VALID DSL from templates + resolved values
//! 4. Validation is deterministic (parser + CSG linter)
//!
//! This minimizes AI variance and maximizes determinism.

use serde::{Deserialize, Serialize};
use std::collections::HashMap;

/// A single DSL action intent - what the user wants to do
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct DslIntent {
    /// The verb to execute (e.g., "cbu.assign-role", "entity.create-proper-person")
    /// AI picks from known verbs, or we infer from action type
    pub verb: Option<String>,

    /// High-level action type when verb isn't specified
    /// e.g., "create", "assign", "add", "remove", "update"
    pub action: String,

    /// The domain this operates on (e.g., "cbu", "entity", "document")
    pub domain: String,

    /// Arguments with their search keys (not UUIDs - those come from lookups)
    pub args: HashMap<String, ArgIntent>,

    /// Symbol to bind result to (e.g., "fund", "john")
    pub bind_as: Option<String>,

    /// Original natural language for this action (for error messages)
    pub source_text: Option<String>,
}

/// An argument value that needs resolution
#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(tag = "type")]
pub enum ArgIntent {
    /// A literal value (string, number, boolean)
    Literal { value: serde_json::Value },

    /// A reference to a previously bound symbol (e.g., @fund)
    SymbolRef { symbol: String },

    /// An entity lookup by search text
    /// EntityGateway will resolve this to a real ID
    EntityLookup {
        /// What the user typed/meant (e.g., "John Smith", "Apex Capital")
        search_text: String,
        /// Expected entity type if known (e.g., "person", "cbu", "entity")
        entity_type: Option<String>,
    },

    /// A reference data lookup (role, jurisdiction, etc.)
    RefDataLookup {
        /// The code or name to look up (e.g., "director", "Luxembourg")
        search_text: String,
        /// The reference type (e.g., "role", "jurisdiction", "currency")
        ref_type: String,
    },
}

/// Multiple actions to perform in sequence
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct DslIntentBatch {
    /// The individual actions in order
    pub actions: Vec<DslIntent>,

    /// Overall context/goal
    pub context: Option<String>,

    /// Original natural language request
    pub original_request: String,
}

/// Result of resolving an ArgIntent via EntityGateway
#[derive(Debug, Clone)]
pub struct ResolvedArg {
    /// The DSL value to insert (UUID, code, or literal)
    pub value: String,

    /// Whether this is a symbol reference (@name)
    pub is_symbol_ref: bool,

    /// Whether this needs quoting in DSL
    pub needs_quotes: bool,

    /// Display text for the resolution (for logging/debugging)
    pub display: Option<String>,
}

impl DslIntent {
    /// Create a simple literal arg
    pub fn literal(key: &str, value: impl Into<serde_json::Value>) -> (String, ArgIntent) {
        (
            key.to_string(),
            ArgIntent::Literal {
                value: value.into(),
            },
        )
    }

    /// Create a symbol reference arg
    pub fn symbol_ref(key: &str, symbol: &str) -> (String, ArgIntent) {
        (
            key.to_string(),
            ArgIntent::SymbolRef {
                symbol: symbol.to_string(),
            },
        )
    }

    /// Create an entity lookup arg
    pub fn entity_lookup(
        key: &str,
        search_text: &str,
        entity_type: Option<&str>,
    ) -> (String, ArgIntent) {
        (
            key.to_string(),
            ArgIntent::EntityLookup {
                search_text: search_text.to_string(),
                entity_type: entity_type.map(String::from),
            },
        )
    }

    /// Create a ref data lookup arg
    pub fn ref_lookup(key: &str, search_text: &str, ref_type: &str) -> (String, ArgIntent) {
        (
            key.to_string(),
            ArgIntent::RefDataLookup {
                search_text: search_text.to_string(),
                ref_type: ref_type.to_string(),
            },
        )
    }
}

impl DslIntentBatch {
    pub fn new(original_request: impl Into<String>) -> Self {
        Self {
            actions: Vec::new(),
            context: None,
            original_request: original_request.into(),
        }
    }

    pub fn with_context(mut self, context: impl Into<String>) -> Self {
        self.context = Some(context.into());
        self
    }

    pub fn add_action(mut self, action: DslIntent) -> Self {
        self.actions.push(action);
        self
    }
}

/// Example intent for: "Add John Smith as director of Apex Fund"
///
/// ```rust,ignore
/// DslIntent {
///     verb: Some("cbu.assign-role"),
///     action: "assign".to_string(),
///     domain: "cbu".to_string(),
///     args: HashMap::from([
///         DslIntent::entity_lookup("cbu-id", "Apex Fund", Some("cbu")),
///         DslIntent::entity_lookup("entity-id", "John Smith", Some("person")),
///         DslIntent::ref_lookup("role", "director", "role"),
///     ]),
///     bind_as: None,
///     source_text: Some("Add John Smith as director of Apex Fund"),
/// }
/// ```
#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_create_intent() {
        let intent = DslIntent {
            verb: Some("cbu.assign-role".to_string()),
            action: "assign".to_string(),
            domain: "cbu".to_string(),
            args: HashMap::from([
                DslIntent::entity_lookup("cbu-id", "Apex Fund", Some("cbu")),
                DslIntent::entity_lookup("entity-id", "John Smith", Some("person")),
                DslIntent::ref_lookup("role", "director", "role"),
            ]),
            bind_as: None,
            source_text: Some("Add John Smith as director".to_string()),
        };

        assert_eq!(intent.verb, Some("cbu.assign-role".to_string()));
        assert_eq!(intent.args.len(), 3);
    }

    #[test]
    fn test_batch_intent() {
        let batch = DslIntentBatch::new("Create fund and add director")
            .with_context("New fund onboarding")
            .add_action(DslIntent {
                verb: Some("cbu.ensure".to_string()),
                action: "create".to_string(),
                domain: "cbu".to_string(),
                args: HashMap::from([
                    DslIntent::literal("name", "Test Fund"),
                    DslIntent::literal("jurisdiction", "LU"),
                ]),
                bind_as: Some("fund".to_string()),
                source_text: None,
            });

        assert_eq!(batch.actions.len(), 1);
        assert!(batch.context.is_some());
    }
}
