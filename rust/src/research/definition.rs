//! Research macro definition types
//!
//! These types are loaded from YAML files in `config/macros/research/`.

use serde::{Deserialize, Serialize};
use serde_json::Value;

/// Research macro definition loaded from YAML
#[derive(Debug, Clone, Deserialize, Serialize)]
pub struct ResearchMacroDef {
    /// Unique macro name (e.g., "client-discovery")
    pub name: String,

    /// Version string for compatibility tracking
    #[serde(default = "default_version")]
    pub version: String,

    /// Human-readable description
    pub description: String,

    /// Parameter definitions
    #[serde(default)]
    pub parameters: Vec<MacroParamDef>,

    /// Tools the LLM can use (e.g., ["web_search"])
    #[serde(default)]
    pub tools: Vec<String>,

    /// Handlebars prompt template
    pub prompt: String,

    /// Output schema and review requirements
    pub output: ResearchOutput,

    /// Handlebars template for generating DSL verbs from result
    pub suggested_verbs: Option<String>,

    /// Tags for categorization and search
    #[serde(default)]
    pub tags: Vec<String>,
}

fn default_version() -> String {
    "1.0".to_string()
}

/// Parameter definition for a research macro
#[derive(Debug, Clone, Deserialize, Serialize)]
pub struct MacroParamDef {
    /// Parameter name
    pub name: String,

    /// Type: string, integer, boolean, array, object
    #[serde(rename = "type")]
    pub param_type: String,

    /// Whether the parameter is required
    #[serde(default)]
    pub required: bool,

    /// Human-readable description for prompts
    pub description: Option<String>,

    /// Default value if not provided
    pub default: Option<Value>,

    /// Allowed values (for enum-like parameters)
    #[serde(rename = "enum")]
    pub enum_values: Option<Vec<String>>,

    /// Example value for documentation
    pub example: Option<Value>,
}

/// Output schema and review configuration
#[derive(Debug, Clone, Deserialize, Serialize)]
pub struct ResearchOutput {
    /// Schema name for reference
    pub schema_name: String,

    /// JSON Schema for validating LLM output
    pub schema: Value,

    /// Whether human review is required
    #[serde(default)]
    pub review: ReviewRequirement,
}

/// Whether human review is required before using research results
#[derive(Debug, Clone, Copy, Deserialize, Serialize, PartialEq, Eq, Default)]
#[serde(rename_all = "lowercase")]
pub enum ReviewRequirement {
    /// Always require human review (default for safety)
    #[default]
    Required,
    /// Review recommended but not enforced
    Optional,
    /// No review needed (use with caution)
    None,
}

/// YAML file wrapper - macros are nested under `macro:` key
#[derive(Debug, Clone, Deserialize)]
pub struct ResearchMacroWrapper {
    #[serde(rename = "macro")]
    pub macro_def: ResearchMacroDef,
}

impl ResearchMacroDef {
    /// Get parameter by name
    pub fn get_param(&self, name: &str) -> Option<&MacroParamDef> {
        self.parameters.iter().find(|p| p.name == name)
    }

    /// Get required parameters
    pub fn required_params(&self) -> Vec<&MacroParamDef> {
        self.parameters.iter().filter(|p| p.required).collect()
    }

    /// Check if a specific tool is enabled
    pub fn has_tool(&self, tool: &str) -> bool {
        self.tools.iter().any(|t| t == tool)
    }

    /// Check if review is required
    pub fn requires_review(&self) -> bool {
        self.output.review == ReviewRequirement::Required
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_parse_macro_yaml() {
        let yaml = r#"
macro:
  name: test-macro
  version: "1.0"
  description: Test macro for unit testing
  parameters:
    - name: client_name
      type: string
      required: true
      description: Name of the client to research
    - name: jurisdiction_hint
      type: string
      required: false
      default: null
  tools:
    - web_search
  prompt: "Research {{client_name}} in {{jurisdiction_hint}}"
  output:
    schema_name: test-result
    schema:
      type: object
      properties:
        name:
          type: string
      required:
        - name
    review: required
  suggested_verbs: |
    (gleif.enrich :lei "{{lei}}")
  tags:
    - test
    - discovery
"#;
        let wrapper: ResearchMacroWrapper = serde_yaml::from_str(yaml).unwrap();
        let def = wrapper.macro_def;

        assert_eq!(def.name, "test-macro");
        assert_eq!(def.version, "1.0");
        assert_eq!(def.parameters.len(), 2);
        assert!(def.parameters[0].required);
        assert!(!def.parameters[1].required);
        assert!(def.has_tool("web_search"));
        assert!(!def.has_tool("code_exec"));
        assert!(def.requires_review());
        assert_eq!(def.tags, vec!["test", "discovery"]);
    }

    #[test]
    fn test_review_requirement_default() {
        let output: ResearchOutput = serde_yaml::from_str(
            r#"
schema_name: test
schema: {}
"#,
        )
        .unwrap();
        assert_eq!(output.review, ReviewRequirement::Required);
    }
}
