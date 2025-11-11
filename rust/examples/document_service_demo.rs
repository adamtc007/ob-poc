//! Document Service Demo - Comprehensive CRUD Operations Example
//!
//! This example demonstrates the complete document-attribute bridge system with:
//! - ISO asset types management
//! - Document types and catalog operations
//! - Investment mandate processing with asset validation
//! - AI-powered extraction workflows
//! - Cross-domain document relationships
//!
//! Usage:
//!   cargo run --example document_service_demo

use ob_poc::{error::DslError, models::document_models::*, services::DocumentService};
use serde_json::json;
use sqlx::PgPool;
use std::env;
use time::PrimitiveDateTime;
use tokio;
use uuid::Uuid;

#[tokio::main]
async fn main() -> Result<(), Box<dyn std::error::Error>> {
    // Initialize logging
    env_logger::init();

    // Get database connection
    let database_url = env::var("DATABASE_URL")
        .unwrap_or_else(|_| "postgresql://localhost:5432/ob-poc".to_string());

    let pool = PgPool::connect(&database_url).await?;
    let document_service = DocumentService::new(pool);

    println!("🚀 Document Service Demo - Comprehensive CRUD Operations");
    println!("======================================================");

    // Demonstrate ISO Asset Types Operations
    demonstrate_iso_asset_types(&document_service).await?;

    // Demonstrate Document Types Operations
    demonstrate_document_types(&document_service).await?;

    // Demonstrate Document Catalog Operations
    demonstrate_document_catalog(&document_service).await?;

    // Demonstrate Investment Mandate Processing
    demonstrate_investment_mandate(&document_service).await?;

    // Demonstrate AI Extraction
    demonstrate_ai_extraction(&document_service).await?;

    // Demonstrate Analytics and Statistics
    demonstrate_analytics(&document_service).await?;

    println!("\n🎉 Document Service Demo Completed Successfully!");
    println!("✅ All CRUD operations and validations working correctly");

    Ok(())
}

async fn demonstrate_iso_asset_types(service: &DocumentService) -> Result<(), DslError> {
    println!("\n📊 ISO Asset Types Operations");
    println!("-----------------------------");

    // Test creating a new ISO asset type
    let new_asset_type = NewIsoAssetType {
        iso_code: "TEST".to_string(),
        asset_name: "Test Securities".to_string(),
        asset_category: "Test".to_string(),
        asset_subcategory: Some("Demo".to_string()),
        description: Some("Test asset type for demonstration".to_string()),
        regulatory_classification: Some("Test".to_string()),
        liquidity_profile: Some("High".to_string()),
        suitable_for_conservative: true,
        suitable_for_moderate: true,
        suitable_for_aggressive: false,
        suitable_for_balanced: true,
        credit_risk_level: Some("low".to_string()),
        market_risk_level: Some("medium".to_string()),
        liquidity_risk_level: Some("low".to_string()),
    };

    let created_asset = service.create_iso_asset_type(new_asset_type).await?;
    println!(
        "✅ Created ISO asset type: {} ({})",
        created_asset.asset_name, created_asset.iso_code
    );

    // Test retrieving asset types for different risk profiles
    let conservative_assets = service
        .get_iso_asset_types_for_risk_profile("conservative")
        .await?;
    println!(
        "📈 Found {} assets suitable for conservative portfolios",
        conservative_assets.len()
    );

    let aggressive_assets = service
        .get_iso_asset_types_for_risk_profile("aggressive")
        .await?;
    println!(
        "🚀 Found {} assets suitable for aggressive portfolios",
        aggressive_assets.len()
    );

    // Test asset code validation
    let test_codes = vec!["GOVT".to_string(), "EQTY".to_string(), "TEST".to_string()];
    let validated_codes = service.validate_iso_asset_codes(&test_codes).await?;
    println!(
        "✅ Validated {} asset codes: {:?}",
        validated_codes.len(),
        validated_codes
    );

    // Test asset suitability check
    let suitability_check = service
        .check_asset_suitability_for_risk_profile(
            &["GOVT".to_string(), "EQTY".to_string(), "HEDG".to_string()],
            "conservative",
        )
        .await?;

    println!("📋 Asset Suitability Analysis for Conservative Profile:");
    for check in suitability_check {
        let status = if check.is_suitable { "✅" } else { "❌" };
        println!(
            "  {} {} ({}): {}",
            status, check.asset_name, check.iso_code, check.reason
        );
    }

    Ok(())
}

