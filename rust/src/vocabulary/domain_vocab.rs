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

