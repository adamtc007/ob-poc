//! Business Request Lifecycle Demo
//!
//! This example demonstrates the complete business request lifecycle management
//! system implemented in DSL Manager V3. It shows how to:
//!
//! 1. Create new business requests (KYC.Case, Onboarding.request, Account.Opening)
//! 2. Manage DSL amendments throughout the request lifecycle
//! 3. Track workflow state progression
//! 4. Build business-context-aware visualizations
//! 5. Perform lifecycle analytics and reporting
//!
//! This represents the complete business context that was missing from the original
//! DSL architecture - each DSL instance now has proper business request context.

use ob_poc::database::{DslBusinessRequestRepository, DslDomainRepository};
use ob_poc::dsl_manager_v3::DslManagerV3;
use ob_poc::models::business_request_models::*;
use sqlx::PgPool;
use std::env;
use tracing::{info, Level};
use tracing_subscriber;

#[tokio::main]
async fn main() -> Result<(), Box<dyn std::error::Error>> {
    // Initialize logging
    tracing_subscriber::fmt().with_max_level(Level::INFO).init();

    info!("🚀 Business Request Lifecycle Demo");
    info!("=====================================");
    info!("");

    // Try to connect to database, fall back to mock mode if unavailable
    let database_url = env::var("DATABASE_URL")
        .unwrap_or_else(|_| "postgresql://localhost:5432/ob-poc".to_string());

    info!("🔌 Connecting to database...");
    let pool_result = PgPool::connect(&database_url).await;

    match pool_result {
        Ok(pool) => {
            info!("   ✅ Database connection successful");
            let domain_repository = DslDomainRepository::new(pool.clone());
            let business_repository = DslBusinessRequestRepository::new(pool);
            let manager = DslManagerV3::new(domain_repository, business_repository);
            run_database_demo(&manager).await
        }
        Err(e) => {
            info!("   ⚠️  Database connection failed: {}", e);
            info!("   📝 Running comprehensive mock demonstration");
            run_mock_demonstration().await
        }
    }
}

/// Run demonstration with database connectivity
async fn run_database_demo(manager: &DslManagerV3) -> Result<(), Box<dyn std::error::Error>> {
    info!("🎯 Business Request Lifecycle: Database-Connected Demo");
    info!("====================================================");

    // Demo 1: Create KYC Case
    demo_1_create_kyc_case(manager).await?;

    // Demo 2: Create Onboarding Request
    demo_2_create_onboarding_request(manager).await?;

    // Demo 3: Create Account Opening
    demo_3_create_account_opening(manager).await?;

    // Demo 4: DSL Amendment Lifecycle
    demo_4_dsl_amendment_lifecycle(manager).await?;

    // Demo 5: Workflow State Management
    demo_5_workflow_state_management(manager).await?;

    // Demo 6: Business Request Analytics
    demo_6_business_request_analytics(manager).await?;

    // Demo 7: Visualization with Business Context
    demo_7_business_context_visualization(manager).await?;

    info!("🎉 Database-connected lifecycle demo completed!");
    Ok(())
}

async fn demo_1_create_kyc_case(manager: &DslManagerV3) -> Result<(), Box<dyn std::error::Error>> {
    info!("📊 Demo 1: Creating KYC Case with Business Context");
    info!("--------------------------------------------------");

    // Create a new KYC case
    let kyc_case = manager
        .create_kyc_case(
            "KYC-2024-001-MERIDIAN".to_string(),
            "CLIENT-MERIDIAN-FUND".to_string(),
            "analyst@bank.com".to_string(),
            Some(
                r#"
DOMAIN KYC
STATE collecting_documents

WORKFLOW "KYC Investigation - Meridian Global Fund"
    PROPERTIES {
        entity_type: "fund",
        jurisdiction: "LU",
        risk_level: "medium",
        regulatory_framework: "UCITS"
    }

BEGIN
    DECLARE_ENTITY "meridian_global_fund" {
        legal_name: "Meridian Global Fund SICAV",
        entity_type: "investment_fund",
        jurisdiction: "Luxembourg",
        registration_number: "LU1234567890"
    }

    SOLICIT_ATTRIBUTE "fund_documents" FROM "meridian_global_fund" {
        document_types: ["prospectus", "articles_of_incorporation", "management_agreement"],
        priority: "high",
        deadline: "5_business_days"
    }

    CALCULATE_UBO "meridian_global_fund" {
        threshold: 25.0,
        method: "fund_structure_analysis",
        include_management_company: true
    }
END
"#,
            ),
        )
        .await?;

    info!("✅ Created KYC case:");
    info!("   Request ID: {}", kyc_case.request_id);
    info!("   Business Reference: {}", kyc_case.business_reference);
    info!("   Client ID: {:?}", kyc_case.client_id);
    info!("   Status: {:?}", kyc_case.request_status);
    info!("   Created By: {}", kyc_case.created_by);
    info!("   Created At: {}", kyc_case.created_at);

    Ok(())
}

