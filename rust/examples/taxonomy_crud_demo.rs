//! Taxonomy CRUD Demo - Natural Language DSL Operations
//! Run with: cargo run --example taxonomy_crud_demo --features database

use anyhow::Result;
use ob_poc::services::taxonomy_crud::TaxonomyCrudService;
use sqlx::PgPool;
use std::env;
use uuid::Uuid;

#[tokio::main]
async fn main() -> Result<()> {
    println!("🚀 Taxonomy CRUD Demo - Natural Language DSL Operations\n");
    println!("{}", "=".repeat(80));
    
    let database_url = env::var("DATABASE_URL").expect("DATABASE_URL must be set");
    let pool = PgPool::connect(&database_url).await?;
    println!("✅ Connected to database\n");
    
    // Cleanup any previous test data
    cleanup_previous_tests(&pool).await?;
    
    let service = TaxonomyCrudService::new(pool.clone());
    
    println!("\n📦 DEMO 1: Product CRUD Operations");
    println!("{}", "-".repeat(80));
    demo_product_operations(&service).await?;
    
    println!("\n🔧 DEMO 2: Service Operations");
    println!("{}", "-".repeat(80));
    demo_service_operations(&service).await?;
    
    println!("\n📋 DEMO 3: Complete Onboarding Workflow");
    println!("{}", "-".repeat(80));
    demo_onboarding_workflow(&service, &pool).await?;
    
    println!("\n✅ All demos completed successfully!");
    println!("{}", "=".repeat(80));
    
    Ok(())
}

