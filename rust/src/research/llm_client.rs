//! LLM client trait for research macro execution
//!
//! Defines the interface for LLM interactions with tool use support.

use async_trait::async_trait;
use serde::{Deserialize, Serialize};
use serde_json::Value;

use super::error::Result;

/// Tool definition for LLM
#[derive(Debug, Clone, Serialize)]
pub struct ToolDef {
    pub name: String,
    pub description: String,
    pub input_schema: Value,
}

/// Source from web search
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ResearchSource {
    pub url: String,
    pub title: Option<String>,
    pub snippet: Option<String>,
}

/// Result of LLM completion with potential tool use
#[derive(Debug, Clone)]
pub struct LlmResponse {
    /// The final text content from the LLM
    pub content: String,

    /// Sources discovered during web search tool use
    pub sources: Vec<ResearchSource>,

    /// Raw tool use interactions for debugging
    pub tool_calls: Vec<ToolCall>,
}

/// A single tool call and its result
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ToolCall {
    pub tool_name: String,
    pub input: Value,
    pub output: Value,
}

/// Trait for LLM client used by research executor
#[async_trait]
pub trait ResearchLlmClient: Send + Sync {
    /// Complete with tools enabled (e.g., web_search)
    ///
    /// This method should:
    /// 1. Send the initial request with tools available
    /// 2. Handle tool_use responses iteratively
    /// 3. Execute tool calls (like web_search)
    /// 4. Send tool results back to the LLM
    /// 5. Continue until LLM returns final text
    /// 6. Collect sources from web_search results
    async fn complete_with_tools(
        &self,
        system_prompt: &str,
        user_prompt: &str,
        tools: &[ToolDef],
    ) -> Result<LlmResponse>;

    /// Complete expecting JSON response (no tools)
    async fn complete_json(
        &self,
        system_prompt: &str,
        user_prompt: &str,
        schema: &Value,
    ) -> Result<Value>;

    /// Get the model name for logging
    fn model_name(&self) -> &str;
}

/// Web search tool definition
pub fn web_search_tool() -> ToolDef {
    ToolDef {
        name: "web_search".to_string(),
        description: "Search the web for current information about companies, regulations, \
                       ownership structures, and other business intelligence."
            .to_string(),
        input_schema: serde_json::json!({
            "type": "object",
            "properties": {
                "query": {
                    "type": "string",
                    "description": "The search query"
                }
            },
            "required": ["query"]
        }),
    }
}

/// Claude API implementation of ResearchLlmClient
pub struct ClaudeResearchClient {
    api_key: String,
    model: String,
    client: reqwest::Client,
}

impl ClaudeResearchClient {
    pub fn new(api_key: String) -> Self {
        Self {
            api_key,
            model: "claude-sonnet-4-20250514".to_string(),
            client: reqwest::Client::new(),
        }
    }

    pub fn with_model(api_key: String, model: String) -> Self {
        Self {
            api_key,
            model,
            client: reqwest::Client::new(),
        }
    }

    pub fn from_env() -> Result<Self> {
        let api_key = std::env::var("ANTHROPIC_API_KEY").map_err(|_| {
            super::error::ResearchError::LlmClient("ANTHROPIC_API_KEY not set".into())
        })?;
        Ok(Self::new(api_key))
    }

