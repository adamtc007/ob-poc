//! Template Definition Types
//!
//! YAML schema for workflow templates that expand to DSL source text.

use serde::{Deserialize, Serialize};
use std::collections::HashMap;

/// A complete template definition loaded from YAML
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct TemplateDefinition {
    /// Template identifier (e.g., "onboard-director")
    pub template: String,

    /// Version number
    #[serde(default = "default_version")]
    pub version: u32,

    /// Rich metadata for agent understanding
    pub metadata: TemplateMetadata,

    /// Tags for discovery
    #[serde(default)]
    pub tags: Vec<String>,

    /// Links to workflows, states, and blockers
    #[serde(default)]
    pub workflow_context: WorkflowContext,

    /// Parameter definitions
    #[serde(default)]
    pub params: HashMap<String, ParamDefinition>,

    /// DSL template body with $param placeholders
    pub body: String,

    /// Output bindings produced by the template
    #[serde(default)]
    pub outputs: HashMap<String, OutputDefinition>,

    /// Related templates for chaining
    #[serde(default)]
    pub related_templates: Vec<String>,
}

fn default_version() -> u32 {
    1
}

/// Rich metadata to help agent understand when/how to use template
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct TemplateMetadata {
    /// Human-readable name
    pub name: String,

    /// One-line summary
    pub summary: String,

    /// Detailed description
    #[serde(default)]
    pub description: String,

    /// When should agent use this template?
    #[serde(default)]
    pub when_to_use: Vec<String>,

    /// When should agent NOT use this template?
    #[serde(default)]
    pub when_not_to_use: Vec<String>,

    /// What happens when this template runs?
    #[serde(default)]
    pub effects: Vec<String>,

    /// What might agent do next?
    #[serde(default)]
    pub next_steps: Vec<String>,
}

/// Links template to workflow states and blockers
#[derive(Debug, Clone, Default, Serialize, Deserialize)]
pub struct WorkflowContext {
    /// Which workflows this template applies to
    #[serde(default)]
    pub applicable_workflows: Vec<String>,

    /// Which workflow states this template can be used in
    #[serde(default)]
    pub applicable_states: Vec<String>,

    /// Blocker types this template resolves (e.g., "missing_role:DIRECTOR")
    #[serde(default)]
    pub resolves_blockers: Vec<String>,
}

/// Parameter definition
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ParamDefinition {
    /// Parameter type (string, uuid, date, cbu_ref, entity_ref, enum, etc.)
    #[serde(rename = "type")]
    pub param_type: String,

    /// Is this parameter required?
    #[serde(default)]
    pub required: bool,

    /// Where to get the value: "session", "blocker", "$other_param"
    #[serde(default)]
    pub source: Option<String>,

    /// Default value (can be literal or "$other_param" reference)
    #[serde(default)]
    pub default: Option<String>,

    /// Human-readable prompt for the value
    #[serde(default)]
    pub prompt: Option<String>,

    /// Example value
    #[serde(default)]
    pub example: Option<String>,

    /// Validation rules (human-readable)
    #[serde(default)]
    pub validation: Option<String>,

    /// Description of the parameter
    #[serde(default)]
    pub description: Option<String>,

    /// Enum values if param_type is "enum"
    #[serde(default)]
    pub enum_values: Option<Vec<String>>,
}

/// Output definition
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct OutputDefinition {
    /// Output type
    #[serde(rename = "type")]
    pub output_type: String,

    /// Description
    #[serde(default)]
    pub description: String,

    /// Binding name in the DSL let expression
    #[serde(default)]
    pub binding: Option<String>,
}

impl TemplateDefinition {
    /// Get list of required parameters
    pub fn required_params(&self) -> Vec<(&String, &ParamDefinition)> {
        self.params.iter().filter(|(_, p)| p.required).collect()
    }

    /// Get list of optional parameters
    pub fn optional_params(&self) -> Vec<(&String, &ParamDefinition)> {
        self.params.iter().filter(|(_, p)| !p.required).collect()
    }

