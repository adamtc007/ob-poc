//! Source Registry - manages available source loaders
//!
//! The registry provides lookup by source ID, jurisdiction, and data type.

use super::traits::{SourceDataType, SourceLoader};
use serde::{Deserialize, Serialize};
use std::collections::HashMap;
use std::sync::Arc;

/// Registry of available source loaders
pub struct SourceRegistry {
    loaders: HashMap<String, Arc<dyn SourceLoader>>,
}

impl SourceRegistry {
    /// Create a new empty registry
    pub fn new() -> Self {
        Self {
            loaders: HashMap::new(),
        }
    }

    /// Register a source loader
    pub fn register(&mut self, loader: Arc<dyn SourceLoader>) {
        self.loaders.insert(loader.source_id().to_string(), loader);
    }

    /// Get a source loader by ID
    pub fn get(&self, source_id: &str) -> Option<Arc<dyn SourceLoader>> {
        self.loaders.get(source_id).cloned()
    }

    /// Find sources that cover a jurisdiction and provide a data type
    ///
    /// # Arguments
    ///
    /// * `jurisdiction` - ISO 3166-1 alpha-2 code
    /// * `data_type` - Type of data needed
    ///
    /// # Returns
    ///
    /// Sources ordered by specificity (jurisdiction-specific before global)
    pub fn find_for_jurisdiction(
        &self,
        jurisdiction: &str,
        data_type: SourceDataType,
    ) -> Vec<Arc<dyn SourceLoader>> {
        let mut specific = Vec::new();
        let mut global = Vec::new();

        for loader in self.loaders.values() {
            if !loader.provides().contains(&data_type) {
                continue;
            }

            let jurisdictions = loader.jurisdictions();
            if jurisdictions.contains(&jurisdiction) {
                specific.push(loader.clone());
            } else if jurisdictions.contains(&"*") {
                global.push(loader.clone());
            }
        }

        // Specific jurisdictions first, then global sources
        specific.extend(global);
        specific
    }

    /// Find the best source for a jurisdiction and data type
    pub fn find_best(
        &self,
        jurisdiction: &str,
        data_type: SourceDataType,
    ) -> Option<Arc<dyn SourceLoader>> {
        self.find_for_jurisdiction(jurisdiction, data_type)
            .into_iter()
            .next()
    }

    /// List all registered sources
    pub fn list(&self) -> Vec<SourceInfo> {
        self.loaders
            .values()
            .map(|l| SourceInfo {
                id: l.source_id().to_string(),
                name: l.source_name().to_string(),
                jurisdictions: l.jurisdictions().iter().map(|s| s.to_string()).collect(),
                provides: l.provides().to_vec(),
                key_type: l.key_type().to_string(),
            })
            .collect()
    }

    /// Get all source IDs
    pub fn source_ids(&self) -> Vec<String> {
        self.loaders.keys().cloned().collect()
    }

    /// Check if a source is registered
    pub fn contains(&self, source_id: &str) -> bool {
        self.loaders.contains_key(source_id)
    }

    /// Number of registered sources
    pub fn len(&self) -> usize {
        self.loaders.len()
    }

    /// Check if registry is empty
    pub fn is_empty(&self) -> bool {
        self.loaders.is_empty()
    }
}

impl Default for SourceRegistry {
    fn default() -> Self {
        Self::new()
    }
}

