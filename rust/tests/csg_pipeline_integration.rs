//! Integration tests for CSG linter in validation pipeline
//!
//! These tests verify that the CSG linter correctly enforces
//! document-entity applicability rules from the database.
//!
//! Run with: cargo test --features database --test csg_pipeline_integration -- --ignored

use ob_poc::dsl_v2::{
    parse_program,
    validation::{ClientType, DiagnosticCode, ValidationContext},
    CsgLinter,
};

/// Get test database pool
async fn get_test_pool() -> sqlx::PgPool {
    let url = std::env::var("DATABASE_URL")
        .unwrap_or_else(|_| "postgresql://localhost/data_designer".to_string());
    sqlx::PgPool::connect(&url)
        .await
        .expect("Failed to connect to database")
}

/// Test that PASSPORT document is rejected for LIMITED_COMPANY entity
#[tokio::test]
#[ignore] // Requires database
async fn test_passport_for_company_rejected() {
    let pool = get_test_pool().await;

    let mut linter = CsgLinter::new(pool);
    linter
        .initialize()
        .await
        .expect("Failed to initialize linter");

    let source = r#"
        (entity.create-limited-company :name "Acme Corp" :as @company)
        (document.catalog :document-type "PASSPORT" :entity-id @company)
    "#;

    let ast = parse_program(source).expect("Failed to parse");
    let context = ValidationContext::default().with_client_type(ClientType::Corporate);

    let result = linter.lint(ast, &context, source).await;

    assert!(
        result.has_errors(),
        "Expected errors for PASSPORT on company"
    );
    assert!(
        result
            .diagnostics
            .iter()
            .any(|d| d.code == DiagnosticCode::DocumentNotApplicableToEntityType),
        "Expected DocumentNotApplicableToEntityType error"
    );
}

/// Test that PASSPORT document is accepted for PROPER_PERSON entity
#[tokio::test]
#[ignore] // Requires database
async fn test_passport_for_person_accepted() {
    let pool = get_test_pool().await;

    let mut linter = CsgLinter::new(pool);
    linter
        .initialize()
        .await
        .expect("Failed to initialize linter");

    let source = r#"
        (entity.create-proper-person :name "John Doe" :as @person)
        (document.catalog :document-type "PASSPORT" :entity-id @person)
    "#;

    let ast = parse_program(source).expect("Failed to parse");
    let context = ValidationContext::default().with_client_type(ClientType::Individual);

    let result = linter.lint(ast, &context, source).await;

    assert!(
        !result.has_errors(),
        "PASSPORT should be valid for proper person. Errors: {:?}",
        result.diagnostics
    );
}

/// Test that CERTIFICATE_OF_INCORPORATION is accepted for LIMITED_COMPANY
#[tokio::test]
#[ignore] // Requires database
async fn test_cert_incorporation_for_company_accepted() {
    let pool = get_test_pool().await;

    let mut linter = CsgLinter::new(pool);
    linter
        .initialize()
        .await
        .expect("Failed to initialize linter");

    let source = r#"
        (entity.create-limited-company :name "Acme Corp" :as @company)
        (document.catalog :document-type "ARTICLES_OF_INCORPORATION" :entity-id @company)
    "#;

    let ast = parse_program(source).expect("Failed to parse");
    let context = ValidationContext::default().with_client_type(ClientType::Corporate);

    let result = linter.lint(ast, &context, source).await;

    assert!(
        !result.has_errors(),
        "ARTICLES_OF_INCORPORATION should be valid for company. Errors: {:?}",
        result.diagnostics
    );
}

/// Test that undefined symbols are detected
#[tokio::test]
#[ignore] // Requires database
async fn test_undefined_symbol_detected() {
    let pool = get_test_pool().await;

    let mut linter = CsgLinter::new(pool);
    linter
        .initialize()
        .await
        .expect("Failed to initialize linter");

    let source = r#"
        (document.catalog :document-type "PASSPORT" :entity-id @nonexistent)
    "#;

    let ast = parse_program(source).expect("Failed to parse");
    let context = ValidationContext::default();

    let result = linter.lint(ast, &context, source).await;

    assert!(result.has_errors(), "Expected errors for undefined symbol");
    assert!(
        result
            .diagnostics
            .iter()
            .any(|d| d.code == DiagnosticCode::UndefinedSymbol),
        "Expected UndefinedSymbol error"
    );
}