    /// Get parameters that need user input (no source, no default)
    pub fn user_input_params(&self) -> Vec<(&String, &ParamDefinition)> {
        self.params
            .iter()
            .filter(|(_, p)| p.required && p.source.is_none() && p.default.is_none())
            .collect()
    }

    /// Check if template resolves a specific blocker
    pub fn resolves_blocker(&self, blocker_type: &str) -> bool {
        self.workflow_context
            .resolves_blockers
            .iter()
            .any(|b| b == blocker_type || blocker_type.starts_with(b))
    }

    /// Check if template applies to workflow state
    pub fn applies_to_state(&self, workflow: &str, state: &str) -> bool {
        let workflow_match = self.workflow_context.applicable_workflows.is_empty()
            || self
                .workflow_context
                .applicable_workflows
                .iter()
                .any(|w| w == workflow);
        let state_match = self.workflow_context.applicable_states.is_empty()
            || self
                .workflow_context
                .applicable_states
                .iter()
                .any(|s| s == state);
        workflow_match && state_match
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    const SAMPLE_TEMPLATE: &str = r#"
template: onboard-director
version: 1

metadata:
  name: Onboard Director
  summary: Add a natural person as director with full KYC setup
  description: |
    Creates a new person entity and onboards them as a director.
  when_to_use:
    - Adding a new director who doesn't exist
  effects:
    - New person entity created
    - DIRECTOR role assigned

tags:
  - director
  - person

workflow_context:
  applicable_workflows:
    - kyc_onboarding
  applicable_states:
    - ENTITY_COLLECTION
  resolves_blockers:
    - missing_role:DIRECTOR

params:
  cbu_id:
    type: cbu_ref
    required: true
    source: session
  name:
    type: string
    required: true
    prompt: "Director's full legal name"
    example: "John Smith"
  date_of_birth:
    type: date
    required: true
    prompt: "Date of birth"

body: |
  (let [person (entity.create-proper-person :name "$name" :date-of-birth "$date_of_birth")]
    (cbu.assign-role :cbu "$cbu_id" :entity person :role DIRECTOR))

outputs:
  person:
    type: entity_ref
    description: Created person entity
    binding: person
"#;

    #[test]
    fn test_parse_template() {
        let template: TemplateDefinition = serde_yaml::from_str(SAMPLE_TEMPLATE).unwrap();

        assert_eq!(template.template, "onboard-director");
        assert_eq!(template.version, 1);
        assert_eq!(template.metadata.name, "Onboard Director");
        assert_eq!(template.tags.len(), 2);
        assert!(template.tags.contains(&"director".to_string()));
    }

    #[test]
    fn test_required_params() {
        let template: TemplateDefinition = serde_yaml::from_str(SAMPLE_TEMPLATE).unwrap();
        let required = template.required_params();
        assert_eq!(required.len(), 3);
    }

    #[test]
    fn test_user_input_params() {
        let template: TemplateDefinition = serde_yaml::from_str(SAMPLE_TEMPLATE).unwrap();
        let user_input = template.user_input_params();
        // cbu_id has source=session, so only name and date_of_birth need user input
        assert_eq!(user_input.len(), 2);
    }

    #[test]
    fn test_resolves_blocker() {
        let template: TemplateDefinition = serde_yaml::from_str(SAMPLE_TEMPLATE).unwrap();
        assert!(template.resolves_blocker("missing_role:DIRECTOR"));
        assert!(!template.resolves_blocker("missing_document"));
    }

    #[test]
    fn test_applies_to_state() {
        let template: TemplateDefinition = serde_yaml::from_str(SAMPLE_TEMPLATE).unwrap();
        assert!(template.applies_to_state("kyc_onboarding", "ENTITY_COLLECTION"));
        assert!(!template.applies_to_state("kyc_onboarding", "SCREENING"));
    }
}