async fn demonstrate_document_types(service: &DocumentService) -> Result<(), DslError> {
    println!("\n📄 Document Types Operations");
    println!("----------------------------");

    // Create a new document type
    let new_doc_type = NewDocumentType {
        type_code: "test_mandate".to_string(),
        display_name: "Test Investment Mandate".to_string(),
        category: "fund".to_string(),
        domain: Some("fund_management".to_string()),
        primary_attribute_id: None,
        description: Some("Test investment mandate document type".to_string()),
        typical_issuers: vec![
            "fund_manager".to_string(),
            "investment_committee".to_string(),
        ],
        validity_period_days: Some(365),
        expected_attribute_ids: vec![
            "d0cf0021-0000-0000-0000-000000000001".parse().unwrap(),
            "d0cf0021-0000-0000-0000-000000000002".parse().unwrap(),
            "d0cf0021-0000-0000-0000-000000000004".parse().unwrap(),
        ],
        key_data_point_attributes: Some(vec![
            "d0cf0021-0000-0000-0000-000000000001".parse().unwrap(),
            "d0cf0021-0000-0000-0000-000000000004".parse().unwrap(),
        ]),
        ai_description: Some("Investment mandate defining fund investment parameters".to_string()),
        common_contents: Some(
            "Fund name, investment objective, permitted assets, risk profile".to_string(),
        ),
    };

    let created_doc_type = service.create_document_type(new_doc_type).await?;
    println!(
        "✅ Created document type: {} ({})",
        created_doc_type.display_name, created_doc_type.type_code
    );

    // Retrieve document types by category
    let fund_doc_types = service.get_document_types_by_category("fund").await?;
    println!("📁 Found {} fund document types", fund_doc_types.len());

    // Get specific document type
    if let Some(doc_type) = service
        .get_document_type_by_code("investment_mandate")
        .await?
    {
        println!("📋 Investment Mandate Document Type:");
        println!("  - Name: {}", doc_type.display_name);
        println!("  - Category: {}", doc_type.category);
        println!(
            "  - Expected Attributes: {}",
            doc_type.expected_attribute_ids.len()
        );
        println!(
            "  - AI Description: {}",
            doc_type.ai_description.unwrap_or("None".to_string())
        );
    }

    Ok(())
}

async fn demonstrate_document_catalog(service: &DocumentService) -> Result<(), DslError> {
    println!("\n📚 Document Catalog Operations");
    println!("------------------------------");

    // Get the investment mandate document type
    let mandate_type = service
        .get_document_type_by_code("investment_mandate")
        .await?
        .ok_or_else(|| {
            DslError::NotFoundError("Investment mandate document type not found".to_string())
        })?;

    // Create a new document
    let new_document = NewDocumentCatalog {
        document_code: "DOC-MANDATE-TEST-001".to_string(),
        document_type_id: mandate_type.type_id,
        issuer_id: None,
        title: Some("Test Fund Investment Mandate".to_string()),
        description: Some("Sample investment mandate for demonstration purposes".to_string()),
        issue_date: Some(
            PrimitiveDateTime::parse(
                "2024-01-15 00:00:00",
                &time::format_description::parse("[year]-[month]-[day] [hour]:[minute]:[second]")
                    .unwrap(),
            )
            .unwrap(),
        ),
        expiry_date: Some(
            PrimitiveDateTime::parse(
                "2025-01-15 00:00:00",
                &time::format_description::parse("[year]-[month]-[day] [hour]:[minute]:[second]")
                    .unwrap(),
            )
            .unwrap(),
        ),
        language: Some("en".to_string()),
        related_entities: Some(vec!["fund-test-001".to_string()]),
        tags: Some(vec![
            "investment_mandate".to_string(),
            "test".to_string(),
            "demo".to_string(),
        ]),
        confidentiality_level: Some("internal".to_string()),
    };

    let created_document = service.create_document(new_document).await?;
    println!(
        "✅ Created document: {} (ID: {})",
        created_document.document_code, created_document.document_id
    );

    // Update document with extracted attributes
    let extracted_attributes = json!({
        "d0cf0021-0000-0000-0000-000000000001": "Demo Growth Fund",
        "d0cf0021-0000-0000-0000-000000000002": "Capital appreciation through diversified equity investments",
        "d0cf0021-0000-0000-0000-000000000004": "EQTY,GOVT,CORP",
        "d0cf0021-0000-0000-0000-000000000006": "moderate"
    });

    let updated_document = service
        .update_document_attributes(
            created_document.document_id,
            extracted_attributes,
            Some(0.92),
            Some("demo".to_string()),
        )
        .await?;

    println!(
        "✅ Updated document with extracted attributes (confidence: {})",
        updated_document.extraction_confidence.unwrap_or(0.0)
    );

    // Get document with full attributes
    if let Some(doc_with_attrs) = service
        .get_document_with_attributes(created_document.document_id)
        .await?
    {
        println!("📋 Document Details:");
        println!("  - Code: {}", doc_with_attrs.document.document_code);
        println!(
            "  - Type: {}",
            doc_with_attrs
                .document_type_name
                .unwrap_or("Unknown".to_string())
        );
        println!(
            "  - Category: {}",
            doc_with_attrs
                .document_category
                .unwrap_or("Unknown".to_string())
        );
        println!(
            "  - Extraction Method: {}",
            doc_with_attrs
                .document
                .extraction_method
                .unwrap_or("None".to_string())
        );

        if let Some(extracted) = &doc_with_attrs.document.extracted_attributes {
            println!(
                "  - Extracted Data Keys: {}",
                extracted.as_object().unwrap().keys().len()
            );
        }
    }

    // Perform document search
    let search_request = DocumentSearchRequest {
        query: Some("investment".to_string()),
        document_type: Some("investment_mandate".to_string()),
        category: None,
        domain: None,
        issuer: None,
        tags: None,
        confidentiality_level: None,
        verification_status: None,
        issue_date_from: None,
        issue_date_to: None,
        extracted_attributes: None,
        limit: Some(10),
        offset: Some(0),
    };

    let search_results = service.search_documents(search_request).await?;
    println!(
        "🔍 Document search found {} results (total: {})",
        search_results.documents.len(),
        search_results.total_count
    );

    Ok(())
}

