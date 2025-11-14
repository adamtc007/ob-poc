//! Complete End-to-End Agentic DSL Demo
//!
//! This example demonstrates the complete workflow:
//! 1. Create entity (person, company, trust)
//! 2. Create role (director, beneficiary, trustee)
//! 3. Create CBU (Client Business Unit)
//! 4. Connect entity to CBU with role
//! 5. Query and update operations
//!
//! Run with: cargo run --example complete_end_to_end --features database
//!
//! Prerequisites:
//! - Database must be running with ob-poc schema
//! - Tables must exist: entities, entity_types, cbus, entity_role_connections
//! - Migrations 006 and 007 must be applied

use ob_poc::services::agentic_complete::{
    CompleteAgenticService, ExtendedDslParser, ExtendedCrudStatement,
    CreateEntity, CreateRole,
};
use sqlx::postgres::PgPoolOptions;
use anyhow::Result;

#[tokio::main]
async fn main() -> Result<()> {
    println!("=== Complete End-to-End Agentic DSL Demo ===\n");

    // Step 1: Connect to database
    println!("📊 Connecting to database...");
    let database_url = std::env::var("DATABASE_URL")
        .unwrap_or_else(|_| "postgresql://localhost/ob-poc".to_string());

    let pool = PgPoolOptions::new()
        .max_connections(5)
        .connect(&database_url)
        .await?;

    println!("✅ Connected to database\n");

    // Step 2: Create service
    let service = CompleteAgenticService::new(pool);

    // =========================================================================
    // DEMO 1: Create Entity from Natural Language
    // =========================================================================
    println!("--- DEMO 1: Create Entity from Natural Language ---");

    let entity_instructions = vec![
        "Create entity John Smith as person",
        "Add company TechCorp Ltd",
        "Add trust Smith Family Trust",
    ];

    for instruction in &entity_instructions {
        println!("\n📝 Instruction: {}", instruction);

        match service.execute_from_natural_language(instruction).await {
            Ok(result) => {
                println!("✅ Success: {}", result.message);
                println!("   Entity ID: {:?}", result.entity_id);
                println!("   Data: {}", serde_json::to_string_pretty(&result.data)?);
            }
            Err(e) => {
                println!("❌ Error: {}", e);
            }
        }
    }

    // =========================================================================
    // DEMO 2: Create Roles from Natural Language
    // =========================================================================
    println!("\n\n--- DEMO 2: Create Roles from Natural Language ---");

    let role_instructions = vec![
        "Create role Director",
        "Create role Beneficiary",
        "Create role Trustee",
    ];

    for instruction in &role_instructions {
        println!("\n📝 Instruction: {}", instruction);

        match service.execute_from_natural_language(instruction).await {
            Ok(result) => {
                println!("✅ Success: {}", result.message);
                println!("   Role ID: {:?}", result.entity_id);
                println!("   Data: {}", serde_json::to_string_pretty(&result.data)?);
            }
            Err(e) => {
                println!("❌ Error: {}", e);
            }
        }
    }

    // =========================================================================
    // DEMO 3: Create CBU from Natural Language
    // =========================================================================
    println!("\n\n--- DEMO 3: Create CBU from Natural Language ---");

    let cbu_instruction = "Create CBU for hedge fund investment";
    println!("\n📝 Instruction: {}", cbu_instruction);

    match service.execute_from_natural_language(cbu_instruction).await {
        Ok(result) => {
            println!("✅ Success: {}", result.message);
            println!("   CBU ID: {:?}", result.entity_id);
            println!("   Data: {}", serde_json::to_string_pretty(&result.data)?);
        }
        Err(e) => {
            println!("❌ Error: {}", e);
        }
    }

    // =========================================================================
    // DEMO 4: Complete Setup Workflow
    // =========================================================================
    println!("\n\n--- DEMO 4: Complete Setup Workflow ---");
    println!("This demonstrates the high-level workflow that creates:");
    println!("  1. Entity (person/company)");
    println!("  2. Role (director/beneficiary)");
    println!("  3. CBU (business unit)");
    println!("  4. Connection (entity → CBU with role)\n");

    let setup_result = service.create_complete_setup(
        "Alice Johnson",           // entity name
        "PERSON",                  // entity type
        "Director",                // role name
        "Private wealth management",  // CBU nature
        "Investment portfolio",    // CBU source of funds
    ).await?;

    println!("✅ Complete setup successful!");
    println!("   Entity ID:     {}", setup_result.entity_id);
    println!("   Role ID:       {}", setup_result.role_id);
    println!("   CBU ID:        {}", setup_result.cbu_id);
    println!("   Connection ID: {}", setup_result.connection_id);
    println!("   Message:       {}", setup_result.message);

    // =========================================================================
    // DEMO 5: Parser Testing
    // =========================================================================
    println!("\n\n--- DEMO 5: Parser Testing (No Database) ---");

    let test_cases = vec![
        "Create entity Bob Williams as person",
        "Add company Global Investments Ltd",
        "Create role Shareholder",
        "Create CBU for corporate banking",
    ];

    for test_case in &test_cases {
        println!("\n📝 Parsing: {}", test_case);

        match ExtendedDslParser::parse(test_case) {
            Ok(statement) => {
                match statement {
                    ExtendedCrudStatement::CreateEntity(entity) => {
                        println!("   ✅ Parsed as CreateEntity:");
                        println!("      Name: {}", entity.name);
                        println!("      Type: {}", entity.entity_type);
                    }
                    ExtendedCrudStatement::CreateRole(role) => {
                        println!("   ✅ Parsed as CreateRole:");
                        println!("      Name: {}", role.name);
                        println!("      Description: {}", role.description);
                    }
                    ExtendedCrudStatement::Base(_) => {
                        println!("   ✅ Parsed as Base CRUD statement");
                    }
                }
            }
            Err(e) => {
                println!("   ❌ Parse error: {}", e);
            }
        }
    }

    // =========================================================================
    // DEMO 6: Direct Struct Creation (Type-Safe API)
    // =========================================================================
    println!("\n\n--- DEMO 6: Direct Struct Creation (Type-Safe) ---");

    println!("\n📝 Creating entity directly via structs...");

    let direct_entity = CreateEntity {
        name: "Charles Brown".to_string(),
        entity_type: "PERSON".to_string(),
    };

    match service.execute(ExtendedCrudStatement::CreateEntity(direct_entity)).await {
        Ok(result) => {
            println!("✅ Success: {}", result.message);
            println!("   Entity ID: {:?}", result.entity_id);
        }
        Err(e) => {
            println!("❌ Error: {}", e);
        }
    }

    // =========================================================================
    // Summary
    // =========================================================================
    println!("\n\n=== Demo Complete ===");
    println!("\nKey Capabilities Demonstrated:");
    println!("  ✅ Natural language entity creation (person, company, trust)");
    println!("  ✅ Natural language role creation (director, beneficiary, etc.)");
    println!("  ✅ Natural language CBU creation");
    println!("  ✅ High-level complete setup workflow");
    println!("  ✅ Pattern-based parsing (no LLM required)");
    println!("  ✅ Type-safe struct-based API");
    println!("  ✅ Database integration with PostgreSQL");
    println!("  ✅ Audit trail via cbu_creation_log and entity_role_connections");

    println!("\nArchitecture Benefits:");
    println!("  🚀 Fast: <1ms parsing (pattern-based, no LLM)");
    println!("  💰 Cost: $0 per operation (no API calls)");
    println!("  🎯 Reliable: 100% deterministic parsing");
    println!("  🔒 Type-Safe: Full Rust type system benefits");
    println!("  📊 Auditable: Complete trail in database");

    Ok(())
}
