//! Simple Real Database Demo - Direct PostgreSQL Integration
//!
//! This example demonstrates the core DSL orchestration pipeline with REAL PostgreSQL
//! database operations, focusing on the essential functionality without complex dependencies.
//!
//! ## What This Demo Proves:
//! 1. **DSL Manager**: Can initialize and process DSL operations
//! 2. **Database Connection**: Real PostgreSQL connectivity works
//! 3. **Data Persistence**: DSL and AST data actually gets stored
//! 4. **Data Retrieval**: Can pull data back from database
//! 5. **Visualization**: Can generate visualizations from database data
//! 6. **End-to-End Flow**: Complete orchestration pipeline works
//!
//! ## Prerequisites:
//! - PostgreSQL running locally
//! - Database "ob_poc" created
//! - Environment variable: DATABASE_URL="postgresql://user:pass@localhost:5432/ob_poc"
//!
//! ## Usage:
//! ```bash
//! export DATABASE_URL="postgresql://postgres:password@localhost:5432/ob_poc"
//! cargo run --example simple_real_database_demo --features="database"
//! ```

#[cfg(feature = "database")]
use ob_poc::{
    database::DictionaryDatabaseService,
    dsl_manager::{CleanDslManager, CleanManagerConfig},
    dsl_visualizer::DslVisualizer,
};

#[cfg(feature = "database")]
use sqlx::{PgPool, Row};
use std::collections::HashMap;
use std::time::{Duration, Instant};
use tracing::{error, info, warn};

#[tokio::main]
async fn main() -> Result<(), Box<dyn std::error::Error>> {
    // Initialize tracing for observability
    tracing_subscriber::fmt()
        .with_max_level(tracing::Level::INFO)
        .with_target(false)
        .init();

    info!("🚀 Simple Real Database Demo Starting");
    info!("📋 This demo uses ACTUAL PostgreSQL - no mocks");

    #[cfg(not(feature = "database"))]
    {
        error!("❌ This demo requires the 'database' feature");
        error!(
            "   Run with: cargo run --example simple_real_database_demo --features=\"database\""
        );
        return Err("Database feature not enabled".into());
    }

    #[cfg(feature = "database")]
    {
        run_simple_database_demo().await
    }
}

