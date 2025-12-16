//! Services module for core business logic implementations
//!
//! This module contains the service implementations that provide
//! business logic and external interfaces for the DSL v2 engine.
//!
//! ## Architecture
//! DSL operations flow through dsl_v2::DslExecutor. Services here provide
//! specialized operations for entity search, document extraction, etc.

// Entity search is now handled by EntityGateway gRPC service.
// See rust/crates/entity-gateway/ for the central lookup service.

// Attribute services
pub mod attribute_executor;

// Dictionary and document services
pub mod dictionary_service_impl;
pub mod document_attribute_crud_service;
pub mod document_catalog_source;
pub mod document_extraction_service;
pub mod extraction_service;

// Executor services
pub mod sink_executor;
pub mod source_executor;

// Resolution service (entity reference disambiguation)
pub mod resolution_service;

// DSL enrichment (source → segments for UI display)
pub mod dsl_enrichment;

// Re-exports
pub use dictionary_service_impl::DictionaryServiceImpl;
pub use document_attribute_crud_service::DocumentAttributeCrudService;
pub use document_extraction_service::DocumentExtractionService;
pub use dsl_enrichment::{bindings_from_session_context, enrich_dsl, BindingInfo};
pub use resolution_service::{create_resolution_store, ResolutionService, ResolutionStore};
pub use sink_executor::CompositeSinkExecutor;
pub use source_executor::CompositeSourceExecutor;
