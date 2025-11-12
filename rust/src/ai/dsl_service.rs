//! Enhanced DSL Service with Canonical KYC Templates
//!
//! This service provides AI-powered DSL generation using canonical templates
//! for KYC orchestration workflows. It ensures all generated DSL conforms
//! to the canonical v3.1 specification with proper verb and key normalization.
//!
//! Note: This replaces the broken gemini.rs and openai.rs clients which have
//! incompatible interfaces. This implementation provides a clean, modern
//! architecture specifically designed for canonical DSL generation.

// use crate::ai::gemini::GeminiClient;
use crate::ai::openai::OpenAiClient;
use crate::ai::{AiConfig, AiError, AiResult, AiService};
use serde::{Deserialize, Serialize};
use std::collections::HashMap;
use uuid::Uuid;

/// Enhanced DSL service with canonical template support
pub struct AiDslService {
    /// AI client for generation
    ai_client: Box<dyn AiService + Send + Sync>,
    /// Template cache for canonical DSL patterns
    template_cache: HashMap<String, String>,
    /// Service configuration
    config: AiConfig,
}

/// Request for KYC case generation
#[derive(Debug, Clone, Serialize)]
pub struct KycCaseRequest {
    /// Client name for the KYC investigation
    pub client_name: String,
    /// Legal jurisdiction code (GB, US, KY, etc.)
    pub jurisdiction: String,
    /// Type of entity being investigated
    pub entity_type: String,
    /// Analyst assigned to the case
    pub analyst_id: String,
    /// Business reference number
    pub business_reference: Option<String>,
    /// Additional entity properties
    pub entity_properties: Option<HashMap<String, String>>,
    /// Expected UBO threshold (default 25.0)
    pub ubo_threshold: Option<f64>,
}

/// Request for UBO analysis generation
#[derive(Debug, Clone, Serialize)]
pub struct UboAnalysisRequest {
    /// Target entity for UBO analysis
    pub target_entity_name: String,
    /// Target entity type
    pub target_entity_type: String,
    /// Legal jurisdiction
    pub jurisdiction: String,
    /// UBO threshold percentage
    pub ubo_threshold: f64,
    /// Known ownership structure
    pub ownership_structure: Option<Vec<OwnershipLink>>,
    /// Analyst conducting the analysis
    pub analyst_id: String,
}

/// Ownership link definition for UBO analysis
#[derive(Debug, Clone, Serialize)]
pub(crate) struct OwnershipLink {
    pub from_entity_name: String,
    pub to_entity_name: String,
    pub relationship_type: String,
    pub ownership_percentage: Option<f64>,
    pub control_type: Option<String>,
}

/// Response from DSL generation
#[derive(Debug, Clone, Deserialize)]
pub(crate) struct DslGenerationResponse {
    /// Generated canonical DSL content
    pub generated_dsl: String,
    /// Template used for generation
    pub template_used: String,
    /// Explanation of the generated workflow
    pub explanation: String,
    /// Confidence score for the generation
    pub confidence: f64,
    /// Generated entity IDs for reference
    pub entity_ids: Vec<String>,
    /// Generated document IDs for reference
    pub document_ids: Vec<String>,
    /// Warnings or notes about the generation
    pub warnings: Vec<String>,
}

/// Canonical DSL template structure
#[derive(Debug, Clone)]
pub(crate) struct KycDslTemplate {
    /// Template name
    pub name: String,
    /// Template content with placeholders
    pub content: String,
    /// Required variables for substitution
    pub variables: Vec<String>,
    /// Template description
    pub description: String,
}

impl AiDslService {
    /// Create new DSL service with OpenAI client
    pub async fn new_with_openai(api_key: Option<String>) -> AiResult<Self> {
        let config = AiConfig {
            api_key: api_key.unwrap_or_else(|| std::env::var("OPENAI_API_KEY").unwrap_or_default()),
            model: "gpt-4".to_string(),
            max_tokens: Some(4096),
            temperature: Some(0.1),
            timeout_seconds: 60,
        };

        let client = OpenAiClient::new(config.clone())?;

        let mut service = Self {
            ai_client: Box::new(client),
            template_cache: HashMap::new(),
            config,
        };

        service.load_canonical_templates().await?;
        Ok(service)
    }

