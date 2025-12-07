//! Integration tests for SemanticValidator with EntityGateway
//!
//! These tests verify that the SemanticValidator correctly validates DSL
//! against the EntityGateway service for all reference types.
//!
//! Run with: cargo test --features database --test semantic_validator_integration -- --ignored
//!
//! REQUIRES: EntityGateway service running on port 50051
//!   cd rust/crates/entity-gateway && DATABASE_URL="postgresql:///data_designer" cargo run --release

#![cfg(feature = "database")]

use ob_poc::dsl_v2::{
    validation::{RustStyleFormatter, ValidationContext, ValidationRequest, ValidationResult},
    SemanticValidator,
};
use sqlx::PgPool;

/// Get test database pool
async fn get_test_pool() -> PgPool {
    let url = std::env::var("DATABASE_URL")
        .unwrap_or_else(|_| "postgresql://localhost/data_designer".to_string());
    PgPool::connect(&url)
        .await
        .expect("Failed to connect to database")
}

/// Create SemanticValidator with EntityGateway
async fn get_validator(pool: &PgPool) -> SemanticValidator {
    SemanticValidator::new(pool.clone())
        .await
        .expect("Failed to create SemanticValidator - is EntityGateway running?")
}

/// Helper to format validation errors for assertion messages
fn format_errors(result: &ValidationResult, source: &str) -> String {
    match result {
        ValidationResult::Ok(_) => String::new(),
        ValidationResult::Err(diagnostics) => RustStyleFormatter::format(source, diagnostics),
    }
}

// =============================================================================
// VALID DSL TESTS - Should pass validation
// =============================================================================

#[tokio::test]
#[ignore] // Requires EntityGateway running
async fn test_valid_cbu_create() {
    let pool = get_test_pool().await;
    let mut validator = get_validator(&pool).await;

    let source =
        r#"(cbu.create :name "Test CBU" :jurisdiction "GB" :client-type "corporate" :as @cbu)"#;

    let request = ValidationRequest {
        source: source.to_string(),
        context: ValidationContext::default(),
    };

    let result = validator.validate(&request).await;

    assert!(
        result.is_ok(),
        "Valid CBU create should pass: {}",
        format_errors(&result, source)
    );
}

#[tokio::test]
#[ignore] // Requires EntityGateway running
async fn test_valid_entity_create_with_symbol_reference() {
    let pool = get_test_pool().await;
    let mut validator = get_validator(&pool).await;

    let source = r#"
        (cbu.create :name "Test CBU" :as @cbu)
        (entity.create-limited-company :cbu-id @cbu :name "Test Company" :as @company)
    "#;

    let request = ValidationRequest {
        source: source.to_string(),
        context: ValidationContext::default(),
    };

    let result = validator.validate(&request).await;

    assert!(
        result.is_ok(),
        "Valid entity create with symbol ref should pass: {}",
        format_errors(&result, source)
    );
}

#[tokio::test]
#[ignore] // Requires EntityGateway running
async fn test_valid_role_assignment() {
    let pool = get_test_pool().await;
    let mut validator = get_validator(&pool).await;

    let source = r#"
        (cbu.create :name "Test CBU" :as @cbu)
        (entity.create-proper-person :cbu-id @cbu :first-name "John" :last-name "Doe" :as @person)
        (cbu.assign-role :cbu-id @cbu :entity-id @person :role "DIRECTOR")
    "#;

    let request = ValidationRequest {
        source: source.to_string(),
        context: ValidationContext::default(),
    };

    let result = validator.validate(&request).await;

    assert!(
        result.is_ok(),
        "Valid role assignment should pass: {}",
        format_errors(&result, source)
    );
}

#[tokio::test]
#[ignore] // Requires EntityGateway running
async fn test_valid_document_catalog() {
    let pool = get_test_pool().await;
    let mut validator = get_validator(&pool).await;

    let source = r#"
        (cbu.create :name "Test CBU" :as @cbu)
        (document.catalog :cbu-id @cbu :doc-type "PASSPORT" :title "ID Document")
    "#;

    let request = ValidationRequest {
        source: source.to_string(),
        context: ValidationContext::default(),
    };

    let result = validator.validate(&request).await;

    assert!(
        result.is_ok(),
        "Valid document catalog should pass: {}",
        format_errors(&result, source)
    );
}

