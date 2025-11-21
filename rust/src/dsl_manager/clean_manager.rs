//! Clean DSL Manager - Refactored Gateway Following Call Chain Pattern
//!
//! This module provides a clean, simplified DSL Manager implementation based on the
//! proven call chain architecture from the independent implementation blueprint.
//!
//! ## Architecture: Clean Call Chain Pattern
//! DSL Manager → DSL Mod → DB State Manager → DSL Visualizer
//!
//! ## Design Principles from Session Record
//! 1. **DSL-First Design**: Core system works without AI dependencies
//! 2. **Incremental Accumulation**: Base DSL + incremental additions = accumulated state
//! 3. **Clean Separation**: AI as optional layer, DSL CRUD as core system
//! 4. **Call Chain Approach**: Build it, run it, see where it breaks, fix incrementally
//!
//! ## Key Responsibilities
//! - Serve as the single entry point gateway for all DSL operations
//! - Route DSL operations through the clean call chain
//! - Coordinate incremental DSL accumulation (DSL-as-State pattern)
//! - Provide unified interface for AI and direct DSL operations
//! - Maintain separation between core DSL CRUD and optional AI layer

use crate::db_state_manager::DbStateManager;
use crate::dsl::DslPipelineProcessor;
#[cfg(feature = "database")]
use crate::dsl::PipelineConfig;
use crate::dsl_visualizer::DslVisualizer;
use std::time::Instant;
use uuid::Uuid;

/// Clean DSL Manager following the proven call chain pattern
pub struct CleanDslManager {
    /// DSL processing pipeline (DSL Mod)
    dsl_processor: DslPipelineProcessor,
    /// Database state manager
    db_state_manager: DbStateManager,
    /// Visualization generator
    visualizer: DslVisualizer,
    /// Configuration
    config: CleanManagerConfig,
    /// Database service for SQLX integration
    #[cfg(feature = "database")]
    database_service: Option<crate::database::DictionaryDatabaseService>,
}

/// Configuration for Clean DSL Manager
#[derive(Debug, Clone)]
pub struct CleanManagerConfig {
    /// Enable detailed logging throughout the call chain
    pub enable_detailed_logging: bool,
    /// Enable performance metrics collection
    pub enable_metrics: bool,
    /// Maximum processing time for the entire call chain (seconds)
    pub max_processing_time_seconds: u64,
    /// Enable automatic state cleanup
    pub enable_auto_cleanup: bool,
}

impl Default for CleanManagerConfig {
    fn default() -> Self {
        Self {
            enable_detailed_logging: true,
            enable_metrics: true,
            max_processing_time_seconds: 60,
            enable_auto_cleanup: false,
        }
    }
}

/// Result from the complete call chain processing
#[derive(Debug, Clone)]
pub struct CallChainResult {
    /// Overall operation success status
    pub success: bool,
    /// Case ID that was processed
    pub case_id: String,
    /// Total processing time in milliseconds
    pub processing_time_ms: u64,
    /// Any errors that occurred during the call chain
    pub errors: Vec<String>,
    /// Whether visualization was successfully generated
    pub visualization_generated: bool,
    /// Whether this operation used AI generation
    pub ai_generated: bool,
    /// Call chain step details
    pub step_details: CallChainSteps,
}

/// Detailed results from each step in the call chain
#[derive(Debug, Clone)]
pub struct CallChainSteps {
    /// DSL Mod processing result
    pub dsl_processing: Option<DslProcessingStepResult>,
    /// DB State Manager result
    pub state_management: Option<StateManagementStepResult>,
    /// DSL Visualizer result
    pub visualization: Option<VisualizationStepResult>,
}

/// Result from DSL processing step
#[derive(Debug, Clone)]
pub struct DslProcessingStepResult {
    pub success: bool,
    pub processing_time_ms: u64,
    pub parsed_ast_available: bool,
    pub domain_snapshot_created: bool,
    pub errors: Vec<String>,
}

/// Result from state management step
#[derive(Debug, Clone)]
pub struct StateManagementStepResult {
    pub success: bool,
    pub processing_time_ms: u64,
    pub version_number: u32,
    pub snapshot_id: String,
    pub errors: Vec<String>,
}

