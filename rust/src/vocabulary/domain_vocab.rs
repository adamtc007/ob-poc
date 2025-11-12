//! Domain-specific vocabulary management
//!
//! This module provides management capabilities for domain-specific DSL vocabularies,
//! allowing different domains to maintain their own verb sets while ensuring
//! consistency and avoiding conflicts.

use crate::ast::types::VocabularyVerb;
use crate::vocabulary::VocabularyError;
use std::collections::HashMap;
use uuid::Uuid;

/// Domain-specific vocabulary manager
pub(crate) struct DomainVocabularyManager {
    domain_vocabularies: HashMap<String, Vec<VocabularyVerb>>,
    domain_metadata: HashMap<String, DomainMetadata>,
}

/// Metadata for a domain
#[derive(Debug, Clone)]
pub(crate) struct DomainMetadata {
    pub domain_name: String,
    pub description: Option<String>,
    pub owner: Option<String>,
    pub version: String,
    pub deprecated: bool,
    pub replacement_domain: Option<String>,
}

/// Domain vocabulary configuration
pub(crate) struct DomainConfig {
    pub allow_cross_domain_references: bool,
    pub enforce_naming_conventions: bool,
    pub auto_resolve_conflicts: bool,
}

impl Default for DomainConfig {
    fn default() -> Self {
        Self {
            allow_cross_domain_references: true,
            enforce_naming_conventions: true,
            auto_resolve_conflicts: false,
        }
    }
}

impl DomainVocabularyManager {
    /// Create a new domain vocabulary manager
    pub fn new() -> Self {
        Self {
            domain_vocabularies: HashMap::new(),
            domain_metadata: HashMap::new(),
        }
    }

    /// Register a new domain
    pub fn register_domain(
        &mut self,
        domain_name: String,
        metadata: DomainMetadata,
    ) -> Result<(), VocabularyError> {
        if self.domain_metadata.contains_key(&domain_name) {
            return Err(VocabularyError::InvalidDefinition(format!(
                "Domain '{}' already exists",
                domain_name
            )));
        }

        self.domain_metadata.insert(domain_name.clone(), metadata);
        self.domain_vocabularies
            .insert(domain_name.clone(), Vec::new());

        tracing::info!("Registered domain: {}", domain_name);
        Ok(())
    }

    /// Add vocabulary to domain
    pub(crate) fn add_vocabulary_to_domain(
        &mut self,
        domain_name: &str,
        vocabulary: VocabularyVerb,
    ) -> Result<(), VocabularyError> {
        let domain_vocabs = self
            .domain_vocabularies
            .get_mut(domain_name)
            .ok_or_else(|| VocabularyError::DomainNotFound(domain_name.to_string()))?;

        // Check for duplicates
        if domain_vocabs.iter().any(|v| v.verb == vocabulary.verb) {
            return Err(VocabularyError::VerbConflict {
                verb: vocabulary.verb.clone(),
                domain: domain_name.to_string(),
            });
        }

        domain_vocabs.push(vocabulary);
        Ok(())
    }

    /// Get vocabulary for domain
    pub(crate) fn get_domain_vocabulary(&self, domain_name: &str) -> Option<&Vec<VocabularyVerb>> {
        self.domain_vocabularies.get(domain_name)
    }

    /// List all domains
    pub fn list_domains(&self) -> Vec<&String> {
        self.domain_metadata.keys().collect()
    }

    /// Get domain metadata
    pub(crate) fn get_domain_metadata(&self, domain_name: &str) -> Option<&DomainMetadata> {
        self.domain_metadata.get(domain_name)
    }

    /// Remove domain
    pub(crate) fn remove_domain(&mut self, domain_name: &str) -> Result<(), VocabularyError> {
        if !self.domain_metadata.contains_key(domain_name) {
            return Err(VocabularyError::DomainNotFound(domain_name.to_string()));
        }

        self.domain_metadata.remove(domain_name);
        self.domain_vocabularies.remove(domain_name);

        tracing::info!("Removed domain: {}", domain_name);
        Ok(())
    }

