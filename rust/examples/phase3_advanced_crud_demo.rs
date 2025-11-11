//! Phase 3 Advanced CRUD Operations Demo
//!
//! This demo showcases the advanced CRUD capabilities implemented in Phase 3:
//! - Complex queries with joins, filters, and aggregations
//! - Conditional update operations
//! - Batch operations with transaction management
//! - Comprehensive validation and safety checks
//! - Operation simulation and integrity checking

use anyhow::Result;
use ob_poc::ai::{AgenticCrudRequest, AgenticCrudService};
use ob_poc::services::{CrudValidator, ValidatorConfig};
use ob_poc::{
    AggregateClause, AggregateFunction, AggregateOperation, BatchOperation, ComplexQuery,
    ConditionalUpdate, CrudStatement, CrudTransaction, DataCreate, DataRead, DataUpdate,
    JoinClause, JoinType, Key, Literal, OrderClause, OrderDirection, PropertyMap, RollbackStrategy,
    TransactionMode, ValidationResult, Value,
};
use serde_json;
use std::collections::HashMap;
use uuid::Uuid;

#[tokio::main]
async fn main() -> Result<()> {
    println!("🚀 Phase 3 Advanced CRUD Operations Demo");
    println!("========================================\n");

    // Initialize services
    let validator = CrudValidator::with_config(ValidatorConfig {
        strict_mode: false,
        max_bulk_records: 500,
        check_referential_integrity: true,
        check_permissions: true,
        simulation_timeout_seconds: 30,
    });

    let agentic_service = AgenticCrudService::with_mock();

    println!("✅ Services initialized\n");

    // Demo 1: Complex Query Operations
    demo_complex_queries(&validator, &agentic_service).await?;

    // Demo 2: Conditional Update Operations
    demo_conditional_updates(&validator, &agentic_service).await?;

    // Demo 3: Batch Operations with Transactions
    demo_batch_operations(&validator, &agentic_service).await?;

    // Demo 4: Validation and Safety Systems
    demo_validation_systems(&validator).await?;

    // Demo 5: AI-Powered Advanced Operations
    demo_ai_advanced_operations(&agentic_service).await?;

    println!("\n🎉 Phase 3 Advanced CRUD Demo completed successfully!");
    println!("All advanced operations validated and demonstrated.");

    Ok(())
}

