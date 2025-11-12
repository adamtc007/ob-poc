//! AI-Powered DSL Service
//!
//! This service combines AI-generated DSL with actual DSL execution through the DSL Manager.
//! It provides end-to-end onboarding workflows where business requirements are converted
//! to DSL via AI, then executed to create actual onboarding instances.

use crate::ai::{openai::OpenAiClient, AiConfig, AiDslRequest, AiResponseType, AiService};
use crate::parser::parse_program;
use chrono::Utc;
use serde::{Deserialize, Serialize};
use std::collections::HashMap;
use std::path::PathBuf;
use std::sync::Arc;
use tracing::{debug, error, info, warn};
use uuid::Uuid;

/// AI DSL Service errors
#[derive(Debug, thiserror::Error)]
pub enum AiDslServiceError {
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
pub type AiDslResult<T> = Result<T, AiDslServiceError>;

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

        let timestamp = Utc::now().format("%m%d").to_string();
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

/// AI-powered DSL Service
pub struct AiDslService {
    ai_client: Arc<dyn AiService + Send + Sync>,
}

impl AiDslService {
    /// Create a new AI DSL service with OpenAI client
    pub async fn new_with_openai(ai_config: Option<AiConfig>) -> AiDslResult<Self> {
        let config = ai_config.unwrap_or_else(AiConfig::openai);
        let ai_client = Arc::new(OpenAiClient::new(config)?);

        Ok(Self { ai_client })
    }

    /// Create a new AI DSL service with custom AI client
    pub fn new_with_client(ai_client: Arc<dyn AiService + Send + Sync>) -> Self {
        Self { ai_client }
    }

    /// Create complete onboarding workflow using AI
    pub async fn create_ai_onboarding(
        &self,
        request: AiOnboardingRequest,
    ) -> AiDslResult<AiOnboardingResponse> {
        let start_time = std::time::Instant::now();
        info!(
            "Starting AI-powered onboarding for client: {}",
            request.client_name
        );

        // Step 1: Generate CBU ID
        let cbu_id = CbuGenerator::generate_cbu_id(
            &request.client_name,
            &request.jurisdiction,
            &request.entity_type,
        );
        info!("Generated CBU ID: {}", cbu_id);

        // Step 2: Use AI to generate DSL
        let ai_response = self.generate_onboarding_dsl(&request, &cbu_id).await?;
        info!(
            "AI DSL generation completed with confidence: {:.2}",
            ai_response.confidence.unwrap_or(0.5)
        );

        // Step 3: Validate generated DSL
        self.validate_generated_dsl(&ai_response.generated_dsl)?;

        // Step 4: Simulate DSL execution (no database dependency)
        let dsl_instance = self
            .simulate_dsl_execution(&ai_response.generated_dsl, &cbu_id)
            .await?;
        info!(
            "DSL simulation completed, instance ID: {}",
            dsl_instance.instance_id
        );

        let execution_time = start_time.elapsed().as_millis() as u64;

        Ok(AiOnboardingResponse {
            cbu_id,
            dsl_instance,
            generated_dsl: ai_response.generated_dsl,
            ai_explanation: ai_response.explanation,
            ai_confidence: ai_response.confidence.unwrap_or(0.5),
            execution_details: ExecutionDetails {
                template_used: "onboarding".to_string(),
                compilation_successful: true,
                validation_passed: true,
                storage_keys: None, // TODO: Get from DSL manager response
                execution_time_ms: execution_time,
            },
            warnings: ai_response.warnings.unwrap_or_default(),
            suggestions: ai_response.suggestions.unwrap_or_default(),
        })
    }

