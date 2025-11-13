//! Phase 3 Database Integration Demo
//!
//! This example demonstrates the complete implementation of Phase 3 from the
//! DSL_MANAGER_TO_DSL_MOD_PLAN.md: DSL Manager → DSL Mod → Database orchestration.
//!
//! ## What This Demo Shows
//! 1. **Clean Module Facades**: Using proper re-exports from lib.rs
//! 2. **Database Integration**: DSL Processor with optional database connectivity
//! 3. **SQLX Trait Integration**: Proper database service integration patterns
//! 4. **Orchestration Interface**: Complete call chain from Manager to Processor to Database
//! 5. **Feature Gates**: Conditional compilation for database features
//!
//! ## Architecture Demonstrated
//! ```
//! DSL Manager → DSL Processor (DSL Mod) → Database Service → PostgreSQL
//!     ↓              ↓                        ↓               ↓
//! [Gateway]    [Orchestration]         [SQLX Integration] [Database]
//! ```

use ob_poc::{
    dsl::{
        DslOrchestrationInterface, DslPipelineProcessor, OrchestrationContext,
        OrchestrationOperation, OrchestrationOperationType,
    },
    dsl_manager::{CleanDslManager, CleanManagerConfig},
};
use std::time::Instant;
use uuid::Uuid;

#[cfg(feature = "database")]
use ob_poc::database::{DatabaseConfig, DatabaseManager, DictionaryDatabaseService};

/// Demo: DSL Processor Database Integration Patterns
async fn demo_dsl_processor_database_patterns() {
    println!("🔧 Phase 3 Demo: DSL Processor Database Integration Patterns");
    println!("{}", "=".repeat(80));

    // Test processor without database
    println!("\n1. DSL Processor without Database:");
    let processor = DslPipelineProcessor::new();
    println!("   Has database: {}", processor.has_database());
    println!(
        "   Database service: {:?}",
        processor.database_service().is_some()
    );

    // Test processor with mock database service
    #[cfg(feature = "database")]
    {
        println!("\n2. DSL Processor with Mock Database:");
        let mock_db_service = DictionaryDatabaseService::new_mock();
        let processor_with_db = DslPipelineProcessor::with_database(mock_db_service);

        println!("   Has database: {}", processor_with_db.has_database());
        println!(
            "   Database service: {:?}",
            processor_with_db.database_service().is_some()
        );
    }

    #[cfg(not(feature = "database"))]
    {
        println!("\n2. DSL Processor with Database (Feature Disabled):");
        println!("   Database feature not enabled - would work with --features database");
    }
}

/// Demo: Clean DSL Manager Database Integration
async fn demo_clean_manager_database_integration() {
    println!("\n🎯 Phase 3 Demo: Clean DSL Manager Database Integration");
    println!("{}", "=".repeat(80));

    // Test manager without database
    println!("\n1. Clean DSL Manager without Database:");
    let manager = CleanDslManager::new();
    println!("   Has database: {}", manager.has_database());
    println!(
        "   Database service: {:?}",
        manager.database_service().is_some()
    );

    // Test manager with database (when feature is enabled)
    #[cfg(feature = "database")]
    {
        println!("\n2. Clean DSL Manager with Mock Database:");
        let mock_db_service = DictionaryDatabaseService::new_mock();
        let manager_with_db = CleanDslManager::with_database(mock_db_service);

        println!("   Has database: {}", manager_with_db.has_database());
        println!(
            "   Database service: {:?}",
            manager_with_db.database_service().is_some()
        );

        // Test configuration with database
        println!("\n3. Clean DSL Manager with Config and Database:");
        let config = ob_poc::dsl_manager::CleanManagerConfig {
            enable_detailed_logging: true,
            enable_metrics: true,
            max_processing_time_seconds: 30,
            enable_auto_cleanup: true,
        };

        let mock_db_service_2 = DictionaryDatabaseService::new_mock();
        let configured_manager =
            CleanDslManager::with_config_and_database(config, mock_db_service_2);

        println!("   Has database: {}", configured_manager.has_database());
        println!("   Configuration applied: ✓");
    }

    #[cfg(not(feature = "database"))]
    {
        println!("\n2. Database Integration (Feature Disabled):");
        println!(
            "   Run with: cargo run --example phase3_database_integration_demo --features database"
        );
        println!("   This would demonstrate full database integration with SQLX");
    }
}