    /// Create new DSL service with Gemini client (TEMPORARILY DISABLED)
    pub async fn new_with_gemini(_api_key: Option<String>) -> AiResult<Self> {
        Err(AiError::ApiError("Gemini client temporarily disabled due to API compatibility issues. Use OpenAI client instead.".to_string()))
        // let config = AiConfig {
        //     api_key: api_key.unwrap_or_else(|| std::env::var("GEMINI_API_KEY").unwrap_or_default()),
        //     model: "gemini-2.5-flash-preview-09-2025".to_string(),
        //     max_tokens: Some(8192),
        //     temperature: Some(0.1),
        //     timeout_seconds: 60,
        // };
        //
        // let client = GeminiClient::new(config.clone())?;
        //
        // let mut service = Self {
        //     ai_client: Box::new(client),
        //     template_cache: HashMap::new(),
        //     config,
        // };
        //
        // service.load_canonical_templates().await?;
        // Ok(service)
    }

    /// Generate canonical KYC case workflow
    pub async fn generate_canonical_kyc_case(
        &self,
        request: KycCaseRequest,
    ) -> AiResult<DslGenerationResponse> {
        let template = self.load_canonical_kyc_template()?;
        let prompt = self.build_canonical_kyc_prompt(&template, &request)?;

        let ai_request = crate::ai::AiDslRequest {
            instruction: prompt,
            context: Some(self.build_context_map(&request)),
            response_type: crate::ai::AiResponseType::DslGeneration,
            temperature: Some(0.1),
            max_tokens: Some(4096),
        };

        let ai_response = self.ai_client.generate_dsl(ai_request).await?;

        // Parse and enhance the response
        let (entity_ids, document_ids) = self.extract_generated_ids(&ai_response.generated_dsl);

        Ok(DslGenerationResponse {
            generated_dsl: ai_response.generated_dsl,
            template_used: "kyc_investigation".to_string(),
            explanation: ai_response.explanation,
            confidence: ai_response.confidence.unwrap_or(0.85),
            entity_ids,
            document_ids,
            warnings: ai_response.warnings.unwrap_or_default(),
        })
    }

    /// Generate canonical UBO analysis workflow
    pub async fn generate_canonical_ubo_analysis(
        &self,
        request: UboAnalysisRequest,
    ) -> AiResult<DslGenerationResponse> {
        let template = self.load_canonical_ubo_template()?;
        let prompt = self.build_canonical_ubo_prompt(&template, &request)?;

        let ai_request = crate::ai::AiDslRequest {
            instruction: prompt,
            context: Some(self.build_ubo_context_map(&request)),
            response_type: crate::ai::AiResponseType::DslGeneration,
            temperature: Some(0.1),
            max_tokens: Some(6144),
        };

        let ai_response = self.ai_client.generate_dsl(ai_request).await?;

        let (entity_ids, document_ids) = self.extract_generated_ids(&ai_response.generated_dsl);

        Ok(DslGenerationResponse {
            generated_dsl: ai_response.generated_dsl,
            template_used: "ubo_analysis".to_string(),
            explanation: ai_response.explanation,
            confidence: ai_response.confidence.unwrap_or(0.85),
            entity_ids,
            document_ids,
            warnings: ai_response.warnings.unwrap_or_default(),
        })
    }

    /// Load canonical templates into cache
    async fn load_canonical_templates(&mut self) -> AiResult<()> {
        // Load KYC investigation template
        let kyc_template = include_str!("prompts/canonical/kyc_investigation.template");
        if kyc_template.is_empty() {
            return Err(AiError::InvalidResponse(
                "KYC template is empty".to_string(),
            ));
        }
        self.template_cache
            .insert("kyc_investigation".to_string(), kyc_template.to_string());

        // Load UBO analysis template
        let ubo_template = include_str!("prompts/canonical/ubo_analysis.template");
        if ubo_template.is_empty() {
            return Err(AiError::InvalidResponse(
                "UBO template is empty".to_string(),
            ));
        }
        self.template_cache
            .insert("ubo_analysis".to_string(), ubo_template.to_string());

        // Load canonical instructions
        let instructions = include_str!("prompts/canonical/canonical_instructions.md");
        if instructions.is_empty() {
            return Err(AiError::InvalidResponse(
                "Canonical instructions are empty".to_string(),
            ));
        }
        self.template_cache.insert(
            "canonical_instructions".to_string(),
            instructions.to_string(),
        );

        Ok(())
    }

