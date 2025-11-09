//! Phase 2 DSL Execution Demo
//!
//! This example demonstrates the comprehensive DSL execution capabilities
//! implemented in Phase 2, including:
//! - DSL operation execution with business rule validation
//! - External system integrations (mocked)
//! - State management with event sourcing
//! - Workflow-specific execution contexts
//! - Batch operation processing
//! - Complete audit trail generation

use anyhow::Result;
use serde_json::json;
use std::collections::HashMap;

use ob_poc::execution::{
    context::{ComplianceType, ExecutionContextBuilder},
    engine::{ComprehensiveDslEngine, EngineBuilder, WorkflowType},
    integrations::MockIntegration,
    rules::{ComplianceWorkflowRule, OwnershipLimitsRule},
};
use ob_poc::{data_dictionary::AttributeId, dsl::operations::ExecutableDslOperation};

#[tokio::main]
async fn main() -> Result<()> {
    // Initialize logging
    tracing_subscriber::fmt::init();

    println!("🚀 Phase 2 DSL Execution Engine Demo");
    println!("====================================\n");

    // Create comprehensive DSL engine with custom integrations
    let engine = create_demo_engine().await?;

    // Demo 1: Single operation execution
    println!("📋 Demo 1: Single Operation Execution");
    demo_single_operation(&engine).await?;

    // Demo 2: Batch operation execution
    println!("\n📦 Demo 2: Batch Operation Execution");
    demo_batch_operations(&engine).await?;

    // Demo 3: KYC workflow execution
    println!("\n🔍 Demo 3: KYC Workflow Execution");
    demo_kyc_workflow(&engine).await?;

    // Demo 4: UBO discovery workflow
    println!("\n🏢 Demo 4: UBO Discovery Workflow");
    demo_ubo_workflow(&engine).await?;

    // Demo 5: Business rule validation
    println!("\n⚖️ Demo 5: Business Rule Validation");
    demo_business_rules(&engine).await?;

    // Demo 6: State management and history
    println!("\n📊 Demo 6: State Management and History");
    demo_state_management(&engine).await?;

    println!("\n✅ Phase 2 Demo Complete!");
    println!("The DSL-as-State execution engine is fully operational with:");
    println!("  ✓ Operation handlers for core DSL operations");
    println!("  ✓ Business rules engine for validation");
    println!("  ✓ External integration framework");
    println!("  ✓ Comprehensive state management");
    println!("  ✓ Workflow-specific execution contexts");
    println!("  ✓ Complete audit trails");

    Ok(())
}

async fn create_demo_engine() -> Result<ComprehensiveDslEngine> {
    // Create engine with custom integrations for demo
    let mock_risk_engine = MockIntegration::new("risk_engine").with_response(
        "risk-engine",
        json!({
            "risk_score": 1.8,
            "risk_rating": "LOW",
            "factors": ["clean_sanctions", "good_credit_history", "stable_financials"],
            "confidence": 0.92,
            "last_assessment": "2024-01-15T10:30:00Z"
        }),
    );

    let mock_document_store = MockIntegration::new("document_store").with_response(
        "document-store",
        json!({
            "documents": [
                {
                    "document_id": "doc-cert-zenith-001",
                    "document_type": "certificate_of_incorporation",
                    "status": "verified",
                    "verification_date": "2024-01-10T14:20:00Z",
                    "jurisdiction": "KY",
                    "issuing_authority": "Cayman Islands Registry"
                },
                {
                    "document_id": "doc-mem-articles-001",
                    "document_type": "memorandum_and_articles",
                    "status": "verified",
                    "verification_date": "2024-01-10T14:25:00Z"
                }
            ]
        }),
    );

    let mock_crs_compliance = MockIntegration::new("crs_compliance").with_response(
        "crs-check",
        json!({
            "crs_status": "REPORTABLE",
            "jurisdiction": "US",
            "tax_residence": ["US", "KY"],
            "fatca_status": "US_PERSON",
            "reporting_required": true,
            "next_review_date": "2024-07-15"
        }),
    );

    let engine = EngineBuilder::new()
        .with_integration(std::sync::Arc::new(mock_risk_engine))
        .with_integration(std::sync::Arc::new(mock_document_store))
        .with_integration(std::sync::Arc::new(mock_crs_compliance))
        .with_rule(std::sync::Arc::new(ComplianceWorkflowRule::kyc_workflow()))
        .with_rule(std::sync::Arc::new(OwnershipLimitsRule::new()))
        .build()
        .await?;

    Ok(engine)
}

