//! Integration tests for document extraction workflow
//!
//! These tests require a running PostgreSQL database with the ob-poc schema.
//! Run with: cargo test --test document_extraction_integration --features database -- --ignored

use ob_poc::database::document_type_repository::DocumentTypeRepository;
use ob_poc::models::document_type_models::{ExtractedAttribute, ExtractionMethod};
use ob_poc::services::RealDocumentExtractionService;
use sqlx::PgPool;
use std::sync::Arc;
use uuid::Uuid;

/// Helper to get database connection
async fn get_test_pool() -> PgPool {
    let database_url = std::env::var("DATABASE_URL")
        .unwrap_or_else(|_| "postgresql:///data_designer?user=adamtc007".to_string());
    
    PgPool::connect(&database_url)
        .await
        .expect("Failed to connect to database")
}

#[tokio::test]
#[ignore] // Requires database
async fn test_get_document_type_by_code() {
    let pool = get_test_pool().await;
    let repo = DocumentTypeRepository::new(Arc::new(pool));

    // Test getting PASSPORT document type
    let doc_type = repo
        .get_by_code("PASSPORT")
        .await
        .expect("Failed to query document type");

    assert!(doc_type.is_some(), "PASSPORT document type should exist");
    
    let passport = doc_type.unwrap();
    assert_eq!(passport.type_code, "PASSPORT");
    assert_eq!(passport.category, "IDENTITY");
    assert_eq!(passport.domain, "KYC");
}

#[tokio::test]
#[ignore] // Requires database
async fn test_get_document_type_mappings() {
    let pool = get_test_pool().await;
    let repo = DocumentTypeRepository::new(Arc::new(pool));

    // Get PASSPORT document type
    let passport = repo
        .get_by_code("PASSPORT")
        .await
        .expect("Failed to query")
        .expect("PASSPORT should exist");

    // Get its mappings
    let mappings = repo
        .get_mappings(passport.type_id)
        .await
        .expect("Failed to get mappings");

    // Should have 5 mappings for passport
    assert_eq!(mappings.len(), 5, "PASSPORT should have 5 attribute mappings");

    // All should use MRZ extraction
    for mapping in &mappings {
        assert_eq!(mapping.extraction_method, ExtractionMethod::MRZ);
        assert!(mapping.confidence_threshold >= 0.90);
    }

    // Check that all are required
    assert!(
        mappings.iter().all(|m| m.is_required),
        "All passport attributes should be required"
    );
}

#[tokio::test]
#[ignore] // Requires database
async fn test_get_all_document_types() {
    let pool = get_test_pool().await;
    let repo = DocumentTypeRepository::new(Arc::new(pool));

    let types = repo.get_all().await.expect("Failed to get all types");

    // Should have at least 5 document types from seed data
    assert!(types.len() >= 5, "Should have at least 5 document types");

    // Check that expected types exist
    let type_codes: Vec<_> = types.iter().map(|t| t.type_code.as_str()).collect();
    assert!(type_codes.contains(&"PASSPORT"));
    assert!(type_codes.contains(&"BANK_STATEMENT"));
    assert!(type_codes.contains(&"UTILITY_BILL"));
    assert!(type_codes.contains(&"NATIONAL_ID"));
    assert!(type_codes.contains(&"ARTICLES_OF_INCORPORATION"));
}

#[tokio::test]
#[ignore] // Requires database
async fn test_store_and_retrieve_extracted_values() {
    let pool = get_test_pool().await;
    let repo = DocumentTypeRepository::new(Arc::new(pool));

    // Create test IDs
    let document_id = Uuid::new_v4();
    let entity_id = Uuid::new_v4();
    let attr_uuid = Uuid::parse_str("3020d46f-472c-5437-9647-1b0682c35935") // first_name
        .expect("Valid UUID");

    // Create extracted attribute
    let extracted = ExtractedAttribute {
        attribute_uuid: attr_uuid,
        value: serde_json::json!("John"),
        confidence: 0.98,
        extraction_method: ExtractionMethod::MRZ,
        metadata: Some(std::collections::HashMap::from([
            ("test".to_string(), serde_json::json!(true)),
        ])),
    };

    // Store the extracted value
    repo.store_extracted_value(document_id, entity_id, &extracted)
        .await
        .expect("Failed to store extracted value");

    // Retrieve all extracted values for the document
    let retrieved = repo
        .get_extracted_values(document_id)
        .await
        .expect("Failed to retrieve extracted values");

    assert_eq!(retrieved.len(), 1, "Should have 1 extracted value");
    assert_eq!(retrieved[0].attribute_uuid, attr_uuid);
    assert_eq!(retrieved[0].value, serde_json::json!("John"));
    assert_eq!(retrieved[0].confidence, 0.98);
}

#[tokio::test]
#[ignore] // Requires database
async fn test_can_extract_attribute() {
    let pool = get_test_pool().await;
    let repo = DocumentTypeRepository::new(Arc::new(pool));

    let passport = repo
        .get_by_code("PASSPORT")
        .await
        .expect("Query failed")
        .expect("PASSPORT exists");

    // first_name should be extractable from passport
    let first_name_uuid = Uuid::parse_str("3020d46f-472c-5437-9647-1b0682c35935").unwrap();
    let can_extract = repo
        .can_extract_attribute(passport.type_id, first_name_uuid)
        .await
        .expect("Query failed");

    assert!(can_extract, "first_name should be extractable from PASSPORT");

    // Some random UUID should not be extractable
    let random_uuid = Uuid::new_v4();
    let cannot_extract = repo
        .can_extract_attribute(passport.type_id, random_uuid)
        .await
        .expect("Query failed");

    assert!(!cannot_extract, "Random UUID should not be extractable");
}

#[tokio::test]
#[ignore] // Requires database
async fn test_get_extraction_method() {
    let pool = get_test_pool().await;
    let repo = DocumentTypeRepository::new(Arc::new(pool));

    let passport = repo
        .get_by_code("PASSPORT")
        .await
        .expect("Query failed")
        .expect("PASSPORT exists");

    let first_name_uuid = Uuid::parse_str("3020d46f-472c-5437-9647-1b0682c35935").unwrap();
    
    let method = repo
        .get_extraction_method(passport.type_id, first_name_uuid)
        .await
        .expect("Query failed");

    assert_eq!(method, Some(ExtractionMethod::MRZ));
}

#[tokio::test]
#[ignore] // Requires database  
async fn test_get_required_attributes() {
    let pool = get_test_pool().await;
    let repo = DocumentTypeRepository::new(Arc::new(pool));

    let passport = repo
        .get_by_code("PASSPORT")
        .await
        .expect("Query failed")
        .expect("PASSPORT exists");

    let required = repo
        .get_required_attributes(passport.type_id)
        .await
        .expect("Query failed");

    // PASSPORT should have 5 required attributes
    assert_eq!(required.len(), 5, "PASSPORT should have 5 required attributes");
}

#[tokio::test]
#[ignore] // Requires database with test data
async fn test_extraction_service_mock() {
    let pool = get_test_pool().await;
    let repo = DocumentTypeRepository::new(Arc::new(pool));
    let service = RealDocumentExtractionService::new(repo.clone());

    // This test would require setting up a document in the catalog
    // For now, just verify service can be created
    assert!(true, "Service created successfully");
    
    // To fully test extraction, you would need to:
    // 1. Insert a test document into document_catalog with a document_type_id
    // 2. Call service.extract_from_document(doc_id, entity_id)
    // 3. Verify extracted values are stored
}