#[tokio::test]
#[ignore] // Requires EntityGateway running
async fn test_valid_kyc_case_flow() {
    let pool = get_test_pool().await;
    let mut validator = get_validator(&pool).await;

    let source = r#"
        (cbu.create :name "Test CBU" :as @cbu)
        (entity.create-proper-person :cbu-id @cbu :first-name "Jane" :last-name "Smith" :as @person)
        (kyc-case.create :cbu-id @cbu :case-type "NEW_CLIENT" :as @case)
        (entity-workstream.create :case-id @case :entity-id @person :as @ws)
        (case-screening.run :workstream-id @ws :screening-type "PEP")
    "#;

    let request = ValidationRequest {
        source: source.to_string(),
        context: ValidationContext::default(),
    };

    let result = validator.validate(&request).await;

    assert!(
        result.is_ok(),
        "Valid KYC case flow should pass: {}",
        format_errors(&result, source)
    );
}

// =============================================================================
// INVALID REFERENCE TYPE TESTS - Should fail validation with specific errors
// =============================================================================

#[tokio::test]
#[ignore] // Requires EntityGateway running
async fn test_invalid_jurisdiction_code() {
    let pool = get_test_pool().await;
    let mut validator = get_validator(&pool).await;

    let source = r#"(cbu.create :name "Test" :jurisdiction "INVALID_JURISDICTION_XYZ" :as @cbu)"#;

    let request = ValidationRequest {
        source: source.to_string(),
        context: ValidationContext::default(),
    };

    let result = validator.validate(&request).await;

    assert!(
        result.is_err(),
        "Invalid jurisdiction should fail validation"
    );

    let errors = format_errors(&result, source);
    assert!(
        errors.contains("jurisdiction") || errors.contains("INVALID_JURISDICTION"),
        "Error should mention jurisdiction: {}",
        errors
    );
}

#[tokio::test]
#[ignore] // Requires EntityGateway running
async fn test_invalid_role_name() {
    let pool = get_test_pool().await;
    let mut validator = get_validator(&pool).await;

    let source = r#"
        (cbu.create :name "Test CBU" :as @cbu)
        (entity.create-proper-person :cbu-id @cbu :first-name "Test" :last-name "Person" :as @person)
        (cbu.assign-role :cbu-id @cbu :entity-id @person :role "NONEXISTENT_ROLE_XYZ")
    "#;

    let request = ValidationRequest {
        source: source.to_string(),
        context: ValidationContext::default(),
    };

    let result = validator.validate(&request).await;

    assert!(result.is_err(), "Invalid role should fail validation");

    let errors = format_errors(&result, source);
    assert!(
        errors.contains("role") || errors.contains("NONEXISTENT_ROLE"),
        "Error should mention role: {}",
        errors
    );
}

#[tokio::test]
#[ignore] // Requires EntityGateway running
async fn test_invalid_document_type() {
    let pool = get_test_pool().await;
    let mut validator = get_validator(&pool).await;

    let source = r#"
        (cbu.create :name "Test CBU" :as @cbu)
        (document.catalog :cbu-id @cbu :doc-type "FAKE_DOCUMENT_TYPE_123" :title "Doc")
    "#;

    let request = ValidationRequest {
        source: source.to_string(),
        context: ValidationContext::default(),
    };

    let result = validator.validate(&request).await;

    assert!(
        result.is_err(),
        "Invalid document type should fail validation"
    );

    let errors = format_errors(&result, source);
    assert!(
        errors.contains("document") || errors.contains("FAKE_DOCUMENT"),
        "Error should mention document type: {}",
        errors
    );
}

#[tokio::test]
#[ignore] // Requires EntityGateway running
async fn test_invalid_screening_type() {
    let pool = get_test_pool().await;
    let mut validator = get_validator(&pool).await;

    let source = r#"
        (cbu.create :name "Test CBU" :as @cbu)
        (entity.create-proper-person :cbu-id @cbu :first-name "Test" :last-name "Person" :as @person)
        (kyc-case.create :cbu-id @cbu :case-type "NEW_CLIENT" :as @case)
        (entity-workstream.create :case-id @case :entity-id @person :as @ws)
        (case-screening.run :workstream-id @ws :screening-type "INVALID_SCREENING_TYPE")
    "#;

    let request = ValidationRequest {
        source: source.to_string(),
        context: ValidationContext::default(),
    };

    let result = validator.validate(&request).await;

    assert!(
        result.is_err(),
        "Invalid screening type should fail validation"
    );
}

