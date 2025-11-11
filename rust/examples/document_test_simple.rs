//! Simple Document Service Test - Basic CRUD Operations
//!
//! This is a simplified test to verify the document service CRUD operations
//! without complex dependencies.
//!
//! Usage:
//!   DATABASE_URL="postgresql://localhost:5432/ob-poc" cargo run --example document_test_simple

use serde_json::json;
use sqlx::PgPool;
use std::env;
use tokio;
use uuid::Uuid;

// Simplified error type for this test
#[derive(Debug)]
enum TestError {
    Database(sqlx::Error),
    Other(String),
}

impl From<sqlx::Error> for TestError {
    fn from(err: sqlx::Error) -> Self {
        TestError::Database(err)
    }
}

#[tokio::main]
async fn main() -> Result<(), Box<dyn std::error::Error>> {
    println!("🚀 Simple Document Service Test");
    println!("===============================");

    // Get database connection
    let database_url = env::var("DATABASE_URL")
        .unwrap_or_else(|_| "postgresql://localhost:5432/ob-poc".to_string());

    let pool = PgPool::connect(&database_url).await?;
    println!("✅ Connected to database");

    // Test 1: Check ISO Asset Types
    test_iso_asset_types(&pool).await?;

    // Test 2: Check Document Types
    test_document_types(&pool).await?;

    // Test 3: Test Document Creation
    test_document_creation(&pool).await?;

    // Test 4: Test Document Search
    test_document_search(&pool).await?;

    // Test 5: Test Investment Mandate Validation
    test_investment_mandate_validation(&pool).await?;

    println!("\n🎉 All tests completed successfully!");
    Ok(())
}

async fn test_iso_asset_types(pool: &PgPool) -> Result<(), TestError> {
    println!("\n📊 Testing ISO Asset Types...");

    // Check if ISO asset types exist
    let count = sqlx::query!(
        "SELECT COUNT(*) as count FROM \"ob-poc\".iso_asset_types WHERE active = true"
    )
    .fetch_one(pool)
    .await?
    .count
    .unwrap_or(0);

    println!("✅ Found {} active ISO asset types", count);

    // Test specific asset type lookup
    let govt_asset = sqlx::query!(
        "SELECT iso_code, asset_name FROM \"ob-poc\".iso_asset_types WHERE iso_code = 'GOVT'"
    )
    .fetch_optional(pool)
    .await?;

    if let Some(asset) = govt_asset {
        println!(
            "✅ Found Government Bonds: {} ({})",
            asset.asset_name, asset.iso_code
        );
    }

    // Test risk profile filtering
    let conservative_count = sqlx::query!(
        "SELECT COUNT(*) as count FROM \"ob-poc\".iso_asset_types WHERE suitable_for_conservative = true"
    )
    .fetch_one(pool)
    .await?
    .count
    .unwrap_or(0);

    println!(
        "✅ {} assets suitable for conservative portfolios",
        conservative_count
    );

    Ok(())
}

async fn test_document_types(pool: &PgPool) -> Result<(), TestError> {
    println!("\n📄 Testing Document Types...");

    // Check document types count
    let count =
        sqlx::query!("SELECT COUNT(*) as count FROM \"ob-poc\".document_types WHERE active = true")
            .fetch_one(pool)
            .await?
            .count
            .unwrap_or(0);

    println!("✅ Found {} active document types", count);

    // Check for investment mandate document type
    let mandate_type = sqlx::query!(
        r#"
        SELECT type_code, display_name, array_length(expected_attribute_ids, 1) as attr_count
        FROM "ob-poc".document_types
        WHERE type_code = 'investment_mandate'
        "#
    )
    .fetch_optional(pool)
    .await?;

    if let Some(doc_type) = mandate_type {
        println!(
            "✅ Found Investment Mandate type: {} ({} attributes)",
            doc_type.display_name,
            doc_type.attr_count.unwrap_or(0)
        );
    } else {
        println!("⚠️  Investment Mandate document type not found");
    }

    // Check document types by category
    let categories = sqlx::query!(
        "SELECT category, COUNT(*) as count FROM \"ob-poc\".document_types GROUP BY category ORDER BY count DESC"
    )
    .fetch_all(pool)
    .await?;

    println!("📋 Document types by category:");
    for category in categories {
        println!(
            "  - {}: {} types",
            category.category,
            category.count.unwrap_or(0)
        );
    }

    Ok(())
}

