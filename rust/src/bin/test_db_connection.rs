//! Simple Database Connection Test
//!
//! This binary tests basic database connectivity and verifies that the required tables exist.
//! It's designed to be a simple smoke test for the DSL manager database setup.

use sqlx::PgPool;
use std::env;
use tracing::{error, info};

#[tokio::main]
async fn main() -> Result<(), Box<dyn std::error::Error>> {
    // Initialize logging
    tracing_subscriber::fmt()
        .with_max_level(tracing::Level::INFO)
        .init();

    info!("🔧 Starting Database Connection Test");

    // Get database URL from environment
    let database_url = env::var("DATABASE_URL")
        .unwrap_or_else(|_| "postgresql://localhost:5432/ob-poc".to_string());

    info!("📊 Connecting to database: {}", mask_url(&database_url));

    // Create connection pool
    let pool = PgPool::connect(&database_url).await?;
    info!("✅ Database connection successful!");

    // Test 1: Check if schema exists
    info!("🔍 Testing schema existence...");
    let schema_exists = sqlx::query_scalar!(
        r#"SELECT EXISTS(SELECT 1 FROM information_schema.schemata WHERE schema_name = 'ob-poc')"#
    )
    .fetch_one(&pool)
    .await?;

    if schema_exists == Some(true) {
        info!("✅ Schema 'ob-poc' exists");
    } else {
        error!("❌ Schema 'ob-poc' does not exist");
        return Err("Missing schema".into());
    }

    // Test 2: Check required tables
    let required_tables = vec![
        "dsl_instances",
        "dsl_instance_versions",
        "ast_nodes",
        "dsl_templates",
    ];

    for table_name in &required_tables {
        info!("🔍 Checking table: {}", table_name);
        let table_exists = sqlx::query_scalar!(
            r#"SELECT EXISTS(SELECT 1 FROM information_schema.tables WHERE table_schema = 'ob-poc' AND table_name = $1)"#,
            table_name
        )
        .fetch_one(&pool)
        .await?;

        if table_exists == Some(true) {
            info!("✅ Table 'ob-poc.{}' exists", table_name);
        } else {
            error!("❌ Table 'ob-poc.{}' does not exist", table_name);
            return Err(format!("Missing table: {}", table_name).into());
        }
    }

    // Test 3: Check table structure for dsl_instances
    info!("🔍 Checking dsl_instances table structure...");
    let columns = sqlx::query!(
        r#"
        SELECT column_name, data_type, is_nullable
        FROM information_schema.columns
        WHERE table_schema = 'ob-poc' AND table_name = 'dsl_instances'
        ORDER BY ordinal_position
        "#
    )
    .fetch_all(&pool)
    .await?;

    info!("📋 dsl_instances table columns:");
    for column in columns {
        info!(
            "   - {}: {} (nullable: {})",
            column.column_name.unwrap_or_else(|| "unknown".to_string()),
            column.data_type.unwrap_or_else(|| "unknown".to_string()),
            column.is_nullable.unwrap_or_else(|| "unknown".to_string())
        );
    }

    // Test 4: Simple insert/select/delete test
    info!("🔧 Testing basic CRUD operations...");

    // Insert test data
    let test_instance_id = uuid::Uuid::new_v4();
    sqlx::query!(
        r#"
        INSERT INTO "ob-poc".dsl_instances
        (instance_id, domain_name, business_reference, current_version, status, metadata)
        VALUES ($1, $2, $3, $4, $5, $6)
        "#,
        test_instance_id,
        "test",
        "TEST-CONNECTION-001",
        1,
        "CREATED",
        serde_json::json!({"test": true})
    )
    .execute(&pool)
    .await?;

    info!("✅ Test data inserted successfully");

    // Select test data
    let retrieved = sqlx::query!(
        r#"
        SELECT instance_id, domain_name, business_reference, status
        FROM "ob-poc".dsl_instances
        WHERE instance_id = $1
        "#,
        test_instance_id
    )
    .fetch_one(&pool)
    .await?;

    info!(
        "✅ Test data retrieved: {} - {} ({})",
        retrieved.business_reference, retrieved.domain_name, retrieved.status
    );

    // Clean up test data
    sqlx::query!(
        r#"DELETE FROM "ob-poc".dsl_instances WHERE instance_id = $1"#,
        test_instance_id
    )
    .execute(&pool)
    .await?;

    info!("✅ Test data cleaned up");

    // Test 5: Check for any existing data
    let instance_count = sqlx::query_scalar!(r#"SELECT COUNT(*) FROM "ob-poc".dsl_instances"#)
        .fetch_one(&pool)
        .await?;

    let version_count =
        sqlx::query_scalar!(r#"SELECT COUNT(*) FROM "ob-poc".dsl_instance_versions"#)
            .fetch_one(&pool)
            .await?;

    info!("📊 Database statistics:");
    info!("   - DSL instances: {}", instance_count.unwrap_or(0));
    info!("   - DSL versions: {}", version_count.unwrap_or(0));

    // Close the connection pool
    pool.close().await;
    info!("✅ Database connection closed");

    info!("🎉 All database tests passed successfully!");
    info!("   Database is ready for DSL Manager operations");
    info!("   You can now run:");
    info!("   - DSL Manager demos");
    info!("   - AST visualization tools");
    info!("   - Egui DSL Visualizer");

    Ok(())
}

/// Mask sensitive information in database URL for logging
fn mask_url(url: &str) -> String {
    if let Ok(parsed) = url::Url::parse(url) {
        let mut masked = parsed.clone();
        if parsed.password().is_some() {
            let _ = masked.set_password(Some("***"));
        }
        masked.to_string()
    } else {
        // If URL parsing fails, just mask the middle part
        if url.len() > 20 {
            format!("{}***{}", &url[..10], &url[url.len() - 10..])
        } else {
            "***".to_string()
        }
    }
}
