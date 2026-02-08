//! Workflow configuration — load from YAML, route lookup, task binding lookup.
//!
//! The `WorkflowConfig` provides the routing table that determines whether
//! a verb should be executed directly or dispatched through bpmn-lite.

use anyhow::{Context, Result};
use std::collections::HashMap;
use std::path::Path;

use super::types::{ExecutionRoute, TaskBinding, WorkflowBinding};

// ---------------------------------------------------------------------------
// WorkflowConfig
// ---------------------------------------------------------------------------

/// Root workflow configuration loaded from YAML.
///
/// Contains the routing table: verb FQN → `WorkflowBinding`.
/// All verbs not explicitly listed are assumed `Direct`.
#[derive(Debug, Clone, serde::Serialize, serde::Deserialize)]
pub struct WorkflowConfig {
    /// Workflow bindings keyed by verb FQN.
    pub workflows: Vec<WorkflowBinding>,
}

/// Indexed workflow configuration for fast lookups.
#[derive(Debug, Clone)]
pub struct WorkflowConfigIndex {
    /// verb_fqn → WorkflowBinding
    by_verb: HashMap<String, WorkflowBinding>,
    /// task_type → (verb_fqn, TaskBinding)
    by_task_type: HashMap<String, (String, TaskBinding)>,
    /// All task types collected from orchestrated workflows (for ActivateJobs).
    all_task_types: Vec<String>,
}

impl WorkflowConfigIndex {
    /// Build an indexed config from the raw YAML config.
    pub fn from_config(config: &WorkflowConfig) -> Self {
        let mut by_verb = HashMap::new();
        let mut by_task_type = HashMap::new();
        let mut all_task_types = Vec::new();

        for binding in &config.workflows {
            by_verb.insert(binding.verb_fqn.clone(), binding.clone());

            if binding.route == ExecutionRoute::Orchestrated {
                for tb in &binding.task_bindings {
                    by_task_type
                        .insert(tb.task_type.clone(), (binding.verb_fqn.clone(), tb.clone()));
                    all_task_types.push(tb.task_type.clone());
                }
            }
        }

        all_task_types.sort();
        all_task_types.dedup();

        Self {
            by_verb,
            by_task_type,
            all_task_types,
        }
    }

    /// Load from a YAML file and build the index.
    pub fn load_from_file(path: &Path) -> Result<Self> {
        let content =
            std::fs::read_to_string(path).with_context(|| format!("Reading {}", path.display()))?;
        let config: WorkflowConfig = serde_yaml::from_str(&content)
            .with_context(|| format!("Parsing {}", path.display()))?;
        Ok(Self::from_config(&config))
    }

    /// Determine the execution route for a verb.
    ///
    /// Returns `Direct` for any verb not explicitly listed.
    pub fn route_for_verb(&self, verb_fqn: &str) -> ExecutionRoute {
        self.by_verb
            .get(verb_fqn)
            .map(|b| b.route)
            .unwrap_or(ExecutionRoute::Direct)
    }

    /// Get the full workflow binding for a verb, if it exists.
    pub fn binding_for_verb(&self, verb_fqn: &str) -> Option<&WorkflowBinding> {
        self.by_verb.get(verb_fqn)
    }

    /// Look up a task binding by BPMN task type.
    ///
    /// Returns the workflow verb FQN and the task binding.
    pub fn binding_for_task_type(&self, task_type: &str) -> Option<(&str, &TaskBinding)> {
        self.by_task_type
            .get(task_type)
            .map(|(fqn, tb)| (fqn.as_str(), tb))
    }

    /// All task types across all orchestrated workflows.
    ///
    /// Used by the JobWorker to register interest in ActivateJobs.
    pub fn all_task_types(&self) -> &[String] {
        &self.all_task_types
    }

    /// All orchestrated workflow bindings.
    pub fn orchestrated_workflows(&self) -> Vec<&WorkflowBinding> {
        self.by_verb
            .values()
            .filter(|b| b.route == ExecutionRoute::Orchestrated)
            .collect()
    }

