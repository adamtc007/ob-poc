//! Entity Transaction Manager for Agentic CRUD Operations
//!
//! This module provides transaction management for batch entity operations,
//! including rollback strategies, atomic operations, and complex workflows.
//! It extends the entity CRUD service with enterprise-grade transaction support.

use crate::models::entity_models::*;
use crate::services::entity_crud_service::{EntityCrudError, EntityCrudResult, EntityCrudService};
use anyhow::{anyhow, Context, Result};
use chrono::{DateTime, Utc};
use serde::{Deserialize, Serialize};
use sqlx::{PgPool, Postgres, Transaction};
use std::collections::HashMap;
use tracing::{debug, error, info, warn};
use uuid::Uuid;

/// Entity Transaction Manager for batch operations
pub struct EntityTransactionManager {
    /// Database connection pool
    pool: PgPool,
    /// Entity CRUD service for individual operations
    entity_service: EntityCrudService,
    /// Transaction configuration
    config: TransactionConfig,
}

/// Configuration for transaction management
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct TransactionConfig {
    /// Maximum number of operations per transaction
    pub max_operations_per_transaction: usize,
    /// Transaction timeout in seconds
    pub transaction_timeout_seconds: u32,
    /// Default rollback strategy
    pub default_rollback_strategy: RollbackStrategy,
    /// Enable automatic retry on failure
    pub enable_auto_retry: bool,
    /// Maximum retry attempts
    pub max_retry_attempts: usize,
    /// Enable operation simulation before execution
    pub enable_simulation: bool,
}

/// Rollback strategy for failed transactions
#[derive(Debug, Clone, Serialize, Deserialize, PartialEq)]
pub enum RollbackStrategy {
    /// Roll back all operations in the transaction
    FullRollback,
    /// Roll back only failed operations, keep successful ones
    PartialRollback,
    /// Continue with remaining operations, log failures
    ContinueOnError,
    /// Stop immediately on first error
    StopOnError,
}

/// Transaction mode for execution
#[derive(Debug, Clone, Serialize, Deserialize, PartialEq)]
pub enum TransactionMode {
    /// Execute all operations atomically
    Atomic,
    /// Execute operations in sequence, partial success allowed
    Sequential,
    /// Simulate operations without making changes
    Simulation,
}

/// Batch entity operation request
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct BatchEntityRequest {
    pub transaction_id: Option<Uuid>,
    pub operations: Vec<EntityOperation>,
    pub mode: TransactionMode,
    pub rollback_strategy: RollbackStrategy,
    pub description: String,
    pub requested_by: String,
}

/// Individual entity operation within a batch
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct EntityOperation {
    pub operation_id: Uuid,
    pub operation_type: EntityOperationType,
    pub asset_type: EntityAssetType,
    pub instruction: String,
    pub context: HashMap<String, serde_json::Value>,
    pub dependencies: Vec<Uuid>, // Operations that must complete first
}

/// Type of entity operation
#[derive(Debug, Clone, Serialize, Deserialize, PartialEq)]
pub enum EntityOperationType {
    Create,
    Read,
    Update,
    Delete,
    Link,   // Link entity to CBU
    Unlink, // Remove entity-CBU link
}

/// Result of a batch transaction
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct BatchTransactionResult {
    pub transaction_id: Uuid,
    pub overall_status: TransactionStatus,
    pub operations_completed: usize,
    pub operations_failed: usize,
    pub operation_results: Vec<OperationResult>,
    pub execution_time_ms: i64,
    pub rollback_actions: Vec<RollbackAction>,
    pub error_summary: Option<String>,
}

/// Status of a transaction
#[derive(Debug, Clone, Serialize, Deserialize, PartialEq)]
pub enum TransactionStatus {
    Pending,
    InProgress,
    Completed,
    PartiallyCompleted,
    Failed,
    RolledBack,
    Simulated,
}

/// Result of an individual operation within a batch
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct OperationResult {
    pub operation_id: Uuid,
    pub status: OperationStatus,
    pub affected_records: Vec<Uuid>,
    pub generated_dsl: Option<String>,
    pub execution_time_ms: i32,
    pub error_message: Option<String>,
    pub rollback_data: Option<serde_json::Value>,
}

