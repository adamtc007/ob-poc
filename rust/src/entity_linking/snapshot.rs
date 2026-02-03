//! Entity snapshot types for in-memory entity resolution
//!
//! The `EntitySnapshot` provides fast, in-memory entity lookup without
//! database access in the hot path. It's compiled from database tables
//! and serialized to disk using bincode.

use serde::{Deserialize, Serialize};
use smallvec::SmallVec;
use std::collections::HashMap;
use std::path::Path;
use uuid::Uuid;

/// Type alias for entity IDs
pub type EntityId = Uuid;

/// Snapshot format version - increment when struct layout changes
pub const SNAPSHOT_VERSION: u32 = 1;

/// In-memory entity snapshot for fast resolution
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct EntitySnapshot {
    /// Format version for bincode compatibility
    pub version: u32,

    /// Content-based hash for cache invalidation
    pub hash: String,

    /// All active entities
    pub entities: Vec<EntityRow>,

    /// Alias lookup: normalized alias → candidate entity IDs
    /// Bounded to 4 entities per alias to prevent pathological cases
    pub alias_index: HashMap<String, SmallVec<[EntityId; 4]>>,

    /// Canonical name lookup: normalized name → unique entity ID
    pub name_index: HashMap<String, EntityId>,

    /// Token overlap index: token → entity IDs
    /// Used for fuzzy matching when exact alias doesn't match
    pub token_index: HashMap<String, SmallVec<[EntityId; 8]>>,

    /// Concept links for disambiguation: entity → [(concept_id, weight)]
    pub concept_links: HashMap<EntityId, SmallVec<[(String, f32); 8]>>,

    /// Entity kind index: kind → entity IDs
    /// Used for kind constraint filtering
    pub kind_index: HashMap<String, SmallVec<[EntityId; 16]>>,
}

/// A single entity row in the snapshot
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct EntityRow {
    /// Entity UUID
    pub entity_id: EntityId,

    /// Entity type/kind (e.g., "limited_company", "proper_person", "fund")
    pub entity_kind: String,

    /// Display name (original casing)
    pub canonical_name: String,

    /// Normalized name for matching
    pub canonical_name_norm: String,
}

impl EntitySnapshot {
    /// Load snapshot from disk
    pub fn load(path: &Path) -> anyhow::Result<Self> {
        let bytes = std::fs::read(path)?;
        let snapshot: Self = bincode::deserialize(&bytes)?;

        // Version check
        if snapshot.version != SNAPSHOT_VERSION {
            anyhow::bail!(
                "Entity snapshot version mismatch: expected {}, got {}",
                SNAPSHOT_VERSION,
                snapshot.version
            );
        }

        Ok(snapshot)
    }

    /// Save snapshot to disk
    pub fn save(&self, path: &Path) -> anyhow::Result<()> {
        if let Some(parent) = path.parent() {
            std::fs::create_dir_all(parent)?;
        }
        std::fs::write(path, bincode::serialize(self)?)?;
        Ok(())
    }

    /// Get entity by ID
    pub fn get(&self, id: &EntityId) -> Option<&EntityRow> {
        self.entities.iter().find(|e| &e.entity_id == id)
    }

    /// Get entities by kind
    pub fn get_by_kind(&self, kind: &str) -> Vec<&EntityRow> {
        self.kind_index
            .get(kind)
            .map(|ids| ids.iter().filter_map(|id| self.get(id)).collect())
            .unwrap_or_default()
    }

    /// Lookup by exact normalized alias
    pub fn lookup_by_alias(&self, alias_norm: &str) -> Option<&SmallVec<[EntityId; 4]>> {
        self.alias_index.get(alias_norm)
    }

    /// Lookup by exact normalized canonical name
    pub fn lookup_by_name(&self, name_norm: &str) -> Option<EntityId> {
        self.name_index.get(name_norm).copied()
    }

    /// Lookup by token for overlap matching
    pub fn lookup_by_token(&self, token: &str) -> Option<&SmallVec<[EntityId; 8]>> {
        self.token_index.get(token)
    }