async fn demo_single_operation(engine: &ComprehensiveDslEngine) -> Result<()> {
    println!("Executing single email validation operation...");

    let context = ExecutionContextBuilder::new()
        .with_business_unit_id("SINGLE-001")
        .with_domain("demo")
        .with_executor("demo_user")
        .with_compliance_mode(&[ComplianceType::PII])
        .build()?;

    let operation = ExecutableDslOperation {
        operation_type: "validate".to_string(),
        parameters: {
            let mut params = HashMap::new();
            params.insert(
                "attribute_id".to_string(),
                serde_json::to_value(AttributeId::new())?,
            );
            params.insert("value".to_string(), json!("john.doe@zenithcapital.com"));
            params
        },
        metadata: HashMap::new(),
    };

    let result = engine.execute_operation(operation, context).await?;

    println!("  ✅ Operation executed successfully: {}", result.success);
    println!("  📊 New state version: {}", result.new_state.version);
    println!("  💬 Messages: {}", result.messages.len());
    for message in &result.messages {
        println!(
            "    - {}: {}",
            format!("{:?}", message.level),
            message.message
        );
    }
    println!("  ⏱️ Duration: {}ms", result.duration_ms);

    Ok(())
}

async fn demo_batch_operations(engine: &ComprehensiveDslEngine) -> Result<()> {
    println!("Executing batch of onboarding operations...");

    let context = ExecutionContextBuilder::new()
        .with_business_unit_id("BATCH-001")
        .with_domain("onboarding")
        .with_executor("onboarding_specialist")
        .with_compliance_mode(&[ComplianceType::PII, ComplianceType::SOX])
        .with_integration("risk_engine")
        .with_integration("document_store")
        .build()?;

    let operations = vec![
        // Step 1: Validate customer email
        ExecutableDslOperation {
            operation_type: "validate".to_string(),
            parameters: {
                let mut params = HashMap::new();
                params.insert("attribute_id".to_string(), json!(AttributeId::new()));
                params.insert("value".to_string(), json!("customer@example.com"));
                params
            },
            metadata: HashMap::new(),
        },
        // Step 2: Collect risk assessment
        ExecutableDslOperation {
            operation_type: "collect".to_string(),
            parameters: {
                let mut params = HashMap::new();
                params.insert("attribute_id".to_string(), json!(AttributeId::new()));
                params.insert("from".to_string(), json!("risk-engine"));
                params
            },
            metadata: HashMap::new(),
        },
        // Step 3: Collect documents
        ExecutableDslOperation {
            operation_type: "collect".to_string(),
            parameters: {
                let mut params = HashMap::new();
                params.insert("attribute_id".to_string(), json!(AttributeId::new()));
                params.insert("from".to_string(), json!("document-store"));
                params.insert(
                    "document_type".to_string(),
                    json!("certificate_of_incorporation"),
                );
                params
            },
            metadata: HashMap::new(),
        },
    ];

    let batch_result = engine.execute_batch(operations, context).await?;

    println!("  ✅ Batch execution completed");
    println!("  📊 Total operations: {}", batch_result.total_operations);
    println!(
        "  ✅ Successful operations: {}",
        batch_result.successful_operations
    );
    println!("  ⏱️ Total duration: {}ms", batch_result.total_duration_ms);

    if let Some(failed_at) = batch_result.failed_at_operation {
        println!("  ❌ Failed at operation: {}", failed_at);
        if let Some(error) = &batch_result.error_message {
            println!("  🚨 Error: {}", error);
        }
    }

    Ok(())
}