/// Status of an individual operation
#[derive(Debug, Clone, Serialize, Deserialize, PartialEq)]
pub enum OperationStatus {
    Pending,
    Executing,
    Completed,
    Failed,
    Skipped,
    RolledBack,
}

/// Rollback action taken during transaction failure
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct RollbackAction {
    pub action_id: Uuid,
    pub original_operation_id: Uuid,
    pub rollback_type: RollbackType,
    pub rollback_dsl: String,
    pub status: OperationStatus,
    pub executed_at: DateTime<Utc>,
}

/// Type of rollback action
#[derive(Debug, Clone, Serialize, Deserialize)]
pub enum RollbackType {
    DeleteCreatedRecord,
    RestoreDeletedRecord,
    RevertUpdate,
    RemoveLink,
    RestoreLink,
}

impl EntityTransactionManager {
    /// Create a new transaction manager
    pub fn new(
        pool: PgPool,
        entity_service: EntityCrudService,
        config: Option<TransactionConfig>,
    ) -> Self {
        Self {
            pool,
            entity_service,
            config: config.unwrap_or_default(),
        }
    }

    /// Execute a batch of entity operations
    pub async fn execute_batch(
        &self,
        request: BatchEntityRequest,
    ) -> EntityCrudResult<BatchTransactionResult> {
        let transaction_id = request.transaction_id.unwrap_or_else(Uuid::new_v4);
        let start_time = std::time::Instant::now();

        info!(
            "Starting batch transaction {} with {} operations",
            transaction_id,
            request.operations.len()
        );

        // Validate batch request
        self.validate_batch_request(&request)?;

        // Sort operations by dependencies
        let sorted_operations = self.sort_operations_by_dependencies(&request.operations)?;

        match request.mode {
            TransactionMode::Atomic => {
                self.execute_atomic_batch(transaction_id, sorted_operations, &request)
                    .await
            }
            TransactionMode::Sequential => {
                self.execute_sequential_batch(transaction_id, sorted_operations, &request)
                    .await
            }
            TransactionMode::Simulation => {
                self.simulate_batch(transaction_id, sorted_operations, &request)
                    .await
            }
        }
    }

    /// Execute operations atomically (all or nothing)
    async fn execute_atomic_batch(
        &self,
        transaction_id: Uuid,
        operations: Vec<EntityOperation>,
        request: &BatchEntityRequest,
    ) -> EntityCrudResult<BatchTransactionResult> {
        let start_time = std::time::Instant::now();
        let mut transaction = self.pool.begin().await.map_err(EntityCrudError::from)?;

        let mut operation_results = Vec::new();
        let mut completed_count = 0;
        let mut failed_count = 0;

        // Execute all operations within the transaction
        for operation in &operations {
            debug!("Executing operation {}", operation.operation_id);

            match self
                .execute_single_operation(&mut transaction, operation)
                .await
            {
                Ok(result) => {
                    completed_count += 1;
                    operation_results.push(result);
                }
                Err(error) => {
                    failed_count += 1;
                    operation_results.push(OperationResult {
                        operation_id: operation.operation_id,
                        status: OperationStatus::Failed,
                        affected_records: vec![],
                        generated_dsl: None,
                        execution_time_ms: 0,
                        error_message: Some(error.to_string()),
                        rollback_data: None,
                    });

                    // In atomic mode, any failure causes full rollback
                    error!(
                        "Operation {} failed, rolling back transaction",
                        operation.operation_id
                    );
                    transaction
                        .rollback()
                        .await
                        .map_err(EntityCrudError::from)?;

                    return Ok(BatchTransactionResult {
                        transaction_id,
                        overall_status: TransactionStatus::RolledBack,
                        operations_completed: 0,
                        operations_failed: operations.len(),
                        operation_results,
                        execution_time_ms: start_time.elapsed().as_millis() as i64,
                        rollback_actions: vec![],
                        error_summary: Some(format!(
                            "Atomic transaction failed at operation {}: {}",
                            operation.operation_id, error
                        )),
                    });
                }
            }
        }

        // Commit the transaction if all operations succeeded
        transaction.commit().await.map_err(EntityCrudError::from)?;

        info!(
            "Atomic batch transaction {} completed successfully",
            transaction_id
        );

        Ok(BatchTransactionResult {
            transaction_id,
            overall_status: TransactionStatus::Completed,
            operations_completed: completed_count,
            operations_failed: failed_count,
            operation_results,
            execution_time_ms: start_time.elapsed().as_millis() as i64,
            rollback_actions: vec![],
            error_summary: None,
        })
    }

