//! LexiconSnapshot - Compiled, immutable lexicon for fast lookup.
//!
//! The snapshot is built at compile time from YAML config files and loaded
//! as a binary blob at runtime. This ensures:
//! - No YAML parsing at query time
//! - No DB queries in hot path
//! - Fast HashMap-based lookups

use serde::{Deserialize, Serialize};
use smallvec::SmallVec;
use std::collections::HashMap;
use std::path::Path;

use super::types::*;

/// Compiled lexicon snapshot for in-memory lookup.
///
/// Loaded once at startup via `load_binary()`. The snapshot is immutable
/// and can be shared via `Arc<LexiconSnapshot>`.
#[derive(Debug, Serialize, Deserialize)]
pub struct LexiconSnapshot {
    /// Deterministic hash of input YAML files (SHA-256).
    /// Used for cache invalidation and version tracking.
    pub hash: String,

    /// Version string for compatibility checking
    pub version: String,

    // =========================================================================
    // Lexical Lane Indexes (Hot Path)
    // =========================================================================
    /// Normalized label → concept IDs that match exactly.
    /// Key: lowercase, whitespace-collapsed label
    /// Value: concept IDs like "verb.cbu.create", "entity_type.fund"
    ///
    /// Bounded to max 4 concepts per label to avoid explosion.
    pub label_to_concepts: HashMap<LabelNorm, SmallVec<[ConceptId; 4]>>,

    /// Single token → concept IDs that contain this token.
    /// Used for partial/token overlap matching.
    ///
    /// Bounded to max 8 concepts per token.
    pub token_to_concepts: HashMap<String, SmallVec<[ConceptId; 8]>>,

    // =========================================================================
    // Metadata (for enrichment after match)
    // =========================================================================
    /// Verb metadata keyed by DSL verb name (e.g., "cbu.create")
    pub verb_meta: HashMap<String, VerbMeta>,

    /// Entity type metadata keyed by type name (e.g., "fund")
    pub entity_types: HashMap<String, EntityTypeMeta>,

    /// Domain metadata keyed by domain ID (e.g., "cbu", "session")
    pub domains: HashMap<String, DomainMeta>,

    /// Keyword → domain ID for domain inference.
    /// E.g., "kyc" → "kyc", "fund" → "cbu"
    pub keyword_to_domain: HashMap<String, String>,
}

impl Default for LexiconSnapshot {
    fn default() -> Self {
        Self::empty()
    }
}

impl LexiconSnapshot {
    /// Create an empty snapshot (for testing or when YAML is missing)
    pub fn empty() -> Self {
        Self {
            hash: "empty".to_string(),
            version: "1.0.0".to_string(),
            label_to_concepts: HashMap::new(),
            token_to_concepts: HashMap::new(),
            verb_meta: HashMap::new(),
            entity_types: HashMap::new(),
            domains: HashMap::new(),
            keyword_to_domain: HashMap::new(),
        }
    }

    /// Load snapshot from binary file (bincode format).
    ///
    /// This is the hot path entry point - should be fast.
    pub fn load_binary(path: &Path) -> anyhow::Result<Self> {
        let bytes = std::fs::read(path).map_err(|e| {
            anyhow::anyhow!(
                "Failed to read lexicon snapshot from {}: {}",
                path.display(),
                e
            )
        })?;

        let snapshot: Self = bincode::deserialize(&bytes).map_err(|e| {
            anyhow::anyhow!(
                "Failed to deserialize lexicon snapshot from {}: {}",
                path.display(),
                e
            )
        })?;

        Ok(snapshot)
    }

    /// Save snapshot to binary file (bincode format).
    ///
    /// Used by the compiler to produce the artifact.
    pub fn save_binary(&self, path: &Path) -> anyhow::Result<()> {
        // Ensure parent directory exists
        if let Some(parent) = path.parent() {
            std::fs::create_dir_all(parent)?;
        }

        let bytes = bincode::serialize(self).map_err(|e| {
            anyhow::anyhow!(
                "Failed to serialize lexicon snapshot to {}: {}",
                path.display(),
                e
            )
        })?;

        std::fs::write(path, bytes).map_err(|e| {
            anyhow::anyhow!(
                "Failed to write lexicon snapshot to {}: {}",
                path.display(),
                e
            )
        })?;

        Ok(())
    }

    // =========================================================================
    // Query Methods (Used by LexiconServiceImpl)
    // =========================================================================

