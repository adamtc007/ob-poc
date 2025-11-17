//! AI-Powered DSL Service
//!
//! This service acts as a high-level API that delegates to the DSL Manager.
//! ALL DSL operations go through the DSL Manager as the single entry point.
//! This service provides backwards compatibility while ensuring proper DSL lifecycle management.

use crate::dsl_manager::CleanDslManager;
use chrono::Utc;
use serde::{Deserialize, Serialize};
use std::collections::HashMap;
use tracing::{debug, error, info, warn};
use uuid::Uuid;

/// Result type for AI DSL service operations
pub(crate) type AiDslResult<T> = Result<T, AiDslServiceError>;

/// AI DSL service error types
#[derive(Debug, thiserror::Error)]
pub(crate) enum AiDslServiceError {
    #[error("AI service error: {0}")]
    AiError(String),

    #[error("Validation error: {0}")]
    ValidationError(String),

    #[error("DSL processing error: {0}")]
    DslProcessingError(String),

    #[error("Database error: {0}")]
    DatabaseError(String),

    #[error("Configuration error: {0}")]
    ConfigurationError(String),
}

/// AI configuration placeholder (since AI module is not available)
#[derive(Debug, Clone, Default)]
pub struct AiConfig {
    pub model: String,
    pub max_tokens: u32,
    pub temperature: f32,
}

/// Mock OpenAI client for compilation compatibility
#[derive(Debug, Clone)]
pub struct OpenAiClient {
    config: AiConfig,
}

impl OpenAiClient {
    pub fn new(_config: AiConfig) -> Result<Self, AiDslServiceError> {
        // Mock implementation - AI module not available
        Err(AiDslServiceError::AiError(
            "AI module not available - mock implementation".to_string(),
        ))
    }
}

/// AI onboarding request structure
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct AiOnboardingRequest {
    pub instruction: String,
    pub client_name: String,
    pub jurisdiction: String,
    pub entity_type: String,
    pub services: Vec<String>,
    pub context_hints: Vec<String>,
    pub metadata: HashMap<String, String>,
}

/// AI onboarding response structure
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct AiOnboardingResponse {
    pub success: bool,
    pub cbu_id: String,
    pub generated_dsl: String,
    pub validation_passed: bool,
    pub execution_details: ExecutionDetails,
    pub processing_time_ms: u64,
    pub ai_confidence_score: f64,
    pub suggestions: Vec<String>,
}

/// Execution details for AI operations
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ExecutionDetails {
    pub template_used: String,
    pub compilation_successful: bool,
    pub validation_passed: bool,
    pub storage_keys: Vec<String>,
    pub execution_time_ms: u64,
}

/// CBU (Client Business Unit) generator
#[derive(Debug, Clone)]
pub struct CbuGenerator;

impl CbuGenerator {
    pub fn generate_cbu_id(client_name: &str, jurisdiction: &str) -> String {
        let timestamp = Utc::now().timestamp();
        let clean_name = client_name
            .chars()
            .filter(|c| c.is_alphanumeric())
            .collect::<String>()
            .to_uppercase();

        format!(
            "CBU-{}-{}-{}",
            jurisdiction.to_uppercase(),
            clean_name,
            timestamp
        )
    }
}

/// DSL instance summary
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct DslInstanceSummary {
    pub instance_id: String,
    pub case_id: String,
    pub status: String,
    pub created_at: chrono::DateTime<Utc>,
    pub last_updated: chrono::DateTime<Utc>,
}

/// Health check result
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct HealthCheckResult {
    pub healthy: bool,
    pub ai_service_available: bool,
    pub dsl_manager_available: bool,
    pub database_available: bool,
    pub response_time_ms: u64,
    pub message: String,
}

/// Validation result
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ValidationResult {
    pub valid: bool,
    pub errors: Vec<String>,
    pub warnings: Vec<String>,
    pub suggestions: Vec<String>,
    pub explanation: String,
}

/// AI DSL Service - delegates all operations to DSL Manager
pub(crate) struct AiDslService {
    dsl_manager: CleanDslManager,
}

impl AiDslService {
    /// Create a new AI DSL service - uses DSL Manager as backend
    pub async fn new() -> AiDslResult<Self> {
        let dsl_manager = CleanDslManager::new();

        Ok(Self { dsl_manager })
    }

    /// Create AI DSL service with database connectivity
    #[cfg(feature = "database")]
    pub async fn new_with_database(
        database_service: crate::database::DslDomainRepository,
    ) -> AiDslResult<Self> {
        let dsl_manager = CleanDslManager::with_database(database_service);

        Ok(Self { dsl_manager })
    }