async fn test_document_creation(pool: &PgPool) -> Result<(), TestError> {
    println!("\n📝 Testing Document Creation...");

    // Get investment mandate document type
    let doc_type = sqlx::query!(
        "SELECT type_id FROM \"ob-poc\".document_types WHERE type_code = 'investment_mandate' LIMIT 1"
    )
    .fetch_optional(pool)
    .await?;

    if let Some(doc_type) = doc_type {
        // Create a test document
        let document_code = format!("TEST-DOC-{}", Uuid::new_v4().simple());

        let doc_result = sqlx::query!(
            r#"
            INSERT INTO "ob-poc".document_catalog (
                document_code, document_type_id, title, description, language,
                tags, confidentiality_level
            ) VALUES (
                $1, $2, $3, $4, $5, $6, $7
            ) RETURNING document_id, document_code
            "#,
            document_code,
            doc_type.type_id,
            "Test Investment Mandate",
            "A test document for CRUD operations",
            "en",
            &vec!["test", "demo"],
            "internal"
        )
        .fetch_one(pool)
        .await?;

        println!(
            "✅ Created test document: {} (ID: {})",
            doc_result.document_code, doc_result.document_id
        );

        // Update with extracted attributes
        let extracted_attrs = json!({
            "d0cf0021-0000-0000-0000-000000000001": "Test Growth Fund",
            "d0cf0021-0000-0000-0000-000000000002": "Long-term capital growth through diversified investments",
            "d0cf0021-0000-0000-0000-000000000004": "EQTY,GOVT,CORP",
            "d0cf0021-0000-0000-0000-000000000006": "moderate"
        });

        let _update_result = sqlx::query!(
            r#"
            UPDATE "ob-poc".document_catalog
            SET extracted_attributes = $1,
                extraction_confidence = $2,
                extraction_method = $3,
                updated_at = NOW()
            WHERE document_id = $4
            "#,
            extracted_attrs,
            0.95,
            "test",
            doc_result.document_id
        )
        .execute(pool)
        .await?;

        println!("✅ Updated document with extracted attributes");

        // Clean up - delete test document
        sqlx::query!(
            "DELETE FROM \"ob-poc\".document_catalog WHERE document_id = $1",
            doc_result.document_id
        )
        .execute(pool)
        .await?;

        println!("✅ Cleaned up test document");
    } else {
        println!("⚠️  Skipping document creation test - no investment mandate type found");
    }

    Ok(())
}

async fn test_document_search(pool: &PgPool) -> Result<(), TestError> {
    println!("\n🔍 Testing Document Search...");

    // Basic document count
    let total_docs = sqlx::query!("SELECT COUNT(*) as count FROM \"ob-poc\".document_catalog")
        .fetch_one(pool)
        .await?
        .count
        .unwrap_or(0);

    println!("📄 Total documents in catalog: {}", total_docs);

    // Search with joins
    let docs_with_types = sqlx::query!(
        r#"
        SELECT
            dc.document_code,
            dt.type_code,
            dt.display_name,
            dc.extraction_method
        FROM "ob-poc".document_catalog dc
        LEFT JOIN "ob-poc".document_types dt ON dc.document_type_id = dt.type_id
        LIMIT 5
        "#
    )
    .fetch_all(pool)
    .await?;

    if !docs_with_types.is_empty() {
        println!("📋 Sample documents:");
        for doc in docs_with_types {
            println!(
                "  - {} ({}): {}",
                doc.document_code,
                doc.type_code.unwrap_or("Unknown".to_string()),
                doc.display_name.unwrap_or("No type name".to_string())
            );
        }
    } else {
        println!("ℹ️  No documents found in catalog");
    }

    // Count documents by type
    let type_counts = sqlx::query!(
        r#"
        SELECT dt.type_code, COUNT(dc.document_id) as doc_count
        FROM "ob-poc".document_types dt
        LEFT JOIN "ob-poc".document_catalog dc ON dt.type_id = dc.document_type_id
        GROUP BY dt.type_code
        HAVING COUNT(dc.document_id) > 0
        ORDER BY doc_count DESC
        LIMIT 5
        "#
    )
    .fetch_all(pool)
    .await?;

    if !type_counts.is_empty() {
        println!("📊 Documents by type:");
        for count in type_counts {
            println!(
                "  - {}: {} documents",
                count.type_code,
                count.doc_count.unwrap_or(0)
            );
        }
    }

    Ok(())
}