    /// Execute a web search using Brave Search API
    ///
    /// Requires BRAVE_SEARCH_API_KEY environment variable.
    /// Falls back to LLM knowledge if API key not set or search fails.
    async fn execute_web_search(&self, query: &str) -> Result<Value> {
        tracing::info!(query = %query, "Executing web search");

        // Check for Brave API key
        let api_key = match std::env::var("BRAVE_SEARCH_API_KEY") {
            Ok(key) if !key.is_empty() => key,
            _ => {
                tracing::warn!("BRAVE_SEARCH_API_KEY not set - falling back to LLM knowledge");
                return Ok(serde_json::json!({
                    "query": query,
                    "results": [],
                    "fallback": true,
                    "note": "Web search unavailable (BRAVE_SEARCH_API_KEY not set). Using LLM knowledge only."
                }));
            }
        };

        // Call Brave Search API
        let response = self
            .client
            .get("https://api.search.brave.com/res/v1/web/search")
            .header("X-Subscription-Token", &api_key)
            .header("Accept", "application/json")
            .query(&[("q", query), ("count", "5")])
            .send()
            .await;

        match response {
            Ok(resp) if resp.status().is_success() => {
                let body: Value = resp.json().await.unwrap_or_default();

                // Extract web results from Brave response
                let results: Vec<Value> = body["web"]["results"]
                    .as_array()
                    .unwrap_or(&vec![])
                    .iter()
                    .take(5)
                    .map(|r| {
                        serde_json::json!({
                            "title": r["title"].as_str().unwrap_or(""),
                            "url": r["url"].as_str().unwrap_or(""),
                            "snippet": r["description"].as_str().unwrap_or("")
                        })
                    })
                    .collect();

                tracing::info!(result_count = results.len(), "Web search completed");

                Ok(serde_json::json!({
                    "query": query,
                    "results": results,
                    "fallback": false
                }))
            }
            Ok(resp) => {
                let status = resp.status();
                let error_body = resp.text().await.unwrap_or_default();
                tracing::warn!(
                    status = %status,
                    error = %error_body,
                    "Brave Search API error - falling back to LLM knowledge"
                );
                Ok(serde_json::json!({
                    "query": query,
                    "results": [],
                    "fallback": true,
                    "note": format!("Search API error ({}). Using LLM knowledge only.", status)
                }))
            }
            Err(e) => {
                tracing::warn!(error = %e, "Web search request failed - falling back to LLM knowledge");
                Ok(serde_json::json!({
                    "query": query,
                    "results": [],
                    "fallback": true,
                    "note": format!("Search request failed: {}. Using LLM knowledge only.", e)
                }))
            }
        }
    }
}