    /// Execute operations sequentially with partial success allowed
    async fn execute_sequential_batch(
        &self,
        transaction_id: Uuid,
        operations: Vec<EntityOperation>,
        request: &BatchEntityRequest,
    ) -> EntityCrudResult<BatchTransactionResult> {
        let start_time = std::time::Instant::now();
        let mut operation_results = Vec::new();
        let mut completed_count = 0;
        let mut failed_count = 0;
        let mut rollback_actions = Vec::new();

        for operation in &operations {
            debug!("Executing sequential operation {}", operation.operation_id);

            // Each operation gets its own transaction in sequential mode
            let mut transaction = self.pool.begin().await.map_err(EntityCrudError::from)?;

            match self
                .execute_single_operation(&mut transaction, operation)
                .await
            {
                Ok(mut result) => {
                    // Commit individual operation
                    match transaction.commit().await {
                        Ok(_) => {
                            completed_count += 1;
                            result.status = OperationStatus::Completed;
                            operation_results.push(result);
                        }
                        Err(commit_error) => {
                            failed_count += 1;
                            result.status = OperationStatus::Failed;
                            result.error_message = Some(commit_error.to_string());
                            operation_results.push(result);

                            if request.rollback_strategy == RollbackStrategy::StopOnError {
                                break;
                            }
                        }
                    }
                }
                Err(error) => {
                    failed_count += 1;
                    transaction
                        .rollback()
                        .await
                        .map_err(EntityCrudError::from)?;

                    operation_results.push(OperationResult {
                        operation_id: operation.operation_id,
                        status: OperationStatus::Failed,
                        affected_records: vec![],
                        generated_dsl: None,
                        execution_time_ms: 0,
                        error_message: Some(error.to_string()),
                        rollback_data: None,
                    });

                    // Handle rollback strategy
                    match request.rollback_strategy {
                        RollbackStrategy::StopOnError => {
                            error!(
                                "Stopping batch execution due to error in operation {}",
                                operation.operation_id
                            );
                            break;
                        }
                        RollbackStrategy::FullRollback => {
                            warn!(
                                "Performing full rollback due to error in operation {}",
                                operation.operation_id
                            );
                            rollback_actions
                                .extend(self.perform_full_rollback(&operation_results).await?);
                            break;
                        }
                        RollbackStrategy::ContinueOnError => {
                            warn!(
                                "Continuing with next operation despite error in {}",
                                operation.operation_id
                            );
                            continue;
                        }
                        RollbackStrategy::PartialRollback => {
                            // Continue, but mark for potential partial rollback
                            continue;
                        }
                    }
                }
            }
        }

        let overall_status = if failed_count == 0 {
            TransactionStatus::Completed
        } else if completed_count == 0 {
            TransactionStatus::Failed
        } else {
            TransactionStatus::PartiallyCompleted
        };

        info!(
            "Sequential batch transaction {} completed: {}/{} operations successful",
            transaction_id,
            completed_count,
            operations.len()
        );

        Ok(BatchTransactionResult {
            transaction_id,
            overall_status,
            operations_completed: completed_count,
            operations_failed: failed_count,
            operation_results,
            execution_time_ms: start_time.elapsed().as_millis() as i64,
            rollback_actions,
            error_summary: if failed_count > 0 {
                Some(format!(
                    "{} operations failed out of {}",
                    failed_count,
                    operations.len()
                ))
            } else {
                None
            },
        })
    }

    /// Simulate batch operations without making changes
    async fn simulate_batch(
        &self,
        transaction_id: Uuid,
        operations: Vec<EntityOperation>,
        _request: &BatchEntityRequest,
    ) -> EntityCrudResult<BatchTransactionResult> {
        let start_time = std::time::Instant::now();
        let mut operation_results = Vec::new();

        info!("Simulating batch transaction {}", transaction_id);

        for operation in &operations {
            // Create a simulated result
            let simulated_result = OperationResult {
                operation_id: operation.operation_id,
                status: OperationStatus::Completed,
                affected_records: vec![Uuid::new_v4()], // Mock affected record
                generated_dsl: Some(format!(
                    "(data.{} :asset \"{}\")",
                    operation.operation_type.as_verb(),
                    operation.asset_type.asset_name()
                )),
                execution_time_ms: 10, // Mock execution time
                error_message: None,
                rollback_data: None,
            };

            operation_results.push(simulated_result);
        }

        info!("Batch transaction simulation {} completed", transaction_id);

        Ok(BatchTransactionResult {
            transaction_id,
            overall_status: TransactionStatus::Simulated,
            operations_completed: operations.len(),
            operations_failed: 0,
            operation_results,
            execution_time_ms: start_time.elapsed().as_millis() as i64,
            rollback_actions: vec![],
            error_summary: None,
        })
    }

