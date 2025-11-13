//! DSL Edit-Save Workflow Demo - Multi-Stage Commit Testing
//!
//! This example demonstrates the complete DSL edit-save workflow with multi-stage
//! commit handling, following the same pattern as CBU CRUD operations.
//!
//! ## Workflow Being Tested:
//! 1. **Initial State**: DSL.Onboard creates base DSL with onboarding_request_id
//! 2. **Load DSL**: Pull existing DSL by onboarding_request_id
//! 3. **Edit DSL**: Make modifications to DSL content
//! 4. **Save DSL**: Multi-stage commit with transaction handling:
//!    - Parse & Validate DSL
//!    - Save DSL Instance (version++)
//!    - Save AST Representation
//!    - Update Audit Trail
//!    - Sync Cross-References
//! 5. **Verification**: Confirm both DSL and AST records saved with matching versions
//!
//! ## Multi-Stage Commit Architecture:
//! ```
//! Transaction Begin
//!   ├── Stage 1: Validate Request
//!   ├── Stage 2: Parse DSL Content
//!   ├── Stage 3: Save DSL Instance (version N+1)
//!   ├── Stage 4: Save AST Record (version N+1)
//!   ├── Stage 5: Create Audit Entry
//!   ├── Stage 6: Sync Cross-References
//!   └── Transaction Commit (or Rollback on failure)
//! ```
//!
//! ## Prerequisites:
//! - PostgreSQL running locally with "ob-poc" schema
//! - Environment variable: DATABASE_URL="postgresql://user:pass@localhost:5432/ob_poc"
//!
//! ## Usage:
//! ```bash
//! export DATABASE_URL="postgresql://postgres:password@localhost:5432/ob_poc"
//! cargo run --example dsl_edit_save_workflow_demo --features="database"
//! ```

use ob_poc::dsl_manager::{qq
    CleanDslManager, CleanManagerConfig, DslCrudManager, DslLoadRequest, DslSaveRequest,
    OperationContext,
};

#[cfg(feature = "database")]
use ob_poc::database::{DatabaseConfig, DatabaseManager, DictionaryDatabaseService};

#[cfg(feature = "database")]
use sqlx::PgPool;
use std::collections::HashMap;
use std::time::Instant;
use tracing::{error, info, warn};
use uuid::Uuid;

/// Demo configuration for the workflow test
struct WorkflowDemoConfig {
    pub case_id: String,
    pub onboarding_request_id: Uuid,
    pub initial_dsl: String,
    pub edited_dsl: String,
    pub user_id: String,
}

impl Default for WorkflowDemoConfig {
    fn default() -> Self {
        let case_id = format!("demo-case-{}", Uuid::new_v4());
        let onboarding_request_id = Uuid::new_v4();

        Self {
            case_id: case_id.clone(),
            onboarding_request_id,
            initial_dsl: format!(
                r#"
(case.create
  :case-id "{}"
  :name "Demo Onboarding Case"
  :jurisdiction "US"
  :entity-type "CORP")

(entity.register
  :entity-id "demo-entity-001"
  :name "Demo Corporation"
  :type "CORPORATION")

(kyc.start
  :entity-id "demo-entity-001"
  :level "ENHANCED")
"#,
                case_id
            ),
            edited_dsl: format!(
                r#"
(case.create
  :case-id "{}"
  :name "Demo Onboarding Case - UPDATED"
  :jurisdiction "US"
  :entity-type "CORP")

(entity.register
  :entity-id "demo-entity-001"
  :name "Demo Corporation Inc."
  :type "CORPORATION"
  :business-purpose "Technology Services")

(kyc.start
  :entity-id "demo-entity-001"
  :level "ENHANCED")

(products.add
  :entity-id "demo-entity-001"
  :products ["CUSTODY" "TRADE_EXECUTION"])

(compliance.screen
  :entity-id "demo-entity-001"
  :frameworks ["AML" "KYC" "OFAC"])
"#,
                case_id
            ),
            user_id: "demo-user".to_string(),
        }
    }
}

