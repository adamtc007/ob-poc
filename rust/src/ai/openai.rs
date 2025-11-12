//! OpenAI API Client Implementation
//!
//! This module provides a complete implementation of the OpenAI API client
//! for DSL generation, transformation, and validation tasks using GPT models.

use super::{AiConfig, AiDslRequest, AiDslResponse, AiError, AiResponseType, AiResult, AiService};
use reqwest::Client;
use serde::{Deserialize, Serialize};
use serde_json::json;
use std::time::Duration;
use tracing::{debug, error, info, warn};

/// OpenAI API client
#[derive(Debug, Clone)]
pub struct OpenAiClient {
    config: AiConfig,
    client: Client,
    base_url: String,
}

/// OpenAI API request format
#[derive(Debug, Serialize)]
struct OpenAiRequest {
    model: String,
    messages: Vec<OpenAiMessage>,
    #[serde(skip_serializing_if = "Option::is_none")]
    max_tokens: Option<u32>,
    #[serde(skip_serializing_if = "Option::is_none")]
    temperature: Option<f32>,
    #[serde(skip_serializing_if = "Option::is_none")]
    response_format: Option<OpenAiResponseFormat>,
}

/// OpenAI message structure
#[derive(Debug, Serialize)]
struct OpenAiMessage {
    role: String,
    content: String,
}

/// OpenAI response format
#[derive(Debug, Serialize)]
struct OpenAiResponseFormat {
    #[serde(rename = "type")]
    format_type: String,
}

/// OpenAI API response format
#[derive(Debug, Deserialize)]
struct OpenAiResponse {
    id: String,
    object: String,
    created: u64,
    model: String,
    choices: Vec<OpenAiChoice>,
    usage: Option<OpenAiUsage>,
}

/// OpenAI choice structure
#[derive(Debug, Deserialize)]
struct OpenAiChoice {
    index: u32,
    message: OpenAiResponseMessage,
    finish_reason: Option<String>,
}

/// OpenAI response message
#[derive(Debug, Deserialize)]
struct OpenAiResponseMessage {
    role: String,
    content: String,
}

/// OpenAI usage statistics
#[derive(Debug, Deserialize)]
struct OpenAiUsage {
    prompt_tokens: u32,
    completion_tokens: u32,
    total_tokens: u32,
}

/// OpenAI error response
#[derive(Debug, Deserialize)]
struct OpenAiErrorResponse {
    error: OpenAiErrorDetail,
}

/// OpenAI error detail
#[derive(Debug, Deserialize)]
struct OpenAiErrorDetail {
    message: String,
    #[serde(rename = "type")]
    error_type: String,
    param: Option<String>,
    code: Option<String>,
}

impl OpenAiClient {
    /// Create a new OpenAI client
    pub fn new(config: AiConfig) -> AiResult<Self> {
        if config.api_key.is_empty() {
            return Err(AiError::AuthenticationError);
        }

        let client = Client::builder()
            .timeout(Duration::from_secs(config.timeout_seconds))
            .build()
            .map_err(AiError::HttpError)?;

        Ok(Self {
            config,
            client,
            base_url: "https://api.openai.com/v1".to_string(),
        })
    }