    /// Get concept links for an entity
    pub fn get_concepts(&self, entity_id: &EntityId) -> Option<&SmallVec<[(String, f32); 8]>> {
        self.concept_links.get(entity_id)
    }

    /// Check if entity has a specific concept
    pub fn has_concept(&self, entity_id: &EntityId, concept_id: &str) -> bool {
        self.concept_links
            .get(entity_id)
            .map(|links| links.iter().any(|(c, _)| c == concept_id))
            .unwrap_or(false)
    }

    /// Statistics for debugging
    pub fn stats(&self) -> SnapshotStats {
        SnapshotStats {
            version: self.version,
            hash: self.hash.clone(),
            entity_count: self.entities.len(),
            alias_index_size: self.alias_index.len(),
            name_index_size: self.name_index.len(),
            token_index_size: self.token_index.len(),
            concept_link_count: self.concept_links.values().map(|v| v.len()).sum(),
            entities_with_concepts: self.concept_links.len(),
            kind_count: self.kind_index.len(),
        }
    }
}

/// Snapshot statistics
#[derive(Debug, Clone)]
pub struct SnapshotStats {
    pub version: u32,
    pub hash: String,
    pub entity_count: usize,
    pub alias_index_size: usize,
    pub name_index_size: usize,
    pub token_index_size: usize,
    pub concept_link_count: usize,
    pub entities_with_concepts: usize,
    pub kind_count: usize,
}

impl std::fmt::Display for SnapshotStats {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        writeln!(f, "Entity Snapshot Statistics:")?;
        writeln!(f, "  Version: {}", self.version)?;
        writeln!(f, "  Hash: {}", &self.hash[..16])?;
        writeln!(f, "  Entities: {}", self.entity_count)?;
        writeln!(f, "  Alias index entries: {}", self.alias_index_size)?;
        writeln!(f, "  Name index entries: {}", self.name_index_size)?;
        writeln!(f, "  Token index entries: {}", self.token_index_size)?;
        writeln!(f, "  Concept links: {}", self.concept_link_count)?;
        writeln!(
            f,
            "  Entities with concepts: {}",
            self.entities_with_concepts
        )?;
        writeln!(f, "  Entity kinds: {}", self.kind_count)?;
        Ok(())
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use smallvec::smallvec;

    fn make_test_snapshot() -> EntitySnapshot {
        let entity_id = Uuid::new_v4();
        let mut alias_index = HashMap::new();
        alias_index.insert("apple".to_string(), smallvec![entity_id]);

        let mut name_index = HashMap::new();
        name_index.insert("apple inc".to_string(), entity_id);

        let mut token_index = HashMap::new();
        token_index.insert("apple".to_string(), smallvec![entity_id]);

        let mut kind_index: HashMap<String, SmallVec<[EntityId; 16]>> = HashMap::new();
        kind_index.insert("company".to_string(), smallvec![entity_id]);

        EntitySnapshot {
            version: SNAPSHOT_VERSION,
            hash: "test_hash".to_string(),
            entities: vec![EntityRow {
                entity_id,
                entity_kind: "company".to_string(),
                canonical_name: "Apple Inc.".to_string(),
                canonical_name_norm: "apple inc".to_string(),
            }],
            alias_index,
            name_index,
            token_index,
            concept_links: HashMap::new(),
            kind_index,
        }
    }

    #[test]
    fn test_lookup_by_alias() {
        let snapshot = make_test_snapshot();
        let ids = snapshot.lookup_by_alias("apple");
        assert!(ids.is_some());
        assert_eq!(ids.unwrap().len(), 1);
    }

    #[test]
    fn test_lookup_by_name() {
        let snapshot = make_test_snapshot();
        let id = snapshot.lookup_by_name("apple inc");
        assert!(id.is_some());
    }

    #[test]
    fn test_lookup_miss() {
        let snapshot = make_test_snapshot();
        let ids = snapshot.lookup_by_alias("microsoft");
        assert!(ids.is_none());
    }

    #[test]
    fn test_stats() {
        let snapshot = make_test_snapshot();
        let stats = snapshot.stats();
        assert_eq!(stats.entity_count, 1);
        assert_eq!(stats.alias_index_size, 1);
    }
}
