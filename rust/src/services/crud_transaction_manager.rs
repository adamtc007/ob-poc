//! CRUD Transaction Manager - Phase 3 Transaction Support
//!
//! This module provides comprehensive transaction management for batch CRUD operations
//! with support for different execution modes and rollback strategies.

use crate::{
    BatchOperation, CrudStatement, DataCreate, DataDelete, DataRead, DataUpdate, RollbackStrategy,
    TransactionMode, TransactionResult,
};
use anyhow::{anyhow, Context, Result};
use sqlx::{PgPool, Postgres, Transaction};
use std::collections::HashMap;
use std::time::{Duration, Instant};
use tokio::time::timeout;
use uuid::Uuid;

/// Transaction Manager for CRUD operations
#[derive(Debug, Clone)]
pub struct CrudTransactionManager {
    /// Database connection pool
    pool: PgPool,
    /// Transaction configuration
    config: TransactionConfig,
    /// Active transactions
    active_transactions: HashMap<String, TransactionInfo>,
}

/// Configuration for transaction management
#[derive(Debug, Clone)]
pub struct TransactionConfig {
    /// Default timeout for transactions (seconds)
    pub default_timeout_seconds: u64,
    /// Maximum batch size
    pub max_batch_size: usize,
    /// Enable parallel execution
    pub enable_parallel_execution: bool,
    /// Maximum concurrent transactions
    pub max_concurrent_transactions: usize,
    /// Retry attempts for failed operations
    pub max_retries: usize,
    /// Delay between retries (milliseconds)
    pub retry_delay_ms: u64,
}

/// Information about an active transaction
#[derive(Debug, Clone)]
pub struct TransactionInfo {
    pub id: String,
    pub start_time: Instant,
    pub operations_count: usize,
    pub completed_count: usize,
    pub status: TransactionStatus,
    pub rollback_strategy: RollbackStrategy,
}

#[derive(Debug, Clone, PartialEq)]
pub enum TransactionStatus {
    Active,
    Committing,
    RollingBack,
    Completed,
    Failed,
}

/// Result of a single operation within a transaction
#[derive(Debug, Clone)]
pub struct OperationResult {
    pub operation_index: usize,
    pub success: bool,
    pub affected_rows: u64,
    pub duration_ms: u64,
    pub error_message: Option<String>,
}

/// Detailed transaction execution result
#[derive(Debug, Clone)]
pub struct DetailedTransactionResult {
    pub transaction_id: String,
    pub overall_success: bool,
    pub operation_results: Vec<OperationResult>,
    pub completed_operations: Vec<usize>,
    pub failed_operations: Vec<(usize, String)>,
    pub rollback_performed: bool,
    pub total_duration_ms: u64,
    pub rollback_duration_ms: Option<u64>,
    pub final_status: TransactionStatus,
}

impl Default for TransactionConfig {
    fn default() -> Self {
        Self {
            default_timeout_seconds: 300, // 5 minutes
            max_batch_size: 1000,
            enable_parallel_execution: false, // Start with sequential for safety
            max_concurrent_transactions: 10,
            max_retries: 3,
            retry_delay_ms: 1000,
        }
    }
}

impl CrudTransactionManager {
    /// Creates a new transaction manager
    pub fn new(pool: PgPool) -> Self {
        Self {
            pool,
            config: TransactionConfig::default(),
            active_transactions: HashMap::new(),
        }
    }

    /// Creates a transaction manager with custom configuration
    pub fn with_config(pool: PgPool, config: TransactionConfig) -> Self {
        Self {
            pool,
            config,
            active_transactions: HashMap::new(),
        }
    }

    /// Executes a batch operation with transaction management
    pub async fn execute_batch(&mut self, batch: BatchOperation) -> Result<TransactionResult> {
        let transaction_id = Uuid::new_v4().to_string();
        let start_time = Instant::now();

        // Validate batch operation
        self.validate_batch(&batch)?;

        // Register transaction
        let transaction_info = TransactionInfo {
            id: transaction_id.clone(),
            start_time,
            operations_count: batch.operations.len(),
            completed_count: 0,
            status: TransactionStatus::Active,
            rollback_strategy: batch.rollback_strategy.clone(),
        };
        self.active_transactions
            .insert(transaction_id.clone(), transaction_info);

        // Execute based on transaction mode
        let result = match batch.transaction_mode {
            TransactionMode::Atomic => self.execute_atomic_batch(&transaction_id, batch).await,
            TransactionMode::Sequential => {
                self.execute_sequential_batch(&transaction_id, batch).await
            }
            TransactionMode::Parallel => self.execute_parallel_batch(&transaction_id, batch).await,
        };

        // Clean up transaction info
        self.active_transactions.remove(&transaction_id);

        result
    }