    /// Build system prompt for DSL operations
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
            AiResponseType::GenerateDsl => {
                format!(
                    "{}\n\nTASK: Generate new DSL based on the instruction.",
                    base_prompt
                )
            }
            AiResponseType::TransformDsl => {
                format!("{}\n\nTASK: Transform the existing DSL according to the instruction while preserving correct syntax and semantics.", base_prompt)
            }
            AiResponseType::ValidateDsl => {
                format!("{}\n\nTASK: Validate the DSL and provide feedback on correctness, compliance, and improvements.", base_prompt)
            }
            AiResponseType::ExplainDsl => {
                format!(
                    "{}\n\nTASK: Explain the DSL structure, meaning, and business logic.",
                    base_prompt
                )
            }
            AiResponseType::SuggestImprovements => {
                format!("{}\n\nTASK: Analyze the DSL and suggest improvements for better structure, compliance, or functionality.", base_prompt)
            }
        }
    }

    /// Build user prompt from request
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

        prompt.push_str("\nRespond with valid JSON only.");
        prompt
    }

    /// Parse OpenAI response into structured DSL response (unified with Gemini)
    fn parse_response(&self, content: &str) -> AiResult<AiDslResponse> {
        debug!("Parsing OpenAI response: {} chars", content.len());

        // Use the same structured parsing as Gemini
        let cleaned = super::utils::clean_dsl_response(content);
        let parsed = super::utils::parse_structured_response(&cleaned)?;

        Ok(AiDslResponse {
            dsl_content: parsed["dsl_content"].as_str().unwrap_or("").to_string(),
            explanation: parsed["explanation"]
                .as_str()
                .unwrap_or("AI operation completed")
                .to_string(),
            confidence: parsed["confidence"].as_f64().unwrap_or(0.8),
            changes: parsed["changes"]
                .as_array()
                .unwrap_or(&Vec::new())
                .iter()
                .filter_map(|v| v.as_str().map(|s| s.to_string()))
                .collect(),
            warnings: parsed["warnings"]
                .as_array()
                .unwrap_or(&Vec::new())
                .iter()
                .filter_map(|v| v.as_str().map(|s| s.to_string()))
                .collect(),
            suggestions: parsed["suggestions"]
                .as_array()
                .unwrap_or(&Vec::new())
                .iter()
                .filter_map(|v| v.as_str().map(|s| s.to_string()))
                .collect(),
        })
    }
}

#[async_trait::async_trait]
impl AiService for OpenAiClient {
    async fn generate_dsl(&self, request: AiDslRequest) -> AiResult<AiDslResponse> {
        debug!("Sending DSL request to OpenAI: {:?}", request.response_type);

        let system_prompt = self.build_system_prompt(&request);
        let user_prompt = self.build_user_prompt(&request);

        let openai_request = OpenAiRequest {
            model: self.config.model.clone(),
            messages: vec![
                OpenAiMessage {
                    role: "system".to_string(),
                    content: system_prompt,
                },
                OpenAiMessage {
                    role: "user".to_string(),
                    content: user_prompt,
                },
            ],
            max_tokens: self.config.max_tokens,
            temperature: self.config.temperature,
            response_format: Some(OpenAiResponseFormat {
                format_type: "json_object".to_string(),
            }),
        };

        let response = self
            .client
            .post(format!("{}/chat/completions", self.base_url))
            .header("Authorization", &format!("Bearer {}", self.config.api_key))
            .header("Content-Type", "application/json")
            .json(&openai_request)
            .send()
            .await
            .map_err(AiError::HttpError)?;

        let status = response.status();
        let response_text = response.text().await.map_err(AiError::HttpError)?;

        if !status.is_success() {
            error!("OpenAI API error: {} - {}", status, response_text);

            // Try to parse error response
            if let Ok(error_response) = serde_json::from_str::<OpenAiErrorResponse>(&response_text)
            {
                return match error_response.error.error_type.as_str() {
                    "insufficient_quota" | "rate_limit_exceeded" => Err(AiError::RateLimitError),
                    "invalid_api_key" | "invalid_organization" => Err(AiError::AuthenticationError),
                    _ => Err(AiError::ApiError(error_response.error.message)),
                };
            } else {
                return Err(AiError::ApiError(format!(
                    "HTTP {} - {}",
                    status, response_text
                )));
            }
        }

        let openai_response: OpenAiResponse = serde_json::from_str(&response_text)
            .map_err(|e| AiError::InvalidResponse(format!("Failed to parse response: {}", e)))?;

        if openai_response.choices.is_empty() {
            return Err(AiError::InvalidResponse(
                "No choices in response".to_string(),
            ));
        }

        let content = &openai_response.choices[0].message.content;
        info!(
            "OpenAI response received, {} tokens used",
            openai_response
                .usage
                .as_ref()
                .map(|u| u.total_tokens)
                .unwrap_or(0)
        );

        self.parse_response(content)
    }

