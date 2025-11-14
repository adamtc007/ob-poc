//! Universal DSL Change Lifecycle Service
//!
//! This service implements the universal pattern for all DSL changes across all domains and states:
//! DSL Edit → AST Generation & Validation → Parse → [Pass/Fail] → Save Both or Return for Re-edit
//!
//! ## Universal Lifecycle Pattern
//! ```
//! ┌─────────────┐    ┌─────────────┐    ┌─────────────┐    ┌─────────────┐
//! │ DSL Edit    │───▶│ AST Gen &   │───▶│ Parse &     │───▶│ Save Both   │
//! │ Triggered   │    │ Validation  │    │ Validate    │    │ DSL + AST   │
//! └─────────────┘    └─────────────┘    └─────────────┘    └─────────────┘
//!                                              │
//!                    ┌─────────────┐          │ FAIL
//!                    │ Return for  │◀─────────┘
//!                    │ Re-edit     │
//!                    └─────────────┘
//! ```
//!
//! This pattern is common for ALL DSL regardless of:
//! - Domain (KYC, UBO, ISDA, Onboarding, etc.)
//! - State (new, incremental, rollback, etc.)
//! - Source (human edit, AI generation, template expansion, etc.)

use crate::db_state_manager::DbStateManager;
use crate::dsl::{DslPipelineProcessor, DslPipelineResult};
use crate::services::DslAstSyncService;
use serde::{Deserialize, Serialize};
use std::collections::HashMap;
use std::time::Instant;
use uuid::Uuid;

/// Universal DSL Change Lifecycle Service
/// Handles the edit→validate→parse→save pattern for ALL DSL operations
pub struct DslLifecycleService {
    /// DSL pipeline processor for AST generation and validation
    dsl_processor: DslPipelineProcessor,
    /// Database state manager for DSL state persistence
    db_state_manager: DbStateManager,
    /// DSL/AST sync service for table synchronization
    sync_service: DslAstSyncService,
    /// Configuration for lifecycle operations
    config: LifecycleConfig,
    /// Active edit sessions
    active_sessions: HashMap<String, EditSession>,
}

/// Configuration for DSL lifecycle operations
#[derive(Debug, Clone)]
pub struct LifecycleConfig {
    /// Maximum number of edit attempts before requiring manual intervention
    pub max_edit_attempts: u32,
    /// Timeout for parse operations (seconds)
    pub parse_timeout_seconds: u64,
    /// Enable automatic validation retry with suggestions
    pub enable_auto_retry: bool,
    /// Enable detailed lifecycle logging
    pub enable_lifecycle_logging: bool,
    /// Cache parsed ASTs for performance
    pub enable_ast_caching: bool,
}

impl Default for LifecycleConfig {
    fn default() -> Self {
        Self {
            max_edit_attempts: 5,
            parse_timeout_seconds: 30,
            enable_auto_retry: true,
            enable_lifecycle_logging: true,
            enable_ast_caching: true,
        }
    }
}

/// Active edit session tracking
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct EditSession {
    /// Session ID
    pub session_id: String,
    /// Case ID being edited
    pub case_id: String,
    /// Domain context
    pub domain: String,
    /// Current DSL content
    pub current_dsl: String,
    /// Number of edit attempts
    pub edit_attempts: u32,
    /// Last parse errors (if any)
    pub last_errors: Vec<String>,
    /// Validation feedback for re-editing
    pub validation_feedback: Vec<String>,
    /// Session start time
    pub started_at: chrono::DateTime<chrono::Utc>,
    /// Last activity time
    pub last_activity: chrono::DateTime<chrono::Utc>,
    /// Session status
    pub status: EditSessionStatus,
}

/// Status of an edit session
#[derive(Debug, Clone, Serialize, Deserialize, PartialEq)]
pub enum EditSessionStatus {
    /// Editing in progress
    Editing,
    /// Parsing in progress
    Parsing,
    /// Validation failed, needs re-edit
    ValidationFailed,
    /// Parse succeeded, ready to save
    ReadyToSave,
    /// Successfully saved
    Saved,
    /// Session terminated due to errors
    Terminated,
}

