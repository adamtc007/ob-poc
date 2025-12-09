//! LLM Client Trait
//!
//! Unified interface for LLM providers (Anthropic, OpenAI).

use anyhow::Result;
use async_trait::async_trait;

/// Unified LLM client interface for both Anthropic and OpenAI
#[async_trait]
pub trait LlmClient: Send + Sync {
    /// Call the LLM with system + user prompts, return raw text response
    async fn chat(&self, system_prompt: &str, user_prompt: &str) -> Result<String>;

    /// Call the LLM expecting JSON response
    /// - For OpenAI: uses response_format json_object mode
    /// - For Anthropic: adds JSON instruction to system prompt
    async fn chat_json(&self, system_prompt: &str, user_prompt: &str) -> Result<String>;

    /// Get the model name for logging
    fn model_name(&self) -> &str;

    /// Get the provider name for logging
    fn provider_name(&self) -> &str;
}