#[async_trait]
impl ResearchLlmClient for ClaudeResearchClient {
    async fn complete_with_tools(
        &self,
        system_prompt: &str,
        user_prompt: &str,
        tools: &[ToolDef],
    ) -> Result<LlmResponse> {
        let mut messages = vec![serde_json::json!({
            "role": "user",
            "content": user_prompt
        })];

        let mut all_sources = Vec::new();
        let mut all_tool_calls = Vec::new();

        // Convert tools to Claude format
        let claude_tools: Vec<Value> = tools
            .iter()
            .map(|t| {
                serde_json::json!({
                    "name": t.name,
                    "description": t.description,
                    "input_schema": t.input_schema
                })
            })
            .collect();

        // Iterative tool use loop (max 10 iterations to prevent infinite loops)
        for iteration in 0..10 {
            let request_body = serde_json::json!({
                "model": self.model,
                "max_tokens": 4096,
                "system": system_prompt,
                "messages": messages,
                "tools": claude_tools
            });

            let response = self
                .client
                .post("https://api.anthropic.com/v1/messages")
                .header("x-api-key", &self.api_key)
                .header("anthropic-version", "2023-06-01")
                .header("content-type", "application/json")
                .json(&request_body)
                .send()
                .await
                .map_err(|e| super::error::ResearchError::LlmClient(e.to_string()))?;

            let status = response.status();
            let body: Value = response
                .json()
                .await
                .map_err(|e| super::error::ResearchError::LlmClient(e.to_string()))?;

            if !status.is_success() {
                return Err(super::error::ResearchError::LlmClient(format!(
                    "Claude API error {}: {:?}",
                    status, body
                )));
            }

            let stop_reason = body["stop_reason"].as_str().unwrap_or("unknown");
            let content = body["content"].as_array().cloned().unwrap_or_default();

            // Check for tool use
            if stop_reason == "tool_use" {
                let mut tool_results = Vec::new();

                for block in &content {
                    if block["type"] == "tool_use" {
                        let tool_name = block["name"].as_str().unwrap_or("");
                        let tool_id = block["id"].as_str().unwrap_or("");
                        let tool_input = &block["input"];

                        tracing::debug!(
                            iteration = iteration,
                            tool = tool_name,
                            "Processing tool use"
                        );

                        // Execute the tool
                        let tool_output = if tool_name == "web_search" {
                            let query = tool_input["query"].as_str().unwrap_or("");
                            let result = self.execute_web_search(query).await?;

                            // Extract sources from results
                            if let Some(results) = result["results"].as_array() {
                                for r in results {
                                    all_sources.push(ResearchSource {
                                        url: r["url"].as_str().unwrap_or("").to_string(),
                                        title: r["title"].as_str().map(String::from),
                                        snippet: r["snippet"].as_str().map(String::from),
                                    });
                                }
                            }

                            result
                        } else {
                            serde_json::json!({
                                "error": format!("Unknown tool: {}", tool_name)
                            })
                        };

                        all_tool_calls.push(ToolCall {
                            tool_name: tool_name.to_string(),
                            input: tool_input.clone(),
                            output: tool_output.clone(),
                        });

                        tool_results.push(serde_json::json!({
                            "type": "tool_result",
                            "tool_use_id": tool_id,
                            "content": serde_json::to_string(&tool_output).unwrap_or_default()
                        }));
                    }
                }

                // Add assistant message with tool use blocks
                messages.push(serde_json::json!({
                    "role": "assistant",
                    "content": content
                }));

                // Add tool results
                messages.push(serde_json::json!({
                    "role": "user",
                    "content": tool_results
                }));

                continue;
            }

            // No more tool use - extract final text content
            let mut final_text = String::new();
            for block in content {
                if block["type"] == "text" {
                    if let Some(text) = block["text"].as_str() {
                        final_text.push_str(text);
                    }
                }
            }

            return Ok(LlmResponse {
                content: final_text,
                sources: all_sources,
                tool_calls: all_tool_calls,
            });
        }

        Err(super::error::ResearchError::LlmClient(
            "Max tool use iterations exceeded".into(),
        ))
    }

    async fn complete_json(
        &self,
        system_prompt: &str,
        user_prompt: &str,
        _schema: &Value,
    ) -> Result<Value> {
        let enhanced_system = format!(
            "{}\n\nIMPORTANT: Return ONLY valid JSON. No markdown, no explanation, just JSON.",
            system_prompt
        );

        let request_body = serde_json::json!({
            "model": self.model,
            "max_tokens": 4096,
            "system": enhanced_system,
            "messages": [{
                "role": "user",
                "content": user_prompt
            }]
        });

        let response = self
            .client
            .post("https://api.anthropic.com/v1/messages")
            .header("x-api-key", &self.api_key)
            .header("anthropic-version", "2023-06-01")
            .header("content-type", "application/json")
            .json(&request_body)
            .send()
            .await
            .map_err(|e| super::error::ResearchError::LlmClient(e.to_string()))?;

        let status = response.status();
        let body: Value = response
            .json()
            .await
            .map_err(|e| super::error::ResearchError::LlmClient(e.to_string()))?;

        if !status.is_success() {
            return Err(super::error::ResearchError::LlmClient(format!(
                "Claude API error {}: {:?}",
                status, body
            )));
        }

        // Extract text content
        let content = body["content"]
            .as_array()
            .and_then(|arr| arr.first())
            .and_then(|block| block["text"].as_str())
            .unwrap_or("");

        // Parse as JSON
        serde_json::from_str(content).map_err(|e| {
            super::error::ResearchError::JsonParse(format!(
                "Failed to parse LLM response as JSON: {}. Content: {}",
                e,
                &content[..content.len().min(200)]
            ))
        })
    }

    fn model_name(&self) -> &str {
        &self.model
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_web_search_tool_schema() {
        let tool = web_search_tool();
        assert_eq!(tool.name, "web_search");
        assert!(tool.input_schema["properties"]["query"].is_object());
    }
}
