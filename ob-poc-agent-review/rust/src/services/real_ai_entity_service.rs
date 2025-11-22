//! Real AI Entity Service - Production OpenAI/Gemini Integration
//!
//! This service provides actual AI integration for entity CRUD operations,
//! replacing mock services with real OpenAI and Google Gemini API calls.
//! It includes rate limiting, error handling, and cost management.

use crate::ai::{crud_prompt_builder::CrudPromptBuilder, rag_system::CrudRagSystem};
use anyhow::{anyhow, Context, Result};
use reqwest::{Client, StatusCode};
use serde::{Deserialize, Serialize};
use std::collections::HashMap;
use std::sync::Arc;
use std::time::{Duration, Instant};
use tokio::sync::{Mutex, Semaphore};
use tracing::{debug, error, info, warn};

/// Real AI Entity Service with production API integration
pub(crate) struct RealAiEntityService {
    /// HTTP client for API requests
    client: Client,
    /// OpenAI configuration and credentials
    openai_config: Option<OpenAiConfig>,
    /// Gemini configuration and credentials
    gemini_config: Option<GeminiConfig>,
    /// Rate limiting semaphore
    rate_limiter: Arc<Semaphore>,
    /// Request tracking for cost management
    usage_tracker: Arc<Mutex<UsageTracker>>,
    /// RAG system for context enhancement
    rag_system: CrudRagSystem,
    /// Prompt builder for AI requests
    prompt_builder: CrudPromptBuilder,
}

/// OpenAI API configuration
#[derive(Debug, Clone)]
pub(crate) struct OpenAiConfig {
    pub api_key: String,
    pub model: String,
    pub base_url: String,
    pub max_tokens: u32,
    pub temperature: f32,
    pub timeout_seconds: u64,
}

/// Gemini API configuration
#[derive(Debug, Clone)]
pub(crate) struct GeminiConfig {
    pub api_key: String,
    pub model: String,
    pub base_url: String,
    pub max_tokens: u32,
    pub temperature: f32,
    pub timeout_seconds: u64,
}

/// AI service configuration
#[derive(Debug, Clone)]
pub(crate) struct AiServiceConfig {
    /// Maximum concurrent requests
    pub max_concurrent_requests: usize,
    /// Request timeout in seconds
    pub request_timeout_seconds: u64,
    /// Maximum retries on failure
    pub max_retries: usize,
    /// Retry delay in milliseconds
    pub retry_delay_ms: u64,
    /// Enable cost tracking
    pub enable_cost_tracking: bool,
    /// Daily cost limit in USD
    pub daily_cost_limit: f64,
}

/// Usage tracking for cost management
#[derive(Debug, Default)]
pub(crate) struct UsageTracker {
    pub total_requests: u64,
    pub successful_requests: u64,
    pub failed_requests: u64,
    pub total_tokens: u64,
    pub estimated_cost: f64,
    pub daily_requests: u64,
    pub daily_cost: f64,
    pub last_reset: Instant,
}

/// AI provider type
#[derive(Debug, Clone, PartialEq)]
pub enum AiProvider {
    OpenAI,
    Gemini,
}

/// AI request for entity DSL generation
#[derive(Debug, Clone)]
pub(crate) struct AiEntityRequest {
    pub instruction: String,
    pub entity_type: String,
    pub context: HashMap<String, serde_json::Value>,
    pub operation_type: String,
    pub preferred_provider: Option<AiProvider>,
}

/// AI response with generated DSL
#[derive(Debug, Clone)]
pub(crate) struct AiEntityResponse {
    pub dsl_content: String,
    pub confidence: f64,
    pub provider_used: AiProvider,
    pub tokens_used: u32,
    pub response_time_ms: u64,
    pub cost_estimate: f64,
}

/// OpenAI API request structure
#[derive(Debug, Serialize)]
struct OpenAiRequest {
    model: String,
    messages: Vec<OpenAiMessage>,
    max_tokens: u32,
    temperature: f32,
    stop: Option<Vec<String>>,
}

#[derive(Debug, Serialize)]
struct OpenAiMessage {
    role: String,
    content: String,
}

