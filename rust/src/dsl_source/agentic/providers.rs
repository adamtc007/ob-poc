//! Multi-provider LLM support for DSL generation
//!
//! Supports Anthropic (Claude), OpenAI, and Google Gemini APIs
//! with rate limiting, cost tracking, and automatic failover.

use anyhow::{anyhow, Context, Result};
use reqwest::Client;
use serde::{Deserialize, Serialize};
use std::sync::Arc;
use std::time::Duration;
use tokio::sync::Semaphore;
use tracing::{debug, info, warn};

/// Supported LLM providers
#[derive(Debug, Clone, PartialEq)]
pub enum LlmProvider {
    Anthropic,
    OpenAI,
    Gemini,
}

/// Configuration for LLM provider
#[derive(Debug, Clone)]
pub struct ProviderConfig {
    pub provider: LlmProvider,
    pub api_key: String,
    pub model: String,
    pub base_url: String,
    pub max_tokens: u32,
    pub temperature: f32,
    pub timeout_seconds: u64,
}

/// Response from LLM provider
#[derive(Debug, Clone)]
pub struct LlmResponse {
    pub content: String,
    pub tokens_used: u32,
    pub cost_estimate: f64,
    pub provider: LlmProvider,
}

/// Multi-provider LLM client
pub struct MultiProviderLlm {
    client: Client,
    providers: Vec<ProviderConfig>,
    rate_limiter: Arc<Semaphore>,
}

impl MultiProviderLlm {
    /// Create a new multi-provider LLM client
    pub fn new(providers: Vec<ProviderConfig>, max_concurrent: usize) -> Result<Self> {
        if providers.is_empty() {
            return Err(anyhow!("At least one provider must be configured"));
        }

        let client = Client::builder()
            .timeout(Duration::from_secs(60))
            .build()
            .context("Failed to create HTTP client")?;

        Ok(Self {
            client,
            providers,
            rate_limiter: Arc::new(Semaphore::new(max_concurrent)),
        })
    }

    /// Create from environment variables
    pub fn from_env() -> Result<Self> {
        let mut providers = Vec::new();

        // Check for Anthropic
        if let Ok(api_key) = std::env::var("ANTHROPIC_API_KEY") {
            providers.push(ProviderConfig {
                provider: LlmProvider::Anthropic,
                api_key,
                model: std::env::var("ANTHROPIC_MODEL")
                    .unwrap_or_else(|_| "claude-sonnet-4-20250514".to_string()),
                base_url: "https://api.anthropic.com/v1".to_string(),
                max_tokens: 1024,
                temperature: 0.1,
                timeout_seconds: 30,
            });
        }

        // Check for OpenAI
        if let Ok(api_key) = std::env::var("OPENAI_API_KEY") {
            providers.push(ProviderConfig {
                provider: LlmProvider::OpenAI,
                api_key,
                model: std::env::var("OPENAI_MODEL")
                    .unwrap_or_else(|_| "gpt-4-turbo-preview".to_string()),
                base_url: "https://api.openai.com/v1".to_string(),
                max_tokens: 1024,
                temperature: 0.1,
                timeout_seconds: 30,
            });
        }

        // Check for Gemini
        if let Ok(api_key) = std::env::var("GEMINI_API_KEY") {
            providers.push(ProviderConfig {
                provider: LlmProvider::Gemini,
                api_key,
                model: std::env::var("GEMINI_MODEL").unwrap_or_else(|_| "gemini-pro".to_string()),
                base_url: "https://generativelanguage.googleapis.com/v1".to_string(),
                max_tokens: 1024,
                temperature: 0.1,
                timeout_seconds: 30,
            });
        }

        Self::new(providers, 5)
    }

    /// Generate completion with automatic failover
    pub async fn generate(&self, system_prompt: &str, user_prompt: &str) -> Result<LlmResponse> {
        let _permit = self
            .rate_limiter
            .acquire()
            .await
            .map_err(|_| anyhow!("Failed to acquire rate limit permit"))?;

        let mut last_error = None;

        for config in &self.providers {
            match self.call_provider(config, system_prompt, user_prompt).await {
                Ok(response) => {
                    info!("LLM request successful via {:?}", config.provider);
                    return Ok(response);
                }
                Err(e) => {
                    warn!("Provider {:?} failed: {}", config.provider, e);
                    last_error = Some(e);
                }
            }
        }

        Err(last_error.unwrap_or_else(|| anyhow!("All providers failed")))
    }

