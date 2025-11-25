//! Public facade for the DSL Forth Engine.
//!
//! This module provides the main entry points for DSL execution using
//! direct AST interpretation (no stack machine).

use crate::forth_engine::env::RuntimeEnv;
use crate::forth_engine::parser_nom::NomDslParser;
use crate::forth_engine::vocab_registry::create_standard_runtime;

#[cfg(feature = "database")]
use crate::cbu_model_dsl::CbuModelService;
#[cfg(feature = "database")]
use crate::database::{CrudExecutor, DslRepository};
#[cfg(feature = "database")]
use sqlx::PgPool;

// Module declarations
pub mod ast;
pub mod cbu_model_parser;
pub mod ebnf;
pub mod env;
pub mod errors;
pub mod parser_nom;
pub mod runtime;
pub mod value;
pub mod vocab_registry;
pub mod words;

// Re-export key types
pub use ast::{DslParser, DslSheet, Expr};
pub use env::{generate_onboarding_template, mint_ob_request_id};
pub use errors::EngineError;
pub use value::Value;

/// Result of DSL execution
#[derive(Debug)]
pub struct ExecutionResult {
    /// Execution logs
    pub logs: Vec<String>,
    /// Extracted case_id from DSL
    pub case_id: Option<String>,
    /// Whether execution succeeded
    pub success: bool,
    /// Version number (sequence) for this DSL instance
    pub version: i32,
}

/// Executes a DSL Sheet without database connection.
/// This is the main entry point for the Forth-style engine.
pub fn execute_sheet(sheet: &DslSheet) -> Result<Vec<String>, EngineError> {
    let result = execute_sheet_internal(sheet, None)?;
    Ok(result.logs)
}