async fn demo_2_create_onboarding_request(
    manager: &DslManagerV3,
) -> Result<(), Box<dyn std::error::Error>> {
    info!("🎯 Demo 2: Creating Onboarding Request");
    info!("--------------------------------------");

    // Create a new onboarding request
    let onboarding_request = manager
        .create_onboarding_request(
            "ONB-2024-002-TECHCORP".to_string(),
            "CLIENT-TECHCORP-LLC".to_string(),
            "onboarding@bank.com".to_string(),
            Some(
                r#"
DOMAIN Onboarding
STATE identity_verification

WORKFLOW "Corporate Client Onboarding - TechCorp LLC"
    PROPERTIES {
        client_type: "corporate",
        channel: "relationship_manager",
        product_suite: ["checking", "credit_facility", "fx_services"]
    }

BEGIN
    DECLARE_ENTITY "techcorp_llc" {
        legal_name: "TechCorp LLC",
        entity_type: "limited_liability_company",
        formation_state: "Delaware",
        business_purpose: "software_development"
    }

    PARALLEL_OBTAIN {
        BRANCH "identity_verification" {
            SOLICIT_ATTRIBUTE "corporate_identity" FROM "techcorp_llc" {
                document_types: ["articles_of_organization", "operating_agreement"],
                verification_method: "secretary_of_state_database"
            }
        }

        BRANCH "beneficial_ownership" {
            SOLICIT_ATTRIBUTE "ownership_disclosure" FROM "techcorp_llc" {
                beneficial_owner_threshold: 25.0,
                certification_required: true
            }
        }
    }

    RESOLVE_CONFLICT "risk_assessment" {
        resolution_method: "automated_screening",
        escalation_rules: ["sanctions_hit", "adverse_media"]
    }
END
"#,
            ),
        )
        .await?;

    info!("✅ Created Onboarding request:");
    info!("   Request ID: {}", onboarding_request.request_id);
    info!(
        "   Business Reference: {}",
        onboarding_request.business_reference
    );
    info!("   Request Type: {}", onboarding_request.request_type);
    info!("   Status: {:?}", onboarding_request.request_status);

    Ok(())
}

async fn demo_3_create_account_opening(
    manager: &DslManagerV3,
) -> Result<(), Box<dyn std::error::Error>> {
    info!("🏦 Demo 3: Creating Account Opening Request");
    info!("-------------------------------------------");

    // Create a new account opening request
    let account_opening = manager
        .create_account_opening(
            "ACT-2024-003-GLOBALBANK".to_string(),
            "CLIENT-GLOBALBANK-CORP".to_string(),
            "accounts@bank.com".to_string(),
            Some(
                r#"
DOMAIN Account_Opening
STATE application_review

WORKFLOW "Premium Business Account Opening"
    PROPERTIES {
        account_type: "premium_business",
        initial_deposit: 50000.00,
        currency: "USD",
        regulatory_tier: "tier_1"
    }

BEGIN
    DECLARE_ENTITY "globalbank_corp" {
        legal_name: "GlobalBank Corporation",
        entity_type: "corporation",
        industry: "financial_services",
        annual_revenue: 500000000.00
    }

    SOLICIT_ATTRIBUTE "financial_statements" FROM "globalbank_corp" {
        document_types: ["audited_financials", "tax_returns"],
        years_required: 3,
        audit_firm_required: true
    }

    CALCULATE_UBO "globalbank_corp" {
        threshold: 10.0,
        enhanced_due_diligence: true,
        pep_screening: true
    }

    GENERATE_REPORT "account_opening_summary" {
        template: "premium_account_approval",
        approval_required: true,
        signatory_level: "senior_vice_president"
    }
END
"#,
            ),
        )
        .await?;

    info!("✅ Created Account Opening request:");
    info!("   Request ID: {}", account_opening.request_id);
    info!(
        "   Business Reference: {}",
        account_opening.business_reference
    );
    info!("   Request Type: {}", account_opening.request_type);
    info!("   Priority: {:?}", account_opening.priority_level);

    Ok(())
}