    /// Call a specific provider
    async fn call_provider(
        &self,
        config: &ProviderConfig,
        system_prompt: &str,
        user_prompt: &str,
    ) -> Result<LlmResponse> {
        match config.provider {
            LlmProvider::Anthropic => {
                self.call_anthropic(config, system_prompt, user_prompt)
                    .await
            }
            LlmProvider::OpenAI => self.call_openai(config, system_prompt, user_prompt).await,
            LlmProvider::Gemini => self.call_gemini(config, system_prompt, user_prompt).await,
        }
    }

    /// Call Anthropic API
    async fn call_anthropic(
        &self,
        config: &ProviderConfig,
        system_prompt: &str,
        user_prompt: &str,
    ) -> Result<LlmResponse> {
        #[derive(Serialize)]
        struct AnthropicRequest {
            model: String,
            max_tokens: u32,
            system: String,
            messages: Vec<AnthropicMessage>,
        }

        #[derive(Serialize)]
        struct AnthropicMessage {
            role: String,
            content: String,
        }

        #[derive(Deserialize)]
        struct AnthropicResponse {
            content: Vec<ContentBlock>,
            usage: AnthropicUsage,
        }

        #[derive(Deserialize)]
        struct ContentBlock {
            text: String,
        }

        #[derive(Deserialize)]
        struct AnthropicUsage {
            input_tokens: u32,
            output_tokens: u32,
        }

        let request = AnthropicRequest {
            model: config.model.clone(),
            max_tokens: config.max_tokens,
            system: system_prompt.to_string(),
            messages: vec![AnthropicMessage {
                role: "user".to_string(),
                content: user_prompt.to_string(),
            }],
        };

        debug!("Calling Anthropic API with model: {}", config.model);

        let response = self
            .client
            .post(format!("{}/messages", config.base_url))
            .header("x-api-key", &config.api_key)
            .header("anthropic-version", "2023-06-01")
            .header("Content-Type", "application/json")
            .json(&request)
            .send()
            .await
            .context("Failed to send Anthropic request")?;

        if !response.status().is_success() {
            let error_text = response.text().await.unwrap_or_default();
            return Err(anyhow!("Anthropic API error: {}", error_text));
        }

        let result: AnthropicResponse = response
            .json()
            .await
            .context("Failed to parse Anthropic response")?;

        let content = result
            .content
            .first()
            .map(|c| c.text.clone())
            .unwrap_or_default();

        let tokens = result.usage.input_tokens + result.usage.output_tokens;
        let cost = estimate_anthropic_cost(
            &config.model,
            result.usage.input_tokens,
            result.usage.output_tokens,
        );

        Ok(LlmResponse {
            content,
            tokens_used: tokens,
            cost_estimate: cost,
            provider: LlmProvider::Anthropic,
        })
    }

    /// Call OpenAI API
    async fn call_openai(
        &self,
        config: &ProviderConfig,
        system_prompt: &str,
        user_prompt: &str,
    ) -> Result<LlmResponse> {
        #[derive(Serialize)]
        struct OpenAiRequest {
            model: String,
            messages: Vec<OpenAiMessage>,
            max_tokens: u32,
            temperature: f32,
        }

        #[derive(Serialize, Deserialize)]
        struct OpenAiMessage {
            role: String,
            content: String,
        }

        #[derive(Deserialize)]
        struct OpenAiResponse {
            choices: Vec<OpenAiChoice>,
            usage: Option<OpenAiUsage>,
        }

        #[derive(Deserialize)]
        struct OpenAiChoice {
            message: OpenAiMessage,
        }

        #[derive(Deserialize)]
        struct OpenAiUsage {
            total_tokens: u32,
        }

        let request = OpenAiRequest {
            model: config.model.clone(),
            messages: vec![
                OpenAiMessage {
                    role: "system".to_string(),
                    content: system_prompt.to_string(),
                },
                OpenAiMessage {
                    role: "user".to_string(),
                    content: user_prompt.to_string(),
                },
            ],
            max_tokens: config.max_tokens,
            temperature: config.temperature,
        };

        debug!("Calling OpenAI API with model: {}", config.model);

        let response = self
            .client
            .post(format!("{}/chat/completions", config.base_url))
            .header("Authorization", format!("Bearer {}", config.api_key))
            .header("Content-Type", "application/json")
            .json(&request)
            .send()
            .await
            .context("Failed to send OpenAI request")?;

        if !response.status().is_success() {
            let error_text = response.text().await.unwrap_or_default();
            return Err(anyhow!("OpenAI API error: {}", error_text));
        }

        let result: OpenAiResponse = response
            .json()
            .await
            .context("Failed to parse OpenAI response")?;

        let content = result
            .choices
            .first()
            .map(|c| c.message.content.clone())
            .unwrap_or_default();

        let tokens = result.usage.map(|u| u.total_tokens).unwrap_or(0);
        let cost = estimate_openai_cost(&config.model, tokens);

        Ok(LlmResponse {
            content,
            tokens_used: tokens,
            cost_estimate: cost,
            provider: LlmProvider::OpenAI,
        })
    }

