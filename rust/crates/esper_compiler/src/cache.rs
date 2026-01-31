//! Cache key generation and snapshot caching.

use crate::config::CompilerConfig;
use crate::input::GraphInput;
use serde::{Deserialize, Serialize};
use sha2::{Digest, Sha256};
use std::collections::HashMap;
use std::sync::{Arc, RwLock};

/// Content-addressed cache key for snapshots.
#[derive(Debug, Clone, PartialEq, Eq, Hash, Serialize, Deserialize)]
pub struct CacheKey {
    /// Hash of the graph data.
    pub graph_hash: String,
    /// Hash of the compiler config.
    pub config_hash: String,
    /// Combined hash.
    pub combined_hash: String,
}

impl CacheKey {
    /// Compute cache key for a graph and config.
    pub fn compute(input: &dyn GraphInput, config: &CompilerConfig) -> Self {
        let graph_hash = Self::hash_graph(input);
        let config_hash = Self::hash_config(config);
        let combined_hash = Self::combine_hashes(&graph_hash, &config_hash);

        Self {
            graph_hash,
            config_hash,
            combined_hash,
        }
    }

    /// Hash graph data.
    fn hash_graph(input: &dyn GraphInput) -> String {
        let mut hasher = Sha256::new();

        // Hash entity IDs (sorted for determinism)
        let mut ids = input.entity_ids();
        ids.sort();
        for id in ids {
            hasher.update(id.to_le_bytes());
            if let Some(entity) = input.get_entity(id) {
                hasher.update(entity.name.as_bytes());
                hasher.update(entity.kind_id.to_le_bytes());
            }
        }

        // Hash edges
        let mut edges = input.edges();
        edges.sort_by_key(|e| (e.from, e.to));
        for edge in edges {
            hasher.update(edge.from.to_le_bytes());
            hasher.update(edge.to.to_le_bytes());
        }

        hex::encode(hasher.finalize())
    }

    /// Hash compiler config.
    fn hash_config(config: &CompilerConfig) -> String {
        let mut hasher = Sha256::new();

        // Hash relevant config fields
        hasher.update(config.schema_version.to_le_bytes());
        hasher.update(config.layout.viewport_width.to_le_bytes());
        hasher.update(config.layout.viewport_height.to_le_bytes());
        hasher.update(config.layout.node_spacing.to_le_bytes());
        hasher.update(config.layout.level_spacing.to_le_bytes());
        hasher.update((config.layout.algorithm as u8).to_le_bytes());
        hasher.update(config.chamber.max_entities.to_le_bytes());
        hasher.update(config.chamber.grid_cell_size.to_le_bytes());

        hex::encode(hasher.finalize())
    }

    /// Combine two hashes.
    fn combine_hashes(a: &str, b: &str) -> String {
        let mut hasher = Sha256::new();
        hasher.update(a.as_bytes());
        hasher.update(b.as_bytes());
        hex::encode(hasher.finalize())
    }

    /// Get a short version of the combined hash (first 16 chars).
    pub fn short(&self) -> &str {
        &self.combined_hash[..16.min(self.combined_hash.len())]
    }
}

/// In-memory snapshot cache.
#[derive(Debug)]
pub struct SnapshotCache {
    /// Cached snapshots.
    cache: Arc<RwLock<HashMap<String, CachedSnapshot>>>,
    /// Maximum cache size.
    max_entries: usize,
}

/// Cached snapshot with metadata.
#[derive(Debug, Clone)]
pub struct CachedSnapshot {
    /// Serialized snapshot data.
    pub data: Vec<u8>,
    /// Cache key.
    pub key: CacheKey,
    /// Creation timestamp.
    pub created_at: u64,
    /// Last access timestamp.
    pub last_accessed: u64,
    /// Access count.
    pub access_count: u64,
}

impl SnapshotCache {
    /// Create a new cache with default capacity.
    pub fn new() -> Self {
        Self::with_capacity(100)
    }

    /// Create a cache with specified capacity.
    pub fn with_capacity(max_entries: usize) -> Self {
        Self {
            cache: Arc::new(RwLock::new(HashMap::new())),
            max_entries,
        }
    }

    /// Get a snapshot from cache.
    pub fn get(&self, key: &CacheKey) -> Option<CachedSnapshot> {
        let mut cache = self.cache.write().ok()?;
        let entry = cache.get_mut(&key.combined_hash)?;
        entry.last_accessed = current_timestamp();
        entry.access_count += 1;
        Some(entry.clone())
    }

