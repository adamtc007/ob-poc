//! End-to-End UUID Integration Test
//!
//! Tests the complete UUID workflow:
//! 1. Parse DSL with UUID references
//! 2. Extract UUIDs from AST
//! 3. Resolve UUIDs to semantic IDs
//! 4. Bind values from sources
//! 5. Store in database

use ob_poc::domains::attributes::execution_context::ExecutionContext;
use ob_poc::domains::attributes::kyc::{FirstName, LastName, PassportNumber};
use ob_poc::domains::attributes::types::AttributeType;
use ob_poc::domains::attributes::validator::AttributeValidator;
use ob_poc::execution::dsl_executor::{DslExecutor, ExecutionResult};
use ob_poc::services::AttributeService;
use uuid::Uuid;

#[tokio::test]
async fn test_uuid_dsl_end_to_end() {
    // Skip if no database
    if std::env::var("DATABASE_URL").is_err() {
        println!("Skipping E2E test - DATABASE_URL not set");
        return;
    }

    // Setup
    let pool = sqlx::PgPool::connect(&std::env::var("DATABASE_URL").unwrap())
        .await
        .unwrap();

    // Create validator and register attributes
    let mut validator = AttributeValidator::new();
    validator.register(FirstName::metadata());
    validator.register(LastName::metadata());
    validator.register(PassportNumber::metadata());

    let service = AttributeService::from_pool(pool.clone(), validator);
    let executor = DslExecutor::new(service.clone());

    // DSL with UUID references
    let dsl = r#"
        (kyc.collect
            :request-id "REQ-001"
            :first-name @attr{3020d46f-472c-5437-9647-1b0682c35935}
            :last-name @attr{0af112fd-ec04-5938-84e8-6e5949db0b52}
            :passport @attr{c09501c7-2ea9-5ad7-b330-7d664c678e37}
        )
    "#;

    let entity_id = Uuid::new_v4();

    // Execute
    let result = executor.execute(dsl, entity_id).await;

    // Verify execution succeeded
    assert!(result.is_ok(), "Execution failed: {:?}", result);
    let result = result.unwrap();

    println!("Execution result: {:?}", result);

    // Verify UUIDs were resolved
    assert!(
        result.attributes_resolved > 0,
        "No attributes resolved: {:?}",
        result
    );

    // Verify values were stored
    assert!(
        result.attributes_stored > 0,
        "No attributes stored: {:?}",
        result
    );

    // Verify no errors
    assert!(
        result.errors.is_empty(),
        "Errors occurred: {:?}",
        result.errors
    );

    // Verify we can retrieve the stored values
    let first_name_uuid = FirstName::uuid();
    let stored = service.get_by_uuid(entity_id, first_name_uuid).await;

    assert!(
        stored.is_ok(),
        "Failed to retrieve stored value: {:?}",
        stored
    );
    let stored_value = stored.unwrap();

    assert!(stored_value.is_some(), "No value found for first_name UUID");

    println!("Stored first name value: {:?}", stored_value);

    // Value should be "John" from DocumentExtractionSource
    assert_eq!(stored_value, Some(serde_json::json!("John")));
}

#[tokio::test]
async fn test_uuid_resolution_without_database() {
    // This test verifies UUID resolution works without requiring database

    let pool = sqlx::PgPool::connect_lazy("postgresql://test").unwrap();
    let validator = AttributeValidator::new();
    let service = AttributeService::from_pool(pool, validator);
    let executor = DslExecutor::new(service);

    let dsl = r#"
        (kyc.collect
            :first-name @attr{3020d46f-472c-5437-9647-1b0682c35935}
            :last-name @attr{0af112fd-ec04-5938-84e8-6e5949db0b52}
        )
    "#;

    // Parse and extract UUIDs
    let program = ob_poc::parser::parse_program(dsl).unwrap();
    let uuids = executor.extract_uuids(&program);

    // Verify extraction
    assert_eq!(uuids.len(), 2);
    assert!(uuids.contains(&FirstName::uuid()));
    assert!(uuids.contains(&LastName::uuid()));
}

#[tokio::test]
async fn test_uuid_value_binding() {
    // Test that values are properly bound from sources

    use ob_poc::execution::value_binder::ValueBinder;

    let binder = ValueBinder::new();
    let mut context = ExecutionContext::new();

    let first_name_uuid = FirstName::uuid();
    let last_name_uuid = LastName::uuid();
    let passport_uuid = PassportNumber::uuid();

    // Bind values
    let result1 = binder.bind_attribute(first_name_uuid, &mut context).await;
    let result2 = binder.bind_attribute(last_name_uuid, &mut context).await;
    let result3 = binder.bind_attribute(passport_uuid, &mut context).await;

    // All should succeed (DocumentExtractionSource has these)
    assert!(result1.is_ok(), "Failed to bind first_name");
    assert!(result2.is_ok(), "Failed to bind last_name");
    assert!(result3.is_ok(), "Failed to bind passport");

    // Verify values
    assert_eq!(
        context.get_value(&first_name_uuid),
        Some(&serde_json::json!("John"))
    );
    assert_eq!(
        context.get_value(&last_name_uuid),
        Some(&serde_json::json!("Smith"))
    );
    assert_eq!(
        context.get_value(&passport_uuid),
        Some(&serde_json::json!("AB123456"))
    );

    // Verify source tracking
    assert!(context.get_sources(&first_name_uuid).is_some());
}

#[tokio::test]
async fn test_mixed_uuid_and_semantic_refs() {
    // Test DSL with both UUID and semantic attribute references

    let pool = sqlx::PgPool::connect_lazy("postgresql://test").unwrap();
    let validator = AttributeValidator::new();
    let service = AttributeService::from_pool(pool, validator);
    let executor = DslExecutor::new(service);

    let dsl = r#"
        (kyc.collect
            :first-name @attr{3020d46f-472c-5437-9647-1b0682c35935}
            :last-name @attr.identity.last_name
        )
    "#;

    let program = ob_poc::parser::parse_program(dsl).unwrap();
    let uuids = executor.extract_uuids(&program);

    // Should only extract the UUID format, not the semantic format
    assert_eq!(uuids.len(), 1);
    assert!(uuids.contains(&FirstName::uuid()));
}