/// Executes a DSL Sheet with database connection.
/// This is async because it persists results to the database.
/// Uses DslRepository for fully transactional saves - all operations
/// (DSL, AST, CBU, attributes) are saved atomically.
#[cfg(feature = "database")]
pub async fn execute_sheet_with_db(
    sheet: &DslSheet,
    pool: PgPool,
) -> Result<ExecutionResult, EngineError> {
    let start_time = std::time::Instant::now();

    // Execute the DSL synchronously (parse + compile + run)
    let (mut result, mut env) = execute_sheet_internal_with_env(sheet, Some(pool.clone()))?;

    // Load CBU Model for validation if this is a CBU-related operation
    if sheet.domain == "cbu" || sheet.content.contains("cbu.") {
        let model_service = CbuModelService::new(pool.clone());

        // Try to load the generic CBU model for validation
        match model_service.load_model_by_id("CBU.GENERIC").await {
            Ok(Some(model)) => {
                tracing::debug!("Loaded CBU.GENERIC model for execution context");
                env.set_cbu_model(model);
            }
            Ok(None) => {
                tracing::debug!("No CBU.GENERIC model found, proceeding without model validation");
            }
            Err(e) => {
                tracing::warn!(
                    "Failed to load CBU model: {}, proceeding without validation",
                    e
                );
            }
        }
    }

    let _parse_time_ms = start_time.elapsed().as_millis() as u64;

    // Execute pending CRUD statements against database with model validation
    let pending_crud = env.take_pending_crud();
    if !pending_crud.is_empty() {
        let executor = CrudExecutor::new(pool.clone());

        // Execute with environment for state validation
        let crud_results = executor
            .execute_all_with_env(&pending_crud, &mut env)
            .await
            .map_err(|e| EngineError::Database(format!("CRUD execution failed: {}", e)))?;

        // Log CRUD results
        for crud_result in &crud_results {
            result.logs.push(format!(
                "CRUD {}: {} - {} rows affected",
                crud_result.operation, crud_result.asset, crud_result.rows_affected
            ));

            // If a CBU was created, use its ID as the case_id
            if crud_result.asset == "CBU" && crud_result.operation == "CREATE" {
                if let Some(id) = &crud_result.generated_id {
                    if result.case_id.is_none() {
                        result.case_id = Some(id.to_string());
                        env.set_case_id(id.to_string());
                    }
                }
            }
        }
    }

    // Persist to database using the database facade
    if let Some(case_id) = &result.case_id {
        // Extract domain and operation from DSL content
        let domain = if sheet.content.contains("cbu.") {
            "cbu"
        } else if sheet.content.contains("case.") {
            "case"
        } else if sheet.content.contains("kyc.") {
            "kyc"
        } else if sheet.content.contains("entity.") {
            "entity"
        } else if sheet.content.contains("crud.") {
            "crud"
        } else if sheet.content.contains("attr.") {
            "attr"
        } else if sheet.content.contains("document.") {
            "document"
        } else {
            "general"
        };

        let operation_type = sheet
            .content
            .split_whitespace()
            .next()
            .and_then(|s| s.strip_prefix('('))
            .unwrap_or("unknown");

        // Build AST JSON
        let ast_json = serde_json::json!({
            "sheet_id": sheet.id,
            "domain": sheet.domain,
            "version": sheet.version,
            "logs": result.logs,
            "attributes": env.attribute_cache.iter()
                .map(|(k, v)| (k.0.clone(), format!("{:?}", v)))
                .collect::<std::collections::HashMap<_, _>>()
        })
        .to_string();

        // Extract client name and type for CBU
        let _client_name = env
            .attribute_cache
            .iter()
            .find(|(k, _)| k.0.contains("client-name"))
            .map(|(_, v)| match v {
                Value::Str(s) => s.clone(),
                _ => case_id.to_string(),
            })
            .unwrap_or_else(|| case_id.to_string());

        let _case_type = env
            .attribute_cache
            .iter()
            .find(|(k, _)| k.0.contains("case-type"))
            .map(|(_, v)| match v {
                Value::Str(s) => s.clone(),
                _ => "ONBOARDING".to_string(),
            })
            .unwrap_or_else(|| "ONBOARDING".to_string());

        // Save DSL instance and version via DslRepository
        // Note: Attributes are now handled by CrudExecutor via AttributeValuesService
        let repo = DslRepository::new(pool.clone());
        let ast_value: serde_json::Value = serde_json::from_str(&ast_json)
            .unwrap_or_else(|_| serde_json::json!({"raw": ast_json}));

        let save_result = repo
            .save_dsl_instance(
                case_id,          // business_reference
                domain,           // domain_name
                &sheet.content,   // dsl_content
                Some(&ast_value), // ast_json
                operation_type,   // operation_type
            )
            .await
            .map_err(|e| EngineError::Database(format!("Failed to save DSL instance: {}", e)))?;

        // Set version in result
        result.version = save_result.version;
    }

    Ok(result)
}

/// Create a new OB (Onboarding) Request
/// This mints a new OB Request ID, generates the DSL template, parses it,
/// and saves both DSL and AST to the database with version 1.
#[cfg(feature = "database")]
pub async fn create_ob_request(
    pool: PgPool,
    client_name: &str,
    client_type: &str,
) -> Result<(String, ExecutionResult), EngineError> {
    // 1. Mint new OB Request ID
    let ob_request_id = env::mint_ob_request_id();

    // 2. Generate DSL onboarding template
    let dsl_content = env::generate_onboarding_template(&ob_request_id, client_name, client_type);

    // 3. Create sheet and execute (parse + save)
    let sheet = DslSheet {
        id: ob_request_id.clone(),
        domain: "onboarding".to_string(),
        version: "1".to_string(),
        content: dsl_content,
    };

    // 4. Execute - this will parse, validate, and save to DB
    let result = execute_sheet_with_db(&sheet, pool).await?;

    Ok((ob_request_id, result))
}

/// Execute a DSL sheet and return both result and RuntimeEnv with pending_crud.
/// This is the entry point for tests that want to:
/// 1. Run Forth to populate pending_crud
/// 2. Manually call CrudExecutor.execute_all
///
/// Does NOT automatically execute CRUD against DB - caller must do that.
#[cfg(feature = "database")]
pub fn execute_sheet_into_env(
    sheet: &DslSheet,
    pool: Option<PgPool>,
) -> Result<(ExecutionResult, RuntimeEnv), EngineError> {
    execute_sheet_internal_with_env(sheet, pool)
}