    /// Executes operations atomically (all or nothing)
    async fn execute_atomic_batch(
        &mut self,
        transaction_id: &str,
        batch: BatchOperation,
    ) -> Result<TransactionResult> {
        let start_time = Instant::now();
        let timeout_duration = Duration::from_secs(
            batch
                .operations
                .first()
                .and_then(|_| Some(self.config.default_timeout_seconds))
                .unwrap_or(self.config.default_timeout_seconds),
        );

        let result = timeout(timeout_duration, async {
            let mut tx = self.pool.begin().await?;
            let mut completed_operations = Vec::new();
            let mut failed_operations = Vec::new();

            for (index, operation) in batch.operations.iter().enumerate() {
                match self.execute_single_operation(&mut tx, operation).await {
                    Ok(_) => {
                        completed_operations.push(index);
                        self.update_transaction_progress(transaction_id, index + 1);
                    }
                    Err(e) => {
                        failed_operations.push((index, e.to_string()));
                        // For atomic operations, any failure triggers rollback
                        tx.rollback().await?;
                        return Ok(TransactionResult {
                            success: false,
                            completed_operations,
                            failed_operations,
                            rollback_performed: true,
                            total_duration_ms: start_time.elapsed().as_millis() as u64,
                        });
                    }
                }
            }

            // Commit if all operations succeeded
            tx.commit().await?;

            Ok(TransactionResult {
                success: true,
                completed_operations,
                failed_operations,
                rollback_performed: false,
                total_duration_ms: start_time.elapsed().as_millis() as u64,
            })
        })
        .await;

        match result {
            Ok(transaction_result) => transaction_result,
            Err(_) => {
                // Timeout occurred
                self.update_transaction_status(transaction_id, TransactionStatus::Failed);
                Ok(TransactionResult {
                    success: false,
                    completed_operations: Vec::new(),
                    failed_operations: vec![(0, "Transaction timeout".to_string())],
                    rollback_performed: true,
                    total_duration_ms: start_time.elapsed().as_millis() as u64,
                })
            }
        }
    }

    /// Executes operations sequentially with configurable rollback
    async fn execute_sequential_batch(
        &mut self,
        transaction_id: &str,
        batch: BatchOperation,
    ) -> Result<TransactionResult> {
        let start_time = Instant::now();
        let mut completed_operations = Vec::new();
        let mut failed_operations = Vec::new();
        let mut rollback_performed = false;

        match batch.rollback_strategy {
            RollbackStrategy::FullRollback => {
                // Use transaction for full rollback capability
                let mut tx = self.pool.begin().await?;

                for (index, operation) in batch.operations.iter().enumerate() {
                    match self.execute_single_operation(&mut tx, operation).await {
                        Ok(_) => {
                            completed_operations.push(index);
                            self.update_transaction_progress(transaction_id, index + 1);
                        }
                        Err(e) => {
                            failed_operations.push((index, e.to_string()));
                            tx.rollback().await?;
                            rollback_performed = true;
                            break;
                        }
                    }
                }

                if failed_operations.is_empty() {
                    tx.commit().await?;
                }
            }
            RollbackStrategy::PartialRollback => {
                // Execute in smaller transactions
                for (index, operation) in batch.operations.iter().enumerate() {
                    let mut tx = self.pool.begin().await?;

                    match self.execute_single_operation(&mut tx, operation).await {
                        Ok(_) => {
                            tx.commit().await?;
                            completed_operations.push(index);
                            self.update_transaction_progress(transaction_id, index + 1);
                        }
                        Err(e) => {
                            tx.rollback().await?;
                            failed_operations.push((index, e.to_string()));
                            rollback_performed = true;
                            break;
                        }
                    }
                }
            }
            RollbackStrategy::ContinueOnError => {
                // Execute each operation independently
                for (index, operation) in batch.operations.iter().enumerate() {
                    let mut tx = self.pool.begin().await?;

                    match self.execute_single_operation(&mut tx, operation).await {
                        Ok(_) => {
                            tx.commit().await?;
                            completed_operations.push(index);
                            self.update_transaction_progress(transaction_id, index + 1);
                        }
                        Err(e) => {
                            tx.rollback().await?;
                            failed_operations.push((index, e.to_string()));
                            // Continue with next operation
                        }
                    }
                }
            }
        }

        let success = failed_operations.is_empty();
        self.update_transaction_status(
            transaction_id,
            if success {
                TransactionStatus::Completed
            } else {
                TransactionStatus::Failed
            },
        );

        Ok(TransactionResult {
            success,
            completed_operations,
            failed_operations,
            rollback_performed,
            total_duration_ms: start_time.elapsed().as_millis() as u64,
        })
    }