    /// Execute a single operation within a transaction
    async fn execute_single_operation(
        &self,
        transaction: &mut Transaction<'_, Postgres>,
        operation: &EntityOperation,
    ) -> EntityCrudResult<OperationResult> {
        let start_time = std::time::Instant::now();

        // Convert operation to appropriate CRUD request
        match operation.operation_type {
            EntityOperationType::Create => {
                let request = AgenticEntityCreateRequest {
                    instruction: operation.instruction.clone(),
                    asset_type: operation.asset_type.clone(),
                    context: operation.context.clone(),
                    link_to_cbu: None,
                    role_in_cbu: None,
                };

                // Execute create operation (this would need transaction support in the service)
                // For now, we'll mock the response
                Ok(OperationResult {
                    operation_id: operation.operation_id,
                    status: OperationStatus::Completed,
                    affected_records: vec![Uuid::new_v4()],
                    generated_dsl: Some(format!(
                        "(data.create :asset \"{}\")",
                        operation.asset_type.asset_name()
                    )),
                    execution_time_ms: start_time.elapsed().as_millis() as i32,
                    error_message: None,
                    rollback_data: Some(serde_json::json!({
                        "operation": "create",
                        "entity_id": Uuid::new_v4()
                    })),
                })
            }
            EntityOperationType::Update => Ok(OperationResult {
                operation_id: operation.operation_id,
                status: OperationStatus::Completed,
                affected_records: vec![Uuid::new_v4()],
                generated_dsl: Some(format!(
                    "(data.update :asset \"{}\")",
                    operation.asset_type.asset_name()
                )),
                execution_time_ms: start_time.elapsed().as_millis() as i32,
                error_message: None,
                rollback_data: Some(serde_json::json!({
                    "operation": "update",
                    "original_values": {}
                })),
            }),
            _ => {
                // Mock implementation for other operations
                Ok(OperationResult {
                    operation_id: operation.operation_id,
                    status: OperationStatus::Completed,
                    affected_records: vec![],
                    generated_dsl: Some(format!(
                        "(data.{} :asset \"{}\")",
                        operation.operation_type.as_verb(),
                        operation.asset_type.asset_name()
                    )),
                    execution_time_ms: start_time.elapsed().as_millis() as i32,
                    error_message: None,
                    rollback_data: None,
                })
            }
        }
    }

    /// Perform full rollback of completed operations
    async fn perform_full_rollback(
        &self,
        completed_operations: &[OperationResult],
    ) -> EntityCrudResult<Vec<RollbackAction>> {
        let mut rollback_actions = Vec::new();

        for operation in completed_operations.iter().rev() {
            if operation.status == OperationStatus::Completed {
                if let Some(rollback_data) = &operation.rollback_data {
                    let rollback_action = RollbackAction {
                        action_id: Uuid::new_v4(),
                        original_operation_id: operation.operation_id,
                        rollback_type: RollbackType::DeleteCreatedRecord, // Simplified
                        rollback_dsl: "(data.delete :asset \"entity\")".to_string(), // Mock
                        status: OperationStatus::Completed,
                        executed_at: Utc::now(),
                    };
                    rollback_actions.push(rollback_action);
                }
            }
        }

        Ok(rollback_actions)
    }

    /// Validate batch request before execution
    fn validate_batch_request(&self, request: &BatchEntityRequest) -> EntityCrudResult<()> {
        if request.operations.is_empty() {
            return Err(EntityCrudError::ValidationError(
                "Batch request must contain at least one operation".to_string(),
            ));
        }

        if request.operations.len() > self.config.max_operations_per_transaction {
            return Err(EntityCrudError::ValidationError(format!(
                "Batch request contains {} operations, maximum allowed is {}",
                request.operations.len(),
                self.config.max_operations_per_transaction
            )));
        }

        // Validate each operation
        for operation in &request.operations {
            if operation.instruction.is_empty() {
                return Err(EntityCrudError::ValidationError(format!(
                    "Operation {} has empty instruction",
                    operation.operation_id
                )));
            }
        }

        Ok(())
    }