/// Result of a DSL change operation
#[derive(Debug, Clone)]
pub struct DslChangeResult {
    /// Overall operation success
    pub success: bool,
    /// Session ID for tracking
    pub session_id: String,
    /// Case ID that was processed
    pub case_id: String,
    /// Final DSL content (if successful)
    pub final_dsl: Option<String>,
    /// Generated AST (if successful)
    pub generated_ast: Option<String>,
    /// Operation phase where result was determined
    pub result_phase: LifecyclePhase,
    /// Total processing time
    pub total_time_ms: u64,
    /// Parse/validation errors (if any)
    pub errors: Vec<String>,
    /// Validation feedback for re-editing
    pub feedback: Vec<String>,
    /// Success metrics
    pub metrics: LifecycleMetrics,
}

/// Phases of the DSL lifecycle
#[derive(Debug, Clone, PartialEq)]
pub enum LifecyclePhase {
    DslEdit,
    AstGeneration,
    Validation,
    Parsing,
    Saving,
    Complete,
    Failed,
}

/// Metrics for lifecycle operations
#[derive(Debug, Clone, Default)]
pub struct LifecycleMetrics {
    /// Time spent in AST generation (ms)
    pub ast_generation_time_ms: u64,
    /// Time spent in validation (ms)
    pub validation_time_ms: u64,
    /// Time spent in parsing (ms)
    pub parsing_time_ms: u64,
    /// Time spent in saving (ms)
    pub saving_time_ms: u64,
    /// Number of validation rules checked
    pub validation_rules_checked: u32,
    /// Parse success rate for this session
    pub parse_success_rate: f64,
}

/// Input for DSL change operation
#[derive(Debug, Clone)]
pub struct DslChangeRequest {
    /// Case ID to modify
    pub case_id: String,
    /// New or modified DSL content
    pub dsl_content: String,
    /// Domain context
    pub domain: String,
    /// Change type (new, incremental, replace, etc.)
    pub change_type: DslChangeType,
    /// Session ID (if continuing existing session)
    pub session_id: Option<String>,
    /// User/system making the change
    pub changed_by: String,
    /// Force save even with warnings
    pub force_save: bool,
}

/// Types of DSL changes
#[derive(Debug, Clone, PartialEq)]
pub enum DslChangeType {
    /// Brand new DSL creation
    New,
    /// Incremental addition to existing DSL
    Incremental,
    /// Complete replacement of existing DSL
    Replace,
    /// Rollback to previous version
    Rollback,
    /// Template expansion
    TemplateExpansion,
}

impl DslLifecycleService {
    /// Create new lifecycle service
    pub fn new() -> Self {
        Self {
            dsl_processor: DslPipelineProcessor::new(),
            db_state_manager: DbStateManager::new(),
            sync_service: DslAstSyncService::new(),
            config: LifecycleConfig::default(),
            active_sessions: HashMap::new(),
        }
    }

    /// Create lifecycle service with custom configuration
    pub fn with_config(config: LifecycleConfig) -> Self {
        Self {
            dsl_processor: DslPipelineProcessor::new(),
            db_state_manager: DbStateManager::new(),
            sync_service: DslAstSyncService::new(),
            config,
            active_sessions: HashMap::new(),
        }
    }