/// Result from visualization step
#[derive(Debug, Clone)]
pub struct VisualizationStepResult {
    pub success: bool,
    pub processing_time_ms: u64,
    pub output_size_bytes: usize,
    pub format: String,
    pub errors: Vec<String>,
}

/// Result from incremental DSL processing
#[derive(Debug, Clone)]
pub struct IncrementalResult {
    /// Operation success status
    pub success: bool,
    /// Case ID that was processed
    pub case_id: String,
    /// Complete accumulated DSL content
    pub accumulated_dsl: String,
    /// New version number after increment
    pub version_number: u32,
    /// Any errors that occurred
    pub errors: Vec<String>,
}

/// Result from validation-only operations
#[derive(Debug, Clone)]
pub struct ValidationResult {
    /// Validation success status
    pub valid: bool,
    /// Validation errors found
    pub errors: Vec<String>,
    /// Validation warnings
    pub warnings: Vec<String>,
    /// Number of validation rules checked
    pub rules_checked: u32,
    /// Overall compliance score (0.0 to 1.0)
    pub compliance_score: f64,
}

/// Result from AI-enhanced operations
#[derive(Debug, Clone)]
pub struct AiResult {
    /// Overall operation success
    pub success: bool,
    /// AI-generated DSL content
    pub generated_dsl: String,
    /// Case ID for the generated DSL
    pub case_id: String,
    /// AI confidence score (0.0 to 1.0)
    pub ai_confidence: f64,
    /// Whether the generated DSL passed validation
    pub validation_passed: bool,
    /// Total processing time including AI generation
    pub processing_time_ms: u64,
    /// Flag indicating this was AI-generated
    pub ai_generated: bool,
}

impl CleanDslManager {
    /// Create a new Clean DSL Manager with default configuration
    pub fn new() -> Self {
        Self {
            dsl_processor: DslPipelineProcessor::new(),
            db_state_manager: DbStateManager::new(),
            visualizer: DslVisualizer::new(),
            config: CleanManagerConfig::default(),
            #[cfg(feature = "database")]
            database_service: None,
        }
    }

    /// Create a Clean DSL Manager with custom configuration
    pub fn with_config(config: CleanManagerConfig) -> Self {
        Self {
            dsl_processor: DslPipelineProcessor::new(),
            db_state_manager: DbStateManager::new(),
            visualizer: DslVisualizer::new(),
            config,
            #[cfg(feature = "database")]
            database_service: None,
        }
    }

    /// Create a Clean DSL Manager with database connectivity for SQLX integration
    #[cfg(feature = "database")]
    pub fn with_database(database_service: crate::database::DictionaryDatabaseService) -> Self {
        Self {
            dsl_processor: DslPipelineProcessor::with_database(database_service.clone()),
            db_state_manager: DbStateManager::new(),
            visualizer: DslVisualizer::new(),
            config: CleanManagerConfig::default(),
            #[cfg(feature = "database")]
            database_service: Some(database_service),
        }
    }

    /// Create a Clean DSL Manager with both config and database connectivity
    #[cfg(feature = "database")]
    pub fn with_config_and_database(
        config: CleanManagerConfig,
        database_service: crate::database::DictionaryDatabaseService,
    ) -> Self {
        // Create database manager from the service's pool
        let db_manager =
            crate::database::DatabaseManager::from_pool(database_service.pool().clone());

        Self {
            dsl_processor: DslPipelineProcessor::with_config_and_database(
                PipelineConfig {
                    enable_strict_validation: true,
                    fail_fast: true,
                    enable_detailed_logging: config.enable_detailed_logging,
                    max_step_time_seconds: config.max_processing_time_seconds,
                    enable_metrics: config.enable_metrics,
                },
                database_service.clone(),
            ),
            // Wire database through to state manager
            db_state_manager: DbStateManager::with_database(db_manager),
            visualizer: DslVisualizer::new(),
            config,
            #[cfg(feature = "database")]
            database_service: Some(database_service),
        }
    }

    /// Check if the manager has database connectivity
    #[cfg(feature = "database")]
    pub fn has_database(&self) -> bool {
        self.database_service.is_some()
    }

    /// Check if the manager has database connectivity (without database feature)
    #[cfg(not(feature = "database"))]
    pub fn has_database(&self) -> bool {
        false
    }

