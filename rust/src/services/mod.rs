//! Services module for gRPC implementations
//!
//! This module contains the gRPC service implementations that provide
//! external interfaces to the DSL engine functionality.

pub mod ai_dsl_service;
pub mod crud_transaction_manager;
pub mod crud_validator;
pub mod dsl_retrieval_service;
pub mod dsl_transform_service;
pub mod entity_crud_service;
pub mod entity_transaction_manager;

pub use ai_dsl_service::{AiDslService, AiOnboardingRequest, AiOnboardingResponse, CbuGenerator};
pub use crud_transaction_manager::{CrudTransactionManager, TransactionConfig, TransactionResult};
pub use crud_validator::{CrudValidator, ValidationResult, ValidatorConfig};
pub use dsl_retrieval_service::DslRetrievalService;
pub use dsl_transform_service::DslTransformService;
pub use entity_crud_service::{
    EntityCrudConfig, EntityCrudError, EntityCrudResult, EntityCrudService,
};
pub use entity_transaction_manager::{
    BatchEntityRequest, BatchTransactionResult, EntityOperation, EntityOperationType,
    EntityTransactionManager, RollbackStrategy, TransactionMode, TransactionStatus,
};
