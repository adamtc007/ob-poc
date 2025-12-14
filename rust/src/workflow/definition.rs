//! Workflow Definition Types and YAML Loading
//!
//! Workflows are defined in YAML files and loaded at startup.

use serde::{Deserialize, Serialize};
use std::collections::HashMap;
use std::path::Path;

use super::WorkflowError;

/// A complete workflow definition loaded from YAML
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct WorkflowDefinition {
    /// Workflow identifier
    pub workflow: String,
    /// Version number
    #[serde(default = "default_version")]
    pub version: u32,
    /// Human-readable description
    #[serde(default)]
    pub description: String,

    /// What triggers this workflow
    #[serde(default)]
    pub trigger: Option<TriggerDef>,

    /// State definitions
    pub states: HashMap<String, StateDef>,

    /// Valid transitions between states
    #[serde(default)]
    pub transitions: Vec<TransitionDef>,

    /// Requirements for each state
    #[serde(default)]
    pub requirements: HashMap<String, Vec<RequirementDef>>,

    /// Actions available at each state
    #[serde(default)]
    pub actions: HashMap<String, Vec<ActionDef>>,
}

fn default_version() -> u32 {
    1
}

impl WorkflowDefinition {
    /// Get the initial state for this workflow
    pub fn initial_state(&self) -> Option<&str> {
        self.states
            .iter()
            .find(|(_, s)| s.initial)
            .map(|(name, _)| name.as_str())
    }

    /// Get terminal states
    pub fn terminal_states(&self) -> Vec<String> {
        self.states
            .iter()
            .filter(|(_, s)| s.terminal)
            .map(|(name, _)| name.clone())
            .collect()
    }

    /// Get transitions from a specific state
    pub fn transitions_from(&self, state: &str) -> Vec<&TransitionDef> {
        self.transitions
            .iter()
            .filter(|t| t.from == state)
            .collect()
    }

    /// Get available actions for a state
    pub fn actions_for_state(&self, state: &str) -> Vec<&ActionDef> {
        self.actions
            .get(state)
            .map(|a| a.iter().collect())
            .unwrap_or_default()
    }

    /// Check if a transition is valid
    pub fn is_valid_transition(&self, from: &str, to: &str) -> bool {
        self.transitions
            .iter()
            .any(|t| t.from == from && t.to == to)
    }

    /// Get transition definition
    pub fn get_transition(&self, from: &str, to: &str) -> Option<&TransitionDef> {
        self.transitions
            .iter()
            .find(|t| t.from == from && t.to == to)
    }
}

/// Workflow trigger definition
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct TriggerDef {
    /// Event that triggers this workflow (e.g., "cbu.created")
    pub event: String,
    /// Conditions that must be met
    #[serde(default)]
    pub conditions: Vec<TriggerCondition>,
}

/// A trigger condition
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct TriggerCondition {
    /// Field to check
    pub field: String,
    /// Values that match (OR)
    #[serde(rename = "in", default)]
    pub in_values: Vec<String>,
    /// Value that must match
    #[serde(default)]
    pub equals: Option<String>,
}

/// State definition
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct StateDef {
    /// Human-readable description
    #[serde(default)]
    pub description: String,
    /// Is this the initial state?
    #[serde(default)]
    pub initial: bool,
    /// Is this a terminal state?
    #[serde(default)]
    pub terminal: bool,
    /// Timeout for this state (optional)
    #[serde(default)]
    pub timeout_hours: Option<u32>,
}

/// Transition definition
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct TransitionDef {
    /// Source state
    pub from: String,
    /// Target state
    pub to: String,
    /// Guard function name (must pass to allow transition)
    #[serde(default)]
    pub guard: Option<String>,
    /// Is this an automatic transition (happens when guard passes)?
    #[serde(default)]
    pub auto: bool,
    /// Requires manual action?
    #[serde(default)]
    pub manual: bool,
    /// Description of this transition
    #[serde(default)]
    pub description: Option<String>,
}

