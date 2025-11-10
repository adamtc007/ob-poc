//! Google Gemini API Client Implementation
//!
//! This module provides a complete implementation of the Gemini API client
//! for DSL generation, transformation, and validation tasks.

use super::{AiConfig, AiDslRequest, AiDslResponse, AiError, AiResult, AiService};
use reqwest::Client;
use serde::{Deserialize, Serialize};
use serde_json::json;
use std::time::Duration;
use tracing::{debug, error, info, warn};

/// Gemini API client
#[derive(Debug, Clone)]
pub struct GeminiClient {
    config: AiConfig,
    client: Client,
    base_url: String,
}

/// Gemini API request format
#[derive(Debug, Serialize)]
struct GeminiRequest {
    contents: Vec<GeminiContent>,
    #[serde(skip_serializing_if = "Option::is_none")]
    generation_config: Option<GeminiGenerationConfig>,
}

/// Gemini content structure
#[derive(Debug, Serialize)]
struct GeminiContent {
    parts: Vec<GeminiPart>,
}

/// Gemini content part
#[derive(Debug, Serialize)]
struct GeminiPart {
    text: String,
}

/// Gemini generation configuration
#[derive(Debug, Serialize)]
struct GeminiGenerationConfig {
    #[serde(skip_serializing_if = "Option::is_none")]
    temperature: Option<f32>,
    #[serde(skip_serializing_if = "Option::is_none")]
    max_output_tokens: Option<u32>,
    #[serde(skip_serializing_if = "Option::is_none")]
    top_p: Option<f32>,
    #[serde(skip_serializing_if = "Option::is_none")]
    top_k: Option<u32>,
}

/// Gemini API response format
#[derive(Debug, Deserialize)]
struct GeminiResponse {
    candidates: Vec<GeminiCandidate>,
    #[serde(default)]
    usage_metadata: Option<GeminiUsageMetadata>,
}

/// Gemini candidate response
#[derive(Debug, Deserialize)]
struct GeminiCandidate {
    content: GeminiResponseContent,
    #[serde(default)]
    finish_reason: Option<String>,
    #[serde(default)]
    safety_ratings: Option<Vec<GeminiSafetyRating>>,
}

/// Gemini response content
#[derive(Debug, Deserialize)]
struct GeminiResponseContent {
    parts: Vec<GeminiResponsePart>,
}

/// Gemini response part
#[derive(Debug, Deserialize)]
struct GeminiResponsePart {
    text: String,
}

/// Gemini usage metadata
#[derive(Debug, Deserialize)]
struct GeminiUsageMetadata {
    #[serde(default)]
    prompt_token_count: Option<u32>,
    #[serde(default)]
    candidates_token_count: Option<u32>,
    #[serde(default)]
    total_token_count: Option<u32>,
}

/// Gemini safety rating
#[derive(Debug, Deserialize)]
struct GeminiSafetyRating {
    category: String,
    probability: String,
}

impl GeminiClient {
    /// Create a new Gemini client
    pub fn new(config: AiConfig) -> AiResult<Self> {
        if config.api_key.is_empty() {
            return Err(AiError::AuthenticationError);
        }

        let client = Client::builder()
            .timeout(Duration::from_secs(config.timeout_seconds))
            .build()
            .map_err(AiError::HttpError)?;

        let base_url = "https://generativelanguage.googleapis.com/v1beta/models".to_string();

        Ok(Self {
            config,
            client,
            base_url,
        })
    }