    /// Process DSL change through universal lifecycle
    /// This is the master method that implements the universal pattern for ALL DSL changes
    pub async fn process_dsl_change(&mut self, request: DslChangeRequest) -> DslChangeResult {
        let start_time = Instant::now();
        let session_id = request
            .session_id.clone()
            .unwrap_or_else(|| self.generate_session_id());

        if self.config.enable_lifecycle_logging {
            println!(
                "🔄 Starting universal DSL lifecycle for case: {} (session: {})",
                request.case_id, session_id
            );
        }

        // Create or update edit session
        let mut session = self.get_or_create_session(&request, &session_id);
        session.status = EditSessionStatus::Editing;
        session.current_dsl = request.dsl_content.clone();
        session.last_activity = chrono::Utc::now();

        // Phase 1: DSL Edit (already completed - we have the edited DSL)
        if self.config.enable_lifecycle_logging {
            println!("📝 Phase 1: DSL Edit completed - content received");
        }

        // Phase 2: AST Generation & Validation
        let ast_start = Instant::now();
        session.status = EditSessionStatus::Parsing;

        let pipeline_result = self
            .dsl_processor
            .process_dsl_content(&request.dsl_content)
            .await;
        let ast_time = ast_start.elapsed().as_millis() as u64;

        if self.config.enable_lifecycle_logging {
            println!(
                "🔧 Phase 2: AST Generation & Validation completed in {}ms - Success: {}",
                ast_time, pipeline_result.success
            );
        }

        // Phase 3: Parse & Validate
        if !pipeline_result.success {
            // FAIL path: Return for re-edit
            session.status = EditSessionStatus::ValidationFailed;
            session.edit_attempts += 1;
            session.last_errors = pipeline_result.errors.clone();
            session.validation_feedback = self.generate_validation_feedback(&pipeline_result);

            self.active_sessions.insert(session_id.clone(), session);

            return DslChangeResult {
                success: false,
                session_id,
                case_id: request.case_id,
                final_dsl: None,
                generated_ast: None,
                result_phase: LifecyclePhase::Validation,
                total_time_ms: start_time.elapsed().as_millis() as u64,
                feedback: self.generate_validation_feedback(&pipeline_result),
                errors: pipeline_result.errors,
                metrics: LifecycleMetrics {
                    ast_generation_time_ms: ast_time,
                    validation_time_ms: ast_time, // Combined in pipeline
                    parsing_time_ms: 0,
                    saving_time_ms: 0,
                    validation_rules_checked: pipeline_result.step_results.len() as u32,
                    parse_success_rate: 0.0,
                },
            };
        }

        // Phase 4: Save Both DSL and AST (PASS path)
        if self.config.enable_lifecycle_logging {
            println!("💾 Phase 4: Saving DSL and AST with same keys");
        }

        let save_start = Instant::now();
        session.status = EditSessionStatus::ReadyToSave;

        // Create DSL/AST sync request
        let sync_request = self.create_sync_request(&request, &pipeline_result);
        let sync_result = self.sync_service.sync_dsl_and_ast(sync_request).await;
        let save_time = save_start.elapsed().as_millis() as u64;

        if sync_result.success {
            // SUCCESS: Both DSL and AST saved with same keys
            session.status = EditSessionStatus::Saved;
            session.last_activity = chrono::Utc::now();

            if self.config.enable_lifecycle_logging {
                println!(
                    "✅ Universal DSL lifecycle completed successfully in {}ms",
                    start_time.elapsed().as_millis()
                );
            }

            // Remove completed session from active sessions
            self.active_sessions.remove(&session_id);

            DslChangeResult {
                success: true,
                session_id,
                case_id: request.case_id,
                final_dsl: Some(request.dsl_content),
                generated_ast: pipeline_result.parsed_ast,
                result_phase: LifecyclePhase::Complete,
                total_time_ms: start_time.elapsed().as_millis() as u64,
                errors: Vec::new(),
                feedback: Vec::new(),
                metrics: LifecycleMetrics {
                    ast_generation_time_ms: ast_time,
                    validation_time_ms: ast_time,
                    parsing_time_ms: 0,
                    saving_time_ms: save_time,
                    validation_rules_checked: pipeline_result.step_results.len() as u32,
                    parse_success_rate: 1.0,
                },
            }
        } else {
            // FAIL: Save operation failed
            session.status = EditSessionStatus::Terminated;
            session.last_errors = sync_result.errors.clone();

            self.active_sessions.insert(session_id.clone(), session);

            DslChangeResult {
                success: false,
                session_id,
                case_id: request.case_id,
                final_dsl: None,
                generated_ast: pipeline_result.parsed_ast,
                result_phase: LifecyclePhase::Saving,
                total_time_ms: start_time.elapsed().as_millis() as u64,
                errors: sync_result.errors,
                feedback: vec!["Save operation failed - please retry".to_string()],
                metrics: LifecycleMetrics {
                    ast_generation_time_ms: ast_time,
                    validation_time_ms: ast_time,
                    parsing_time_ms: 0,
                    saving_time_ms: save_time,
                    validation_rules_checked: pipeline_result.step_results.len() as u32,
                    parse_success_rate: 0.0,
                },
            }
        }
    }

