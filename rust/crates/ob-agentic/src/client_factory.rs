//! Client Factory
//!
//! Factory for creating LLM clients based on environment configuration.

use anyhow::{anyhow, Result};
use std::sync::Arc;

use super::anthropic_client::AnthropicClient;
use super::backend::AgentBackend;
use super::llm_client::LlmClient;
use super::openai_client::OpenAiClient;

/// Create an LLM client based on AGENT_BACKEND environment variable
///
/// Uses the appropriate API key environment variable:
/// - Anthropic: ANTHROPIC_API_KEY
/// - OpenAI: OPENAI_API_KEY
pub fn create_llm_client() -> Result<Arc<dyn LlmClient>> {
    let backend = AgentBackend::from_env()?;
    match backend {
        AgentBackend::Anthropic => {
            let client = AnthropicClient::from_env()?;
            Ok(Arc::new(client))
        }
        AgentBackend::OpenAi => {
            let client = OpenAiClient::from_env()?;
            Ok(Arc::new(client))
        }
    }
}

/// Create an LLM client with explicit API key
///
/// For backward compatibility where an API key is passed directly.
/// The key is used for the selected backend (from AGENT_BACKEND env).
pub fn create_llm_client_with_key(api_key: String) -> Result<Arc<dyn LlmClient>> {
    let backend = AgentBackend::from_env()?;
    match backend {
        AgentBackend::Anthropic => Ok(Arc::new(AnthropicClient::new(api_key))),
        AgentBackend::OpenAi => {
            // For OpenAI, the passed key might be ANTHROPIC_API_KEY (legacy),
            // so prefer OPENAI_API_KEY from env if available
            if let Ok(openai_key) = std::env::var("OPENAI_API_KEY") {
                Ok(Arc::new(OpenAiClient::new(openai_key)))
            } else {
                // Fall back to passed key (user might have passed OpenAI key)
                Ok(Arc::new(OpenAiClient::new(api_key)))
            }
        }
    }
}

/// Create an LLM client for a specific backend
///
/// Ignores AGENT_BACKEND env var and uses the specified backend.
pub fn create_llm_client_for_backend(backend: AgentBackend) -> Result<Arc<dyn LlmClient>> {
    match backend {
        AgentBackend::Anthropic => {
            let client = AnthropicClient::from_env()?;
            Ok(Arc::new(client))
        }
        AgentBackend::OpenAi => {
            let client = OpenAiClient::from_env()?;
            Ok(Arc::new(client))
        }
    }
}

/// Get the currently configured backend from environment
pub fn current_backend() -> Result<AgentBackend> {
    AgentBackend::from_env()
}

/// Check if an API key is available for the given backend
pub fn has_api_key_for(backend: AgentBackend) -> bool {
    match backend {
        AgentBackend::Anthropic => std::env::var("ANTHROPIC_API_KEY").is_ok(),
        AgentBackend::OpenAi => std::env::var("OPENAI_API_KEY").is_ok(),
    }
}

/// Get the API key for the given backend, if available
pub fn get_api_key_for(backend: AgentBackend) -> Result<String> {
    match backend {
        AgentBackend::Anthropic => {
            std::env::var("ANTHROPIC_API_KEY").map_err(|_| anyhow!("ANTHROPIC_API_KEY not set"))
        }
        AgentBackend::OpenAi => {
            std::env::var("OPENAI_API_KEY").map_err(|_| anyhow!("OPENAI_API_KEY not set"))
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_has_api_key() {
        // This test just verifies the function runs without panic
        let _ = has_api_key_for(AgentBackend::Anthropic);
        let _ = has_api_key_for(AgentBackend::OpenAi);
    }
}