    /// Load canonical KYC template
    fn load_canonical_kyc_template(&self) -> AiResult<KycDslTemplate> {
        let content = self
            .template_cache
            .get("kyc_investigation")
            .ok_or_else(|| AiError::InvalidResponse("KYC template not loaded".to_string()))?;

        Ok(KycDslTemplate {
            name: "kyc_investigation".to_string(),
            content: content.clone(),
            variables: vec![
                "case-id".to_string(),
                "business-reference".to_string(),
                "analyst-id".to_string(),
                "case-title".to_string(),
                "primary-entity-id".to_string(),
                "entity-type".to_string(),
                "legal-name".to_string(),
                "jurisdiction".to_string(),
            ],
            description: "Canonical KYC investigation workflow template".to_string(),
        })
    }

    /// Load canonical UBO template
    fn load_canonical_ubo_template(&self) -> AiResult<KycDslTemplate> {
        let content = self
            .template_cache
            .get("ubo_analysis")
            .ok_or_else(|| AiError::InvalidResponse("UBO template not loaded".to_string()))?;

        Ok(KycDslTemplate {
            name: "ubo_analysis".to_string(),
            content: content.clone(),
            variables: vec![
                "ubo-case-id".to_string(),
                "target-entity-id".to_string(),
                "target-entity-type".to_string(),
                "ubo-threshold".to_string(),
                "analyst-id".to_string(),
            ],
            description: "Canonical UBO analysis workflow template".to_string(),
        })
    }

    /// Build canonical prompt for KYC case generation
    fn build_canonical_kyc_prompt(
        &self,
        template: &KycDslTemplate,
        request: &KycCaseRequest,
    ) -> AiResult<String> {
        let instructions = self
            .template_cache
            .get("canonical_instructions")
            .ok_or_else(|| {
                AiError::InvalidResponse("Canonical instructions not loaded".to_string())
            })?;

        let case_id = format!(
            "kyc-case-{}-{}",
            request.client_name.to_lowercase().replace(" ", "-"),
            &Uuid::new_v4().to_string()[..8]
        );

        let entity_id = format!(
            "entity-{}",
            request.client_name.to_lowercase().replace(" ", "-")
        );

        let business_ref = request
            .business_reference
            .clone()
            .unwrap_or_else(|| format!("KYC-{}", chrono::Utc::now().format("%Y-%m-%d")));

        let prompt = format!(
            r#"You are an expert KYC analyst generating canonical DSL for a KYC investigation.

CRITICAL INSTRUCTIONS:
{}

TEMPLATE TO FOLLOW:
The following template shows the exact canonical structure you must follow.
Replace the {{placeholder}} variables with actual values based on the request.

{}

REQUEST DETAILS:
- Client Name: {}
- Jurisdiction: {}
- Entity Type: {}
- Analyst ID: {}
- Business Reference: {}

GENERATED IDs TO USE:
- Case ID: {}
- Primary Entity ID: {}

REQUIREMENTS:
1. Follow the template structure EXACTLY
2. Use ONLY canonical verbs and keys listed in the instructions
3. Replace ALL {{placeholder}} variables with appropriate values
4. Generate realistic supporting entity IDs (GP, beneficial owners)
5. Create appropriate document IDs with realistic file hashes
6. Include complete workflow from case creation to approval
7. Ensure all entity.link entries use :relationship-props maps
8. Use workflow.transition between phases with clear reasons
9. Include UBO calculation and outcome at the end

Generate complete, executable canonical DSL following this template:"#,
            instructions,
            template.content,
            request.client_name,
            request.jurisdiction,
            request.entity_type,
            request.analyst_id,
            business_ref,
            case_id,
            entity_id
        );

        Ok(prompt)
    }

    /// Build canonical prompt for UBO analysis generation
    fn build_canonical_ubo_prompt(
        &self,
        template: &KycDslTemplate,
        request: &UboAnalysisRequest,
    ) -> AiResult<String> {
        let instructions = self
            .template_cache
            .get("canonical_instructions")
            .ok_or_else(|| {
                AiError::InvalidResponse("Canonical instructions not loaded".to_string())
            })?;

        let case_id = format!(
            "ubo-case-{}-{}",
            request.target_entity_name.to_lowercase().replace(" ", "-"),
            &Uuid::new_v4().to_string()[..8]
        );

        let target_entity_id = format!(
            "target-{}",
            request.target_entity_name.to_lowercase().replace(" ", "-")
        );

        let prompt = format!(
            r#"You are an expert UBO analyst generating canonical DSL for beneficial ownership analysis.

CRITICAL INSTRUCTIONS:
{}

TEMPLATE TO FOLLOW:
{}

REQUEST DETAILS:
- Target Entity: {}
- Entity Type: {}
- Jurisdiction: {}
- UBO Threshold: {}%
- Analyst ID: {}

GENERATED IDs TO USE:
- UBO Case ID: {}
- Target Entity ID: {}

REQUIREMENTS:
1. Follow the UBO analysis template structure EXACTLY
2. Use ONLY canonical verbs and keys from the instructions
3. Create realistic ownership chain with holding companies
4. Generate 2-3 natural person beneficial owners
5. Include direct and indirect ownership links
6. Add control relationships where applicable
7. Create supporting documents (articles, registers, etc.)
8. Link all documents as evidence using document.use
9. Calculate effective ownership percentages
10. Generate final UBO outcome with proper prongs analysis

Generate complete canonical DSL for UBO analysis:"#,
            instructions,
            template.content,
            request.target_entity_name,
            request.target_entity_type,
            request.jurisdiction,
            request.ubo_threshold,
            request.analyst_id,
            case_id,
            target_entity_id
        );

        Ok(prompt)
    }