async fn demo_4_dsl_amendment_lifecycle(
    manager: &DslManagerV3,
) -> Result<(), Box<dyn std::error::Error>> {
    info!("📝 Demo 4: DSL Amendment Lifecycle");
    info!("----------------------------------");

    // First, get an existing request to amend
    let requests = manager
        .list_business_requests(Some("KYC"), None, None, Some(1), None)
        .await?;

    if let Some(request) = requests.first() {
        info!("📋 Amending KYC request: {}", request.business_reference);

        // Create first amendment
        let version_1 = manager
            .create_dsl_amendment(
                &request.request_id,
                r#"
DOMAIN KYC
STATE document_review

WORKFLOW "KYC Investigation - Meridian Global Fund (Updated)"
    PROPERTIES {
        entity_type: "fund",
        jurisdiction: "LU",
        risk_level: "high",  // Updated from medium
        regulatory_framework: "UCITS",
        updated_reason: "elevated_risk_indicators"
    }

BEGIN
    DECLARE_ENTITY "meridian_global_fund" {
        legal_name: "Meridian Global Fund SICAV",
        entity_type: "investment_fund",
        jurisdiction: "Luxembourg",
        registration_number: "LU1234567890"
    }

    # Enhanced document collection due to elevated risk
    SOLICIT_ATTRIBUTE "enhanced_fund_documents" FROM "meridian_global_fund" {
        document_types: ["prospectus", "articles_of_incorporation", "management_agreement", "audit_report", "risk_management_policy"],
        priority: "critical",
        deadline: "3_business_days"
    }

    # Additional UBO analysis
    CALCULATE_UBO "meridian_global_fund" {
        threshold: 10.0,  // Lowered threshold for enhanced scrutiny
        method: "enhanced_fund_structure_analysis",
        include_management_company: true,
        include_depositary: true,
        ultimate_parent_analysis: true
    }

    # Add compliance review step
    RESOLVE_CONFLICT "enhanced_compliance_review" {
        resolution_method: "senior_analyst_review",
        approval_level: "compliance_officer"
    }
END
"#,
                Some("document_review"),
                Some("Enhanced KYC analysis due to elevated risk indicators"),
                "senior.analyst@bank.com",
            )
            .await?;

        info!("✅ Created DSL amendment version: {}", version_1);

        // Compile the amended DSL
        let compiled_ast = manager
            .compile_request_dsl(&request.request_id, &version_1)
            .await?;

        info!("✅ Compiled DSL amendment:");
        info!("   AST ID: {}", compiled_ast.ast_id);
        info!("   Grammar Version: {}", compiled_ast.grammar_version);
        info!("   Parsed At: {}", compiled_ast.parsed_at);

        // Create second amendment with final review
        let version_2 = manager
            .create_dsl_amendment(
                &request.request_id,
                r#"
DOMAIN KYC
STATE final_review

WORKFLOW "KYC Investigation - Meridian Global Fund (Final)"
    PROPERTIES {
        entity_type: "fund",
        jurisdiction: "LU",
        risk_level: "high",
        regulatory_framework: "UCITS",
        review_status: "ready_for_approval",
        final_recommendation: "approve_with_enhanced_monitoring"
    }

BEGIN
    DECLARE_ENTITY "meridian_global_fund" {
        legal_name: "Meridian Global Fund SICAV",
        entity_type: "investment_fund",
        jurisdiction: "Luxembourg",
        registration_number: "LU1234567890"
    }

    # All documents collected and verified
    SOLICIT_ATTRIBUTE "verified_documents" FROM "meridian_global_fund" {
        document_types: ["prospectus", "articles_of_incorporation", "management_agreement", "audit_report", "risk_management_policy"],
        verification_status: "completed",
        verification_date: "2024-12-01"
    }

    # UBO analysis completed
    CALCULATE_UBO "meridian_global_fund" {
        threshold: 10.0,
        analysis_status: "completed",
        ultimate_beneficial_owners_identified: true,
        risk_assessment: "acceptable_with_monitoring"
    }

    # Final approval workflow
    GENERATE_REPORT "kyc_final_report" {
        template: "kyc_approval_recommendation",
        recommendation: "approve_with_enhanced_monitoring",
        monitoring_frequency: "quarterly",
        approval_required: true
    }

    SCHEDULE_MONITORING "ongoing_kyc_monitoring" {
        frequency: "quarterly",
        trigger_events: ["ownership_change", "material_adverse_change"],
        review_type: "enhanced"
    }
END
"#,
                Some("final_review"),
                Some("Final KYC analysis with approval recommendation"),
                "compliance.officer@bank.com",
            )
            .await?;

        info!("✅ Created final DSL amendment version: {}", version_2);

        // Get request summary after amendments
        let summary = manager
            .get_business_request_summary(&request.request_id)
            .await?;

        if let Some(summary) = summary {
            info!("📊 Request Summary After Amendments:");
            info!("   Total Versions: {}", summary.total_versions);
            info!("   Latest Version: {}", summary.latest_version_number);
            info!("   Status: {:?}", summary.request_status);
            info!(
                "   Current Workflow State: {:?}",
                summary.current_workflow_state
            );
        }
    } else {
        info!("⚠️  No KYC requests found to demonstrate amendments");
    }

    Ok(())
}