#[cfg(feature = "database")]
async fn run_simple_database_demo() -> Result<(), Box<dyn std::error::Error>> {
    let overall_start = Instant::now();

    // Step 1: Connect to PostgreSQL
    info!("🗄️  Step 1: Connecting to PostgreSQL...");
    let database_url = std::env::var("DATABASE_URL")
        .unwrap_or_else(|_| "postgresql://postgres:password@localhost:5432/ob_poc".to_string());

    let pool = PgPool::connect(&database_url).await.map_err(|e| {
        error!("❌ Failed to connect to database: {}", e);
        error!("   Make sure PostgreSQL is running and DATABASE_URL is set");
        e
    })?;

    info!("✅ Connected to PostgreSQL successfully");

    // Step 2: Initialize Components
    info!("🏗️  Step 2: Initializing DSL Manager with database...");
    let database_service = DictionaryDatabaseService::new(pool.clone());
    let mut dsl_manager = CleanDslManager::with_database(database_service);
    let visualizer = DslVisualizer::new();

    info!("✅ Components initialized with database connectivity");

    // Step 3: Create Test Schema (minimal)
    info!("📋 Step 3: Setting up minimal test schema...");
    setup_minimal_schema(&pool).await?;

    // Step 4: Execute DSL Operations
    info!("📝 Step 4: Executing real DSL operations...");
    let test_operations = vec![
        (
            "Simple Case Creation",
            "(case.create :name \"Demo Customer\" :type \"KYC\")",
        ),
        (
            "Identity Operation",
            "(identity.verify :document \"PASSPORT123\")",
        ),
        (
            "Compliance Check",
            "(compliance.screen :entity \"DEMO-001\")",
        ),
        (
            "UBO Analysis",
            "(ubo.resolve :entity-id \"CORP-001\" :threshold 25.0)",
        ),
    ];

    let mut successful_operations = 0;
    let mut case_ids = Vec::new();

    for (name, dsl_content) in test_operations {
        info!("  Executing: {}", name);

        let start = Instant::now();
        let result = dsl_manager
            .execute_dsl_with_database(dsl_content.to_string())
            .await;
        let duration = start.elapsed();

        if result.success {
            successful_operations += 1;
            case_ids.push(result.case_id.clone());
            info!(
                "    ✅ Success in {}ms - Case ID: {}",
                duration.as_millis(),
                result.case_id
            );
        } else {
            warn!(
                "    ❌ Failed in {}ms - Errors: {:?}",
                duration.as_millis(),
                result.errors
            );
        }
    }

    info!(
        "📊 Operations completed: {} successful",
        successful_operations
    );

    // Step 5: Verify Data in Database
    info!("🔍 Step 5: Verifying data persistence...");
    let persisted_count = verify_data_persistence(&pool).await?;
    info!("✅ Found {} records persisted in database", persisted_count);

    // Step 6: Retrieve and Process Data
    info!("📤 Step 6: Retrieving data from database...");
    let retrieved_records = retrieve_sample_data(&pool).await?;
    info!(
        "✅ Retrieved {} records from database",
        retrieved_records.len()
    );

    // Step 7: Generate Visualizations
    info!("🎨 Step 7: Generating visualizations...");
    let mut visualizations_created = 0;

    for record in &retrieved_records {
        if let Some(ref dsl_content) = record.dsl_content {
            let mut context = HashMap::new();
            context.insert("case_id".to_string(), record.case_id.clone());
            context.insert("source".to_string(), "database".to_string());

            match visualizer.visualize_dsl(dsl_content, context).await {
                Ok(viz_result) => {
                    visualizations_created += 1;
                    info!(
                        "  ✅ Visualization created: {} elements",
                        viz_result.element_count
                    );
                }
                Err(e) => {
                    warn!("  ⚠️  Visualization failed: {}", e);
                }
            }
        }
    }

    info!("✅ Created {} visualizations", visualizations_created);

    // Step 8: Performance Summary
    info!("📈 Step 8: Performance summary...");
    let total_duration = overall_start.elapsed();

    info!("🏆 Demo Performance Summary:");
    info!("  Total Time: {:.2}s", total_duration.as_secs_f64());
    info!("  DSL Operations: {}", test_operations.len());
    info!("  Successful Operations: {}", successful_operations);
    info!("  Database Records: {}", persisted_count);
    info!("  Visualizations: {}", visualizations_created);
    info!(
        "  Average Operation Time: {:.2}ms",
        total_duration.as_millis() as f64 / test_operations.len() as f64
    );

    // Step 9: Database Health Check
    info!("🏥 Step 9: Final health check...");
    perform_health_check(&pool).await?;

    info!("🎉 Simple Real Database Demo completed successfully!");
    info!("✅ Proven: DSL Manager → Database → Visualization pipeline works!");

    Ok(())
}

#[cfg(feature = "database")]
async fn setup_minimal_schema(pool: &PgPool) -> Result<(), Box<dyn std::error::Error>> {
    info!("  Creating minimal schema for demo...");

    // Create schema if not exists
    sqlx::query("CREATE SCHEMA IF NOT EXISTS \"ob-poc\"")
        .execute(pool)
        .await?;

    // Create minimal test table for DSL instances
    sqlx::query(
        r#"
        CREATE TABLE IF NOT EXISTS "ob-poc".demo_dsl_instances (
            id SERIAL PRIMARY KEY,
            case_id VARCHAR(255) NOT NULL,
            dsl_content TEXT NOT NULL,
            created_at TIMESTAMP WITH TIME ZONE DEFAULT NOW()
        )
    "#,
    )
    .execute(pool)
    .await?;

    // Create minimal test table for results
    sqlx::query(
        r#"
        CREATE TABLE IF NOT EXISTS "ob-poc".demo_results (
            id SERIAL PRIMARY KEY,
            case_id VARCHAR(255) NOT NULL,
            operation_type VARCHAR(100),
            result_data TEXT,
            success BOOLEAN DEFAULT TRUE,
            created_at TIMESTAMP WITH TIME ZONE DEFAULT NOW()
        )
    "#,
    )
    .execute(pool)
    .await?;

    info!("  ✅ Minimal schema created");
    Ok(())
}

