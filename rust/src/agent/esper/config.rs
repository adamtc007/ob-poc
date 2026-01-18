//! ESPER command YAML configuration types
//!
//! Defines the serde schema for `config/esper-commands.yaml`.

use serde::Deserialize;
use std::collections::HashMap;
use std::path::Path;

/// Root configuration structure
#[derive(Debug, Deserialize)]
pub struct EsperConfig {
    pub version: String,
    pub commands: HashMap<String, EsperCommandDef>,
}

/// Definition of a single ESPER command
#[derive(Debug, Clone, Deserialize)]
pub struct EsperCommandDef {
    /// The "proper" way to say it (for help text)
    pub canonical: String,

    /// What to show user when command matches
    pub response: String,

    /// Higher wins for overlapping patterns (default: 100)
    #[serde(default = "default_priority")]
    pub priority: u32,

    /// Maps to AgentCommand enum variant
    pub agent_command: AgentCommandSpec,

    /// All phrases that trigger this command
    #[serde(default)]
    pub aliases: AliasSpec,
}

fn default_priority() -> u32 {
    100
}

/// Specification for building an AgentCommand
#[derive(Debug, Clone, Deserialize)]
pub struct AgentCommandSpec {
    /// Maps to AgentCommand variant name (e.g., "ZoomIn", "ScaleUniverse")
    #[serde(rename = "type")]
    pub command_type: String,

    /// Parameter extraction hints
    #[serde(default)]
    pub params: HashMap<String, ParamSource>,
}

/// How to extract a parameter value
#[derive(Debug, Clone, Deserialize, PartialEq)]
#[serde(rename_all = "snake_case")]
pub enum ParamSource {
    /// Parse from phrase (number, direction, etc.)
    Extract,
    /// Get from session context (current entity/cbu)
    Context,
    /// Everything after prefix match
    RestOfPhrase,
}

/// Alias patterns for matching phrases
#[derive(Debug, Clone, Default, Deserialize)]
pub struct AliasSpec {
    /// Must match exactly (after normalization)
    #[serde(default)]
    pub exact: Vec<String>,

    /// Substring match
    #[serde(default)]
    pub contains: Vec<String>,

    /// Match prefix, extract rest as parameter
    #[serde(default)]
    pub prefix: Vec<String>,
}

impl EsperConfig {
    /// Load configuration from YAML file
    pub fn load<P: AsRef<Path>>(path: P) -> anyhow::Result<Self> {
        let content = std::fs::read_to_string(path.as_ref())?;
        let config: EsperConfig = serde_yaml::from_str(&content)?;
        Ok(config)
    }

    /// Load configuration from YAML string (for testing)
    pub fn from_yaml(yaml: &str) -> anyhow::Result<Self> {
        let config: EsperConfig = serde_yaml::from_str(yaml)?;
        Ok(config)
    }
}

#[cfg(test)]
mod config_tests {
    use super::*;

    #[test]
    fn test_load_config() {
        let yaml = r#"
version: "1.0"
commands:
  stop:
    canonical: "stop"
    response: "Stopping."
    priority: 1000
    agent_command:
      type: Stop
    aliases:
      exact:
        - "stop"
        - "halt"
  zoom_in:
    canonical: "enhance"
    response: "Enhancing..."
    agent_command:
      type: ZoomIn
      params:
        factor: extract
    aliases:
      exact:
        - "enhance"
      prefix:
        - "zoom in"
"#;
        let config = EsperConfig::from_yaml(yaml).unwrap();
        assert_eq!(config.version, "1.0");
        assert_eq!(config.commands.len(), 2);

        let stop = config.commands.get("stop").unwrap();
        assert_eq!(stop.canonical, "stop");
        assert_eq!(stop.priority, 1000);
        assert_eq!(stop.agent_command.command_type, "Stop");
        assert_eq!(stop.aliases.exact, vec!["stop", "halt"]);

        let zoom = config.commands.get("zoom_in").unwrap();
        assert_eq!(zoom.priority, 100); // default
        assert_eq!(
            zoom.agent_command.params.get("factor"),
            Some(&ParamSource::Extract)
        );
    }
}