    /// Get all concept IDs that match a normalized label exactly.
    pub fn get_concepts_for_label(&self, label_norm: &str) -> Option<&SmallVec<[ConceptId; 4]>> {
        self.label_to_concepts.get(label_norm)
    }

    /// Get all concept IDs that contain a given token.
    pub fn get_concepts_for_token(&self, token: &str) -> Option<&SmallVec<[ConceptId; 8]>> {
        self.token_to_concepts.get(token)
    }

    /// Get verb metadata by DSL verb name.
    pub fn get_verb_meta(&self, dsl_verb: &str) -> Option<&VerbMeta> {
        self.verb_meta.get(dsl_verb)
    }

    /// Get entity type metadata by type name.
    pub fn get_entity_type_meta(&self, type_name: &str) -> Option<&EntityTypeMeta> {
        self.entity_types.get(type_name)
    }

    /// Get domain for a keyword.
    pub fn get_domain_for_keyword(&self, keyword: &str) -> Option<&String> {
        self.keyword_to_domain.get(keyword)
    }

    // =========================================================================
    // Statistics (for debugging/logging)
    // =========================================================================

    /// Get statistics about this snapshot.
    pub fn stats(&self) -> SnapshotStats {
        SnapshotStats {
            hash: self.hash.clone(),
            version: self.version.clone(),
            verb_count: self.verb_meta.len(),
            entity_type_count: self.entity_types.len(),
            domain_count: self.domains.len(),
            label_index_size: self.label_to_concepts.len(),
            token_index_size: self.token_to_concepts.len(),
            keyword_count: self.keyword_to_domain.len(),
        }
    }
}

/// Statistics about a lexicon snapshot.
#[derive(Debug, Clone)]
pub struct SnapshotStats {
    pub hash: String,
    pub version: String,
    pub verb_count: usize,
    pub entity_type_count: usize,
    pub domain_count: usize,
    pub label_index_size: usize,
    pub token_index_size: usize,
    pub keyword_count: usize,
}

impl std::fmt::Display for SnapshotStats {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        writeln!(f, "Lexicon Snapshot Statistics:")?;
        writeln!(f, "  Hash: {}", self.hash)?;
        writeln!(f, "  Version: {}", self.version)?;
        writeln!(f, "  Verbs: {}", self.verb_count)?;
        writeln!(f, "  Entity types: {}", self.entity_type_count)?;
        writeln!(f, "  Domains: {}", self.domain_count)?;
        writeln!(f, "  Label index entries: {}", self.label_index_size)?;
        writeln!(f, "  Token index entries: {}", self.token_index_size)?;
        writeln!(f, "  Domain keywords: {}", self.keyword_count)?;
        Ok(())
    }
}

// =============================================================================
// Tests
// =============================================================================

#[cfg(test)]
mod tests {
    use super::*;
    use tempfile::tempdir;

    #[test]
    fn test_empty_snapshot() {
        let snapshot = LexiconSnapshot::empty();
        assert_eq!(snapshot.hash, "empty");
        assert!(snapshot.verb_meta.is_empty());
        assert!(snapshot.label_to_concepts.is_empty());
    }

    #[test]
    fn test_save_and_load_binary() {
        let mut snapshot = LexiconSnapshot::empty();
        snapshot.hash = "test_hash_123".to_string();
        snapshot.verb_meta.insert(
            "cbu.create".to_string(),
            VerbMeta {
                dsl_verb: "cbu.create".to_string(),
                pref_label: "Create CBU".to_string(),
                domain: Some("cbu".to_string()),
                ..Default::default()
            },
        );

        let dir = tempdir().unwrap();
        let path = dir.path().join("test_snapshot.bin");

        // Save
        snapshot.save_binary(&path).unwrap();
        assert!(path.exists());

        // Load
        let loaded = LexiconSnapshot::load_binary(&path).unwrap();
        assert_eq!(loaded.hash, "test_hash_123");
        assert!(loaded.verb_meta.contains_key("cbu.create"));
    }

    #[test]
    fn test_stats() {
        let mut snapshot = LexiconSnapshot::empty();
        snapshot
            .verb_meta
            .insert("v1".to_string(), VerbMeta::default());
        snapshot
            .verb_meta
            .insert("v2".to_string(), VerbMeta::default());
        snapshot
            .entity_types
            .insert("e1".to_string(), EntityTypeMeta::default());

        let stats = snapshot.stats();
        assert_eq!(stats.verb_count, 2);
        assert_eq!(stats.entity_type_count, 1);
    }
}
