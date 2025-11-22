//! E2E Test: CBU Document-Directed CRUD Flow
//!
//! Tests the full pipeline: DSL.CBU.MODEL -> DSL.CRUD.CBU.TEMPLATE -> DSL.CRUD.CBU -> Forth -> CrudExecutor -> DB

use ob_poc::cbu_crud_template::CbuCrudTemplateService;
use ob_poc::cbu_model_dsl::CbuModelParser;
use ob_poc::database::{CbuService, CrudExecutor};
use ob_poc::forth_engine::{execute_sheet_into_env, DslSheet};
use sqlx::PgPool;
use std::collections::HashMap;
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
        INSERT INTO "ob-poc".document_types (type_code, display_name, category, description)
        VALUES
            ('DSL.CBU.MODEL', 'CBU Model', 'DSL', 'CBU Model specification'),
            ('DSL.CRUD.CBU.TEMPLATE', 'CBU CRUD Template', 'DSL', 'Parametrized CBU CRUD recipe'),
            ('DSL.CRUD.CBU', 'CBU CRUD Sheet', 'DSL', 'Concrete CBU CRUD execution document')
        ON CONFLICT (type_code) DO NOTHING
        "#,
    )
    .execute(pool)
    .await
    .expect("Failed to seed document types");

    // Seed dictionary entries matching actual schema
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