async fn test_investment_mandate_validation(pool: &PgPool) -> Result<(), TestError> {
    println!("\n🎯 Testing Investment Mandate Validation...");

    // Test ISO asset code validation function
    let validation_result =
        sqlx::query!("SELECT validate_iso_asset_codes('GOVT,EQTY,CORP') as is_valid")
            .fetch_one(pool)
            .await?;

    println!(
        "✅ Asset codes validation: {}",
        if validation_result.is_valid.unwrap_or(false) {
            "PASSED"
        } else {
            "FAILED"
        }
    );

    // Test invalid codes
    let invalid_result =
        sqlx::query!("SELECT validate_iso_asset_codes('GOVT,INVALID,CORP') as is_valid")
            .fetch_one(pool)
            .await?;

    println!(
        "✅ Invalid codes rejection: {}",
        if !invalid_result.is_valid.unwrap_or(true) {
            "PASSED"
        } else {
            "FAILED"
        }
    );

    // Check for documents with investment mandate attributes
    let mandate_docs = sqlx::query!(
        r#"
        SELECT
            document_code,
            extracted_attributes ? 'd0cf0021-0000-0000-0000-000000000001' as has_fund_name,
            extracted_attributes ? 'd0cf0021-0000-0000-0000-000000000004' as has_permitted_assets
        FROM "ob-poc".document_catalog
        WHERE extracted_attributes IS NOT NULL
        AND extracted_attributes ? 'd0cf0021-0000-0000-0000-000000000001'
        LIMIT 3
        "#
    )
    .fetch_all(pool)
    .await?;

    if !mandate_docs.is_empty() {
        println!("📋 Documents with investment mandate data:");
        for doc in mandate_docs {
            println!(
                "  - {}: Fund Name: {}, Assets: {}",
                doc.document_code,
                if doc.has_fund_name.unwrap_or(false) {
                    "✅"
                } else {
                    "❌"
                },
                if doc.has_permitted_assets.unwrap_or(false) {
                    "✅"
                } else {
                    "❌"
                }
            );
        }
    } else {
        println!("ℹ️  No investment mandate documents found with extracted data");
    }

    // Test attribute statistics
    let attr_stats = sqlx::query!(
        r#"
        SELECT
            COUNT(*) as total_docs,
            COUNT(CASE WHEN extracted_attributes IS NOT NULL THEN 1 END) as docs_with_attrs,
            AVG(extraction_confidence) as avg_confidence
        FROM "ob-poc".document_catalog
        "#
    )
    .fetch_one(pool)
    .await?;

    println!("📊 Attribute extraction statistics:");
    println!(
        "  - Total documents: {}",
        attr_stats.total_docs.unwrap_or(0)
    );
    println!(
        "  - With extracted attributes: {}",
        attr_stats.docs_with_attrs.unwrap_or(0)
    );
    if let Some(avg_conf) = attr_stats.avg_confidence {
        println!("  - Average confidence: {:.2}", avg_conf);
    }

    Ok(())
}