/// Requirement definition for a state
#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(tag = "type", rename_all = "snake_case")]
pub enum RequirementDef {
    /// Minimum count of a role
    RoleCount {
        role: String,
        min: u32,
        #[serde(default)]
        description: String,
    },
    /// All entities must be screened
    AllEntitiesScreened {
        #[serde(default)]
        description: String,
    },
    /// Required document set
    DocumentSet {
        documents: Vec<String>,
        #[serde(default)]
        description: String,
    },
    /// Document required per entity of a type
    PerEntityDocument {
        entity_type: String,
        documents: Vec<String>,
        #[serde(default)]
        description: String,
    },
    /// Ownership must sum to threshold
    OwnershipComplete {
        threshold: f64,
        #[serde(default)]
        description: String,
    },
    /// All UBOs must be verified
    AllUbosVerified {
        #[serde(default)]
        description: String,
    },
    /// No open alerts
    NoOpenAlerts {
        #[serde(default)]
        description: String,
    },
    /// Case checklist complete
    CaseChecklistComplete {
        #[serde(default)]
        description: String,
    },
    /// Custom requirement
    Custom {
        code: String,
        #[serde(default)]
        params: HashMap<String, serde_json::Value>,
        #[serde(default)]
        description: String,
    },
}

/// Action available at a state
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ActionDef {
    /// Action identifier
    pub action: String,
    /// DSL verb to execute
    pub verb: String,
    /// Human-readable description
    #[serde(default)]
    pub description: String,
    /// Required parameters
    #[serde(default)]
    pub params: Vec<String>,
}

/// Loader for workflow definitions
pub struct WorkflowLoader;

impl WorkflowLoader {
    /// Load all workflow definitions from a directory
    pub fn load_from_dir(dir: &Path) -> Result<HashMap<String, WorkflowDefinition>, WorkflowError> {
        let mut definitions = HashMap::new();

        if !dir.exists() {
            return Ok(definitions);
        }

        for entry in std::fs::read_dir(dir)? {
            let entry = entry?;
            let path = entry.path();

            if path
                .extension()
                .map(|e| e == "yaml" || e == "yml")
                .unwrap_or(false)
            {
                let content = std::fs::read_to_string(&path)?;
                let def: WorkflowDefinition = serde_yaml::from_str(&content)?;
                definitions.insert(def.workflow.clone(), def);
            }
        }

        Ok(definitions)
    }

    /// Load a single workflow definition from a file
    pub fn load_from_file(path: &Path) -> Result<WorkflowDefinition, WorkflowError> {
        let content = std::fs::read_to_string(path)?;
        let def: WorkflowDefinition = serde_yaml::from_str(&content)?;
        Ok(def)
    }

    /// Load from a YAML string
    pub fn load_from_str(yaml: &str) -> Result<WorkflowDefinition, WorkflowError> {
        let def: WorkflowDefinition = serde_yaml::from_str(yaml)?;
        Ok(def)
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    const SAMPLE_WORKFLOW: &str = r#"
workflow: test_workflow
version: 1
description: Test workflow

states:
  INTAKE:
    description: Initial state
    initial: true
  PROCESSING:
    description: Processing state
  COMPLETE:
    description: Done
    terminal: true

transitions:
  - from: INTAKE
    to: PROCESSING
    auto: true
  - from: PROCESSING
    to: COMPLETE
    guard: processing_complete
    manual: true

requirements:
  PROCESSING:
    - type: role_count
      role: DIRECTOR
      min: 1
      description: Need a director

actions:
  INTAKE:
    - action: add_data
      verb: entity.create
      description: Add initial data
"#;

    #[test]
    fn test_parse_workflow() {
        let def = WorkflowLoader::load_from_str(SAMPLE_WORKFLOW).unwrap();

        assert_eq!(def.workflow, "test_workflow");
        assert_eq!(def.version, 1);
        assert_eq!(def.states.len(), 3);
        assert_eq!(def.transitions.len(), 2);
    }

    #[test]
    fn test_initial_state() {
        let def = WorkflowLoader::load_from_str(SAMPLE_WORKFLOW).unwrap();
        assert_eq!(def.initial_state(), Some("INTAKE"));
    }

    #[test]
    fn test_terminal_states() {
        let def = WorkflowLoader::load_from_str(SAMPLE_WORKFLOW).unwrap();
        let terminals = def.terminal_states();
        assert_eq!(terminals, vec!["COMPLETE"]);
    }

    #[test]
    fn test_transitions_from() {
        let def = WorkflowLoader::load_from_str(SAMPLE_WORKFLOW).unwrap();
        let transitions = def.transitions_from("INTAKE");
        assert_eq!(transitions.len(), 1);
        assert_eq!(transitions[0].to, "PROCESSING");
    }
}