    /// Create AI onboarding - delegates to DSL Manager
    pub async fn create_ai_onboarding(
        &mut self,
        request: AiOnboardingRequest,
    ) -> AiDslResult<AiOnboardingResponse> {
        info!(
            "Processing AI onboarding request for client: {}",
            request.client_name
        );

        let start_time = std::time::Instant::now();

        // Generate CBU ID
        let cbu_id = CbuGenerator::generate_cbu_id(&request.client_name, &request.jurisdiction);

        // Generate DSL based on request (mock implementation)
        let generated_dsl = self.generate_dsl_from_request(&request, &cbu_id);

        // Process through DSL Manager
        let dsl_result = self
            .dsl_manager
            .process_dsl_request(generated_dsl.clone())
            .await;

        let processing_time = start_time.elapsed().as_millis() as u64;

        let response = AiOnboardingResponse {
            success: dsl_result.success,
            cbu_id,
            generated_dsl,
            validation_passed: dsl_result.success,
            execution_details: ExecutionDetails {
                template_used: "onboarding_template".to_string(),
                compilation_successful: dsl_result.success,
                validation_passed: dsl_result.success,
                storage_keys: vec![dsl_result.case_id.clone()],
                execution_time_ms: dsl_result.processing_time_ms,
            },
            processing_time_ms: processing_time,
            ai_confidence_score: 0.85, // Mock score
            suggestions: vec![],
        };

        if response.success {
            info!(
                "AI onboarding completed successfully for CBU: {}",
                response.cbu_id
            );
        } else {
            warn!(
                "AI onboarding failed for CBU: {}, errors: {:?}",
                response.cbu_id, dsl_result.errors
            );
        }

        Ok(response)
    }

    /// Generate DSL from AI request (mock implementation)
    fn generate_dsl_from_request(&self, request: &AiOnboardingRequest, cbu_id: &str) -> String {
        // Mock DSL generation based on request parameters
        format!(
            r#"(case.create
                :case-id "{}"
                :case-type "AI_ONBOARDING"
                :customer-name "{}"
                :jurisdiction "{}"
                :entity-type "{}"
                :services {:?}
                :cbu-id "{}"
                :ai-generated true)"#,
            Uuid::new_v4(),
            request.client_name,
            request.jurisdiction,
            request.entity_type,
            request.services,
            cbu_id
        )
    }

    /// Validate DSL with AI assistance (mock implementation)
    pub async fn validate_dsl_with_ai(&self, dsl_content: &str) -> AiDslResult<ValidationResult> {
        debug!("Validating DSL with AI assistance: {}", dsl_content);

        // Basic validation - check if it looks like valid DSL
        let is_valid = dsl_content.starts_with('(') && dsl_content.ends_with(')');

        let result = ValidationResult {
            valid: is_valid,
            errors: if is_valid {
                vec![]
            } else {
                vec!["DSL syntax error - must be s-expression".to_string()]
            },
            warnings: vec![],
            suggestions: if is_valid {
                vec![]
            } else {
                vec!["Ensure DSL is wrapped in parentheses".to_string()]
            },
            explanation: if is_valid {
                "DSL appears to be syntactically valid".to_string()
            } else {
                "DSL syntax is invalid".to_string()
            },
        };

        Ok(result)
    }

    /// Get service health status
    pub async fn health_check(&self) -> AiDslResult<HealthCheckResult> {
        let start_time = std::time::Instant::now();

        // Check DSL Manager health
        let dsl_manager_health = self.dsl_manager.health_check().await;
        let response_time = start_time.elapsed().as_millis() as u64;

        let result = HealthCheckResult {
            healthy: dsl_manager_health,
            ai_service_available: false, // AI module not available
            dsl_manager_available: dsl_manager_health,
            database_available: self.dsl_manager.has_database(),
            response_time_ms: response_time,
            message: if dsl_manager_health {
                "Service healthy - DSL Manager operational".to_string()
            } else {
                "Service degraded - DSL Manager issues".to_string()
            },
        };

        Ok(result)
    }

    /// Get DSL instance summary (mock implementation)
    pub async fn get_dsl_instance_summary(
        &self,
        instance_id: &str,
    ) -> AiDslResult<DslInstanceSummary> {
        // Mock implementation - would normally query database
        Ok(DslInstanceSummary {
            instance_id: instance_id.to_string(),
            case_id: format!("CASE-{}", instance_id),
            status: "ACTIVE".to_string(),
            created_at: Utc::now(),
            last_updated: Utc::now(),
        })
    }
}

impl Default for AiOnboardingRequest {
    fn default() -> Self {
        Self {
            instruction: "Create standard onboarding".to_string(),
            client_name: "Default Client".to_string(),
            jurisdiction: "US".to_string(),
            entity_type: "CORPORATION".to_string(),
            services: vec!["KYC".to_string()],
            context_hints: vec![],
            metadata: HashMap::new(),
        }
    }
}