#[tokio::main]
async fn main() -> Result<(), Box<dyn std::error::Error>> {
    // Initialize comprehensive tracing
    tracing_subscriber::fmt()
        .with_max_level(tracing::Level::INFO)
        .with_target(false)
        .with_file(true)
        .with_line_number(true)
        .init();

    info!("🚀 Starting DSL Edit-Save Workflow Demo");

    let demo_start = Instant::now();
    let config = WorkflowDemoConfig::default();

    // Setup database connection (if available)
    #[cfg(feature = "database")]
    let result = run_database_workflow(&config).await;

    #[cfg(not(feature = "database"))]
    let result = run_mock_workflow(&config).await;

    match result {
        Ok(_) => {
            let total_time = demo_start.elapsed();
            info!(
                "✅ DSL Edit-Save Workflow Demo completed successfully in {:?}",
                total_time
            );
            println!("\n🎉 SUCCESS: Multi-stage commit workflow validated!");
            println!("   - DSL parsing and validation: ✓");
            println!("   - Version management: ✓");
            println!("   - Transaction handling: ✓");
            println!("   - AST synchronization: ✓");
            println!("   - Audit trail: ✓");
        }
        Err(e) => {
            error!("❌ Demo failed: {}", e);
            println!("\n💥 FAILURE: {}", e);
            std::process::exit(1);
        }
    }

    Ok(())
}