/// Demonstrates complex query operations with joins, filters, and aggregations
async fn demo_complex_queries(
    validator: &CrudValidator,
    agentic_service: &AgenticCrudService,
) -> Result<()> {
    println!("📊 Demo 1: Complex Query Operations");
    println!("-----------------------------------");

    // Example 1: Multi-table join with filters
    let join_query = ComplexQuery {
        asset: "cbu".to_string(),
        joins: Some(vec![
            JoinClause {
                join_type: JoinType::Left,
                target_asset: "entities".to_string(),
                on_condition: {
                    let mut on_map = PropertyMap::new();
                    on_map.insert(
                        Key {
                            parts: vec!["cbu_id".to_string()],
                        },
                        Value::Literal(Literal::String("entities.parent_cbu_id".to_string())),
                    );
                    on_map
                },
            },
            JoinClause {
                join_type: JoinType::Inner,
                target_asset: "documents".to_string(),
                on_condition: {
                    let mut on_map = PropertyMap::new();
                    on_map.insert(
                        Key {
                            parts: vec!["entity_id".to_string()],
                        },
                        Value::Literal(Literal::String("documents.entity_id".to_string())),
                    );
                    on_map
                },
            },
        ]),
        filters: Some({
            let mut filters = PropertyMap::new();
            filters.insert(
                Key {
                    parts: vec!["created_after".to_string()],
                },
                Value::Literal(Literal::String("2024-01-01".to_string())),
            );
            filters.insert(
                Key {
                    parts: vec!["jurisdiction".to_string()],
                },
                Value::Literal(Literal::String("US".to_string())),
            );
            filters
        }),
        aggregate: Some(AggregateClause {
            operations: vec![
                AggregateOperation {
                    function: AggregateFunction::Count,
                    field: "*".to_string(),
                    alias: Some("total_records".to_string()),
                },
                AggregateOperation {
                    function: AggregateFunction::Sum,
                    field: "aum".to_string(),
                    alias: Some("total_aum".to_string()),
                },
                AggregateOperation {
                    function: AggregateFunction::CountDistinct,
                    field: "entity_type".to_string(),
                    alias: Some("unique_entity_types".to_string()),
                },
            ],
            group_by: Some(vec!["jurisdiction".to_string(), "entity_type".to_string()]),
            having: Some({
                let mut having = PropertyMap::new();
                having.insert(
                    Key {
                        parts: vec!["total_aum".to_string()],
                    },
                    Value::Literal(Literal::String("> 1000000".to_string())),
                );
                having
            }),
        }),
        select_fields: Some(vec![
            Value::Literal(Literal::String("cbu.name".to_string())),
            Value::Literal(Literal::String("entities.legal_name".to_string())),
            Value::Literal(Literal::String("total_aum".to_string())),
        ]),
        order_by: Some(vec![
            OrderClause {
                field: "total_aum".to_string(),
                direction: OrderDirection::Desc,
            },
            OrderClause {
                field: "cbu.name".to_string(),
                direction: OrderDirection::Asc,
            },
        ]),
        limit: Some(100),
        offset: Some(0),
    };

    let complex_statement = CrudStatement::ComplexQuery(join_query);

    // Validate the complex query
    let validation_result = validator.validate_operation(&complex_statement);
    println!(
        "   Complex query validation: {}",
        if validation_result.is_valid {
            "✅ PASSED"
        } else {
            "❌ FAILED"
        }
    );

    if !validation_result.errors.is_empty() {
        println!("   Validation errors: {:?}", validation_result.errors);
    }

    if !validation_result.suggestions.is_empty() {
        println!("   Suggestions: {:?}", validation_result.suggestions);
    }

    // Simulate the operation
    let simulation = validator.simulate_operation(&complex_statement);
    println!("   Simulation result:");
    println!("     - Would succeed: {}", simulation.would_succeed);
    println!(
        "     - Estimated affected records: {}",
        simulation.affected_records
    );
    println!(
        "     - Estimated duration: {}ms",
        simulation.estimated_duration_ms
    );
    println!(
        "     - Memory usage: {}KB",
        simulation.resource_usage.memory_kb
    );

    // Example 2: Aggregation-focused query
    let aggregation_query = ComplexQuery {
        asset: "attribute_values".to_string(),
        joins: None,
        filters: Some({
            let mut filters = PropertyMap::new();
            filters.insert(
                Key {
                    parts: vec!["attribute_type".to_string()],
                },
                Value::Literal(Literal::String("financial".to_string())),
            );
            filters
        }),
        aggregate: Some(AggregateClause {
            operations: vec![
                AggregateOperation {
                    function: AggregateFunction::Avg,
                    field: "numeric_value".to_string(),
                    alias: Some("average_value".to_string()),
                },
                AggregateOperation {
                    function: AggregateFunction::Min,
                    field: "numeric_value".to_string(),
                    alias: Some("min_value".to_string()),
                },
                AggregateOperation {
                    function: AggregateFunction::Max,
                    field: "numeric_value".to_string(),
                    alias: Some("max_value".to_string()),
                },
            ],
            group_by: Some(vec!["cbu_id".to_string()]),
            having: None,
        }),
        select_fields: None,
        order_by: None,
        limit: None,
        offset: None,
    };

    let aggregation_statement = CrudStatement::ComplexQuery(aggregation_query);
    let agg_validation = validator.validate_operation(&aggregation_statement);
    println!(
        "   Aggregation query validation: {}",
        if agg_validation.is_valid {
            "✅ PASSED"
        } else {
            "❌ FAILED"
        }
    );

    println!("   ✅ Complex queries demonstrated\n");
    Ok(())
}

