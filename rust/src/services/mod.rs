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

// Agent E2E test harness
#[cfg(feature = "database")]
pub mod agent_e2e_test_harness;
#[cfg(feature = "database")]
pub use agent_e2e_test_harness::{AgentE2ETestHarness, E2ETestScenario, TestResult};

// Attribute services
pub mod attribute_executor;
pub mod attribute_lifecycle;

// Dictionary and document services
pub mod dictionary_service_impl;
pub mod document_attribute_crud_service;
pub mod document_catalog_source;
pub mod document_extraction_service;
pub mod document_type_detector;
pub mod extraction_service;

// Executor services
pub mod sink_executor;
pub mod source_executor;

// Test harness
pub mod document_attribute_test_harness;

// Taxonomy
pub mod product_services_resources;

// Re-exports
pub use dictionary_service_impl::DictionaryServiceImpl;
pub use document_attribute_crud_service::DocumentAttributeCrudService;
pub use document_extraction_service::DocumentExtractionService;
pub use sink_executor::CompositeSinkExecutor;
pub use source_executor::CompositeSourceExecutor;