    /// Build context map for KYC request
    fn build_context_map(&self, request: &KycCaseRequest) -> HashMap<String, String> {
        let mut context = HashMap::new();
        context.insert("client_name".to_string(), request.client_name.clone());
        context.insert("jurisdiction".to_string(), request.jurisdiction.clone());
        context.insert("entity_type".to_string(), request.entity_type.clone());
        context.insert("analyst_id".to_string(), request.analyst_id.clone());

        if let Some(ref business_ref) = request.business_reference {
            context.insert("business_reference".to_string(), business_ref.clone());
        }

        if let Some(threshold) = request.ubo_threshold {
            context.insert("ubo_threshold".to_string(), threshold.to_string());
        }

        context
    }

    /// Build context map for UBO request
    fn build_ubo_context_map(&self, request: &UboAnalysisRequest) -> HashMap<String, String> {
        let mut context = HashMap::new();
        context.insert(
            "target_entity".to_string(),
            request.target_entity_name.clone(),
        );
        context.insert(
            "entity_type".to_string(),
            request.target_entity_type.clone(),
        );
        context.insert("jurisdiction".to_string(), request.jurisdiction.clone());
        context.insert(
            "ubo_threshold".to_string(),
            request.ubo_threshold.to_string(),
        );
        context.insert("analyst_id".to_string(), request.analyst_id.clone());

        context
    }

    /// Extract generated entity and document IDs from DSL
    fn extract_generated_ids(&self, dsl_content: &str) -> (Vec<String>, Vec<String>) {
        let mut entity_ids = Vec::new();
        let mut document_ids = Vec::new();

        // Simple regex-like extraction for entity-id patterns
        for line in dsl_content.lines() {
            if line.contains(":entity-id") {
                if let Some(start) = line.find("\"") {
                    if let Some(end) = line[start + 1..].find("\"") {
                        let entity_id = line[start + 1..start + 1 + end].to_string();
                        if !entity_ids.contains(&entity_id) {
                            entity_ids.push(entity_id);
                        }
                    }
                }
            }

            if line.contains(":document-id") {
                if let Some(start) = line.find("\"") {
                    if let Some(end) = line[start + 1..].find("\"") {
                        let document_id = line[start + 1..start + 1 + end].to_string();
                        if !document_ids.contains(&document_id) {
                            document_ids.push(document_id);
                        }
                    }
                }
            }
        }

        (entity_ids, document_ids)
    }

    /// Validate generated DSL uses canonical forms
    ///
    /// Returns Ok(warnings) if validation passes with minor warnings,
    /// or Err(errors) if critical canonical form violations are found.
    pub(crate) fn validate_canonical_dsl(&self, dsl_content: &str) -> Result<Vec<String>, Vec<String>> {
        let mut warnings = Vec::new();
        let mut errors = Vec::new();

        if dsl_content.trim().is_empty() {
            errors.push("DSL content is empty".to_string());
            return Err(errors);
        }

        // Check for forbidden legacy verbs
        let legacy_verbs = vec![
            "kyc.start_case",
            "kyc.transition_state",
            "kyc.add_finding",
            "kyc.approve_case",
            "ubo.link_ownership",
            "ubo.link_control",
            "ubo.add_evidence",
        ];

        for verb in legacy_verbs {
            if dsl_content.contains(verb) {
                errors.push(format!(
                    "Legacy verb '{}' detected. Use canonical equivalent.",
                    verb
                ));
            }
        }

        // Check for forbidden legacy keys
        let legacy_keys = vec![
            ":new_state",
            ":file_hash",
            ":approver_id",
            ":case_id",
            ":entity_id",
            ":document_id",
        ];

        for key in legacy_keys {
            if dsl_content.contains(key) {
                warnings.push(format!(
                    "Legacy key '{}' detected. Should use kebab-case.",
                    key
                ));
            }
        }

        // Check for required canonical patterns
        if dsl_content.contains("entity.link") && !dsl_content.contains(":relationship-props") {
            errors
                .push("entity.link entries must use :relationship-props map structure".to_string());
        }

        if dsl_content.contains("document.use") && !dsl_content.contains(":evidence.of-link") {
            warnings.push(
                "document.use should include :evidence.of-link for proper evidence linking"
                    .to_string(),
            );
        }

        if errors.is_empty() {
            Ok(warnings)
        } else {
            Err(errors)
        }
    }

