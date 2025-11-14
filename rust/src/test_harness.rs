//! Test Harness with Canned Prompts for End-to-End Testing
//!
//! This module provides pre-defined test scenarios for validating
//! the agentic DSL CRUD system end-to-end.

use serde::{Deserialize, Serialize};
use uuid::Uuid;

/// Test scenario with multiple prompts
#[derive(Debug, Clone)]
pub struct TestScenario {
    pub name: String,
    pub description: String,
    pub prompts: Vec<String>,
}

/// Result of running a test scenario
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct TestResults {
    pub scenario_name: String,
    pub total_prompts: usize,
    pub successful: usize,
    pub failed: usize,
    pub details: Vec<TestDetail>,
}

/// Detail of a single prompt execution
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct TestDetail {
    pub prompt: String,
    pub success: bool,
    pub message: String,
    pub entity_id: Option<Uuid>,
}

/// Canned test scenarios
pub struct TestHarness;

impl TestHarness {
    /// Get all predefined test scenarios
    pub fn get_test_scenarios() -> Vec<TestScenario> {
        vec![
            // Scenario 1: Simple CBU Creation
            TestScenario {
                name: "Simple Hedge Fund CBU".to_string(),
                description: "Create a basic hedge fund CBU".to_string(),
                prompts: vec![
                    "CREATE CBU WITH nature_purpose \"Hedge Fund Management for High Net Worth Individuals\" AND source_of_funds \"Private Capital and Investment Returns\"".to_string(),
                ],
            },

            // Scenario 2: Complete Onboarding Flow
            TestScenario {
                name: "Complete Investment Bank Onboarding".to_string(),
                description: "Full onboarding with entities and roles".to_string(),
                prompts: vec![
                    "CREATE CBU WITH nature_purpose \"Investment Banking Services\" AND source_of_funds \"Corporate Finance and M&A Fees\"".to_string(),
                ],
            },

            // Scenario 3: Trust Structure
            TestScenario {
                name: "Family Trust Setup".to_string(),
                description: "Create trust with beneficiaries".to_string(),
                prompts: vec![
                    "CREATE CBU WITH nature_purpose \"Family Trust for Estate Planning\" AND source_of_funds \"Family Assets and Inheritance\"".to_string(),
                ],
            },

            // Scenario 4: Corporate Structure
            TestScenario {
                name: "Multi-Entity Corporate Structure".to_string(),
                description: "Complex corporate setup with subsidiaries".to_string(),
                prompts: vec![
                    "CREATE CBU WITH nature_purpose \"Holding Company for International Operations\" AND source_of_funds \"Revenue from Subsidiaries and Investment Income\"".to_string(),
                ],
            },

            // Scenario 5: Pension Fund
            TestScenario {
                name: "Pension Fund Setup".to_string(),
                description: "Retirement fund with managers".to_string(),
                prompts: vec![
                    "CREATE CBU WITH nature_purpose \"Corporate Pension Fund Management\" AND source_of_funds \"Employee and Employer Contributions\"".to_string(),
                ],
            },
        ]
    }
}
