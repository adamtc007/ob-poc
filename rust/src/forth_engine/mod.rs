//! Public facade for the DSL Forth Engine.

use crate::forth_engine::env::RuntimeEnv;
use crate::forth_engine::kyc_vocab::kyc_orch_vocab;
use crate::forth_engine::parser_nom::NomKycParser;
use crate::forth_engine::vm::VM;
use std::sync::Arc;

#[cfg(feature = "database")]
use crate::database::DslRepository;
#[cfg(feature = "database")]
use sqlx::PgPool;

// Module declarations
pub mod ast;
pub mod compiler;
pub mod ebnf;
pub mod env;
pub mod errors;
pub mod kyc_vocab;
pub mod parser_nom;
pub mod value;
pub mod vm;
pub mod vocab;

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
    let (mut result, env) = execute_sheet_internal_with_env(sheet, Some(pool.clone()))?;

    let parse_time_ms = start_time.elapsed().as_millis() as u64;

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
        let client_name = env
            .attribute_cache
            .iter()
            .find(|(k, _)| k.0.contains("client-name"))
            .map(|(_, v)| match v {
                Value::Str(s) => s.clone(),
                _ => case_id.to_string(),
            })
            .unwrap_or_else(|| case_id.to_string());

        let case_type = env
            .attribute_cache
            .iter()
            .find(|(k, _)| k.0.contains("case-type"))
            .map(|(_, v)| match v {
                Value::Str(s) => s.clone(),
                _ => "ONBOARDING".to_string(),
            })
            .unwrap_or_else(|| "ONBOARDING".to_string());

        // Build attributes map for transactional save
        let mut attributes = std::collections::HashMap::new();
        for (attr_id, value) in &env.attribute_cache {
            let (value_text, value_type) = match value {
                Value::Str(s) => (s.clone(), "STRING".to_string()),
                Value::Int(i) => (i.to_string(), "INTEGER".to_string()),
                Value::Bool(b) => (b.to_string(), "BOOLEAN".to_string()),
                Value::Keyword(k) => (k.clone(), "KEYWORD".to_string()),
                _ => continue,
            };
            attributes.insert(attr_id.0.clone(), (value_text, value_type));
        }

        // Use DslRepository for fully transactional save
        // All operations (DSL, AST, CBU, attributes) are atomic
        let repo = DslRepository::new(pool.clone());
        let save_result = repo
            .save_execution_transactionally(
                case_id,
                &sheet.content,
                &ast_json,
                domain,
                operation_type,
                parse_time_ms as i64,
                &client_name,
                &case_type,
                &attributes,
            )
            .await
            .map_err(|e| EngineError::Database(format!("Failed to save execution: {}", e)))?;

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
    let parser = NomKycParser::new();
    let ast = parser.parse(&sheet.content)?;

    // 2. Compiling (AST -> Bytecode)
    let vocab = kyc_orch_vocab();
    let program = compiler::compile_sheet(&ast, &vocab)?;
    let program_arc = Arc::new(program);

    // 3. Create runtime environment
    #[cfg(feature = "database")]
    let mut env = if let Some(p) = pool {
        RuntimeEnv::with_pool(env::OnboardingRequestId(sheet.id.clone()), p)
    } else {
        RuntimeEnv::new(env::OnboardingRequestId(sheet.id.clone()))
    };

    #[cfg(not(feature = "database"))]
    let mut env = RuntimeEnv::new(env::OnboardingRequestId(sheet.id.clone()));

    // 4. VM Execution (Bytecode)
    let mut vm = VM::new(program_arc, Arc::new(vocab), &mut env);

    let mut logs = Vec::new();
    loop {
        match vm.step_with_logging() {
            Ok(Some(log_msg)) => {
                logs.push(log_msg);
            }
            Ok(None) => {
                // End of program
                break;
            }
            Err(e) => {
                return Err(EngineError::Vm(e));
            }
        }
    }

    // Extract case_id from the execution
    let case_id = vm.env.get_case_id().cloned();

    // Clone the environment for return (need to transfer ownership)
    let final_env = std::mem::replace(
        &mut env,
        RuntimeEnv::new(env::OnboardingRequestId(String::new())),
    );

    Ok((
        ExecutionResult {
            logs,
            case_id,
            success: true,
            version: 0, // Will be set by execute_sheet_with_db after DB query
        },
        final_env,
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
    fn test_stack_effect_validation() {
        // Test that stack underflow is caught at compile time
        let sheet = DslSheet {
            id: "test-stack".to_string(),
            domain: "case".to_string(),
            version: "1".to_string(),
            // case.create expects 4 items (2 pairs), but we only provide 2
            content: r#"(case.create :case-id "UNDER-001")"#.to_string(),
        };

        let result = execute_sheet(&sheet);
        // Should fail due to stack underflow
        assert!(result.is_err());
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
}