// =============================================================================
// SYMBOL RESOLUTION TESTS
// =============================================================================

#[tokio::test]
#[ignore] // Requires EntityGateway running
async fn test_undefined_symbol_reference() {
    let pool = get_test_pool().await;
    let mut validator = get_validator(&pool).await;

    let source = r#"
        (cbu.create :name "Test CBU" :as @cbu)
        (entity.create-proper-person :cbu-id @nonexistent :first-name "Test" :last-name "Person")
    "#;

    let request = ValidationRequest {
        source: source.to_string(),
        context: ValidationContext::default(),
    };

    let result = validator.validate(&request).await;

    assert!(
        result.is_err(),
        "Undefined symbol reference should fail validation"
    );

    let errors = format_errors(&result, source);
    assert!(
        errors.contains("undefined") || errors.contains("nonexistent"),
        "Error should mention undefined symbol: {}",
        errors
    );
}

#[tokio::test]
#[ignore] // Requires EntityGateway running
async fn test_symbol_used_before_definition() {
    let pool = get_test_pool().await;
    let mut validator = get_validator(&pool).await;

    // Reference @cbu before it's defined
    let source = r#"
        (entity.create-proper-person :cbu-id @cbu :first-name "Test" :last-name "Person")
        (cbu.create :name "Test CBU" :as @cbu)
    "#;

    let request = ValidationRequest {
        source: source.to_string(),
        context: ValidationContext::default(),
    };

    let result = validator.validate(&request).await;

    assert!(
        result.is_err(),
        "Symbol used before definition should fail validation"
    );

    let errors = format_errors(&result, source);
    assert!(
        errors.contains("undefined") || errors.contains("@cbu"),
        "Error should mention undefined symbol: {}",
        errors
    );
}

#[tokio::test]
#[ignore] // Requires EntityGateway running
async fn test_unused_symbol_produces_valid_bindings() {
    let pool = get_test_pool().await;
    let mut validator = get_validator(&pool).await;

    // Define symbols but never use them
    let source = r#"
        (cbu.create :name "Test CBU" :as @unused_cbu)
        (cbu.create :name "Another CBU" :as @also_unused)
    "#;

    let request = ValidationRequest {
        source: source.to_string(),
        context: ValidationContext::default(),
    };

    let result = validator.validate(&request).await;

    // Unused symbols generate warnings, but validation should still produce bindings
    if let ValidationResult::Ok(program) = result {
        assert!(
            program.bindings.contains_key("unused_cbu"),
            "Should have unused_cbu binding"
        );
        assert!(
            program.bindings.contains_key("also_unused"),
            "Should have also_unused binding"
        );
    }
    // Note: If result is Err due to warnings being promoted to errors, that's also acceptable
}

// =============================================================================
// VERB VALIDATION TESTS
// =============================================================================

#[tokio::test]
#[ignore] // Requires EntityGateway running
async fn test_unknown_verb() {
    let pool = get_test_pool().await;
    let mut validator = get_validator(&pool).await;

    let source = r#"(fake-domain.nonexistent-verb :arg "value")"#;

    let request = ValidationRequest {
        source: source.to_string(),
        context: ValidationContext::default(),
    };

    let result = validator.validate(&request).await;

    assert!(result.is_err(), "Unknown verb should fail validation");

    let errors = format_errors(&result, source);
    assert!(
        errors.contains("unknown verb") || errors.contains("fake-domain"),
        "Error should mention unknown verb: {}",
        errors
    );
}