async fn demo_5_workflow_state_management(
    manager: &DslManagerV3,
) -> Result<(), Box<dyn std::error::Error>> {
    info!("🔄 Demo 5: Workflow State Management");
    info!("-----------------------------------");

    // Get an existing request to demonstrate workflow transitions
    let requests = manager
        .list_business_requests(None, None, None, Some(1), None)
        .await?;

    if let Some(request) = requests.first() {
        info!(
            "🔍 Managing workflow states for: {}",
            request.business_reference
        );

        // Get current workflow state
        let current_state = manager
            .get_current_workflow_state(&request.request_id)
            .await?;

        info!("📍 Current State:");
        if let Some(state) = current_state {
            info!("   State: {}", state.workflow_state);
            info!("   Description: {:?}", state.state_description);
            info!("   Entered At: {}", state.entered_at);
            info!("   Entered By: {}", state.entered_by);
            info!("   Duration: {:.1} hours", state.duration_in_hours());
        } else {
            info!("   No current state found");
        }

        // Demonstrate state transitions
        info!("🔄 Performing state transitions...");

        // Transition to review state
        let review_state = manager
            .transition_workflow_state(
                &request.request_id,
                "compliance_review",
                Some("Moving to compliance review phase"),
                "workflow.manager@bank.com",
                Some(serde_json::json!({
                    "reviewer_assigned": "senior.compliance@bank.com",
                    "review_priority": "high",
                    "expected_duration_hours": 24
                })),
            )
            .await?;

        info!("✅ Transitioned to review state:");
        info!("   New State: {}", review_state.workflow_state);
        info!("   State ID: {}", review_state.state_id);

        // Transition to approved state
        let approved_state = manager
            .transition_workflow_state(
                &request.request_id,
                "approved",
                Some("Compliance review completed - approved"),
                "senior.compliance@bank.com",
                Some(serde_json::json!({
                    "approval_date": "2024-12-01",
                    "approval_conditions": ["enhanced_monitoring", "quarterly_review"],
                    "next_review_date": "2025-03-01"
                })),
            )
            .await?;

        info!("✅ Transitioned to approved state:");
        info!("   New State: {}", approved_state.workflow_state);
        info!("   Approval Conditions: Applied");

        // Get workflow history
        let history = manager.get_workflow_history(&request.request_id).await?;

        info!("📋 Complete Workflow History ({} states):", history.len());
        for (i, entry) in history.iter().enumerate() {
            let status_icon = if entry.is_current_state {
                "🔄"
            } else {
                "✅"
            };

            info!(
                "   {}. {} {} ({:.1}h) - {}",
                i + 1,
                status_icon,
                entry.workflow_state,
                entry.hours_in_state,
                entry.entered_by
            );

            if let Some(desc) = &entry.state_description {
                info!("      Description: {}", desc);
            }
        }
    } else {
        info!("⚠️  No requests found to demonstrate workflow management");
    }

    Ok(())
}

