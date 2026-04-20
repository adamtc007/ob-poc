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
pub(crate) mod attribute_identity_service;
pub(crate) mod attribute_registry_enrichment;

// Dictionary and document services
pub mod dictionary_service_impl;
pub mod document_attribute_crud_service;
pub mod document_catalog_source;
pub mod document_extraction_service;
pub mod extraction_service;

// Executor services
pub mod sink_executor;
pub mod source_executor;

// DSL enrichment (source → segments for UI display)
pub mod dsl_enrichment;

// Viewport resolution (lazy loading for viewport state)
pub mod viewport_resolution_service;

// Board control rules engine (computes who controls the board)
pub mod board_control_rules;

// Phase 5a — ob-poc-side impl of the `dsl_runtime::service_traits::SemanticStateService`
// trait, registered with the platform `ServiceRegistry` at host startup.
pub mod semantic_state_service_impl;

// Phase 5a — ob-poc-side impl of the
// `dsl_runtime::service_traits::StewardshipDispatch` trait; bridges to
// `crate::sem_reg::stewardship::dispatch_phase{0,1}_tool`.
pub mod stewardship_dispatch_impl;

// Phase 5a composite-blocker #1 — ob-poc-side impl of
// `dsl_runtime::service_traits::McpToolRegistry`; bridges to
// `crate::sem_reg::agent::mcp_tools::all_tool_specs()`.
pub mod mcp_tool_registry_impl;

// Re-exports
pub use board_control_rules::{BoardControlResult, BoardControlRulesEngine, RulesEngineConfig};
pub use mcp_tool_registry_impl::ObPocMcpToolRegistry;
pub use semantic_state_service_impl::ObPocSemanticStateService;
pub use stewardship_dispatch_impl::ObPocStewardshipDispatch;
pub use dictionary_service_impl::DictionaryServiceImpl;
pub use document_attribute_crud_service::DocumentAttributeCrudService;
pub use document_extraction_service::DocumentExtractionService;
pub use dsl_enrichment::{bindings_from_session_context, enrich_dsl, BindingInfo};
pub use sink_executor::CompositeSinkExecutor;
pub use source_executor::CompositeSourceExecutor;
pub use viewport_resolution_service::ViewportResolutionService;