    /// Insert a snapshot into cache.
    pub fn insert(&self, key: CacheKey, data: Vec<u8>) {
        let mut cache = match self.cache.write() {
            Ok(c) => c,
            Err(_) => return,
        };

        // Evict if at capacity
        if cache.len() >= self.max_entries {
            self.evict_lru(&mut cache);
        }

        let now = current_timestamp();
        cache.insert(
            key.combined_hash.clone(),
            CachedSnapshot {
                data,
                key,
                created_at: now,
                last_accessed: now,
                access_count: 1,
            },
        );
    }

    /// Check if cache contains a key.
    pub fn contains(&self, key: &CacheKey) -> bool {
        self.cache
            .read()
            .map(|c| c.contains_key(&key.combined_hash))
            .unwrap_or(false)
    }

    /// Remove a snapshot from cache.
    pub fn remove(&self, key: &CacheKey) -> Option<CachedSnapshot> {
        self.cache.write().ok()?.remove(&key.combined_hash)
    }

    /// Clear the cache.
    pub fn clear(&self) {
        if let Ok(mut cache) = self.cache.write() {
            cache.clear();
        }
    }

    /// Get cache statistics.
    pub fn stats(&self) -> CacheStats {
        let cache = self.cache.read().unwrap();
        CacheStats {
            entries: cache.len(),
            max_entries: self.max_entries,
            total_bytes: cache.values().map(|e| e.data.len()).sum(),
        }
    }

    /// Evict least recently used entry.
    fn evict_lru(&self, cache: &mut HashMap<String, CachedSnapshot>) {
        if let Some(key) = cache
            .iter()
            .min_by_key(|(_, v)| v.last_accessed)
            .map(|(k, _)| k.clone())
        {
            cache.remove(&key);
        }
    }
}

impl Default for SnapshotCache {
    fn default() -> Self {
        Self::new()
    }
}

/// Cache statistics.
#[derive(Debug, Clone)]
pub struct CacheStats {
    pub entries: usize,
    pub max_entries: usize,
    pub total_bytes: usize,
}

fn current_timestamp() -> u64 {
    std::time::SystemTime::now()
        .duration_since(std::time::UNIX_EPOCH)
        .map(|d| d.as_secs())
        .unwrap_or(0)
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::input::MemoryGraphInput;

    #[test]
    fn cache_key_deterministic() {
        let graph = MemoryGraphInput::simple_tree(vec![(1, "Root", 0), (2, "Child", 1)]);

        let config = CompilerConfig::default();

        let key1 = CacheKey::compute(&graph, &config);
        let key2 = CacheKey::compute(&graph, &config);

        assert_eq!(key1.combined_hash, key2.combined_hash);
    }

    #[test]
    fn cache_key_changes_with_data() {
        let graph1 = MemoryGraphInput::simple_tree(vec![(1, "Root", 0), (2, "Child", 1)]);

        let graph2 = MemoryGraphInput::simple_tree(vec![(1, "Root", 0), (2, "Different", 1)]);

        let config = CompilerConfig::default();

        let key1 = CacheKey::compute(&graph1, &config);
        let key2 = CacheKey::compute(&graph2, &config);

        assert_ne!(key1.combined_hash, key2.combined_hash);
    }

    #[test]
    fn snapshot_cache_basic() {
        let cache = SnapshotCache::with_capacity(2);

        let key1 = CacheKey {
            graph_hash: "a".to_string(),
            config_hash: "b".to_string(),
            combined_hash: "ab".to_string(),
        };

        let key2 = CacheKey {
            graph_hash: "c".to_string(),
            config_hash: "d".to_string(),
            combined_hash: "cd".to_string(),
        };

        cache.insert(key1.clone(), vec![1, 2, 3]);
        cache.insert(key2.clone(), vec![4, 5, 6]);

        assert!(cache.contains(&key1));
        assert!(cache.contains(&key2));

        let entry = cache.get(&key1).unwrap();
        assert_eq!(entry.data, vec![1, 2, 3]);
    }

    #[test]
    fn snapshot_cache_eviction() {
        let cache = SnapshotCache::with_capacity(2);

        for i in 0..5 {
            let key = CacheKey {
                graph_hash: format!("g{}", i),
                config_hash: format!("c{}", i),
                combined_hash: format!("gc{}", i),
            };
            cache.insert(key, vec![i as u8]);
        }

        // Should have evicted oldest entries
        assert!(cache.stats().entries <= 2);
    }
}