    async fn health_check(&self) -> AiResult<bool> {
        debug!("Performing OpenAI health check");

        let test_request = OpenAiRequest {
            model: self.config.model.clone(),
            messages: vec![OpenAiMessage {
                role: "user".to_string(),
                content:
                    "Hello, this is a health check. Please respond with JSON: {\"status\": \"OK\"}"
                        .to_string(),
            }],
            max_tokens: Some(50),
            temperature: Some(0.0),
            response_format: Some(OpenAiResponseFormat {
                format_type: "json_object".to_string(),
            }),
        };

        let response = self
            .client
            .post(format!("{}/chat/completions", self.base_url))
            .header("Authorization", &format!("Bearer {}", self.config.api_key))
            .header("Content-Type", "application/json")
            .json(&test_request)
            .send()
            .await
            .map_err(AiError::HttpError)?;

        Ok(response.status().is_success())
    }

    fn config(&self) -> &AiConfig {
        &self.config
    }
}

// Update AiConfig default to use OpenAI settings
impl AiConfig {
    /// Create configuration for OpenAI
    pub fn openai() -> Self {
        Self {
            api_key: std::env::var("OPENAI_API_KEY").unwrap_or_default(),
            model: "gpt-3.5-turbo".to_string(),
            max_tokens: Some(2048),
            temperature: Some(0.1),
            timeout_seconds: 30,
        }
    }

    /// Create configuration for GPT-4
    pub fn gpt4() -> Self {
        Self {
            api_key: std::env::var("OPENAI_API_KEY").unwrap_or_default(),
            model: "gpt-4".to_string(),
            max_tokens: Some(4096),
            temperature: Some(0.1),
            timeout_seconds: 60,
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::collections::HashMap;

    fn create_test_config() -> AiConfig {
        AiConfig {
            api_key: "test-key".to_string(),
            model: "gpt-3.5-turbo".to_string(),
            max_tokens: Some(1024),
            temperature: Some(0.1),
            timeout_seconds: 30,
        }
    }

    #[test]
    fn test_openai_config() {
        let config = AiConfig::openai();
        assert_eq!(config.model, "gpt-3.5-turbo");
        assert_eq!(config.max_tokens, Some(2048));
        assert_eq!(config.temperature, Some(0.1));
    }

    #[test]
    fn test_gpt4_config() {
        let config = AiConfig::gpt4();
        assert_eq!(config.model, "gpt-4");
        assert_eq!(config.max_tokens, Some(4096));
    }

    #[test]
    fn test_build_system_prompt() {
        let config = create_test_config();
        let client = OpenAiClient::new(config).unwrap();

        let request = AiDslRequest {
            instruction: "Test".to_string(),
            current_dsl: None,
            context: HashMap::new(),
            response_type: AiResponseType::GenerateDsl,
            constraints: vec![],
        };

        let prompt = client.build_system_prompt(&request);
        assert!(prompt.contains("Generate new DSL"));
        assert!(prompt.contains("S-expression"));
        assert!(prompt.contains("case.create"));
    }

    #[test]
    fn test_build_user_prompt() {
        let config = create_test_config();
        let client = OpenAiClient::new(config).unwrap();

        let mut context = HashMap::new();
        context.insert("entity_type".to_string(), "hedge_fund".to_string());

        let request = AiDslRequest {
            instruction: "Create onboarding DSL".to_string(),
            current_dsl: None,
            context,
            response_type: AiResponseType::GenerateDsl,
            constraints: vec!["Use approved verbs".to_string()],
        };

        let prompt = client.build_user_prompt(&request);
        assert!(prompt.contains("Create onboarding DSL"));
        assert!(prompt.contains("entity_type: hedge_fund"));
        assert!(prompt.contains("Use approved verbs"));
    }
}