/// Test that unused symbol bindings generate warnings
#[tokio::test]
#[ignore] // Requires database
async fn test_unused_symbol_warning() {
    let pool = get_test_pool().await;

    let mut linter = CsgLinter::new(pool);
    linter
        .initialize()
        .await
        .expect("Failed to initialize linter");

    let source = r#"
        (entity.create-limited-company :name "Acme Corp" :as @unused_company)
        (cbu.create :name "Test CBU" :as @cbu)
    "#;

    let ast = parse_program(source).expect("Failed to parse");
    let context = ValidationContext::default();

    let result = linter.lint(ast, &context, source).await;

    assert!(
        result.has_warnings(),
        "Expected warnings for unused symbols"
    );
    assert!(
        result
            .diagnostics
            .iter()
            .any(|d| d.code == DiagnosticCode::UnusedBinding),
        "Expected UnusedBinding warning"
    );
}

/// Test FINANCIAL_STATEMENT for company (should be accepted)
#[tokio::test]
#[ignore] // Requires database
async fn test_financial_statement_for_company_accepted() {
    let pool = get_test_pool().await;

    let mut linter = CsgLinter::new(pool);
    linter
        .initialize()
        .await
        .expect("Failed to initialize linter");

    let source = r#"
        (entity.create-limited-company :name "Acme Corp" :as @company)
        (document.catalog :document-type "FINANCIAL_STATEMENT" :entity-id @company)
    "#;

    let ast = parse_program(source).expect("Failed to parse");
    let context = ValidationContext::default().with_client_type(ClientType::Corporate);

    let result = linter.lint(ast, &context, source).await;

    assert!(
        !result.has_errors(),
        "FINANCIAL_STATEMENT should be valid for company. Errors: {:?}",
        result.diagnostics
    );
}

/// Test FINANCIAL_STATEMENT for person (should be rejected - not in entity_types)
#[tokio::test]
#[ignore] // Requires database
async fn test_financial_statement_for_person_rejected() {
    let pool = get_test_pool().await;

    let mut linter = CsgLinter::new(pool);
    linter
        .initialize()
        .await
        .expect("Failed to initialize linter");

    let source = r#"
        (entity.create-proper-person :name "John Doe" :as @person)
        (document.catalog :document-type "FINANCIAL_STATEMENT" :entity-id @person)
    "#;

    let ast = parse_program(source).expect("Failed to parse");
    let context = ValidationContext::default().with_client_type(ClientType::Individual);

    let result = linter.lint(ast, &context, source).await;

    assert!(
        result.has_errors(),
        "FINANCIAL_STATEMENT should be rejected for proper person"
    );
    assert!(
        result
            .diagnostics
            .iter()
            .any(|d| d.code == DiagnosticCode::DocumentNotApplicableToEntityType),
        "Expected DocumentNotApplicableToEntityType error"
    );
}

/// Test that rules are loaded from database
#[tokio::test]
#[ignore] // Requires database
async fn test_rules_loaded_from_database() {
    let pool = get_test_pool().await;

    let mut linter = CsgLinter::new(pool);
    linter
        .initialize()
        .await
        .expect("Failed to initialize linter");

    let rules = linter.rules();

    // Verify we loaded document rules
    assert!(
        !rules.document_rules.is_empty(),
        "Should have loaded document rules from database"
    );

    // Check specific rule exists
    assert!(
        rules.document_rules.contains_key("PASSPORT")
            || rules.document_rules.contains_key("passport"),
        "Should have PASSPORT rule loaded"
    );
}

/// Test entity type inference from verb names
#[tokio::test]
#[ignore] // Requires database
async fn test_entity_type_inference() {
    let pool = get_test_pool().await;

    let mut linter = CsgLinter::new(pool);
    linter
        .initialize()
        .await
        .expect("Failed to initialize linter");

    let source = r#"
        (entity.create-limited-company :name "Acme Corp" :as @company)
        (entity.create-proper-person :name "John Doe" :as @person)
    "#;

    let ast = parse_program(source).expect("Failed to parse");
    let context = ValidationContext::default();

    let result = linter.lint(ast, &context, source).await;

    // Check inferred entity types
    let company_info = result.inferred_context.symbols.get("company");
    assert!(company_info.is_some(), "Should have company symbol");
    assert_eq!(
        company_info.unwrap().entity_type.as_deref(),
        Some("LIMITED_COMPANY_PRIVATE"),
        "Should infer LIMITED_COMPANY_PRIVATE from create-limited-company"
    );

    let person_info = result.inferred_context.symbols.get("person");
    assert!(person_info.is_some(), "Should have person symbol");
    assert_eq!(
        person_info.unwrap().entity_type.as_deref(),
        Some("PROPER_PERSON_NATURAL"),
        "Should infer PROPER_PERSON_NATURAL from create-proper-person"
    );
}