/// Demo: Orchestration Interface Database Operations
async fn demo_orchestration_database_operations() {
    println!("\n🚀 Phase 3 Demo: Orchestration Interface Database Operations");
    println!("{}", "=".repeat(80));

    #[cfg(feature = "database")]
    {
        let mock_db_service = DictionaryDatabaseService::new_mock();
        let processor = DslPipelineProcessor::with_database(mock_db_service);

        let context = OrchestrationContext::new("demo-user".to_string(), "onboarding".to_string())
            .with_case_id("DEMO-CASE-001".to_string());

        // Test different operation types
        let test_cases = vec![
            (OrchestrationOperationType::Parse, "Parse Operation"),
            (OrchestrationOperationType::Validate, "Validate Operation"),
            (OrchestrationOperationType::Execute, "Execute Operation"),
            (
                OrchestrationOperationType::ProcessComplete,
                "Complete Pipeline",
            ),
        ];

        for (op_type, description) in test_cases {
            println!("\n   Testing: {}", description);
            let start_time = std::time::Instant::now();

            let operation = OrchestrationOperation::new(
                op_type,
                "(case.create :case-id \"DEMO-CASE-001\" :case-type \"ONBOARDING\")".to_string(),
                context.clone(),
            );

            match processor.process_orchestrated_operation(operation).await {
                Ok(result) => {
                    println!("     ✓ Success: {}", result.success);
                    println!("     ⏱  Time: {}ms", result.processing_time_ms);
                    println!("     📊 Operation ID: {}", result.operation_id);
                }
                Err(e) => {
                    println!("     ✗ Error: {}", e);
                }
            }

            println!("     🔄 Actual processing time: {:?}", start_time.elapsed());
        }
    }

    #[cfg(not(feature = "database"))]
    {
        println!("\n   Mock Orchestration (Database Feature Disabled):");
        let processor = DslPipelineProcessor::new();

        let context = OrchestrationContext::new("demo-user".to_string(), "onboarding".to_string())
            .with_case_id("DEMO-CASE-001".to_string());

        let operation = OrchestrationOperation::new(
            OrchestrationOperationType::Execute,
            "(case.create :case-id \"DEMO-CASE-001\")".to_string(),
            context,
        );

        match processor.process_orchestrated_operation(operation).await {
            Ok(result) => {
                println!("     ✓ Mock Success: {}", result.success);
                println!("     ⏱  Time: {}ms", result.processing_time_ms);
                println!("     📝 Note: This uses mock database operations");
            }
            Err(e) => {
                println!("     ✗ Error: {}", e);
            }
        }
    }
}

/// Demo: DSL Execution with Database Operations
async fn demo_dsl_execution_database_operations() {
    println!("\n💾 Phase 3 Demo: DSL Execution with Database Operations");
    println!("{}", "=".repeat(80));

    #[cfg(feature = "database")]
    {
        let mock_db_service = DictionaryDatabaseService::new_mock();
        let processor = DslPipelineProcessor::with_database(mock_db_service);

        let context = OrchestrationContext::new("demo-user".to_string(), "onboarding".to_string())
            .with_case_id("EXEC-DEMO-001".to_string());

        // Test different DSL operations that map to database operations
        let test_dsl_operations = vec![
            ("(case.create :case-id \"TEST-001\")", "CREATE_CASE"),
            ("(case.update :case-id \"TEST-002\")", "UPDATE_CASE"),
            ("(entity.register :entity-id \"ENT-001\")", "CREATE_ENTITY"),
            ("(kyc.start :case-id \"KYC-001\")", "START_KYC"),
            ("(unknown.operation :id \"UNK-001\")", "UNKNOWN_OPERATION"),
        ];

        for (dsl_content, expected_op_type) in test_dsl_operations {
            println!("\n   Testing DSL: {}", dsl_content);

            match processor
                .execute_orchestrated_dsl(dsl_content, context.clone())
                .await
            {
                Ok(result) => {
                    println!("     ✓ Execution: {}", result.success);
                    println!("     ⏱  Time: {}ms", result.execution_time_ms);
                    println!(
                        "     📊 Database Operations: {}",
                        result.database_operations.len()
                    );

                    if !result.database_operations.is_empty() {
                        let db_op = &result.database_operations[0];
                        println!(
                            "       - Type: {} (expected: {})",
                            db_op.operation_type, expected_op_type
                        );
                        println!("       - Target: {}", db_op.target);
                        println!("       - Affected: {} records", db_op.affected_count);
                        println!("       - Success: {}", db_op.success);
                    }
                }
                Err(e) => {
                    println!("     ✗ Error: {}", e);
                }
            }
        }
    }

    #[cfg(not(feature = "database"))]
    {
        println!("\n   Mock Database Operations (Feature Disabled):");
        let processor = DslPipelineProcessor::new();

        let context = OrchestrationContext::new("demo-user".to_string(), "test".to_string())
            .with_case_id("NO-DB-TEST".to_string());

        match processor
            .execute_orchestrated_dsl("(case.create :case-id \"NO-DB-TEST\")", context)
            .await
        {
            Ok(execution_result) => {
                println!("     ✓ Mock Execution: {}", execution_result.success);
                println!(
                    "     📊 Mock Database Operations: {}",
                    execution_result.database_operations.len()
                );

                if !execution_result.database_operations.is_empty() {
                    let db_op = &execution_result.database_operations[0];
                    println!("       - Type: {}", db_op.operation_type);
                    println!("       - Target: {}", db_op.target);
                    println!("       - Mock Success: {}", db_op.success);
                }
            }
            Err(e) => {
                println!("     ✗ Error: {}", e);
            }
        }
    }
}

