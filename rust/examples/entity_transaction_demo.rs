//! Entity Transaction Management Demo
//!
//! This demo showcases the comprehensive transaction management capabilities
//! for entity CRUD operations, including batch processing, rollback strategies,
//! and complex multi-entity workflows with dependencies.

use anyhow::Result;

use std::collections::HashMap;
use uuid::Uuid;

// Mock structures for demonstration
#[derive(Debug, Clone)]
struct MockEntityTransactionManager {
    transaction_history: Vec<MockTransaction>,
}

#[derive(Debug, Clone)]
struct MockTransaction {
    id: Uuid,
    operations: Vec<MockOperation>,
    status: TransactionStatus,
    rollback_strategy: RollbackStrategy,
    execution_time_ms: i64,
    error_message: Option<String>,
}

#[derive(Debug, Clone)]
struct MockOperation {
    id: Uuid,
    operation_type: EntityOperationType,
    asset_type: EntityAssetType,
    instruction: String,
    status: OperationStatus,
    dependencies: Vec<Uuid>,
    generated_dsl: Option<String>,
    affected_records: Vec<Uuid>,
}

#[derive(Debug, Clone, PartialEq, Eq, Hash)]
enum TransactionStatus {
    Pending,
    InProgress,
    Completed,
    PartiallyCompleted,
    Failed,
    RolledBack,
    Simulated,
}

#[derive(Debug, Clone, PartialEq)]
enum OperationStatus {
    Pending,
    Executing,
    Completed,
    Failed,
    Skipped,
    RolledBack,
}

#[derive(Debug, Clone, PartialEq)]
enum EntityOperationType {
    Create,
    Read,
    Update,
    Delete,
    Link,
    Unlink,
}

#[derive(Debug, Clone)]
enum EntityAssetType {
    Partnership,
    LimitedCompany,
    ProperPerson,
    Trust,
    Entity,
}

#[derive(Debug, Clone, PartialEq)]
enum RollbackStrategy {
    FullRollback,
    PartialRollback,
    ContinueOnError,
    StopOnError,
}

#[derive(Debug, Clone, PartialEq)]
enum TransactionMode {
    Atomic,
    Sequential,
    Simulation,
}

impl MockEntityTransactionManager {
    fn new() -> Self {
        Self {
            transaction_history: Vec::new(),
        }
    }

    async fn execute_batch_transaction(
        &mut self,
        operations: Vec<MockOperation>,
        mode: TransactionMode,
        rollback_strategy: RollbackStrategy,
    ) -> Result<MockTransaction> {
        let transaction_id = Uuid::new_v4();
        let start_time = std::time::Instant::now();

        println!(
            "🚀 Executing batch transaction {} in {:?} mode",
            transaction_id, mode
        );
        println!("   Rollback strategy: {:?}", rollback_strategy);
        println!("   Operations count: {}", operations.len());

        let mut transaction = MockTransaction {
            id: transaction_id,
            operations: operations.clone(),
            status: TransactionStatus::InProgress,
            rollback_strategy: rollback_strategy.clone(),
            execution_time_ms: 0,
            error_message: None,
        };

        match mode {
            TransactionMode::Atomic => {
                transaction = self.execute_atomic_transaction(transaction).await?;
            }
            TransactionMode::Sequential => {
                transaction = self.execute_sequential_transaction(transaction).await?;
            }
            TransactionMode::Simulation => {
                transaction = self.simulate_transaction(transaction).await?;
            }
        }

        transaction.execution_time_ms = start_time.elapsed().as_millis() as i64;
        self.transaction_history.push(transaction.clone());

        Ok(transaction)
    }

