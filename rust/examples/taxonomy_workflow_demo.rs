//! Complete Taxonomy Workflow Demo
//!
//! Demonstrates the full Product-Service-Resource taxonomy system with
//! incremental DSL generation and state management.
//!
//! Run with: cargo run --example taxonomy_workflow_demo --features database

use ob_poc::database::DatabaseManager;
use ob_poc::taxonomy::{DslOperation, TaxonomyDslManager};
use std::collections::HashMap;
use uuid::Uuid;

#[tokio::main]
async fn main() -> anyhow::Result<()> {
    // Initialize logging
    tracing_subscriber::fmt()
        .with_max_level(tracing::Level::INFO)
        .init();

    println!("\n╔══════════════════════════════════════════════════════════╗");
    println!("║   Product-Service-Resource Taxonomy Workflow Demo       ║");
    println!("╚══════════════════════════════════════════════════════════╝\n");

    // Setup database connection
    let db = DatabaseManager::with_default_config().await?;
    println!("✅ Connected to database\n");

    // Create taxonomy manager
    let manager = TaxonomyDslManager::new(db.pool().clone());

    // Create a test CBU
    let cbu_id = create_demo_cbu(&db).await?;
    println!("✅ Created demo CBU: {}\n", cbu_id);

    println!("━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━\n");

    // ============================================
    // Step 1: Create Onboarding Request
    // ============================================
    println!("📝 STEP 1: Creating Onboarding Request");
    println!("   Action: Initialize new onboarding workflow");

    let result = manager
        .execute(DslOperation::CreateOnboarding {
            cbu_id,
            initiated_by: "demo_agent".to_string(),
        })
        .await?;

    print_result(&result);

    let request_id: Uuid = serde_json::from_value(
        result.data.as_ref().unwrap()["request_id"].clone()
    )?;

    println!("\n━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━\n");

    // ============================================
    // Step 2: Add Products
    // ============================================
    println!("📦 STEP 2: Adding Products");
    println!("   Action: Select Institutional Custody product");

    let result = manager
        .execute(DslOperation::AddProducts {
            request_id,
            product_codes: vec!["CUSTODY_INST".to_string()],
        })
        .await?;

    print_result(&result);

    let products: Vec<serde_json::Value> = serde_json::from_value(result.data.unwrap())?;
    let product_id: Uuid = serde_json::from_value(products[0]["product_id"].clone())?;

    println!("\n━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━\n");

    // ============================================
    // Step 3: Discover Services
    // ============================================
    println!("🔍 STEP 3: Discovering Available Services");
    println!("   Action: Find all services for selected product");

    let result = manager
        .execute(DslOperation::DiscoverServices {
            request_id,
            product_id,
        })
        .await?;

    print_result(&result);

    let services: Vec<serde_json::Value> = serde_json::from_value(result.data.unwrap())?;
    println!("\n   📋 Discovered Services:");
    for (idx, service) in services.iter().enumerate() {
        let service_obj = &service["service"];
        let options = &service["options"];

        println!(
            "   {}. {} ({})",
            idx + 1,
            service_obj["name"].as_str().unwrap_or("unknown"),
            service_obj["service_code"].as_str().unwrap_or("unknown")
        );

        if let Some(options_array) = options.as_array() {
            for option in options_array {
                let def = &option["definition"];
                println!(
                    "      • Option: {} ({})",
                    def["option_key"].as_str().unwrap_or("unknown"),
                    def["option_type"].as_str().unwrap_or("unknown")
                );

                if let Some(choices) = option["choices"].as_array() {
                    println!("        Choices:");
                    for choice in choices {
                        println!(
                            "          - {}",
                            choice["choice_value"].as_str().unwrap_or("unknown")
                        );
                    }
                }
            }
        }
    }

    println!("\n━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━\n");

    // ============================================
    // Step 4: Configure Services
    // ============================================
    println!("⚙️  STEP 4: Configuring Settlement Service");
    println!("   Action: Configure markets and settlement speed");

    let mut options = HashMap::new();
    options.insert(
        "markets".to_string(),
        serde_json::json!(["US_EQUITY", "EU_EQUITY"]),
    );
    options.insert("speed".to_string(), serde_json::json!("T1"));

    println!("   Selected Options:");
    println!("     • Markets: US Equities, European Equities");
    println!("     • Speed: T+1 (Next Day)\n");

    let result = manager
        .execute(DslOperation::ConfigureService {
            request_id,
            service_code: "SETTLEMENT".to_string(),
            options,
        })
        .await?;

    print_result(&result);

    println!("\n━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━\n");

    // ============================================
    // Step 5: Check Status
    // ============================================
    println!("📊 STEP 5: Checking Workflow Status");

    let result = manager
        .execute(DslOperation::GetStatus { request_id })
        .await?;

    println!("   Current State: {}", result.current_state.as_ref().unwrap());
    println!("   Next Available Operations:");
    for op in &result.next_operations {
        println!("     • {}", op);
    }

    println!("\n━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━\n");

    // ============================================
    // Cleanup
    // ============================================
    println!("🧹 Cleaning Up Demo Data");
    cleanup_demo_data(&db, request_id, cbu_id).await?;
    println!("   ✅ Cleanup complete\n");

    println!("━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━\n");
    println!("🎉 Taxonomy Workflow Demo Completed Successfully!\n");
    println!("Key Features Demonstrated:");
    println!("  ✓ Incremental DSL generation at each step");
    println!("  ✓ State management and transitions");
    println!("  ✓ Product-Service discovery");
    println!("  ✓ Service option configuration");
    println!("  ✓ Multi-market resource capabilities\n");

    Ok(())
}

fn print_result(result: &ob_poc::taxonomy::DslResult) {
    println!("   ✅ Success: {}", result.message);
    if let Some(state) = &result.current_state {
        println!("   📌 State: {}", state);
    }
    if let Some(dsl) = &result.dsl_fragment {
        println!("   📝 Generated DSL:");
        for line in dsl.lines() {
            println!("      {}", line);
        }
    }
    if !result.next_operations.is_empty() {
        println!("   ➡️  Next Operations: {}", result.next_operations.join(", "));
    }
}

async fn create_demo_cbu(db: &DatabaseManager) -> anyhow::Result<Uuid> {
    let cbu_id = Uuid::new_v4();

    sqlx::query!(
        r#"
        INSERT INTO "ob-poc".cbus (cbu_id, name, description)
        VALUES ($1, 'Acme Hedge Fund', 'Demo hedge fund for taxonomy workflow')
        "#,
        cbu_id
    )
    .execute(db.pool())
    .await?;

    Ok(cbu_id)
}

async fn cleanup_demo_data(
    db: &DatabaseManager,
    request_id: Uuid,
    cbu_id: Uuid,
) -> anyhow::Result<()> {
    sqlx::query!(
        r#"DELETE FROM "ob-poc".onboarding_requests WHERE request_id = $1"#,
        request_id
    )
    .execute(db.pool())
    .await?;

    sqlx::query!(
        r#"DELETE FROM "ob-poc".cbus WHERE cbu_id = $1"#,
        cbu_id
    )
    .execute(db.pool())
    .await?;

    Ok(())
}