/// Demonstrates conditional update operations
async fn demo_conditional_updates(
    validator: &CrudValidator,
    agentic_service: &AgenticCrudService,
) -> Result<()> {
    println!("🔄 Demo 2: Conditional Update Operations");
    println!("---------------------------------------");

    // Example 1: Update with existence condition
    let conditional_update = ConditionalUpdate {
        asset: "cbu".to_string(),
        where_clause: {
            let mut where_map = PropertyMap::new();
            where_map.insert(
                Key {
                    parts: vec!["jurisdiction".to_string()],
                },
                Value::Literal(Literal::String("US".to_string())),
            );
            where_map.insert(
                Key {
                    parts: vec!["status".to_string()],
                },
                Value::Literal(Literal::String("pending".to_string())),
            );
            where_map
        },
        if_exists: Some({
            let mut if_exists = PropertyMap::new();
            if_exists.insert(
                Key {
                    parts: vec!["kyc_status".to_string()],
                },
                Value::Literal(Literal::String("approved".to_string())),
            );
            if_exists.insert(
                Key {
                    parts: vec!["documentation_complete".to_string()],
                },
                Value::Literal(Literal::String("true".to_string())),
            );
            if_exists
        }),
        if_not_exists: None,
        set_values: {
            let mut set_values = PropertyMap::new();
            set_values.insert(
                Key {
                    parts: vec!["status".to_string()],
                },
                Value::Literal(Literal::String("active".to_string())),
            );
            set_values.insert(
                Key {
                    parts: vec!["activation_date".to_string()],
                },
                Value::Literal(Literal::String("NOW()".to_string())),
            );
            set_values.insert(
                Key {
                    parts: vec!["updated_by".to_string()],
                },
                Value::Literal(Literal::String("system".to_string())),
            );
            set_values
        },
        increment_values: Some({
            let mut increment = PropertyMap::new();
            increment.insert(
                Key {
                    parts: vec!["activation_count".to_string()],
                },
                Value::Literal(Literal::Integer(1)),
            );
            increment
        }),
    };

    let conditional_statement = CrudStatement::ConditionalUpdate(conditional_update);

    // Validate the conditional update
    let validation_result = validator.validate_operation(&conditional_statement);
    println!(
        "   Conditional update validation: {}",
        if validation_result.is_valid {
            "✅ PASSED"
        } else {
            "❌ FAILED"
        }
    );

    // Check referential integrity
    let integrity_result = validator.validate_referential_integrity(&conditional_statement);
    println!(
        "   Referential integrity check: {}",
        if integrity_result.referential_integrity_ok {
            "✅ PASSED"
        } else {
            "❌ FAILED"
        }
    );

    // Simulate the operation
    let simulation = validator.simulate_operation(&conditional_statement);
    println!("   Simulation result:");
    println!("     - Would succeed: {}", simulation.would_succeed);
    println!(
        "     - Estimated affected records: {}",
        simulation.affected_records
    );

    if !simulation.potential_issues.is_empty() {
        println!("     - Potential issues: {:?}", simulation.potential_issues);
    }

    // Example 2: Conditional update with increment operations
    let increment_update = ConditionalUpdate {
        asset: "account_balances".to_string(),
        where_clause: {
            let mut where_map = PropertyMap::new();
            where_map.insert(
                Key {
                    parts: vec!["account_id".to_string()],
                },
                Value::Literal(Literal::String("ACC-001".to_string())),
            );
            where_map
        },
        if_exists: Some({
            let mut if_exists = PropertyMap::new();
            if_exists.insert(
                Key {
                    parts: vec!["status".to_string()],
                },
                Value::Literal(Literal::String("active".to_string())),
            );
            if_exists
        }),
        if_not_exists: Some({
            let mut if_not_exists = PropertyMap::new();
            if_not_exists.insert(
                Key {
                    parts: vec!["frozen".to_string()],
                },
                Value::Literal(Literal::String("true".to_string())),
            );
            if_not_exists
        }),
        set_values: {
            let mut set_values = PropertyMap::new();
            set_values.insert(
                Key {
                    parts: vec!["last_transaction_date".to_string()],
                },
                Value::Literal(Literal::String("NOW()".to_string())),
            );
            set_values
        },
        increment_values: Some({
            let mut increment = PropertyMap::new();
            increment.insert(
                Key {
                    parts: vec!["balance".to_string()],
                },
                Value::Literal(Literal::Integer(1000)),
            );
            increment.insert(
                Key {
                    parts: vec!["transaction_count".to_string()],
                },
                Value::Literal(Literal::Integer(1)),
            );
            increment
        }),
    };

    let increment_statement = CrudStatement::ConditionalUpdate(increment_update);
    let increment_validation = validator.validate_operation(&increment_statement);
    println!(
        "   Increment update validation: {}",
        if increment_validation.is_valid {
            "✅ PASSED"
        } else {
            "❌ FAILED"
        }
    );

    println!("   ✅ Conditional updates demonstrated\n");
    Ok(())
}