    /// Call Gemini API
    async fn call_gemini(
        &self,
        config: &ProviderConfig,
        system_prompt: &str,
        user_prompt: &str,
    ) -> Result<LlmResponse> {
        #[derive(Serialize)]
        struct GeminiRequest {
            contents: Vec<GeminiContent>,
            #[serde(rename = "generationConfig")]
            generation_config: GeminiConfig,
        }

        #[derive(Serialize, Deserialize)]
        struct GeminiContent {
            parts: Vec<GeminiPart>,
        }

        #[derive(Serialize, Deserialize)]
        struct GeminiPart {
            text: String,
        }

        #[derive(Serialize)]
        struct GeminiConfig {
            temperature: f32,
            #[serde(rename = "maxOutputTokens")]
            max_output_tokens: u32,
        }

        #[derive(Deserialize)]
        struct GeminiResponse {
            candidates: Vec<GeminiCandidate>,
            #[serde(rename = "usageMetadata")]
            usage_metadata: Option<GeminiUsage>,
        }

        #[derive(Deserialize)]
        struct GeminiCandidate {
            content: GeminiContent,
        }

        #[derive(Deserialize)]
        struct GeminiUsage {
            #[serde(rename = "totalTokenCount")]
            total_token_count: u32,
        }

        let combined_prompt = format!("{}\n\n{}", system_prompt, user_prompt);

        let request = GeminiRequest {
            contents: vec![GeminiContent {
                parts: vec![GeminiPart {
                    text: combined_prompt,
                }],
            }],
            generation_config: GeminiConfig {
                temperature: config.temperature,
                max_output_tokens: config.max_tokens,
            },
        };

        let url = format!(
            "{}/models/{}:generateContent?key={}",
            config.base_url, config.model, config.api_key
        );

        debug!("Calling Gemini API with model: {}", config.model);

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
            return Err(anyhow!("Gemini API error: {}", error_text));
        }

        let result: GeminiResponse = response
            .json()
            .await
            .context("Failed to parse Gemini response")?;

        let content = result
            .candidates
            .first()
            .and_then(|c| c.content.parts.first())
            .map(|p| p.text.clone())
            .unwrap_or_default();

        let tokens = result
            .usage_metadata
            .map(|u| u.total_token_count)
            .unwrap_or(0);
        let cost = estimate_gemini_cost(tokens);

        Ok(LlmResponse {
            content,
            tokens_used: tokens,
            cost_estimate: cost,
            provider: LlmProvider::Gemini,
        })
    }
}

fn estimate_anthropic_cost(model: &str, input_tokens: u32, output_tokens: u32) -> f64 {
    let (input_rate, output_rate) = match model {
        m if m.contains("opus") => (0.015, 0.075),
        m if m.contains("sonnet") => (0.003, 0.015),
        m if m.contains("haiku") => (0.00025, 0.00125),
        _ => (0.003, 0.015), // Default to Sonnet
    };
    (input_tokens as f64 / 1000.0) * input_rate + (output_tokens as f64 / 1000.0) * output_rate
}

fn estimate_openai_cost(model: &str, tokens: u32) -> f64 {
    let rate = match model {
        "gpt-4" | "gpt-4-turbo-preview" => 0.03,
        "gpt-3.5-turbo" => 0.002,
        _ => 0.01,
    };
    (tokens as f64 / 1000.0) * rate
}

fn estimate_gemini_cost(tokens: u32) -> f64 {
    // Gemini Pro pricing
    (tokens as f64 / 1000.0) * 0.00025
}
