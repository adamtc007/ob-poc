//! Backend Selection
//!
//! Enum for selecting between LLM providers (Anthropic, OpenAI).

use anyhow::{anyhow, Result};
use std::str::FromStr;

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
        value
            .parse()
            .map_err(|e: ParseBackendError| anyhow!("{}", e))
    }

    /// Get display name
    pub fn name(&self) -> &'static str {
        match self {
            AgentBackend::Anthropic => "Anthropic",
            AgentBackend::OpenAi => "OpenAI",
        }
    }
}

/// Error type for parsing AgentBackend
#[derive(Debug)]
pub struct ParseBackendError(String);

impl std::fmt::Display for ParseBackendError {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        write!(f, "{}", self.0)
    }
}

impl std::error::Error for ParseBackendError {}

impl FromStr for AgentBackend {
    type Err = ParseBackendError;

    fn from_str(s: &str) -> std::result::Result<Self, Self::Err> {
        match s.to_lowercase().as_str() {
            "anthropic" | "claude" => Ok(AgentBackend::Anthropic),
            "openai" | "gpt" => Ok(AgentBackend::OpenAi),
            other => Err(ParseBackendError(format!(
                "Unknown AGENT_BACKEND '{}'. Valid values: anthropic, claude, openai, gpt",
                other
            ))),
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
            "anthropic".parse::<AgentBackend>().unwrap(),
            AgentBackend::Anthropic
        );
        assert_eq!(
            "claude".parse::<AgentBackend>().unwrap(),
            AgentBackend::Anthropic
        );
        assert_eq!(
            "ANTHROPIC".parse::<AgentBackend>().unwrap(),
            AgentBackend::Anthropic
        );
        assert_eq!(
            "openai".parse::<AgentBackend>().unwrap(),
            AgentBackend::OpenAi
        );
        assert_eq!("gpt".parse::<AgentBackend>().unwrap(), AgentBackend::OpenAi);
        assert!("invalid".parse::<AgentBackend>().is_err());
    }

    #[test]
    fn test_default() {
        assert_eq!(AgentBackend::default(), AgentBackend::Anthropic);
    }
}
