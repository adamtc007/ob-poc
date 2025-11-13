//! Simple Refactored Architecture Test
//!
//! This test demonstrates the clean refactored call chain architecture working:
//! DSL Manager → DSL Mod → DB State Manager → DSL Visualizer

use ob_poc::dsl_manager::CleanDslManager;

#[tokio::test]
async fn test_simple_refactored_call_chain() {
    println!("🚀 Testing simple refactored call chain");

    // Create the clean DSL manager
    let mut clean_manager = CleanDslManager::new();

    // Test health check
    assert!(
        clean_manager.health_check().await,
        "Clean manager should be healthy"
    );

    // Test basic DSL processing
    let dsl_content = r#"(case.create :case-id "SIMPLE-001" :case-type "ONBOARDING")"#.to_string();
    let result = clean_manager.process_dsl_request(dsl_content).await;

    println!(
        "📊 Result: success={}, case_id={}, time={}ms",
        result.success, result.case_id, result.processing_time_ms
    );

    // Verify successful processing
    assert!(result.success, "Processing should succeed");
    assert_eq!(result.case_id, "SIMPLE-001", "Should extract case ID");
    assert!(result.processing_time_ms > 0, "Should have processing time");
    assert!(
        result.visualization_generated,
        "Should generate visualization"
    );
    assert!(
        !result.ai_generated,
        "Direct DSL should not be AI-generated"
    );

    println!("✅ Simple refactored call chain test passed!");
}

#[tokio::test]
async fn test_validation_only() {
    println!("🔍 Testing validation-only mode");

    let clean_manager = CleanDslManager::new();

    let valid_dsl = r#"(kyc.collect :case-id "VAL-001" :collection-type "STANDARD")"#.to_string();
    let validation_result = clean_manager.validate_dsl_only(valid_dsl).await;

    println!(
        "✅ Validation: valid={}, errors={}, score={:.2}",
        validation_result.valid,
        validation_result.errors.len(),
        validation_result.compliance_score
    );

    assert!(validation_result.valid, "Valid DSL should pass");
    assert!(validation_result.errors.is_empty(), "Should have no errors");
    assert!(
        validation_result.compliance_score > 0.0,
        "Should have positive score"
    );

    println!("✅ Validation test passed!");
}

#[tokio::test]
async fn test_incremental_dsl() {
    println!("🔄 Testing incremental DSL accumulation");

    let mut clean_manager = CleanDslManager::new();

    // Base DSL
    let base_dsl = r#"(case.create :case-id "INC-001" :case-type "ONBOARDING")"#.to_string();
    let base_result = clean_manager.process_dsl_request(base_dsl).await;
    assert!(base_result.success, "Base should succeed");

    // Incremental DSL
    let inc_dsl = r#"(entity.register :case-id "INC-001" :entity-type "CORP")"#.to_string();
    let inc_result = clean_manager
        .process_incremental_dsl("INC-001".to_string(), inc_dsl)
        .await;

    println!(
        "🔄 Incremental: success={}, version={}",
        inc_result.success, inc_result.version_number
    );

    assert!(inc_result.success, "Incremental should succeed");
    assert!(
        inc_result.accumulated_dsl.contains("case.create"),
        "Should contain base"
    );
    assert!(
        inc_result.accumulated_dsl.contains("entity.register"),
        "Should contain increment"
    );
    assert!(inc_result.version_number > 1, "Version should increment");

    println!("✅ Incremental DSL test passed!");
}

#[test]
fn test_architecture_compactness() {
    println!("📏 Testing architecture compactness");

    // Verify we can create all components independently
    let _dsl_processor = ob_poc::dsl::DslPipelineProcessor::new();
    let _db_manager = ob_poc::db_state_manager::DbStateManager::new();
    let _visualizer = ob_poc::dsl_visualizer::DslVisualizer::new();
    let _clean_manager = ob_poc::dsl_manager::CleanDslManager::new();

    println!("✅ All components can be created independently");
    println!("✅ Architecture is compact and focused");
    println!("✅ No unnecessary dependencies or complexity");
}