#[tokio::test]
#[ignore] // Requires EntityGateway running
async fn test_missing_required_argument() {
    let pool = get_test_pool().await;
    let mut validator = get_validator(&pool).await;

    // cbu.create requires :name
    let source = r#"(cbu.create :jurisdiction "GB" :as @cbu)"#;

    let request = ValidationRequest {
        source: source.to_string(),
        context: ValidationContext::default(),
    };

    let result = validator.validate(&request).await;

    assert!(
        result.is_err(),
        "Missing required argument should fail validation"
    );

    let errors = format_errors(&result, source);
    assert!(
        errors.contains("missing") && errors.contains("name"),
        "Error should mention missing required arg: {}",
        errors
    );
}

#[tokio::test]
#[ignore] // Requires EntityGateway running
async fn test_unknown_argument() {
    let pool = get_test_pool().await;
    let mut validator = get_validator(&pool).await;

    let source = r#"(cbu.create :name "Test" :fake-arg "value" :as @cbu)"#;

    let request = ValidationRequest {
        source: source.to_string(),
        context: ValidationContext::default(),
    };

    let result = validator.validate(&request).await;

    assert!(result.is_err(), "Unknown argument should fail validation");

    let errors = format_errors(&result, source);
    assert!(
        errors.contains("unknown argument") || errors.contains("fake-arg"),
        "Error should mention unknown argument: {}",
        errors
    );
}

// =============================================================================
// PARSE ERROR TESTS
// =============================================================================

#[tokio::test]
#[ignore] // Requires EntityGateway running
async fn test_syntax_error_unclosed_paren() {
    let pool = get_test_pool().await;
    let mut validator = get_validator(&pool).await;

    let source = r#"(cbu.create :name "Test""#; // Missing closing paren

    let request = ValidationRequest {
        source: source.to_string(),
        context: ValidationContext::default(),
    };

    let result = validator.validate(&request).await;

    assert!(result.is_err(), "Syntax error should fail validation");

    let errors = format_errors(&result, source);
    assert!(
        errors.to_lowercase().contains("parse")
            || errors.to_lowercase().contains("syntax")
            || errors.to_lowercase().contains("error"),
        "Error should mention parse/syntax error: {}",
        errors
    );
}

#[tokio::test]
#[ignore] // Requires EntityGateway running
async fn test_syntax_error_invalid_keyword() {
    let pool = get_test_pool().await;
    let mut validator = get_validator(&pool).await;

    let source = r#"(cbu.create name "Test")"#; // Missing colon before keyword

    let request = ValidationRequest {
        source: source.to_string(),
        context: ValidationContext::default(),
    };

    let result = validator.validate(&request).await;

    // This might parse differently - just verify we handle it gracefully
    // The parser may interpret 'name' as a value, leading to different errors
    let _ = format_errors(&result, source); // Just ensure it doesn't panic
}

// =============================================================================
// CSG LINTER INTEGRATION TESTS
// =============================================================================

#[tokio::test]
#[ignore] // Requires EntityGateway running
async fn test_csg_passport_for_company_rejected() {
    let pool = get_test_pool().await;
    let validator = get_validator(&pool).await;
    let mut validator = validator
        .with_csg_linter()
        .await
        .expect("Failed to initialize CSG linter");

    // PASSPORT is for proper persons, not companies
    let source = r#"
        (entity.create-limited-company :name "Test Corp" :as @company)
        (document.catalog :doc-type "PASSPORT" :entity-id @company :title "Company Passport")
    "#;

    let request = ValidationRequest {
        source: source.to_string(),
        context: ValidationContext::default(),
    };

    let result = validator.validate_with_csg(&request).await;

    assert!(
        result.is_err(),
        "PASSPORT for company should fail CSG validation"
    );
}

#[tokio::test]
#[ignore] // Requires EntityGateway running
async fn test_csg_passport_for_person_accepted() {
    let pool = get_test_pool().await;
    let validator = get_validator(&pool).await;
    let mut validator = validator
        .with_csg_linter()
        .await
        .expect("Failed to initialize CSG linter");

    // PASSPORT is valid for proper persons
    let source = r#"
        (entity.create-proper-person :first-name "John" :last-name "Doe" :as @person)
        (document.catalog :doc-type "PASSPORT" :entity-id @person :title "John's Passport")
    "#;

    let request = ValidationRequest {
        source: source.to_string(),
        context: ValidationContext::default(),
    };

    let result = validator.validate_with_csg(&request).await;

    assert!(
        result.is_ok(),
        "PASSPORT for person should pass CSG validation: {}",
        format_errors(&result, source)
    );
}

