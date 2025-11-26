@# Design: Onboarding DSL Test Harness

**Created:** 2025-11-25  
**Status:** DESIGN SPECIFICATION  
**Scope:** Test harness for onboarding request → DSL validation → AST persistence  
**Dependencies:** DESIGN_VERB_SCHEMA.md (schema validator)

---

## Executive Summary

This document specifies a test harness for the onboarding DSL pipeline that:

1. Creates an onboarding request (CBU → Products)
2. Submits DSL source for validation
3. Saves validated AST to database using existing repositories
4. **Verifies all database operations by querying back**

Uses existing CRUD operations — minimal new code required.

---

## Part 1: Existing Infrastructure

### 1.1 Available Repositories

```rust
// rust/src/database/dsl_repository.rs
pub struct DslRepository {
    pub async fn save_dsl_instance(
        &self,
        business_reference: &str,      // Links to onboarding request
        domain_name: &str,             // "onboarding"
        dsl_content: &str,
        ast_json: Option<&serde_json::Value>,
        operation_type: &str,          // "VALIDATE", "EXECUTE"
    ) -> Result<DslSaveResult>;
    
    pub async fn load_dsl(&self, business_reference: &str) -> Result<Option<(String, i32)>>;
    pub async fn load_ast(&self, business_reference: &str) -> Result<Option<serde_json::Value>>;
    pub async fn get_instance_by_reference(&self, business_reference: &str) -> Result<Option<DslInstanceRow>>;
}

// rust/src/taxonomy/crud_operations.rs
pub struct TaxonomyCrudOperations {
    pub async fn create_onboarding(&self, create: CreateOnboarding) -> Result<Uuid>;
    pub async fn add_products_to_onboarding(&self, add: AddProductsToOnboarding) -> Result<Vec<Uuid>>;
}
```

### 1.2 Database Tables

```sql
-- Onboarding (existing)
"ob-poc".onboarding_requests (request_id, cbu_id, request_state, dsl_draft, validation_errors, ...)
"ob-poc".onboarding_products (request_id, product_id, selection_order)

-- DSL with versioning (existing)
"ob-poc".dsl_instances (instance_id, domain_name, business_reference, current_version, status)
"ob-poc".dsl_instance_versions (instance_id, version_number, dsl_content, ast_json, compilation_status)
```

### 1.3 Business Reference Convention

```
business_reference = "onboarding:{request_id}"

Example: "onboarding:550e8400-e29b-41d4-a716-446655440000"
```

This links `onboarding_requests.request_id` to `dsl_instances.business_reference`.

---

## Part 2: Test Harness Implementation

### 2.1 Core Types

```rust
// rust/src/dsl_runtime/test_harness.rs

use crate::database::{DslRepository, DslSaveResult};
use crate::taxonomy::{TaxonomyCrudOperations, CreateOnboarding, AddProductsToOnboarding};
use crate::dsl_runtime::{SchemaValidator, SchemaCache, parse_program, RuntimeEnv, ContextKey};
use sqlx::PgPool;
use uuid::Uuid;
use std::sync::Arc;
use serde::{Serialize, Deserialize};

/// Test harness for onboarding DSL validation pipeline
pub struct OnboardingTestHarness {
    pool: PgPool,
    dsl_repo: DslRepository,
    taxonomy_ops: TaxonomyCrudOperations,
    schema_cache: Arc<SchemaCache>,
    validator: SchemaValidator,
}

/// Test input
#[derive(Debug, Clone)]
pub struct OnboardingTestInput {
    pub cbu_id: Uuid,
    pub product_codes: Vec<String>,
    pub dsl_source: String,
}

/// Test result with verification
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct OnboardingTestResult {
    // IDs
    pub request_id: Uuid,
    pub dsl_instance_id: Option<Uuid>,
    pub dsl_version: Option<i32>,
    
    // Validation outcome
    pub validation_passed: bool,
    pub errors: Vec<ValidationErrorInfo>,
    
    // Performance
    pub parse_time_ms: u64,
    pub validate_time_ms: u64,
    pub persist_time_ms: u64,
    pub total_time_ms: u64,
    
    // Verification (proves DB writes worked)
    pub verification: VerificationResult,
}

/// Verification that DB writes succeeded
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct VerificationResult {
    // Onboarding request
    pub request_exists: bool,
    pub request_state: String,
    pub products_linked: usize,
    pub expected_products: usize,
    
    // DSL instance
    pub dsl_instance_exists: bool,
    pub dsl_content_matches: bool,
    pub dsl_version: i32,
    
    // AST
    pub ast_exists: bool,
    pub ast_has_expressions: bool,
    pub ast_has_symbol_table: bool,
    pub symbol_count: usize,
    
    // Errors (if validation failed)
    pub errors_stored: bool,
    pub error_count: usize,
    
    // Overall
    pub all_checks_passed: bool,
}

/// Serializable validation error
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ValidationErrorInfo {
    pub line: u32,
    pub column: u32,
    pub code: String,
    pub message: String,
    pub suggestion: Option<String>,
}
```