/// Demo: End-to-End DSL Manager Processing
async fn demo_end_to_end_processing() {
    println!("\n🔄 Phase 3 Demo: End-to-End DSL Manager Processing");
    println!("{}", "=".repeat(80));

    #[cfg(feature = "database")]
    {
        let mock_db_service = DictionaryDatabaseService::new_mock();
        let mut manager = CleanDslManager::with_database(mock_db_service);

        let case_id = Uuid::new_v4();
        let dsl_content = format!(
            r#"(case.create
                :case-id "{}"
                :case-type "ONBOARDING"
                :customer-name "Phase 3 Demo Customer"
                :jurisdiction "US"
                :risk-level "MEDIUM")"#,
            case_id
        );

        println!("\n   Processing DSL through complete call chain:");
        println!("   DSL: {}", dsl_content.lines().next().unwrap_or(""));
        println!("   Case ID: {}", case_id);

        let start_time = std::time::Instant::now();
        let result = manager.execute_dsl_with_database(dsl_content).await;
        let total_time = start_time.elapsed();

        println!("\n   📊 Results:");
        println!("     ✓ Success: {}", result.success);
        println!("     🆔 Case ID: {}", result.case_id);
        println!(
            "     ⏱  Total Time: {:?} (reported: {}ms)",
            total_time, result.processing_time_ms
        );
        println!("     🎨 Visualization: {}", result.visualization_generated);
        println!("     🤖 AI Generated: {}", result.ai_generated);
        println!("     🔗 Errors: {}", result.errors.len());

        if !result.errors.is_empty() {
            for error in &result.errors {
                println!("       - {}", error);
            }
        }

        // Show step details
        if let Some(dsl_step) = &result.step_details.dsl_processing {
            println!("     📈 DSL Processing Step:");
            println!("       - Success: {}", dsl_step.success);
            println!("       - Time: {}ms", dsl_step.processing_time_ms);
            println!("       - AST Available: {}", dsl_step.parsed_ast_available);
            println!(
                "       - Domain Snapshot: {}",
                dsl_step.domain_snapshot_created
            );
        }

        if let Some(state_step) = &result.step_details.state_management {
            println!("     💾 State Management Step:");
            println!("       - Success: {}", state_step.success);
            println!("       - Time: {}ms", state_step.processing_time_ms);
            println!("       - Version: {}", state_step.version_number);
            println!("       - Snapshot ID: {}", state_step.snapshot_id);
        }
    }

    #[cfg(not(feature = "database"))]
    {
        let mut manager = CleanDslManager::new();

        let dsl_content = format!(
            r#"(case.create
                :case-id "{}"
                :case-type "DEMO"
                :customer-name "Mock Demo Customer")"#,
            Uuid::new_v4()
        );

        println!("\n   Processing DSL without database:");
        let result = manager.process_dsl_request(dsl_content).await;

        println!("     ✓ Mock Success: {}", result.success);
        println!("     🆔 Case ID: {}", result.case_id);
        println!("     📝 Note: This demonstrates the architecture without live database");

        // Test database execution (should fail gracefully)
        let dsl_content = "(case.create :case-id \"NO-DB-TEST\")".to_string();
        let db_result = manager.execute_dsl_with_database(dsl_content).await;

        println!("\n   Database Execution without DB Service:");
        println!("     ✗ Expected Failure: {}", !db_result.success);
        println!(
            "     📝 Error Message: {}",
            db_result.errors.get(0).unwrap_or(&"No error".to_string())
        );
    }
}