/// Information about a registered source
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct SourceInfo {
    /// Unique source identifier
    pub id: String,
    /// Human-readable name
    pub name: String,
    /// Covered jurisdictions
    pub jurisdictions: Vec<String>,
    /// Provided data types
    pub provides: Vec<SourceDataType>,
    /// Primary key type name
    pub key_type: String,
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::research::sources::normalized::{
        NormalizedControlHolder, NormalizedEntity, NormalizedOfficer, NormalizedRelationship,
    };
    use crate::research::sources::traits::{
        FetchControlHoldersOptions, FetchOfficersOptions, FetchOptions, FetchParentChainOptions,
        SearchCandidate, SearchOptions,
    };
    use anyhow::Result;
    use async_trait::async_trait;

    /// Mock source for testing
    struct MockSource {
        id: &'static str,
        jurisdictions: Vec<&'static str>,
        provides: Vec<SourceDataType>,
    }

    #[async_trait]
    impl SourceLoader for MockSource {
        fn source_id(&self) -> &'static str {
            self.id
        }

        fn source_name(&self) -> &'static str {
            "Mock Source"
        }

        fn jurisdictions(&self) -> &[&'static str] {
            &self.jurisdictions
        }

        fn provides(&self) -> &[SourceDataType] {
            &self.provides
        }

        fn key_type(&self) -> &'static str {
            "MOCK_KEY"
        }

        fn validate_key(&self, _key: &str) -> bool {
            true
        }

        async fn search(
            &self,
            _query: &str,
            _options: Option<SearchOptions>,
        ) -> Result<Vec<SearchCandidate>> {
            Ok(vec![])
        }

        async fn fetch_entity(
            &self,
            _key: &str,
            _options: Option<FetchOptions>,
        ) -> Result<NormalizedEntity> {
            Ok(NormalizedEntity::new(
                "key".into(),
                "mock".into(),
                "Test".into(),
            ))
        }

        async fn fetch_control_holders(
            &self,
            _key: &str,
            _options: Option<FetchControlHoldersOptions>,
        ) -> Result<Vec<NormalizedControlHolder>> {
            Ok(vec![])
        }

        async fn fetch_officers(
            &self,
            _key: &str,
            _options: Option<FetchOfficersOptions>,
        ) -> Result<Vec<NormalizedOfficer>> {
            Ok(vec![])
        }

        async fn fetch_parent_chain(
            &self,
            _key: &str,
            _options: Option<FetchParentChainOptions>,
        ) -> Result<Vec<NormalizedRelationship>> {
            Ok(vec![])
        }
    }

    #[test]
    fn test_registry_register_and_get() {
        let mut registry = SourceRegistry::new();

        let source = Arc::new(MockSource {
            id: "test-source",
            jurisdictions: vec!["GB"],
            provides: vec![SourceDataType::Entity],
        });

        registry.register(source);

        assert!(registry.contains("test-source"));
        assert!(!registry.contains("unknown"));
        assert_eq!(registry.len(), 1);

        let retrieved = registry.get("test-source");
        assert!(retrieved.is_some());
        assert_eq!(retrieved.unwrap().source_id(), "test-source");
    }

    #[test]
    fn test_registry_find_for_jurisdiction() {
        let mut registry = SourceRegistry::new();

        // UK-specific source
        registry.register(Arc::new(MockSource {
            id: "uk-source",
            jurisdictions: vec!["GB"],
            provides: vec![SourceDataType::Entity, SourceDataType::ControlHolders],
        }));

        // Global source
        registry.register(Arc::new(MockSource {
            id: "global-source",
            jurisdictions: vec!["*"],
            provides: vec![SourceDataType::Entity, SourceDataType::ParentChain],
        }));

        // US-specific source
        registry.register(Arc::new(MockSource {
            id: "us-source",
            jurisdictions: vec!["US"],
            provides: vec![SourceDataType::Entity, SourceDataType::ControlHolders],
        }));

        // Find for GB + Entity
        let gb_entity = registry.find_for_jurisdiction("GB", SourceDataType::Entity);
        assert_eq!(gb_entity.len(), 2);
        assert_eq!(gb_entity[0].source_id(), "uk-source"); // Specific first
        assert_eq!(gb_entity[1].source_id(), "global-source"); // Global second

        // Find for GB + ControlHolders
        let gb_psc = registry.find_for_jurisdiction("GB", SourceDataType::ControlHolders);
        assert_eq!(gb_psc.len(), 1);
        assert_eq!(gb_psc[0].source_id(), "uk-source");

        // Find for DE + Entity (only global matches)
        let de_entity = registry.find_for_jurisdiction("DE", SourceDataType::Entity);
        assert_eq!(de_entity.len(), 1);
        assert_eq!(de_entity[0].source_id(), "global-source");

        // Find for US + ControlHolders
        let us_holders = registry.find_for_jurisdiction("US", SourceDataType::ControlHolders);
        assert_eq!(us_holders.len(), 1);
        assert_eq!(us_holders[0].source_id(), "us-source");
    }

    #[test]
    fn test_registry_list() {
        let mut registry = SourceRegistry::new();

        registry.register(Arc::new(MockSource {
            id: "source-a",
            jurisdictions: vec!["GB"],
            provides: vec![SourceDataType::Entity],
        }));

        registry.register(Arc::new(MockSource {
            id: "source-b",
            jurisdictions: vec!["*"],
            provides: vec![SourceDataType::ParentChain],
        }));

        let list = registry.list();
        assert_eq!(list.len(), 2);

        let ids: Vec<_> = list.iter().map(|s| s.id.as_str()).collect();
        assert!(ids.contains(&"source-a"));
        assert!(ids.contains(&"source-b"));
    }
}
