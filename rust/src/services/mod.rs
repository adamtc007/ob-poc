//! Services module for core business logic implementations
//!
//! This module contains the service implementations that provide
//! business logic and external interfaces for the DSL engine functionality.
//!
//! NOTE: All services now delegate to DSL Manager as the single entry point
//! for DSL operations. This ensures proper lifecycle management and consistency.
//!
//! ## Architecture Update (November 2025)
//! Legacy services (ai_dsl_service, dsl_ast_sync, dsl_lifecycle) have been
//! removed in favor of the new Forth-engine architecture. All DSL operations
//! now flow through the Forth stack-based execution engine.

// Agentic services

// agentic_dsl_crud removed - use dsl_source::orchestrator::AgenticOrchestrator instead

// Entity search service for typeahead/autocomplete
pub mod entity_search;
pub use entity_search::{
    EntityMatch, EntitySearchRequest, EntitySearchResponse, EntitySearchService, EntityType,
};

// Attribute services
pub mod attribute_executor;
pub mod attribute_lifecycle;

// Dictionary and document services
pub mod dictionary_service_impl;
pub mod document_catalog_source;
pub mod document_extraction_service;
pub mod document_type_detector;
pub mod extraction_service;
// pub mod real_document_extraction_service; // TODO: fix imports

// AI services
// pub mod real_ai_entity_service; // TODO: fix imports

// Executor services
pub mod sink_executor;
pub mod source_executor;

// Taxonomy
pub mod product_services_resources;

// RAG and LLM services for agentic DSL generation

// Re-export DSL Manager types for direct access when needed
pub use crate::dsl_manager::{
    CallChainResult, CleanDslManager, DslManagerError, IncrementalResult,
};

// Re-export RAG and LLM types
// Re-export from dsl_source
pub use crate::dsl_source::agentic::llm_generator::{GeneratorConfig, LlmDslGenerator};
pub use crate::dsl_source::agentic::providers::{
    LlmProvider, LlmResponse, MultiProviderLlm, ProviderConfig,
};
pub use crate::dsl_source::agentic::rag_context::{
    AttributeDefinition as RagAttributeDefinition, DslExample, VocabEntry,
};
pub use crate::dsl_source::agentic::GeneratedDsl;
pub use crate::dsl_source::agentic::{RagContext, RagContextProvider};
pub use crate::dsl_source::validation::{
    ValidationError, ValidationPipeline, ValidationResult, ValidationStage,
};
pub use dictionary_service_impl::DictionaryServiceImpl;
pub use sink_executor::CompositeSinkExecutor;
pub use source_executor::CompositeSourceExecutor;
