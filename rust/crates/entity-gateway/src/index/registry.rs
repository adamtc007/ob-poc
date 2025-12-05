//! Index registry for managing multiple entity indexes
//!
//! The `IndexRegistry` maintains a mapping from entity nicknames
//! to their corresponding search indexes.

use std::collections::HashMap;
use std::sync::Arc;
use tokio::sync::RwLock;

use crate::config::EntityConfig;
use crate::index::traits::SearchIndex;

/// Registry of search indexes keyed by entity nickname
pub struct IndexRegistry {
    /// Map of nickname -> index implementation
    indexes: RwLock<HashMap<String, Arc<dyn SearchIndex>>>,
    /// Entity configurations (immutable after construction)
    configs: HashMap<String, EntityConfig>,
}

impl IndexRegistry {
    /// Create a new registry with the given entity configurations
    pub fn new(configs: HashMap<String, EntityConfig>) -> Self {
        Self {
            indexes: RwLock::new(HashMap::new()),
            configs,
        }
    }

    /// Get an index by nickname
    ///
    /// Returns None if the nickname is not registered.
    pub async fn get(&self, nickname: &str) -> Option<Arc<dyn SearchIndex>> {
        self.indexes.read().await.get(nickname).cloned()
    }

    /// Get the configuration for an entity by nickname
    pub fn get_config(&self, nickname: &str) -> Option<&EntityConfig> {
        self.configs.get(nickname)
    }

    /// Register an index for a nickname
    ///
    /// This will replace any existing index for the same nickname.
    pub async fn register(&self, nickname: String, index: Arc<dyn SearchIndex>) {
        self.indexes.write().await.insert(nickname, index);
    }

    /// Get all registered nicknames
    pub fn nicknames(&self) -> Vec<&str> {
        self.configs.keys().map(|s| s.as_str()).collect()
    }

    /// Get all entity configurations
    pub fn all_configs(&self) -> impl Iterator<Item = (&String, &EntityConfig)> {
        self.configs.iter()
    }

    /// Check if all indexes are ready
    pub async fn all_ready(&self) -> bool {
        let indexes = self.indexes.read().await;

        // All configured entities must have indexes and be ready
        for nickname in self.configs.keys() {
            match indexes.get(nickname) {
                Some(idx) if idx.is_ready() => continue,
                _ => return false,
            }
        }

        true
    }

    /// Get status of all indexes
    pub async fn status(&self) -> HashMap<String, bool> {
        let indexes = self.indexes.read().await;

        self.configs
            .keys()
            .map(|nickname| {
                let ready = indexes
                    .get(nickname)
                    .map(|idx| idx.is_ready())
                    .unwrap_or(false);
                (nickname.clone(), ready)
            })
            .collect()
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::config::{SearchKeyConfig, ShardConfig};

    fn sample_config() -> EntityConfig {
        EntityConfig {
            nickname: "test".to_string(),
            source_table: "test_table".to_string(),
            return_key: "id".to_string(),
            display_template: None,
            index_mode: crate::config::IndexMode::Trigram,
            filter: None,
            search_keys: vec![SearchKeyConfig {
                name: "name".to_string(),
                column: "name".to_string(),
                default: true,
            }],
            shard: ShardConfig {
                enabled: false,
                prefix_len: 0,
            },
        }
    }

    #[tokio::test]
    async fn test_registry_nicknames() {
        let mut configs = HashMap::new();
        configs.insert("person".to_string(), sample_config());
        configs.insert("fund".to_string(), sample_config());

        let registry = IndexRegistry::new(configs);
        let nicknames = registry.nicknames();

        assert_eq!(nicknames.len(), 2);
        assert!(nicknames.contains(&"person"));
        assert!(nicknames.contains(&"fund"));
    }

    #[tokio::test]
    async fn test_get_nonexistent() {
        let registry = IndexRegistry::new(HashMap::new());
        assert!(registry.get("nonexistent").await.is_none());
    }
}