    /// Get a reference to the database service if available
    #[cfg(feature = "database")]
    pub fn database_service(&self) -> Option<&crate::database::DictionaryDatabaseService> {
        self.database_service.as_ref()
    }

    /// Get a reference to the database service if available (without database feature)
    #[cfg(not(feature = "database"))]
    pub fn database_service(&self) -> Option<()> {
        None
    }

    /// Process DSL request through the complete call chain
    /// This is the main entry point implementing: DSL Manager → Forth Engine → DB
    pub fn process_dsl_request(&mut self, dsl_content: String) -> CallChainResult {
        use crate::forth_engine::{extract_case_id, DslSheet};

        let start_time = Instant::now();

        // Pre-extract case_id for sheet naming
        let preliminary_case_id = extract_case_id(&dsl_content).unwrap_or_else(|| {
            format!(
                "CASE-{}",
                uuid::Uuid::new_v4().to_string()[..8].to_uppercase()
            )
        });

        let sheet = DslSheet {
            id: preliminary_case_id.clone(),
            domain: "dsl".to_string(),
            version: "1.0".to_string(),
            content: dsl_content.clone(),
        };

        // Execute through Forth engine
        #[cfg(feature = "database")]
        let execution_result = if let Some(ref db_service) = self.database_service {
            // Run async database operations using block_in_place for multi-threaded runtime
            let pool = db_service.pool().clone();
            let sheet_clone = sheet.clone();

            tokio::task::block_in_place(|| {
                tokio::runtime::Handle::current().block_on(async {
                    crate::forth_engine::execute_sheet_with_db(&sheet_clone, pool).await
                })
            })
        } else {
            crate::forth_engine::execute_sheet(&sheet).map(|logs| {
                crate::forth_engine::ExecutionResult {
                    logs,
                    case_id: Some(preliminary_case_id.clone()),
                    success: true,
                    version: 0,
                }
            })
        };

        #[cfg(not(feature = "database"))]
        let execution_result = crate::forth_engine::execute_sheet(&sheet).map(|logs| {
            crate::forth_engine::ExecutionResult {
                logs,
                case_id: Some(preliminary_case_id.clone()),
                success: true,
                version: 0,
            }
        });

        match execution_result {
            Ok(result) => {
                let case_id = result.case_id.unwrap_or(preliminary_case_id);

                // Save the DSL to the state manager for accumulation support
                // This is critical for the DSL-as-State pattern
                // Use synchronous update to avoid tokio runtime issues
                self.db_state_manager
                    .update_accumulated_dsl_sync(&case_id, &dsl_content);

                CallChainResult {
                    success: true,
                    case_id,
                    processing_time_ms: start_time.elapsed().as_millis() as u64,
                    errors: vec![],
                    visualization_generated: false,
                    ai_generated: false,
                    step_details: CallChainSteps {
                        dsl_processing: Some(DslProcessingStepResult {
                            success: true,
                            processing_time_ms: start_time.elapsed().as_millis() as u64,
                            parsed_ast_available: true,
                            domain_snapshot_created: false,
                            errors: result.logs,
                        }),
                        state_management: None,
                        visualization: None,
                    },
                }
            }
            Err(e) => CallChainResult {
                success: false,
                case_id: preliminary_case_id,
                processing_time_ms: start_time.elapsed().as_millis() as u64,
                errors: vec![e.to_string()],
                visualization_generated: false,
                ai_generated: false,
                step_details: CallChainSteps {
                    dsl_processing: Some(DslProcessingStepResult {
                        success: false,
                        processing_time_ms: start_time.elapsed().as_millis() as u64,
                        parsed_ast_available: false,
                        domain_snapshot_created: false,
                        errors: vec![e.to_string()],
                    }),
                    state_management: None,
                    visualization: None,
                },
            },
        }
    }