    /// Number of registered workflows.
    pub fn workflow_count(&self) -> usize {
        self.by_verb.len()
    }
}

// ---------------------------------------------------------------------------
// Tests
// ---------------------------------------------------------------------------

#[cfg(test)]
mod tests {
    use super::*;
    use crate::bpmn_integration::types::{ExecutionRoute, TaskBinding, WorkflowBinding};

    fn sample_config() -> WorkflowConfig {
        WorkflowConfig {
            workflows: vec![
                WorkflowBinding {
                    verb_fqn: "kyc.open-case".to_string(),
                    route: ExecutionRoute::Orchestrated,
                    process_key: Some("kyc-open-case".to_string()),
                    task_bindings: vec![
                        TaskBinding {
                            task_type: "create_case_record".to_string(),
                            verb_fqn: "kyc.create-case".to_string(),
                            timeout_ms: Some(30_000),
                            max_retries: 3,
                        },
                        TaskBinding {
                            task_type: "request_documents".to_string(),
                            verb_fqn: "document.solicit-set".to_string(),
                            timeout_ms: None,
                            max_retries: 3,
                        },
                    ],
                },
                WorkflowBinding {
                    verb_fqn: "cbu.create".to_string(),
                    route: ExecutionRoute::Direct,
                    process_key: None,
                    task_bindings: vec![],
                },
            ],
        }
    }

    #[test]
    fn test_route_for_orchestrated_verb() {
        let index = WorkflowConfigIndex::from_config(&sample_config());
        assert_eq!(
            index.route_for_verb("kyc.open-case"),
            ExecutionRoute::Orchestrated
        );
    }

    #[test]
    fn test_route_for_direct_verb() {
        let index = WorkflowConfigIndex::from_config(&sample_config());
        assert_eq!(index.route_for_verb("cbu.create"), ExecutionRoute::Direct);
    }

    #[test]
    fn test_route_for_unlisted_verb_defaults_to_direct() {
        let index = WorkflowConfigIndex::from_config(&sample_config());
        assert_eq!(
            index.route_for_verb("entity.create"),
            ExecutionRoute::Direct
        );
    }

    #[test]
    fn test_binding_for_task_type() {
        let index = WorkflowConfigIndex::from_config(&sample_config());

        let (workflow_fqn, tb) = index
            .binding_for_task_type("create_case_record")
            .expect("should find task binding");
        assert_eq!(workflow_fqn, "kyc.open-case");
        assert_eq!(tb.verb_fqn, "kyc.create-case");
        assert_eq!(tb.timeout_ms, Some(30_000));
    }

    #[test]
    fn test_binding_for_unknown_task_type() {
        let index = WorkflowConfigIndex::from_config(&sample_config());
        assert!(index.binding_for_task_type("unknown_task").is_none());
    }

    #[test]
    fn test_all_task_types() {
        let index = WorkflowConfigIndex::from_config(&sample_config());
        let types = index.all_task_types();
        assert_eq!(types.len(), 2);
        assert!(types.contains(&"create_case_record".to_string()));
        assert!(types.contains(&"request_documents".to_string()));
    }

    #[test]
    fn test_orchestrated_workflows() {
        let index = WorkflowConfigIndex::from_config(&sample_config());
        let orch = index.orchestrated_workflows();
        assert_eq!(orch.len(), 1);
        assert_eq!(orch[0].verb_fqn, "kyc.open-case");
    }

    #[test]
    fn test_yaml_roundtrip() {
        let config = sample_config();
        let yaml = serde_yaml::to_string(&config).unwrap();
        let parsed: WorkflowConfig = serde_yaml::from_str(&yaml).unwrap();
        assert_eq!(parsed.workflows.len(), 2);
        assert_eq!(parsed.workflows[0].verb_fqn, "kyc.open-case");
    }
}
