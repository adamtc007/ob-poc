//! AI-Powered DSL Service
//!
//! This service now acts as a high-level API that delegates to the DSL Manager.
//! ALL DSL operations go through the DSL Manager as the single entry point.
//! This service provides backwards compatibility while ensuring proper DSL lifecycle management.

use crate::ai::{openai::OpenAiClient, AiConfig, AiService};
use crate::dsl_manager::{DslContext, DslManager, DslManagerFactory, DslProcessingOptions};
use chrono::Utc;
use serde::{Deserialize, Serialize};
use std::collections::HashMap;
use std::sync::Arc;
use tracing::{debug, error, info, warn};
use uuid::Uuid;

/// AI DSL Service errors
#[derive(Debug, thiserror::Error)]
pub(crate) enum AiDslServiceError {
    #[error("AI service error: {0}")]
    AiError(#[from] crate::ai::AiError),

    #[error("Validation error: {0}")]
    ValidationError(String),

    #[error("CBU generation error: {0}")]
    CbuGenerationError(String),

    #[error("DSL parsing error: {0}")]
    ParsingError(String),

    #[error("Configuration error: {0}")]
    ConfigError(String),
}

/// Result type for AI DSL operations
pub(crate) type AiDslResult<T> = Result<T, AiDslServiceError>;

/// Request for AI-powered onboarding creation
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct AiOnboardingRequest {
    /// Natural language description of the client and requirements
    pub instruction: String,

    /// Client/entity information
    pub client_name: String,
    pub jurisdiction: String,
    pub entity_type: String,

    /// Services requested
    pub services: Vec<String>,

    /// Compliance requirements
    pub compliance_level: Option<String>,

    /// Additional context
    pub context: HashMap<String, String>,

    /// AI provider to use (optional, defaults to OpenAI)
    pub ai_provider: Option<String>,
}

/// Response from AI-powered onboarding creation
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct AiOnboardingResponse {
    /// Generated CBU ID
    pub cbu_id: String,

    /// Created DSL instance
    pub dsl_instance: DslInstanceSummary,

    /// Generated DSL content
    pub generated_dsl: String,

    /// AI explanation of what was generated
    pub ai_explanation: String,

    /// AI confidence score
    pub ai_confidence: f64,

    /// Execution details
    pub execution_details: ExecutionDetails,

    /// Any warnings or suggestions
    pub warnings: Vec<String>,
    pub suggestions: Vec<String>,
}

/// Summary of created DSL instance
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct DslInstanceSummary {
    pub instance_id: String,
    pub domain: String,
    pub status: String,
    pub created_at: chrono::DateTime<Utc>,
    pub current_version: i32,
}

/// Execution details
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ExecutionDetails {
    pub template_used: String,
    pub compilation_successful: bool,
    pub validation_passed: bool,
    pub storage_keys: Option<String>,
    pub execution_time_ms: u64,
}

/// CBU ID generator
pub struct CbuGenerator;

impl CbuGenerator {
    /// Generate a unique CBU ID based on client information
    pub fn generate_cbu_id(client_name: &str, jurisdiction: &str, entity_type: &str) -> String {
        let sanitized_name = client_name
            .chars()
            .filter(|c| c.is_alphanumeric())
            .collect::<String>()
            .to_uppercase();

        let short_name = if sanitized_name.len() > 8 {
            &sanitized_name[..8]
        } else {
            &sanitized_name
        };

        let _timestamp = Utc::now().format("%m%d").to_string();
        let random_suffix: u16 = (Utc::now().timestamp_subsec_millis() % 1000) as u16;

        format!(
            "CBU-{}-{}-{}-{:03}",
            short_name,
            jurisdiction.to_uppercase(),
            entity_type.to_uppercase(),
            random_suffix
        )
    }

