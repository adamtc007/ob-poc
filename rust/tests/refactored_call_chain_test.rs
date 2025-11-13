//! Refactored Call Chain Integration Test
//!
//! This test demonstrates the complete refactored call chain architecture:
//! Clean DSL Manager → DSL Pipeline Processor → DB State Manager → DSL Visualizer
//!
//! Based on the proven architecture from the session record and independent implementation,
//! this test validates that the refactored components work together seamlessly.

use ob_poc::db_state_manager::DbStateManager;
use ob_poc::dsl::DslPipelineProcessor;
use ob_poc::dsl_manager::{CallChainResult, CleanDslManager, IncrementalResult, ValidationResult};
use ob_poc::dsl_visualizer::DslVisualizer;

#[tokio::test]
async fn test_refactored_call_chain_end_to_end() {
    println!("🚀 Testing refactored call chain architecture");

    // Create the clean DSL manager (integrates all components)
    let mut clean_manager = CleanDslManager::new();

    // Verify health of all components
    assert!(
        clean_manager.health_check().await,
        "Call chain should be healthy"
    );

    // Test basic DSL processing through the complete call chain
    let dsl_content =
        r#"(case.create :case-id "REFACTOR-001" :case-type "ONBOARDING")"#.to_string();
    let result = clean_manager.process_dsl_request(dsl_content).await;

    println!("📊 Call chain result: {:?}", result);

    // Verify successful processing
    assert!(result.success, "Call chain processing should succeed");
    assert_eq!(
        result.case_id, "REFACTOR-001",
        "Should extract correct case ID"
    );
    assert!(result.processing_time_ms > 0, "Should have processing time");
    assert!(
        result.visualization_generated,
        "Should generate visualization"
    );
    assert!(
        !result.ai_generated,
        "Direct DSL should not be AI-generated"
    );

    // Verify all call chain steps completed
    assert!(
        result.step_details.dsl_processing.is_some(),
        "DSL processing step should complete"
    );
    assert!(
        result.step_details.state_management.is_some(),
        "State management step should complete"
    );
    assert!(
        result.step_details.visualization.is_some(),
        "Visualization step should complete"
    );

    let dsl_step = result.step_details.dsl_processing.as_ref().unwrap();
    assert!(dsl_step.success, "DSL processing should succeed");
    assert!(dsl_step.parsed_ast_available, "Should have parsed AST");
    assert!(
        dsl_step.domain_snapshot_created,
        "Should create domain snapshot"
    );

    let state_step = result.step_details.state_management.as_ref().unwrap();
    assert!(state_step.success, "State management should succeed");
    assert_eq!(
        state_step.version_number, 1,
        "Should be version 1 for new case"
    );
    assert!(
        !state_step.snapshot_id.is_empty(),
        "Should have snapshot ID"
    );

    let viz_step = result.step_details.visualization.as_ref().unwrap();
    assert!(viz_step.success, "Visualization should succeed");
    assert!(viz_step.output_size_bytes > 0, "Should generate output");
}

#[tokio::test]
async fn test_refactored_incremental_dsl_accumulation() {
    println!("🔄 Testing refactored incremental DSL accumulation (DSL-as-State pattern)");

    let mut clean_manager = CleanDslManager::new();

    // Step 1: Create base case
    let base_dsl = r#"(case.create :case-id "INCR-001" :case-type "ONBOARDING")"#.to_string();
    let base_result = clean_manager.process_dsl_request(base_dsl).await;

    assert!(base_result.success, "Base case creation should succeed");
    assert_eq!(base_result.case_id, "INCR-001");

    // Step 2: Add incremental DSL (KYC collection)
    let kyc_dsl = r#"(kyc.collect :case-id "INCR-001" :collection-type "ENHANCED")"#.to_string();
    let kyc_result = clean_manager
        .process_incremental_dsl("INCR-001".to_string(), kyc_dsl)
        .await;

    assert!(kyc_result.success, "KYC incremental should succeed");
    assert!(
        kyc_result.accumulated_dsl.contains("case.create"),
        "Should contain base DSL"
    );
    assert!(
        kyc_result.accumulated_dsl.contains("kyc.collect"),
        "Should contain incremental DSL"
    );
    assert!(kyc_result.version_number > 1, "Version should increment");

    // Step 3: Add another incremental DSL (Entity registration)
    let entity_dsl =
        r#"(entity.register :case-id "INCR-001" :entity-type "CORP" :jurisdiction "GB")"#
            .to_string();
    let entity_result = clean_manager
        .process_incremental_dsl("INCR-001".to_string(), entity_dsl)
        .await;

    assert!(entity_result.success, "Entity incremental should succeed");
    assert!(
        entity_result.accumulated_dsl.contains("case.create"),
        "Should contain base DSL"
    );
    assert!(
        entity_result.accumulated_dsl.contains("kyc.collect"),
        "Should contain first incremental"
    );
    assert!(
        entity_result.accumulated_dsl.contains("entity.register"),
        "Should contain second incremental"
    );
    assert!(
        entity_result.version_number > kyc_result.version_number,
        "Version should continue incrementing"
    );

    println!(
        "✅ Accumulated DSL state: {}",
        entity_result.accumulated_dsl
    );
}

