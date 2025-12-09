//! Anthropic Client
//!
//! LLM client implementation for Anthropic Claude API.

use anyhow::{anyhow, Result};
use async_trait::async_trait;
use serde::Deserialize;

use super::llm_client::LlmClient;

/// Default Anthropic model
const DEFAULT_MODEL: &str = "claude-sonnet-4-20250514";

/// Anthropic Claude API client
#[derive(Clone)]
pub struct AnthropicClient {
    api_key: String,
    client: reqwest::Client,
    model: String,
}

impl AnthropicClient {
    /// Create a new Anthropic client with the given API key
    pub fn new(api_key: String) -> Self {
        let model = std::env::var("ANTHROPIC_MODEL").unwrap_or_else(|_| DEFAULT_MODEL.to_string());
        Self {
            api_key,
            client: reqwest::Client::new(),
            model,
        }
    }

    /// Create with a specific model
    pub fn with_model(api_key: String, model: &str) -> Self {
        Self {
            api_key,
            client: reqwest::Client::new(),
            model: model.to_string(),
        }
    }

    /// Create from environment variables
    pub fn from_env() -> Result<Self> {
        let api_key = std::env::var("ANTHROPIC_API_KEY")
            .map_err(|_| anyhow!("ANTHROPIC_API_KEY environment variable not set"))?;
        Ok(Self::new(api_key))
    }

    /// Internal API call implementation
    async fn call_api(&self, system_prompt: &str, user_prompt: &str) -> Result<String> {
        let response = self
            .client
            .post("https://api.anthropic.com/v1/messages")
            .header("x-api-key", &self.api_key)
            .header("anthropic-version", "2023-06-01")
            .header("content-type", "application/json")
            .json(&serde_json::json!({
                "model": &self.model,
                "max_tokens": 4096,
                "system": system_prompt,
                "messages": [{"role": "user", "content": user_prompt}]
            }))
            .send()
            .await?;

        if !response.status().is_success() {
            let status = response.status();
            let body = response.text().await.unwrap_or_default();
            return Err(anyhow!("Anthropic API error {}: {}", status, body));
        }

        #[derive(Deserialize)]
        struct ContentBlock {
            text: Option<String>,
        }
        #[derive(Deserialize)]
        struct ApiResponse {
            content: Vec<ContentBlock>,
        }

        let api_response: ApiResponse = response.json().await?;
        api_response
            .content
            .first()
            .and_then(|c| c.text.clone())
            .ok_or_else(|| anyhow!("Empty response from Anthropic"))
    }
}

#[async_trait]
impl LlmClient for AnthropicClient {
    async fn chat(&self, system_prompt: &str, user_prompt: &str) -> Result<String> {
        self.call_api(system_prompt, user_prompt).await
    }

    async fn chat_json(&self, system_prompt: &str, user_prompt: &str) -> Result<String> {
        // Anthropic doesn't have json_object mode, rely on prompt engineering
        let json_system = format!(
            "{}\n\nIMPORTANT: Respond with valid JSON only. No markdown code blocks, no explanations.",
            system_prompt
        );
        self.call_api(&json_system, user_prompt).await
    }

    fn model_name(&self) -> &str {
        &self.model
    }

    fn provider_name(&self) -> &str {
        "Anthropic"
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_new_client() {
        let client = AnthropicClient::new("test-key".to_string());
        assert_eq!(client.model_name(), DEFAULT_MODEL);
        assert_eq!(client.provider_name(), "Anthropic");
    }

    #[test]
    fn test_with_model() {
        let client = AnthropicClient::with_model("test-key".to_string(), "claude-3-opus");
        assert_eq!(client.model_name(), "claude-3-opus");
    }
}