    /// Executes operations in parallel (not implemented for safety)
    async fn execute_parallel_batch(
        &mut self,
        transaction_id: &str,
        batch: BatchOperation,
    ) -> Result<TransactionResult> {
        // For safety, fall back to sequential execution
        // Parallel execution would require careful coordination of transactions
        println!(
            "Warning: Parallel execution not implemented, falling back to sequential for transaction {}",
            transaction_id
        );
        self.execute_sequential_batch(transaction_id, batch).await
    }

    /// Executes a single CRUD operation within a transaction
    async fn execute_single_operation(
        &self,
        tx: &mut Transaction<'_, Postgres>,
        operation: &CrudStatement,
    ) -> Result<u64> {
        match operation {
            CrudStatement::DataCreate(op) => self.execute_create_in_tx(tx, op).await,
            CrudStatement::DataRead(op) => self.execute_read_in_tx(tx, op).await,
            CrudStatement::DataUpdate(op) => self.execute_update_in_tx(tx, op).await,
            CrudStatement::DataDelete(op) => self.execute_delete_in_tx(tx, op).await,
            _ => Err(anyhow!("Operation type not supported in transactions yet")),
        }
    }

    /// Executes a CREATE operation within a transaction
    async fn execute_create_in_tx(
        &self,
        tx: &mut Transaction<'_, Postgres>,
        op: &DataCreate,
    ) -> Result<u64> {
        // For demo purposes, return success
        // In real implementation, this would execute the actual SQL
        println!("Executing CREATE for asset '{}' in transaction", op.asset);

        // Simulate some work
        tokio::time::sleep(Duration::from_millis(10)).await;

        // Mock successful insert
        Ok(1)
    }

    /// Executes a READ operation within a transaction
    async fn execute_read_in_tx(
        &self,
        tx: &mut Transaction<'_, Postgres>,
        op: &DataRead,
    ) -> Result<u64> {
        println!("Executing READ for asset '{}' in transaction", op.asset);

        // Simulate some work
        tokio::time::sleep(Duration::from_millis(5)).await;

        // Mock read results
        Ok(10)
    }

    /// Executes an UPDATE operation within a transaction
    async fn execute_update_in_tx(
        &self,
        tx: &mut Transaction<'_, Postgres>,
        op: &DataUpdate,
    ) -> Result<u64> {
        println!("Executing UPDATE for asset '{}' in transaction", op.asset);

        // Simulate some work
        tokio::time::sleep(Duration::from_millis(15)).await;

        // Mock successful update
        Ok(5)
    }

    /// Executes a DELETE operation within a transaction
    async fn execute_delete_in_tx(
        &self,
        tx: &mut Transaction<'_, Postgres>,
        op: &DataDelete,
    ) -> Result<u64> {
        println!("Executing DELETE for asset '{}' in transaction", op.asset);

        // Simulate some work
        tokio::time::sleep(Duration::from_millis(20)).await;

        // Mock successful delete
        Ok(3)
    }

    /// Validates a batch operation before execution
    fn validate_batch(&self, batch: &BatchOperation) -> Result<()> {
        if batch.operations.is_empty() {
            return Err(anyhow!("Batch operation cannot be empty"));
        }

        if batch.operations.len() > self.config.max_batch_size {
            return Err(anyhow!(
                "Batch size {} exceeds maximum allowed size {}",
                batch.operations.len(),
                self.config.max_batch_size
            ));
        }

        if self.active_transactions.len() >= self.config.max_concurrent_transactions {
            return Err(anyhow!(
                "Maximum concurrent transactions ({}) exceeded",
                self.config.max_concurrent_transactions
            ));
        }

        Ok(())
    }

    /// Updates transaction progress
    fn update_transaction_progress(&mut self, transaction_id: &str, completed_count: usize) {
        if let Some(info) = self.active_transactions.get_mut(transaction_id) {
            info.completed_count = completed_count;
        }
    }

    /// Updates transaction status
    fn update_transaction_status(&mut self, transaction_id: &str, status: TransactionStatus) {
        if let Some(info) = self.active_transactions.get_mut(transaction_id) {
            info.status = status;
        }
    }

    /// Gets information about an active transaction
    pub fn get_transaction_info(&self, transaction_id: &str) -> Option<&TransactionInfo> {
        self.active_transactions.get(transaction_id)
    }

    /// Lists all active transactions
    pub fn list_active_transactions(&self) -> Vec<&TransactionInfo> {
        self.active_transactions.values().collect()
    }