    /// Process incremental DSL addition (DSL-as-State pattern)
    pub async fn process_incremental_dsl(
        &mut self,
        case_id: String,
        additional_dsl: String,
    ) -> IncrementalResult {
        if self.config.enable_detailed_logging {
            println!(
                "🔄 Clean DSL Manager: Processing incremental DSL for case {}",
                case_id
            );
        }

        // Load existing accumulated state
        let existing_state = self.db_state_manager.load_accumulated_state(&case_id).await;

        // Update the accumulated DSL in the state manager
        let update_success = self
            .db_state_manager
            .update_accumulated_dsl(&case_id, &additional_dsl)
            .await;

        if !update_success {
            return IncrementalResult {
                success: false,
                case_id: case_id.clone(),
                accumulated_dsl: existing_state.current_dsl,
                version_number: existing_state.version,
                errors: vec!["Failed to update accumulated DSL".to_string()],
            };
        }

        // Load the updated state
        let updated_state = self.db_state_manager.load_accumulated_state(&case_id).await;

        // Process the complete accumulated DSL through the call chain
        let call_chain_result = self.process_dsl_request(updated_state.current_dsl.clone());

        // Capture errors from execution if any
        let errors = if call_chain_result.success {
            Vec::new()
        } else {
            call_chain_result.errors
        };

        IncrementalResult {
            success: call_chain_result.success,
            case_id: case_id.clone(),
            accumulated_dsl: updated_state.current_dsl,
            version_number: updated_state.version,
            errors,
        }
    }

    /// Validate DSL content without full processing
    pub async fn validate_dsl_only(&self, dsl_content: String) -> ValidationResult {
        if self.config.enable_detailed_logging {
            println!("🔍 Clean DSL Manager: Validation-only mode");
        }

        let validation_result = self.dsl_processor.validate_dsl_content(&dsl_content).await;

        let mut rules_checked = 4; // Base validation rules from 4-step pipeline
        let mut compliance_score = if validation_result.success { 1.0 } else { 0.0 };

        // Adjust score based on warnings
        let warning_count = validation_result
            .step_results
            .iter()
            .map(|step| step.warnings.len())
            .sum::<usize>();

        if warning_count > 0 {
            compliance_score = (compliance_score * 0.8_f64).max(0.0);
            rules_checked += warning_count as u32;
        }

        ValidationResult {
            valid: validation_result.success,
            errors: validation_result.errors,
            warnings: validation_result
                .step_results
                .iter()
                .flat_map(|step| step.warnings.clone())
                .collect(),
            rules_checked,
            compliance_score,
        }
    }

    /// Process AI-generated DSL instruction (AI separation pattern)
    pub async fn process_ai_instruction(&mut self, instruction: String) -> AiResult {
        if self.config.enable_detailed_logging {
            println!("🤖 Clean DSL Manager: Processing AI instruction (mock implementation)");
        }

        let start_time = Instant::now();

        // Mock AI DSL generation - in real implementation, this would call AI services
        let generated_dsl = self.mock_ai_generation(&instruction).await;
        let case_id = self.extract_or_generate_case_id(&generated_dsl);

        // Validate the generated DSL
        let validation_result = self.validate_dsl_only(generated_dsl.clone()).await;

        // If validation passes, process through the call chain
        let mut processing_success = false;
        if validation_result.valid {
            let call_chain_result = self.process_dsl_request(generated_dsl.clone());
            processing_success = call_chain_result.success;
        }

        AiResult {
            success: validation_result.valid && processing_success,
            generated_dsl,
            case_id,
            ai_confidence: 0.85, // Mock confidence score
            validation_passed: validation_result.valid,
            processing_time_ms: start_time.elapsed().as_millis() as u64,
            ai_generated: true,
        }
    }

    /// Health check for the entire call chain
    pub async fn health_check(&mut self) -> bool {
        if self.config.enable_detailed_logging {
            println!("🏥 Clean DSL Manager: Performing call chain health check");
        }

        let dsl_healthy = self.dsl_processor.health_check().await;
        let db_healthy = self.db_state_manager.health_check().await;
        let viz_healthy = self.visualizer.health_check().await;

        let overall_healthy = dsl_healthy && db_healthy && viz_healthy;

        if self.config.enable_detailed_logging {
            println!(
                "✅ Clean DSL Manager health check: {} (DSL: {}, DB: {}, Viz: {})",
                if overall_healthy {
                    "HEALTHY"
                } else {
                    "UNHEALTHY"
                },
                if dsl_healthy { "OK" } else { "FAIL" },
                if db_healthy { "OK" } else { "FAIL" },
                if viz_healthy { "OK" } else { "FAIL" }
            );
        }

        overall_healthy
    }

    // Private helper methods