    /// Get edit session for re-editing after validation failure
    pub fn get_edit_session(&self, session_id: &str) -> Option<&EditSession> {
        self.active_sessions.get(session_id)
    }

    /// Continue editing after validation failure
    pub async fn continue_editing(
        &mut self,
        session_id: &str,
        revised_dsl: String,
    ) -> DslChangeResult {
        if let Some(session) = self.active_sessions.get(session_id) {
            if session.edit_attempts >= self.config.max_edit_attempts {
                return DslChangeResult {
                    success: false,
                    session_id: session_id.to_string(),
                    case_id: session.case_id.clone(),
                    final_dsl: None,
                    generated_ast: None,
                    result_phase: LifecyclePhase::Failed,
                    total_time_ms: 0,
                    errors: vec!["Maximum edit attempts exceeded".to_string()],
                    feedback: vec!["Manual intervention required".to_string()],
                    metrics: LifecycleMetrics::default(),
                };
            }

            // Create new request for continued editing
            let request = DslChangeRequest {
                case_id: session.case_id.clone(),
                dsl_content: revised_dsl,
                domain: session.domain.clone(),
                change_type: DslChangeType::Replace,
                session_id: Some(session_id.to_string()),
                changed_by: "editor".to_string(),
                force_save: false,
            };

            // Process through universal lifecycle again
            self.process_dsl_change(request).await
        } else {
            DslChangeResult {
                success: false,
                session_id: session_id.to_string(),
                case_id: "unknown".to_string(),
                final_dsl: None,
                generated_ast: None,
                result_phase: LifecyclePhase::Failed,
                total_time_ms: 0,
                errors: vec!["Edit session not found".to_string()],
                feedback: Vec::new(),
                metrics: LifecycleMetrics::default(),
            }
        }
    }

    /// Get all active edit sessions
    pub fn get_active_sessions(&self) -> Vec<&EditSession> {
        self.active_sessions.values().collect()
    }

    /// Cleanup expired edit sessions
    pub fn cleanup_expired_sessions(&mut self) {
        let now = chrono::Utc::now();
        let timeout = chrono::Duration::minutes(30); // 30 minute session timeout

        self.active_sessions
            .retain(|_id, session| now.signed_duration_since(session.last_activity) < timeout);
    }

    /// Health check for lifecycle service
    pub async fn health_check(&self) -> bool {
        // Check all underlying services
        self.dsl_processor.health_check().await
            && self.db_state_manager.health_check().await
            && self.sync_service.health_check().await
    }

    // Private helper methods

    fn generate_session_id(&self) -> String {
        let uuid_str = Uuid::new_v4().to_string();
        format!("sess_{}", &uuid_str[..8])
    }

    fn get_or_create_session(&self, request: &DslChangeRequest, session_id: &str) -> EditSession {
        if let Some(existing) = self.active_sessions.get(session_id) {
            existing.clone()
        } else {
            EditSession {
                session_id: session_id.to_string(),
                case_id: request.case_id.clone(),
                domain: request.domain.clone(),
                current_dsl: request.dsl_content.clone(),
                edit_attempts: 0,
                last_errors: Vec::new(),
                validation_feedback: Vec::new(),
                started_at: chrono::Utc::now(),
                last_activity: chrono::Utc::now(),
                status: EditSessionStatus::Editing,
            }
        }
    }

    fn generate_validation_feedback(&self, pipeline_result: &DslPipelineResult) -> Vec<String> {
        let mut feedback = Vec::new();

        for step_result in &pipeline_result.step_results {
            if !step_result.success {
                feedback.push(format!(
                    "Step {}: {} failed - {}",
                    step_result.step_number,
                    step_result.step_name,
                    step_result.errors.join("; ")
                ));
            }

            // Add warnings as feedback
            for warning in &step_result.warnings {
                feedback.push(format!("Warning: {}", warning));
            }
        }

        if feedback.is_empty() {
            feedback.push("DSL structure is valid but contains semantic errors".to_string());
        }

        feedback
    }