async fn cleanup_test_data(pool: &PgPool, cbu_name: &str) {
    sqlx::query(r#"DELETE FROM "ob-poc".cbus WHERE name = $1"#)
        .bind(cbu_name)
        .execute(pool)
        .await
        .ok();

    sqlx::query(r#"DELETE FROM "ob-poc".document_catalog WHERE type_code = 'DSL.CRUD.CBU.TEMPLATE'"#)
        .execute(pool)
        .await
        .ok();

    sqlx::query(r#"DELETE FROM "ob-poc".dsl_instances WHERE domain_name = 'CBU-CRUD-TEMPLATE'"#)
        .execute(pool)
        .await
        .ok();
}

const TEST_CBU_MODEL: &str = r#"
(cbu-model
  :id "CBU.TEST.E2E"
  :version "1.0"

  (attributes
    (group :name "core"
      :required [@attr("CBU.LEGAL_NAME"), @attr("CBU.JURISDICTION")]
      :optional [@attr("CBU.NATURE_PURPOSE")]))

  (states
    :initial "Proposed"
    :final ["Active", "Closed"]
    (state "Proposed" :description "Initial state")
    (state "Active" :description "Active state")
    (state "Closed" :description "Closed state"))

  (transitions
    (-> "Proposed" "Active" :verb "cbu.create" :chunks ["core"] :preconditions [])
    (-> "Active" "Closed" :verb "cbu.close" :preconditions []))

  (roles
    (role "Owner" :min 1)))
"#;

#[tokio::test]
#[ignore]
async fn test_cbu_document_crud_e2e_flow() {
    let pool = get_test_pool().await;
    let test_cbu_name = format!("E2E Test Corp {}", Uuid::new_v4());

    seed_test_data(&pool).await;
    cleanup_test_data(&pool, &test_cbu_name).await;

    // Step 1: Parse CBU Model DSL
    let model = CbuModelParser::parse_str(TEST_CBU_MODEL).expect("Failed to parse CBU Model DSL");
    assert_eq!(model.id, "CBU.TEST.E2E");

    // Step 2: Generate templates from model
    let template_service = CbuCrudTemplateService::new(pool.clone());
    let templates = template_service.generate_templates(&model);
    assert!(!templates.is_empty(), "Should generate at least one template");

    let submit_template = templates
        .iter()
        .find(|t| t.transition_verb == "cbu.create")
        .expect("Should have cbu.create template");

    assert!(submit_template.content.contains("{{CBU.LEGAL_NAME}}"));

    // Step 3: Save templates to database
    let save_result = template_service
        .save_templates(&templates, &model)
        .await
        .expect("Failed to save templates");

    // Step 4: Instantiate CRUD sheet from template
    let submit_doc_id = save_result.document_ids[0];
    let mut initial_values = HashMap::new();
    initial_values.insert("CBU.LEGAL_NAME".to_string(), test_cbu_name.clone());
    initial_values.insert("CBU.JURISDICTION".to_string(), "US".to_string());
    initial_values.insert("CBU.NATURE_PURPOSE".to_string(), "Test E2E flow".to_string());

    let (crud_instance_id, _crud_doc_id) = template_service
        .instantiate_crud_from_template(submit_doc_id, initial_values)
        .await
        .expect("Failed to instantiate CRUD sheet");

    // Step 5: Load the CRUD content and create DslSheet
    let dsl_repo = ob_poc::database::DslRepository::new(pool.clone());
    let crud_content = dsl_repo
        .get_dsl_content(crud_instance_id)
        .await
        .expect("Failed to load CRUD content")
        .expect("CRUD content should exist");

    assert!(crud_content.contains(&test_cbu_name));

    let sheet = DslSheet {
        id: crud_instance_id.to_string(),
        domain: "cbu".to_string(),
        version: "1".to_string(),
        content: crud_content,
    };

    // Step 6: Execute DSL through Forth engine to get pending_crud
    let (result, mut env) =
        execute_sheet_into_env(&sheet, Some(pool.clone())).expect("Failed to execute DSL sheet");

    assert!(result.success, "DSL execution should succeed");

    let pending_crud = env.take_pending_crud();
    assert!(!pending_crud.is_empty(), "Should have pending CRUD statements");

    // Step 7: Execute CRUD statements against database
    let executor = CrudExecutor::new(pool.clone());
    let crud_results = executor
        .execute_all(&pending_crud)
        .await
        .expect("Failed to execute CRUD statements");

    let cbu_create_result = crud_results
        .iter()
        .find(|r| r.asset == "CBU" && r.operation == "CREATE");
    assert!(cbu_create_result.is_some(), "Should have CBU CREATE result");

    let cbu_id = cbu_create_result
        .unwrap()
        .generated_id
        .expect("CBU CREATE should return generated ID");

    // Step 8: Verify CBU exists in database
    let cbu_service = CbuService::new(pool.clone());
    let cbu = cbu_service
        .get_cbu_by_id(cbu_id)
        .await
        .expect("Failed to query CBU")
        .expect("CBU should exist in database");

    assert_eq!(cbu.name, test_cbu_name);

    cleanup_test_data(&pool, &test_cbu_name).await;
}

#[tokio::test]
#[ignore]
async fn test_cbu_crud_direct_dsl_execution() {
    let pool = get_test_pool().await;
    let test_cbu_name = format!("Direct DSL Test {}", Uuid::new_v4());

    seed_test_data(&pool).await;
    cleanup_test_data(&pool, &test_cbu_name).await;

    let dsl_content = format!(
        r#"(cbu.create :cbu-name "{}" :client-type "FUND" :jurisdiction "GB")"#,
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
    assert_eq!(pending_crud.len(), 1, "Should have exactly one CRUD statement");

    let executor = CrudExecutor::new(pool.clone());
    let crud_results = executor
        .execute_all(&pending_crud)
        .await
        .expect("CRUD execution failed");

    assert_eq!(crud_results.len(), 1);
    assert_eq!(crud_results[0].asset, "CBU");
    assert_eq!(crud_results[0].operation, "CREATE");

    let cbu_id = crud_results[0]
        .generated_id
        .expect("Should have generated CBU ID");

    let cbu_service = CbuService::new(pool.clone());
    let cbu = cbu_service
        .get_cbu_by_id(cbu_id)
        .await
        .expect("Query failed")
        .expect("CBU should exist");

    assert_eq!(cbu.name, test_cbu_name);

    cleanup_test_data(&pool, &test_cbu_name).await;
}

#[tokio::test]
#[ignore]
async fn test_cbu_persistence_check() {
    let pool = get_test_pool().await;
    let test_cbu_name = "PERSIST_CHECK_001";

    seed_test_data(&pool).await;

    let dsl_content = format!(
        r#"(cbu.create :cbu-name "{}" :client-type "HEDGE_FUND" :jurisdiction "US")"#,
        test_cbu_name
    );

    let sheet = DslSheet {
        id: uuid::Uuid::new_v4().to_string(),
        domain: "cbu".to_string(),
        version: "1".to_string(),
        content: dsl_content,
    };

    let (result, mut env) =
        execute_sheet_into_env(&sheet, Some(pool.clone())).expect("Failed to execute DSL");

    assert!(result.success);

    let pending_crud = env.take_pending_crud();
    println!("Pending CRUD: {:?}", pending_crud);

    let executor = CrudExecutor::new(pool.clone());
    let crud_results = executor
        .execute_all(&pending_crud)
        .await
        .expect("CRUD execution failed");

    let cbu_id = crud_results[0].generated_id.expect("Should have CBU ID");
    println!("Created CBU ID: {}", cbu_id);

    // Query back and print
    let cbu_service = CbuService::new(pool.clone());
    let cbu = cbu_service.get_cbu_by_id(cbu_id).await.expect("Query failed").expect("CBU should exist");
    
    println!("=== CBU Persisted ===");
    println!("Name: {}", cbu.name);
    println!("CBU ID: {}", cbu.cbu_id);
    
    // Don't cleanup - leave for inspection
}

#[tokio::test]
#[ignore]
async fn test_document_crud_operations() {
    let pool = get_test_pool().await;

    seed_test_data(&pool).await;

    // Seed a document type for testing
    sqlx::query(
        r#"
        INSERT INTO "ob-poc".document_types (type_id, type_code, display_name, category, description)
        VALUES (gen_random_uuid(), 'UK-PASSPORT', 'UK Passport', 'Identity', 'United Kingdom Passport')
        ON CONFLICT (type_code) DO NOTHING
        "#,
    )
    .execute(&pool)
    .await
    .expect("Failed to seed UK-PASSPORT type");

    // Test document.catalog - should emit DataCreate for DOCUMENT
    let dsl_content = r#"
        (document.catalog :doc-id "DOC-TEST-001" :doc-type "UK-PASSPORT")
    "#;

    let sheet = DslSheet {
        id: Uuid::new_v4().to_string(),
        domain: "document".to_string(),
        version: "1".to_string(),
        content: dsl_content.to_string(),
    };

    let (result, mut env) =
        execute_sheet_into_env(&sheet, Some(pool.clone())).expect("Failed to execute DSL");

    assert!(result.success, "DSL execution should succeed");

    let pending_crud = env.take_pending_crud();
    assert_eq!(pending_crud.len(), 1, "Should have one CRUD statement");

    // Verify it's a DOCUMENT create
    match &pending_crud[0] {
        ob_poc::parser::ast::CrudStatement::DataCreate(create) => {
            assert_eq!(create.asset, "DOCUMENT");
            assert!(create.values.contains_key("doc-id"));
            assert!(create.values.contains_key("doc-type"));
            println!("Document CRUD statement: {:?}", create);
        }
        _ => panic!("Expected DataCreate for DOCUMENT"),
    }

    println!("document.catalog emits correct CRUD statement");
}

#[tokio::test]
#[ignore]
async fn test_document_verify_update() {
    let pool = get_test_pool().await;

    // Test document.verify - should emit DataUpdate for DOCUMENT
    let dsl_content = r#"
        (document.verify :doc-id "DOC-VERIFY-001")
    "#;

    let sheet = DslSheet {
        id: Uuid::new_v4().to_string(),
        domain: "document".to_string(),
        version: "1".to_string(),
        content: dsl_content.to_string(),
    };

    let (result, mut env) =
        execute_sheet_into_env(&sheet, Some(pool.clone())).expect("Failed to execute DSL");

    assert!(result.success, "DSL execution should succeed");

    let pending_crud = env.take_pending_crud();
    assert_eq!(pending_crud.len(), 1, "Should have one CRUD statement");

    // Verify it's a DOCUMENT update with verification status
    match &pending_crud[0] {
        ob_poc::parser::ast::CrudStatement::DataUpdate(update) => {
            assert_eq!(update.asset, "DOCUMENT");
            assert!(update.values.contains_key("verification-status"));
            println!("Document verify CRUD statement: {:?}", update);
        }
        _ => panic!("Expected DataUpdate for DOCUMENT"),
    }

    println!("document.verify emits correct CRUD statement");
}

#[tokio::test]
#[ignore]
async fn test_full_document_flow() {
    let pool = get_test_pool().await;

    seed_test_data(&pool).await;

    // Seed document type
    sqlx::query(
        r#"
        INSERT INTO "ob-poc".document_types (type_id, type_code, display_name, category, description)
        VALUES (gen_random_uuid(), 'UK-PASSPORT', 'UK Passport', 'Identity', 'United Kingdom Passport')
        ON CONFLICT (type_code) DO NOTHING
        "#,
    )
    .execute(&pool)
    .await
    .expect("Failed to seed UK-PASSPORT type");

    // Test a full onboarding flow with CBU and documents
    let dsl_content = r#"
        (cbu.create :cbu-name "Full Flow Corp" :client-type "CORP" :jurisdiction "US")
        (document.catalog :doc-id "PASS-001" :doc-type "UK-PASSPORT")
        (document.extract-attributes :document-id "PASS-001" :document-type "UK-PASSPORT")
    "#;

    let sheet = DslSheet {
        id: Uuid::new_v4().to_string(),
        domain: "onboarding".to_string(),
        version: "1".to_string(),
        content: dsl_content.to_string(),
    };

    let (result, mut env) =
        execute_sheet_into_env(&sheet, Some(pool.clone())).expect("Failed to execute DSL");

    assert!(result.success, "DSL execution should succeed");

    let pending_crud = env.take_pending_crud();
    
    // Should have 3 CRUD statements: CBU create, document catalog, document extraction
    assert_eq!(pending_crud.len(), 3, "Should have 3 CRUD statements");

    println!("=== Full Document Flow CRUD Statements ===");
    for (i, stmt) in pending_crud.iter().enumerate() {
        match stmt {
            ob_poc::parser::ast::CrudStatement::DataCreate(create) => {
                println!("{}. CREATE {}: {:?}", i + 1, create.asset, create.values.keys().collect::<Vec<_>>());
            }
            ob_poc::parser::ast::CrudStatement::DataUpdate(update) => {
                println!("{}. UPDATE {}: {:?}", i + 1, update.asset, update.values.keys().collect::<Vec<_>>());
            }
            _ => println!("{}. Other statement", i + 1),
        }
    }

    // Verify statement types
    assert!(matches!(&pending_crud[0], ob_poc::parser::ast::CrudStatement::DataCreate(c) if c.asset == "CBU"));
    assert!(matches!(&pending_crud[1], ob_poc::parser::ast::CrudStatement::DataCreate(c) if c.asset == "DOCUMENT"));
    assert!(matches!(&pending_crud[2], ob_poc::parser::ast::CrudStatement::DataCreate(c) if c.asset == "DOCUMENT_EXTRACTION"));

    println!("Full document flow emits correct CRUD statements");

    // Cleanup
    sqlx::query(r#"DELETE FROM "ob-poc".cbus WHERE name = 'Full Flow Corp'"#)
        .execute(&pool)
        .await
        .ok();
}