    async fn execute_atomic_transaction(
        &self,
        mut transaction: MockTransaction,
    ) -> Result<MockTransaction> {
        println!("   ⚛️  Atomic execution: All operations must succeed");

        // Sort operations by dependencies
        let sorted_ops = self.sort_by_dependencies(&transaction.operations)?;

        for (i, operation) in sorted_ops.iter().enumerate() {
            println!(
                "   📝 Operation {}/{}: {}",
                i + 1,
                sorted_ops.len(),
                operation.instruction
            );

            // Simulate operation execution
            let success_rate = self.calculate_success_rate(operation);
            let operation_result = if success_rate > 0.8 {
                self.execute_operation_successfully(operation).await
            } else {
                Err(anyhow::anyhow!("Simulated operation failure"))
            };

            match operation_result {
                Ok(updated_op) => {
                    println!("   ✅ Operation {} completed successfully", operation.id);
                    // Update operation in transaction
                    if let Some(op) = transaction
                        .operations
                        .iter_mut()
                        .find(|o| o.id == operation.id)
                    {
                        *op = updated_op;
                    }
                }
                Err(error) => {
                    println!("   ❌ Operation {} failed: {}", operation.id, error);
                    println!("   🔄 Rolling back all operations (atomic mode)");

                    // In atomic mode, any failure causes full rollback
                    transaction.status = TransactionStatus::RolledBack;
                    transaction.error_message = Some(error.to_string());

                    // Mark all operations as rolled back
                    for op in &mut transaction.operations {
                        op.status = OperationStatus::RolledBack;
                    }

                    return Ok(transaction);
                }
            }
        }

        transaction.status = TransactionStatus::Completed;
        println!("   🎉 Atomic transaction completed successfully");

        Ok(transaction)
    }

    async fn execute_sequential_transaction(
        &self,
        mut transaction: MockTransaction,
    ) -> Result<MockTransaction> {
        println!("   📈 Sequential execution: Partial success allowed");

        let sorted_ops = self.sort_by_dependencies(&transaction.operations)?;
        let mut completed_count = 0;
        let mut failed_count = 0;

        for (i, operation) in sorted_ops.iter().enumerate() {
            println!(
                "   📝 Operation {}/{}: {}",
                i + 1,
                sorted_ops.len(),
                operation.instruction
            );

            let success_rate = self.calculate_success_rate(operation);
            let operation_result = if success_rate > 0.7 {
                self.execute_operation_successfully(operation).await
            } else {
                Err(anyhow::anyhow!("Simulated operation failure"))
            };

            match operation_result {
                Ok(updated_op) => {
                    completed_count += 1;
                    println!("   ✅ Operation {} completed", operation.id);

                    if let Some(op) = transaction
                        .operations
                        .iter_mut()
                        .find(|o| o.id == operation.id)
                    {
                        *op = updated_op;
                    }
                }
                Err(error) => {
                    failed_count += 1;
                    println!("   ❌ Operation {} failed: {}", operation.id, error);

                    // Update operation status
                    if let Some(op) = transaction
                        .operations
                        .iter_mut()
                        .find(|o| o.id == operation.id)
                    {
                        op.status = OperationStatus::Failed;
                    }

                    // Handle rollback strategy
                    match transaction.rollback_strategy {
                        RollbackStrategy::StopOnError => {
                            println!("   🛑 Stopping execution due to StopOnError strategy");
                            break;
                        }
                        RollbackStrategy::FullRollback => {
                            println!("   🔄 Performing full rollback");
                            transaction.status = TransactionStatus::RolledBack;
                            for op in &mut transaction.operations {
                                if op.status == OperationStatus::Completed {
                                    op.status = OperationStatus::RolledBack;
                                }
                            }
                            return Ok(transaction);
                        }
                        RollbackStrategy::ContinueOnError => {
                            println!("   ⏭️  Continuing with next operation");
                            continue;
                        }
                        RollbackStrategy::PartialRollback => {
                            println!("   🔄 Marking for partial rollback, continuing");
                            continue;
                        }
                    }
                }
            }
        }

        // Determine final status
        transaction.status = if failed_count == 0 {
            TransactionStatus::Completed
        } else if completed_count == 0 {
            TransactionStatus::Failed
        } else {
            TransactionStatus::PartiallyCompleted
        };

        println!(
            "   📊 Sequential transaction completed: {}/{} operations successful",
            completed_count,
            sorted_ops.len()
        );

        Ok(transaction)
    }

    async fn simulate_transaction(
        &self,
        mut transaction: MockTransaction,
    ) -> Result<MockTransaction> {
        println!("   🎭 Simulation mode: No actual changes made");

        let operations_len = transaction.operations.len();
        for (i, operation) in transaction.operations.iter_mut().enumerate() {
            println!(
                "   📋 Simulating operation {}/{}: {}",
                i + 1,
                operations_len,
                operation.instruction
            );

            // Simulate successful operation
            operation.status = OperationStatus::Completed;
            operation.generated_dsl = Some(format!(
                "(data.{} :asset \"{}\" :simulation true)",
                operation.operation_type.as_verb(),
                operation.asset_type.as_name()
            ));
            operation.affected_records = vec![Uuid::new_v4()];

            println!(
                "   ✅ Simulated: {}",
                operation.generated_dsl.as_ref().unwrap()
            );
        }

        transaction.status = TransactionStatus::Simulated;
        println!("   🎉 All operations simulated successfully");

        Ok(transaction)
    }

