//! Vocabulary registry for managing verb conflicts and ownership
//!
//! This module provides a centralized registry for tracking DSL verbs across
//! all domains, detecting conflicts, and managing verb ownership and sharing.
//!
//! ## Verb Naming Convention
//!
//! Verbs follow the domain-prefixed format: "domain.action"
//!
//! ### Examples:
//! - "kyc.declare-entity" - KYC domain entity declaration
//! - "kyc.obtain-document" - KYC domain document retrieval
//! - "compliance.generate-report" - Compliance domain reporting
//!
//! ### Benefits:
//! - Matches EDN/Clojure conventions (consistent with :keyword syntax)
//! - Simpler lookups (no string formatting required)
//! - Clear domain ownership
//! - Domain can be extracted from verb if needed

use crate::error::VocabularyError;
use crate::vocabulary::VerbRegistryEntry;
use std::collections::HashMap;

/// Central registry for all vocabulary verbs
pub struct VocabularyRegistry {
    /// Registry of all verbs with their metadata
    /// Key format: "domain.action" (e.g., "kyc.declare-entity")
    registry: HashMap<String, VerbRegistryEntry>,
    /// Domain ownership mapping: domain -> list of fully-qualified verbs
    domain_ownership: HashMap<String, Vec<String>>,
}

/// Registry configuration options
pub struct RegistryConfig {
    pub allow_shared_verbs: bool,
    pub enforce_domain_ownership: bool,
    pub auto_resolve_conflicts: bool,
    pub deprecation_policy: DeprecationPolicy,
}

/// Policy for handling deprecated verbs
#[derive(Debug, Clone)]
pub enum DeprecationPolicy {
    /// Immediately remove deprecated verbs
    Immediate,
    /// Grace period before removal
    GracePeriod(u64), // days
    /// Keep deprecated verbs but warn
    WarnOnly,
}

/// Registry statistics
#[derive(Debug, Clone)]
pub struct RegistryStats {
    pub total_verbs: usize,
    pub shared_verbs: usize,
    pub domains: usize,
    pub deprecated_verbs: usize,
}

impl Default for RegistryConfig {
    fn default() -> Self {
        Self {
            allow_shared_verbs: true,
            enforce_domain_ownership: true,
            auto_resolve_conflicts: false,
            deprecation_policy: DeprecationPolicy::GracePeriod(30),
        }
    }
}

impl VocabularyRegistry {
    /// Create a new vocabulary registry
    pub fn new() -> Self {
        Self {
            registry: HashMap::new(),
            domain_ownership: HashMap::new(),
        }
    }

    /// Validate verb format: must be "domain.action"
    ///
    /// # Examples
    /// ```
    /// use ob_poc::vocabulary::vocab_registry::VocabularyRegistry;
    ///
    /// let registry = VocabularyRegistry::new();
    /// assert!(VocabularyRegistry::validate_verb_format("kyc.declare-entity").is_ok());
    /// assert!(VocabularyRegistry::validate_verb_format("invalid-verb").is_err());
    /// ```
    pub fn validate_verb_format(verb: &str) -> Result<(String, String), VocabularyError> {
        let parts: Vec<&str> = verb.split('.').collect();

        if parts.len() != 2 {
            return Err(VocabularyError::InvalidSignature {
                verb: verb.to_string(),
                expected: "domain.action format".to_string(),
                found: format!("{} parts", parts.len()),
            });
        }

        let domain = parts[0];
        let action = parts[1];

        // Validate domain name (alphanumeric + hyphen)
        if domain.is_empty() || !domain.chars().all(|c| c.is_alphanumeric() || c == '-') {
            return Err(VocabularyError::InvalidSignature {
                verb: verb.to_string(),
                expected: "alphanumeric domain with hyphens".to_string(),
                found: format!("'{}'", domain),
            });
        }

        // Validate action name (alphanumeric + hyphen)
        if action.is_empty() || !action.chars().all(|c| c.is_alphanumeric() || c == '-') {
            return Err(VocabularyError::InvalidSignature {
                verb: verb.to_string(),
                expected: "alphanumeric action with hyphens".to_string(),
                found: format!("'{}'", action),
            });
        }

        Ok((domain.to_string(), action.to_string()))
    }