async fn demonstrate_investment_mandate(service: &DocumentService) -> Result<(), DslError> {
    println!("\n🎯 Investment Mandate Processing");
    println!("--------------------------------");

    // Find a test investment mandate document
    let search_request = DocumentSearchRequest {
        document_type: Some("investment_mandate".to_string()),
        limit: Some(1),
        offset: Some(0),
        query: None,
        category: None,
        domain: None,
        issuer: None,
        tags: None,
        confidentiality_level: None,
        verification_status: None,
        issue_date_from: None,
        issue_date_to: None,
        extracted_attributes: None,
    };

    let search_results = service.search_documents(search_request).await?;

    if let Some(document) = search_results.documents.first() {
        let document_id = document.document.document_id;

        // Extract investment mandate data
        let mandate_data = service.extract_investment_mandate_data(document_id).await?;
        println!("📊 Investment Mandate Data Extraction:");
        println!(
            "  - Fund Name: {}",
            mandate_data.fund_name.unwrap_or("N/A".to_string())
        );
        println!(
            "  - Investment Objective: {}",
            mandate_data
                .investment_objective
                .as_ref()
                .map(|s| if s.len() > 50 {
                    format!("{}...", &s[..50])
                } else {
                    s.clone()
                })
                .unwrap_or("N/A".to_string())
        );
        println!(
            "  - Risk Profile: {}",
            mandate_data.risk_profile.unwrap_or("N/A".to_string())
        );

        if let Some(permitted_assets) = &mandate_data.permitted_assets {
            println!("  - Permitted Assets: {:?}", permitted_assets);
        }

        // Validate investment mandate
        let validation_result = service.validate_investment_mandate(document_id).await?;
        println!("🔍 Investment Mandate Validation:");
        println!(
            "  - Valid: {}",
            if validation_result.is_valid {
                "✅ Yes"
            } else {
                "❌ No"
            }
        );

        if !validation_result.validation_errors.is_empty() {
            println!("  - Validation Errors:");
            for error in &validation_result.validation_errors {
                println!("    • {}", error);
            }
        }

        if !validation_result.asset_suitability_issues.is_empty() {
            println!("  - Asset Suitability Issues:");
            for issue in &validation_result.asset_suitability_issues {
                println!(
                    "    • {} ({}): {}",
                    issue.asset_name, issue.iso_code, issue.reason
                );
            }
        }

        if validation_result.is_valid {
            println!("✅ Investment mandate passes all validations");
        }
    } else {
        println!("ℹ️  No investment mandate documents found for validation demo");
    }

    Ok(())
}