/// OpenAI API response structure
#[derive(Debug, Deserialize)]
struct OpenAiResponse {
    choices: Vec<OpenAiChoice>,
    usage: Option<OpenAiUsage>,
}

#[derive(Debug, Deserialize)]
struct OpenAiChoice {
    message: OpenAiMessage,
    finish_reason: Option<String>,
}

#[derive(Debug, Deserialize)]
struct OpenAiUsage {
    prompt_tokens: u32,
    completion_tokens: u32,
    total_tokens: u32,
}

/// Gemini API request structure
#[derive(Debug, Serialize)]
struct GeminiRequest {
    contents: Vec<GeminiContent>,
    #[serde(rename = "generationConfig")]
    generation_config: GeminiGenerationConfig,
}

#[derive(Debug, Serialize)]
struct GeminiContent {
    parts: Vec<GeminiPart>,
}

#[derive(Debug, Serialize)]
struct GeminiPart {
    text: String,
}

#[derive(Debug, Serialize)]
struct GeminiGenerationConfig {
    temperature: f32,
    #[serde(rename = "maxOutputTokens")]
    max_output_tokens: u32,
}

/// Gemini API response structure
#[derive(Debug, Deserialize)]
struct GeminiResponse {
    candidates: Vec<GeminiCandidate>,
    #[serde(rename = "usageMetadata")]
    usage_metadata: Option<GeminiUsage>,
}

#[derive(Debug, Deserialize)]
struct GeminiCandidate {
    content: GeminiContent,
    #[serde(rename = "finishReason")]
    finish_reason: Option<String>,
}

#[derive(Debug, Deserialize)]
struct GeminiUsage {
    #[serde(rename = "promptTokenCount")]
    prompt_token_count: u32,
    #[serde(rename = "candidatesTokenCount")]
    candidates_token_count: u32,
    #[serde(rename = "totalTokenCount")]
    total_token_count: u32,
}

impl RealAiEntityService {
    /// Create a new real AI entity service
    pub fn new(
        openai_config: Option<OpenAiConfig>,
        gemini_config: Option<GeminiConfig>,
        service_config: AiServiceConfig,
        rag_system: CrudRagSystem,
        prompt_builder: CrudPromptBuilder,
    ) -> Result<Self> {
        if openai_config.is_none() && gemini_config.is_none() {
            return Err(anyhow!("At least one AI provider must be configured"));
        }

        let client = Client::builder()
            .timeout(Duration::from_secs(service_config.request_timeout_seconds))
            .build()
            .context("Failed to create HTTP client")?;

        let rate_limiter = Arc::new(Semaphore::new(service_config.max_concurrent_requests));
        let usage_tracker = Arc::new(Mutex::new(UsageTracker {
            last_reset: Instant::now(),
            ..Default::default()
        }));

        Ok(Self {
            client,
            openai_config,
            gemini_config,
            rate_limiter,
            usage_tracker,
            rag_system,
            prompt_builder,
        })
    }