    /// Extract domain from a fully-qualified verb
    ///
    /// # Examples
    /// ```
    /// use ob_poc::vocabulary::vocab_registry::VocabularyRegistry;
    ///
    /// assert_eq!(VocabularyRegistry::extract_domain("kyc.declare-entity"), Some("kyc".to_string()));
    /// assert_eq!(VocabularyRegistry::extract_domain("invalid"), None);
    /// ```
    pub fn extract_domain(verb: &str) -> Option<String> {
        Self::validate_verb_format(verb)
            .map(|(domain, _)| domain)
            .ok()
    }

    /// Extract action from a fully-qualified verb
    pub fn extract_action(verb: &str) -> Option<String> {
        Self::validate_verb_format(verb)
            .map(|(_, action)| action)
            .ok()
    }

    /// Register a new verb in the registry
    pub fn register_verb(
        &mut self,
        verb: String,
        entry: VerbRegistryEntry,
    ) -> Result<(), VocabularyError> {
        // Validate verb format
        let (domain, _) = Self::validate_verb_format(&verb)?;

        // Check for conflicts
        if self.registry.contains_key(&verb) && !entry.shared {
            return Err(VocabularyError::UnknownVerb {
                verb: verb.clone(),
                domain: domain.clone(),
            });
        }

        // Register the verb
        self.registry.insert(verb.clone(), entry);

        // Update domain ownership
        self.domain_ownership
            .entry(domain)
            .or_default()
            .push(verb);

        Ok(())
    }

    /// Get all shared verbs (replaces the removed shared_verbs HashMap)
    pub fn get_shared_verbs(&self) -> Vec<&VerbRegistryEntry> {
        self.registry.values().filter(|e| e.shared).collect()
    }

    /// Get all verbs for a specific domain
    pub fn get_domain_verbs(&self, domain: &str) -> Vec<&String> {
        self.domain_ownership
            .get(domain)
            .map(|verbs| verbs.iter().collect())
            .unwrap_or_default()
    }

    /// Check if a verb is available for use
    pub fn is_verb_available(&self, verb: &str) -> bool {
        if let Some(entry) = self.registry.get(verb) {
            entry.shared || !entry.deprecated
        } else {
            true // Verb doesn't exist, so it's available
        }
    }

    /// Get verb entry by fully-qualified name
    pub fn get_verb(&self, verb: &str) -> Option<&VerbRegistryEntry> {
        self.registry.get(verb)
    }

    /// List all registered verbs
    pub fn list_verbs(&self) -> Vec<&String> {
        self.registry.keys().collect()
    }

    /// Get registry statistics
    pub fn get_stats(&self) -> RegistryStats {
        let total_verbs = self.registry.len();
        let shared_verbs = self.get_shared_verbs().len();
        let domains = self.domain_ownership.len();
        let deprecated_verbs = self.registry.values().filter(|e| e.deprecated).count();

        RegistryStats {
            total_verbs,
            shared_verbs,
            domains,
            deprecated_verbs,
        }
    }

    /// Get all domains
    pub fn get_domains(&self) -> Vec<&String> {
        self.domain_ownership.keys().collect()
    }

    /// Remove a verb from the registry
    pub fn remove_verb(&mut self, verb: &str) -> Result<VerbRegistryEntry, VocabularyError> {
        let entry = self
            .registry
            .remove(verb)
            .ok_or_else(|| VocabularyError::UnknownVerb {
                verb: verb.to_string(),
                domain: Self::extract_domain(verb).unwrap_or_default(),
            })?;

        // Remove from domain ownership
        if let Some(domain) = Self::extract_domain(verb) {
            if let Some(verbs) = self.domain_ownership.get_mut(&domain) {
                verbs.retain(|v| v != verb);
                if verbs.is_empty() {
                    self.domain_ownership.remove(&domain);
                }
            }
        }

        Ok(entry)
    }
}