async fn demo_kyc_workflow(engine: &ComprehensiveDslEngine) -> Result<()> {
    println!("Executing complete KYC workflow...");

    let kyc_operations = vec![
        // Declare the target entity
        ExecutableDslOperation {
            operation_type: "declare-entity".to_string(),
            parameters: {
                let mut params = HashMap::new();
                params.insert("node-id".to_string(), json!("zenith-capital-lp"));
                params.insert("label".to_string(), json!("Company"));
                params.insert(
                    "properties".to_string(),
                    json!({
                        "legal-name": "Zenith Capital Partners LP",
                        "registration-number": "KY-123456",
                        "jurisdiction": "KY",
                        "entity-type": "Limited Partnership"
                    }),
                );
                params
            },
            metadata: HashMap::new(),
        },
        // Validate primary contact email
        ExecutableDslOperation {
            operation_type: "validate".to_string(),
            parameters: {
                let mut params = HashMap::new();
                params.insert("attribute_id".to_string(), json!(AttributeId::new()));
                params.insert("value".to_string(), json!("compliance@zenithcapital.com"));
                params
            },
            metadata: HashMap::new(),
        },
        // Collect risk rating from risk engine
        ExecutableDslOperation {
            operation_type: "collect".to_string(),
            parameters: {
                let mut params = HashMap::new();
                params.insert("attribute_id".to_string(), json!(AttributeId::new()));
                params.insert("from".to_string(), json!("risk-engine"));
                params
            },
            metadata: HashMap::new(),
        },
        // Check FATCA status
        ExecutableDslOperation {
            operation_type: "check".to_string(),
            parameters: {
                let mut params = HashMap::new();
                params.insert("attribute_id".to_string(), json!(AttributeId::new()));
                params.insert("condition".to_string(), json!("equals"));
                params.insert("equals".to_string(), json!("US_PERSON"));
                params
            },
            metadata: HashMap::new(),
        },
    ];

    let workflow_result = engine
        .execute_workflow(
            WorkflowType::KYC,
            "KYC-ZENITH-001".to_string(),
            "kyc_analyst".to_string(),
            kyc_operations,
        )
        .await?;

    println!("  ✅ KYC workflow completed");
    println!("  🏢 Business unit: {}", workflow_result.business_unit_id);
    println!(
        "  📊 Workflow status: {:?}",
        workflow_result.workflow_status
    );
    println!(
        "  🔄 Final state version: {}",
        workflow_result.final_state.version
    );
    println!(
        "  📝 Total operations in state: {}",
        workflow_result.final_state.operations.len()
    );

    // Print the accumulated DSL document
    println!("\n  📄 Accumulated DSL Document:");
    let dsl_document = workflow_result.final_state.to_dsl_document();
    for (i, line) in dsl_document.lines().take(10).enumerate() {
        println!("    {}: {}", i + 1, line);
    }
    if dsl_document.lines().count() > 10 {
        println!("    ... ({} more lines)", dsl_document.lines().count() - 10);
    }

    Ok(())
}

async fn demo_ubo_workflow(engine: &ComprehensiveDslEngine) -> Result<()> {
    println!("Executing UBO discovery workflow...");

    let ubo_operations = vec![
        // Declare the main company
        ExecutableDslOperation {
            operation_type: "declare-entity".to_string(),
            parameters: {
                let mut params = HashMap::new();
                params.insert("node-id".to_string(), json!("zenith-spv-001"));
                params.insert("label".to_string(), json!("Company"));
                params.insert(
                    "properties".to_string(),
                    json!({
                        "legal-name": "Zenith Capital SPV I Ltd",
                        "jurisdiction": "KY",
                        "entity-type": "Special Purpose Vehicle"
                    }),
                );
                params
            },
            metadata: HashMap::new(),
        },
        // Declare parent company
        ExecutableDslOperation {
            operation_type: "declare-entity".to_string(),
            parameters: {
                let mut params = HashMap::new();
                params.insert("node-id".to_string(), json!("alpha-holdings-sg"));
                params.insert("label".to_string(), json!("Company"));
                params.insert(
                    "properties".to_string(),
                    json!({
                        "legal-name": "Alpha Holdings Pte Ltd",
                        "jurisdiction": "SG"
                    }),
                );
                params
            },
            metadata: HashMap::new(),
        },
        // Create ownership edge with evidence
        ExecutableDslOperation {
            operation_type: "create-edge".to_string(),
            parameters: {
                let mut params = HashMap::new();
                params.insert("from".to_string(), json!("alpha-holdings-sg"));
                params.insert("to".to_string(), json!("zenith-spv-001"));
                params.insert("type".to_string(), json!("HAS_OWNERSHIP"));
                params.insert(
                    "properties".to_string(),
                    json!({
                        "percent": 65.0,
                        "share-class": "Ordinary Shares",
                        "voting-rights": true
                    }),
                );
                params.insert("evidenced-by".to_string(), json!(["doc-cert-zenith-001"]));
                params
            },
            metadata: HashMap::new(),
        },
    ];

    let workflow_result = engine
        .execute_workflow(
            WorkflowType::UBO,
            "UBO-ZENITH-001".to_string(),
            "ubo_analyst".to_string(),
            ubo_operations,
        )
        .await?;

    println!("  ✅ UBO workflow completed");
    println!("  🏢 Business unit: {}", workflow_result.business_unit_id);
    println!(
        "  📊 Workflow status: {:?}",
        workflow_result.workflow_status
    );
    println!(
        "  🔗 Ownership relationships established: {}",
        workflow_result
            .final_state
            .operations
            .iter()
            .filter(|op| op.operation_type == "create-edge")
            .count()
    );

    Ok(())
}

