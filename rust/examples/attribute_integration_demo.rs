//! Attribute Integration Demo
//!
//! This example demonstrates the complete Phase 1-3 integration:
//! - Phase 1: Type-safe domain attributes
//! - Phase 2: DSL parsing and validation
//! - Phase 3: Database persistence
//!
//! Run with: cargo run --example attribute_integration_demo --features database

use ob_poc::database::attribute_repository::AttributeRepository;
use ob_poc::domains::attributes::kyc::{FirstName, LastName, LegalEntityName};
use ob_poc::domains::attributes::types::{
    AttributeCategory, AttributeMetadata, AttributeType, DataType, ValidationRules,
};
use ob_poc::domains::attributes::validator::AttributeValidator;
use ob_poc::services::attribute_service::AttributeService;
use sqlx::PgPool;
use uuid::Uuid;

#[tokio::main]
async fn main() -> Result<(), Box<dyn std::error::Error>> {
    println!("🚀 Attribute Integration Demo - Phase 1-3 Complete Integration\n");

    // Setup: Connect to database
    let database_url = std::env::var("DATABASE_URL")
        .unwrap_or_else(|_| "postgresql://localhost/ob_poc".to_string());

    println!("📊 Connecting to database...");
    let pool = PgPool::connect(&database_url).await?;
    println!("✅ Database connected\n");

    // Phase 1: Create type-safe domain attributes
    println!("=== PHASE 1: Type-Safe Domain Attributes ===");
    println!("✓ FirstName: {}", FirstName::ID);
    println!("✓ LastName: {}", LastName::ID);
    println!("✓ LegalEntityName: {}", LegalEntityName::ID);
    println!();

    // Phase 2: Setup DSL validator with attribute registry
    println!("=== PHASE 2: DSL Validator Setup ===");
    let mut validator = AttributeValidator::new();

    // Register all KYC attributes
    validator.register(FirstName::metadata());
    validator.register(LastName::metadata());
    validator.register(LegalEntityName::metadata());

    println!("✓ Registered {} attributes in validator", validator.count());
    println!();

    // Phase 3: Create attribute repository with database
    println!("=== PHASE 3: Database Repository Setup ===");
    let repository = AttributeRepository::new(pool.clone());
    println!("✓ AttributeRepository initialized with database connection");
    println!();

    // INTEGRATION: Create AttributeService (bridges all phases)
    println!("=== INTEGRATION: AttributeService Creation ===");
    let service = AttributeService::new(repository, validator);
    println!("✅ AttributeService created - Phase 1-3 fully integrated!\n");

    // Demo 1: Type-safe attribute storage
    println!("=== DEMO 1: Type-Safe Attribute Storage ===");
    let entity_id = Uuid::new_v4();
    println!("Entity ID: {}", entity_id);

    // Store FirstName
    let first_name_value = "John".to_string();
    println!("\n1. Storing FirstName: '{}'", first_name_value);
    let id1 = service
        .set_attribute::<FirstName>(entity_id, first_name_value.clone(), Some("demo"))
        .await?;
    println!("   ✓ Stored with ID: {}", id1);

    // Store LastName
    let last_name_value = "Doe".to_string();
    println!("\n2. Storing LastName: '{}'", last_name_value);
    let id2 = service
        .set_attribute::<LastName>(entity_id, last_name_value.clone(), Some("demo"))
        .await?;
    println!("   ✓ Stored with ID: {}", id2);

    // Retrieve attributes
    println!("\n3. Retrieving stored attributes:");
    let retrieved_first = service.get_attribute::<FirstName>(entity_id).await?;
    let retrieved_last = service.get_attribute::<LastName>(entity_id).await?;

    println!("   ✓ FirstName: {:?}", retrieved_first);
    println!("   ✓ LastName: {:?}", retrieved_last);

    // Verify values match
    assert_eq!(retrieved_first, Some(first_name_value));
    assert_eq!(retrieved_last, Some(last_name_value));
    println!("   ✅ All values match!");
    println!();

    // Demo 2: DSL Processing with automatic persistence
    println!("=== DEMO 2: DSL Processing → Database Persistence ===");
    let entity_id_2 = Uuid::new_v4();
    println!("Entity ID: {}", entity_id_2);

    let dsl = format!(
        r#"(entity.register
            :entity-id "{}"
            :first-name @attr.identity.first_name
            :last-name @attr.identity.last_name
        )"#,
        entity_id_2
    );

    println!("\nDSL Input:\n{}", dsl);
    println!("\nProcessing DSL...");

    let result = service
        .process_attribute_dsl(entity_id_2, &dsl, Some("dsl_demo"))
        .await?;

    println!("\n✅ Processing Results:");
    println!("   Forms processed: {}", result.forms_processed);
    println!("   Validation passed: {}", result.validation_passed);
    println!("   Attributes extracted: {}", result.attributes_extracted);
    println!("   Attributes persisted: {}", result.attributes_persisted);
    println!("   Persisted IDs: {:?}", result.persisted_attribute_ids);
    println!();

    // Demo 3: Batch attribute retrieval
    println!("=== DEMO 3: Batch Attribute Retrieval ===");
    let attr_ids = vec![FirstName::ID, LastName::ID];

    let attributes = service.get_many_attributes(entity_id, &attr_ids).await?;

    println!("Retrieved {} attributes:", attributes.len());
    for (id, value) in &attributes {
        println!("   {} = {}", id, value);
    }
    println!();

    // Demo 4: Attribute history
    println!("=== DEMO 4: Attribute History (Temporal Versioning) ===");

    // Update the same attribute multiple times
    println!("Creating history by updating FirstName...");
    service
        .set_attribute::<FirstName>(entity_id, "Jane".to_string(), Some("updater1"))
        .await?;
    println!("   ✓ Updated to 'Jane'");

    tokio::time::sleep(tokio::time::Duration::from_millis(100)).await;

    service
        .set_attribute::<FirstName>(entity_id, "Janet".to_string(), Some("updater2"))
        .await?;
    println!("   ✓ Updated to 'Janet'");

    let history = service
        .get_attribute_history::<FirstName>(entity_id, 10)
        .await?;
    println!("\nAttribute history ({} entries):", history.len());
    for (i, entry) in history.iter().enumerate() {
        println!(
            "   {}. Value: '{}', From: {}, By: {}",
            i + 1,
            entry.value,
            entry.effective_from.format("%Y-%m-%d %H:%M:%S"),
            entry.created_by
        );
    }
    println!();

    // Demo 5: DSL Generation
    println!("=== DEMO 5: DSL Generation from Domain ===");
    let generated_get = service.generate_get_attribute_dsl::<FirstName>(entity_id)?;
    println!("Generated GET DSL:\n{}", generated_get);

    let generated_set =
        service.generate_set_attribute_dsl::<FirstName>(entity_id, &"Test".to_string())?;
    println!("\nGenerated SET DSL:\n{}", generated_set);
    println!();

    // Summary
    println!("=== INTEGRATION SUMMARY ===");
    println!(
        "✅ Phase 1 (Domain Types): Working - Type-safe attributes with compile-time checking"
    );
    println!("✅ Phase 2 (DSL Integration): Working - Validation, parsing, and building");
    println!("✅ Phase 3 (Database): Working - SQLX persistence with temporal versioning");
    println!("✅ AttributeService: Successfully bridges all three phases!");
    println!();
    println!("🎉 Phase 1-3 Database Integration: COMPLETE");

    Ok(())
}