/// Demonstrates batch operations with different transaction modes
async fn demo_batch_operations(
    validator: &CrudValidator,
    agentic_service: &AgenticCrudService,
) -> Result<()> {
    println!("🔄 Demo 3: Batch Operations with Transactions");
    println!("--------------------------------------------");

    // Create sample operations for batch processing
    let mut create_values_1 = PropertyMap::new();
    create_values_1.insert(
        Key {
            parts: vec!["name".to_string()],
        },
        Value::Literal(Literal::String("Batch Client 1".to_string())),
    );
    create_values_1.insert(
        Key {
            parts: vec!["jurisdiction".to_string()],
        },
        Value::Literal(Literal::String("US".to_string())),
    );

    let mut create_values_2 = PropertyMap::new();
    create_values_2.insert(
        Key {
            parts: vec!["name".to_string()],
        },
        Value::Literal(Literal::String("Batch Client 2".to_string())),
    );
    create_values_2.insert(
        Key {
            parts: vec!["jurisdiction".to_string()],
        },
        Value::Literal(Literal::String("UK".to_string())),
    );

    let mut update_where = PropertyMap::new();
    update_where.insert(
        Key {
            parts: vec!["status".to_string()],
        },
        Value::Literal(Literal::String("pending".to_string())),
    );

    let mut update_values = PropertyMap::new();
    update_values.insert(
        Key {
            parts: vec!["status".to_string()],
        },
        Value::Literal(Literal::String("reviewed".to_string())),
    );

    // Example 1: Atomic batch operation
    let atomic_batch = BatchOperation {
        operations: vec![
            CrudStatement::DataCreate(DataCreate {
                asset: "cbu".to_string(),
                values: create_values_1.clone(),
            }),
            CrudStatement::DataCreate(DataCreate {
                asset: "cbu".to_string(),
                values: create_values_2.clone(),
            }),
            CrudStatement::DataUpdate(DataUpdate {
                asset: "cbu".to_string(),
                where_clause: update_where.clone(),
                values: update_values.clone(),
            }),
        ],
        transaction_mode: TransactionMode::Atomic,
        rollback_strategy: RollbackStrategy::FullRollback,
    };

    let atomic_statement = CrudStatement::BatchOperation(atomic_batch);

    // Validate the batch operation
    let validation_result = validator.validate_operation(&atomic_statement);
    println!(
        "   Atomic batch validation: {}",
        if validation_result.is_valid {
            "✅ PASSED"
        } else {
            "❌ FAILED"
        }
    );

    // Simulate the batch operation
    let simulation = validator.simulate_operation(&atomic_statement);
    println!("   Atomic batch simulation:");
    println!("     - Would succeed: {}", simulation.would_succeed);
    println!(
        "     - Estimated affected records: {}",
        simulation.affected_records
    );
    println!(
        "     - Estimated duration: {}ms",
        simulation.estimated_duration_ms
    );
    println!(
        "     - Resource usage: {}KB memory, {} disk ops",
        simulation.resource_usage.memory_kb, simulation.resource_usage.disk_operations
    );

    // Example 2: Sequential batch with continue-on-error strategy
    let sequential_batch = BatchOperation {
        operations: vec![
            CrudStatement::DataRead(DataRead {
                asset: "cbu".to_string(),
                where_clause: Some({
                    let mut where_map = PropertyMap::new();
                    where_map.insert(
                        Key {
                            parts: vec!["jurisdiction".to_string()],
                        },
                        Value::Literal(Literal::String("US".to_string())),
                    );
                    where_map
                }),
                select_fields: Some(vec![
                    Value::Literal(Literal::String("id".to_string())),
                    Value::Literal(Literal::String("name".to_string())),
                ]),
            }),
            CrudStatement::DataCreate(DataCreate {
                asset: "entities".to_string(),
                values: {
                    let mut entity_values = PropertyMap::new();
                    entity_values.insert(
                        Key {
                            parts: vec!["legal_name".to_string()],
                        },
                        Value::Literal(Literal::String("Test Entity".to_string())),
                    );
                    entity_values
                },
            }),
            // This operation might fail, but we want to continue
            CrudStatement::DataUpdate(DataUpdate {
                asset: "nonexistent_table".to_string(),
                where_clause: {
                    let mut where_map = PropertyMap::new();
                    where_map.insert(
                        Key {
                            parts: vec!["id".to_string()],
                        },
                        Value::Literal(Literal::String("123".to_string())),
                    );
                    where_map
                },
                values: {
                    let mut values = PropertyMap::new();
                    values.insert(
                        Key {
                            parts: vec!["status".to_string()],
                        },
                        Value::Literal(Literal::String("updated".to_string())),
                    );
                    values
                },
            }),
        ],
        transaction_mode: TransactionMode::Sequential,
        rollback_strategy: RollbackStrategy::ContinueOnError,
    };

    let sequential_statement = CrudStatement::BatchOperation(sequential_batch);
    let sequential_validation = validator.validate_operation(&sequential_statement);
    println!(
        "   Sequential batch validation: {}",
        if sequential_validation.is_valid {
            "✅ PASSED"
        } else {
            "❌ FAILED"
        }
    );

    let sequential_simulation = validator.simulate_operation(&sequential_statement);
    println!("   Sequential batch simulation:");
    println!(
        "     - Would succeed: {}",
        sequential_simulation.would_succeed
    );
    println!(
        "     - Potential issues: {:?}",
        sequential_simulation.potential_issues
    );

    println!("   ✅ Batch operations demonstrated\n");
    Ok(())
}