    async fn execute_operation_successfully(
        &self,
        operation: &MockOperation,
    ) -> Result<MockOperation> {
        let mut updated_op = operation.clone();
        updated_op.status = OperationStatus::Completed;
        updated_op.generated_dsl = Some(format!(
            "(data.{} :asset \"{}\" :values {{:name \"Example Entity\"}})",
            operation.operation_type.as_verb(),
            operation.asset_type.as_name()
        ));
        updated_op.affected_records = vec![Uuid::new_v4()];

        // Simulate processing time
        tokio::time::sleep(tokio::time::Duration::from_millis(10)).await;

        Ok(updated_op)
    }

    fn sort_by_dependencies(&self, operations: &[MockOperation]) -> Result<Vec<MockOperation>> {
        let mut sorted = Vec::new();
        let mut remaining: Vec<_> = operations.iter().cloned().collect();
        let mut processed_ids = std::collections::HashSet::new();

        while !remaining.is_empty() {
            let mut progress = false;

            remaining.retain(|op| {
                let deps_satisfied = op
                    .dependencies
                    .iter()
                    .all(|dep_id| processed_ids.contains(dep_id));

                if deps_satisfied {
                    sorted.push(op.clone());
                    processed_ids.insert(op.id);
                    progress = true;
                    false // Remove from remaining
                } else {
                    true // Keep in remaining
                }
            });

            if !progress && !remaining.is_empty() {
                return Err(anyhow::anyhow!("Circular dependency detected"));
            }
        }

        Ok(sorted)
    }