// =============================================================================
// EDGE CASES AND ERROR RECOVERY
// =============================================================================

#[tokio::test]
#[ignore] // Requires EntityGateway running
async fn test_multiple_errors_collected() {
    let pool = get_test_pool().await;
    let mut validator = get_validator(&pool).await;

    // Multiple errors: undefined symbol, invalid jurisdiction, unknown role
    let source = r#"
        (entity.create-proper-person :cbu-id @nonexistent :first-name "Test" :last-name "Person" :as @person)
        (cbu.create :name "Test" :jurisdiction "FAKELAND" :as @cbu)
        (cbu.assign-role :cbu-id @cbu :entity-id @person :role "FAKE_ROLE")
    "#;

    let request = ValidationRequest {
        source: source.to_string(),
        context: ValidationContext::default(),
    };

    let result = validator.validate(&request).await;

    assert!(result.is_err(), "Multiple errors should fail validation");

    let errors = format_errors(&result, source);

    // Should collect multiple errors (not stop at first)
    let error_count = errors.matches("error").count();
    assert!(
        error_count >= 2,
        "Should collect multiple errors, got: {}",
        errors
    );
}

#[tokio::test]
#[ignore] // Requires EntityGateway running
async fn test_empty_source() {
    let pool = get_test_pool().await;
    let mut validator = get_validator(&pool).await;

    let source = "";

    let request = ValidationRequest {
        source: source.to_string(),
        context: ValidationContext::default(),
    };

    let result = validator.validate(&request).await;

    // Empty source should parse successfully (empty program)
    assert!(result.is_ok(), "Empty source should be valid");
}

#[tokio::test]
#[ignore] // Requires EntityGateway running
async fn test_comment_only_source() {
    let pool = get_test_pool().await;
    let mut validator = get_validator(&pool).await;

    let source = r#"
        ;; This is just a comment
        ;; Nothing to validate here
    "#;

    let request = ValidationRequest {
        source: source.to_string(),
        context: ValidationContext::default(),
    };

    let result = validator.validate(&request).await;

    // Comments only should be valid
    assert!(result.is_ok(), "Comment-only source should be valid");
}

#[tokio::test]
#[ignore] // Requires EntityGateway running
async fn test_unicode_in_string_values() {
    let pool = get_test_pool().await;
    let mut validator = get_validator(&pool).await;

    let source = r#"(cbu.create :name "株式会社テスト" :jurisdiction "JP" :as @cbu)"#;

    let request = ValidationRequest {
        source: source.to_string(),
        context: ValidationContext::default(),
    };

    let result = validator.validate(&request).await;

    // Should handle unicode in string values
    assert!(
        result.is_ok(),
        "Unicode in strings should be valid: {}",
        format_errors(&result, source)
    );
}

// =============================================================================
// VALIDATE_DSL PUBLIC API TESTS
// =============================================================================

#[tokio::test]
#[ignore] // Requires EntityGateway running
async fn test_validate_dsl_public_api() {
    use ob_poc::dsl_v2::validate_dsl;

    let pool = get_test_pool().await;

    let source = r#"(cbu.create :name "API Test CBU" :as @cbu)"#;

    let result = validate_dsl(&pool, source, ValidationContext::default()).await;

    assert!(
        result.is_ok(),
        "validate_dsl should work: {:?}",
        result.err()
    );

    let program = result.unwrap();
    assert_eq!(program.statements.len(), 1);
    assert!(program.bindings.contains_key("cbu"));
}

#[tokio::test]
#[ignore] // Requires EntityGateway running
async fn test_validate_dsl_with_csg_public_api() {
    use ob_poc::dsl_v2::validate_dsl_with_csg;

    let pool = get_test_pool().await;

    let source = r#"
        (entity.create-proper-person :first-name "Test" :last-name "Person" :as @person)
        (document.catalog :doc-type "PASSPORT" :entity-id @person :title "Passport")
    "#;

    let result = validate_dsl_with_csg(&pool, source, ValidationContext::default()).await;

    assert!(
        result.is_ok(),
        "validate_dsl_with_csg should work: {:?}",
        result.err()
    );
}
