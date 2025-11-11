//! Phase 3 Core Demo - Advanced CRUD Operations (No Database Required)
//!
//! This demo showcases the advanced CRUD capabilities implemented in Phase 3:
//! - Complex query structures and parsing
//! - Conditional update operations
//! - Batch operation definitions
//! - Comprehensive validation systems
//! - AI-powered DSL generation for advanced operations

use anyhow::Result;
use ob_poc::ai::agentic_crud_service::{AgenticCrudRequest, AgenticCrudService};
use ob_poc::parser::idiomatic_parser::parse_crud_statement;
use ob_poc::{
    AggregateClause, AggregateFunction, AggregateOperation, BatchOperation, ComplexQuery,
    ConditionalUpdate, CrudStatement, DataCreate, DataRead, DataUpdate, JoinClause, JoinType, Key,
    Literal, OrderClause, OrderDirection, PropertyMap, RollbackStrategy, TransactionMode, Value,
};

#[tokio::main]
async fn main() -> Result<()> {
    println!("🚀 Phase 3 Core CRUD Operations Demo");
    println!("=====================================\n");

    // Initialize AI service
    let agentic_service = AgenticCrudService::with_mock();
    println!("✅ AI Service initialized\n");

    // Demo 1: Complex Query Structures
    demo_complex_query_structures().await?;

    // Demo 2: Conditional Update Operations
    demo_conditional_update_operations().await?;

    // Demo 3: Batch Operations Structure
    demo_batch_operations_structure().await?;

    // Demo 4: DSL Parsing for Advanced Operations
    demo_advanced_dsl_parsing().await?;

    // Demo 5: AI-Generated Advanced Operations
    demo_ai_advanced_generation(&agentic_service).await?;

    println!("\n🎉 Phase 3 Core Demo completed successfully!");
    println!("All advanced CRUD structures validated and demonstrated.");

    Ok(())
}

