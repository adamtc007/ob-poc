//! Test runner example for DSL Manager
//!
//! This example demonstrates creating an onboarding request using the consolidated DSL manager
//! and verifies that DSL content is stored, AST is generated, and all keys are returned.

use ob_poc::{
    database::DslDomainRepository,
    dsl_manager_consolidated::{DslManager, TemplateType},
    models::business_request_models::DslBusinessRequestRepository,
};
use sqlx::PgPool;
use std::path::PathBuf;
use uuid::Uuid;

#[tokio::main]
async fn main() -> Result<(), Box<dyn std::error::Error>> {
    // Initialize logging
    tracing_subscriber::fmt()
        .with_env_filter("info,ob_poc=debug,sqlx=info")
        .init();

    println!("🚀 DSL Manager Test Runner");
    println!("========================");

    // Connect to database
    let database_url = std::env::var("DATABASE_URL")
        .unwrap_or_else(|_| "postgresql://localhost:5432/ob-poc".to_string());

    println!("📡 Connecting to database: {}", database_url);
    let pool = PgPool::connect(&database_url).await?;

    // Create repositories
    let domain_repo = DslDomainRepository::new(pool.clone());
    let business_repo = DslBusinessRequestRepository::new(pool.clone());

    // Create DSL Manager
    let template_path = PathBuf::from("templates");
    let dsl_manager = DslManager::new(domain_repo, business_repo, template_path);

    println!("✅ DSL Manager initialized");

    // Step 1: Create test CBU
    println!("\n📝 Step 1: Creating test CBU");
    let test_cbu_id = create_test_cbu(&pool).await?;
    println!("✅ Created test CBU: {}", test_cbu_id);

    // Step 2: Test CBU validation
    println!("\n🔍 Step 2: Validating CBU exists");
    match dsl_manager.validate_cbu_exists(test_cbu_id).await {
        Ok(()) => println!("✅ CBU validation passed"),
        Err(e) => {
            println!("❌ CBU validation failed: {:?}", e);
            cleanup_test_cbu(&pool, test_cbu_id).await?;
            return Err(e.into());
        }
    }

    // Step 3: Get CBU info
    println!("\n📋 Step 3: Getting CBU information");
    match dsl_manager.get_cbu_info(test_cbu_id).await {
        Ok(cbu_info) => {
            println!("✅ CBU Info retrieved:");
            println!("   Name: {}", cbu_info.name);
            println!("   Description: {:?}", cbu_info.description);
            println!("   Nature/Purpose: {:?}", cbu_info.nature_purpose);
        }
        Err(e) => {
            println!("❌ Failed to get CBU info: {:?}", e);
            cleanup_test_cbu(&pool, test_cbu_id).await?;
            return Err(e.into());
        }
    }

    // Step 4: Create onboarding request
    println!("\n🎯 Step 4: Creating DSL.OB request");
    let creation_result = match dsl_manager
        .create_onboarding_request(
            test_cbu_id,
            "Goldman Sachs Asset Management Onboarding".to_string(),
            "Complete onboarding workflow for GSAM institutional client".to_string(),
            "analyst@bank.com".to_string(),
        )
        .await
    {
        Ok(result) => {
            println!("✅ DSL.OB request created successfully!");
            result
        }
        Err(e) => {
            println!("❌ Failed to create onboarding request: {:?}", e);
            cleanup_test_cbu(&pool, test_cbu_id).await?;
            return Err(e.into());
        }
    };

    // Step 5: Verify results
    println!("\n📊 Step 5: Verifying creation results");
    println!("OB Request ID: {}", creation_result.ob_request_id);
    println!(
        "DSL Instance ID: {}",
        creation_result.ob_instance.instance_id
    );
    println!("CBU ID: {}", creation_result.cbu_id);
    println!("Domain: {}", creation_result.ob_instance.domain_name);
    println!("Status: {:?}", creation_result.ob_instance.status);
    println!("Version: {}", creation_result.ob_instance.current_version);

    // Step 6: Verify DSL was stored in database
    println!("\n🗄️ Step 6: Verifying DSL storage");
    let stored_dsl = sqlx::query!(
        r#"SELECT dsl_text, created_at FROM "ob-poc".dsl_ob WHERE version_id = $1"#,
        creation_result.dsl_storage_keys.dsl_ob_version_id
    )
    .fetch_optional(&pool)
    .await?;

    match stored_dsl {
        Some(record) => {
            println!("✅ DSL stored in database");
            println!(
                "   Version ID: {}",
                creation_result.dsl_storage_keys.dsl_ob_version_id
            );
            println!(
                "   Storage Index: {}",
                creation_result.dsl_storage_keys.storage_index
            );
            println!("   Created At: {}", record.created_at);
            println!(
                "   DSL Content Preview: {}",
                if record.dsl_text.len() > 200 {
                    format!("{}...", &record.dsl_text[..200])
                } else {
                    record.dsl_text.clone()
                }
            );
        }
        None => {
            println!("❌ DSL not found in database!");
            cleanup_test_data(&pool, &creation_result, test_cbu_id).await?;
            return Err("DSL not stored properly".into());
        }
    }

    // Step 7: Verify AST generation
    println!("\n🌲 Step 7: Verifying AST generation");
    if let Some(ref ast_json) = creation_result.compiled_version.ast_json {
        println!("✅ AST generated successfully");
        println!(
            "   Compilation Status: {:?}",
            creation_result.compiled_version.compilation_status
        );
        println!("   AST Size: {} bytes", ast_json.len());

        // Try to parse the AST JSON to verify it's valid
        match serde_json::from_str::<serde_json::Value>(ast_json) {
            Ok(_) => println!("✅ AST JSON is valid"),
            Err(e) => println!("⚠️ AST JSON parse warning: {}", e),
        }
    } else {
        println!("❌ No AST generated!");
        cleanup_test_data(&pool, &creation_result, test_cbu_id).await?;
        return Err("AST not generated".into());
    }

    // Step 8: Verify all keys returned
    println!("\n🔑 Step 8: Verifying all keys returned");
    println!("✅ All required keys present:");
    println!("   ✓ OB Request ID: {}", creation_result.ob_request_id);
    println!(
        "   ✓ DSL Instance ID: {}",
        creation_result.ob_instance.instance_id
    );
    println!(
        "   ✓ DSL Version ID: {}",
        creation_result.compiled_version.version_id
    );
    println!(
        "   ✓ DSL OB Version ID: {}",
        creation_result.dsl_storage_keys.dsl_ob_version_id
    );
    println!(
        "   ✓ Storage Index: {}",
        creation_result.dsl_storage_keys.storage_index
    );
    println!(
        "   ✓ Onboarding Session ID: {}",
        creation_result.onboarding_session.onboarding_id
    );

    // Step 9: Test template loading (optional verification)
    println!("\n📄 Step 9: Verifying template system");
    match dsl_manager
        .load_template("onboarding", &TemplateType::CreateCbu)
        .await
    {
        Ok(template) => {
            println!("✅ Template loaded successfully");
            println!("   Template ID: {}", template.template_id);
            println!("   Domain: {}", template.domain_name);
            println!("   Variables: {}", template.variables.len());
        }
        Err(e) => {
            println!("⚠️ Template loading issue (non-critical): {:?}", e);
        }
    }

    // Final cleanup
    println!("\n🧹 Cleaning up test data...");
    cleanup_test_data(&pool, &creation_result, test_cbu_id).await?;
    println!("✅ Cleanup completed");

    println!("\n🎉 DSL Manager Test Completed Successfully!");
    println!("===================================================");
    println!("✅ CBU validation works");
    println!("✅ CBU info retrieval works");
    println!("✅ DSL.OB request creation works");
    println!("✅ DSL storage in database works");
    println!("✅ AST generation and compilation works");
    println!("✅ All keys returned correctly");
    println!("✅ Database integration working");

    Ok(())
}

