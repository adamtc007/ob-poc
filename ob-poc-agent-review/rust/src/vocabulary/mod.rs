//! Vocabulary management for the UBO DSL system
//!
//! This module provides vocabulary management with domain-prefixed verb conventions,
//! following idiomatic Rust patterns for clean architecture and type safety.

pub(crate) mod vocab_registry;

// Re-export the clean vocabulary registry
// pub use verb_registry::{DeprecationPolicy, RegistryConfig, RegistryStats, VocabularyRegistry};

use serde::{Deserialize, Serialize};

/// Registry entry for vocabulary verbs with domain-prefixed convention
#[derive(Debug, Clone, Serialize, Deserialize)]
pub(crate) struct VerbRegistryEntry {
    /// Fully-qualified verb name (e.g., "kyc.declare-entity")
    pub verb_name: String,
    /// Domain extracted from verb name (e.g., "kyc")
    pub domain: String,
    /// Whether this verb can be shared across domains
    pub shared: bool,
    /// Whether this verb is deprecated
    pub deprecated: bool,
    /// Optional replacement verb for deprecated verbs
    pub replacement_verb: Option<String>,
    /// Optional description
    pub description: Option<String>,
    /// Version information
    pub version: String,
}

impl VerbRegistryEntry {
    /// Create a new registry entry
    #[allow(dead_code)]
    pub fn new(verb_name: impl Into<String>, domain: impl Into<String>, shared: bool) -> Self {
        Self {
            verb_name: verb_name.into(),
            domain: domain.into(),
            shared,
            deprecated: false,
            replacement_verb: None,
            description: None,
            version: "1.0".to_string(),
        }
    }

    /// Mark this verb as deprecated with an optional replacement
    #[allow(dead_code)]
    pub fn deprecate(mut self, replacement: Option<String>) -> Self {
        self.deprecated = true;
        self.replacement_verb = replacement;
        self
    }

    /// Add a description to this verb
    #[allow(dead_code)]
    pub fn with_description(mut self, description: impl Into<String>) -> Self {
        self.description = Some(description.into());
        self
    }

    /// Set the version for this verb
    #[allow(dead_code)]
    pub(crate) fn with_version(mut self, version: impl Into<String>) -> Self {
        self.version = version.into();
        self
    }
}