### 2.2 Harness Implementation

```rust
impl OnboardingTestHarness {
    /// Create new test harness
    pub async fn new(pool: PgPool) -> Result<Self> {
        let schema_cache = Arc::new(SchemaCache::load(&pool).await?);
        let validator = SchemaValidator::new(schema_cache.clone());
        let dsl_repo = DslRepository::new(pool.clone());
        let taxonomy_ops = TaxonomyCrudOperations::new(pool.clone());
        
        Ok(Self {
            pool,
            dsl_repo,
            taxonomy_ops,
            schema_cache,
            validator,
        })
    }
    
    /// Run complete onboarding test
    pub async fn run_test(&self, input: OnboardingTestInput) -> Result<OnboardingTestResult> {
        let total_start = std::time::Instant::now();
        
        // ═══════════════════════════════════════════════════════════════
        // STEP 1: Create onboarding request
        // ═══════════════════════════════════════════════════════════════
        let request_id = self.taxonomy_ops.create_onboarding(CreateOnboarding {
            cbu_id: input.cbu_id,
            metadata: None,
        }).await?;
        
        // ═══════════════════════════════════════════════════════════════
        // STEP 2: Link products
        // ═══════════════════════════════════════════════════════════════
        if !input.product_codes.is_empty() {
            self.taxonomy_ops.add_products_to_onboarding(AddProductsToOnboarding {
                onboarding_id: request_id,
                product_codes: input.product_codes.clone(),
            }).await?;
        }
        
        // ═══════════════════════════════════════════════════════════════
        // STEP 3: Parse DSL
        // ═══════════════════════════════════════════════════════════════
        let parse_start = std::time::Instant::now();
        let raw_ast = match parse_program(&input.dsl_source) {
            Ok(ast) => ast,
            Err(parse_err) => {
                let error = ValidationErrorInfo {
                    line: 0,
                    column: 0,
                    code: "E000".to_string(),
                    message: format!("Parse error: {:?}", parse_err),
                    suggestion: None,
                };
                
                // Store error in onboarding request
                self.store_validation_errors(request_id, &[error.clone()]).await?;
                
                let verification = self.verify(
                    request_id, 
                    &input.product_codes, 
                    &input.dsl_source,
                    false
                ).await?;
                
                return Ok(OnboardingTestResult {
                    request_id,
                    dsl_instance_id: None,
                    dsl_version: None,
                    validation_passed: false,
                    errors: vec![error],
                    parse_time_ms: parse_start.elapsed().as_millis() as u64,
                    validate_time_ms: 0,
                    persist_time_ms: 0,
                    total_time_ms: total_start.elapsed().as_millis() as u64,
                    verification,
                });
            }
        };
        let parse_time_ms = parse_start.elapsed().as_millis() as u64;
        
        // ═══════════════════════════════════════════════════════════════
        // STEP 4: Build runtime environment
        // ═══════════════════════════════════════════════════════════════
        let mut env = RuntimeEnv::new();
        env.set_context(ContextKey::CbuId, input.cbu_id);
        
        // ═══════════════════════════════════════════════════════════════
        // STEP 5: Validate against schema
        // ═══════════════════════════════════════════════════════════════
        let validate_start = std::time::Instant::now();
        let validation_result = self.validator.validate(&raw_ast, &env);
        let validate_time_ms = validate_start.elapsed().as_millis() as u64;
        
        match validation_result {
            Ok(validated_ast) => {
                // ═══════════════════════════════════════════════════════
                // STEP 6: Persist DSL + AST via DslRepository
                // ═══════════════════════════════════════════════════════
                let persist_start = std::time::Instant::now();
                
                let business_reference = format!("onboarding:{}", request_id);
                let ast_json = serde_json::json!({
                    "expressions": validated_ast.expressions,
                    "symbol_table": validated_ast.symbol_table,
                    "validator_version": env!("CARGO_PKG_VERSION"),
                });
                
                let save_result = self.dsl_repo.save_dsl_instance(
                    &business_reference,
                    "onboarding",
                    &input.dsl_source,
                    Some(&ast_json),
                    "VALIDATE",
                ).await?;
                
                // Update onboarding request state
                self.update_onboarding_state(request_id, "validated").await?;
                
                let persist_time_ms = persist_start.elapsed().as_millis() as u64;
                
                // ═══════════════════════════════════════════════════════
                // STEP 7: Verify all writes
                // ═══════════════════════════════════════════════════════
                let verification = self.verify(
                    request_id, 
                    &input.product_codes,
                    &input.dsl_source,
                    true
                ).await?;
                
                Ok(OnboardingTestResult {
                    request_id,
                    dsl_instance_id: Some(save_result.instance_id),
                    dsl_version: Some(save_result.version),
                    validation_passed: true,
                    errors: vec![],
                    parse_time_ms,
                    validate_time_ms,
                    persist_time_ms,
                    total_time_ms: total_start.elapsed().as_millis() as u64,
                    verification,
                })
            }
            
            Err(report) => {
                // ═══════════════════════════════════════════════════════
                // STEP 6 (error path): Store validation errors
                // ═══════════════════════════════════════════════════════
                let errors: Vec<ValidationErrorInfo> = report.errors.iter().map(|e| {
                    ValidationErrorInfo {
                        line: e.span.line,
                        column: e.span.column,
                        code: e.kind.code().to_string(),
                        message: e.kind.message(),
                        suggestion: e.kind.hint(),
                    }
                }).collect();
                
                self.store_validation_errors(request_id, &errors).await?;
                
                let verification = self.verify(
                    request_id,
                    &input.product_codes,
                    &input.dsl_source,
                    false
                ).await?;
                
                Ok(OnboardingTestResult {
                    request_id,
                    dsl_instance_id: None,
                    dsl_version: None,
                    validation_passed: false,
                    errors,
                    parse_time_ms,
                    validate_time_ms,
                    persist_time_ms: 0,
                    total_time_ms: total_start.elapsed().as_millis() as u64,
                    verification,
                })
            }
        }
    }
    
    /// Store validation errors in onboarding_requests
    async fn store_validation_errors(
        &self,
        request_id: Uuid,
        errors: &[ValidationErrorInfo],
    ) -> Result<()> {
        sqlx::query!(
            r#"
            UPDATE "ob-poc".onboarding_requests
            SET validation_errors = $1,
                request_state = 'draft',
                updated_at = NOW()
            WHERE request_id = $2
            "#,
            serde_json::to_value(errors)?,
            request_id
        )
        .execute(&self.pool)
        .await?;
        
        Ok(())
    }
    
    /// Update onboarding request state
    async fn update_onboarding_state(&self, request_id: Uuid, state: &str) -> Result<()> {
        sqlx::query!(
            r#"
            UPDATE "ob-poc".onboarding_requests
            SET request_state = $1,
                validation_errors = NULL,
                updated_at = NOW()
            WHERE request_id = $2
            "#,
            state,
            request_id
        )
        .execute(&self.pool)
        .await?;
        
        Ok(())
    }
}
```