    fn create_sync_request(
        &self,
        request: &DslChangeRequest,
        pipeline_result: &DslPipelineResult,
    ) -> crate::services::DslAstSyncRequest {
        use crate::db_state_manager::{create_domain_snapshot, StoredDslState};

        let domain_snapshot = create_domain_snapshot(&request.dsl_content, &request.domain);

        let stored_state = StoredDslState {
            case_id: request.case_id.clone(),
            current_dsl: request.dsl_content.clone(),
            version: 1, // Will be updated by sync service
            domain_snapshot: domain_snapshot.clone(),
            parsed_ast: pipeline_result.parsed_ast.clone(),
            metadata: HashMap::new(),
            updated_at: chrono::Utc::now(),
            audit_entries: Vec::new(),
        };

        crate::services::DslAstSyncRequest {
            case_id: request.case_id.clone(),
            dsl_state: stored_state,
            ast_data: pipeline_result.parsed_ast.clone(),
            domain_snapshot: crate::dsl::DomainSnapshot {
                primary_domain: domain_snapshot.primary_domain,
                involved_domains: domain_snapshot.involved_domains,
                domain_data: domain_snapshot.domain_data,
                compliance_markers: domain_snapshot.compliance_markers,
                risk_assessment: domain_snapshot.risk_assessment,
                snapshot_at: domain_snapshot.snapshot_at,
                dsl_version: 1,
                snapshot_hash: "generated".to_string(),
            },
            dsl_metadata: pipeline_result.dsl_sync_metadata.clone(),
            ast_metadata: pipeline_result.ast_sync_metadata.clone(),
            force_sync: request.force_save,
        }
    }
}

impl Default for DslLifecycleService {
    fn default() -> Self {
        Self::new()
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[tokio::test]
    async fn test_universal_dsl_lifecycle() {
        let mut service = DslLifecycleService::new();

        let request = DslChangeRequest {
            case_id: "TEST-001".to_string(),
            dsl_content: r#"(case.create :case-id "TEST-001" :case-type "ONBOARDING")"#.to_string(),
            domain: "onboarding".to_string(),
            change_type: DslChangeType::New,
            session_id: None,
            changed_by: "test_user".to_string(),
            force_save: false,
        };

        let result = service.process_dsl_change(request).await;

        // Should succeed for valid DSL
        assert!(result.success);
        assert_eq!(result.case_id, "TEST-001");
        assert!(result.final_dsl.is_some());
        assert!(result.generated_ast.is_some());
        assert_eq!(result.result_phase, LifecyclePhase::Complete);
    }

    #[tokio::test]
    async fn test_validation_failure_and_retry() {
        let mut service = DslLifecycleService::new();

        // Invalid DSL should fail validation
        let invalid_request = DslChangeRequest {
            case_id: "TEST-002".to_string(),
            dsl_content: "invalid dsl content".to_string(),
            domain: "kyc".to_string(),
            change_type: DslChangeType::New,
            session_id: None,
            changed_by: "test_user".to_string(),
            force_save: false,
        };

        let result = service.process_dsl_change(invalid_request).await;

        // Should fail validation
        assert!(!result.success);
        assert_eq!(result.result_phase, LifecyclePhase::Validation);
        assert!(!result.errors.is_empty());
        assert!(!result.feedback.is_empty());

        // Should have active session for re-editing
        let session = service.get_edit_session(&result.session_id);
        assert!(session.is_some());
        assert_eq!(session.unwrap().status, EditSessionStatus::ValidationFailed);
    }

    #[test]
    fn test_lifecycle_config() {
        let config = LifecycleConfig::default();
        assert_eq!(config.max_edit_attempts, 5);
        assert_eq!(config.parse_timeout_seconds, 30);
        assert!(config.enable_auto_retry);
    }

    #[tokio::test]
    async fn test_health_check() {
        let service = DslLifecycleService::new();
        assert!(service.health_check().await);
    }
}