async fn demo_6_business_request_analytics(
    manager: &DslManagerV3,
) -> Result<(), Box<dyn std::error::Error>> {
    info!("📈 Demo 6: Business Request Analytics");
    info!("------------------------------------");

    // Get domain statistics
    let domains = ["KYC", "Onboarding", "Account_Opening"];

    for domain in domains {
        info!("📊 {} Domain Statistics (30 days):", domain);

        let stats = manager
            .get_domain_request_statistics(domain, Some(30))
            .await?;

        info!("   Total Requests: {}", stats.total_requests);
        info!("   Draft: {}", stats.draft_requests);
        info!("   In Progress: {}", stats.in_progress_requests);
        info!("   Completed: {}", stats.completed_requests);
        info!("   Critical Priority: {}", stats.critical_requests);

        if let Some(avg_hours) = stats.avg_completion_hours {
            info!("   Avg Completion Time: {:.1} hours", avg_hours);
        }

        // Calculate efficiency metrics
        let completion_rate = if stats.total_requests > 0 {
            (stats.completed_requests as f64 / stats.total_requests as f64) * 100.0
        } else {
            0.0
        };

        let critical_rate = if stats.total_requests > 0 {
            (stats.critical_requests as f64 / stats.total_requests as f64) * 100.0
        } else {
            0.0
        };

        info!("   Completion Rate: {:.1}%", completion_rate);
        info!("   Critical Rate: {:.1}%", critical_rate);
        info!("");
    }

    // List active requests by status
    info!("🔍 Active Requests by Status:");

    let active_requests = manager
        .list_business_requests(None, Some(RequestStatus::InProgress), None, Some(10), None)
        .await?;

    info!("   In Progress: {} requests", active_requests.len());
    for request in &active_requests {
        info!(
            "      • {} - {} ({})",
            request.business_reference,
            request.request_type,
            request
                .current_workflow_state
                .as_deref()
                .unwrap_or("No state")
        );
    }

    let review_requests = manager
        .list_business_requests(None, Some(RequestStatus::Review), None, Some(10), None)
        .await?;

    info!("   In Review: {} requests", review_requests.len());
    for request in &review_requests {
        info!(
            "      • {} - {} ({})",
            request.business_reference,
            request.request_type,
            request.assigned_to.as_deref().unwrap_or("Unassigned")
        );
    }

    // Get available request types
    let request_types = manager.list_request_types().await?;
    info!("📋 Available Request Types: {}", request_types.len());
    for req_type in &request_types {
        info!("   • {} - {}", req_type.request_type, req_type.display_name);
        if let Some(duration) = req_type.estimated_duration_hours {
            info!("     Estimated Duration: {} hours", duration);
        }
        info!(
            "     Requires Approval: {}",
            if req_type.requires_approval {
                "Yes"
            } else {
                "No"
            }
        );
    }

    Ok(())
}