async fn demo_business_rules(engine: &ComprehensiveDslEngine) -> Result<()> {
    println!("Demonstrating business rule validation...");

    let context = ExecutionContextBuilder::new()
        .with_business_unit_id("RULES-001")
        .with_domain("ubo")
        .with_executor("compliance_officer")
        .build()?;

    // This should fail ownership limits rule (over 100%)
    let invalid_operation = ExecutableDslOperation {
        operation_type: "create-edge".to_string(),
        parameters: {
            let mut params = HashMap::new();
            params.insert("from".to_string(), json!("owner-entity"));
            params.insert("to".to_string(), json!("target-entity"));
            params.insert("type".to_string(), json!("HAS_OWNERSHIP"));
            params.insert(
                "properties".to_string(),
                json!({
                    "percent": 150.0  // This will trigger business rule violation
                }),
            );
            params
        },
        metadata: HashMap::new(),
    };

    println!("  Testing ownership limits rule with 150% ownership...");
    let result = engine
        .execute_operation(invalid_operation, context.clone())
        .await?;

    println!("  ❌ Operation failed as expected: {}", !result.success);
    println!("  📝 Validation messages:");
    for message in &result.messages {
        if matches!(message.level, ob_poc::execution::MessageLevel::Error) {
            println!("    🚨 ERROR: {}", message.message);
        } else {
            println!(
                "    ⚠️  {}: {}",
                format!("{:?}", message.level),
                message.message
            );
        }
    }

    // Now try with valid ownership percentage
    println!("\n  Testing with valid 45% ownership...");
    let valid_operation = ExecutableDslOperation {
        operation_type: "create-edge".to_string(),
        parameters: {
            let mut params = HashMap::new();
            params.insert("from".to_string(), json!("owner-entity"));
            params.insert("to".to_string(), json!("target-entity-2"));
            params.insert("type".to_string(), json!("HAS_OWNERSHIP"));
            params.insert(
                "properties".to_string(),
                json!({
                    "percent": 45.0
                }),
            );
            params
        },
        metadata: HashMap::new(),
    };

    let valid_result = engine.execute_operation(valid_operation, context).await?;
    println!("  ✅ Operation succeeded: {}", valid_result.success);

    Ok(())
}

async fn demo_state_management(engine: &ComprehensiveDslEngine) -> Result<()> {
    println!("Demonstrating state management and history...");

    let business_unit_id = "STATE-DEMO-001";

    // Show initial empty state
    let initial_state = engine.get_current_state(business_unit_id).await?;
    println!("  📊 Initial state version: {}", initial_state.version);
    println!(
        "  📝 Initial operations count: {}",
        initial_state.operations.len()
    );

    // Execute several operations to build up state
    let context = ExecutionContextBuilder::new()
        .with_business_unit_id(business_unit_id)
        .with_domain("demo")
        .with_executor("state_demo_user")
        .build()?;

    for i in 1..=3 {
        let operation = ExecutableDslOperation {
            operation_type: "validate".to_string(),
            parameters: {
                let mut params = HashMap::new();
                params.insert("attribute_id".to_string(), json!(AttributeId::new()));
                params.insert("value".to_string(), json!(format!("test{}@example.com", i)));
                params
            },
            metadata: {
                let mut meta = HashMap::new();
                meta.insert("step".to_string(), json!(i));
                meta
            },
        };

        engine.execute_operation(operation, context.clone()).await?;
        println!("  ⏭️  Executed operation {}", i);
    }

    // Show final state
    let final_state = engine.get_current_state(business_unit_id).await?;
    println!("  📊 Final state version: {}", final_state.version);
    println!(
        "  📝 Final operations count: {}",
        final_state.operations.len()
    );
    println!(
        "  🗂️  Current state attributes: {}",
        final_state.current_state.len()
    );

    // Show state metadata
    println!("  📋 State metadata:");
    println!("    Created: {}", final_state.metadata.created_at);
    println!("    Updated: {}", final_state.metadata.updated_at);
    println!("    Domain: {}", final_state.metadata.domain);
    println!("    Status: {}", final_state.metadata.status);

    // Demonstrate the DSL-as-State concept
    println!("\n  📄 Complete DSL Document (State as DSL):");
    let dsl_document = final_state.to_dsl_document();
    for (i, line) in dsl_document.lines().enumerate() {
        if !line.trim().is_empty() {
            println!("    {}: {}", i + 1, line);
        }
    }

    Ok(())
}
