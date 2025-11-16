//! Services module for core business logic implementations
//!
//! This module contains the service implementations that provide
//! business logic and external interfaces for the DSL engine functionality.
//!
//! NOTE: All services now delegate to DSL Manager as the single entry point
//! for DSL operations. This ensures proper lifecycle management and consistency.

// AI DSL service - using proper facade imports
pub(crate) mod ai_dsl_service;
pub mod dsl_ast_sync;
pub mod dsl_lifecycle;

// Attribute service - Phase 1-3 integration layer
// Temporarily disabled - requires attribute_values_typed table from migration
// pub mod attribute_service;

// Document extraction and attribute resolution services
pub mod attribute_executor;
pub mod document_catalog_source;
pub mod document_type_detector;
pub mod extraction_service;

// Dictionary service implementation
#[cfg(feature = "database")]
pub mod dictionary_service_impl;

// Document extraction service - Phase 10
#[cfg(feature = "database")]
pub mod document_extraction_service;

// Real document extraction service - Phase 4 (Document Attribution Plan)
#[cfg(feature = "database")]
pub mod real_document_extraction_service;

// Source/Sink execution services - Phase 10
#[cfg(feature = "database")]
pub mod source_executor;

#[cfg(feature = "database")]
pub mod sink_executor;

#[cfg(feature = "database")]
pub mod attribute_lifecycle;

// Agentic DSL CRUD - Natural Language → DSL → Database
pub mod agentic_dsl_crud;

// Complete Agentic System - Entity, Role, and CBU Management
pub mod agentic_complete;

// Taxonomy CRUD Service - Natural Language → Taxonomy Operations
#[cfg(feature = "database")]
pub mod taxonomy_crud;

// Re-export service types for backwards compatibility

// DSL/AST sync service - master sync endpoints
pub use dsl_ast_sync::{
    DslAstSyncRequest, DslAstSyncService, SyncConfig, SyncOpType, SyncResult, SyncStatus,
};

// Universal DSL lifecycle service - edit→validate→parse→save pattern for ALL DSL
pub use dsl_lifecycle::{
    DslChangeRequest, DslChangeResult, DslChangeType, DslLifecycleService, EditSession,
    EditSessionStatus, LifecycleConfig, LifecycleMetrics, LifecyclePhase,
};

// Re-export DSL Manager types for direct access when needed
pub use crate::dsl_manager::{
    CallChainResult, CleanDslManager, DslManagerError, IncrementalResult,
};

// Re-export ValidationResult from ai_dsl_service to avoid conflicts
pub use ai_dsl_service::ValidationResult;

// Re-export attribute service types
// Temporarily disabled - requires attribute_values_typed table from migration
// pub use attribute_service::{AttributeService, AttributeServiceError, ProcessingResult};

// Re-export extraction and resolution types
pub use attribute_executor::{
    AttributeDictionary, AttributeExecutor, AttributeSink, DatabaseSink, ExecutorError,
};
pub use document_catalog_source::{
    ApiDataSource, AttributeSource, DocumentCatalogSource, FormDataSource, SourceError,
};
pub use document_type_detector::DocumentTypeDetector;
pub use extraction_service::{
    ExtractionError, ExtractionMetadata, ExtractionService, MockExtractionService,
    OcrExtractionService,
};

// Re-export dictionary service
#[cfg(feature = "database")]
pub use dictionary_service_impl::DictionaryServiceImpl;

// Re-export document extraction service
#[cfg(feature = "database")]
pub use document_extraction_service::DocumentExtractionService;

// Re-export real document extraction service
#[cfg(feature = "database")]
pub use real_document_extraction_service::{
    ExtractionError as RealExtractionError, ExtractionResult, RealDocumentExtractionService,
};

// Re-export source/sink execution services
#[cfg(feature = "database")]
pub use source_executor::{CompositeSourceExecutor, SourceExecutor};

#[cfg(feature = "database")]
pub use sink_executor::{CompositeSinkExecutor, SinkExecutor};

#[cfg(feature = "database")]
pub use attribute_lifecycle::AttributeLifecycleService;

/// Master sync service factory for DSL/AST table synchronization
pub fn create_sync_service() -> DslAstSyncService {
    DslAstSyncService::new()
}

/// Create sync service with database integration
#[cfg(feature = "database")]
pub fn create_sync_service_with_db(pool: sqlx::PgPool) -> DslAstSyncService {
    let mut service = DslAstSyncService::new();
    service.set_database_pool(pool);
    service
}

/// Create universal DSL lifecycle service
/// Implements the universal edit→validate→parse→save pattern for ALL DSL changes
pub fn create_lifecycle_service() -> DslLifecycleService {
    DslLifecycleService::new()
}

/// Create lifecycle service with custom configuration
pub fn create_lifecycle_service_with_config(config: LifecycleConfig) -> DslLifecycleService {
    DslLifecycleService::with_config(config)
}