/// Demonstrates complex query data structures
async fn demo_complex_query_structures() -> Result<()> {
    println!("📊 Demo 1: Complex Query Structures");
    println!("-----------------------------------");

    // Example 1: Multi-table join with aggregation
    let complex_query = ComplexQuery {
        asset: "cbu".to_string(),
        joins: Some(vec![
            JoinClause {
                join_type: JoinType::Left,
                target_asset: "entities".to_string(),
                on_condition: create_property_map(vec![("cbu_id", "entities.parent_cbu_id")]),
            },
            JoinClause {
                join_type: JoinType::Inner,
                target_asset: "documents".to_string(),
                on_condition: create_property_map(vec![("entity_id", "documents.entity_id")]),
            },
        ]),
        filters: Some(create_property_map(vec![
            ("created_after", "2024-01-01"),
            ("jurisdiction", "US"),
            ("status", "active"),
        ])),
        aggregate: Some(AggregateClause {
            operations: vec![
                AggregateOperation {
                    function: AggregateFunction::Count,
                    field: "*".to_string(),
                    alias: Some("total_count".to_string()),
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
            having: Some(create_property_map(vec![("total_aum", "> 1000000")])),
        }),
        select_fields: Some(vec![
            Value::Literal(Literal::String("cbu.name".to_string())),
            Value::Literal(Literal::String("entities.legal_name".to_string())),
            Value::Literal(Literal::String("total_aum".to_string())),
            Value::Literal(Literal::String("unique_entity_types".to_string())),
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

    println!("   ✅ Complex query structure created:");
    println!("     - Asset: {}", complex_query.asset);
    println!(
        "     - Joins: {} tables",
        complex_query.joins.as_ref().unwrap().len()
    );
    println!(
        "     - Filters: {} conditions",
        complex_query.filters.as_ref().unwrap().len()
    );
    println!(
        "     - Aggregations: {} operations",
        complex_query.aggregate.as_ref().unwrap().operations.len()
    );
    println!(
        "     - Order by: {} fields",
        complex_query.order_by.as_ref().unwrap().len()
    );
    println!("     - Limit: {}", complex_query.limit.unwrap());

    // Validate structure integrity
    validate_complex_query(&complex_query)?;

    // Example 2: Analytics-focused query
    let analytics_query = ComplexQuery {
        asset: "attribute_values".to_string(),
        joins: None,
        filters: Some(create_property_map(vec![
            ("attribute_type", "financial"),
            ("value_date", ">= '2024-01-01'"),
        ])),
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
            group_by: Some(vec!["cbu_id".to_string(), "attribute_id".to_string()]),
            having: None,
        }),
        select_fields: None,
        order_by: Some(vec![OrderClause {
            field: "average_value".to_string(),
            direction: OrderDirection::Desc,
        }]),
        limit: Some(50),
        offset: None,
    };

    println!("   ✅ Analytics query structure created:");
    println!(
        "     - Aggregation functions: {}",
        analytics_query.aggregate.as_ref().unwrap().operations.len()
    );
    println!(
        "     - Group by fields: {}",
        analytics_query
            .aggregate
            .as_ref()
            .unwrap()
            .group_by
            .as_ref()
            .unwrap()
            .len()
    );

    println!("   ✅ Complex query structures demonstrated\n");
    Ok(())
}

/// Demonstrates conditional update operations
async fn demo_conditional_update_operations() -> Result<()> {
    println!("🔄 Demo 2: Conditional Update Operations");
    println!("---------------------------------------");

    // Example 1: Conditional update with existence checks
    let conditional_update = ConditionalUpdate {
        asset: "cbu".to_string(),
        where_clause: create_property_map(vec![
            ("jurisdiction", "US"),
            ("status", "pending_approval"),
        ]),
        if_exists: Some(create_property_map(vec![
            ("kyc_status", "approved"),
            ("documentation_complete", "true"),
            ("compliance_check_passed", "true"),
        ])),
        if_not_exists: Some(create_property_map(vec![
            ("compliance_issues", "true"),
            ("pending_documents", "true"),
        ])),
        set_values: create_property_map(vec![
            ("status", "active"),
            ("activation_date", "NOW()"),
            ("updated_by", "system"),
            ("activation_reason", "automatic_approval"),
        ]),
        increment_values: Some(create_property_map_with_numbers(vec![
            ("activation_count", 1),
            ("total_updates", 1),
        ])),
    };

    println!("   ✅ Conditional update structure created:");
    println!("     - Asset: {}", conditional_update.asset);
    println!(
        "     - Where conditions: {}",
        conditional_update.where_clause.len()
    );
    println!(
        "     - If-exists conditions: {}",
        conditional_update.if_exists.as_ref().unwrap().len()
    );
    println!(
        "     - If-not-exists conditions: {}",
        conditional_update.if_not_exists.as_ref().unwrap().len()
    );
    println!("     - Set values: {}", conditional_update.set_values.len());
    println!(
        "     - Increment values: {}",
        conditional_update.increment_values.as_ref().unwrap().len()
    );

    // Example 2: Balance update with safety conditions
    let balance_update = ConditionalUpdate {
        asset: "account_balances".to_string(),
        where_clause: create_property_map(vec![("account_id", "ACC-12345"), ("currency", "USD")]),
        if_exists: Some(create_property_map(vec![
            ("account_status", "active"),
            ("balance_locked", "false"),
        ])),
        if_not_exists: Some(create_property_map(vec![
            ("frozen", "true"),
            ("suspended", "true"),
        ])),
        set_values: create_property_map(vec![
            ("last_transaction_date", "NOW()"),
            ("last_updated_by", "trading_system"),
        ]),
        increment_values: Some(create_property_map_with_numbers(vec![
            ("balance", 50000),
            ("transaction_count", 1),
            ("daily_transaction_volume", 50000),
        ])),
    };

    println!("   ✅ Balance update structure created:");
    println!(
        "     - Safety conditions: {} if-exists, {} if-not-exists",
        balance_update.if_exists.as_ref().unwrap().len(),
        balance_update.if_not_exists.as_ref().unwrap().len()
    );

    // Validate conditional update logic
    validate_conditional_update(&conditional_update)?;

    println!("   ✅ Conditional update operations demonstrated\n");
    Ok(())
}

/// Demonstrates batch operation structures
async fn demo_batch_operations_structure() -> Result<()> {
    println!("📦 Demo 3: Batch Operations Structure");
    println!("------------------------------------");

    // Example 1: Atomic batch for client onboarding
    let onboarding_batch = BatchOperation {
        operations: vec![
            // Create CBU
            CrudStatement::DataCreate(DataCreate {
                asset: "cbu".to_string(),
                values: create_property_map(vec![
                    ("name", "Hedge Fund Alpha"),
                    ("jurisdiction", "US"),
                    ("entity_type", "HEDGE_FUND"),
                    ("status", "pending"),
                ]),
            }),
            // Create primary entity
            CrudStatement::DataCreate(DataCreate {
                asset: "entities".to_string(),
                values: create_property_map(vec![
                    ("legal_name", "Alpha Investment Management LLC"),
                    ("entity_type", "CORPORATION"),
                    ("incorporation_jurisdiction", "Delaware"),
                ]),
            }),
            // Update CBU with entity reference
            CrudStatement::DataUpdate(DataUpdate {
                asset: "cbu".to_string(),
                where_clause: create_property_map(vec![("name", "Hedge Fund Alpha")]),
                values: create_property_map(vec![
                    ("primary_entity_linked", "true"),
                    ("setup_progress", "entities_created"),
                ]),
            }),
        ],
        transaction_mode: TransactionMode::Atomic,
        rollback_strategy: RollbackStrategy::FullRollback,
    };

    println!("   ✅ Atomic onboarding batch created:");
    println!("     - Operations: {}", onboarding_batch.operations.len());
    println!(
        "     - Transaction mode: {:?}",
        onboarding_batch.transaction_mode
    );
    println!(
        "     - Rollback strategy: {:?}",
        onboarding_batch.rollback_strategy
    );

    // Example 2: Sequential batch with error handling
    let maintenance_batch = BatchOperation {
        operations: vec![
            // Read all pending CBUs
            CrudStatement::DataRead(DataRead {
                asset: "cbu".to_string(),
                where_clause: Some(create_property_map(vec![("status", "pending_maintenance")])),
                select_fields: Some(vec![
                    Value::Literal(Literal::String("id".to_string())),
                    Value::Literal(Literal::String("name".to_string())),
                    Value::Literal(Literal::String("last_maintenance_date".to_string())),
                ]),
            }),
            // Update maintenance status
            CrudStatement::DataUpdate(DataUpdate {
                asset: "cbu".to_string(),
                where_clause: create_property_map(vec![("status", "pending_maintenance")]),
                values: create_property_map(vec![
                    ("status", "under_maintenance"),
                    ("maintenance_started", "NOW()"),
                ]),
            }),
            // This might fail for some records, but we want to continue
            CrudStatement::DataUpdate(DataUpdate {
                asset: "maintenance_log".to_string(),
                where_clause: create_property_map(vec![("cbu_id", "ALL_PENDING")]),
                values: create_property_map(vec![
                    ("maintenance_type", "scheduled"),
                    ("status", "in_progress"),
                ]),
            }),
        ],
        transaction_mode: TransactionMode::Sequential,
        rollback_strategy: RollbackStrategy::ContinueOnError,
    };

    println!("   ✅ Sequential maintenance batch created:");
    println!("     - Operations: {}", maintenance_batch.operations.len());
    println!("     - Error handling: Continue on error");

    // Validate batch structure
    validate_batch_operation(&onboarding_batch)?;
    validate_batch_operation(&maintenance_batch)?;

    println!("   ✅ Batch operation structures demonstrated\n");
    Ok(())
}

/// Demonstrates DSL parsing for advanced operations
async fn demo_advanced_dsl_parsing() -> Result<()> {
    println!("🔧 Demo 4: Advanced DSL Parsing");
    println!("-------------------------------");

    // Example 1: Parse complex query DSL
    let complex_query_dsl = r#"
    (data.query
        :asset "cbu"
        :joins [{:type "left" :asset "entities" :on {:cbu_id "entities.parent_cbu_id"}}
                {:type "inner" :asset "documents" :on {:entity_id "documents.entity_id"}}]
        :filters {:jurisdiction "US" :status "active" :created_after "2024-01-01"}
        :aggregate {:operations [{:function "count" :field "*" :alias "total_count"}
                                {:function "sum" :field "aum" :alias "total_aum"}]
                   :group-by ["jurisdiction" "entity_type"]
                   :having {:total_aum "> 1000000"}}
        :select ["cbu.name" "entities.legal_name" "total_aum"]
        :order-by [{:field "total_aum" :direction "desc"}
                  {:field "cbu.name" :direction "asc"}]
        :limit 100)
    "#;

    match parse_crud_statement(complex_query_dsl.trim()) {
        Ok(parsed_statement) => match parsed_statement {
            CrudStatement::ComplexQuery(query) => {
                println!("   ✅ Complex query DSL parsed successfully:");
                println!("     - Asset: {}", query.asset);
                if let Some(joins) = &query.joins {
                    println!("     - Joins: {} tables", joins.len());
                }
                if let Some(filters) = &query.filters {
                    println!("     - Filters: {} conditions", filters.len());
                }
                if let Some(aggregate) = &query.aggregate {
                    println!(
                        "     - Aggregations: {} operations",
                        aggregate.operations.len()
                    );
                }
            }
            _ => println!("   ❌ Unexpected statement type"),
        },
        Err(e) => println!("   ❌ Parse error: {}", e),
    }

    // Example 2: Parse conditional update DSL
    let conditional_update_dsl = r#"
    (data.conditional-update
        :asset "cbu"
        :where {:jurisdiction "US" :status "pending"}
        :if-exists {:kyc_status "approved" :documentation_complete "true"}
        :if-not-exists {:compliance_issues "true"}
        :set {:status "active" :activation_date "NOW()" :updated_by "system"}
        :increment {:activation_count 1})
    "#;

    match parse_crud_statement(conditional_update_dsl.trim()) {
        Ok(parsed_statement) => match parsed_statement {
            CrudStatement::ConditionalUpdate(update) => {
                println!("   ✅ Conditional update DSL parsed successfully:");
                println!("     - Asset: {}", update.asset);
                println!("     - Where conditions: {}", update.where_clause.len());
                if let Some(if_exists) = &update.if_exists {
                    println!("     - If-exists conditions: {}", if_exists.len());
                }
                if let Some(increment) = &update.increment_values {
                    println!("     - Increment values: {}", increment.len());
                }
            }
            _ => println!("   ❌ Unexpected statement type"),
        },
        Err(e) => println!("   ❌ Parse error: {}", e),
    }

    // Example 3: Parse batch operation DSL
    let batch_dsl = r#"
    (data.batch
        :operations [
            "(data.create :asset \"cbu\" :values {:name \"Test Corp\" :jurisdiction \"US\"})"
            "(data.create :asset \"entities\" :values {:legal_name \"Test Entity LLC\"})"
            "(data.update :asset \"cbu\" :where {:name \"Test Corp\"} :values {:entity_linked \"true\"})"
        ]
        :mode "atomic"
        :rollback "full")
    "#;

    match parse_crud_statement(batch_dsl.trim()) {
        Ok(parsed_statement) => match parsed_statement {
            CrudStatement::BatchOperation(batch) => {
                println!("   ✅ Batch operation DSL parsed successfully:");
                println!("     - Operations: {}", batch.operations.len());
                println!("     - Transaction mode: {:?}", batch.transaction_mode);
                println!("     - Rollback strategy: {:?}", batch.rollback_strategy);
            }
            _ => println!("   ❌ Unexpected statement type"),
        },
        Err(e) => println!("   ❌ Parse error: {}", e),
    }

    println!("   ✅ Advanced DSL parsing demonstrated\n");
    Ok(())
}

/// Demonstrates AI-generated advanced operations
async fn demo_ai_advanced_generation(service: &AgenticCrudService) -> Result<()> {
    println!("🤖 Demo 5: AI-Generated Advanced Operations");
    println!("------------------------------------------");

    // Example 1: Complex analytics query generation
    let analytics_request = AgenticCrudRequest {
        instruction: "Generate a complex query to analyze US hedge funds with their entities and documents, showing total AUM by jurisdiction and entity type, only including funds with AUM over $1M, ordered by total AUM descending".to_string(),
        context_hints: Some(vec![
            "complex query".to_string(),
            "joins".to_string(),
            "aggregation".to_string(),
            "hedge funds".to_string(),
        ]),
        execute: false,
        request_id: Some("analytics-demo".to_string()),
    };

    let analytics_response = service.process_request(analytics_request)?;
    println!("   ✅ Analytics query generation:");
    println!("     - Success: {}", analytics_response.success);
    println!("     - Generated DSL: {}", analytics_response.generated_dsl);
    println!(
        "     - AI generation time: {}ms",
        analytics_response.generation_metadata.ai_generation_time_ms
    );
    println!(
        "     - RAG confidence: {:.2}",
        analytics_response.rag_context.confidence_score
    );

    if let Some(parsed) = &analytics_response.parsed_statement {
        println!("     - Successfully parsed: ✅");
        match parsed {
            CrudStatement::ComplexQuery(_) => println!("     - Query type: Complex Query ✅"),
            _ => println!("     - Query type: Other"),
        }
    }

    // Example 2: Batch onboarding generation
    let batch_request = AgenticCrudRequest {
        instruction: "Create a comprehensive batch operation to onboard 3 new institutional clients with full KYC, entity setup, and document requirements, using atomic transaction mode with full rollback".to_string(),
        context_hints: Some(vec![
            "batch operation".to_string(),
            "institutional clients".to_string(),
            "atomic".to_string(),
            "onboarding".to_string(),
        ]),
        execute: false,
        request_id: Some("batch-onboarding-demo".to_string()),
    };

    let batch_response = service.process_request(batch_request)?;
    println!("   ✅ Batch onboarding generation:");
    println!("     - Success: {}", batch_response.success);
    println!("     - Generated DSL: {}", batch_response.generated_dsl);

    if let Some(parsed) = &batch_response.parsed_statement {
        match parsed {
            CrudStatement::BatchOperation(batch) => {
                println!("     - Operations count: {}", batch.operations.len());
                println!("     - Transaction mode: {:?}", batch.transaction_mode);
                println!("     - Rollback strategy: {:?}", batch.rollback_strategy);
            }
            _ => println!("     - Not a batch operation"),
        }
    }

    // Example 3: Conditional update for compliance
    let compliance_request = AgenticCrudRequest {
        instruction: "Generate a conditional update to activate all pending US clients, but only if their KYC is approved AND documentation is complete AND there are no compliance issues, also increment their activation counter".to_string(),
        context_hints: Some(vec![
            "conditional update".to_string(),
            "compliance".to_string(),
            "activation".to_string(),
            "increment".to_string(),
        ]),
        execute: false,
        request_id: Some("compliance-demo".to_string()),
    };

    let compliance_response = service.process_request(compliance_request)?;
    println!("   ✅ Compliance conditional update generation:");
    println!("     - Success: {}", compliance_response.success);
    println!(
        "     - Generated DSL: {}",
        compliance_response.generated_dsl
    );

    // Example 4: Performance metrics
    println!("   📊 AI Service Performance Metrics:");
    println!(
        "     - Available assets: {}",
        service.get_available_assets().len()
    );

    let creation_examples = service.get_examples_by_category("creation");
    println!("     - Creation examples: {}", creation_examples.len());

    let query_examples = service.get_examples_by_category("query");
    println!("     - Query examples: {}", query_examples.len());

    let update_examples = service.get_examples_by_category("update");
    println!("     - Update examples: {}", update_examples.len());

    println!("   ✅ AI-generated advanced operations demonstrated\n");
    Ok(())
}

// Helper functions

fn create_property_map(fields: Vec<(&str, &str)>) -> PropertyMap {
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

fn create_property_map_with_numbers(fields: Vec<(&str, i32)>) -> PropertyMap {
    let mut map = PropertyMap::new();
    for (key, value) in fields {
        map.insert(
            Key {
                parts: vec![key.to_string()],
            },
            Value::Literal(Literal::Number(value as f64)),
        );
    }
    map
}

fn validate_complex_query(query: &ComplexQuery) -> Result<()> {
    if query.asset.is_empty() {
        return Err(anyhow::anyhow!("Asset cannot be empty"));
    }

    if let Some(joins) = &query.joins {
        for join in joins {
            if join.target_asset.is_empty() {
                return Err(anyhow::anyhow!("Join target asset cannot be empty"));
            }
        }
    }

    if let Some(aggregate) = &query.aggregate {
        if aggregate.operations.is_empty() {
            return Err(anyhow::anyhow!("Aggregate operations cannot be empty"));
        }
    }

    println!("   ✅ Complex query validation passed");
    Ok(())
}

fn validate_conditional_update(update: &ConditionalUpdate) -> Result<()> {
    if update.asset.is_empty() {
        return Err(anyhow::anyhow!("Asset cannot be empty"));
    }

    if update.where_clause.is_empty() {
        return Err(anyhow::anyhow!("Where clause cannot be empty"));
    }

    if update.set_values.is_empty() {
        return Err(anyhow::anyhow!("Set values cannot be empty"));
    }

    println!("   ✅ Conditional update validation passed");
    Ok(())
}

fn validate_batch_operation(batch: &BatchOperation) -> Result<()> {
    if batch.operations.is_empty() {
        return Err(anyhow::anyhow!("Batch operations cannot be empty"));
    }

    for (i, operation) in batch.operations.iter().enumerate() {
        match operation {
            CrudStatement::DataCreate(create) => {
                if create.asset.is_empty() {
                    return Err(anyhow::anyhow!("Create operation {} has empty asset", i));
                }
            }
            CrudStatement::DataRead(read) => {
                if read.asset.is_empty() {
                    return Err(anyhow::anyhow!("Read operation {} has empty asset", i));
                }
            }
            CrudStatement::DataUpdate(update) => {
                if update.asset.is_empty() {
                    return Err(anyhow::anyhow!("Update operation {} has empty asset", i));
                }
            }
            CrudStatement::DataDelete(delete) => {
                if delete.asset.is_empty() {
                    return Err(anyhow::anyhow!("Delete operation {} has empty asset", i));
                }
            }
            _ => {
                // Advanced operations validation would go here
            }
        }
    }

    println!("   ✅ Batch operation validation passed");
    Ok(())
}

#[cfg(test)]
mod tests {
    use super::*;

    #[tokio::test]
    async fn test_demo_functions() {
        // Test that demo functions don't panic
        assert!(demo_complex_query_structures().await.is_ok());
        assert!(demo_conditional_update_operations().await.is_ok());
        assert!(demo_batch_operations_structure().await.is_ok());
        assert!(demo_advanced_dsl_parsing().await.is_ok());

        let service = AgenticCrudService::with_mock();
        assert!(demo_ai_advanced_generation(&service).await.is_ok());
    }

    #[test]
    fn test_property_map_creation() {
        let map = create_property_map(vec![("name", "Test"), ("status", "active")]);
        assert_eq!(map.len(), 2);

        let key = Key {
            parts: vec!["name".to_string()],
        };
        assert!(map.contains_key(&key));
    }

    #[test]
    fn test_validation_functions() {
        let query = ComplexQuery {
            asset: "test".to_string(),
            joins: None,
            filters: None,
            aggregate: None,
            select_fields: None,
            order_by: None,
            limit: None,
            offset: None,
        };
        assert!(validate_complex_query(&query).is_ok());

        let update = ConditionalUpdate {
            asset: "test".to_string(),
            where_clause: create_property_map(vec![("id", "123")]),
            if_exists: None,
            if_not_exists: None,
            set_values: create_property_map(vec![("status", "updated")]),
            increment_values: None,
        };
        assert!(validate_conditional_update(&update).is_ok());
    }
}