### 2.3 Database Verification

```rust
impl OnboardingTestHarness {
    /// Verify all database writes by querying back
    async fn verify(
        &self,
        request_id: Uuid,
        expected_product_codes: &[String],
        expected_dsl: &str,
        expect_ast: bool,
    ) -> Result<VerificationResult> {
        let business_reference = format!("onboarding:{}", request_id);
        
        // ═══════════════════════════════════════════════════════════════
        // VERIFY 1: Onboarding request exists
        // ═══════════════════════════════════════════════════════════════
        let request = sqlx::query!(
            r#"
            SELECT request_id, request_state, validation_errors
            FROM "ob-poc".onboarding_requests
            WHERE request_id = $1
            "#,
            request_id
        )
        .fetch_optional(&self.pool)
        .await?;
        
        let (request_exists, request_state, errors_stored, error_count) = match &request {
            Some(r) => {
                let err_count = r.validation_errors
                    .as_ref()
                    .and_then(|v| v.as_array())
                    .map(|a| a.len())
                    .unwrap_or(0);
                (
                    true,
                    r.request_state.clone().unwrap_or_default(),
                    err_count > 0,
                    err_count,
                )
            }
            None => (false, String::new(), false, 0),
        };
        
        // ═══════════════════════════════════════════════════════════════
        // VERIFY 2: Products are linked
        // ═══════════════════════════════════════════════════════════════
        let products_linked = sqlx::query_scalar!(
            r#"
            SELECT COUNT(*) as "count!"
            FROM "ob-poc".onboarding_products
            WHERE request_id = $1
            "#,
            request_id
        )
        .fetch_one(&self.pool)
        .await? as usize;
        
        // ═══════════════════════════════════════════════════════════════
        // VERIFY 3: DSL instance exists and content matches
        // ═══════════════════════════════════════════════════════════════
        let dsl_instance = self.dsl_repo
            .get_instance_by_reference(&business_reference)
            .await?;
        
        let (dsl_instance_exists, dsl_version) = match &dsl_instance {
            Some(inst) => (true, inst.current_version),
            None => (false, 0),
        };
        
        let loaded_dsl = self.dsl_repo.load_dsl(&business_reference).await?;
        let dsl_content_matches = loaded_dsl
            .as_ref()
            .map(|(content, _)| content == expected_dsl)
            .unwrap_or(false);
        
        // ═══════════════════════════════════════════════════════════════
        // VERIFY 4: AST exists and is valid
        // ═══════════════════════════════════════════════════════════════
        let loaded_ast = self.dsl_repo.load_ast(&business_reference).await?;
        
        let (ast_exists, ast_has_expressions, ast_has_symbol_table, symbol_count) = match &loaded_ast {
            Some(ast) => {
                let has_expr = ast.get("expressions").is_some();
                let has_st = ast.get("symbol_table").is_some();
                let sym_count = ast
                    .get("symbol_table")
                    .and_then(|st| st.as_object())
                    .map(|o| o.len())
                    .unwrap_or(0);
                (true, has_expr, has_st, sym_count)
            }
            None => (false, false, false, 0),
        };
        
        // ═══════════════════════════════════════════════════════════════
        // VERIFY 5: All checks passed?
        // ═══════════════════════════════════════════════════════════════
        let all_checks_passed = request_exists
            && products_linked == expected_product_codes.len()
            && (!expect_ast || (dsl_instance_exists && dsl_content_matches && ast_exists && ast_has_expressions && ast_has_symbol_table))
            && (expect_ast || errors_stored);
        
        Ok(VerificationResult {
            request_exists,
            request_state,
            products_linked,
            expected_products: expected_product_codes.len(),
            dsl_instance_exists,
            dsl_content_matches,
            dsl_version,
            ast_exists,
            ast_has_expressions,
            ast_has_symbol_table,
            symbol_count,
            errors_stored,
            error_count,
            all_checks_passed,
        })
    }
    
    /// Verify specific symbols exist in stored AST
    pub async fn verify_symbols(
        &self,
        request_id: Uuid,
        expected_symbols: &[&str],
    ) -> Result<SymbolVerification> {
        let business_reference = format!("onboarding:{}", request_id);
        let ast = self.dsl_repo.load_ast(&business_reference).await?;
        
        let symbol_table = ast
            .as_ref()
            .and_then(|a| a.get("symbol_table"))
            .and_then(|st| st.as_object())
            .cloned()
            .unwrap_or_default();
        
        let found: Vec<String> = symbol_table.keys().cloned().collect();
        let missing: Vec<String> = expected_symbols
            .iter()
            .filter(|s| !found.contains(&s.to_string()))
            .map(|s| s.to_string())
            .collect();
        
        Ok(SymbolVerification {
            expected: expected_symbols.iter().map(|s| s.to_string()).collect(),
            found,
            missing: missing.clone(),
            all_present: missing.is_empty(),
        })
    }
    
    /// Verify validation errors match expected codes
    pub async fn verify_errors(
        &self,
        request_id: Uuid,
        expected_codes: &[&str],
    ) -> Result<ErrorVerification> {
        let request = sqlx::query!(
            r#"
            SELECT validation_errors
            FROM "ob-poc".onboarding_requests
            WHERE request_id = $1
            "#,
            request_id
        )
        .fetch_one(&self.pool)
        .await?;
        
        let stored: Vec<ValidationErrorInfo> = request
            .validation_errors
            .map(|v| serde_json::from_value(v).unwrap_or_default())
            .unwrap_or_default();
        
        let stored_codes: Vec<String> = stored.iter().map(|e| e.code.clone()).collect();
        let missing: Vec<String> = expected_codes
            .iter()
            .filter(|c| !stored_codes.contains(&c.to_string()))
            .map(|c| c.to_string())
            .collect();
        
        Ok(ErrorVerification {
            expected: expected_codes.iter().map(|c| c.to_string()).collect(),
            found: stored_codes,
            missing: missing.clone(),
            all_present: missing.is_empty(),
        })
    }
}

/// Symbol verification result
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct SymbolVerification {
    pub expected: Vec<String>,
    pub found: Vec<String>,
    pub missing: Vec<String>,
    pub all_present: bool,
}

/// Error verification result
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ErrorVerification {
    pub expected: Vec<String>,
    pub found: Vec<String>,
    pub missing: Vec<String>,
    pub all_present: bool,
}
```