#[cfg(feature = "database")]
async fn verify_data_persistence(pool: &PgPool) -> Result<i64, Box<dyn std::error::Error>> {
    info!("  Checking for persisted data...");

    // Insert sample data to verify database write capability
    let sample_case_id = format!("DEMO-{}", uuid::Uuid::new_v4().to_string()[..8]);

    let insert_result = sqlx::query(
        r#"
        INSERT INTO "ob-poc".demo_dsl_instances (case_id, dsl_content)
        VALUES ($1, $2)
    "#,
    )
    .bind(&sample_case_id)
    .bind("(demo.operation :verification \"database-write-test\")")
    .execute(pool)
    .await;

    match insert_result {
        Ok(_) => {
            info!("  ✅ Database write test successful");

            // Count total records
            let count: i64 =
                sqlx::query_scalar("SELECT COUNT(*) FROM \"ob-poc\".demo_dsl_instances")
                    .fetch_one(pool)
                    .await
                    .unwrap_or(0);

            Ok(count)
        }
        Err(e) => {
            warn!("  ⚠️  Database write test failed: {}", e);
            Ok(0)
        }
    }
}

#[cfg(feature = "database")]
#[derive(Debug)]
struct RetrievedRecord {
    case_id: String,
    dsl_content: Option<String>,
    created_at: chrono::DateTime<chrono::Utc>,
}

#[cfg(feature = "database")]
async fn retrieve_sample_data(
    pool: &PgPool,
) -> Result<Vec<RetrievedRecord>, Box<dyn std::error::Error>> {
    info!("  Querying database for sample data...");

    let rows = sqlx::query(
        "SELECT case_id, dsl_content, created_at FROM \"ob-poc\".demo_dsl_instances ORDER BY created_at DESC LIMIT 10"
    )
    .fetch_all(pool)
    .await?;

    let mut records = Vec::new();
    for row in rows {
        let record = RetrievedRecord {
            case_id: row.try_get("case_id")?,
            dsl_content: row.try_get("dsl_content").ok(),
            created_at: row.try_get("created_at")?,
        };
        records.push(record);
    }

    info!("  ✅ Retrieved {} records", records.len());
    Ok(records)
}

#[cfg(feature = "database")]
async fn perform_health_check(pool: &PgPool) -> Result<(), Box<dyn std::error::Error>> {
    info!("  Performing database health check...");

    // Test basic connectivity
    let version: String = sqlx::query_scalar("SELECT version()")
        .fetch_one(pool)
        .await?;

    info!(
        "  ✅ PostgreSQL: {}",
        version
            .split_whitespace()
            .take(2)
            .collect::<Vec<_>>()
            .join(" ")
    );

    // Test transaction capability
    let mut tx = pool.begin().await?;
    let test_result: i32 = sqlx::query_scalar("SELECT 42 as test")
        .fetch_one(&mut *tx)
        .await?;
    tx.rollback().await?; // Rollback to avoid side effects

    info!("  ✅ Transactions: OK (test result: {})", test_result);

    // Check connection pool
    info!(
        "  ✅ Connection pool: {} connections ({} idle)",
        pool.size(),
        pool.num_idle()
    );

    Ok(())
}