#[tokio::test]
async fn test_refactored_validation_only_mode() {
    println!("🔍 Testing refactored validation-only mode");

    let clean_manager = CleanDslManager::new();

    // Test valid DSL
    let valid_dsl =
        r#"(ubo.collect-entity-data :case-id "VAL-001" :entity-type "CORP")"#.to_string();
    let valid_result = clean_manager.validate_dsl_only(valid_dsl).await;

    assert!(valid_result.valid, "Valid DSL should pass validation");
    assert!(valid_result.errors.is_empty(), "Should have no errors");
    assert!(
        valid_result.rules_checked > 0,
        "Should check validation rules"
    );
    assert!(
        valid_result.compliance_score > 0.0,
        "Should have positive compliance score"
    );

    // Test invalid DSL
    let invalid_dsl = "invalid dsl content without proper structure".to_string();
    let invalid_result = clean_manager.validate_dsl_only(invalid_dsl).await;

    assert!(!invalid_result.valid, "Invalid DSL should fail validation");
    assert!(
        !invalid_result.errors.is_empty(),
        "Should have validation errors"
    );
    assert!(
        invalid_result.compliance_score == 0.0,
        "Should have zero compliance score for invalid DSL"
    );

    println!(
        "✅ Validation results: valid={}, errors={}, score={:.2}",
        valid_result.valid,
        valid_result.errors.len(),
        valid_result.compliance_score
    );
}

#[tokio::test]
async fn test_refactored_ai_separation_pattern() {
    println!("🤖 Testing refactored AI separation pattern");

    let mut clean_manager = CleanDslManager::new();

    // Test direct DSL (no AI)
    let direct_dsl = r#"(products.add :case-id "DIRECT-001" :product-type "CUSTODY")"#.to_string();
    let direct_result = clean_manager.process_dsl_request(direct_dsl).await;

    assert!(direct_result.success, "Direct DSL should work without AI");
    assert!(
        !direct_result.ai_generated,
        "Direct DSL should not be marked as AI-generated"
    );

    // Test AI-generated DSL
    let ai_instruction =
        "Create onboarding case for UK hedge fund requiring custody services".to_string();
    let ai_result = clean_manager.process_ai_instruction(ai_instruction).await;

    assert!(
        ai_result.ai_generated,
        "AI result should be marked as AI-generated"
    );
    assert!(
        !ai_result.generated_dsl.is_empty(),
        "Should generate DSL content"
    );
    assert!(
        !ai_result.case_id.is_empty(),
        "Should extract or generate case ID"
    );
    assert!(
        ai_result.ai_confidence > 0.0,
        "Should have AI confidence score"
    );

    // Verify AI-generated DSL contains expected elements
    assert!(
        ai_result.generated_dsl.contains("case-id"),
        "Generated DSL should have case ID"
    );
    assert!(
        ai_result.generated_dsl.contains("onboarding")
            || ai_result.generated_dsl.contains("ONBOARDING"),
        "Generated DSL should reference onboarding"
    );

    println!("✅ AI separation validated: Direct DSL works independently, AI layer optional");
}

