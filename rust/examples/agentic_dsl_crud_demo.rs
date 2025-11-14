//! Agentic DSL CRUD Demo
//!
//! Demonstrates AI-powered natural language to DSL to database operations:
//! 1. Create CBU from natural language
//! 2. Connect entities to CBU
//! 3. Read and verify operations
//!
//! Run with: cargo run --example agentic_dsl_crud_demo --features database

use ob_poc::services::agentic_dsl_crud::{
    AgenticDslService, CrudStatement, DslParser,
};
use sqlx::PgPool;
use uuid::Uuid;

#[tokio::main]
async fn main() -> Result<(), Box<dyn std::error::Error>> {
    // Initialize tracing for logging
    tracing_subscriber::fmt::init();

    println!("🚀 Agentic DSL CRUD Demo\n");
    println!("{}", "=".repeat(80));

    // =================================================================
    // STEP 1: Database connection
    // =================================================================
    println!("\n📦 Step 1: Connecting to database...");
    
    let database_url = std::env::var("DATABASE_URL")
        .unwrap_or_else(|_| "postgresql://localhost:5432/ob-poc".to_string());
    
    let pool = PgPool::connect(&database_url).await?;
    println!("✅ Connected to database");

    // =================================================================
    // STEP 2: Create Agentic DSL Service
    // =================================================================
    println!("\n🤖 Step 2: Initializing agentic DSL service...");
    
    let agentic_service = AgenticDslService::new(pool.clone());
    println!("✅ Service initialized");

    // =================================================================
    // STEP 3: Test DSL Parser with various instructions
    // =================================================================
    println!("\n📝 Step 3: Testing DSL parser...");

    // Test 1: Create CBU from natural language
    let instruction1 = "Create a CBU with Nature and Purpose 'Hedge Fund Management' and Source of Funds 'Investment Returns'";
    println!("\n  Input: {}", instruction1);
    
    match DslParser::parse(instruction1) {
        Ok(CrudStatement::CreateCbu(create)) => {
            println!("  ✅ Parsed as CreateCbu:");
            println!("     Nature & Purpose: {}", create.nature_purpose);
            println!("     Source of Funds: {}", create.source_of_funds);
        }
        Ok(other) => println!("  ⚠️  Unexpected statement type: {:?}", other),
        Err(e) => println!("  ❌ Parse error: {}", e),
    }

    // Test 2: Connect entity (using placeholder UUIDs)
    let entity_id = Uuid::new_v4();
    let cbu_id = Uuid::new_v4();
    let role_id = Uuid::new_v4();
    
    let instruction2 = format!(
        "Connect entity {} to CBU {} as role {}",
        entity_id, cbu_id, role_id
    );
    println!("\n  Input: {}", instruction2);
    
    match DslParser::parse(&instruction2) {
        Ok(CrudStatement::ConnectEntity(connect)) => {
            println!("  ✅ Parsed as ConnectEntity:");
            println!("     Entity ID: {}", connect.entity_id);
            println!("     CBU ID: {}", connect.cbu_id);
            println!("     Role ID: {}", connect.role_id);
        }
        Ok(other) => println!("  ⚠️  Unexpected statement type: {:?}", other),
        Err(e) => println!("  ❌ Parse error: {}", e),
    }

    // Test 3: Read CBU
    let read_cbu_id = Uuid::new_v4();
    let instruction3 = format!("Read CBU {}", read_cbu_id);
    println!("\n  Input: {}", instruction3);
    
    match DslParser::parse(&instruction3) {
        Ok(CrudStatement::ReadCbu(read)) => {
            println!("  ✅ Parsed as ReadCbu:");
            println!("     CBU ID: {}", read.cbu_id);
        }
        Ok(other) => println!("  ⚠️  Unexpected statement type: {:?}", other),
        Err(e) => println!("  ❌ Parse error: {}", e),
    }

    // =================================================================
    // STEP 4: Execute CBU creation (if tables exist)
    // =================================================================
    println!("\n🔧 Step 4: Testing CBU creation...");
    
    let nature_purpose = "Real Estate Investment Trust";
    let source_of_funds = "Property Sales";
    
    match agentic_service.create_cbu_from_natural_language(
        "Create investment trust CBU",
        nature_purpose,
        source_of_funds,
    ).await {
        Ok(cbu_id) => {
            println!("  ✅ CBU created successfully!");
            println!("     CBU ID: {}", cbu_id);
            println!("     Nature & Purpose: {}", nature_purpose);
            println!("     Source of Funds: {}", source_of_funds);
            
            // Clean up
            sqlx::query(r#"DELETE FROM "ob-poc".cbu_creation_log WHERE cbu_id = $1"#)
                .bind(cbu_id)
                .execute(&pool)
                .await
                .ok();
            sqlx::query(r#"DELETE FROM "ob-poc".cbus WHERE cbu_id = $1"#)
                .bind(cbu_id)
                .execute(&pool)
                .await
                .ok();
            println!("  🧹 Cleanup complete");
        }
        Err(e) => {
            println!("  ⚠️  CBU creation skipped (tables may not exist)");
            println!("     Error: {}", e);
            println!("     Run migration: sql/migrations/007_agentic_dsl_crud.sql");
        }
    }

    // =================================================================
    // STEP 5: Demonstrate more parsing examples
    // =================================================================
    println!("\n📖 Step 5: Additional parsing examples...");

    let examples = vec![
        "Create a CBU with nature_purpose \"Private Banking\" and source_of_funds \"Salary\"",
        "Update CBU 12345678-1234-1234-1234-123456789012 set description = 'Updated'",
        "Get CBU 12345678-1234-1234-1234-123456789012",
    ];

    for (i, example) in examples.iter().enumerate() {
        println!("\n  Example {}: {}", i + 1, example);
        match DslParser::parse(example) {
            Ok(stmt) => println!("     ✅ Parsed successfully: {:?}", stmt),
            Err(e) => println!("     ❌ Parse error: {}", e),
        }
    }

    // =================================================================
    // Summary
    // =================================================================
    println!("\n{}", "=".repeat(80));
    println!("🎉 Demo Complete!\n");
    println!("Key Features Demonstrated:");
    println!("  ✓ Natural language to DSL parsing");
    println!("  ✓ CBU creation from instructions");
    println!("  ✓ Entity connection operations");
    println!("  ✓ Read/Update operations");
    println!("  ✓ Pattern matching and UUID extraction");
    println!("\n{}", "=".repeat(80));

    Ok(())
}