    /// Generate multiple CBU IDs for testing
    pub fn generate_test_cbu_ids(count: usize) -> Vec<String> {
        let test_clients = vec![
            ("TechCorp Ltd", "GB", "CORP"),
            ("Alpha Capital Partners", "KY", "FUND"),
            ("Global Investments SA", "LU", "FUND"),
            ("Singapore Holdings Pte", "SG", "CORP"),
            ("Zenith Financial Group", "US", "CORP"),
        ];

        (0..count)
            .map(|i| {
                let (name, jurisdiction, entity_type) = &test_clients[i % test_clients.len()];
                Self::generate_cbu_id(name, jurisdiction, entity_type)
            })
            .collect()
    }
}

/// AI-powered DSL Service - Now delegates to DSL Manager
pub struct AiDslService {
    dsl_manager: Arc<DslManager>,
}

impl AiDslService {
    /// Create a new AI DSL service with OpenAI client - now using DSL Manager
    pub async fn new_with_openai(ai_config: Option<AiConfig>) -> AiDslResult<Self> {
        let config = ai_config.unwrap_or_else(AiConfig::openai);
        let ai_client = Arc::new(OpenAiClient::new(config)?);

        let mut dsl_manager = DslManagerFactory::new();
        dsl_manager.set_ai_service(ai_client);

        Ok(Self {
            dsl_manager: Arc::new(dsl_manager),
        })
    }

    /// Create a new AI DSL service with custom AI client - now using DSL Manager
    pub(crate) fn new_with_client(ai_client: Arc<dyn AiService + Send + Sync>) -> Self {
        let mut dsl_manager = DslManagerFactory::new();
        dsl_manager.set_ai_service(ai_client);

        Self {
            dsl_manager: Arc::new(dsl_manager),
        }
    }

    /// Create complete onboarding workflow using AI - NOW DELEGATES TO DSL MANAGER
    pub async fn create_ai_onboarding(
        &self,
        request: AiOnboardingRequest,
    ) -> AiDslResult<AiOnboardingResponse> {
        info!(
            "Delegating AI-powered onboarding to DSL Manager for client: {}",
            request.client_name
        );

        // Convert to DSL Manager types
        let dsl_request = crate::dsl_manager::AiOnboardingRequest {
            instruction: request.instruction,
            client_name: request.client_name,
            jurisdiction: request.jurisdiction,
            entity_type: request.entity_type,
            services: request.services,
            compliance_level: request.compliance_level,
            context: request.context,
            ai_provider: request.ai_provider,
        };

        let context = DslContext {
            request_id: Uuid::new_v4().to_string(),
            user_id: "ai_dsl_service".to_string(),
            domain: "onboarding".to_string(),
            options: DslProcessingOptions::default(),
            audit_metadata: HashMap::new(),
        };

        // Delegate to DSL Manager
        let dsl_response = self
            .dsl_manager
            .process_ai_onboarding(dsl_request, context)
            .await
            .map_err(|e| AiDslServiceError::ValidationError(format!("DSL Manager error: {}", e)))?;

        // Convert response back to service types
        Ok(AiOnboardingResponse {
            cbu_id: dsl_response.cbu_id,
            dsl_instance: DslInstanceSummary {
                instance_id: dsl_response.dsl_instance.instance_id,
                domain: dsl_response.dsl_instance.domain,
                status: dsl_response.dsl_instance.status,
                created_at: dsl_response.dsl_instance.created_at,
                current_version: dsl_response.dsl_instance.current_version,
            },
            generated_dsl: dsl_response.generated_dsl,
            ai_explanation: dsl_response.ai_explanation,
            ai_confidence: dsl_response.ai_confidence,
            execution_details: ExecutionDetails {
                template_used: dsl_response.execution_details.template_used,
                compilation_successful: dsl_response.execution_details.compilation_successful,
                validation_passed: dsl_response.execution_details.validation_passed,
                storage_keys: dsl_response.execution_details.storage_keys,
                execution_time_ms: dsl_response.execution_details.execution_time_ms,
            },
            warnings: dsl_response.warnings,
            suggestions: dsl_response.suggestions,
        })
    }

    // These methods are now handled by DSL Manager internally
    // Keeping this comment as a marker that functionality moved to DSL Manager