    /// Attempts to cancel a transaction (if possible)
    pub async fn cancel_transaction(&mut self, transaction_id: &str) -> Result<bool> {
        if let Some(info) = self.active_transactions.get_mut(transaction_id) {
            match info.status {
                TransactionStatus::Active => {
                    info.status = TransactionStatus::RollingBack;
                    // In a real implementation, this would signal the transaction to abort
                    println!("Cancelling transaction {}", transaction_id);
                    Ok(true)
                }
                _ => {
                    println!(
                        "Cannot cancel transaction {} in status {:?}",
                        transaction_id, info.status
                    );
                    Ok(false)
                }
            }
        } else {
            Err(anyhow!("Transaction {} not found", transaction_id))
        }
    }

    /// Gets transaction statistics
    pub fn get_transaction_stats(&self) -> TransactionStats {
        let active_count = self
            .active_transactions
            .values()
            .filter(|info| info.status == TransactionStatus::Active)
            .count();

        let completing_count = self
            .active_transactions
            .values()
            .filter(|info| {
                info.status == TransactionStatus::Committing
                    || info.status == TransactionStatus::RollingBack
            })
            .count();

        TransactionStats {
            total_active: self.active_transactions.len(),
            active_executing: active_count,
            active_completing: completing_count,
            average_duration_ms: self.calculate_average_duration(),
        }
    }

    fn calculate_average_duration(&self) -> u64 {
        if self.active_transactions.is_empty() {
            return 0;
        }

        let total_duration: u64 = self
            .active_transactions
            .values()
            .map(|info| info.start_time.elapsed().as_millis() as u64)
            .sum();

        total_duration / self.active_transactions.len() as u64
    }
}

/// Transaction statistics
#[derive(Debug, Clone)]
pub struct TransactionStats {
    pub total_active: usize,
    pub active_executing: usize,
    pub active_completing: usize,
    pub average_duration_ms: u64,
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::{Key, Literal, PropertyMap, Value};
    use sqlx::PgPool;

    async fn create_test_pool() -> PgPool {
        // This would create a test database connection
        // For testing purposes, we'll mock this
        PgPool::connect("postgresql://test:test@localhost/test")
            .await
            .unwrap_or_else(|_| panic!("Failed to create test pool"))
    }

    fn create_test_batch() -> BatchOperation {
        let mut values = PropertyMap::new();
        values.insert(
            Key {
                parts: vec!["name".to_string()],
            },
            Value::Literal(Literal::String("Test".to_string())),
        );

        BatchOperation {
            operations: vec![
                CrudStatement::DataCreate(DataCreate {
                    asset: "cbu".to_string(),
                    values: values.clone(),
                }),
                CrudStatement::DataRead(DataRead {
                    asset: "cbu".to_string(),
                    where_clause: Some(values),
                    select_fields: None,
                }),
            ],
            transaction_mode: TransactionMode::Atomic,
            rollback_strategy: RollbackStrategy::FullRollback,
        }
    }

    #[tokio::test]
    async fn test_transaction_manager_creation() {
        let config = TransactionConfig::default();
        assert_eq!(config.default_timeout_seconds, 300);
        assert_eq!(config.max_batch_size, 1000);
    }

    #[test]
    fn test_batch_validation() {
        let pool = PgPool::connect("postgresql://test:test@localhost/test")
            .await
            .unwrap_or_else(|_| {
                // Return early if we can't connect to test DB
                return;
            });

        let manager = CrudTransactionManager::new(pool);

        // Test empty batch
        let empty_batch = BatchOperation {
            operations: vec![],
            transaction_mode: TransactionMode::Atomic,
            rollback_strategy: RollbackStrategy::FullRollback,
        };

        assert!(manager.validate_batch(&empty_batch).is_err());

        // Test valid batch
        let valid_batch = create_test_batch();
        assert!(manager.validate_batch(&valid_batch).is_ok());
    }

    #[test]
    fn test_transaction_info() {
        let info = TransactionInfo {
            id: "test".to_string(),
            start_time: Instant::now(),
            operations_count: 5,
            completed_count: 2,
            status: TransactionStatus::Active,
            rollback_strategy: RollbackStrategy::FullRollback,
        };

        assert_eq!(info.operations_count, 5);
        assert_eq!(info.completed_count, 2);
        assert_eq!(info.status, TransactionStatus::Active);
    }

    #[test]
    fn test_operation_result() {
        let result = OperationResult {
            operation_index: 0,
            success: true,
            affected_rows: 1,
            duration_ms: 150,
            error_message: None,
        };

        assert!(result.success);
        assert_eq!(result.affected_rows, 1);
        assert!(result.error_message.is_none());
    }
}