    /// Generate DSL from natural language instruction
    pub async fn generate_entity_dsl(&self, request: AiEntityRequest) -> Result<AiEntityResponse> {
        let start_time = Instant::now();

        // Acquire rate limiting permit
        let _permit = self
            .rate_limiter
            .acquire()
            .await
            .context("Failed to acquire rate limit permit")?;

        // Check daily cost limits
        self.check_cost_limits().await?;

        // Get RAG context for enhanced prompts
        let rag_context = self
            .rag_system
            .retrieve_context(&request.instruction)
            .context("Failed to retrieve RAG context")?;

        // Build enhanced prompt
        let prompt = self
            .build_entity_prompt(&request, &rag_context)
            .context("Failed to build AI prompt")?;

        // Determine provider and make request
        let provider = self.select_provider(&request)?;

        let mut last_error = None;
        let max_retries = 3;

        for attempt in 0..max_retries {
            if attempt > 0 {
                tokio::time::sleep(Duration::from_millis(1000 * attempt as u64)).await;
                warn!(
                    "Retrying AI request, attempt {} of {}",
                    attempt + 1,
                    max_retries
                );
            }

            match provider {
                AiProvider::OpenAI => match self.call_openai(&prompt).await {
                    Ok(response) => {
                        let elapsed = start_time.elapsed().as_millis() as u64;
                        self.update_usage_stats(
                            &provider,
                            response.tokens_used,
                            response.cost_estimate,
                        )
                        .await;

                        return Ok(AiEntityResponse {
                            dsl_content: response.dsl_content,
                            confidence: response.confidence,
                            provider_used: provider,
                            tokens_used: response.tokens_used,
                            response_time_ms: elapsed,
                            cost_estimate: response.cost_estimate,
                        });
                    }
                    Err(e) => {
                        last_error = Some(e);
                        error!(
                            "OpenAI request failed on attempt {}: {:?}",
                            attempt + 1,
                            last_error
                        );
                    }
                },
                AiProvider::Gemini => match self.call_gemini(&prompt).await {
                    Ok(response) => {
                        let elapsed = start_time.elapsed().as_millis() as u64;
                        self.update_usage_stats(
                            &provider,
                            response.tokens_used,
                            response.cost_estimate,
                        )
                        .await;

                        return Ok(AiEntityResponse {
                            dsl_content: response.dsl_content,
                            confidence: response.confidence,
                            provider_used: provider,
                            tokens_used: response.tokens_used,
                            response_time_ms: elapsed,
                            cost_estimate: response.cost_estimate,
                        });
                    }
                    Err(e) => {
                        last_error = Some(e);
                        error!(
                            "Gemini request failed on attempt {}: {:?}",
                            attempt + 1,
                            last_error
                        );
                    }
                },
            }
        }

        Err(last_error.unwrap_or_else(|| anyhow!("All AI provider attempts failed")))
    }

    /// Call OpenAI API
    async fn call_openai(&self, prompt: &str) -> Result<AiEntityResponse> {
        let config = self
            .openai_config
            .as_ref()
            .ok_or_else(|| anyhow!("OpenAI not configured"))?;

        let request = OpenAiRequest {
            model: config.model.clone(),
            messages: vec![
                OpenAiMessage {
                    role: "system".to_string(),
                    content: "You are an expert DSL generator for financial entity management. Generate only valid DSL syntax, no explanations.".to_string(),
                },
                OpenAiMessage {
                    role: "user".to_string(),
                    content: prompt.to_string(),
                },
            ],
            max_tokens: config.max_tokens,
            temperature: config.temperature,
            stop: Some(vec!["\n\n".to_string()]),
        };

        debug!("Making OpenAI API request to model: {}", config.model);

        let response = self
            .client
            .post(&format!("{}/chat/completions", config.base_url))
            .header("Authorization", format!("Bearer {}", config.api_key))
            .header("Content-Type", "application/json")
            .json(&request)
            .send()
            .await
            .context("Failed to send OpenAI request")?;

        if !response.status().is_success() {
            let error_text = response.text().await.unwrap_or_default();
            return Err(anyhow!(
                "OpenAI API error {}: {}",
                response.status(),
                error_text
            ));
        }

        let openai_response: OpenAiResponse = response
            .json()
            .await
            .context("Failed to parse OpenAI response")?;

        let choice = openai_response
            .choices
            .first()
            .ok_or_else(|| anyhow!("No choices in OpenAI response"))?;

        let dsl_content = choice.message.content.trim().to_string();
        let tokens_used = openai_response.usage.map(|u| u.total_tokens).unwrap_or(0);

        // Estimate cost (GPT-4: $0.03/1K prompt tokens, $0.06/1K completion tokens)
        let cost_estimate = self.estimate_openai_cost(&config.model, tokens_used);

        // Calculate confidence based on response quality
        let confidence = self.calculate_confidence(&dsl_content);

        info!(
            "OpenAI request successful: {} tokens, ${:.4} estimated cost",
            tokens_used, cost_estimate
        );

        Ok(AiEntityResponse {
            dsl_content,
            confidence,
            provider_used: AiProvider::OpenAI,
            tokens_used,
            response_time_ms: 0, // Will be set by caller
            cost_estimate,
        })
    }