/// Demonstrates the validation and safety systems
async fn demo_validation_systems(validator: &CrudValidator) -> Result<()> {
    println!("🛡️  Demo 4: Validation and Safety Systems");
    println!("----------------------------------------");

    // Example 1: Invalid operation (should fail validation)
    let invalid_create = DataCreate {
        asset: "".to_string(),      // Empty asset name
        values: PropertyMap::new(), // Empty values
    };

    let invalid_statement = CrudStatement::DataCreate(invalid_create);
    let validation_result = validator.validate_operation(&invalid_statement);

    println!("   Invalid operation validation:");
    println!("     - Valid: {}", validation_result.is_valid);
    println!("     - Errors: {}", validation_result.errors.len());

    for error in &validation_result.errors {
        println!(
            "       • {} ({}): {}",
            error.severity, error.code, error.message
        );
    }

    // Example 2: Operation with warnings
    let read_without_filters = DataRead {
        asset: "cbu".to_string(),
        where_clause: None,  // No WHERE clause - might be inefficient
        select_fields: None, // No SELECT fields specified
    };

    let read_statement = CrudStatement::DataRead(read_without_filters);
    let read_validation = validator.validate_operation(&read_statement);

    println!("   Read operation validation:");
    println!("     - Valid: {}", read_validation.is_valid);
    println!("     - Warnings: {}", read_validation.warnings.len());
    println!("     - Suggestions: {}", read_validation.suggestions.len());

    for suggestion in &read_validation.suggestions {
        println!("       • {}", suggestion);
    }

    // Example 3: Permission checking
    let delete_operation = CrudStatement::DataDelete(DataDelete {
        asset: "cbu".to_string(),
        where_clause: {
            let mut where_map = PropertyMap::new();
            where_map.insert(
                Key {
                    parts: vec!["id".to_string()],
                },
                Value::Literal(Literal::String("test-id".to_string())),
            );
            where_map
        },
    });

    let has_permission = validator.check_permissions(&delete_operation);
    println!(
        "   Permission check for delete operation: {}",
        if has_permission {
            "✅ ALLOWED"
        } else {
            "❌ DENIED"
        }
    );

    // Example 4: Referential integrity checking
    let integrity_result = validator.validate_referential_integrity(&delete_operation);
    println!("   Referential integrity check:");
    println!("     - OK: {}", integrity_result.referential_integrity_ok);
    println!(
        "     - Constraint violations: {}",
        integrity_result.constraint_violations.len()
    );
    println!(
        "     - Dependency issues: {}",
        integrity_result.dependency_issues.len()
    );

    // Example 5: Operation simulation
    let complex_update = DataUpdate {
        asset: "cbu".to_string(),
        where_clause: {
            let mut where_map = PropertyMap::new();
            where_map.insert(
                Key {
                    parts: vec!["jurisdiction".to_string()],
                },
                Value::Literal(Literal::String("US".to_string())),
            );
            where_map
        },
        values: {
            let mut values = PropertyMap::new();
            values.insert(
                Key {
                    parts: vec!["compliance_status".to_string()],
                },
                Value::Literal(Literal::String("reviewed".to_string())),
            );
            values
        },
    };

    let update_statement = CrudStatement::DataUpdate(complex_update);
    let simulation = validator.simulate_operation(&update_statement);

    println!("   Operation simulation:");
    println!("     - Would succeed: {}", simulation.would_succeed);
    println!("     - Affected records: {}", simulation.affected_records);
    println!(
        "     - Duration estimate: {}ms",
        simulation.estimated_duration_ms
    );
    println!("     - Resource usage:");
    println!("       • Memory: {}KB", simulation.resource_usage.memory_kb);
    println!(
        "       • Disk operations: {}",
        simulation.resource_usage.disk_operations
    );
    println!(
        "       • Network calls: {}",
        simulation.resource_usage.network_calls
    );
    println!(
        "       • CPU time: {}ms",
        simulation.resource_usage.cpu_time_ms
    );

    println!("   ✅ Validation systems demonstrated\n");
    Ok(())
}

