//! Attribute UUID Resolution Service
//!
//! Phase 2: Runtime UUID Resolution
//!
//! This module provides bidirectional resolution between:
//! - UUID ↔ Semantic ID
//! - UUID ↔ AttributeMetadata
//!
//! Uses the Phase 0 UUID constants and provides caching for performance.

use super::types::{AttributeMetadata, AttributeType};
use super::uuid_constants::{build_uuid_map, uuid_to_semantic};
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
            .ok_or_else(|| ResolutionError::UuidNotFound(*uuid))
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

#[cfg(test)]
mod tests {
    use super::*;
    use crate::domains::attributes::kyc::{Email, FirstName, LastName};

    #[test]
    fn test_resolver_creation() {
        let resolver = AttributeResolver::new();

        // Should have all 59 attributes from Phase 0
        assert_eq!(resolver.count(), 59);
        assert!(resolver.cache_enabled);
    }

    #[test]
    fn test_uuid_to_semantic() {
        let resolver = AttributeResolver::new();

        // Test FirstName UUID
        let uuid = FirstName::uuid();
        let semantic_id = resolver.uuid_to_semantic(&uuid).unwrap();
        assert_eq!(semantic_id, "attr.identity.first_name");

        // Test LastName UUID
        let uuid = LastName::uuid();
        let semantic_id = resolver.uuid_to_semantic(&uuid).unwrap();
        assert_eq!(semantic_id, "attr.identity.last_name");
    }

    #[test]
    fn test_semantic_to_uuid() {
        let resolver = AttributeResolver::new();

        // Test FirstName
        let uuid = resolver
            .semantic_to_uuid("attr.identity.first_name")
            .unwrap();
        assert_eq!(uuid, FirstName::uuid());

        // Test Email
        let uuid = resolver.semantic_to_uuid("attr.contact.email").unwrap();
        assert_eq!(uuid, Email::uuid());
    }

    #[test]
    fn test_bidirectional_resolution() {
        let resolver = AttributeResolver::new();

        // UUID → Semantic → UUID should be identity
        let original_uuid = FirstName::uuid();
        let semantic_id = resolver.uuid_to_semantic(&original_uuid).unwrap();
        let resolved_uuid = resolver.semantic_to_uuid(&semantic_id).unwrap();
        assert_eq!(original_uuid, resolved_uuid);

        // Semantic → UUID → Semantic should be identity
        let original_semantic = "attr.identity.last_name";
        let uuid = resolver.semantic_to_uuid(original_semantic).unwrap();
        let resolved_semantic = resolver.uuid_to_semantic(&uuid).unwrap();
        assert_eq!(original_semantic, resolved_semantic);
    }

    #[test]
    fn test_uuid_not_found() {
        let resolver = AttributeResolver::new();

        // Random UUID not in registry
        let random_uuid = Uuid::parse_str("00000000-0000-0000-0000-000000000000").unwrap();
        let result = resolver.uuid_to_semantic(&random_uuid);

        assert!(result.is_err());
        assert!(matches!(result, Err(ResolutionError::UuidNotFound(_))));
    }

    #[test]
    fn test_semantic_id_not_found() {
        let resolver = AttributeResolver::new();

        let result = resolver.semantic_to_uuid("attr.nonexistent.attribute");

        assert!(result.is_err());
        assert!(matches!(
            result,
            Err(ResolutionError::SemanticIdNotFound(_))
        ));
    }

    #[test]
    fn test_has_uuid() {
        let resolver = AttributeResolver::new();

        assert!(resolver.has_uuid(&FirstName::uuid()));
        assert!(resolver.has_uuid(&LastName::uuid()));

        let random_uuid = Uuid::parse_str("00000000-0000-0000-0000-000000000000").unwrap();
        assert!(!resolver.has_uuid(&random_uuid));
    }

    #[test]
    fn test_has_semantic_id() {
        let resolver = AttributeResolver::new();

        assert!(resolver.has_semantic_id("attr.identity.first_name"));
        assert!(resolver.has_semantic_id("attr.identity.last_name"));
        assert!(!resolver.has_semantic_id("attr.nonexistent.field"));
    }

    #[test]
    fn test_all_uuids() {
        let resolver = AttributeResolver::new();

        let uuids = resolver.all_uuids();
        assert_eq!(uuids.len(), 59);
        assert!(uuids.contains(&FirstName::uuid()));
    }

    #[test]
    fn test_all_semantic_ids() {
        let resolver = AttributeResolver::new();

        let ids = resolver.all_semantic_ids();
        assert_eq!(ids.len(), 59);
        assert!(ids.contains(&"attr.identity.first_name".to_string()));
    }

    #[test]
    fn test_resolver_without_cache() {
        let resolver = AttributeResolver::without_cache();

        assert!(!resolver.cache_enabled);
        assert_eq!(resolver.count(), 59);

        // Should still work, just without caching
        let uuid = FirstName::uuid();
        let semantic_id = resolver.uuid_to_semantic(&uuid).unwrap();
        assert_eq!(semantic_id, "attr.identity.first_name");
    }
}