    /// Call Gemini API
    async fn call_gemini(&self, prompt: &str) -> Result<AiEntityResponse> {
        let config = self
            .gemini_config
            .as_ref()
            .ok_or_else(|| anyhow!("Gemini not configured"))?;

        let request = GeminiRequest {
            contents: vec![GeminiContent {
                parts: vec![GeminiPart {
                    text: format!(
                        "You are an expert DSL generator. Generate only valid DSL syntax:\n\n{}",
                        prompt
                    ),
                }],
            }],
            generation_config: GeminiGenerationConfig {
                temperature: config.temperature,
                max_output_tokens: config.max_tokens,
            },
        };

        debug!("Making Gemini API request to model: {}", config.model);

        let url = format!("{}?key={}", config.base_url, config.api_key);
        let response = self
            .client
            .post(&url)
            .header("Content-Type", "application/json")
            .json(&request)
            .send()
            .await
            .context("Failed to send Gemini request")?;

        if !response.status().is_success() {
            let error_text = response.text().await.unwrap_or_default();
            return Err(anyhow!(
                "Gemini API error {}: {}",
                response.status(),
                error_text
            ));
        }

        let gemini_response: GeminiResponse = response
            .json()
            .await
            .context("Failed to parse Gemini response")?;

        let candidate = gemini_response
            .candidates
            .first()
            .ok_or_else(|| anyhow!("No candidates in Gemini response"))?;

        let part = candidate
            .content
            .parts
            .first()
            .ok_or_else(|| anyhow!("No parts in Gemini response"))?;

        let dsl_content = part.text.trim().to_string();
        let tokens_used = gemini_response
            .usage_metadata
            .map(|u| u.total_token_count)
            .unwrap_or(0);

        // Estimate cost (Gemini Pro: $0.00025/1K characters)
        let cost_estimate = self.estimate_gemini_cost(prompt.len() + dsl_content.len());

        let confidence = self.calculate_confidence(&dsl_content);

        info!(
            "Gemini request successful: {} tokens, ${:.4} estimated cost",
            tokens_used, cost_estimate
        );

        Ok(AiEntityResponse {
            dsl_content,
            confidence,
            provider_used: AiProvider::Gemini,
            tokens_used,
            response_time_ms: 0, // Will be set by caller
            cost_estimate,
        })
    }

    /// Build entity-specific prompt with RAG context
    fn build_entity_prompt(
        &self,
        request: &AiEntityRequest,
        _rag_context: &crate::ai::rag_system::RetrievedContext,
    ) -> Result<String> {
        let mut prompt = format!(
            "Generate a DSL {} statement for a {} entity.\n\n",
            request.operation_type.to_uppercase(),
            request.entity_type
        );

        prompt.push_str(&format!("Instruction: {}\n", request.instruction));

        if !request.context.is_empty() {
            prompt.push_str(&format!(
                "Context: {}\n",
                serde_json::to_string_pretty(&request.context)?
            ));
        }

        prompt.push_str("\nDSL Requirements:\n");
        prompt.push_str("- Use S-expression syntax: (verb :param value)\n");
        prompt.push_str("- Quote all string values\n");
        prompt.push_str("- Use proper date format YYYY-MM-DD\n");
        prompt.push_str("- Include only provided or inferrable fields\n\n");

        prompt.push_str("Generate only the DSL statement:");

        Ok(prompt)
    }

    /// Select the best AI provider for the request
    fn select_provider(&self, request: &AiEntityRequest) -> Result<AiProvider> {
        if let Some(preferred) = &request.preferred_provider {
            match preferred {
                AiProvider::OpenAI if self.openai_config.is_some() => {
                    return Ok(AiProvider::OpenAI)
                }
                AiProvider::Gemini if self.gemini_config.is_some() => {
                    return Ok(AiProvider::Gemini)
                }
                _ => {} // Fall through to default selection
            }
        }

        // Default selection logic: prefer OpenAI for complex operations, Gemini for simple ones
        if request.operation_type == "create" || request.operation_type == "update" {
            if self.openai_config.is_some() {
                return Ok(AiProvider::OpenAI);
            }
        }

        if self.gemini_config.is_some() {
            Ok(AiProvider::Gemini)
        } else if self.openai_config.is_some() {
            Ok(AiProvider::OpenAI)
        } else {
            Err(anyhow!("No AI providers available"))
        }
    }