---

## Part 3: Integration Tests

### 3.1 Test Fixtures

```sql
-- fixtures/seed_test_cbu.sql
INSERT INTO "ob-poc".cbus (cbu_id, name, jurisdiction, status)
VALUES ('11111111-1111-1111-1111-111111111111', 'Test Fund Ltd', 'LU', 'ACTIVE')
ON CONFLICT (cbu_id) DO NOTHING;

-- fixtures/seed_test_products.sql
INSERT INTO "ob-poc".products (product_id, name, product_code, is_active)
VALUES 
    ('22222222-2222-2222-2222-222222222222', 'Global Custody', 'GLOB_CUST', true),
    ('33333333-3333-3333-3333-333333333333', 'Fund Administration', 'FUND_ADMIN', true)
ON CONFLICT (product_code) DO NOTHING;

-- fixtures/seed_test_lookups.sql
INSERT INTO "ob-poc".roles (id, name, description) VALUES
    (gen_random_uuid(), 'InvestmentManager', 'Manages investments'),
    (gen_random_uuid(), 'BeneficialOwner', 'Ultimate beneficial owner (>25%)'),
    (gen_random_uuid(), 'Director', 'Board member')
ON CONFLICT (name) DO NOTHING;

INSERT INTO "ob-poc".document_types (document_type_id, type_code, type_name) VALUES
    (gen_random_uuid(), 'CERT_OF_INCORP', 'Certificate of Incorporation'),
    (gen_random_uuid(), 'PASSPORT', 'Passport')
ON CONFLICT (type_code) DO NOTHING;

INSERT INTO "ob-poc".jurisdictions (id, iso_code, name) VALUES
    (gen_random_uuid(), 'LU', 'Luxembourg'),
    (gen_random_uuid(), 'GB', 'United Kingdom')
ON CONFLICT (iso_code) DO NOTHING;
```