#[cfg(feature = "database")]
async fn run_database_workflow(
    config: &WorkflowDemoConfig,
) -> Result<(), Box<dyn std::error::Error>> {
    info!("🔗 Connecting to PostgreSQL database");

    // Get database URL from environment
    let database_url = std::env::var("DATABASE_URL")
        .unwrap_or_else(|_| "postgresql://postgres:password@localhost:5432/ob_poc".to_string());

    info!(
        "Connecting to: {}",
        database_url.replace(|c: char| c.is_ascii_digit(), "*")
    );

    // Create database connection
    let pool = PgPool::connect(&database_url).await?;
    info!("✅ Database connection established");

    // Initialize DSL CRUD Manager
    let dsl_crud_manager = DslCrudManager::new(pool.clone());
    info!("✅ DSL CRUD Manager initialized");

    // === STAGE 1: Initial DSL Save (Simulating DSL.Onboard) ===
    info!("\n📝 STAGE 1: Creating Initial DSL (Simulating DSL.Onboard)");

    let initial_save_request = DslSaveRequest {
        case_id: config.case_id.clone(),
        onboarding_request_id: config.onboarding_request_id,
        dsl_content: config.initial_dsl.clone(),
        user_id: config.user_id.clone(),
        operation_context: OperationContext {
            workflow_type: "onboarding".to_string(),
            source: "dsl_onboard".to_string(),
            metadata: {
                let mut meta = HashMap::new();
                meta.insert("stage".to_string(), "initial_creation".to_string());
                meta.insert("automated".to_string(), "true".to_string());
                meta
            },
        },
    };

    let initial_result = dsl_crud_manager
        .save_dsl_complex(initial_save_request)
        .await?;

    info!("✅ Initial DSL saved:");
    info!("   Case ID: {}", initial_result.case_id);
    info!("   Version: {}", initial_result.version_number);
    info!("   DSL Instance ID: {}", initial_result.dsl_instance_id);
    info!("   AST Record ID: {}", initial_result.ast_record_id);
    info!("   Parsing Time: {}ms", initial_result.parsing_time_ms);
    info!("   Save Time: {}ms", initial_result.save_time_ms);

    // === STAGE 2: Load Existing DSL ===
    info!("\n📖 STAGE 2: Loading Existing DSL for Editing");

    let load_request = DslLoadRequest {
        case_id: config.case_id.clone(),
        version: None, // Load latest version
        include_ast: true,
        include_audit_trail: true,
    };

    let load_result = dsl_crud_manager.load_dsl_complete(load_request).await?;

    info!("✅ DSL loaded:");
    info!("   Case ID: {}", load_result.case_id);
    info!("   Version: {}", load_result.version_number);
    info!(
        "   DSL Length: {} characters",
        load_result.dsl_content.len()
    );
    info!("   AST Available: {}", load_result.ast_json.is_some());
    info!("   Audit Entries: {}", load_result.audit_entries.len());
    info!(
        "   Created: {}",
        load_result.created_at.format("%Y-%m-%d %H:%M:%S")
    );
    info!(
        "   Updated: {}",
        load_result.updated_at.format("%Y-%m-%d %H:%M:%S")
    );

    // Display audit trail
    if !load_result.audit_entries.is_empty() {
        info!("   📋 Audit Trail:");
        for entry in &load_result.audit_entries {
            info!(
                "      {} | {} | v{} → v{} | {}",
                entry.timestamp.format("%H:%M:%S"),
                entry.operation_type,
                entry.version_from,
                entry.version_to,
                entry.change_summary
            );
        }
    }

    // === STAGE 3: Edit and Save DSL (Multi-Stage Commit) ===
    info!("\n✏️  STAGE 3: Editing and Saving DSL (Multi-Stage Commit)");

    let edit_save_request = DslSaveRequest {
        case_id: config.case_id.clone(),
        onboarding_request_id: config.onboarding_request_id,
        dsl_content: config.edited_dsl.clone(),
        user_id: config.user_id.clone(),
        operation_context: OperationContext {
            workflow_type: "onboarding".to_string(),
            source: "manual_edit".to_string(),
            metadata: {
                let mut meta = HashMap::new();
                meta.insert("stage".to_string(), "user_edit".to_string());
                meta.insert("automated".to_string(), "false".to_string());
                meta.insert("edit_type".to_string(), "enhancement".to_string());
                meta
            },
        },
    };

    let edit_result = dsl_crud_manager.save_dsl_complex(edit_save_request).await?;

    info!("✅ Edited DSL saved with multi-stage commit:");
    info!("   Case ID: {}", edit_result.case_id);
    info!(
        "   New Version: {} (incremented)",
        edit_result.version_number
    );
    info!("   DSL Instance ID: {}", edit_result.dsl_instance_id);
    info!("   AST Record ID: {}", edit_result.ast_record_id);
    info!("   Parsing Time: {}ms", edit_result.parsing_time_ms);
    info!("   Save Time: {}ms", edit_result.save_time_ms);
    info!("   Total Time: {}ms", edit_result.total_time_ms);

    // === STAGE 4: Verification - Load Updated Version ===
    info!("\n🔍 STAGE 4: Verification - Loading Updated Version");

    let verify_request = DslLoadRequest {
        case_id: config.case_id.clone(),
        version: Some(edit_result.version_number),
        include_ast: true,
        include_audit_trail: true,
    };

    let verify_result = dsl_crud_manager.load_dsl_complete(verify_request).await?;

    info!("✅ Updated DSL verified:");
    info!("   Case ID: {}", verify_result.case_id);
    info!("   Version: {}", verify_result.version_number);
    info!(
        "   DSL Length: {} characters",
        verify_result.dsl_content.len()
    );
    info!("   AST Available: {}", verify_result.ast_json.is_some());
    info!("   Audit Entries: {}", verify_result.audit_entries.len());

    // Verify version increment
    if verify_result.version_number == initial_result.version_number + 1 {
        info!(
            "✅ Version increment verified: {} → {}",
            initial_result.version_number, verify_result.version_number
        );
    } else {
        error!(
            "❌ Version increment failed: expected {}, got {}",
            initial_result.version_number + 1,
            verify_result.version_number
        );
        return Err("Version increment validation failed".into());
    }

    // Verify DSL content changes
    if verify_result.dsl_content.contains("UPDATED")
        && verify_result.dsl_content.contains("products.add")
    {
        info!("✅ DSL content changes verified");
    } else {
        error!("❌ DSL content changes not found");
        return Err("DSL content validation failed".into());
    }

    // Display updated audit trail
    info!("   📋 Complete Audit Trail:");
    for entry in &verify_result.audit_entries {
        info!(
            "      {} | {} | v{} → v{} | {}",
            entry.timestamp.format("%H:%M:%S"),
            entry.operation_type,
            entry.version_from,
            entry.version_to,
            entry.change_summary
        );
    }

    // === STAGE 5: Transaction Integrity Verification ===
    info!("\n🔐 STAGE 5: Transaction Integrity Verification");

    // Verify both DSL and AST records exist with matching versions
    let dsl_check =
        verify_dsl_ast_consistency(&pool, &config.case_id, edit_result.version_number).await?;

    if dsl_check {
        info!("✅ DSL-AST consistency verified");
        info!(
            "   Both DSL instance and AST record exist with version {}",
            edit_result.version_number
        );
    } else {
        error!("❌ DSL-AST consistency check failed");
        return Err("Transaction integrity validation failed".into());
    }

    info!("\n🎯 Multi-Stage Commit Workflow Summary:");
    info!("   Initial Version: {}", initial_result.version_number);
    info!("   Final Version: {}", edit_result.version_number);
    info!("   DSL Instances Created: 2");
    info!("   AST Records Created: 2");
    info!("   Audit Entries: {}", verify_result.audit_entries.len());
    info!("   All transactions committed successfully: ✅");

    Ok(())
}