    /// Get service health status
    pub async fn health_check(&self) -> AiResult<bool> {
        self.ai_client.health_check().await
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_kyc_case_request_creation() {
        let request = KycCaseRequest {
            client_name: "Test Hedge Fund Ltd".to_string(),
            jurisdiction: "GB".to_string(),
            entity_type: "HEDGE_FUND".to_string(),
            analyst_id: "analyst-1".to_string(),
            business_reference: Some("KYC-2025-001".to_string()),
            entity_properties: None,
            ubo_threshold: Some(25.0),
        };

        assert_eq!(request.client_name, "Test Hedge Fund Ltd");
        assert_eq!(request.jurisdiction, "GB");
        assert_eq!(request.ubo_threshold, Some(25.0));
    }

    #[test]
    fn test_ubo_analysis_request_creation() {
        let request = UboAnalysisRequest {
            target_entity_name: "Target Fund Ltd".to_string(),
            target_entity_type: "INVESTMENT_FUND".to_string(),
            jurisdiction: "KY".to_string(),
            ubo_threshold: 25.0,
            ownership_structure: None,
            analyst_id: "analyst-2".to_string(),
        };

        assert_eq!(request.target_entity_name, "Target Fund Ltd");
        assert_eq!(request.ubo_threshold, 25.0);
    }

    #[test]
    fn test_extract_generated_ids() {
        let service = AiDslService {
            ai_client: Box::new(MockAiClient::new()),
            template_cache: HashMap::new(),
            config: AiConfig::default(),
        };

        let dsl = r#"
            (entity.register :entity-id "test-entity-1")
            (document.catalog :document-id "doc-123")
            (entity.link :from-entity "test-entity-1" :to-entity "test-entity-2")
        "#;

        let (entity_ids, document_ids) = service.extract_generated_ids(dsl);

        assert!(entity_ids.contains(&"test-entity-1".to_string()));
        assert!(document_ids.contains(&"doc-123".to_string()));
    }

    #[test]
    fn test_validate_canonical_dsl() {
        let service = AiDslService {
            ai_client: Box::new(MockAiClient::new()),
            template_cache: HashMap::new(),
            config: AiConfig::default(),
        };

        // Test canonical DSL
        let canonical_dsl = r#"
            (case.create :case-id "test-case")
            (entity.link :link-id "link-1" :relationship-props {:ownership-percentage 50.0})
        "#;

        let result = service.validate_canonical_dsl(canonical_dsl);
        assert!(result.is_ok());

        // Test legacy DSL
        let legacy_dsl = r#"
            (kyc.start_case :case_id "test-case")
            (ubo.link_ownership :new_state "verified")
        "#;

        let result = service.validate_canonical_dsl(legacy_dsl);
        assert!(result.is_err());
    }

    // Mock AI client for testing
    struct MockAiClient {
        config: AiConfig,
    }

    impl MockAiClient {
        fn new() -> Self {
            Self {
                config: AiConfig::default(),
            }
        }
    }

    #[async_trait::async_trait]
    impl AiService for MockAiClient {
        async fn generate_dsl(
            &self,
            _request: crate::ai::AiDslRequest,
        ) -> AiResult<crate::ai::AiDslResponse> {
            Ok(crate::ai::AiDslResponse {
                generated_dsl: "(case.create :case-id \"test\")".to_string(),
                explanation: "Test DSL".to_string(),
                confidence: Some(0.95),
                changes: None,
                warnings: None,
                suggestions: None,
            })
        }

        async fn health_check(&self) -> AiResult<bool> {
            Ok(true)
        }

        fn config(&self) -> &AiConfig {
            &self.config
        }
    }
}