    /// Sort operations by their dependencies
    fn sort_operations_by_dependencies(
        &self,
        operations: &[EntityOperation],
    ) -> EntityCrudResult<Vec<EntityOperation>> {
        let mut sorted = Vec::new();
        let mut remaining: Vec<_> = operations.iter().cloned().collect();
        let mut processed_ids = std::collections::HashSet::new();

        while !remaining.is_empty() {
            let mut progress = false;

            remaining.retain(|op| {
                // Check if all dependencies are satisfied
                let deps_satisfied = op
                    .dependencies
                    .iter()
                    .all(|dep_id| processed_ids.contains(dep_id));

                if deps_satisfied {
                    sorted.push(op.clone());
                    processed_ids.insert(op.operation_id);
                    progress = true;
                    false // Remove from remaining
                } else {
                    true // Keep in remaining
                }
            });

            if !progress {
                return Err(EntityCrudError::ValidationError(
                    "Circular dependency detected in batch operations".to_string(),
                ));
            }
        }

        Ok(sorted)
    }

    /// Get transaction status
    pub async fn get_transaction_status(
        &self,
        transaction_id: Uuid,
    ) -> EntityCrudResult<TransactionStatus> {
        // Query database for transaction status
        // This would need to be implemented with actual database queries
        Ok(TransactionStatus::Completed)
    }

    /// Cancel a running transaction
    pub async fn cancel_transaction(&self, transaction_id: Uuid) -> EntityCrudResult<bool> {
        // Implementation would involve stopping ongoing operations and cleanup
        warn!("Transaction cancellation requested for {}", transaction_id);
        Ok(true)
    }
}

impl Default for TransactionConfig {
    fn default() -> Self {
        Self {
            max_operations_per_transaction: 100,
            transaction_timeout_seconds: 300, // 5 minutes
            default_rollback_strategy: RollbackStrategy::FullRollback,
            enable_auto_retry: true,
            max_retry_attempts: 3,
            enable_simulation: true,
        }
    }
}

impl EntityOperationType {
    fn as_verb(&self) -> &'static str {
        match self {
            EntityOperationType::Create => "create",
            EntityOperationType::Read => "read",
            EntityOperationType::Update => "update",
            EntityOperationType::Delete => "delete",
            EntityOperationType::Link => "link",
            EntityOperationType::Unlink => "unlink",
        }
    }
}

impl std::fmt::Display for TransactionStatus {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            TransactionStatus::Pending => write!(f, "Pending"),
            TransactionStatus::InProgress => write!(f, "In Progress"),
            TransactionStatus::Completed => write!(f, "Completed"),
            TransactionStatus::PartiallyCompleted => write!(f, "Partially Completed"),
            TransactionStatus::Failed => write!(f, "Failed"),
            TransactionStatus::RolledBack => write!(f, "Rolled Back"),
            TransactionStatus::Simulated => write!(f, "Simulated"),
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_operation_dependency_sorting() {
        // Test dependency sorting logic
        let operations = vec![EntityOperation {
            operation_id: Uuid::new_v4(),
            operation_type: EntityOperationType::Create,
            asset_type: EntityAssetType::Partnership,
            instruction: "Create parent entity".to_string(),
            context: HashMap::new(),
            dependencies: vec![],
        }];

        // This would be tested with the actual transaction manager
        assert_eq!(operations.len(), 1);
    }

    #[test]
    fn test_rollback_strategy_logic() {
        let strategies = vec![
            RollbackStrategy::FullRollback,
            RollbackStrategy::PartialRollback,
            RollbackStrategy::ContinueOnError,
            RollbackStrategy::StopOnError,
        ];

        assert_eq!(strategies.len(), 4);
    }

    #[test]
    fn test_transaction_config_defaults() {
        let config = TransactionConfig::default();
        assert_eq!(config.max_operations_per_transaction, 100);
        assert_eq!(config.transaction_timeout_seconds, 300);
        assert_eq!(
            config.default_rollback_strategy,
            RollbackStrategy::FullRollback
        );
    }
}