async fn demo_7_business_context_visualization(
    manager: &DslManagerV3,
) -> Result<(), Box<dyn std::error::Error>> {
    info!("🎨 Demo 7: Business Context Visualization");
    info!("----------------------------------------");

    // Get a request to visualize
    let requests = manager
        .list_business_requests(None, None, None, Some(1), None)
        .await?;

    if let Some(request) = requests.first() {
        info!(
            "🖼️  Building visualization for: {}",
            request.business_reference
        );

        // Build business request visualization
        let business_viz = manager
            .build_business_request_visualization(&request.request_id, None, None)
            .await?;

        info!("✅ Business Request Visualization Created:");
        info!("   Request ID: {}", business_viz.request_id);
        info!("   Business Reference: {}", business_viz.business_reference);
        info!("   Request Type: {}", business_viz.request_type);
        info!("   Status: {:?}", business_viz.request_status);
        info!("   Version ID: {}", business_viz.version_id);

        // Display domain visualization details
        let domain_viz = &business_viz.domain_enhanced_visualization;
        info!("   🎨 Domain Enhancement Details:");
        info!(
            "      Domain: {}",
            domain_viz
                .domain_context
                .as_ref()
                .map(|d| d.domain_name.as_str())
                .unwrap_or("Unknown")
        );
        info!("      Enhanced Nodes: {}", domain_viz.enhanced_root_node.id);
        info!("      Enhanced Edges: {}", domain_viz.enhanced_edges.len());
        info!(
            "      Domain Highlights: {}",
            domain_viz.domain_specific_highlights.len()
        );

        // Display domain-specific highlights
        if !domain_viz.domain_specific_highlights.is_empty() {
            info!("      🎯 Domain-Specific Highlights:");
            for highlight in &domain_viz.domain_specific_highlights {
                let priority_icon = match highlight.priority {
                    crate::domain_visualizations::HighlightPriority::Critical => "🚨",
                    crate::domain_visualizations::HighlightPriority::High => "🔴",
                    crate::domain_visualizations::HighlightPriority::Medium => "🟡",
                    crate::domain_visualizations::HighlightPriority::Low => "🟢",
                };
                info!(
                    "         {} {}: {}",
                    priority_icon, highlight.highlight_type, highlight.description
                );
            }
        }

        // Display workflow state information
        if let Some(workflow_state) = &business_viz.current_workflow_state {
            info!("   🔄 Current Workflow State:");
            info!("      State: {}", workflow_state.workflow_state);
            info!(
                "      Description: {}",
                workflow_state
                    .state_description
                    .as_deref()
                    .unwrap_or("No description")
            );
            info!(
                "      Duration: {:.1} hours",
                workflow_state.duration_in_hours()
            );
            info!(
                "      Requires Approval: {}",
                if workflow_state.requires_approval {
                    "Yes"
                } else {
                    "No"
                }
            );
        }

        // Display request summary
        if let Some(summary) = &business_viz.request_summary {
            info!("   📊 Request Summary:");
            info!("      Total Versions: {}", summary.total_versions);
            info!("      Latest Version: {}", summary.latest_version_number);
            info!("      Created: {}", summary.created_at);
            info!("      Last Updated: {}", summary.last_updated);
        }

        // Display domain metrics
        if let Some(metrics) = domain_viz.domain_metrics.as_ref() {
            info!("   📈 Domain Metrics:");
            info!("      Entity Count: {}", metrics.entity_count);
            info!("      Relationship Count: {}", metrics.relationship_count);
            info!("      UBO Calculations: {}", metrics.ubo_calculations);
            info!(
                "      Compliance Operations: {}",
                metrics.compliance_operations
            );
            info!("      Risk Score: {}", metrics.risk_score);
            info!("      Complexity Score: {}", metrics.complexity_score);
            info!(
                "      Estimated Execution Time: {}ms",
                metrics.estimated_execution_time
            );
        }

        info!("✅ Visualization demonstrates complete business context integration");
    } else {
        info!("⚠️  No requests found to demonstrate visualization");
    }

    Ok(())
}

