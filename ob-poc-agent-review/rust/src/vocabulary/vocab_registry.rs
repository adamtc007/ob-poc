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
    #[allow(dead_code)]
    registry: HashMap<String, VerbRegistryEntry>,
    /// Domain ownership mapping: domain -> list of fully-qualified verbs
    #[allow(dead_code)]
    domain_ownership: HashMap<String, Vec<String>>,
}

/// Registry configuration options
#[allow(dead_code)]
pub(crate) struct RegistryConfig {
    pub allow_shared_verbs: bool,
    pub enforce_domain_ownership: bool,
    pub auto_resolve_conflicts: bool,
    pub deprecation_policy: DeprecationPolicy,
}

/// Policy for handling deprecated verbs
#[derive(Debug, Clone)]
#[allow(dead_code)]
pub(crate) enum DeprecationPolicy {
    /// Immediately remove deprecated verbs
    Immediate,
    /// Grace period before removal
    GracePeriod(u64), // days
    /// Keep deprecated verbs but warn
    WarnOnly,
}

/// Registry statistics
#[derive(Debug, Clone)]
#[allow(dead_code)]
pub(crate) struct RegistryStats {
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
    #[allow(dead_code)]
    pub(crate) fn validate_verb_format(verb: &str) -> Result<(String, String), VocabularyError> {
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
    pub(crate) fn extract_domain(verb: &str) -> Option<String> {
        Self::validate_verb_format(verb)
            .map(|(domain, _)| domain)
            .ok()
    }

    /// Extract action from a fully-qualified verb
    #[allow(dead_code)]
    pub(crate) fn extract_action(verb: &str) -> Option<String> {
        Self::validate_verb_format(verb)
            .map(|(_, action)| action)
            .ok()
    }

    /// Register a new verb in the registry
    #[allow(dead_code)]
    pub(crate) fn register_verb(
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
        self.domain_ownership.entry(domain).or_default().push(verb);

        Ok(())
    }

    /// Get all shared verbs (replaces the removed shared_verbs HashMap)
    #[allow(dead_code)]
    pub(crate) fn get_shared_verbs(&self) -> Vec<&VerbRegistryEntry> {
        self.registry.values().filter(|e| e.shared).collect()
    }

    /// Get all verbs for a specific domain
    #[allow(dead_code)]
    pub(crate) fn get_domain_verbs(&self, domain: &str) -> Vec<&String> {
        self.domain_ownership
            .get(domain)
            .map(|verbs| verbs.iter().collect())
            .unwrap_or_default()
    }

    /// Get verb entry by fully-qualified name
    #[allow(dead_code)]
    pub(crate) fn get_verb(&self, verb: &str) -> Option<&VerbRegistryEntry> {
        self.registry.get(verb)
    }

    /// Get registry statistics
    #[allow(dead_code)]
    pub(crate) fn get_stats(&self) -> RegistryStats {
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
    #[allow(dead_code)]
    pub(crate) fn get_domains(&self) -> Vec<&String> {
        self.domain_ownership.keys().collect()
    }

    /// Remove a verb from the registry
    #[allow(dead_code)]
    pub(crate) fn remove_verb(&mut self, verb: &str) -> Result<VerbRegistryEntry, VocabularyError> {
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