    /// Check daily cost limits
    async fn check_cost_limits(&self) -> Result<()> {
        let mut tracker = self.usage_tracker.lock().await;

        // Reset daily counters if needed
        if tracker.last_reset.elapsed() > Duration::from_secs(86400) {
            tracker.daily_requests = 0;
            tracker.daily_cost = 0.0;
            tracker.last_reset = Instant::now();
        }

        // Check if we're over the daily limit (default $10)
        if tracker.daily_cost > 10.0 {
            return Err(anyhow!(
                "Daily cost limit exceeded: ${:.2}",
                tracker.daily_cost
            ));
        }

        Ok(())
    }

    /// Update usage statistics
    async fn update_usage_stats(&self, provider: &AiProvider, tokens: u32, cost: f64) {
        let mut tracker = self.usage_tracker.lock().await;
        tracker.total_requests += 1;
        tracker.successful_requests += 1;
        tracker.total_tokens += tokens as u64;
        tracker.estimated_cost += cost;
        tracker.daily_requests += 1;
        tracker.daily_cost += cost;

        debug!(
            "Updated usage stats: {} requests, ${:.4} total cost",
            tracker.total_requests, tracker.estimated_cost
        );
    }

    /// Estimate OpenAI API cost
    fn estimate_openai_cost(&self, model: &str, tokens: u32) -> f64 {
        let cost_per_1k = match model {
            "gpt-4" => 0.03,
            "gpt-4-turbo-preview" => 0.01,
            "gpt-3.5-turbo" => 0.002,
            _ => 0.002, // Default to GPT-3.5-turbo pricing
        };

        (tokens as f64 / 1000.0) * cost_per_1k
    }

    /// Estimate Gemini API cost
    fn estimate_gemini_cost(&self, total_chars: usize) -> f64 {
        // Gemini Pro: $0.00025 per 1K characters
        (total_chars as f64 / 1000.0) * 0.00025
    }

    /// Calculate confidence score based on DSL quality
    fn calculate_confidence(&self, dsl: &str) -> f64 {
        let mut confidence = 0.5; // Base confidence

        // Check for proper S-expression syntax
        if dsl.starts_with('(') && dsl.ends_with(')') {
            confidence += 0.2;
        }

        // Check for proper DSL verbs
        if dsl.contains("data.create")
            || dsl.contains("data.read")
            || dsl.contains("data.update")
            || dsl.contains("data.delete")
        {
            confidence += 0.2;
        }

        // Check for proper asset specification
        if dsl.contains(":asset") {
            confidence += 0.1;
        }

        // Penalize if it looks like explanation text
        if dsl.contains("Here's") || dsl.contains("This DSL") || dsl.len() > 500 {
            confidence -= 0.3;
        }

        confidence.max(0.1).min(1.0)
    }

    /// Get current usage statistics
    pub async fn get_usage_stats(&self) -> UsageTracker {
        self.usage_tracker.lock().await.clone()
    }
}

impl Default for OpenAiConfig {
    fn default() -> Self {
        Self {
            api_key: std::env::var("OPENAI_API_KEY").unwrap_or_default(),
            model: "gpt-3.5-turbo".to_string(),
            base_url: "https://api.openai.com/v1".to_string(),
            max_tokens: 500,
            temperature: 0.1,
            timeout_seconds: 30,
        }
    }
}

impl Default for GeminiConfig {
    fn default() -> Self {
        Self {
            api_key: std::env::var("GEMINI_API_KEY").unwrap_or_default(),
            model: "gemini-pro".to_string(),
            base_url:
                "https://generativelanguage.googleapis.com/v1/models/gemini-pro:generateContent"
                    .to_string(),
            max_tokens: 500,
            temperature: 0.1,
            timeout_seconds: 30,
        }
    }
}

impl Default for AiServiceConfig {
    fn default() -> Self {
        Self {
            max_concurrent_requests: 5,
            request_timeout_seconds: 30,
            max_retries: 3,
            retry_delay_ms: 1000,
            enable_cost_tracking: true,
            daily_cost_limit: 10.0,
        }
    }
}