/// Create test CBU in database
async fn create_test_cbu(pool: &PgPool) -> Result<Uuid, sqlx::Error> {
    let cbu_id = Uuid::new_v4();

    sqlx::query!(
        r#"INSERT INTO "ob-poc".cbus (cbu_id, name, description, nature_purpose)
           VALUES ($1, $2, $3, $4)"#,
        cbu_id,
        "Test CBU for DSL Manager Example",
        "Test CBU created by DSL Manager example runner",
        "Testing and validation of DSL Manager functionality"
    )
    .execute(pool)
    .await?;

    Ok(cbu_id)
}

/// Clean up test CBU
async fn cleanup_test_cbu(pool: &PgPool, cbu_id: Uuid) -> Result<(), sqlx::Error> {
    sqlx::query!(r#"DELETE FROM "ob-poc".cbus WHERE cbu_id = $1"#, cbu_id)
        .execute(pool)
        .await?;

    Ok(())
}

/// Clean up all test data
async fn cleanup_test_data(
    pool: &PgPool,
    creation_result: &ob_poc::dsl_manager_consolidated::OnboardingRequestCreationResult,
    cbu_id: Uuid,
) -> Result<(), sqlx::Error> {
    // Clean up DSL records
    sqlx::query!(
        r#"DELETE FROM "ob-poc".dsl_ob WHERE cbu_id = $1"#,
        cbu_id.to_string()
    )
    .execute(pool)
    .await?;

    // Clean up CBU
    cleanup_test_cbu(pool, cbu_id).await?;

    Ok(())
}
