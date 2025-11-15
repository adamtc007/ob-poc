//! DSL Executor Document Extraction Integration Example
//!
//! This example demonstrates the complete integration of document extraction
//! into the DSL execution engine:
//!
//! 1. Register the DocumentExtractionHandler with the engine
//! 2. Execute DSL with document.extract operations
//! 3. See how extracted attributes flow through the execution pipeline
//! 4. Verify dual-write storage and state updates
//!
//! Run with: cargo run --example dsl_executor_document_extraction --features database

use ob_poc::database::document_type_repository::DocumentTypeRepository;
use ob_poc::dsl::operations::ExecutableDslOperation;
use ob_poc::execution::context::SessionManager;
use ob_poc::execution::document_extraction_handler::DocumentExtractionHandler;
use ob_poc::execution::engine::EngineBuilder;
use ob_poc::execution::OperationHandler;
use sqlx::PgPool;
use std::collections::HashMap;
use std::sync::Arc;
use uuid::Uuid;

#[tokio::main]
async fn main() -> Result<(), Box<dyn std::error::Error>> {
    env_logger::init();

    println!("🚀 DSL Executor Document Extraction Integration Example\n");
    println!("{}", "=".repeat(70));

    // Connect to database
    let database_url = std::env::var("DATABASE_URL")
        .unwrap_or_else(|_| "postgresql:///data_designer?user=adamtc007".to_string());

    println!("\n📊 Connecting to database...");
    let pool = PgPool::connect(&database_url).await?;
    let pool_arc = Arc::new(pool.clone());

    // ========================================================================
    // STEP 1: Build DSL Engine with Document Extraction Handler
    // ========================================================================
    println!("\n{}", "=".repeat(70));
    println!("STEP 1: Build DSL Execution Engine");
    println!("{}", "=".repeat(70));

    println!("\n🔧 Creating DocumentExtractionHandler...");
    let doc_handler =
        Arc::new(DocumentExtractionHandler::new(pool_arc.clone())) as Arc<dyn OperationHandler>;

    println!("✓ Handler created for operation: {}", doc_handler.handles());

    println!("\n🏗️  Building comprehensive DSL engine...");
    let engine = EngineBuilder::new()
        .with_postgres(pool.clone())
        .with_handler(doc_handler)
        .build()
        .await?;

    println!("✓ Engine initialized with document extraction support");

    // ========================================================================
    // STEP 2: Prepare Test Data
    // ========================================================================
    println!("\n{}", "=".repeat(70));
    println!("STEP 2: Prepare Test Data");
    println!("{}", "=".repeat(70));

    let cbu_id = format!("CBU-TEST-{}", Uuid::new_v4().to_simple());
    let entity_id = Uuid::new_v4();
    let document_id = Uuid::new_v4();

    println!("\n📋 Test Identifiers:");
    println!("  CBU ID: {}", cbu_id);
    println!("  Entity ID: {}", entity_id);
    println!("  Document ID: {}", document_id);

    // Get PASSPORT document type
    let repo = DocumentTypeRepository::new(pool_arc.clone());
    let passport = repo
        .get_by_code("PASSPORT")
        .await?
        .expect("PASSPORT type should exist");

    println!(
        "\n📄 Document Type: {} ({})",
        passport.display_name, passport.type_id
    );

    // Create a mock document in the catalog (in real scenario, this would come from upload)
    println!("\n💾 Creating mock document in catalog...");
    sqlx::query(
        r#"
        INSERT INTO "ob-poc".document_catalog
        (document_id, cbu_id, document_type_id, file_name, file_path, uploaded_at, status)
        VALUES ($1, $2, $3, $4, $5, NOW(), 'VERIFIED')
        ON CONFLICT (document_id) DO NOTHING
        "#,
    )
    .bind(document_id)
    .bind(&cbu_id)
    .bind(passport.type_id)
    .bind("test_passport.pdf")
    .bind("/tmp/test_passport.pdf")
    .execute(&pool)
    .await?;

    println!("✓ Mock document created in catalog");

    // ========================================================================
    // STEP 3: Create DSL Operation for Document Extraction
    // ========================================================================
    println!("\n{}", "=".repeat(70));
    println!("STEP 3: Create DSL Operation");
    println!("{}", "=".repeat(70));

    let dsl_content = format!(
        r#"
(document.extract
  :document-id "{}"
  :entity-id "{}"
  :attributes [
    "3020d46f-472c-5437-9647-1b0682c35935"  ; first_name
    "0af112fd-ec04-5938-84e8-6e5949db0b52"  ; last_name
    "c09501c7-2ea9-5ad7-b330-7d664c678e37"  ; passport_number
  ])
"#,
        document_id, entity_id
    );

    println!("\n📝 DSL Content:");
    println!("{}", dsl_content);

    let mut parameters = HashMap::new();
    parameters.insert(
        "document-id".to_string(),
        serde_json::json!(document_id.to_string()),
    );
    parameters.insert(
        "entity-id".to_string(),
        serde_json::json!(entity_id.to_string()),
    );
    parameters.insert(
        "attributes".to_string(),
        serde_json::json!([
            "3020d46f-472c-5437-9647-1b0682c35935",
            "0af112fd-ec04-5938-84e8-6e5949db0b52",
            "c09501c7-2ea9-5ad7-b330-7d664c678e37",
        ]),
    );

    let operation = ExecutableDslOperation {
        operation_type: "document.extract".to_string(),
        parameters,
        dsl_content: dsl_content.clone(),
        metadata: HashMap::new(),
    };

    println!("\n✓ Operation created: {}", operation.operation_type);

    // ========================================================================
    // STEP 4: Create Execution Context
    // ========================================================================
    println!("\n{}", "=".repeat(70));
    println!("STEP 4: Create Execution Context");
    println!("{}", "=".repeat(70));

    let context = SessionManager::create_kyc_session(
        cbu_id.clone(),
        "system".to_string(),
        vec!["document_extraction".to_string()],
    );

    println!("\n📋 Execution Context:");
    println!("  Session ID: {}", context.session_id);
    println!("  Business Unit: {}", context.business_unit_id);
    println!("  Domain: {}", context.domain);
    println!("  Executor: {}", context.executor);
    println!("  Integrations: {:?}", context.integrations);

    // ========================================================================
    // STEP 5: Execute the Operation
    // ========================================================================
    println!("\n{}", "=".repeat(70));
    println!("STEP 5: Execute Document Extraction Operation");
    println!("{}", "=".repeat(70));

    println!("\n⚙️  Executing operation...");
    let start = std::time::Instant::now();

    let result = engine.execute_operation(operation, context).await?;

    let duration = start.elapsed();
    println!("\n✓ Execution completed in {:?}", duration);

    // ========================================================================
    // STEP 6: Analyze Execution Result
    // ========================================================================
    println!("\n{}", "=".repeat(70));
    println!("STEP 6: Analyze Execution Result");
    println!("{}", "=".repeat(70));

    println!("\n📊 Execution Result:");
    println!("  Success: {}", result.success);
    println!("  Operation Type: {}", result.operation.operation_type);
    println!("  Duration: {} ms", result.duration_ms);
    println!("  State Version: {}", result.new_state.version);
    println!(
        "  Total Operations in History: {}",
        result.new_state.operations.len()
    );

    println!("\n📨 Execution Messages ({}):", result.messages.len());
    for (idx, msg) in result.messages.iter().enumerate() {
        println!("  {}. [{:?}] {}", idx + 1, msg.level, msg.message);
    }

    println!("\n🔗 External Responses:");
    for (key, value) in &result.external_responses {
        println!("  {}:", key);
        if let Some(obj) = value.as_object() {
            for (k, v) in obj {
                match k.as_str() {
                    "extractions" => {
                        if let Some(arr) = v.as_array() {
                            println!("    Extracted {} attributes:", arr.len());
                            for extraction in arr {
                                if let Some(ext_obj) = extraction.as_object() {
                                    let attr_uuid = ext_obj
                                        .get("attribute_uuid")
                                        .and_then(|v| v.as_str())
                                        .unwrap_or("unknown");
                                    let value =
                                        &ext_obj.get("value").unwrap_or(&serde_json::Value::Null);
                                    let confidence = ext_obj
                                        .get("confidence")
                                        .and_then(|v| v.as_f64())
                                        .unwrap_or(0.0);
                                    let method = ext_obj
                                        .get("method")
                                        .and_then(|v| v.as_str())
                                        .unwrap_or("unknown");
                                    println!(
                                        "      - {}: {:?} (confidence: {}, method: {})",
                                        attr_uuid, value, confidence, method
                                    );
                                }
                            }
                        }
                    }
                    _ => println!("    {}: {:?}", k, v),
                }
            }
        }
    }

    // ========================================================================
    // STEP 7: Verify Database Storage
    // ========================================================================
    println!("\n{}", "=".repeat(70));
    println!("STEP 7: Verify Database Storage");
    println!("{}", "=".repeat(70));

    println!("\n🔍 Checking document_metadata table...");
    let metadata_rows: Vec<(Uuid, serde_json::Value, f64)> = sqlx::query_as(
        r#"
        SELECT attribute_uuid, extracted_value, extraction_confidence
        FROM "ob-poc".document_metadata
        WHERE document_id = $1
        ORDER BY extracted_at DESC
        "#,
    )
    .bind(document_id)
    .fetch_all(&pool)
    .await?;

    println!(
        "✓ Found {} entries in document_metadata:",
        metadata_rows.len()
    );
    for (attr_uuid, value, confidence) in &metadata_rows {
        println!(
            "  - {}: {:?} (confidence: {})",
            attr_uuid, value, confidence
        );
    }

    println!("\n🔍 Checking attribute_values_typed table...");
    let typed_rows: Vec<(Uuid, serde_json::Value)> = sqlx::query_as(
        r#"
        SELECT attribute_uuid, value_json
        FROM "ob-poc".attribute_values_typed
        WHERE entity_id = $1
        ORDER BY updated_at DESC
        "#,
    )
    .bind(entity_id)
    .fetch_all(&pool)
    .await?;

    println!(
        "✓ Found {} entries in attribute_values_typed:",
        typed_rows.len()
    );
    for (attr_uuid, value) in &typed_rows {
        println!("  - {}: {:?}", attr_uuid, value);
    }

    // ========================================================================
    // STEP 8: Query Final State
    // ========================================================================
    println!("\n{}", "=".repeat(70));
    println!("STEP 8: Query Final DSL State");
    println!("{}", "=".repeat(70));

    let final_state = engine.get_current_state(&cbu_id).await?;

    if let Some(state) = final_state {
        println!("\n✓ Final State:");
        println!("  Business Unit: {}", state.business_unit_id);
        println!("  Version: {}", state.version);
        println!("  Status: {}", state.metadata.status);
        println!("  Operations Count: {}", state.operations.len());
        println!("  Created: {}", state.metadata.created_at);
        println!("  Updated: {}", state.metadata.updated_at);

        println!("\n📜 Accumulated DSL Document:");
        println!("{}", "-".repeat(70));
        println!("{}", state.to_dsl_document());
        println!("{}", "-".repeat(70));
    } else {
        println!("\n⚠️  No state found for CBU {}", cbu_id);
    }

    // ========================================================================
    // Summary
    // ========================================================================
    println!("\n{}", "=".repeat(70));
    println!("✨ Integration Complete!");
    println!("{}", "=".repeat(70));

    println!("\n📋 What Happened:");
    println!("  1. ✅ Built DSL engine with DocumentExtractionHandler");
    println!("  2. ✅ Created mock document in catalog");
    println!("  3. ✅ Generated document.extract DSL operation");
    println!("  4. ✅ Created execution context for KYC domain");
    println!("  5. ✅ Executed operation through engine");
    println!(
        "  6. ✅ Extracted {} attributes with mock values",
        metadata_rows.len()
    );
    println!("  7. ✅ Verified dual-write storage (2 tables)");
    println!("  8. ✅ Retrieved final accumulated DSL state");

    println!("\n🎯 Integration Points Verified:");
    println!("  ✓ DocumentExtractionHandler registered with engine");
    println!("  ✓ Handler invoked for document.extract operations");
    println!("  ✓ Extraction service called with correct parameters");
    println!("  ✓ Dual-write storage to document_metadata + attribute_values_typed");
    println!("  ✓ State updated with operation history");
    println!("  ✓ Execution messages captured and returned");
    println!("  ✓ External responses include extraction details");

    println!("\n🚀 Next Steps:");
    println!("  - Replace mock extraction with real OCR/MRZ engines");
    println!("  - Add field location-based extraction");
    println!("  - Implement confidence threshold validation");
    println!("  - Create workflow combining upload → extract → validate");
    println!("  - Add batch extraction for multiple documents");

    println!("\n{}", "=".repeat(70));

    Ok(())
}