    /// Check for verb conflicts across domains
    pub(crate) fn check_cross_domain_conflicts(&self) -> Vec<ConflictReport> {
        let mut conflicts = Vec::new();
        let mut verb_domains: HashMap<String, Vec<String>> = HashMap::new();

        // Collect all verbs and their domains
        for (domain, vocabs) in &self.domain_vocabularies {
            for vocab in vocabs {
                verb_domains
                    .entry(vocab.verb.clone())
                    .or_insert_with(Vec::new)
                    .push(domain.clone());
            }
        }

        // Find conflicts
        for (verb, domains) in verb_domains {
            if domains.len() > 1 {
                conflicts.push(ConflictReport {
                    verb,
                    conflicting_domains: domains,
                    conflict_type: ConflictType::CrossDomain,
                });
            }
        }

        conflicts
    }

    /// Merge domains
    pub(crate) fn merge_domains(
        &mut self,
        source_domain: &str,
        target_domain: &str,
    ) -> Result<(), VocabularyError> {
        let source_vocabs = self
            .domain_vocabularies
            .remove(source_domain)
            .ok_or_else(|| VocabularyError::DomainNotFound(source_domain.to_string()))?;

        let target_vocabs = self
            .domain_vocabularies
            .get_mut(target_domain)
            .ok_or_else(|| VocabularyError::DomainNotFound(target_domain.to_string()))?;

        // Check for conflicts before merging
        for source_vocab in &source_vocabs {
            if target_vocabs.iter().any(|v| v.verb == source_vocab.verb) {
                // Restore source domain
                self.domain_vocabularies
                    .insert(source_domain.to_string(), source_vocabs);
                return Err(VocabularyError::VerbConflict {
                    verb: source_vocab.verb.clone(),
                    domain: target_domain.to_string(),
                });
            }
        }

        // Perform merge
        target_vocabs.extend(source_vocabs);
        self.domain_metadata.remove(source_domain);

        tracing::info!("Merged domain '{}' into '{}'", source_domain, target_domain);
        Ok(())
    }

    /// Get statistics for all domains
    pub(crate) fn get_domain_statistics(&self) -> HashMap<String, DomainStats> {
        let mut stats = HashMap::new();

        for (domain, vocabs) in &self.domain_vocabularies {
            let active_count = vocabs.iter().filter(|v| v.active).count();
            let deprecated_count = vocabs.len() - active_count;

            let mut category_counts = HashMap::new();
            for vocab in vocabs {
                if let Some(ref category) = vocab.category {
                    *category_counts.entry(category.clone()).or_insert(0) += 1;
                }
            }

            stats.insert(
                domain.clone(),
                DomainStats {
                    total_verbs: vocabs.len(),
                    active_verbs: active_count,
                    deprecated_verbs: deprecated_count,
                    categories: category_counts,
                },
            );
        }

        stats
    }
}

/// Conflict report for cross-domain verb conflicts
#[derive(Debug, Clone)]
pub(crate) struct ConflictReport {
    pub verb: String,
    pub conflicting_domains: Vec<String>,
    pub conflict_type: ConflictType,
}

#[derive(Debug, Clone)]
pub(crate) enum ConflictType {
    CrossDomain,
    Duplicate,
    Deprecated,
}

/// Domain statistics
#[derive(Debug, Clone)]
pub(crate) struct DomainStats {
    pub total_verbs: usize,
    pub active_verbs: usize,
    pub deprecated_verbs: usize,
    pub categories: HashMap<String, usize>,
}