    /// Build the system prompt for DSL operations
    fn build_system_prompt(&self, request: &AiDslRequest) -> String {
        let base_prompt = r#"You are an expert DSL (Domain Specific Language) architect for financial onboarding workflows.

APPROVED DSL VERBS (MUST USE ONLY THESE):
- case.create, case.update, case.validate, case.approve, case.close
- entity.register, entity.classify, entity.link, identity.verify, identity.attest
- products.add, products.configure, services.discover, services.provision, services.activate
- kyc.start, kyc.collect, kyc.verify, kyc.assess, compliance.screen, compliance.monitor
- ubo.collect-entity-data, ubo.get-ownership-structure, ubo.unroll-structure, ubo.resolve-ubos
- ubo.calculate-indirect-ownership, ubo.identify-control-prong, ubo.apply-thresholds
- ubo.identify-trust-parties, ubo.resolve-trust-ubos, ubo.identify-ownership-prong, ubo.resolve-partnership-ubos
- ubo.recursive-entity-resolve, ubo.identify-fincen-control-roles, ubo.apply-fincen-control-prong
- ubo.verify-identity, ubo.screen-person, ubo.assess-risk, ubo.monitor-changes, ubo.refresh-data
- resources.plan, resources.provision, resources.configure, resources.test, resources.deploy
- attributes.define, attributes.resolve, values.bind, values.validate, values.encrypt
- workflow.transition, workflow.gate, tasks.create, tasks.assign, tasks.complete
- notify.send, communicate.request, escalate.trigger, audit.log
- external.query, external.sync, api.call, webhook.register

NEW v3.1 DOCUMENT LIBRARY VERBS:
- document.catalog, document.verify, document.extract, document.link, document.use, document.amend, document.expire, document.query

NEW v3.1 ISDA DERIVATIVE VERBS:
- isda.establish_master, isda.establish_csa, isda.execute_trade, isda.margin_call, isda.post_collateral
- isda.value_portfolio, isda.declare_termination_event, isda.close_out, isda.amend_agreement
- isda.novate_trade, isda.dispute, isda.manage_netting_set

GRAPH CONSTRUCTION VERBS:
- entity, edge, define-kyc-investigation, ubo.calc, ubo.outcome, role.assign

DSL SYNTAX RULES:
- S-expressions format: (verb :key value :key value ...)
- Keywords use : prefix (e.g., :cbu-id, :nature-purpose)
- Strings in double quotes: "value"
- Arrays: ["item1" "item2"]
- Maps: {:key "value" :key2 "value2"}
- Comments: ;; comment text

RESPONSE FORMAT - Respond ONLY with valid JSON:
{
  "dsl_content": "Complete DSL as a string",
  "explanation": "Clear explanation of what was generated/changed",
  "confidence": 0.95,
  "changes": ["List of specific changes made"],
  "warnings": ["Any concerns or issues"],
  "suggestions": ["Recommendations for improvement"]
}"#;

        match request.response_type {
            super::AiResponseType::GenerateDsl => {
                format!(
                    "{}\n\nTASK: Generate new DSL based on the instruction.",
                    base_prompt
                )
            }
            super::AiResponseType::TransformDsl => {
                format!("{}\n\nTASK: Transform the existing DSL according to the instruction while preserving correct syntax and semantics.", base_prompt)
            }
            super::AiResponseType::ValidateDsl => {
                format!("{}\n\nTASK: Validate the DSL and provide feedback on correctness, compliance, and improvements.", base_prompt)
            }
            super::AiResponseType::ExplainDsl => {
                format!(
                    "{}\n\nTASK: Explain the DSL structure, meaning, and business logic.",
                    base_prompt
                )
            }
            super::AiResponseType::SuggestImprovements => {
                format!("{}\n\nTASK: Analyze the DSL and suggest improvements for better structure, compliance, or functionality.", base_prompt)
            }
        }
    }

    /// Build the user prompt for the specific request
    fn build_user_prompt(&self, request: &AiDslRequest) -> String {
        let mut prompt = format!("INSTRUCTION: {}\n", request.instruction);

        if let Some(current_dsl) = &request.current_dsl {
            prompt.push_str(&format!("\nCURRENT DSL:\n{}\n", current_dsl));
        }

        if !request.context.is_empty() {
            prompt.push_str("\nBUSINESS CONTEXT:\n");
            for (key, value) in &request.context {
                prompt.push_str(&format!("- {}: {}\n", key, value));
            }
        }

        if !request.constraints.is_empty() {
            prompt.push_str("\nCONSTRAINTS:\n");
            for constraint in &request.constraints {
                prompt.push_str(&format!("- {}\n", constraint));
            }
        }

        prompt.push_str(
            "\nRespond with valid JSON only. No markdown, no explanatory text outside the JSON.",
        );

        prompt
    }