### 3.2 Test Cases

```rust
// rust/tests/onboarding_harness_tests.rs

use ob_poc::dsl_runtime::test_harness::{OnboardingTestHarness, OnboardingTestInput};
use sqlx::PgPool;
use uuid::uuid;

const TEST_CBU_ID: Uuid = uuid!("11111111-1111-1111-1111-111111111111");

#[sqlx::test(fixtures("seed_test_cbu", "seed_test_products", "seed_test_lookups"))]
async fn test_valid_onboarding_full_pipeline(pool: PgPool) {
    let harness = OnboardingTestHarness::new(pool).await.unwrap();
    
    let dsl = r#"
;; Onboarding: Test Fund → Global Custody

(cbu.ensure 
  :cbu-name "Test Fund Ltd"
  :jurisdiction "LU"
  :as @cbu)

(entity.create-limited-company
  :name "Test ManCo S.à r.l."
  :jurisdiction "LU"
  :as @manco)

(cbu.attach-entity
  :entity-id @manco
  :role "InvestmentManager")

(document.request
  :entity-id @manco
  :document-type "CERT_OF_INCORP")
"#;

    let result = harness.run_test(OnboardingTestInput {
        cbu_id: TEST_CBU_ID,
        product_codes: vec!["GLOB_CUST".to_string()],
        dsl_source: dsl.to_string(),
    }).await.unwrap();
    
    // Assert validation passed
    assert!(result.validation_passed, "Errors: {:?}", result.errors);
    assert!(result.dsl_instance_id.is_some());
    assert_eq!(result.dsl_version, Some(1));
    
    // Assert DB verification passed
    let v = &result.verification;
    assert!(v.all_checks_passed, "Verification failed: {:?}", v);
    assert!(v.request_exists);
    assert_eq!(v.request_state, "validated");
    assert_eq!(v.products_linked, 1);
    assert!(v.dsl_instance_exists);
    assert!(v.dsl_content_matches);
    assert!(v.ast_exists);
    assert!(v.ast_has_expressions);
    assert!(v.ast_has_symbol_table);
    assert!(v.symbol_count >= 2, "Expected at least 2 symbols (@cbu, @manco)");
    
    // Verify specific symbols
    let symbols = harness.verify_symbols(result.request_id, &["cbu", "manco"]).await.unwrap();
    assert!(symbols.all_present, "Missing symbols: {:?}", symbols.missing);
    
    // Performance check
    assert!(result.total_time_ms < 500, "Too slow: {}ms", result.total_time_ms);
}

#[sqlx::test(fixtures("seed_test_cbu", "seed_test_products", "seed_test_lookups"))]
async fn test_invalid_role_error_stored(pool: PgPool) {
    let harness = OnboardingTestHarness::new(pool).await.unwrap();
    
    let dsl = r#"
(cbu.ensure :cbu-name "Test Fund" :as @cbu)
(entity.create-limited-company :name "Test Co" :jurisdiction "GB" :as @co)
(cbu.attach-entity :entity-id @co :role "Investmanager")
"#;
    //                                        ↑ typo

    let result = harness.run_test(OnboardingTestInput {
        cbu_id: TEST_CBU_ID,
        product_codes: vec!["GLOB_CUST".to_string()],
        dsl_source: dsl.to_string(),
    }).await.unwrap();
    
    // Assert validation failed
    assert!(!result.validation_passed);
    assert!(result.dsl_instance_id.is_none());
    
    // Assert error was stored
    let v = &result.verification;
    assert!(v.request_exists);
    assert_eq!(v.request_state, "draft");
    assert!(v.errors_stored);
    assert!(v.error_count > 0);
    
    // Verify error code
    let errors = harness.verify_errors(result.request_id, &["E010"]).await.unwrap();
    assert!(errors.all_present, "Expected E010 error");
    
    // Check suggestion in error
    let error = &result.errors[0];
    assert!(error.message.contains("Investmanager"));
    assert!(error.suggestion.as_ref().unwrap().contains("InvestmentManager"));
}

#[sqlx::test(fixtures("seed_test_cbu", "seed_test_products", "seed_test_lookups"))]
async fn test_missing_required_ownership_percent(pool: PgPool) {
    let harness = OnboardingTestHarness::new(pool).await.unwrap();
    
    let dsl = r#"
(cbu.ensure :cbu-name "Test Fund" :as @cbu)
(entity.create-proper-person :first-name "John" :last-name "Smith" :as @ubo)
(cbu.attach-entity :entity-id @ubo :role "BeneficialOwner")
"#;
    //                                        ↑ missing :ownership-percent

    let result = harness.run_test(OnboardingTestInput {
        cbu_id: TEST_CBU_ID,
        product_codes: vec![],
        dsl_source: dsl.to_string(),
    }).await.unwrap();
    
    assert!(!result.validation_passed);
    
    let errors = harness.verify_errors(result.request_id, &["E003"]).await.unwrap();
    assert!(errors.all_present, "Expected E003 (missing required) error");
    
    assert!(result.errors.iter().any(|e| 
        e.message.contains("ownership-percent")
    ));
}

#[sqlx::test(fixtures("seed_test_cbu", "seed_test_products", "seed_test_lookups"))]
async fn test_undefined_symbol_error(pool: PgPool) {
    let harness = OnboardingTestHarness::new(pool).await.unwrap();
    
    let dsl = r#"
(cbu.ensure :cbu-name "Test Fund" :as @cbu)
(cbu.attach-entity :entity-id @company :role "Director")
"#;
    //                           ↑ @company never defined

    let result = harness.run_test(OnboardingTestInput {
        cbu_id: TEST_CBU_ID,
        product_codes: vec![],
        dsl_source: dsl.to_string(),
    }).await.unwrap();
    
    assert!(!result.validation_passed);
    
    let errors = harness.verify_errors(result.request_id, &["E007"]).await.unwrap();
    assert!(errors.all_present, "Expected E007 (undefined symbol) error");
}

#[sqlx::test(fixtures("seed_test_cbu", "seed_test_products", "seed_test_lookups"))]
async fn test_version_increment_on_resubmit(pool: PgPool) {
    let harness = OnboardingTestHarness::new(pool).await.unwrap();
    
    // First submission
    let dsl_v1 = r#"(cbu.ensure :cbu-name "Test Fund v1" :as @cbu)"#;
    let result1 = harness.run_test(OnboardingTestInput {
        cbu_id: TEST_CBU_ID,
        product_codes: vec![],
        dsl_source: dsl_v1.to_string(),
    }).await.unwrap();
    
    assert!(result1.validation_passed);
    assert_eq!(result1.dsl_version, Some(1));
    
    // Second submission (same request via business_reference)
    // Note: Would need to pass same request_id or lookup by business_reference
    // This tests the versioning mechanism
    let business_ref = format!("onboarding:{}", result1.request_id);
    
    let dsl_v2 = r#"(cbu.ensure :cbu-name "Test Fund v2" :as @cbu)"#;
    let ast_json = serde_json::json!({"expressions": [], "symbol_table": {}});
    
    let save_result = harness.dsl_repo.save_dsl_instance(
        &business_ref,
        "onboarding",
        dsl_v2,
        Some(&ast_json),
        "VALIDATE",
    ).await.unwrap();
    
    assert_eq!(save_result.version, 2);
    
    // Verify both versions exist
    let versions = harness.dsl_repo.get_all_versions(&business_ref).await.unwrap();
    assert_eq!(versions.len(), 2);
    assert_eq!(versions[0].version_number, 1);
    assert_eq!(versions[1].version_number, 2);
}
```