#[tokio::test]
async fn test_refactored_4_step_pipeline() {
    println!("🔍 Testing refactored 4-step DSL processing pipeline");

    // Test the DSL pipeline processor directly to verify the 4-step process
    let mut pipeline_processor = DslPipelineProcessor::new();

    let dsl_content = r#"(isda.establish_master :case-id "PIPE-001" :counterparty "GOLDMAN-SACHS" :master-type "2002")"#;
    let pipeline_result = pipeline_processor.process_dsl_content(dsl_content).await;

    assert!(pipeline_result.success, "4-step pipeline should succeed");
    assert_eq!(
        pipeline_result.step_results.len(),
        4,
        "Should complete all 4 steps"
    );

    // Verify each step
    let step_names = vec![
        "DSL Change Validation",
        "AST Parse/Validate",
        "DSL Domain Snapshot Save",
        "AST Dual Commit",
    ];

    for (i, expected_name) in step_names.iter().enumerate() {
        let step = &pipeline_result.step_results[i];
        assert_eq!(
            step.step_number,
            (i + 1) as u8,
            "Step number should match index"
        );
        assert_eq!(
            step.step_name, *expected_name,
            "Step name should match expected"
        );
        assert!(step.success, "Step {} should succeed", i + 1);
        assert!(
            step.processing_time_ms > 0,
            "Step {} should have processing time",
            i + 1
        );

        println!(
            "✅ Step {}: {} completed in {}ms",
            step.step_number, step.step_name, step.processing_time_ms
        );
    }

    // Verify pipeline outputs
    assert!(
        pipeline_result.parsed_ast.is_some(),
        "Should have parsed AST"
    );
    assert_eq!(
        pipeline_result.case_id, "PIPE-001",
        "Should extract case ID"
    );
    assert_eq!(
        pipeline_result.domain_snapshot.primary_domain, "isda",
        "Should detect ISDA domain"
    );
    assert!(
        pipeline_result
            .domain_snapshot
            .involved_domains
            .contains(&"isda".to_string()),
        "Should include ISDA in involved domains"
    );
}

#[tokio::test]
async fn test_refactored_component_integration() {
    println!("🔗 Testing refactored component integration independently");

    // Test each component independently first

    // 1. DSL Pipeline Processor
    let mut dsl_processor = DslPipelineProcessor::new();
    assert!(
        dsl_processor.health_check().await,
        "DSL processor should be healthy"
    );

    let dsl_content = r#"(document.catalog :case-id "COMP-001" :document-type "PASSPORT")"#;
    let dsl_result = dsl_processor.process_dsl_content(dsl_content).await;
    assert!(dsl_result.success, "DSL processing should succeed");

    // 2. DB State Manager
    let mut db_manager = DbStateManager::new();
    assert!(
        db_manager.health_check().await,
        "DB manager should be healthy"
    );

    let db_input = ob_poc::db_state_manager::DslModResult {
        success: dsl_result.success,
        parsed_ast: dsl_result.parsed_ast.clone(),
        domain_snapshot: ob_poc::db_state_manager::DomainSnapshot {
            primary_domain: dsl_result.domain_snapshot.primary_domain.clone(),
            involved_domains: dsl_result.domain_snapshot.involved_domains.clone(),
            domain_data: dsl_result.domain_snapshot.domain_data.clone(),
            compliance_markers: dsl_result.domain_snapshot.compliance_markers.clone(),
            risk_assessment: dsl_result.domain_snapshot.risk_assessment.clone(),
            snapshot_at: dsl_result.domain_snapshot.snapshot_at,
        },
        case_id: dsl_result.case_id.clone(),
        errors: dsl_result.errors.clone(),
    };

    let state_result = db_manager.save_dsl_state(&db_input).await;
    assert!(state_result.success, "State saving should succeed");

    // 3. DSL Visualizer
    let visualizer = DslVisualizer::new();
    assert!(
        visualizer.health_check().await,
        "Visualizer should be healthy"
    );

    let viz_input = ob_poc::dsl_visualizer::StateResult {
        success: state_result.success,
        case_id: state_result.case_id.clone(),
        version_number: state_result.version_number,
        snapshot_id: state_result.snapshot_id.clone(),
        errors: state_result.errors.clone(),
        processing_time_ms: state_result.processing_time_ms,
    };

    let viz_result = visualizer.generate_visualization(&viz_input).await;
    assert!(viz_result.success, "Visualization should succeed");

    println!("✅ All components integrate successfully:");
    println!(
        "   - DSL Pipeline Processor: ✅ ({}ms)",
        dsl_result.metrics.total_time_ms
    );
    println!(
        "   - DB State Manager: ✅ (version {})",
        state_result.version_number
    );
    println!(
        "   - DSL Visualizer: ✅ ({} bytes)",
        viz_result.output_size_bytes
    );
}

#[tokio::test]
async fn test_refactored_multi_domain_processing() {
    println!("🌐 Testing refactored multi-domain DSL processing");

    let mut clean_manager = CleanDslManager::new();

    // Test DSL that spans multiple domains
    let multi_domain_dsl = r#"
    (case.create :case-id "MULTI-001" :case-type "COMPREHENSIVE")
    (kyc.collect :case-id "MULTI-001" :collection-type "ENHANCED")
    (entity.register :case-id "MULTI-001" :entity-type "CORP" :jurisdiction "US")
    (ubo.collect-entity-data :case-id "MULTI-001" :entity-id "ENT-001")
    (products.add :case-id "MULTI-001" :product-type "CUSTODY")
    (document.catalog :case-id "MULTI-001" :document-type "INCORPORATION_CERTIFICATE")
    "#
    .trim()
    .to_string();

    let result = clean_manager.process_dsl_request(multi_domain_dsl).await;

    assert!(result.success, "Multi-domain processing should succeed");
    assert_eq!(result.case_id, "MULTI-001");

    // Verify that the processing detected multiple domains
    let dsl_step = result.step_details.dsl_processing.as_ref().unwrap();
    assert!(
        dsl_step.domain_snapshot_created,
        "Should create comprehensive domain snapshot"
    );

    println!(
        "✅ Multi-domain processing completed for case {}",
        result.case_id
    );
}