async fn demonstrate_ai_extraction(service: &DocumentService) -> Result<(), DslError> {
    println!("\n🤖 AI Extraction Demonstration");
    println!("------------------------------");

    // Find a document for AI extraction demo
    let search_request = DocumentSearchRequest {
        limit: Some(1),
        offset: Some(0),
        query: None,
        document_type: None,
        category: None,
        domain: None,
        issuer: None,
        tags: None,
        confidentiality_level: None,
        verification_status: None,
        issue_date_from: None,
        issue_date_to: None,
        extracted_attributes: None,
    };

    let search_results = service.search_documents(search_request).await?;

    if let Some(document) = search_results.documents.first() {
        let document_id = document.document.document_id;

        // Create AI extraction request
        let extraction_request = AiExtractionRequest {
            document_id,
            extraction_method: "ai".to_string(),
            template: Some("investment_mandate".to_string()),
            confidence_threshold: Some(0.8),
            ai_model: Some("demo-model-v1".to_string()),
        };

        // Process AI extraction
        let extraction_result = service.process_ai_extraction(extraction_request).await?;

        println!("🤖 AI Extraction Results:");
        println!("  - Document ID: {}", extraction_result.document_id);
        println!(
            "  - Extraction Method: {}",
            extraction_result.extraction_method
        );
        println!(
            "  - Overall Confidence: {:.2}",
            extraction_result.overall_confidence
        );
        println!(
            "  - Processing Time: {}ms",
            extraction_result.processing_time_ms
        );
        println!(
            "  - Extracted Attributes: {}",
            extraction_result.extracted_attributes.len()
        );

        if let Some(ai_model) = &extraction_result.ai_model_used {
            println!("  - AI Model: {}", ai_model);
        }

        if !extraction_result.extracted_attributes.is_empty() {
            println!("  - Sample Extractions:");
            for (attr_id, value) in extraction_result.extracted_attributes.iter().take(3) {
                println!("    • {}: {}", attr_id, value);
            }
        }

        if !extraction_result.errors.is_empty() {
            println!("  - Errors: {:?}", extraction_result.errors);
        }

        if !extraction_result.warnings.is_empty() {
            println!("  - Warnings: {:?}", extraction_result.warnings);
        }
    }

    Ok(())
}

async fn demonstrate_analytics(service: &DocumentService) -> Result<(), DslError> {
    println!("\n📈 Analytics and Statistics");
    println!("---------------------------");

    // Get document mapping summary
    let mapping_summary = service.get_document_mapping_summary().await?;
    println!("📊 Document Mapping Summary:");
    println!(
        "  - Total Document Types: {}",
        mapping_summary.total_document_types
    );
    println!(
        "  - Mapped Document Types: {}",
        mapping_summary.mapped_document_types
    );
    println!(
        "  - Coverage Percentage: {:.1}%",
        mapping_summary.coverage_percentage
    );
    println!(
        "  - ISO Asset Types: {}",
        mapping_summary.total_iso_asset_types
    );
    println!(
        "  - Document Attributes: {}",
        mapping_summary.total_document_attributes
    );
    println!(
        "  - Investment Mandate Ready: {}",
        if mapping_summary.investment_mandate_ready {
            "✅ Yes"
        } else {
            "❌ No"
        }
    );

    // Get document type statistics
    let type_stats = service.get_document_type_statistics().await?;
    println!("\n📋 Document Type Statistics:");

    let mut categories: std::collections::HashMap<String, Vec<&DocumentTypeStatistics>> =
        std::collections::HashMap::new();

    for stat in &type_stats {
        categories
            .entry(stat.category.clone())
            .or_insert_with(Vec::new)
            .push(stat);
    }

    for (category, stats) in categories {
        println!("  📁 {} Category:", category);
        for stat in stats.iter().take(3) {
            // Show top 3 per category
            println!(
                "    • {}: {} documents ({} extracted)",
                stat.type_code, stat.total_documents, stat.extracted_documents
            );
            if let Some(confidence) = stat.average_confidence {
                println!("      Average confidence: {:.2}", confidence);
            }
        }
    }

    // Get overall document attribute statistics
    let attr_stats = service.get_document_attribute_statistics().await?;
    println!("\n📊 Document Attribute Statistics:");
    println!("  - Total Documents: {}", attr_stats.total_documents);
    println!(
        "  - Documents with Extractions: {}",
        attr_stats.documents_with_extractions
    );
    println!(
        "  - Attribute Coverage: {:.1}%",
        attr_stats.attribute_coverage_percentage
    );

    if let Some(avg_confidence) = attr_stats.average_extraction_confidence {
        println!("  - Average Extraction Confidence: {:.2}", avg_confidence);
    }

    if let Some(common_type) = &attr_stats.most_common_document_type {
        println!("  - Most Common Document Type: {}", common_type);
    }

    if !attr_stats.extraction_methods_used.is_empty() {
        println!(
            "  - Extraction Methods Used: {:?}",
            attr_stats.extraction_methods_used
        );
    }

    Ok(())
}