    /// Health check for AI service - NOW DELEGATES TO DSL MANAGER
    pub async fn health_check(&self) -> AiDslResult<HealthCheckResult> {
        info!("Delegating health check to DSL Manager");

        let dsl_health = self
            .dsl_manager
            .comprehensive_health_check()
            .await
            .map_err(|e| {
                AiDslServiceError::ValidationError(format!(
                    "DSL Manager health check failed: {}",
                    e
                ))
            })?;

        Ok(HealthCheckResult {
            ai_service_healthy: dsl_health.ai_service_healthy,
            dsl_manager_healthy: dsl_health.dsl_manager_healthy,
            database_healthy: dsl_health.backend_healthy,
            overall_healthy: dsl_health.overall_healthy,
        })
    }

    /// Generate test CBU IDs - NOW DELEGATES TO DSL MANAGER
    pub fn generate_test_cbus(&self, count: usize) -> Vec<String> {
        self.dsl_manager.generate_test_cbu_ids(count)
    }

    /// Validate AI-generated DSL using AI validation - NOW DELEGATES TO DSL MANAGER
    pub async fn validate_dsl_with_ai(&self, dsl_content: &str) -> AiDslResult<ValidationResult> {
        info!("Delegating DSL validation to DSL Manager");

        let context = DslContext {
            request_id: Uuid::new_v4().to_string(),
            user_id: "ai_dsl_service".to_string(),
            domain: "validation".to_string(),
            options: DslProcessingOptions::default(),
            audit_metadata: HashMap::new(),
        };

        let validation_result = self
            .dsl_manager
            .validate_dsl_with_ai(dsl_content, context)
            .await
            .map_err(|e| {
                AiDslServiceError::ValidationError(format!("DSL Manager validation error: {}", e))
            })?;

        Ok(ValidationResult {
            valid: validation_result.valid,
            confidence: validation_result.ai_confidence,
            issues: validation_result.ai_issues,
            suggestions: validation_result.ai_suggestions,
            explanation: validation_result.ai_explanation,
        })
    }
}

/// Health check result
#[derive(Debug, Serialize, Deserialize)]
pub(crate) struct HealthCheckResult {
    pub ai_service_healthy: bool,
    pub dsl_manager_healthy: bool,
    pub database_healthy: bool,
    pub overall_healthy: bool,
}

/// Validation result from AI
#[derive(Debug, Serialize, Deserialize)]
pub struct ValidationResult {
    pub valid: bool,
    pub confidence: f64,
    pub issues: Vec<String>,
    pub suggestions: Vec<String>,
    pub explanation: String,
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_cbu_generator() {
        let cbu_id = CbuGenerator::generate_cbu_id("TechCorp Ltd", "GB", "CORP");
        assert!(cbu_id.starts_with("CBU-TECHCORP-GB-CORP-"));
        assert!(cbu_id.len() > 20);
    }

    #[test]
    fn test_generate_multiple_cbu_ids() {
        let cbu_ids = CbuGenerator::generate_test_cbu_ids(5);
        assert_eq!(cbu_ids.len(), 5);

        // All should be unique
        let mut unique_ids = std::collections::HashSet::new();
        for id in &cbu_ids {
            unique_ids.insert(id);
        }
        assert_eq!(unique_ids.len(), cbu_ids.len());
    }

    #[test]
    fn test_ai_onboarding_request_serialization() {
        let request = AiOnboardingRequest {
            instruction: "Create onboarding for tech company".to_string(),
            client_name: "TechCorp Ltd".to_string(),
            jurisdiction: "GB".to_string(),
            entity_type: "CORP".to_string(),
            services: vec!["CUSTODY".to_string()],
            compliance_level: Some("standard".to_string()),
            context: HashMap::new(),
            ai_provider: Some("openai".to_string()),
        };

        let json = serde_json::to_string(&request).unwrap();
        let deserialized: AiOnboardingRequest = serde_json::from_str(&json).unwrap();
        assert_eq!(deserialized.client_name, request.client_name);
    }
}