async fn cleanup_previous_tests(pool: &PgPool) -> Result<()> {
    // Clean up any previous test data
    sqlx::query!(r#"DELETE FROM "ob-poc".products WHERE product_code LIKE '%DEMO%' OR product_code LIKE '%_WF'"#)
        .execute(pool).await?;
    sqlx::query!(r#"DELETE FROM "ob-poc".services WHERE service_code LIKE '%DEMO%' OR service_code LIKE '%_WF'"#)
        .execute(pool).await?;
    sqlx::query!(r#"DELETE FROM "ob-poc".cbus WHERE nature_purpose LIKE '%Test CBU%'"#)
        .execute(pool).await?;
    Ok(())
}

async fn demo_product_operations(service: &TaxonomyCrudService) -> Result<()> {
    let test_id = Uuid::new_v4().to_string()[..8].to_string();
    
    println!("\n1️⃣ Creating product via natural language...");
    let instruction = format!(
        "Create a product called Inst Custody {} with code CUSTODY_DEMO_{}",
        test_id, test_id
    );
    println!("   Instruction: {}", instruction);
    
    let result = service.execute(&instruction).await?;
    println!("   ✓ Result: {}", result.message);
    println!("   ✓ Entity ID: {:?}", result.entity_id);
    println!("   ✓ Execution time: {}ms", result.execution_time_ms);
    
    let product_id = result.entity_id.expect("Product ID should be returned");
    
    println!("\n2️⃣ Reading product by ID...");
    let read_instruction = format!("Read product {}", product_id);
    let result = service.execute(&read_instruction).await?;
    println!("   ✓ Found: {}", result.success);
    if let Some(data) = result.data {
        println!("   ✓ Product name: {}", 
            data.get("name").and_then(|v| v.as_str()).unwrap_or("N/A"));
    }
    
    println!("\n3️⃣ Soft deleting product...");
    let delete_instruction = format!("Delete product {} soft", product_id);
    let result = service.execute(&delete_instruction).await?;
    println!("   ✓ Result: {}", result.message);
    
    cleanup_product(&service, product_id).await?;
    
    Ok(())
}

async fn demo_service_operations(service: &TaxonomyCrudService) -> Result<()> {
    let test_id = Uuid::new_v4().to_string()[..8].to_string();
    
    println!("\n1️⃣ Creating service with options...");
    let instruction = format!("Create service SETTLEMENT_DEMO_{} with options for markets and speed", test_id);
    println!("   Instruction: {}", instruction);
    
    let result = service.execute(&instruction).await?;
    println!("   ✓ Result: {}", result.message);
    println!("   ✓ Service ID: {:?}", result.entity_id);
    println!("   ✓ Execution time: {}ms", result.execution_time_ms);
    
    let service_id = result.entity_id.expect("Service ID should be returned");
    
    cleanup_service(&service, service_id).await?;
    
    Ok(())
}

async fn demo_onboarding_workflow(service: &TaxonomyCrudService, pool: &PgPool) -> Result<()> {
    let test_id = Uuid::new_v4().to_string()[..8].to_string();
    let cbu_id = create_test_cbu(pool, &test_id).await?;
    println!("   Setup: Created test CBU {}", cbu_id);
    
    println!("\n1️⃣ Creating onboarding workflow...");
    let instruction = format!("Create onboarding for CBU {}", cbu_id);
    let result = service.execute(&instruction).await?;
    println!("   ✓ Result: {}", result.message);
    println!("   ✓ Onboarding ID: {:?}", result.entity_id);
    println!("   ✓ Execution time: {}ms", result.execution_time_ms);
    
    let onboarding_id = result.entity_id.expect("Onboarding ID should be returned");
    
    println!("\n2️⃣ Creating and adding products...");
    let product_code = format!("CUSTODY_WF_{}", test_id);
    let create_product = format!("Create product Custody Test with code {}", product_code);
    let product_result = service.execute(&create_product).await?;
    println!("   ✓ Created product: {}", product_result.message);
    
    let add_products = format!("Add products {} to onboarding {}", product_code, onboarding_id);
    let result = service.execute(&add_products).await?;
    println!("   ✓ Result: {}", result.message);
    println!("   ✓ Execution time: {}ms", result.execution_time_ms);
    
    println!("\n3️⃣ Creating and configuring service...");
    let service_code = format!("SETTLEMENT_WF_{}", test_id);
    let create_service = format!("Create service {}", service_code);
    let service_result = service.execute(&create_service).await?;
    println!("   ✓ Created service: {}", service_result.message);
    
    let configure = format!(
        "Configure {} for onboarding {} with markets US and EU",
        service_code, onboarding_id
    );
    let result = service.execute(&configure).await?;
    println!("   ✓ Result: {}", result.message);
    println!("   ✓ Execution time: {}ms", result.execution_time_ms);
    
    println!("\n4️⃣ Querying workflow status...");
    let query = format!("Query workflow {}", onboarding_id);
    let result = service.execute(&query).await?;
    println!("   ✓ Result: {}", result.message);
    println!("   ✓ Execution time: {}ms", result.execution_time_ms);
    if let Some(data) = result.data {
        println!("   ✓ Products count: {}", 
            data.get("products").and_then(|v| v.as_array()).map(|a| a.len()).unwrap_or(0));
        println!("   ✓ Services count: {}",
            data.get("services").and_then(|v| v.as_array()).map(|a| a.len()).unwrap_or(0));
    }
    
    cleanup_workflow(pool, onboarding_id, cbu_id).await?;
    
    Ok(())
}

async fn create_test_cbu(pool: &PgPool, test_id: &str) -> Result<Uuid> {
    let cbu_id = Uuid::new_v4();
    sqlx::query!(
        r#"INSERT INTO "ob-poc".cbus (cbu_id, nature_purpose, source_of_funds)
        VALUES ($1, $2, 'Test Funds')"#,
        cbu_id,
        format!("Test CBU for Taxonomy Demo {}", test_id)
    )
    .execute(pool)
    .await?;
    Ok(cbu_id)
}

async fn cleanup_product(service: &TaxonomyCrudService, product_id: Uuid) -> Result<()> {
    let delete = format!("Delete product {} hard", product_id);
    service.execute(&delete).await?;
    Ok(())
}

async fn cleanup_service(service: &TaxonomyCrudService, service_id: Uuid) -> Result<()> {
    let _ = service.execute(&format!("Delete service {}", service_id)).await;
    Ok(())
}

async fn cleanup_workflow(pool: &PgPool, onboarding_id: Uuid, cbu_id: Uuid) -> Result<()> {
    sqlx::query!(r#"DELETE FROM "ob-poc".onboarding_requests WHERE request_id = $1"#, onboarding_id)
        .execute(pool).await?;
    sqlx::query!(r#"DELETE FROM "ob-poc".cbus WHERE cbu_id = $1"#, cbu_id)
        .execute(pool).await?;
    Ok(())
}
