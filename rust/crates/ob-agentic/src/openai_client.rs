//! OpenAI Client
//!
//! LLM client implementation for OpenAI API.

use anyhow::{anyhow, Result};
use async_trait::async_trait;
use serde::Deserialize;

use super::llm_client::{LlmClient, ToolCallResult, ToolDefinition};

/// Default OpenAI model
const DEFAULT_MODEL: &str = "gpt-4o";

/// OpenAI API client
#[derive(Clone)]
pub struct OpenAiClient {
    api_key: String,
    client: reqwest::Client,
    model: String,
}

impl OpenAiClient {
    /// Create a new OpenAI client with the given API key
    pub fn new(api_key: String) -> Self {
        let model = std::env::var("OPENAI_MODEL").unwrap_or_else(|_| DEFAULT_MODEL.to_string());
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
        let api_key = std::env::var("OPENAI_API_KEY")
            .map_err(|_| anyhow!("OPENAI_API_KEY environment variable not set"))?;
        Ok(Self::new(api_key))
    }

    /// Internal API call implementation
    async fn call_api(
        &self,
        system_prompt: &str,
        user_prompt: &str,
        json_mode: bool,
    ) -> Result<String> {
        let mut body = serde_json::json!({
            "model": &self.model,
            "messages": [
                {"role": "system", "content": system_prompt},
                {"role": "user", "content": user_prompt}
            ],
            "temperature": 0.1
        });

        if json_mode {
            body["response_format"] = serde_json::json!({"type": "json_object"});
        }

        let response = self
            .client
            .post("https://api.openai.com/v1/chat/completions")
            .header("Authorization", format!("Bearer {}", self.api_key))
            .header("Content-Type", "application/json")
            .json(&body)
            .send()
            .await?;

        if !response.status().is_success() {
            let status = response.status();
            let body = response.text().await.unwrap_or_default();
            return Err(anyhow!("OpenAI API error {}: {}", status, body));
        }

        #[derive(Deserialize)]
        struct Message {
            content: String,
        }
        #[derive(Deserialize)]
        struct Choice {
            message: Message,
        }
        #[derive(Deserialize)]
        struct ApiResponse {
            choices: Vec<Choice>,
        }

        let api_response: ApiResponse = response.json().await?;
        api_response
            .choices
            .first()
            .map(|c| c.message.content.clone())
            .ok_or_else(|| anyhow!("OpenAI returned no choices"))
    }

    /// Internal API call with function_calling for structured output
    async fn call_api_with_tool(
        &self,
        system_prompt: &str,
        user_prompt: &str,
        tool: &ToolDefinition,
    ) -> Result<ToolCallResult> {
        let response = self
            .client
            .post("https://api.openai.com/v1/chat/completions")
            .header("Authorization", format!("Bearer {}", self.api_key))
            .header("Content-Type", "application/json")
            .json(&serde_json::json!({
                "model": &self.model,
                "messages": [
                    {"role": "system", "content": system_prompt},
                    {"role": "user", "content": user_prompt}
                ],
                "temperature": 0.1,
                "functions": [{
                    "name": &tool.name,
                    "description": &tool.description,
                    "parameters": &tool.parameters
                }],
                "function_call": {"name": &tool.name}
            }))
            .send()
            .await?;

        if !response.status().is_success() {
            let status = response.status();
            let body = response.text().await.unwrap_or_default();
            return Err(anyhow!("OpenAI API error {}: {}", status, body));
        }

        // Parse function_call response
        // Response format: { "choices": [{ "message": { "function_call": { "name": "...", "arguments": "..." } } }] }
        #[derive(Deserialize)]
        struct FunctionCall {
            name: String,
            arguments: String, // OpenAI returns arguments as a JSON string
        }
        #[derive(Deserialize)]
        struct Message {
            function_call: Option<FunctionCall>,
        }
        #[derive(Deserialize)]
        struct Choice {
            message: Message,
        }
        #[derive(Deserialize)]
        struct ApiResponse {
            choices: Vec<Choice>,
        }

        let response_text = response.text().await?;
        tracing::debug!(
            "OpenAI raw response: {}",
            &response_text[..response_text.len().min(1000)]
        );

        let api_response: ApiResponse = serde_json::from_str(&response_text)
            .map_err(|e| anyhow!("Failed to parse OpenAI response: {}", e))?;

        let function_call = api_response
            .choices
            .first()
            .and_then(|c| c.message.function_call.as_ref())
            .ok_or_else(|| anyhow!("No function_call in OpenAI response"))?;

        tracing::debug!(
            "OpenAI function_call arguments: {}",
            &function_call.arguments
        );

        // Parse the arguments JSON string into a Value
        let arguments: serde_json::Value = serde_json::from_str(&function_call.arguments)
            .map_err(|e| anyhow!("Failed to parse function arguments: {}", e))?;

        tracing::info!("OpenAI returned intents: {:?}", arguments.get("intents"));

        Ok(ToolCallResult {
            tool_name: function_call.name.clone(),
            arguments,
        })
    }
}

#[async_trait]
impl LlmClient for OpenAiClient {
    async fn chat(&self, system_prompt: &str, user_prompt: &str) -> Result<String> {
        self.call_api(system_prompt, user_prompt, false).await
    }

    async fn chat_json(&self, system_prompt: &str, user_prompt: &str) -> Result<String> {
        self.call_api(system_prompt, user_prompt, true).await
    }

    async fn chat_with_tool(
        &self,
        system_prompt: &str,
        user_prompt: &str,
        tool: &ToolDefinition,
    ) -> Result<ToolCallResult> {
        self.call_api_with_tool(system_prompt, user_prompt, tool)
            .await
    }

    fn model_name(&self) -> &str {
        &self.model
    }

    fn provider_name(&self) -> &str {
        "OpenAI"
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_new_client() {
        let client = OpenAiClient::new("test-key".to_string());
        assert_eq!(client.model_name(), DEFAULT_MODEL);
        assert_eq!(client.provider_name(), "OpenAI");
    }

    #[test]
    fn test_with_model() {
        let client = OpenAiClient::with_model("test-key".to_string(), "gpt-4o");
        assert_eq!(client.model_name(), "gpt-4o");
    }
}