impl Default for DomainVocabularyManager {
    fn default() -> Self {
        Self::new()
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use chrono::Utc;

    fn create_test_vocabulary(domain: &str, verb: &str) -> VocabularyVerb {
        VocabularyVerb {
            vocab_id: Uuid::new_v4(),
            domain: domain.to_string(),
            verb: verb.to_string(),
            category: Some("test".to_string()),
            description: Some("Test vocabulary".to_string()),
            parameters: None,
            examples: None,
            phase: Some("1".to_string()),
            active: true,
            version: "1.0.0".to_string(),
            created_at: Utc::now(),
            updated_at: Utc::now(),
        }
    }

    fn create_test_metadata(name: &str) -> DomainMetadata {
        DomainMetadata {
            domain_name: name.to_string(),
            description: Some(format!("Test domain {}", name)),
            owner: Some("test".to_string()),
            version: "1.0.0".to_string(),
            deprecated: false,
            replacement_domain: None,
        }
    }

    #[test]
    fn test_register_domain() {
        let mut manager = DomainVocabularyManager::new();
        let metadata = create_test_metadata("test");

        let result = manager.register_domain("test".to_string(), metadata);
        assert!(result.is_ok());

        assert!(manager.domain_metadata.contains_key("test"));
        assert!(manager.domain_vocabularies.contains_key("test"));
    }

    #[test]
    fn test_duplicate_domain_registration() {
        let mut manager = DomainVocabularyManager::new();
        let metadata1 = create_test_metadata("test");
        let metadata2 = create_test_metadata("test");

        assert!(manager
            .register_domain("test".to_string(), metadata1)
            .is_ok());
        assert!(manager
            .register_domain("test".to_string(), metadata2)
            .is_err());
    }

    #[test]
    fn test_add_vocabulary_to_domain() {
        let mut manager = DomainVocabularyManager::new();
        let metadata = create_test_metadata("test");
        let vocabulary = create_test_vocabulary("test", "test.action");

        manager
            .register_domain("test".to_string(), metadata)
            .unwrap();
        let result = manager.add_vocabulary_to_domain("test", vocabulary);
        assert!(result.is_ok());

        let domain_vocabs = manager.get_domain_vocabulary("test").unwrap();
        assert_eq!(domain_vocabs.len(), 1);
        assert_eq!(domain_vocabs[0].verb, "test.action");
    }

    #[test]
    fn test_verb_conflict_detection() {
        let mut manager = DomainVocabularyManager::new();

        // Register two domains
        manager
            .register_domain("domain1".to_string(), create_test_metadata("domain1"))
            .unwrap();
        manager
            .register_domain("domain2".to_string(), create_test_metadata("domain2"))
            .unwrap();

        // Add same verb to both domains
        let vocab1 = create_test_vocabulary("domain1", "shared.verb");
        let vocab2 = create_test_vocabulary("domain2", "shared.verb");

        manager.add_vocabulary_to_domain("domain1", vocab1).unwrap();
        manager.add_vocabulary_to_domain("domain2", vocab2).unwrap();

        let conflicts = manager.check_cross_domain_conflicts();
        assert_eq!(conflicts.len(), 1);
        assert_eq!(conflicts[0].verb, "shared.verb");
        assert_eq!(conflicts[0].conflicting_domains.len(), 2);
    }

    #[test]
    fn test_domain_merge() {
        let mut manager = DomainVocabularyManager::new();

        // Register domains
        manager
            .register_domain("source".to_string(), create_test_metadata("source"))
            .unwrap();
        manager
            .register_domain("target".to_string(), create_test_metadata("target"))
            .unwrap();

        // Add vocabulary to source
        let vocab = create_test_vocabulary("source", "source.action");
        manager.add_vocabulary_to_domain("source", vocab).unwrap();

        // Merge domains
        let result = manager.merge_domains("source", "target");
        assert!(result.is_ok());

        // Check that source domain is removed and target has the vocabulary
        assert!(!manager.domain_metadata.contains_key("source"));
        let target_vocabs = manager.get_domain_vocabulary("target").unwrap();
        assert_eq!(target_vocabs.len(), 1);
        assert_eq!(target_vocabs[0].verb, "source.action");
    }

    #[test]
    fn test_domain_statistics() {
        let mut manager = DomainVocabularyManager::new();
        manager
            .register_domain("test".to_string(), create_test_metadata("test"))
            .unwrap();

        // Add various vocabularies
        let vocab1 = create_test_vocabulary("test", "test.action1");
        let mut vocab2 = create_test_vocabulary("test", "test.action2");
        vocab2.active = false; // Make it deprecated

        manager.add_vocabulary_to_domain("test", vocab1).unwrap();
        manager.add_vocabulary_to_domain("test", vocab2).unwrap();

        let stats = manager.get_domain_statistics();
        let test_stats = stats.get("test").unwrap();

        assert_eq!(test_stats.total_verbs, 2);
        assert_eq!(test_stats.active_verbs, 1);
        assert_eq!(test_stats.deprecated_verbs, 1);
    }
}
