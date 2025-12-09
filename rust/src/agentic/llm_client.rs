//! LLM Client Trait
//!
//! Unified interface for LLM providers (Anthropic, OpenAI).

use anyhow::Result;
use async_trait::async_trait;
use serde::{Deserialize, Serialize};

/// Tool/function definition for structured output
///
/// Used with `chat_with_tool()` to force the LLM to return structured JSON.
/// - Anthropic: maps to `tools` array with `tool_choice`
/// - OpenAI: maps to `functions` array with `function_call`
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ToolDefinition {
    /// Tool/function name (e.g., "generate_dsl_intents")
    pub name: String,
    /// Description of what the tool does
    pub description: String,
    /// JSON Schema for the tool's parameters
    pub parameters: serde_json::Value,
}

/// Result from a tool/function call
///
/// Contains the structured JSON arguments returned by the LLM.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ToolCallResult {
    /// Name of the tool that was called
    pub tool_name: String,
    /// Structured arguments as JSON
    pub arguments: serde_json::Value,
}

/// Unified LLM client interface for both Anthropic and OpenAI
#[async_trait]
pub trait LlmClient: Send + Sync {
    /// Call the LLM with system + user prompts, return raw text response
    async fn chat(&self, system_prompt: &str, user_prompt: &str) -> Result<String>;

    /// Call the LLM expecting JSON response
    /// - For OpenAI: uses response_format json_object mode
    /// - For Anthropic: adds JSON instruction to system prompt
    async fn chat_json(&self, system_prompt: &str, user_prompt: &str) -> Result<String>;

    /// Call LLM with a tool/function, forcing structured output
    ///
    /// This guarantees valid JSON output from both providers:
    /// - Anthropic: uses tool_use with tool_choice
    /// - OpenAI: uses function_calling with function_call
    async fn chat_with_tool(
        &self,
        system_prompt: &str,
        user_prompt: &str,
        tool: &ToolDefinition,
    ) -> Result<ToolCallResult>;

    /// Get the model name for logging
    fn model_name(&self) -> &str;

    /// Get the provider name for logging
    fn provider_name(&self) -> &str;
}