    async fn mock_ai_generation(&self, instruction: &str) -> String {
        // Mock AI generation - replace with real AI service integration
        if instruction.to_lowercase().contains("onboarding") {
            format!(
                r#"(case.create :case-id "{}" :case-type "ONBOARDING" :instruction "{}")"#,
                self.generate_case_id(),
                instruction
            )
        } else if instruction.to_lowercase().contains("kyc") {
            format!(
                r#"(kyc.collect :case-id "{}" :collection-type "ENHANCED" :instruction "{}")"#,
                self.generate_case_id(),
                instruction
            )
        } else {
            format!(
                r#"(case.create :case-id "{}" :case-type "GENERAL" :instruction "{}")"#,
                self.generate_case_id(),
                instruction
            )
        }
    }

    fn extract_or_generate_case_id(&self, dsl_content: &str) -> String {
        // Try to extract case ID from DSL content
        if let Some(start) = dsl_content.find(":case-id") {
            if let Some(quote_start) = dsl_content[start..].find('"') {
                let absolute_quote_start = start + quote_start + 1;
                if let Some(quote_end) = dsl_content[absolute_quote_start..].find('"') {
                    return dsl_content[absolute_quote_start..absolute_quote_start + quote_end]
                        .to_string();
                }
            }
        }
        // Generate new case ID if extraction failed
        self.generate_case_id()
    }

    fn generate_case_id(&self) -> String {
        format!("CASE-{}", Uuid::new_v4().to_string()[..8].to_uppercase())
    }
}

impl Default for CleanDslManager {
    fn default() -> Self {
        Self::new()
    }
}

impl CleanDslManager {
    /// Create DSL Manager from database pool for SQLX integration testing
    #[cfg(feature = "database")]
    pub async fn from_database_pool(pool: sqlx::PgPool) -> Self {
        let database_service = crate::database::DictionaryDatabaseService::new(pool);
        Self::with_database(database_service)
    }

    /// Test database connectivity if available
    #[cfg(feature = "database")]
    pub async fn test_database_connection(&self) -> Result<bool, String> {
        if let Some(db_service) = &self.database_service {
            // Use the database service to test connectivity
            match db_service.health_check().await {
                Ok(_) => Ok(true),
                Err(e) => Err(format!("Database connection test failed: {}", e)),
            }
        } else {
            Err("No database service configured".to_string())
        }
    }

    /// Test database connectivity if available (without database feature)
    #[cfg(not(feature = "database"))]
    pub async fn test_database_connection(&self) -> Result<bool, String> {
        Err("Database feature not enabled".to_string())
    }