/// Internal execution function
fn execute_sheet_internal(
    sheet: &DslSheet,
    #[cfg(feature = "database")] pool: Option<PgPool>,
    #[cfg(not(feature = "database"))] _pool: Option<()>,
) -> Result<ExecutionResult, EngineError> {
    #[cfg(feature = "database")]
    {
        let (result, _env) = execute_sheet_internal_with_env(sheet, pool)?;
        Ok(result)
    }

    #[cfg(not(feature = "database"))]
    {
        let (result, _env) = execute_sheet_internal_with_env(sheet, _pool)?;
        Ok(result)
    }
}

/// Internal execution function that returns both result and environment
fn execute_sheet_internal_with_env(
    sheet: &DslSheet,
    #[cfg(feature = "database")] pool: Option<PgPool>,
    #[cfg(not(feature = "database"))] _pool: Option<()>,
) -> Result<(ExecutionResult, RuntimeEnv), EngineError> {
    // 1. Parsing (sheet.content -> AST)
    let parser = NomDslParser::new();
    let ast = parser.parse(&sheet.content)?;

    // 2. Create runtime environment
    #[cfg(feature = "database")]
    let mut env = if let Some(p) = pool {
        RuntimeEnv::with_pool(env::OnboardingRequestId(sheet.id.clone()), p)
    } else {
        RuntimeEnv::new(env::OnboardingRequestId(sheet.id.clone()))
    };

    #[cfg(not(feature = "database"))]
    let mut env = RuntimeEnv::new(env::OnboardingRequestId(sheet.id.clone()));

    // 3. Direct AST execution (no bytecode compilation, no stack machine)
    let runtime = create_standard_runtime();
    runtime.execute_sheet(&ast, &mut env)?;

    // Generate execution logs
    let logs: Vec<String> = ast
        .iter()
        .filter_map(|expr| {
            if let ast::Expr::WordCall { name, args } = expr {
                Some(format!(
                    "[Runtime] Executed: {} with {} args",
                    name,
                    args.len()
                ))
            } else {
                None
            }
        })
        .collect();

    // Extract case_id from the execution
    let case_id = env.get_case_id().cloned();

    Ok((
        ExecutionResult {
            logs,
            case_id,
            success: true,
            version: 0, // Will be set by execute_sheet_with_db after DB query
        },
        env,
    ))
}