/// Run comprehensive mock demonstration showing all capabilities
async fn run_mock_demonstration() -> Result<(), Box<dyn std::error::Error>> {
    info!("🎭 Business Request Lifecycle: Mock Demonstration");
    info!("=================================================");

    // Demo 1: Business Request Creation Concepts
    demonstrate_business_request_concepts().await?;

    // Demo 2: DSL Amendment Lifecycle Concepts
    demonstrate_dsl_amendment_concepts().await?;

    // Demo 3: Workflow State Management Concepts
    demonstrate_workflow_management_concepts().await?;

    // Demo 4: Business Analytics Concepts
    demonstrate_business_analytics_concepts().await?;

    // Demo 5: Integration Benefits
    demonstrate_integration_benefits().await?;

    info!("🎉 Comprehensive mock demonstration completed successfully!");
    info!("");
    info!("💡 To see full interactive features, connect to a PostgreSQL database");
    info!("   with the business request lifecycle tables configured.");

    Ok(())
}

async fn demonstrate_business_request_concepts() -> Result<(), Box<dyn std::error::Error>> {
    info!("🏗️  Demo 1: Business Request Creation Concepts");
    info!("----------------------------------------------");

    info!("💼 Business Request Types Available:");
    info!("   • KYC_CASE - Know Your Customer investigations");
    info!("     - Creates: KYC-YYYY-NNN-REFERENCE format IDs");
    info!("     - Tracks: Document collection, UBO analysis, compliance review");
    info!("     - Lifecycle: Draft → Collecting → Analysis → Review → Approved → Completed");
    info!("");

    info!("   • ONBOARDING_REQUEST - Customer onboarding processes");
    info!("     - Creates: ONB-YYYY-NNN-REFERENCE format IDs");
    info!("     - Tracks: Identity verification, risk assessment, account setup");
    info!("     - Lifecycle: Draft → Verification → Assessment → Setup → Completed");
    info!("");

    info!("   • ACCOUNT_OPENING - Account opening applications");
    info!("     - Creates: ACT-YYYY-NNN-REFERENCE format IDs");
    info!("     - Tracks: Application review, documentation, approval workflow");
    info!("     - Lifecycle: Draft → Review → Documentation → Approval → Setup → Completed");
    info!("");

    info!("🔑 Key Business Context Features:");
    info!("   • Each request gets unique UUID for database referential integrity");
    info!("   • Business reference for human-readable identification");
    info!("   • Client ID linkage for customer relationship management");
    info!("   • Priority levels (LOW, NORMAL, HIGH, CRITICAL)");
    info!("   • Due date tracking and overdue monitoring");
    info!("   • Assignee and reviewer tracking");
    info!("   • Regulatory requirement specifications");
    info!("   • Business context JSON for flexible metadata");

    Ok(())
}

async fn demonstrate_dsl_amendment_concepts() -> Result<(), Box<dyn std::error::Error>> {
    info!("📝 Demo 2: DSL Amendment Lifecycle Concepts");
    info!("-------------------------------------------");

    info!("🔄 Amendment Workflow:");
    info!("   1. Initial DSL Creation:");
    info!("      • Business request created with request_id");
    info!("      • First DSL version (v1) linked to request_id");
    info!("      • Compilation creates AST with business context");
    info!("");

    info!("   2. DSL Amendments:");
    info!("      • All subsequent versions linked to same request_id");
    info!("      • Version numbers auto-increment (v2, v3, etc.)");
    info!("      • Change descriptions track evolution");
    info!("      • Functional state progression tracked");
    info!("");

    info!("   3. Business Request Lifecycle Preservation:");
    info!("      • request_id remains constant throughout");
    info!("      • Business context preserved across all versions");
    info!("      • Workflow state progresses with DSL changes");
    info!("      • Complete audit trail maintained");
    info!("");

    info!("📋 Example Amendment Sequence:");
    info!("   Request: KYC-2024-001-MERIDIAN (uuid: 123e4567-e89b-12d3-a456-426614174000)");
    info!("   • Version 1: Initial draft (functional_state: collecting_documents)");
    info!("   • Version 2: Enhanced analysis (functional_state: document_review)");
    info!("   • Version 3: Final approval (functional_state: final_review)");
    info!("   All versions share the same request_id - complete business continuity!");

    Ok(())
}

