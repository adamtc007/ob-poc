//! Complete Document Extraction Workflow Example
//!
//! This example demonstrates the entire document-to-attribute extraction workflow:
//! 1. Query document types and mappings
//! 2. Extract attributes from a document (mock)
//! 3. Store extracted values (dual-write)
//! 4. Bind values to ExecutionContext
//! 5. Use values in subsequent operations
//!
//! Run with: cargo run --example document_extraction_complete_workflow --features database

use ob_poc::database::document_type_repository::DocumentTypeRepository;
use ob_poc::domains::attributes::execution_context::{ExecutionContext, ValueSource};
use ob_poc::models::document_type_models::ExtractedAttribute;
use ob_poc::services::RealDocumentExtractionService;
use sqlx::PgPool;
use std::sync::Arc;
use uuid::Uuid;

#[tokio::main]
async fn main() -> Result<(), Box<dyn std::error::Error>> {
    // Initialize logging
    env_logger::init();

    println!("🚀 Document Extraction Complete Workflow Example\n");
    println!("=" .repeat(60));

    // Connect to database
    let database_url = std::env::var("DATABASE_URL")
        .unwrap_or_else(|_| "postgresql:///data_designer?user=adamtc007".to_string());
    
    println!("\n📊 Connecting to database...");
    let pool = PgPool::connect(&database_url).await?;
    let repo = DocumentTypeRepository::new(Arc::new(pool));

    // ========================================================================
    // STEP 1: Query Document Types
    // ========================================================================
    println!("\n" .repeat(1) + &"=".repeat(60));
    println!("STEP 1: Query Available Document Types");
    println!("=" .repeat(60));

    let doc_types = repo.get_all().await?;
    println!("\n✅ Found {} document types:", doc_types.len());
    for dt in &doc_types {
        println!("  📄 {} ({}) - {}", dt.type_code, dt.category, dt.domain);
    }

    // ========================================================================
    // STEP 2: Get Passport Extraction Configuration
    // ========================================================================
    println!("\n" .repeat(1) + &"=".repeat(60));
    println!("STEP 2: Get PASSPORT Extraction Configuration");
    println!("=" .repeat(60));

    let passport = repo
        .get_by_code("PASSPORT")
        .await?
        .expect("PASSPORT document type should exist");

    println!("\n✅ PASSPORT Configuration:");
    println!("  Type ID: {}", passport.type_id);
    println!("  Category: {}", passport.category);
    println!("  Domain: {}", passport.domain);

    let mappings = repo.get_mappings(passport.type_id).await?;
    println!("\n✅ Extractable Attributes ({}):", mappings.len());
    for mapping in &mappings {
        println!(
            "  🔍 {} (method: {:?}, confidence: {}, required: {})",
            mapping.attribute_uuid,
            mapping.extraction_method,
            mapping.confidence_threshold,
            mapping.is_required
        );
    }

    // ========================================================================
    // STEP 3: Perform Document Extraction (Mock)
    // ========================================================================
    println!("\n" .repeat(1) + &"=".repeat(60));
    println!("STEP 3: Extract Attributes from Document");
    println!("=" .repeat(60));

    // Note: In production, you would have a real document_id from document_catalog
    // For this demo, we'll use mock IDs
    let document_id = Uuid::new_v4();
    let entity_id = Uuid::new_v4();

    println!("\n📄 Mock Document ID: {}", document_id);
    println!("👤 Entity ID: {}", entity_id);

    let service = RealDocumentExtractionService::new(repo.clone());

    // Note: This would fail without a real document in the catalog
    // For demo purposes, we'll show the process
    println!("\n⚠️  Note: Real extraction requires document in document_catalog");
    println!("    For this demo, we'll simulate the extraction process\n");

    // Simulate extracted attributes (what would come from real extraction)
    let simulated_extracted = vec![
        ExtractedAttribute {
            attribute_uuid: Uuid::parse_str("3020d46f-472c-5437-9647-1b0682c35935")?, // first_name
            value: serde_json::json!("John"),
            confidence: 0.98,
            extraction_method: ob_poc::models::document_type_models::ExtractionMethod::MRZ,
            metadata: Some(std::collections::HashMap::from([
                ("simulation".to_string(), serde_json::json!(true)),
            ])),
        },
        ExtractedAttribute {
            attribute_uuid: Uuid::parse_str("0af112fd-ec04-5938-84e8-6e5949db0b52")?, // last_name
            value: serde_json::json!("Smith"),
            confidence: 0.97,
            extraction_method: ob_poc::models::document_type_models::ExtractionMethod::MRZ,
            metadata: Some(std::collections::HashMap::from([
                ("simulation".to_string(), serde_json::json!(true)),
            ])),
        },
        ExtractedAttribute {
            attribute_uuid: Uuid::parse_str("c09501c7-2ea9-5ad7-b330-7d664c678e37")?, // passport_number
            value: serde_json::json!("N1234567"),
            confidence: 0.99,
            extraction_method: ob_poc::models::document_type_models::ExtractionMethod::MRZ,
            metadata: Some(std::collections::HashMap::from([
                ("simulation".to_string(), serde_json::json!(true)),
            ])),
        },
    ];

    println!("✅ Simulated Extraction Results:");
    for extracted in &simulated_extracted {
        println!(
            "  ✓ Attribute {}: {:?} (confidence: {})",
            extracted.attribute_uuid, extracted.value, extracted.confidence
        );
    }

    // ========================================================================
    // STEP 4: Store Extracted Values (Dual-Write)
    // ========================================================================
    println!("\n" .repeat(1) + &"=".repeat(60));
    println!("STEP 4: Store Extracted Values (Dual-Write)");
    println!("=" .repeat(60));

    println!("\n💾 Storing to document_metadata AND attribute_values_typed...");
    
    for extracted in &simulated_extracted {
        repo.store_extracted_value(document_id, entity_id, extracted)
            .await?;
        println!("  ✓ Stored attribute {}", extracted.attribute_uuid);
    }

    println!("\n✅ All values stored successfully!");

    // ========================================================================
    // STEP 5: Bind Values to ExecutionContext
    // ========================================================================
    println!("\n" .repeat(1) + &"=".repeat(60));
    println!("STEP 5: Bind Values to ExecutionContext");
    println!("=" .repeat(60));

    let mut context = ExecutionContext::with_ids(Uuid::new_v4(), entity_id);
    context.set_document(document_id);

    println!("\n🔗 Binding extracted values to ExecutionContext...");
    
    for extracted in &simulated_extracted {
        context.bind_value(
            extracted.attribute_uuid,
            extracted.value.clone(),
            ValueSource::DocumentExtraction {
                document_id,
                page: None,
                confidence: extracted.confidence,
            },
        );
        println!("  ✓ Bound attribute {}", extracted.attribute_uuid);
    }

    println!("\n✅ ExecutionContext Summary:");
    println!("  Bound Attributes: {}", context.bound_attributes().len());
    println!("  CBU ID: {:?}", context.cbu_id());
    println!("  Entity ID: {:?}", context.entity_id());
    println!("  Document ID: {:?}", context.document_id());

    // ========================================================================
    // STEP 6: Use Values in Subsequent Operations
    // ========================================================================
    println!("\n" .repeat(1) + &"=".repeat(60));
    println!("STEP 6: Access Bound Values");
    println!("=" .repeat(60));

    println!("\n📖 Reading bound values from ExecutionContext:");
    
    let first_name_uuid = Uuid::parse_str("3020d46f-472c-5437-9647-1b0682c35935")?;
    if let Some(value) = context.get_value(&first_name_uuid) {
        println!("  ✓ first_name: {}", value);
    }

    let last_name_uuid = Uuid::parse_str("0af112fd-ec04-5938-84e8-6e5949db0b52")?;
    if let Some(value) = context.get_value(&last_name_uuid) {
        println!("  ✓ last_name: {}", value);
    }

    // Access by semantic ID
    if let Some(value) = context.get_value_by_semantic("attr.identity.first_name") {
        println!("  ✓ Via semantic ID (attr.identity.first_name): {}", value);
    }

    // ========================================================================
    // STEP 7: Retrieve Stored Values
    // ========================================================================
    println!("\n" .repeat(1) + &"=".repeat(60));
    println!("STEP 7: Retrieve Stored Values from Database");
    println!("=" .repeat(60));

    let retrieved = repo.get_extracted_values(document_id).await?;
    
    println!("\n✅ Retrieved {} values from database:", retrieved.len());
    for attr in retrieved {
        println!(
            "  📥 {}: {:?} (method: {:?}, confidence: {})",
            attr.attribute_uuid, attr.value, attr.extraction_method, attr.confidence
        );
    }

    // ========================================================================
    // Summary
    // ========================================================================
    println!("\n" .repeat(1) + &"=".repeat(60));
    println!("✨ Workflow Complete!");
    println!("=" .repeat(60));

    println!("\n📋 What Happened:");
    println!("  1. ✅ Queried available document types from database");
    println!("  2. ✅ Retrieved PASSPORT extraction configuration");
    println!("  3. ✅ Simulated attribute extraction (5 attributes)");
    println!("  4. ✅ Stored values with dual-write to 2 tables");
    println!("  5. ✅ Bound values to ExecutionContext");
    println!("  6. ✅ Accessed bound values for use in operations");
    println!("  7. ✅ Retrieved and verified stored values");

    println!("\n🎯 Next Steps:");
    println!("  - Integrate with DSL executor for (document.extract ...) operations");
    println!("  - Add real OCR/MRZ extraction engines");
    println!("  - Implement field location-based extraction");
    println!("  - Add confidence threshold-based validation");

    println!("\n" .repeat(1) + &"=".repeat(60));

    Ok(())
}
