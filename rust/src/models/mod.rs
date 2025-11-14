//! Models module for DSL domain architecture
//!
//! This module contains all the data structures and types used to represent
//! DSL domains, versions, AST storage, and execution tracking in the database.

// Allow dead code for models - many are database schema definitions not yet used
#![allow(dead_code)]
#![allow(unused_imports)]

pub(crate) mod business_request_models;
pub mod dictionary_models;
pub(crate) mod document_models;
pub(crate) mod domain_models;
pub(crate) mod entity_models;

// Re-export commonly used types for convenience

pub use dictionary_models::{
    AgenticAttributeCreateRequest, AgenticAttributeCrudResponse, AgenticAttributeDeleteRequest,
    AgenticAttributeDiscoverRequest, AgenticAttributeReadRequest, AgenticAttributeSearchRequest,
    AgenticAttributeUpdateRequest, AgenticAttributeValidateRequest, AttributeDiscoveryRequest,
    AttributeSearchCriteria, AttributeValidationRequest, AttributeValidationResult,
    DictionaryAttribute, DictionaryAttributeWithMetadata, DictionaryHealthCheck,
    DictionaryStatistics, DiscoveredAttribute, NewDictionaryAttribute, UpdateDictionaryAttribute,
};