    /// Send request to Gemini API
    async fn send_request(&self, system_prompt: &str, user_prompt: &str) -> AiResult<String> {
        let full_prompt = format!("{}\n\n{}", system_prompt, user_prompt);

        let request_body = GeminiRequest {
            contents: vec![GeminiContent {
                parts: vec![GeminiPart { text: full_prompt }],
            }],
            generation_config: Some(GeminiGenerationConfig {
                temperature: self.config.temperature,
                max_output_tokens: self.config.max_tokens,
                top_p: Some(0.8),
                top_k: Some(40),
            }),
        };

        let url = format!(
            "{}/{}:generateContent?key={}",
            self.base_url, self.config.model, self.config.api_key
        );

        debug!(
            "Sending request to Gemini API: {}",
            url.replace(&self.config.api_key, "***")
        );

        let response = self
            .client
            .post(&url)
            .json(&request_body)
            .send()
            .await
            .map_err(AiError::HttpError)?;

        let status = response.status();
        let response_text = response.text().await.map_err(AiError::HttpError)?;

        debug!("Gemini API response status: {}", status);

        if !status.is_success() {
            error!("Gemini API error: {} - {}", status, response_text);
            return Err(AiError::ApiError(format!(
                "HTTP {}: {}",
                status, response_text
            )));
        }

        let gemini_response: GeminiResponse =
            serde_json::from_str(&response_text).map_err(|e| {
                error!("Failed to parse Gemini response: {}", e);
                AiError::JsonError(e)
            })?;

        if gemini_response.candidates.is_empty() {
            return Err(AiError::InvalidResponse(
                "No candidates in response".to_string(),
            ));
        }

        let candidate = &gemini_response.candidates[0];
        if candidate.content.parts.is_empty() {
            return Err(AiError::InvalidResponse(
                "No parts in candidate".to_string(),
            ));
        }

        let response_text = &candidate.content.parts[0].text;

        if let Some(usage) = &gemini_response.usage_metadata {
            info!(
                "Gemini API usage - Prompt: {:?} tokens, Response: {:?} tokens, Total: {:?} tokens",
                usage.prompt_token_count, usage.candidates_token_count, usage.total_token_count
            );
        }

        Ok(response_text.clone())
    }

    /// Parse the AI response into structured format
    fn parse_response(&self, raw_response: &str) -> AiResult<AiDslResponse> {
        debug!("Parsing Gemini response: {}", raw_response.len());

        // Clean the response and try to extract JSON
        let cleaned = super::utils::clean_dsl_response(raw_response);
        let parsed = super::utils::parse_structured_response(&cleaned)?;

        Ok(AiDslResponse {
            dsl_content: parsed["dsl_content"].as_str().unwrap_or("").to_string(),
            explanation: parsed["explanation"]
                .as_str()
                .unwrap_or("AI generated DSL")
                .to_string(),
            confidence: parsed["confidence"].as_f64().unwrap_or(0.8),
            changes: parsed["changes"]
                .as_array()
                .map(|arr| {
                    arr.iter()
                        .filter_map(|v| v.as_str())
                        .map(|s| s.to_string())
                        .collect()
                })
                .unwrap_or_default(),
            warnings: parsed["warnings"]
                .as_array()
                .map(|arr| {
                    arr.iter()
                        .filter_map(|v| v.as_str())
                        .map(|s| s.to_string())
                        .collect()
                })
                .unwrap_or_default(),
            suggestions: parsed["suggestions"]
                .as_array()
                .map(|arr| {
                    arr.iter()
                        .filter_map(|v| v.as_str())
                        .map(|s| s.to_string())
                        .collect()
                })
                .unwrap_or_default(),
        })
    }
}

#[async_trait::async_trait]
impl AiService for GeminiClient {
    async fn request_dsl(&self, request: AiDslRequest) -> AiResult<AiDslResponse> {
        info!("Processing DSL request: {:?}", request.response_type);

        let system_prompt = self.build_system_prompt(&request);
        let user_prompt = self.build_user_prompt(&request);

        let raw_response = self.send_request(&system_prompt, &user_prompt).await?;
        let structured_response = self.parse_response(&raw_response)?;

        info!(
            "DSL request completed with confidence: {:.2}",
            structured_response.confidence
        );

        Ok(structured_response)
    }

    async fn health_check(&self) -> AiResult<bool> {
        debug!("Performing Gemini API health check");

        let test_request = AiDslRequest {
            instruction: "Generate a simple test DSL".to_string(),
            current_dsl: None,
            context: std::collections::HashMap::new(),
            response_type: super::AiResponseType::GenerateDsl,
            constraints: vec!["Keep it simple".to_string()],
        };

        match self.request_dsl(test_request).await {
            Ok(response) => {
                info!("Gemini API health check passed");
                Ok(!response.dsl_content.is_empty())
            }
            Err(e) => {
                warn!("Gemini API health check failed: {}", e);
                Ok(false)
            }
        }
    }