impl Default for VocabularyRegistry {
    fn default() -> Self {
        Self::new()
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn create_test_entry(verb_name: &str, domain: &str, shared: bool) -> VerbRegistryEntry {
        VerbRegistryEntry {
            verb_name: verb_name.to_string(),
            domain: domain.to_string(),
            shared,
            deprecated: false,
            replacement_verb: None,
            description: None,
            version: "1.0".to_string(),
        }
    }

    #[test]
    fn test_validate_verb_format() {
        // Valid formats
        assert!(VocabularyRegistry::validate_verb_format("kyc.declare-entity").is_ok());
        assert!(VocabularyRegistry::validate_verb_format("compliance.generate-report").is_ok());

        // Invalid formats
        assert!(VocabularyRegistry::validate_verb_format("invalid").is_err());
        assert!(VocabularyRegistry::validate_verb_format("too.many.parts").is_err());
        assert!(VocabularyRegistry::validate_verb_format(".missing-domain").is_err());
        assert!(VocabularyRegistry::validate_verb_format("missing-action.").is_err());
    }

    #[test]
    fn test_extract_domain_and_action() {
        assert_eq!(
            VocabularyRegistry::extract_domain("kyc.declare-entity"),
            Some("kyc".to_string())
        );
        assert_eq!(
            VocabularyRegistry::extract_action("kyc.declare-entity"),
            Some("declare-entity".to_string())
        );
        assert_eq!(VocabularyRegistry::extract_domain("invalid"), None);
    }

    #[test]
    fn test_register_verb() {
        let mut registry = VocabularyRegistry::new();
        let entry = create_test_entry("kyc.declare-entity", "kyc", false);

        assert!(registry
            .register_verb("kyc.declare-entity".to_string(), entry)
            .is_ok());
        assert!(registry.get_verb("kyc.declare-entity").is_some());
        assert_eq!(registry.get_domain_verbs("kyc").len(), 1);
    }

    #[test]
    fn test_shared_verbs() {
        let mut registry = VocabularyRegistry::new();
        let shared_entry = create_test_entry("common.validate", "common", true);
        let private_entry = create_test_entry("kyc.private-action", "kyc", false);

        registry
            .register_verb("common.validate".to_string(), shared_entry)
            .unwrap();
        registry
            .register_verb("kyc.private-action".to_string(), private_entry)
            .unwrap();

        let shared_verbs = registry.get_shared_verbs();
        assert_eq!(shared_verbs.len(), 1);
        assert!(shared_verbs[0].shared);
    }

    #[test]
    fn test_domain_ownership() {
        let mut registry = VocabularyRegistry::new();
        let entry1 = create_test_entry("kyc.declare-entity", "kyc", false);
        let entry2 = create_test_entry("kyc.obtain-document", "kyc", false);
        let entry3 = create_test_entry("compliance.generate-report", "compliance", false);

        registry
            .register_verb("kyc.declare-entity".to_string(), entry1)
            .unwrap();
        registry
            .register_verb("kyc.obtain-document".to_string(), entry2)
            .unwrap();
        registry
            .register_verb("compliance.generate-report".to_string(), entry3)
            .unwrap();

        assert_eq!(registry.get_domain_verbs("kyc").len(), 2);
        assert_eq!(registry.get_domain_verbs("compliance").len(), 1);
        assert_eq!(registry.get_domains().len(), 2);
    }

    #[test]
    fn test_registry_stats() {
        let mut registry = VocabularyRegistry::new();
        let shared_entry = create_test_entry("common.validate", "common", true);
        let private_entry = create_test_entry("kyc.private-action", "kyc", false);

        registry
            .register_verb("common.validate".to_string(), shared_entry)
            .unwrap();
        registry
            .register_verb("kyc.private-action".to_string(), private_entry)
            .unwrap();

        let stats = registry.get_stats();
        assert_eq!(stats.total_verbs, 2);
        assert_eq!(stats.shared_verbs, 1);
        assert_eq!(stats.domains, 2);
    }

    #[test]
    fn test_remove_verb() {
        let mut registry = VocabularyRegistry::new();
        let entry = create_test_entry("kyc.declare-entity", "kyc", false);

        registry
            .register_verb("kyc.declare-entity".to_string(), entry)
            .unwrap();
        assert!(registry.get_verb("kyc.declare-entity").is_some());

        let removed = registry.remove_verb("kyc.declare-entity").unwrap();
        assert_eq!(removed.verb_name, "kyc.declare-entity");
        assert!(registry.get_verb("kyc.declare-entity").is_none());
        assert_eq!(registry.get_domain_verbs("kyc").len(), 0);
    }
}