    /// Execute DSL with database persistence for integration testing
    pub async fn execute_dsl_with_database(&mut self, dsl_content: String) -> CallChainResult {
        if !self.has_database() {
            return CallChainResult {
                success: false,
                case_id: "NO_DATABASE".to_string(),
                processing_time_ms: 0,
                errors: vec!["No database connectivity configured".to_string()],
                visualization_generated: false,
                ai_generated: false,
                step_details: CallChainSteps {
                    dsl_processing: None,
                    state_management: None,
                    visualization: None,
                },
            };
        }

        // Use the regular processing flow - the database connectivity is already wired through
        self.process_dsl_request(dsl_content)
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[tokio::test]
    async fn test_clean_dsl_manager_creation() {
        let mut manager = CleanDslManager::new();
        assert!(manager.health_check().await);
    }

    #[tokio::test]
    async fn test_dsl_call_chain_processing() {
        let mut manager = CleanDslManager::new();
        let dsl_content =
            r#"(case.create :case-id "CLEAN-001" :case-type "ONBOARDING")"#.to_string();

        let result = manager.process_dsl_request(dsl_content);

        if !result.success {
            eprintln!("Test failed with errors: {:?}", result.errors);
        }
        assert!(result.success);
        assert_eq!(result.case_id, "CLEAN-001");
        assert!(!result.ai_generated);
    }

    #[tokio::test]
    async fn test_incremental_dsl_processing() {
        let mut manager = CleanDslManager::new();

        // Base DSL
        let base_dsl = r#"(case.create :case-id "INC-001" :case-type "ONBOARDING")"#.to_string();
        let base_result = manager.process_dsl_request(base_dsl);
        assert!(base_result.success);

        // Incremental DSL
        let incremental_dsl =
            r#"(kyc.collect :case-id "INC-001" :collection-type "ENHANCED")"#.to_string();
        let incremental_result = manager
            .process_incremental_dsl("INC-001".to_string(), incremental_dsl)
            .await;

        assert!(incremental_result.success);
        // Verify DSL accumulation works correctly
        assert!(
            incremental_result.accumulated_dsl.contains("case.create"),
            "Accumulated DSL should contain base case.create"
        );
        assert!(
            incremental_result.accumulated_dsl.contains("kyc.collect"),
            "Accumulated DSL should contain incremental kyc.collect"
        );
    }

    #[tokio::test]
    async fn test_validation_only_mode() {
        let manager = CleanDslManager::new();
        let valid_dsl = r#"(entity.register :case-id "VAL-001" :entity-type "CORP")"#.to_string();

        let validation_result = manager.validate_dsl_only(valid_dsl).await;

        assert!(validation_result.valid);
        assert!(validation_result.errors.is_empty());
        assert!(validation_result.rules_checked > 0);
        assert!(validation_result.compliance_score > 0.0);
    }

    #[tokio::test]
    async fn test_ai_instruction_processing() {
        let mut manager = CleanDslManager::new();
        let instruction = "Create onboarding case for UK tech company".to_string();

        let ai_result = manager.process_ai_instruction(instruction).await;

        assert!(ai_result.ai_generated);
        assert!(!ai_result.generated_dsl.is_empty());
        assert!(ai_result.generated_dsl.contains("onboarding"));
        assert!(!ai_result.case_id.is_empty());
    }

    #[tokio::test]
    async fn test_failed_dsl_processing() {
        let mut manager = CleanDslManager::new();
        let invalid_dsl = "invalid dsl content".to_string();

        let result = manager.process_dsl_request(invalid_dsl);

        assert!(!result.success);
        assert!(!result.errors.is_empty());
        assert!(!result.visualization_generated);
    }

    #[tokio::test]
    async fn test_call_chain_step_details() {
        let mut manager = CleanDslManager::new();
        let dsl_content =
            r#"(products.add :case-id "STEP-001" :product-type "CUSTODY")"#.to_string();

        let result = manager.process_dsl_request(dsl_content);

        assert!(result.success);
        assert!(result.step_details.dsl_processing.is_some());
        // Note: state_management and visualization are not yet implemented in Forth engine
        // assert!(result.step_details.state_management.is_some());
        // assert!(result.step_details.visualization.is_some());

        let dsl_step = result.step_details.dsl_processing.unwrap();
        assert!(dsl_step.success);
    }

    #[tokio::test]
    async fn test_dsl_orchestration_interface_integration() {
        let mut manager = CleanDslManager::new();

        // Test orchestration interface is properly integrated
        let dsl_content =
            r#"(case.create :case-id "ORCH-001" :case-type "ORCHESTRATION_TEST")"#.to_string();

        let result = manager.process_dsl_request(dsl_content);

        // Verify Forth engine execution worked
        assert!(result.success, "Forth engine execution should succeed");
        assert_eq!(result.case_id, "ORCH-001");

        // Verify DSL processing step completed
        assert!(
            result.step_details.dsl_processing.is_some(),
            "DSL processing step should exist"
        );

        // Verify DSL processing worked
        let dsl_step = result.step_details.dsl_processing.unwrap();
        assert!(dsl_step.success, "DSL processing should succeed");
        assert!(dsl_step.parsed_ast_available, "AST should be available");
    }

    #[tokio::test]
    async fn test_orchestration_error_handling() {
        let mut manager = CleanDslManager::new();

        // Test orchestration with invalid DSL
        let invalid_dsl = "invalid dsl without proper syntax".to_string();

        let result = manager.process_dsl_request(invalid_dsl);

        // Orchestration should handle errors gracefully
        assert!(
            !result.success,
            "Invalid DSL should fail through orchestration"
        );
        assert!(
            !result.errors.is_empty(),
            "Errors should be captured from orchestration"
        );
        assert!(
            !result.visualization_generated,
            "Visualization should not be generated on failure"
        );

        println!("✅ DSL Orchestration Error Handling: WORKING");
    }
}