    fn config(&self) -> &AiConfig {
        &self.config
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::ai::AiResponseType;
    use std::collections::HashMap;

    fn create_test_config() -> AiConfig {
        AiConfig {
            api_key: "test-key".to_string(),
            model: "gemini-2.5-flash-preview-09-2025".to_string(),
            max_tokens: Some(1024),
            temperature: Some(0.1),
            timeout_seconds: 30,
        }
    }

    #[test]
    fn test_gemini_client_creation() {
        let config = create_test_config();
        let client = GeminiClient::new(config);
        assert!(client.is_ok());
    }

    #[test]
    fn test_gemini_client_empty_api_key() {
        let mut config = create_test_config();
        config.api_key = "".to_string();
        let client = GeminiClient::new(config);
        assert!(matches!(client.err(), Some(AiError::AuthenticationError)));
    }

    #[test]
    fn test_build_system_prompt() {
        let config = create_test_config();
        let client = GeminiClient::new(config).unwrap();

        let request = AiDslRequest {
            instruction: "Test".to_string(),
            current_dsl: None,
            context: HashMap::new(),
            response_type: AiResponseType::GenerateDsl,
            constraints: vec![],
        };

        let prompt = client.build_system_prompt(&request);
        assert!(prompt.contains("expert DSL"));
        assert!(prompt.contains("case.create"));
        assert!(prompt.contains("document.catalog"));
        assert!(prompt.contains("isda.establish_master"));
    }

    #[test]
    fn test_build_user_prompt() {
        let config = create_test_config();
        let client = GeminiClient::new(config).unwrap();

        let mut context = HashMap::new();
        context.insert("cbu_name".to_string(), "Test CBU".to_string());

        let request = AiDslRequest {
            instruction: "Create onboarding DSL".to_string(),
            current_dsl: Some("(case.create)".to_string()),
            context,
            response_type: AiResponseType::TransformDsl,
            constraints: vec!["Use proper syntax".to_string()],
        };

        let prompt = client.build_user_prompt(&request);
        assert!(prompt.contains("Create onboarding DSL"));
        assert!(prompt.contains("(case.create)"));
        assert!(prompt.contains("Test CBU"));
        assert!(prompt.contains("Use proper syntax"));
    }

    #[test]
    fn test_parse_response() {
        let config = create_test_config();
        let client = GeminiClient::new(config).unwrap();

        let json_response = r#"{
            "dsl_content": "(case.create :cbu-id \"TEST-001\")",
            "explanation": "Created a new case",
            "confidence": 0.95,
            "changes": ["Added case creation"],
            "warnings": [],
            "suggestions": ["Consider adding validation"]
        }"#;

        let parsed = client.parse_response(json_response).unwrap();
        assert!(parsed.dsl_content.contains("TEST-001"));
        assert_eq!(parsed.explanation, "Created a new case");
        assert_eq!(parsed.confidence, 0.95);
        assert_eq!(parsed.changes.len(), 1);
        assert_eq!(parsed.suggestions.len(), 1);
    }

    // Integration test - requires API key
    #[tokio::test]
    #[ignore = "Requires GEMINI_API_KEY environment variable"]
    async fn test_gemini_integration() {
        let config = AiConfig::default();
        if config.api_key.is_empty() {
            panic!("GEMINI_API_KEY environment variable required for integration test");
        }

        let client = GeminiClient::new(config).unwrap();

        let request = AiDslRequest {
            instruction: "Generate a simple onboarding DSL for a test company".to_string(),
            current_dsl: None,
            context: {
                let mut ctx = HashMap::new();
                ctx.insert("cbu_name".to_string(), "Test Company".to_string());
                ctx.insert("jurisdiction".to_string(), "US".to_string());
                ctx
            },
            response_type: AiResponseType::GenerateDsl,
            constraints: vec!["Use approved verbs only".to_string()],
        };

        let response = client.request_dsl(request).await;
        assert!(response.is_ok());

        let response = response.unwrap();
        assert!(!response.dsl_content.is_empty());
        assert!(response.confidence > 0.0);
        println!("Generated DSL: {}", response.dsl_content);
        println!("Explanation: {}", response.explanation);
        println!("Confidence: {}", response.confidence);
    }
}
