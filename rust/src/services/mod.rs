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
pub mod attribute_service;

// Re-export service types for backwards compatibility
pub(crate) use ai_dsl_service::{
    AiDslService, AiOnboardingRequest, AiOnboardingResponse, CbuGenerator, DslInstanceSummary,
    ExecutionDetails, HealthCheckResult,
};

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
pub use attribute_service::{AttributeService, AttributeServiceError, ProcessingResult};

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
