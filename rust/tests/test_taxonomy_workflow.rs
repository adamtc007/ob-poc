//! Integration test for complete taxonomy workflow
//!
//! Tests the full Product → Service → Resource → Onboarding flow

use ob_poc::database::DatabaseManager;
use ob_poc::taxonomy::{DslOperation, TaxonomyDslManager};
use serde_json::Value;
use std::collections::HashMap;
use uuid::Uuid;

async fn setup_database() -> DatabaseManager {
    let db_url = std::env::var("DATABASE_URL")
        .unwrap_or_else(|_| "postgresql://localhost:5432/ob-poc".to_string());

    std::env::set_var("DATABASE_URL", db_url);

    DatabaseManager::with_default_config()
        .await
        .expect("Failed to connect to database")
}

async fn create_test_cbu(db: &DatabaseManager) -> Uuid {
    let cbu_id = Uuid::new_v4();

    sqlx::query!(
        r#"
        INSERT INTO "ob-poc".cbus (cbu_id, name, description)
        VALUES ($1, 'Test Taxonomy CBU', 'Test client for taxonomy workflow')
        "#,
        cbu_id
    )
    .execute(db.pool())
    .await
    .expect("Failed to create test CBU");

    cbu_id
}

async fn cleanup_test_data(db: &DatabaseManager, request_id: Uuid, cbu_id: Uuid) {
    let _ = sqlx::query!(
        r#"DELETE FROM "ob-poc".onboarding_requests WHERE request_id = $1"#,
        request_id
    )
    .execute(db.pool())
    .await;

    let _ = sqlx::query!(
        r#"DELETE FROM "ob-poc".cbus WHERE cbu_id = $1"#,
        cbu_id
    )
    .execute(db.pool())
    .await;
}

#[tokio::test]
#[ignore] // Requires database - run with: cargo test --features database -- --ignored
async fn test_complete_taxonomy_workflow() {
    // Setup
    let db = setup_database().await;
    let manager = TaxonomyDslManager::new(db.pool().clone());
    let cbu_id = create_test_cbu(&db).await;

    println!("🚀 Starting taxonomy workflow test");
    println!("   CBU ID: {}", cbu_id);

    // Step 1: Create onboarding request
    println!("\n📝 Step 1: Creating onboarding request");
    let result = manager
        .execute(DslOperation::CreateOnboarding {
            cbu_id,
            initiated_by: "test_agent".to_string(),
        })
        .await
        .expect("Failed to create onboarding");

    assert!(result.success, "Create onboarding failed: {}", result.message);
    assert_eq!(result.current_state, Some("draft".to_string()));
    println!("   ✅ Request created: {}", result.message);
    println!("   DSL: {}", result.dsl_fragment.as_ref().unwrap());

    let request_id: Uuid = serde_json::from_value(
        result.data.as_ref().unwrap()["request_id"].clone()
    )
    .expect("Failed to parse request_id");

    // Step 2: Add products
    println!("\n📦 Step 2: Adding products");
    let result = manager
        .execute(DslOperation::AddProducts {
            request_id,
            product_codes: vec!["CUSTODY_INST".to_string()],
        })
        .await
        .expect("Failed to add products");

    assert!(result.success, "Add products failed: {}", result.message);
    assert_eq!(result.current_state, Some("products_selected".to_string()));
    println!("   ✅ {}", result.message);
    println!("   DSL: {}", result.dsl_fragment.as_ref().unwrap());

    let products: Vec<Value> = serde_json::from_value(result.data.unwrap())
        .expect("Failed to parse products");
    let product_id: Uuid = serde_json::from_value(products[0]["product_id"].clone())
        .expect("Failed to parse product_id");

    // Step 3: Discover services
    println!("\n🔍 Step 3: Discovering services");
    let result = manager
        .execute(DslOperation::DiscoverServices {
            request_id,
            product_id,
        })
        .await
        .expect("Failed to discover services");

    assert!(result.success, "Discover services failed: {}", result.message);
    assert_eq!(
        result.current_state,
        Some("services_discovered".to_string())
    );
    println!("   ✅ {}", result.message);

    let services: Vec<Value> = serde_json::from_value(result.data.unwrap())
        .expect("Failed to parse services");

    println!("   Services found: {}", services.len());
    for service in &services {
        let service_obj = &service["service"];
        println!(
            "     - {} ({})",
            service_obj["name"].as_str().unwrap_or("unknown"),
            service_obj["service_code"].as_str().unwrap_or("unknown")
        );
    }

    // Step 4: Configure service
    println!("\n⚙️  Step 4: Configuring service");
    let mut options = HashMap::new();
    options.insert(
        "markets".to_string(),
        serde_json::json!(["US_EQUITY", "EU_EQUITY"]),
    );
    options.insert("speed".to_string(), serde_json::json!("T1"));

    let result = manager
        .execute(DslOperation::ConfigureService {
            request_id,
            service_code: "SETTLEMENT".to_string(),
            options,
        })
        .await
        .expect("Failed to configure service");

    assert!(result.success, "Configure service failed: {}", result.message);
    println!("   ✅ {}", result.message);
    println!("   DSL: {}", result.dsl_fragment.as_ref().unwrap());

    // Step 5: Get status
    println!("\n📊 Step 5: Checking status");
    let result = manager
        .execute(DslOperation::GetStatus { request_id })
        .await
        .expect("Failed to get status");

    assert!(result.success, "Get status failed: {}", result.message);
    println!("   ✅ Current state: {}", result.current_state.as_ref().unwrap());
    println!("   Next operations: {:?}", result.next_operations);

    // Cleanup
    println!("\n🧹 Cleaning up test data");
    cleanup_test_data(&db, request_id, cbu_id).await;
    println!("   ✅ Cleanup complete");

    println!("\n🎉 Taxonomy workflow test completed successfully!");
}

#[tokio::test]
#[ignore] // Requires database
async fn test_product_discovery() {
    let db = setup_database().await;
    let repo = db.taxonomy_repository();

    println!("🔍 Testing product discovery");

    let products = repo
        .list_active_products()
        .await
        .expect("Failed to list products");

    println!("   Found {} products:", products.len());
    for product in &products {
        println!(
            "     - {} ({})",
            product.name,
            product.product_code.as_deref().unwrap_or("no code")
        );
    }

    assert!(!products.is_empty(), "No products found in database");
    println!("   ✅ Product discovery test passed");
}

#[tokio::test]
#[ignore] // Requires database
async fn test_service_options() {
    let db = setup_database().await;
    let repo = db.taxonomy_repository();

    println!("⚙️  Testing service options");

    let service = repo
        .get_service_by_code("SETTLEMENT")
        .await
        .expect("Failed to get service")
        .expect("SETTLEMENT service not found");

    println!("   Service: {}", service.name);

    let options = repo
        .get_service_options(service.service_id)
        .await
        .expect("Failed to get options");

    println!("   Options: {}", options.len());
    for option in &options {
        println!("     - {}: {}", option.option_key, option.option_type);

        let choices = repo
            .get_option_choices(option.option_def_id)
            .await
            .expect("Failed to get choices");

        println!("       Choices: {}", choices.len());
        for choice in &choices {
            println!("         • {}", choice.choice_value);
        }
    }

    assert!(!options.is_empty(), "No options found for SETTLEMENT service");
    println!("   ✅ Service options test passed");
}