/// Demo: SQLX Integration Patterns (when database feature is enabled)
#[cfg(feature = "database")]
async fn demo_sqlx_integration_patterns() {
    println!("\n🗄️  Phase 3 Demo: SQLX Integration Patterns");
    println!("{}", "=".repeat(80));

    // This would demonstrate actual database connection patterns
    // For the demo, we'll show the pattern without requiring a live database

    println!("\n   SQLX Integration Architecture:");
    println!("     1. DatabaseConfig → DatabaseManager → PgPool");
    println!("     2. PgPool → DictionaryDatabaseService → DSL Processor");
    println!("     3. DSL Processor → Database Operations → PostgreSQL");

    // Show how you would create a real database connection
    println!("\n   Example Database Integration Code:");
    println!("   ```rust");
    println!("   // 1. Setup database configuration");
    println!("   let config = DatabaseConfig::default();");
    println!("   ");
    println!("   // 2. Create database manager with SQLX pool");
    println!("   let db_manager = DatabaseManager::new(config).await?;");
    println!("   ");
    println!("   // 3. Create dictionary service from pool");
    println!("   let db_service = db_manager.dictionary_service();");
    println!("   ");
    println!("   // 4. Create DSL processor with database connectivity");
    println!("   let processor = DslPipelineProcessor::with_database(db_service);");
    println!("   ");
    println!("   // 5. Create DSL manager with full database integration");
    println!("   let manager = CleanDslManager::with_database(db_service);");
    println!("   ```");

    // Show mock database service creation
    println!("\n   Mock Database Service for Testing:");
    let mock_service = DictionaryDatabaseService::new_mock();
    println!("     ✓ Mock service created");
    println!("     📝 Note: Real service would use: DictionaryDatabaseService::new(pg_pool)");

    // Show health check pattern
    println!("\n   Health Check Integration:");
    match mock_service.health_check().await {
        Ok(_) => println!("     ✓ Database health check passed (mock)"),
        Err(e) => println!("     ⚠️  Database health check: {}", e),
    }
}

#[tokio::main]
async fn main() {
    println!("Phase 3: DSL Manager to DSL Mod Database Integration Demo");
    println!("Implementation of DSL_MANAGER_TO_DSL_MOD_PLAN.md Phase 3");
    println!();

    // Demo 1: Basic database integration patterns
    demo_dsl_processor_database_patterns().await;

    // Demo 2: Clean DSL Manager integration
    demo_clean_manager_database_integration().await;

    // Demo 3: Orchestration interface operations
    demo_orchestration_database_operations().await;

    // Demo 4: DSL execution with database operations
    demo_dsl_execution_database_operations().await;

    // Demo 5: End-to-end processing
    demo_end_to_end_processing().await;

    // Demo 6: SQLX integration patterns (feature-gated)
    #[cfg(feature = "database")]
    demo_sqlx_integration_patterns().await;

    // Summary
    println!("\n🎉 Phase 3 Demo Complete!");
    println!("{}", "=".repeat(80));
    println!();
    println!("✅ What was demonstrated:");
    println!("   1. ✓ Clean module facades and re-exports");
    println!("   2. ✓ DSL Processor with optional database connectivity");
    println!("   3. ✓ Clean DSL Manager with database integration");
    println!("   4. ✓ Orchestration interface database operations");
    println!("   5. ✓ Feature-gated database code compilation");
    println!("   6. ✓ Mock vs real database service patterns");
    println!("   7. ✓ End-to-end call chain: Manager → Processor → Database");
    println!();
    println!("🚀 Phase 3 Implementation Status: COMPLETE");
    println!();
    println!("📋 Next Steps:");
    println!("   - Phase 4: Integration testing with live database");
    println!("   - Phase 5: Performance optimization and monitoring");
    println!("   - Connect to actual PostgreSQL for full SQLX integration");
    println!();
    println!("💡 To test with database features:");
    println!("   cargo run --example phase3_database_integration_demo --features database");
}
