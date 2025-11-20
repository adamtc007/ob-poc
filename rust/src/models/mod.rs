//! Models module for DSL domain architecture
//!
//! This module contains data structures and types used by the dictionary service.
//!
//! ## Architecture Update (November 2025)
//! Legacy models (business_request, document, domain, entity) have been removed.
//! The Forth engine uses simplified direct database operations.

pub mod dictionary_models;

// Re-export dictionary types
pub use dictionary_models::{
    AgenticAttributeCreateRequest, AgenticAttributeCrudResponse, AgenticAttributeDeleteRequest,
    AgenticAttributeDiscoverRequest, AgenticAttributeReadRequest, AgenticAttributeSearchRequest,
    AgenticAttributeUpdateRequest, AgenticAttributeValidateRequest, AttributeDiscoveryRequest,
    AttributeSearchCriteria, AttributeValidationRequest, AttributeValidationResult,
    DictionaryAttribute, DictionaryAttributeWithMetadata, DictionaryHealthCheck,
    DictionaryStatistics, DiscoveredAttribute, NewDictionaryAttribute, UpdateDictionaryAttribute,
};
