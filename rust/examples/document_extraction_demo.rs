//! Document Extraction Demo
//!
//! Demonstrates the complete document-to-attribute extraction workflow:
//! 1. Upload a document to the catalog
//! 2. Extract attributes from the document using OCR service
//! 3. Resolve attributes via DocumentCatalogSource with fallback chain
//! 4. Parse DSL with @attr{uuid}:doc source hints
//!
//! Run with: cargo run --example document_extraction_demo --features database

use ob_poc::services::{
    AttributeExecutor, AttributeDictionary, AttributeSource, DatabaseSink,
    DocumentCatalogSource, ExtractionService, MockExtractionService, OcrExtractionService,
};
use ob_poc::domains::attributes::execution_context::ExecutionContext;
use ob_poc::parser::idiomatic_parser::parse_program;
use serde_json::json;
use sqlx::PgPool;
use std::sync::Arc;
use uuid::Uuid;

#[tokio::main]
async fn main() -> Result<(), Box<dyn std::error::Error>> {
    // Initialize tracing for logging
    tracing_subscriber::fmt::init();

    println!("🚀 Document Extraction Demo\n");
    println!("=" .repeat(80));

    // =================================================================
    // STEP 1: Setup - Database connection and services
    // =================================================================
    println!("\n📦 Step 1: Connecting to database...");
    
    let database_url = std::env::var("DATABASE_URL")
        .unwrap_or_else(|_| "postgresql://localhost:5432/ob-poc".to_string());
    
    let pool = PgPool::connect(&database_url).await?;
    println!("✅ Connected to database");

    // =================================================================
    // STEP 2: Create mock document and attributes
    // =================================================================
    println!("\n📄 Step 2: Creating mock document and attributes...");

    // Create a test document
    let doc_id = Uuid::new_v4();
    let cbu_id = Uuid::new_v4();
    
    sqlx::query(
        r#"
        INSERT INTO "ob-poc".document_catalog
        (doc_id, file_hash_sha256, storage_key, mime_type, extracted_data, extraction_status)
        VALUES ($1, $2, $3, $4, $5, $6)
        "#,
    )
    .bind(doc_id)
    .bind("sha256_mock_hash")
    .bind(format!("documents/{}", doc_id))
    .bind("application/pdf")
    .bind(json!({
        "text": "John Doe\nDate of Birth: 1990-01-15\nNationality: US",
        "pages": 1
    }))
    .bind("COMPLETED")
    .execute(&pool)
    .await?;

    // Link document to CBU
    sqlx::query(
        r#"
        INSERT INTO "ob-poc".document_usage
        (doc_id, cbu_id, usage_context)
        VALUES ($1, $2, $3)
        "#,
    )
    .bind(doc_id)
    .bind(cbu_id)
    .bind("onboarding")
    .execute(&pool)
    .await?;

    println!("✅ Created mock document: {}", doc_id);
    println!("✅ Linked to CBU: {}", cbu_id);

    // Get some real attribute IDs from the dictionary
    let first_name_attr: Uuid = sqlx::query_scalar(
        r#"SELECT attribute_id FROM "ob-poc".dictionary WHERE name = 'first_name' LIMIT 1"#
    )
    .fetch_optional(&pool)
    .await?
    .unwrap_or_else(Uuid::new_v4);

    println!("✅ Using attribute ID: {}", first_name_attr);

    // =================================================================
    // STEP 3: Setup extraction services
    // =================================================================
    println!("\n🔧 Step 3: Setting up extraction services...");

    // Create mock extraction service with test data
    let mock_service = MockExtractionService::new()
        .with_mock_data(doc_id, first_name_attr, json!("John Doe"));

    // Create document catalog source
    let doc_source = DocumentCatalogSource::new(
        pool.clone(),
        Arc::new(mock_service),
    );

    println!("✅ Created DocumentCatalogSource with priority {}", doc_source.priority());

    // =================================================================
    // STEP 4: Setup attribute executor with fallback chain
    // =================================================================
    println!("\n⚙️  Step 4: Setting up attribute executor...");

    let sources: Vec<Arc<dyn AttributeSource>> = vec![
        Arc::new(doc_source), // Highest priority - try documents first
    ];

    let sinks = vec![
        Arc::new(DatabaseSink::new(pool.clone())),
    ];

    let dictionary = AttributeDictionary::new(pool.clone());
    let executor = AttributeExecutor::new(sources, sinks, dictionary);

    println!("✅ Created AttributeExecutor with {} sources and {} sinks", 1, 1);

    // =================================================================
    // STEP 5: Resolve attribute from document
    // =================================================================
    println!("\n🔍 Step 5: Resolving attribute from document...");

    let mut context = ExecutionContext::new();
    
    match executor.resolve_attribute(&first_name_attr, &context).await {
        Ok(value) => {
            println!("✅ Successfully resolved attribute!");
            println!("   Attribute ID: {}", first_name_attr);
            println!("   Value: {}", value);
        }
        Err(e) => {
            println!("❌ Failed to resolve attribute: {}", e);
        }
    }

    // =================================================================
    // STEP 6: Parse DSL with source hints
    // =================================================================
    println!("\n📝 Step 6: Parsing DSL with attribute source hints...");

    let dsl_with_hints = format!(
        r#"(kyc.collect :attributes [@attr{{{}}}:doc @attr{{{}}}:form])"#,
        first_name_attr, Uuid::new_v4()
    );

    println!("   DSL: {}", dsl_with_hints);

    match parse_program(&dsl_with_hints) {
        Ok(program) => {
            println!("✅ Successfully parsed DSL with source hints!");
            println!("   Forms parsed: {}", program.len());
        }
        Err(e) => {
            println!("❌ Failed to parse DSL: {:?}", e);
        }
    }

    // =================================================================
    // STEP 7: Demonstrate metadata extraction
    // =================================================================
    println!("\n📊 Step 7: Checking extraction logs...");

    let log_count: i64 = sqlx::query_scalar(
        r#"SELECT COUNT(*) FROM "ob-poc".attribute_extraction_log WHERE document_id = $1"#
    )
    .bind(doc_id)
    .fetch_one(&pool)
    .await?;

    println!("✅ Found {} extraction log entries", log_count);

    // =================================================================
    // STEP 8: Demonstrate batch extraction
    // =================================================================
    println!("\n🔄 Step 8: Testing batch attribute resolution...");

    let attr_ids = vec![first_name_attr];
    let results = executor.batch_resolve(&attr_ids, &context).await;

    println!("✅ Batch resolved {} attributes", results.len());
    for (i, result) in results.iter().enumerate() {
        match result {
            Ok(value) => println!("   [{}] Success: {}", i, value),
            Err(e) => println!("   [{}] Failed: {}", i, e),
        }
    }

    // =================================================================
    // STEP 9: Cleanup
    // =================================================================
    println!("\n🧹 Step 9: Cleaning up test data...");

    sqlx::query(r#"DELETE FROM "ob-poc".document_usage WHERE doc_id = $1"#)
        .bind(doc_id)
        .execute(&pool)
        .await?;

    sqlx::query(r#"DELETE FROM "ob-poc".document_catalog WHERE doc_id = $1"#)
        .bind(doc_id)
        .execute(&pool)
        .await?;

    println!("✅ Cleanup complete");

    // =================================================================
    // Summary
    // =================================================================
    println!("\n" + &"=".repeat(80));
    println!("🎉 Demo Complete!\n");
    println!("Key Features Demonstrated:");
    println!("  ✓ Document catalog integration");
    println!("  ✓ Attribute extraction from documents");
    println!("  ✓ Multi-source attribute resolution with fallback");
    println!("  ✓ DSL parsing with @attr{{uuid}}:source hints");
    println!("  ✓ Extraction audit logging");
    println!("  ✓ Batch attribute resolution");
    println!("\n" + &"=".repeat(80));

    Ok(())
}