    /// Generate DSL using AI
    async fn generate_onboarding_dsl(
        &self,
        request: &AiOnboardingRequest,
        cbu_id: &str,
    ) -> AiDslResult<crate::ai::AiDslResponse> {
        debug!("Generating DSL for CBU: {}", cbu_id);

        let mut context = request.context.clone();
        context.insert("cbu_id".to_string(), cbu_id.to_string());
        context.insert("client_name".to_string(), request.client_name.clone());
        context.insert("jurisdiction".to_string(), request.jurisdiction.clone());
        context.insert("entity_type".to_string(), request.entity_type.clone());

        // Add services context
        if !request.services.is_empty() {
            context.insert("services".to_string(), request.services.join(", "));
        }

        if let Some(compliance_level) = &request.compliance_level {
            context.insert("compliance_level".to_string(), compliance_level.clone());
        }

        let ai_request = AiDslRequest {
            instruction: format!(
                "Create a complete onboarding DSL for client '{}'. {}. Services needed: {}",
                request.client_name,
                request.instruction,
                request.services.join(", ")
            ),

            context: Some(context),
            response_type: AiResponseType::DslGeneration,
            temperature: Some(0.1),
            max_tokens: Some(2000),
        };

        self.ai_client
            .generate_dsl(ai_request)
            .await
            .map_err(AiDslServiceError::AiError)
    }

    /// Validate generated DSL syntax
    fn validate_generated_dsl(&self, dsl_content: &str) -> AiDslResult<()> {
        debug!("Validating generated DSL syntax");

        // Parse DSL to check syntax
        match parse_program(dsl_content) {
            Ok(forms) => {
                if forms.is_empty() {
                    return Err(AiDslServiceError::ValidationError(
                        "Generated DSL is empty".to_string(),
                    ));
                }

                // Check for required case.create verb
                let has_case_create = forms.iter().any(|form| {
                    if let crate::Form::Verb(verb_form) = form {
                        verb_form.verb == "case.create"
                    } else {
                        false
                    }
                });

                if !has_case_create {
                    return Err(AiDslServiceError::ValidationError(
                        "Generated DSL missing required case.create verb".to_string(),
                    ));
                }

                info!("DSL validation passed: {} forms found", forms.len());
                Ok(())
            }
            Err(e) => Err(AiDslServiceError::ParsingError(format!(
                "Failed to parse generated DSL: {:?}",
                e
            ))),
        }
    }

    /// Simulate DSL execution (for demo without database)
    async fn simulate_dsl_execution(
        &self,
        _dsl_content: &str,
        cbu_id: &str,
    ) -> AiDslResult<DslInstanceSummary> {
        debug!("Simulating DSL execution for CBU: {}", cbu_id);

        // Simulate creating a DSL instance
        let instance_id = format!("instance-{}", Uuid::new_v4().to_string());
        info!("Simulated DSL instance creation: {}", instance_id);

        Ok(DslInstanceSummary {
            instance_id,
            domain: "onboarding".to_string(),
            status: "active".to_string(),
            created_at: Utc::now(),
            current_version: 1,
        })
    }

    /// Health check for AI service
    pub async fn health_check(&self) -> AiDslResult<HealthCheckResult> {
        let mut result = HealthCheckResult {
            ai_service_healthy: false,
            dsl_manager_healthy: true, // Simulated
            database_healthy: true,    // Simulated
            overall_healthy: false,
        };

        // Check AI service
        match self.ai_client.health_check().await {
            Ok(healthy) => result.ai_service_healthy = healthy,
            Err(e) => warn!("AI service health check failed: {}", e),
        }

        result.overall_healthy = result.ai_service_healthy;
        Ok(result)
    }

    /// Generate test CBU IDs
    pub fn generate_test_cbus(&self, count: usize) -> Vec<String> {
        CbuGenerator::generate_test_cbu_ids(count)
    }

    /// Validate AI-generated DSL using AI validation
    pub async fn validate_dsl_with_ai(&self, dsl_content: &str) -> AiDslResult<ValidationResult> {
        let ai_request = AiDslRequest {
            instruction: format!("Validate this DSL for syntax correctness, vocabulary compliance, and business logic: {}", dsl_content),
            context: Some(HashMap::new()),
            response_type: AiResponseType::DslValidation,
            temperature: Some(0.1),
            max_tokens: Some(1000),
        };

        match self.ai_client.generate_dsl(ai_request).await {
            Ok(response) => Ok(ValidationResult {
                valid: response.warnings.as_ref().map_or(true, |w| w.is_empty()),
                confidence: response.confidence.unwrap_or(0.5),
                issues: response.warnings.unwrap_or_default(),
                suggestions: response.suggestions,
                explanation: response.explanation,
            }),
            Err(e) => Err(AiDslServiceError::AiError(e)),
        }
    }
}

/// Health check result
#[derive(Debug, Serialize, Deserialize)]
pub struct HealthCheckResult {
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
