//! E2E Test: CBU Document-Directed CRUD Flow
//!
//! Tests the full pipeline: DSL -> Forth -> CrudExecutor -> DB
//! These tests MUST execute against a real database to catch schema drift.

use ob_poc::database::{CbuService, CrudExecutor, DocumentService};
use ob_poc::forth_engine::{execute_sheet_into_env, DslSheet};
use sqlx::PgPool;
use uuid::Uuid;

async fn get_test_pool() -> PgPool {
    let database_url = std::env::var("DATABASE_URL").expect("DATABASE_URL must be set");
    PgPool::connect(&database_url)
        .await
        .expect("Failed to connect to database")
}

async fn seed_test_data(pool: &PgPool) {
    // Seed document types
    sqlx::query(
        r#"
        INSERT INTO "ob-poc".document_types (type_id, type_code, display_name, category, description)
        VALUES 
            (gen_random_uuid(), 'UK-PASSPORT', 'UK Passport', 'Identity', 'United Kingdom Passport'),
            (gen_random_uuid(), 'DSL.CBU.MODEL', 'CBU Model', 'DSL', 'CBU Model specification'),
            (gen_random_uuid(), 'DSL.CRUD.CBU.TEMPLATE', 'CBU CRUD Template', 'DSL', 'Parametrized CBU CRUD recipe'),
            (gen_random_uuid(), 'DSL.CRUD.CBU', 'CBU CRUD Sheet', 'DSL', 'Concrete CBU CRUD execution document')
        ON CONFLICT (type_code) DO NOTHING
        "#,
    )
    .execute(pool)
    .await
    .expect("Failed to seed document types");

    // Seed dictionary entries
    let attr_entries = vec![
        ("cbu-name", "Legal name of the CBU", "cbu", "CBU"),
        ("jurisdiction", "Legal jurisdiction code", "cbu", "CBU"),
        ("nature-purpose", "Nature and purpose of business", "cbu", "CBU"),
        ("client-type", "Entity type classification", "cbu", "CBU"),
    ];

    for (name, desc, group, domain) in attr_entries {
        sqlx::query(
            r#"
            INSERT INTO "ob-poc".dictionary (name, long_description, group_id, domain)
            VALUES ($1, $2, $3, $4)
            ON CONFLICT (name) DO UPDATE SET long_description = $2
            "#,
        )
        .bind(name)
        .bind(desc)
        .bind(group)
        .bind(domain)
        .execute(pool)
        .await
        .expect("Failed to seed dictionary entry");
    }
}