/// Extract case_id from DSL content by parsing keyword-value pairs
pub fn extract_case_id(dsl_content: &str) -> Option<String> {
    // Simple extraction: find :case-id followed by a string
    if let Some(start) = dsl_content.find(":case-id") {
        let after_keyword = &dsl_content[start + 8..];
        // Skip whitespace and find the string value
        let trimmed = after_keyword.trim_start();
        if let Some(stripped) = trimmed.strip_prefix('"') {
            if let Some(end) = stripped.find('"') {
                return Some(stripped[..end].to_string());
            }
        }
    }
    None
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_execute_sheet_case_create() {
        let sheet = DslSheet {
            id: "test-1".to_string(),
            domain: "case".to_string(),
            version: "1".to_string(),
            content: r#"(case.create :case-id "TEST-001" :case-type "ONBOARDING")"#.to_string(),
        };

        let result = execute_sheet(&sheet);
        assert!(result.is_ok());
        let logs = result.unwrap();
        assert!(!logs.is_empty());
    }

    #[test]
    fn test_execute_sheet_multiple_operations() {
        let sheet = DslSheet {
            id: "test-2".to_string(),
            domain: "case".to_string(),
            version: "1".to_string(),
            content: r#"
                (case.create :case-id "MULTI-001" :case-type "ONBOARDING")
                (kyc.start :entity-id "ENT-001")
            "#
            .to_string(),
        };

        let result = execute_sheet(&sheet);
        assert!(result.is_ok());
    }

    #[test]
    fn test_execute_sheet_entity_operations() {
        let sheet = DslSheet {
            id: "test-3".to_string(),
            domain: "entity".to_string(),
            version: "1".to_string(),
            content: r#"(entity.register :entity-id "ENT-001" :entity-type "CORP")"#.to_string(),
        };

        let result = execute_sheet(&sheet);
        assert!(result.is_ok());
    }

    #[test]
    fn test_execute_sheet_unknown_verb() {
        let sheet = DslSheet {
            id: "test-4".to_string(),
            domain: "case".to_string(),
            version: "1".to_string(),
            content: r#"(unknown.verb :key "value")"#.to_string(),
        };

        let result = execute_sheet(&sheet);
        assert!(result.is_err());
    }

    #[test]
    fn test_extract_case_id() {
        let dsl = r#"(case.create :case-id "EXTRACT-001" :case-type "ONBOARDING")"#;
        let case_id = extract_case_id(dsl);
        assert_eq!(case_id, Some("EXTRACT-001".to_string()));
    }

    #[test]
    fn test_extract_case_id_with_whitespace() {
        let dsl = r#"(case.create :case-id   "SPACE-001"   :case-type "ONBOARDING")"#;
        let case_id = extract_case_id(dsl);
        assert_eq!(case_id, Some("SPACE-001".to_string()));
    }

    #[test]
    fn test_extract_case_id_not_found() {
        let dsl = r#"(entity.register :entity-id "ENT-001")"#;
        let case_id = extract_case_id(dsl);
        assert_eq!(case_id, None);
    }

    #[test]
    fn test_execute_sheet_kyc_operations() {
        let sheet = DslSheet {
            id: "test-kyc".to_string(),
            domain: "kyc".to_string(),
            version: "1".to_string(),
            content: r#"(kyc.collect :case-id "KYC-001" :collection-type "ENHANCED")"#.to_string(),
        };

        let result = execute_sheet(&sheet);
        assert!(result.is_ok());
    }

    #[test]
    fn test_execute_sheet_ubo_operations() {
        let sheet = DslSheet {
            id: "test-ubo".to_string(),
            domain: "ubo".to_string(),
            version: "1".to_string(),
            content: r#"(ubo.collect-entity-data :entity-id "UBO-ENT-001")"#.to_string(),
        };

        let result = execute_sheet(&sheet);
        assert!(result.is_ok());
    }

    #[test]
    fn test_execute_sheet_document_operations() {
        let sheet = DslSheet {
            id: "test-doc".to_string(),
            domain: "document".to_string(),
            version: "1".to_string(),
            content: r#"(document.catalog :doc-id "DOC-001" :doc-type "PASSPORT")"#.to_string(),
        };

        let result = execute_sheet(&sheet);
        assert!(result.is_ok());
    }

    #[test]
    fn test_execute_sheet_products_operations() {
        let sheet = DslSheet {
            id: "test-prod".to_string(),
            domain: "products".to_string(),
            version: "1".to_string(),
            content: r#"(products.add :case-id "PROD-001" :product-type "CUSTODY")"#.to_string(),
        };

        let result = execute_sheet(&sheet);
        assert!(result.is_ok());
    }

    #[test]
    fn test_unknown_word_error() {
        // Test that unknown words are caught at runtime
        let sheet = DslSheet {
            id: "test-unknown".to_string(),
            domain: "case".to_string(),
            version: "1".to_string(),
            content: r#"(nonexistent.verb :key "value")"#.to_string(),
        };

        let result = execute_sheet(&sheet);
        // Should fail due to unknown word
        assert!(result.is_err());
    }

    #[test]
    fn test_partial_args_succeeds() {
        // With direct AST runtime, partial arguments are allowed
        // (words handle missing optional args gracefully)
        let sheet = DslSheet {
            id: "test-partial".to_string(),
            domain: "case".to_string(),
            version: "1".to_string(),
            content: r#"(case.create :case-id "PARTIAL-001")"#.to_string(),
        };

        let result = execute_sheet(&sheet);
        // Should succeed - partial args are valid
        assert!(result.is_ok());
    }

    #[test]
    fn test_execute_sheet_cbu_create() {
        let sheet = DslSheet {
            id: "test-cbu-create".to_string(),
            domain: "cbu".to_string(),
            version: "1".to_string(),
            content: r#"(cbu.create :cbu-name "ACME Corp" :client-type "CORP" :jurisdiction "US")"#
                .to_string(),
        };

        let result = execute_sheet(&sheet);
        assert!(result.is_ok());
    }

    #[test]
    fn test_execute_sheet_cbu_operations() {
        let sheet = DslSheet {
            id: "test-cbu-ops".to_string(),
            domain: "cbu".to_string(),
            version: "1".to_string(),
            content: r#"
                (cbu.create :cbu-name "Test Fund" :client-type "FUND" :jurisdiction "GB")
                (cbu.attach-entity :entity-id "ENT-001" :role "BENEFICIAL_OWNER")
            "#
            .to_string(),
        };

        let result = execute_sheet(&sheet);
        assert!(result.is_ok());
    }

    #[test]
    fn test_execute_sheet_cbu_read() {
        let sheet = DslSheet {
            id: "test-cbu-read".to_string(),
            domain: "cbu".to_string(),
            version: "1".to_string(),
            content: r#"(cbu.read :cbu-id "CBU-001")"#.to_string(),
        };

        let result = execute_sheet(&sheet);
        assert!(result.is_ok());
    }

    #[test]
    fn test_execute_sheet_cbu_update() {
        let sheet = DslSheet {
            id: "test-cbu-update".to_string(),
            domain: "cbu".to_string(),
            version: "1".to_string(),
            content: r#"(cbu.update :cbu-id "CBU-001" :status "ACTIVE")"#.to_string(),
        };

        let result = execute_sheet(&sheet);
        assert!(result.is_ok());
    }

    #[test]
    fn test_execute_sheet_cbu_delete() {
        let sheet = DslSheet {
            id: "test-cbu-delete".to_string(),
            domain: "cbu".to_string(),
            version: "1".to_string(),
            content: r#"(cbu.delete :cbu-id "CBU-001")"#.to_string(),
        };

        let result = execute_sheet(&sheet);
        assert!(result.is_ok());
    }

    #[test]
    fn test_execute_sheet_crud_operations() {
        let sheet = DslSheet {
            id: "test-crud".to_string(),
            domain: "crud".to_string(),
            version: "1".to_string(),
            content: r#"
                (crud.begin :operation-type "CREATE" :asset-type "CBU")
                (crud.commit :entity-table "cbus" :ai-instruction "Create test CBU" :ai-provider "OPENAI")
            "#.to_string(),
        };

        let result = execute_sheet(&sheet);
        assert!(result.is_ok());
    }

    #[test]
    fn test_execute_sheet_attr_operations() {
        let sheet = DslSheet {
            id: "test-attr".to_string(),
            domain: "attr".to_string(),
            version: "1".to_string(),
            content: r#"(attr.set :attr-id "KYC.LEI" :value "5493001KJTIIGC8Y1R12")"#.to_string(),
        };

        let result = execute_sheet(&sheet);
        assert!(result.is_ok());
    }

    #[test]
    fn test_execute_sheet_document_extended() {
        let sheet = DslSheet {
            id: "test-doc-ext".to_string(),
            domain: "document".to_string(),
            version: "1".to_string(),
            content: r#"(document.extract-attributes :document-id "DOC-001" :document-type "UK-PASSPORT")"#.to_string(),
        };

        let result = execute_sheet(&sheet);
        assert!(result.is_ok());
    }

    #[test]
    fn test_execute_sheet_full_onboarding_flow() {
        let sheet = DslSheet {
            id: "test-full-flow".to_string(),
            domain: "onboarding".to_string(),
            version: "1".to_string(),
            content: r#"
                (cbu.create :cbu-name "Full Flow Corp" :client-type "CORP" :jurisdiction "US")
                (entity.register :entity-id "ENT-001" :entity-type "PROPER_PERSON")
                (cbu.attach-entity :entity-id "ENT-001" :role "BENEFICIAL_OWNER")
                (document.catalog :doc-id "PASS-001" :doc-type "UK-PASSPORT")
                (document.extract-attributes :document-id "PASS-001" :document-type "UK-PASSPORT")
            "#
            .to_string(),
        };

        let result = execute_sheet(&sheet);
        assert!(result.is_ok());
    }

    #[test]
    fn test_cbu_create_emits_crud_statement() {
        use crate::forth_engine::value::CrudStatement;

        let sheet = DslSheet {
            id: "test-crud".to_string(),
            domain: "cbu".to_string(),
            version: "1".to_string(),
            content: r#"(cbu.create :cbu-name "Test Corp" :client-type "CORP" :jurisdiction "US")"#
                .to_string(),
        };

        // Execute using internal function that returns env
        #[cfg(feature = "database")]
        let (_, env) = execute_sheet_internal_with_env(&sheet, None).unwrap();
        #[cfg(not(feature = "database"))]
        let (_, env) = execute_sheet_internal_with_env(&sheet, None).unwrap();

        // Should have one pending CRUD statement
        assert_eq!(env.pending_crud.len(), 1);
        match &env.pending_crud[0] {
            CrudStatement::DataCreate(create) => {
                assert_eq!(create.asset, "CBU");
                assert!(create.values.contains_key("cbu-name"));
                assert!(create.values.contains_key("client-type"));
                assert!(create.values.contains_key("jurisdiction"));
            }
            _ => panic!("Expected DataCreate statement"),
        }
    }

    #[test]
    fn test_cbu_model_state_transitions() {
        use crate::forth_engine::cbu_model_parser::CbuModelParser;
        use crate::forth_engine::env::OnboardingRequestId;

        let model_dsl = r#"
        (cbu-model
          :id "CBU.TEST"
          :version "1.0"
          (attributes
            (group :name "core" :required [@attr("LEGAL_NAME")]))
          (states
            :initial "Proposed"
            :final ["Closed"]
            (state "Proposed" :description "Initial")
            (state "Active" :description "Active")
            (state "Closed" :description "Closed"))
          (transitions
            (-> "Proposed" "Active" :verb "cbu.approve" :preconditions [])
            (-> "Active" "Closed" :verb "cbu.close" :preconditions []))
          (roles
            (role "Owner" :min 1)))
        "#;

        let model = CbuModelParser::parse_str(model_dsl).unwrap();

        let mut env = RuntimeEnv::new(OnboardingRequestId("TEST".to_string()));
        env.set_cbu_model(model);

        // Initial state should be "Proposed"
        assert_eq!(env.get_cbu_state(), Some("Proposed"));

        // Valid transition: Proposed -> Active
        assert!(env.is_valid_transition("Active"));

        // Invalid transition: Proposed -> Closed (not defined)
        assert!(!env.is_valid_transition("Closed"));

        // After transitioning to Active
        env.set_cbu_state("Active".to_string());
        assert!(env.is_valid_transition("Closed"));
    }

    #[test]
    fn test_multiple_cbu_operations_emit_crud() {
        use crate::forth_engine::value::CrudStatement;

        let sheet = DslSheet {
            id: "test-multi-crud".to_string(),
            domain: "cbu".to_string(),
            version: "1".to_string(),
            content: r#"
                (cbu.create :cbu-name "Multi Corp" :client-type "FUND" :jurisdiction "GB")
                (cbu.attach-entity :entity-id "ENT-001" :role "OWNER")
                (cbu.finalize :cbu-id "CBU-001" :status "ACTIVE")
            "#
            .to_string(),
        };

        // Execute using internal function that returns env
        #[cfg(feature = "database")]
        let (_, env) = execute_sheet_internal_with_env(&sheet, None).unwrap();
        #[cfg(not(feature = "database"))]
        let (_, env) = execute_sheet_internal_with_env(&sheet, None).unwrap();

        // Should have 3 pending CRUD statements
        assert_eq!(env.pending_crud.len(), 3);

        // First: DataCreate for CBU
        assert!(matches!(&env.pending_crud[0], CrudStatement::DataCreate(c) if c.asset == "CBU"));

        // Second: DataCreate for CBU-entity role attachment
        assert!(
            matches!(&env.pending_crud[1], CrudStatement::DataCreate(c) if c.asset == "CBU_ENTITY_ROLE")
        );

        // Third: DataUpdate for finalize
        assert!(matches!(&env.pending_crud[2], CrudStatement::DataUpdate(u) if u.asset == "CBU"));
    }
}