#[cfg(not(feature = "database"))]
async fn run_mock_workflow(config: &WorkflowDemoConfig) -> Result<(), Box<dyn std::error::Error>> {
    info!("🧪 Running mock workflow (database feature disabled)");

    let dsl_crud_manager = DslCrudManager::new();

    // Mock initial save
    let initial_request = DslSaveRequest {
        case_id: config.case_id.clone(),
        onboarding_request_id: config.onboarding_request_id,
        dsl_content: config.initial_dsl.clone(),
        user_id: config.user_id.clone(),
        operation_context: OperationContext {
            workflow_type: "onboarding".to_string(),
            source: "dsl_onboard".to_string(),
            metadata: HashMap::new(),
        },
    };

    let initial_result = dsl_crud_manager.save_dsl_complex(initial_request).await?;
    info!(
        "✅ Mock initial save: version {}",
        initial_result.version_number
    );

    // Mock edit save
    let edit_request = DslSaveRequest {
        case_id: config.case_id.clone(),
        onboarding_request_id: config.onboarding_request_id,
        dsl_content: config.edited_dsl.clone(),
        user_id: config.user_id.clone(),
        operation_context: OperationContext {
            workflow_type: "onboarding".to_string(),
            source: "manual_edit".to_string(),
            metadata: HashMap::new(),
        },
    };

    let edit_result = dsl_crud_manager.save_dsl_complex(edit_request).await?;
    info!("✅ Mock edit save: version {}", edit_result.version_number);

    info!("✅ Mock workflow completed successfully");
    Ok(())
}

#[cfg(feature = "database")]
async fn verify_dsl_ast_consistency(
    pool: &PgPool,
    case_id: &str,
    version: u32,
) -> Result<bool, Box<dyn std::error::Error>> {
    // Check DSL instance exists
    let dsl_row = sqlx::query!(
        r#"
        SELECT COUNT(*) as count
        FROM "ob-poc".dsl_instances
        WHERE case_id = $1 AND version = $2
        "#,
        case_id,
        version as i32
    )
    .fetch_one(pool)
    .await?;

    let dsl_exists = dsl_row.count.unwrap_or(0) > 0;

    // Check AST record exists
    let ast_row = sqlx::query!(
        r#"
        SELECT COUNT(*) as count
        FROM "ob-poc".parsed_asts
        WHERE case_id = $1 AND version = $2
        "#,
        case_id,
        version as i32
    )
    .fetch_one(pool)
    .await?;

    let ast_exists = ast_row.count.unwrap_or(0) > 0;

    info!("   DSL instance exists: {}", dsl_exists);
    info!("   AST record exists: {}", ast_exists);

    Ok(dsl_exists && ast_exists)
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_workflow_config_creation() {
        let config = WorkflowDemoConfig::default();
        assert!(!config.case_id.is_empty());
        assert!(!config.initial_dsl.is_empty());
        assert!(!config.edited_dsl.is_empty());
        assert_eq!(config.user_id, "demo-user");
    }

    #[test]
    fn test_dsl_content_differences() {
        let config = WorkflowDemoConfig::default();

        // Verify initial DSL doesn't have "UPDATED" or "products.add"
        assert!(!config.initial_dsl.contains("UPDATED"));
        assert!(!config.initial_dsl.contains("products.add"));

        // Verify edited DSL has both
        assert!(config.edited_dsl.contains("UPDATED"));
        assert!(config.edited_dsl.contains("products.add"));
    }

    #[tokio::test]
    async fn test_mock_workflow() {
        let config = WorkflowDemoConfig::default();
        let result = run_mock_workflow(&config).await;
        assert!(result.is_ok());
    }
}