async fn demonstrate_workflow_management_concepts() -> Result<(), Box<dyn std::error::Error>> {
    info!("🔄 Demo 3: Workflow State Management Concepts");
    info!("---------------------------------------------");

    info!("📊 Workflow State Features:");
    info!("   • State Progression Tracking:");
    info!("     - Each state transition recorded with timestamp");
    info!("     - Previous state linked for full history");
    info!("     - Duration in each state calculated automatically");
    info!("     - Entered by user tracking for accountability");
    info!("");

    info!("   • State Metadata:");
    info!("     - JSON state_data for flexible context");
    info!("     - Automation trigger flags");
    info!("     - Approval requirement indicators");
    info!("     - Estimated duration tracking");
    info!("");

    info!("   • Business Integration:");
    info!("     - Workflow states auto-update request status");
    info!("     - DSL compilation triggers state transitions");
    info!("     - State data includes reviewer assignments");
    info!("     - Next possible states defined per workflow");
    info!("");

    info!("🎯 Example State Progressions:");
    info!("   KYC Workflow:");
    info!("   initial_draft → collecting_documents → ubo_analysis → compliance_review → approved → completed");
    info!("");
    info!("   Onboarding Workflow:");
    info!("   initial_draft → identity_verification → document_collection → risk_assessment → approved → completed");
    info!("");
    info!("   Account Opening Workflow:");
    info!("   initial_draft → application_review → document_verification → approval_workflow → account_setup → completed");

    Ok(())
}

async fn demonstrate_business_analytics_concepts() -> Result<(), Box<dyn std::error::Error>> {
    info!("📈 Demo 4: Business Analytics Concepts");
    info!("-------------------------------------");

    info!("🔍 Analytics Capabilities:");
    info!("   • Domain-Level Statistics:");
    info!("     - Total requests per domain");
    info!("     - Status distribution (Draft, In Progress, Review, etc.)");
    info!("     - Priority level analysis");
    info!("     - Average completion times");
    info!("     - Critical request identification");
    info!("");

    info!("   • Request Lifecycle Metrics:");
    info!("     - Time in each workflow state");
    info!("     - Amendment frequency per request");
    info!("     - Compilation success rates");
    info!("     - Approval/rejection ratios");
    info!("");

    info!("   • Performance Insights:");
    info!("     - Bottleneck identification by state duration");
    info!("     - User productivity metrics");
    info!("     - SLA compliance tracking");
    info!("     - Resource allocation optimization");
    info!("");

    info!("📊 Sample Analytics Output:");
    info!("   KYC Domain (30 days):");
    info!("   • Total Requests: 45");
    info!("   • Completed: 38 (84.4% completion rate)");
    info!("   • Average Time: 72.5 hours");
    info!("   • Critical: 3 (6.7% critical rate)");
    info!("   • Bottleneck: compliance_review (avg 18.5 hours)");

    Ok(())
}

async fn demonstrate_integration_benefits() -> Result<(), Box<dyn std::error::Error>> {
    info!("✨ Demo 5: Integration Benefits");
    info!("------------------------------");

    info!("🎯 Business Context Integration:");
    info!("   • Complete Traceability:");
    info!("     - Every DSL version linked to business request");
    info!("     - Full audit trail from creation to completion");
    info!("     - Client relationship context preserved");
    info!("     - Regulatory compliance documentation");
    info!("");

    info!("   • Operational Efficiency:");
    info!("     - No more orphaned DSL versions");
    info!("     - Clear ownership and accountability");
    info!("     - Automated workflow progression");
    info!("     - Business rule enforcement");
    info!("");

    info!("   • Enhanced Visualization:");
    info!("     - Domain-specific styling with business context");
    info!("     - Workflow state overlay on AST visualization");
    info!("     - Business metrics integrated with technical metrics");
    info!("     - Request timeline visualization");
    info!("");

    info!("🚀 Key Advantages:");
    info!("   ✅ Proper Business Lifecycle Management");
    info!("   ✅ Request ID serves as primary business key");
    info!("   ✅ All DSL amendments linked to business context");
    info!("   ✅ Workflow state progression tracking");
    info!("   ✅ Domain-aware visualization with business overlay");
    info!("   ✅ Complete analytics and reporting capabilities");
    info!("   ✅ Regulatory compliance and audit trail");
    info!("   ✅ Integration ready for Phase 5 web visualization");

    Ok(())
}
