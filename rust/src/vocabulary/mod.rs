//! Vocabulary management for the UBO DSL system
//!
//! This module provides vocabulary management with domain-prefixed verb conventions,
//! following idiomatic Rust patterns for clean architecture and type safety.

pub mod vocab_registry;

// Re-export the clean vocabulary registry
pub use vocab_registry::{DeprecationPolicy, RegistryConfig, RegistryStats, VocabularyRegistry};

use serde::{Deserialize, Serialize};

/// Registry entry for vocabulary verbs with domain-prefixed convention
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct VerbRegistryEntry {
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
    pub fn deprecate(mut self, replacement: Option<String>) -> Self {
        self.deprecated = true;
        self.replacement_verb = replacement;
        self
    }

    /// Add a description to this verb
    pub fn with_description(mut self, description: impl Into<String>) -> Self {
        self.description = Some(description.into());
        self
    }

    /// Set the version for this verb
    pub fn with_version(mut self, version: impl Into<String>) -> Self {
        self.version = version.into();
        self
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_verb_registry_entry_creation() {
        let entry = VerbRegistryEntry::new("kyc.declare-entity", "kyc", false);
        assert_eq!(entry.verb_name, "kyc.declare-entity");
        assert_eq!(entry.domain, "kyc");
        assert!(!entry.shared);
        assert!(!entry.deprecated);
    }

    #[test]
    fn test_verb_registry_entry_builder() {
        let entry = VerbRegistryEntry::new("common.validate", "common", true)
            .with_description("Common validation verb")
            .with_version("2.0");

        assert_eq!(entry.verb_name, "common.validate");
        assert!(entry.shared);
        assert_eq!(
            entry.description,
            Some("Common validation verb".to_string())
        );
        assert_eq!(entry.version, "2.0");
    }

    #[test]
    fn test_deprecation() {
        let entry = VerbRegistryEntry::new("old.action", "old", false)
            .deprecate(Some("new.action".to_string()));

        assert!(entry.deprecated);
        assert_eq!(entry.replacement_verb, Some("new.action".to_string()));
    }
}
