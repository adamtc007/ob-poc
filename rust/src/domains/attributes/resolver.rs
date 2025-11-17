//! Attribute UUID Resolution Service
//!
//! Phase 2: Runtime UUID Resolution
//!
//! This module provides bidirectional resolution between:
//! - UUID ↔ Semantic ID
//! - UUID ↔ AttributeMetadata
//!
//! Uses the Phase 0 UUID constants and provides caching for performance.

use super::types::AttributeMetadata;
use super::uuid_constants::build_uuid_map;
use std::collections::HashMap;
use uuid::Uuid;

/// Result type for resolution operations
pub type ResolutionResult<T> = Result<T, ResolutionError>;

/// Errors that can occur during resolution
#[derive(Debug, Clone, PartialEq)]
pub enum ResolutionError {
    /// UUID not found in registry
    UuidNotFound(Uuid),
    /// Semantic ID not found in registry
    SemanticIdNotFound(String),
    /// Attribute metadata not available
    MetadataNotAvailable(String),
}

impl std::fmt::Display for ResolutionError {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            ResolutionError::UuidNotFound(uuid) => {
                write!(f, "UUID not found in attribute registry: {}", uuid)
            }
            ResolutionError::SemanticIdNotFound(id) => {
                write!(f, "Semantic ID not found in attribute registry: {}", id)
            }
            ResolutionError::MetadataNotAvailable(id) => {
                write!(f, "Attribute metadata not available for: {}", id)
            }
        }
    }
}

impl std::error::Error for ResolutionError {}

/// Attribute UUID Resolver
///
/// Provides fast bidirectional resolution between UUIDs and semantic IDs,
/// with optional caching for performance.
#[derive(Clone)]
pub struct AttributeResolver {
    /// UUID → Semantic ID map (from Phase 0)
    uuid_map: HashMap<Uuid, String>,

    /// Semantic ID → UUID map (reverse of uuid_map)
    semantic_map: HashMap<String, Uuid>,

    /// Cache enabled flag
    cache_enabled: bool,
}

impl AttributeResolver {
    /// Create a new resolver with default settings (caching enabled)
    pub fn new() -> Self {
        let semantic_map = build_uuid_map(); // Returns HashMap<String, Uuid>

        // Build reverse map: UUID → Semantic ID
        let uuid_map: HashMap<Uuid, String> = semantic_map
            .iter()
            .map(|(semantic, uuid)| (*uuid, semantic.clone()))
            .collect();

        Self {
            uuid_map,
            semantic_map,
            cache_enabled: true,
        }
    }

    /// Create a resolver without caching (for testing)
    pub fn without_cache() -> Self {
        let mut resolver = Self::new();
        resolver.cache_enabled = false;
        resolver
    }

    /// Resolve UUID to semantic ID
    ///
    /// # Example
    /// ```ignore
    /// let resolver = AttributeResolver::new();
    /// let uuid = Uuid::parse_str("3020d46f-472c-5437-9647-1b0682c35935")?;
    /// let semantic_id = resolver.uuid_to_semantic(&uuid)?;
    /// assert_eq!(semantic_id, "attr.identity.first_name");
    /// ```
    pub fn uuid_to_semantic(&self, uuid: &Uuid) -> ResolutionResult<String> {
        self.uuid_map
            .get(uuid)
            .cloned()
            .ok_or(ResolutionError::UuidNotFound(*uuid))
    }

    /// Resolve semantic ID to UUID
    ///
    /// # Example
    /// ```ignore
    /// let resolver = AttributeResolver::new();
    /// let uuid = resolver.semantic_to_uuid("attr.identity.first_name")?;
    /// assert_eq!(uuid.to_string(), "3020d46f-472c-5437-9647-1b0682c35935");
    /// ```
    pub fn semantic_to_uuid(&self, semantic_id: &str) -> ResolutionResult<Uuid> {
        self.semantic_map
            .get(semantic_id)
            .copied()
            .ok_or_else(|| ResolutionError::SemanticIdNotFound(semantic_id.to_string()))
    }

    /// Resolve UUID to full AttributeMetadata
    ///
    /// This requires the attribute type to be registered. For now, this is a
    /// placeholder that will be enhanced in future phases.
    pub fn uuid_to_metadata(&self, uuid: &Uuid) -> ResolutionResult<AttributeMetadata> {
        let semantic_id = self.uuid_to_semantic(uuid)?;
        Err(ResolutionError::MetadataNotAvailable(semantic_id))
    }

    /// Check if a UUID is registered
    pub fn has_uuid(&self, uuid: &Uuid) -> bool {
        self.uuid_map.contains_key(uuid)
    }

    /// Check if a semantic ID is registered
    pub fn has_semantic_id(&self, semantic_id: &str) -> bool {
        self.semantic_map.contains_key(semantic_id)
    }

    /// Get count of registered attributes
    pub fn count(&self) -> usize {
        self.uuid_map.len()
    }

    /// Get all registered UUIDs
    pub fn all_uuids(&self) -> Vec<Uuid> {
        self.uuid_map.keys().copied().collect()
    }

    /// Get all registered semantic IDs
    pub fn all_semantic_ids(&self) -> Vec<String> {
        self.semantic_map.keys().cloned().collect()
    }
}

impl Default for AttributeResolver {
    fn default() -> Self {
        Self::new()
    }
}