async fn cleanup_cbu(pool: &PgPool, cbu_name: &str) {
    sqlx::query(r#"DELETE FROM "ob-poc".cbus WHERE name = $1"#)
        .bind(cbu_name)
        .execute(pool)
        .await
        .ok();
}

async fn cleanup_document(pool: &PgPool, document_code: &str) {
    sqlx::query(r#"DELETE FROM "ob-poc".document_catalog WHERE document_code = $1"#)
        .bind(document_code)
        .execute(pool)
        .await
        .ok();
}

#[tokio::test]
#[ignore]
async fn test_cbu_crud_full_db_execution() {
    let pool = get_test_pool().await;
    let test_cbu_name = format!("CBU_DB_TEST_{}", Uuid::new_v4().to_string()[..8].to_uppercase());

    seed_test_data(&pool).await;
    cleanup_cbu(&pool, &test_cbu_name).await;

    // Execute DSL
    let dsl_content = format!(
        r#"(cbu.create :cbu-name "{}" :client-type "HEDGE_FUND" :jurisdiction "GB")"#,
        test_cbu_name
    );

    let sheet = DslSheet {
        id: Uuid::new_v4().to_string(),
        domain: "cbu".to_string(),
        version: "1".to_string(),
        content: dsl_content,
    };

    let (result, mut env) =
        execute_sheet_into_env(&sheet, Some(pool.clone())).expect("Failed to execute DSL");

    assert!(result.success);

    // Execute CRUD against database
    let pending_crud = env.take_pending_crud();
    assert_eq!(pending_crud.len(), 1, "Should have one CRUD statement");

    let executor = CrudExecutor::new(pool.clone());
    let crud_results = executor
        .execute_all(&pending_crud)
        .await
        .expect("CRUD execution failed - CHECK SQL SCHEMA ALIGNMENT");

    assert_eq!(crud_results.len(), 1);
    let cbu_id = crud_results[0].generated_id.expect("Should have CBU ID");

    // Verify in database
    let cbu_service = CbuService::new(pool.clone());
    let cbu = cbu_service
        .get_cbu_by_id(cbu_id)
        .await
        .expect("DB query failed")
        .expect("CBU not found in database");

    assert_eq!(cbu.name, test_cbu_name);
    assert_eq!(cbu.client_type.as_deref(), Some("HEDGE_FUND"));
    assert_eq!(cbu.jurisdiction.as_deref(), Some("GB"));

    println!("✅ CBU persisted to DB: {} ({})", cbu.name, cbu.cbu_id);

    // Cleanup
    cleanup_cbu(&pool, &test_cbu_name).await;
}

#[tokio::test]
#[ignore]
async fn test_document_catalog_full_db_execution() {
    let pool = get_test_pool().await;
    let doc_code = format!("DOC_TEST_{}", Uuid::new_v4().to_string()[..8].to_uppercase());

    seed_test_data(&pool).await;
    cleanup_document(&pool, &doc_code).await;

    // Execute DSL
    let dsl_content = format!(
        r#"(document.catalog :doc-id "{}" :doc-type "UK-PASSPORT")"#,
        doc_code
    );

    let sheet = DslSheet {
        id: Uuid::new_v4().to_string(),
        domain: "document".to_string(),
        version: "1".to_string(),
        content: dsl_content,
    };

    let (result, mut env) =
        execute_sheet_into_env(&sheet, Some(pool.clone())).expect("Failed to execute DSL");

    assert!(result.success);

    // Execute CRUD against database
    let pending_crud = env.take_pending_crud();
    assert_eq!(pending_crud.len(), 1, "Should have one CRUD statement");

    let executor = CrudExecutor::new(pool.clone());
    let crud_results = executor
        .execute_all(&pending_crud)
        .await
        .expect("CRUD execution failed - CHECK SQL SCHEMA ALIGNMENT");

    assert_eq!(crud_results.len(), 1);
    assert_eq!(crud_results[0].asset, "DOCUMENT");
    
    let doc_id = crud_results[0].generated_id.expect("Should have document ID");

    // Verify in database
    let doc_service = DocumentService::new(pool.clone());
    let doc = doc_service
        .get_document_by_id(doc_id)
        .await
        .expect("DB query failed")
        .expect("Document not found in database");

    assert_eq!(doc.document_code(), doc_code);

    println!("✅ Document persisted to DB: {} ({})", doc.document_code(), doc.document_id());

    // Cleanup
    cleanup_document(&pool, &doc_code).await;
}

#[tokio::test]
#[ignore]
async fn test_full_onboarding_flow_db_execution() {
    let pool = get_test_pool().await;
    let test_cbu_name = format!("FULL_FLOW_{}", Uuid::new_v4().to_string()[..8].to_uppercase());
    let doc_code = format!("DOC_{}", Uuid::new_v4().to_string()[..8].to_uppercase());

    seed_test_data(&pool).await;
    cleanup_cbu(&pool, &test_cbu_name).await;
    cleanup_document(&pool, &doc_code).await;

    // Execute full flow DSL
    let dsl_content = format!(
        r#"
        (cbu.create :cbu-name "{}" :client-type "CORP" :jurisdiction "US")
        (document.catalog :doc-id "{}" :doc-type "UK-PASSPORT")
        "#,
        test_cbu_name, doc_code
    );

    let sheet = DslSheet {
        id: Uuid::new_v4().to_string(),
        domain: "onboarding".to_string(),
        version: "1".to_string(),
        content: dsl_content,
    };

    let (result, mut env) =
        execute_sheet_into_env(&sheet, Some(pool.clone())).expect("Failed to execute DSL");

    assert!(result.success);

    // Execute all CRUD against database
    let pending_crud = env.take_pending_crud();
    assert_eq!(pending_crud.len(), 2, "Should have 2 CRUD statements (CBU + Document)");

    let executor = CrudExecutor::new(pool.clone());
    let crud_results = executor
        .execute_all(&pending_crud)
        .await
        .expect("CRUD execution failed - CHECK SQL SCHEMA ALIGNMENT");

    assert_eq!(crud_results.len(), 2);

    // Verify CBU
    let cbu_result = crud_results.iter().find(|r| r.asset == "CBU").expect("No CBU result");
    let cbu_id = cbu_result.generated_id.expect("No CBU ID");
    
    let cbu_service = CbuService::new(pool.clone());
    let cbu = cbu_service.get_cbu_by_id(cbu_id).await.expect("Query failed").expect("CBU not found");
    assert_eq!(cbu.name, test_cbu_name);

    // Verify Document
    let doc_result = crud_results.iter().find(|r| r.asset == "DOCUMENT").expect("No Document result");
    let doc_id = doc_result.generated_id.expect("No Document ID");
    
    let doc_service = DocumentService::new(pool.clone());
    let doc = doc_service.get_document_by_id(doc_id).await.expect("Query failed").expect("Document not found");
    assert_eq!(doc.document_code(), doc_code);

    println!("✅ Full flow persisted to DB:");
    println!("   CBU: {} ({})", cbu.name, cbu.cbu_id);
    println!("   Document: {} ({})", doc.document_code(), doc.document_id());

    // Cleanup
    cleanup_document(&pool, &doc_code).await;
    cleanup_cbu(&pool, &test_cbu_name).await;
}

#[tokio::test]
#[ignore]
async fn test_cbu_fields_persist_correctly() {
    let pool = get_test_pool().await;
    let test_cbu_name = format!("FIELD_TEST_{}", Uuid::new_v4().to_string()[..8].to_uppercase());

    seed_test_data(&pool).await;
    cleanup_cbu(&pool, &test_cbu_name).await;

    // Test all CBU fields - single line
    let dsl_content = format!(
        r#"(cbu.create :cbu-name "{}" :client-type "PENSION_FUND" :jurisdiction "DE" :nature-purpose "Investment management" :description "Test fund for field validation")"#,
        test_cbu_name
    );

    let sheet = DslSheet {
        id: Uuid::new_v4().to_string(),
        domain: "cbu".to_string(),
        version: "1".to_string(),
        content: dsl_content,
    };

    let (result, mut env) =
        execute_sheet_into_env(&sheet, Some(pool.clone())).expect("Failed to execute DSL");

    assert!(result.success);

    let pending_crud = env.take_pending_crud();
    let executor = CrudExecutor::new(pool.clone());
    let crud_results = executor
        .execute_all(&pending_crud)
        .await
        .expect("CRUD execution failed");

    let cbu_id = crud_results[0].generated_id.expect("No CBU ID");

    // Verify all fields
    let row: (String, Option<String>, Option<String>, Option<String>, Option<String>) = sqlx::query_as(
        r#"SELECT name, client_type, jurisdiction, nature_purpose, description 
           FROM "ob-poc".cbus WHERE cbu_id = $1"#
    )
    .bind(cbu_id)
    .fetch_one(&pool)
    .await
    .expect("Query failed");

    assert_eq!(row.0, test_cbu_name);
    assert_eq!(row.1.as_deref(), Some("PENSION_FUND"));
    assert_eq!(row.2.as_deref(), Some("DE"));
    assert_eq!(row.3.as_deref(), Some("Investment management"));
    assert_eq!(row.4.as_deref(), Some("Test fund for field validation"));

    println!("✅ All CBU fields persisted correctly");

    cleanup_cbu(&pool, &test_cbu_name).await;
}