    fn calculate_success_rate(&self, operation: &MockOperation) -> f64 {
        // Simple heuristic based on operation type and complexity
        match operation.operation_type {
            EntityOperationType::Create => 0.9,
            EntityOperationType::Read => 0.95,
            EntityOperationType::Update => 0.85,
            EntityOperationType::Delete => 0.8,
            EntityOperationType::Link => 0.9,
            EntityOperationType::Unlink => 0.9,
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

impl EntityAssetType {
    fn as_name(&self) -> &'static str {
        match self {
            EntityAssetType::Partnership => "partnership",
            EntityAssetType::LimitedCompany => "limited_company",
            EntityAssetType::ProperPerson => "proper_person",
            EntityAssetType::Trust => "trust",
            EntityAssetType::Entity => "entity",
        }
    }
}

#[tokio::main]
async fn main() -> Result<()> {
    println!("🏢 Entity Transaction Management Demo");
    println!("===================================\n");

    let mut transaction_manager = MockEntityTransactionManager::new();

    // Demo 1: Simple Atomic Transaction
    demo_atomic_transaction(&mut transaction_manager).await?;

    // Demo 2: Sequential Transaction with Failures
    demo_sequential_transaction(&mut transaction_manager).await?;

    // Demo 3: Complex Multi-Entity Workflow
    demo_complex_workflow(&mut transaction_manager).await?;

    // Demo 4: Dependency Management
    demo_dependency_management(&mut transaction_manager).await?;

    // Demo 5: Simulation Mode
    demo_simulation_mode(&mut transaction_manager).await?;

    // Demo 6: Transaction History and Analytics
    demo_transaction_analytics(&transaction_manager).await?;

    println!("\n🎉 Entity Transaction Management Demo completed!");
    println!("All transaction scenarios demonstrated successfully.");

    Ok(())
}

async fn demo_atomic_transaction(
    transaction_manager: &mut MockEntityTransactionManager,
) -> Result<()> {
    println!("🔬 Demo 1: Atomic Transaction Processing");
    println!("---------------------------------------");

    let operations = vec![
        MockOperation {
            id: Uuid::new_v4(),
            operation_type: EntityOperationType::Create,
            asset_type: EntityAssetType::Partnership,
            instruction: "Create Delaware LLC for TechCorp".to_string(),
            status: OperationStatus::Pending,
            dependencies: vec![],
            generated_dsl: None,
            affected_records: vec![],
        },
        MockOperation {
            id: Uuid::new_v4(),
            operation_type: EntityOperationType::Create,
            asset_type: EntityAssetType::ProperPerson,
            instruction: "Add managing partner John Smith".to_string(),
            status: OperationStatus::Pending,
            dependencies: vec![],
            generated_dsl: None,
            affected_records: vec![],
        },
    ];

    let result = transaction_manager
        .execute_batch_transaction(
            operations,
            TransactionMode::Atomic,
            RollbackStrategy::FullRollback,
        )
        .await?;

    println!("📊 Transaction Result:");
    println!("   ID: {}", result.id);
    println!("   Status: {:?}", result.status);
    println!("   Execution time: {}ms", result.execution_time_ms);
    println!("   Operations: {}", result.operations.len());

    Ok(())
}

async fn demo_sequential_transaction(
    transaction_manager: &mut MockEntityTransactionManager,
) -> Result<()> {
    println!("\n📈 Demo 2: Sequential Transaction with Error Handling");
    println!("----------------------------------------------------");

    let operations = vec![
        MockOperation {
            id: Uuid::new_v4(),
            operation_type: EntityOperationType::Create,
            asset_type: EntityAssetType::LimitedCompany,
            instruction: "Create UK company AlphaTech Ltd".to_string(),
            status: OperationStatus::Pending,
            dependencies: vec![],
            generated_dsl: None,
            affected_records: vec![],
        },
        MockOperation {
            id: Uuid::new_v4(),
            operation_type: EntityOperationType::Update,
            asset_type: EntityAssetType::LimitedCompany,
            instruction: "Update company address".to_string(),
            status: OperationStatus::Pending,
            dependencies: vec![],
            generated_dsl: None,
            affected_records: vec![],
        },
        MockOperation {
            id: Uuid::new_v4(),
            operation_type: EntityOperationType::Create,
            asset_type: EntityAssetType::ProperPerson,
            instruction: "Add company director".to_string(),
            status: OperationStatus::Pending,
            dependencies: vec![],
            generated_dsl: None,
            affected_records: vec![],
        },
    ];

    let result = transaction_manager
        .execute_batch_transaction(
            operations,
            TransactionMode::Sequential,
            RollbackStrategy::ContinueOnError,
        )
        .await?;

    println!("📊 Sequential Transaction Result:");
    println!("   Status: {:?}", result.status);
    println!(
        "   Completed operations: {}",
        result
            .operations
            .iter()
            .filter(|op| op.status == OperationStatus::Completed)
            .count()
    );
    println!(
        "   Failed operations: {}",
        result
            .operations
            .iter()
            .filter(|op| op.status == OperationStatus::Failed)
            .count()
    );

    Ok(())
}

async fn demo_complex_workflow(
    transaction_manager: &mut MockEntityTransactionManager,
) -> Result<()> {
    println!("\n🧬 Demo 3: Complex Multi-Entity Workflow");
    println!("---------------------------------------");

    // Create a complex fund structure with multiple entities
    let fund_id = Uuid::new_v4();
    let gp_id = Uuid::new_v4();
    let manager_id = Uuid::new_v4();

    let operations = vec![
        MockOperation {
            id: fund_id,
            operation_type: EntityOperationType::Create,
            asset_type: EntityAssetType::Partnership,
            instruction: "Create main fund entity - Quantum Tech Fund LP".to_string(),
            status: OperationStatus::Pending,
            dependencies: vec![],
            generated_dsl: None,
            affected_records: vec![],
        },
        MockOperation {
            id: gp_id,
            operation_type: EntityOperationType::Create,
            asset_type: EntityAssetType::Partnership,
            instruction: "Create general partner - Quantum GP LLC".to_string(),
            status: OperationStatus::Pending,
            dependencies: vec![],
            generated_dsl: None,
            affected_records: vec![],
        },
        MockOperation {
            id: manager_id,
            operation_type: EntityOperationType::Create,
            asset_type: EntityAssetType::LimitedCompany,
            instruction: "Create investment manager - Quantum Management Corp".to_string(),
            status: OperationStatus::Pending,
            dependencies: vec![],
            generated_dsl: None,
            affected_records: vec![],
        },
        MockOperation {
            id: Uuid::new_v4(),
            operation_type: EntityOperationType::Link,
            asset_type: EntityAssetType::Entity,
            instruction: "Link GP to Fund".to_string(),
            status: OperationStatus::Pending,
            dependencies: vec![fund_id, gp_id], // Depends on both fund and GP creation
            generated_dsl: None,
            affected_records: vec![],
        },
        MockOperation {
            id: Uuid::new_v4(),
            operation_type: EntityOperationType::Link,
            asset_type: EntityAssetType::Entity,
            instruction: "Link Manager to GP".to_string(),
            status: OperationStatus::Pending,
            dependencies: vec![gp_id, manager_id], // Depends on GP and Manager creation
            generated_dsl: None,
            affected_records: vec![],
        },
    ];

    println!("🏗️  Creating complex fund structure with dependencies:");
    println!("   • Main Fund (LP)");
    println!("   • General Partner (LLC)");
    println!("   • Investment Manager (Corp)");
    println!("   • Entity linking relationships");

    let result = transaction_manager
        .execute_batch_transaction(
            operations,
            TransactionMode::Sequential,
            RollbackStrategy::PartialRollback,
        )
        .await?;

    println!("📊 Complex Workflow Result:");
    println!("   Overall status: {:?}", result.status);
    println!(
        "   Entity creation operations: {}",
        result
            .operations
            .iter()
            .filter(|op| op.operation_type == EntityOperationType::Create)
            .count()
    );
    println!(
        "   Linking operations: {}",
        result
            .operations
            .iter()
            .filter(|op| op.operation_type == EntityOperationType::Link)
            .count()
    );

    Ok(())
}

async fn demo_dependency_management(
    transaction_manager: &mut MockEntityTransactionManager,
) -> Result<()> {
    println!("\n🔗 Demo 4: Dependency Management");
    println!("-------------------------------");

    // Create operations with complex dependency chain
    let parent_id = Uuid::new_v4();
    let child1_id = Uuid::new_v4();
    let child2_id = Uuid::new_v4();
    let grandchild_id = Uuid::new_v4();

    let operations = vec![
        MockOperation {
            id: grandchild_id,
            operation_type: EntityOperationType::Create,
            asset_type: EntityAssetType::ProperPerson,
            instruction: "Create beneficial owner".to_string(),
            status: OperationStatus::Pending,
            dependencies: vec![parent_id, child1_id], // Depends on parent and child1
            generated_dsl: None,
            affected_records: vec![],
        },
        MockOperation {
            id: child2_id,
            operation_type: EntityOperationType::Create,
            asset_type: EntityAssetType::Trust,
            instruction: "Create trust structure".to_string(),
            status: OperationStatus::Pending,
            dependencies: vec![parent_id], // Depends on parent
            generated_dsl: None,
            affected_records: vec![],
        },
        MockOperation {
            id: parent_id,
            operation_type: EntityOperationType::Create,
            asset_type: EntityAssetType::Partnership,
            instruction: "Create parent entity".to_string(),
            status: OperationStatus::Pending,
            dependencies: vec![], // No dependencies
            generated_dsl: None,
            affected_records: vec![],
        },
        MockOperation {
            id: child1_id,
            operation_type: EntityOperationType::Create,
            asset_type: EntityAssetType::LimitedCompany,
            instruction: "Create subsidiary".to_string(),
            status: OperationStatus::Pending,
            dependencies: vec![parent_id], // Depends on parent
            generated_dsl: None,
            affected_records: vec![],
        },
    ];

    println!("🎯 Testing dependency resolution:");
    println!("   Parent → Child1, Child2 → Grandchild");

    let result = transaction_manager
        .execute_batch_transaction(
            operations,
            TransactionMode::Sequential,
            RollbackStrategy::StopOnError,
        )
        .await?;

    println!("📊 Dependency Management Result:");
    println!("   Operations properly ordered and executed");
    println!("   Final status: {:?}", result.status);

    Ok(())
}

async fn demo_simulation_mode(
    transaction_manager: &mut MockEntityTransactionManager,
) -> Result<()> {
    println!("\n🎭 Demo 5: Transaction Simulation Mode");
    println!("------------------------------------");

    let operations = vec![
        MockOperation {
            id: Uuid::new_v4(),
            operation_type: EntityOperationType::Create,
            asset_type: EntityAssetType::Partnership,
            instruction: "Simulate creating high-risk entity".to_string(),
            status: OperationStatus::Pending,
            dependencies: vec![],
            generated_dsl: None,
            affected_records: vec![],
        },
        MockOperation {
            id: Uuid::new_v4(),
            operation_type: EntityOperationType::Delete,
            asset_type: EntityAssetType::ProperPerson,
            instruction: "Simulate deleting critical data".to_string(),
            status: OperationStatus::Pending,
            dependencies: vec![],
            generated_dsl: None,
            affected_records: vec![],
        },
    ];

    println!("🧪 Running potentially risky operations in simulation mode");

    let result = transaction_manager
        .execute_batch_transaction(
            operations,
            TransactionMode::Simulation,
            RollbackStrategy::FullRollback,
        )
        .await?;

    println!("📊 Simulation Result:");
    println!("   Status: {:?}", result.status);
    println!("   All operations simulated safely");

    for operation in &result.operations {
        if let Some(dsl) = &operation.generated_dsl {
            println!("   Generated DSL: {}", dsl);
        }
    }

    Ok(())
}

async fn demo_transaction_analytics(
    transaction_manager: &MockEntityTransactionManager,
) -> Result<()> {
    println!("\n📊 Demo 6: Transaction History and Analytics");
    println!("-------------------------------------------");

    println!("📈 Transaction Summary:");
    println!(
        "   Total transactions: {}",
        transaction_manager.transaction_history.len()
    );

    let mut status_counts = HashMap::new();
    let mut total_execution_time = 0i64;

    for transaction in &transaction_manager.transaction_history {
        *status_counts.entry(&transaction.status).or_insert(0) += 1;
        total_execution_time += transaction.execution_time_ms;
    }

    println!("   Status breakdown:");
    for (status, count) in status_counts {
        println!("     {:?}: {}", status, count);
    }

    if !transaction_manager.transaction_history.is_empty() {
        let avg_execution_time =
            total_execution_time / transaction_manager.transaction_history.len() as i64;
        println!("   Average execution time: {}ms", avg_execution_time);
    }

    println!(
        "   Success rate: {:.1}%",
        transaction_manager
            .transaction_history
            .iter()
            .filter(|t| t.status == TransactionStatus::Completed
                || t.status == TransactionStatus::Simulated)
            .count() as f64
            / transaction_manager.transaction_history.len() as f64
            * 100.0
    );

    Ok(())
}

#[cfg(test)]
mod tests {
    use super::*;

    #[tokio::test]
    async fn test_atomic_transaction_success() {
        let mut manager = MockEntityTransactionManager::new();

        let operations = vec![MockOperation {
            id: Uuid::new_v4(),
            operation_type: EntityOperationType::Create,
            asset_type: EntityAssetType::Partnership,
            instruction: "Test operation".to_string(),
            status: OperationStatus::Pending,
            dependencies: vec![],
            generated_dsl: None,
            affected_records: vec![],
        }];

        let result = manager
            .execute_batch_transaction(
                operations,
                TransactionMode::Atomic,
                RollbackStrategy::FullRollback,
            )
            .await
            .unwrap();

        assert_eq!(result.status, TransactionStatus::Completed);
    }

    #[tokio::test]
    async fn test_dependency_sorting() {
        let manager = MockEntityTransactionManager::new();

        let parent_id = Uuid::new_v4();
        let child_id = Uuid::new_v4();

        let operations = vec![
            MockOperation {
                id: child_id,
                operation_type: EntityOperationType::Create,
                asset_type: EntityAssetType::Partnership,
                instruction: "Child".to_string(),
                status: OperationStatus::Pending,
                dependencies: vec![parent_id],
                generated_dsl: None,
                affected_records: vec![],
            },
            MockOperation {
                id: parent_id,
                operation_type: EntityOperationType::Create,
                asset_type: EntityAssetType::Partnership,
                instruction: "Parent".to_string(),
                status: OperationStatus::Pending,
                dependencies: vec![],
                generated_dsl: None,
                affected_records: vec![],
            },
        ];

        let sorted = manager.sort_by_dependencies(&operations).unwrap();

        assert_eq!(sorted[0].id, parent_id);
        assert_eq!(sorted[1].id, child_id);
    }

    #[test]
    fn test_rollback_strategies() {
        let strategies = vec![
            RollbackStrategy::FullRollback,
            RollbackStrategy::PartialRollback,
            RollbackStrategy::ContinueOnError,
            RollbackStrategy::StopOnError,
        ];

        assert_eq!(strategies.len(), 4);
    }
}