/// Demonstrates AI-powered advanced operations
async fn demo_ai_advanced_operations(agentic_service: &AgenticCrudService) -> Result<()> {
    println!("🤖 Demo 5: AI-Powered Advanced Operations");
    println!("----------------------------------------");

    // Example 1: Complex query generation
    let complex_query_request = AgenticCrudRequest {
        instruction: "Find all US-based CBUs with their related entities and documents, group by jurisdiction and entity type, showing total AUM greater than $1M".to_string(),
        context_hints: Some(vec![
            "complex query".to_string(),
            "joins".to_string(),
            "aggregation".to_string(),
        ]),
        execute: false,
        request_id: Some("complex-query-demo".to_string()),
    };

    let complex_response = agentic_service.process_request(complex_query_request)?;
    println!("   Complex query generation:");
    println!("     - Success: {}", complex_response.success);
    println!("     - Generated DSL: {}", complex_response.generated_dsl);
    println!(
        "     - AI generation time: {}ms",
        complex_response.generation_metadata.ai_generation_time_ms
    );
    println!(
        "     - RAG confidence: {:.2}",
        complex_response.rag_context.confidence_score
    );

    // Example 2: Batch operation generation
    let batch_request = AgenticCrudRequest {
        instruction: "Create a batch operation to onboard 3 new hedge fund clients with proper KYC validation and document requirements".to_string(),
        context_hints: Some(vec![
            "batch".to_string(),
            "transaction".to_string(),
            "hedge fund".to_string(),
        ]),
        execute: false,
        request_id: Some("batch-demo".to_string()),
    };

    let batch_response = agentic_service.process_request(batch_request)?;
    println!("   Batch operation generation:");
    println!("     - Success: {}", batch_response.success);
    println!("     - Generated DSL: {}", batch_response.generated_dsl);
    if let Some(parsed) = &batch_response.parsed_statement {
        println!("     - Parsed successfully: ✅");
        match parsed {
            CrudStatement::BatchOperation(batch) => {
                println!("     - Operations count: {}", batch.operations.len());
                println!("     - Transaction mode: {:?}", batch.transaction_mode);
                println!("     - Rollback strategy: {:?}", batch.rollback_strategy);
            }
            _ => println!("     - Not a batch operation"),
        }
    }

    // Example 3: Conditional update generation
    let conditional_request = AgenticCrudRequest {
        instruction: "Update all pending CBUs to active status, but only if their KYC is approved and documentation is complete, and increment their activation counter".to_string(),
        context_hints: Some(vec![
            "conditional".to_string(),
            "update".to_string(),
            "increment".to_string(),
        ]),
        execute: false,
        request_id: Some("conditional-demo".to_string()),
    };

    let conditional_response = agentic_service.process_request(conditional_request)?;
    println!("   Conditional update generation:");
    println!("     - Success: {}", conditional_response.success);
    println!(
        "     - Generated DSL: {}",
        conditional_response.generated_dsl
    );

    // Example 4: Validation of AI-generated operations
    if let Some(parsed_statement) = &complex_response.parsed_statement {
        let validator = CrudValidator::new();
        let validation = validator.validate_operation(parsed_statement);
        println!("   AI-generated operation validation:");
        println!("     - Valid: {}", validation.is_valid);
        if !validation.errors.is_empty() {
            println!("     - Errors: {:?}", validation.errors);
        }
        if !validation.suggestions.is_empty() {
            println!("     - Suggestions: {:?}", validation.suggestions);
        }
    }

    // Example 5: Performance metrics
    println!("   AI Service performance metrics:");
    println!(
        "     - Available assets: {}",
        agentic_service.get_available_assets().len()
    );

    let creation_examples = agentic_service.get_examples_by_category("creation");
    println!("     - Creation examples: {}", creation_examples.len());

    let query_examples = agentic_service.get_examples_by_category("query");
    println!("     - Query examples: {}", query_examples.len());

    println!("   ✅ AI-powered operations demonstrated\n");
    Ok(())
}

/// Creates sample data for demonstrations
fn create_sample_property_map(fields: Vec<(&str, &str)>) -> PropertyMap {
    let mut map = PropertyMap::new();
    for (key, value) in fields {
        map.insert(
            Key {
                parts: vec![key.to_string()],
            },
            Value::Literal(Literal::String(value.to_string())),
        );
    }
    map
}

#[cfg(test)]
mod tests {
    use super::*;

    #[tokio::test]
    async fn test_demo_functions() {
        let validator = CrudValidator::new();
        let service = AgenticCrudService::with_mock();

        // Test that demo functions don't panic
        assert!(demo_complex_queries(&validator, &service).await.is_ok());
        assert!(demo_conditional_updates(&validator, &service).await.is_ok());
        assert!(demo_batch_operations(&validator, &service).await.is_ok());
        assert!(demo_validation_systems(&validator).await.is_ok());
        assert!(demo_ai_advanced_operations(&service).await.is_ok());
    }

    #[test]
    fn test_sample_property_map() {
        let map = create_sample_property_map(vec![("name", "Test"), ("jurisdiction", "US")]);

        assert_eq!(map.len(), 2);
        assert!(map.contains_key(&Key {
            parts: vec!["name".to_string()]
        }));
    }
}
