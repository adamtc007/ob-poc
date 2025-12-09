//! Backend Selection
//!
//! Enum for selecting between LLM providers (Anthropic, OpenAI).

use anyhow::{anyhow, Result};

/// LLM backend provider selection
#[derive(Debug, Clone, Copy, PartialEq, Eq, Default)]
pub enum AgentBackend {
    /// Anthropic Claude (default)
    #[default]
    Anthropic,
    /// OpenAI GPT
    OpenAi,
}

impl AgentBackend {
    /// Create from AGENT_BACKEND environment variable
    ///
    /// Valid values: "anthropic", "claude", "openai", "gpt"
    /// Defaults to Anthropic if not set
    pub fn from_env() -> Result<Self> {
        let value = std::env::var("AGENT_BACKEND").unwrap_or_else(|_| "anthropic".to_string());
        Self::from_str(&value)
    }

    /// Parse from string
    pub fn from_str(s: &str) -> Result<Self> {
        match s.to_lowercase().as_str() {
            "anthropic" | "claude" => Ok(AgentBackend::Anthropic),
            "openai" | "gpt" => Ok(AgentBackend::OpenAi),
            other => Err(anyhow!(
                "Unknown AGENT_BACKEND '{}'. Valid values: anthropic, claude, openai, gpt",
                other
            )),
        }
    }

    /// Get display name
    pub fn name(&self) -> &'static str {
        match self {
            AgentBackend::Anthropic => "Anthropic",
            AgentBackend::OpenAi => "OpenAI",
        }
    }
}

impl std::fmt::Display for AgentBackend {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        write!(f, "{}", self.name())
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_from_str() {
        assert_eq!(
            AgentBackend::from_str("anthropic").unwrap(),
            AgentBackend::Anthropic
        );
        assert_eq!(
            AgentBackend::from_str("claude").unwrap(),
            AgentBackend::Anthropic
        );
        assert_eq!(
            AgentBackend::from_str("ANTHROPIC").unwrap(),
            AgentBackend::Anthropic
        );
        assert_eq!(
            AgentBackend::from_str("openai").unwrap(),
            AgentBackend::OpenAi
        );
        assert_eq!(AgentBackend::from_str("gpt").unwrap(), AgentBackend::OpenAi);
        assert!(AgentBackend::from_str("invalid").is_err());
    }

    #[test]
    fn test_default() {
        assert_eq!(AgentBackend::default(), AgentBackend::Anthropic);
    }
}