---

## Part 4: CLI Runner

```rust
// rust/src/bin/dsl-test.rs

use clap::Parser;
use ob_poc::dsl_runtime::test_harness::{OnboardingTestHarness, OnboardingTestInput};
use sqlx::PgPool;
use uuid::Uuid;
use std::path::PathBuf;

#[derive(Parser)]
#[command(name = "dsl-test")]
#[command(about = "Test onboarding DSL validation pipeline")]
struct Args {
    /// CBU ID to onboard
    #[arg(long)]
    cbu_id: Uuid,
    
    /// Product codes (comma-separated)
    #[arg(long, value_delimiter = ',')]
    products: Vec<String>,
    
    /// DSL file path
    #[arg(long)]
    dsl_file: PathBuf,
    
    /// Output format
    #[arg(long, default_value = "pretty")]
    format: OutputFormat,
}

#[derive(Clone, Debug, Default)]
enum OutputFormat {
    #[default]
    Pretty,
    Json,
}

impl std::str::FromStr for OutputFormat {
    type Err = String;
    fn from_str(s: &str) -> Result<Self, Self::Err> {
        match s.to_lowercase().as_str() {
            "pretty" => Ok(Self::Pretty),
            "json" => Ok(Self::Json),
            _ => Err(format!("Unknown format: {}", s)),
        }
    }
}

#[tokio::main]
async fn main() -> anyhow::Result<()> {
    let args = Args::parse();
    
    let pool = PgPool::connect(&std::env::var("DATABASE_URL")?).await?;
    let harness = OnboardingTestHarness::new(pool).await?;
    
    let dsl_source = std::fs::read_to_string(&args.dsl_file)?;
    
    let result = harness.run_test(OnboardingTestInput {
        cbu_id: args.cbu_id,
        product_codes: args.products,
        dsl_source,
    }).await?;
    
    match args.format {
        OutputFormat::Pretty => {
            if result.validation_passed {
                println!("✅ Validation passed\n");
                println!("  Request ID:   {}", result.request_id);
                println!("  Instance ID:  {:?}", result.dsl_instance_id);
                println!("  Version:      {:?}", result.dsl_version);
                println!("\n⏱️  Timing:");
                println!("  Parse:    {}ms", result.parse_time_ms);
                println!("  Validate: {}ms", result.validate_time_ms);
                println!("  Persist:  {}ms", result.persist_time_ms);
                println!("  Total:    {}ms", result.total_time_ms);
                println!("\n🔍 Verification:");
                let v = &result.verification;
                println!("  Request exists:      {}", v.request_exists);
                println!("  Products linked:     {}/{}", v.products_linked, v.expected_products);
                println!("  DSL stored:          {}", v.dsl_instance_exists);
                println!("  Content matches:     {}", v.dsl_content_matches);
                println!("  AST stored:          {}", v.ast_exists);
                println!("  Symbols found:       {}", v.symbol_count);
                println!("\n  All checks: {}", if v.all_checks_passed { "✅ PASSED" } else { "❌ FAILED" });
            } else {
                eprintln!("❌ Validation failed ({} errors)\n", result.errors.len());
                for (i, err) in result.errors.iter().enumerate() {
                    eprintln!("  {}. [{}] Line {}:{}", i + 1, err.code, err.line, err.column);
                    eprintln!("     {}", err.message);
                    if let Some(hint) = &err.suggestion {
                        eprintln!("     💡 {}", hint);
                    }
                }
                eprintln!("\n🔍 Verification:");
                let v = &result.verification;
                eprintln!("  Errors stored: {} (count: {})", v.errors_stored, v.error_count);
                std::process::exit(1);
            }
        }
        OutputFormat::Json => {
            println!("{}", serde_json::to_string_pretty(&result)?);
            if !result.validation_passed {
                std::process::exit(1);
            }
        }
    }
    
    Ok(())
}
```

**Usage:**

```bash
# Test valid DSL
cargo run --bin dsl-test -- \
  --cbu-id 11111111-1111-1111-1111-111111111111 \
  --products GLOB_CUST \
  --dsl-file examples/onboarding.dsl

# JSON output for CI
cargo run --bin dsl-test -- \
  --cbu-id 11111111-... \
  --products GLOB_CUST,FUND_ADMIN \
  --dsl-file session.dsl \
  --format json
```

---

## Summary

| Component | Source |
|-----------|--------|
| Create onboarding | `TaxonomyCrudOperations::create_onboarding()` |
| Link products | `TaxonomyCrudOperations::add_products_to_onboarding()` |
| Save DSL + AST | `DslRepository::save_dsl_instance()` |
| Load for verify | `DslRepository::load_dsl()`, `load_ast()` |
| Validation errors | `onboarding_requests.validation_errors` |
| Versioning | Automatic via `DslRepository` |

**Key verification queries:**
- Request exists with correct state
- Products linked count matches
- DSL content matches input
- AST has `expressions` and `symbol_table`
- Error count matches on failures