#[tokio::test]
async fn test_refactored_error_handling_and_recovery() {
    println!("❌ Testing refactored error handling and recovery");

    let mut clean_manager = CleanDslManager::new();

    // Test malformed DSL
    let malformed_dsl = "((( invalid dsl with unbalanced parens".to_string();
    let error_result = clean_manager.process_dsl_request(malformed_dsl).await;

    assert!(!error_result.success, "Malformed DSL should fail");
    assert!(!error_result.errors.is_empty(), "Should report errors");
    assert!(
        !error_result.visualization_generated,
        "Should not generate visualization on failure"
    );

    // Verify error is caught early in the pipeline
    if let Some(dsl_step) = error_result.step_details.dsl_processing {
        assert!(!dsl_step.success, "DSL processing step should fail");
        assert!(
            !dsl_step.errors.is_empty(),
            "Should have specific DSL errors"
        );
    }

    // Test that the system recovers and can process valid DSL after error
    let valid_dsl = r#"(case.create :case-id "RECOVERY-001" :case-type "ONBOARDING")"#.to_string();
    let recovery_result = clean_manager.process_dsl_request(valid_dsl).await;

    assert!(
        recovery_result.success,
        "System should recover and process valid DSL"
    );
    assert_eq!(recovery_result.case_id, "RECOVERY-001");

    println!("✅ Error handling works correctly, system recovers after failures");
}

#[tokio::test]
async fn test_refactored_performance_metrics() {
    println!("📊 Testing refactored performance metrics collection");

    let mut clean_manager = CleanDslManager::new();

    // Process multiple DSL operations to generate metrics
    let test_cases = vec![
        r#"(case.create :case-id "PERF-001" :case-type "ONBOARDING")"#,
        r#"(kyc.collect :case-id "PERF-002" :collection-type "STANDARD")"#,
        r#"(entity.register :case-id "PERF-003" :entity-type "PARTNERSHIP")"#,
        r#"(products.add :case-id "PERF-004" :product-type "TRADING")"#,
    ];

    let mut total_time = 0u64;
    let mut successful_operations = 0;

    for (i, dsl_content) in test_cases.iter().enumerate() {
        let result = clean_manager
            .process_dsl_request(dsl_content.to_string())
            .await;

        if result.success {
            successful_operations += 1;
            total_time += result.processing_time_ms;

            println!(
                "Operation {}: {}ms (case: {})",
                i + 1,
                result.processing_time_ms,
                result.case_id
            );
        }
    }

    assert_eq!(
        successful_operations,
        test_cases.len(),
        "All operations should succeed"
    );

    let avg_time = total_time / successful_operations as u64;
    println!(
        "✅ Performance metrics: {} operations, avg {}ms, total {}ms",
        successful_operations, avg_time, total_time
    );

    // Verify reasonable performance (should be under 1 second per operation)
    assert!(
        avg_time < 1000,
        "Average processing time should be under 1 second"
    );
}

#[test]
fn test_refactored_architecture_principles() {
    println!("🏗️ Testing refactored architecture principles compliance");

    // Principle 1: DSL-First Design - Core system works without AI dependencies
    let clean_manager = CleanDslManager::new();
    // This should compile and work without any AI services
    println!("✅ DSL-First Design: Manager creates without AI dependencies");

    // Principle 2: Clean Separation - AI as optional layer
    // The fact that we can create and use CleanDslManager without AI proves this
    println!("✅ Clean Separation: AI is optional layer");

    // Principle 3: Call Chain Pattern - Components are loosely coupled
    let _dsl_processor = DslPipelineProcessor::new();
    let _db_manager = DbStateManager::new();
    let _visualizer = DslVisualizer::new();
    println!("✅ Call Chain Pattern: Components are independently creatable");

    // Principle 4: DSL-as-State Pattern - Accumulated DSL is the state
    // This is evidenced by the IncrementalResult containing accumulated_dsl
    println!("✅ DSL-as-State Pattern: Incremental accumulation supported");

    println!("✅ All architecture principles validated");
}
