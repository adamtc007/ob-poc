//! Agentic Scenario Test Harness
//!
//! Deterministic, repeatable end-to-end tests for the single orchestrator pipeline.
//! Asserts on structured fields only (outcome kind, chosen verb, SemReg mode,
//! run_sheet deltas, trace flags) — never on LLM prose.
//!
//! # Modes
//! - **Stub** (CI default): pre-canned PipelineResults, no DB/LLM/network
//! - **Live** (nightly): real DB + Candle, relaxed assertions

pub mod assertions;
pub mod runner;
pub mod stub;

use serde::{Deserialize, Serialize};
use std::collections::HashMap;
use std::path::Path;

/// Top-level scenario suite loaded from YAML.
#[derive(Debug, Clone, Deserialize, Serialize)]
pub struct ScenarioSuite {
    pub name: String,
    #[serde(default)]
    pub description: Option<String>,
    pub suite_id: String,
    #[serde(default)]
    pub mode_expectations: ModeExpectations,
    #[serde(default)]
    pub session_seed: SessionSeed,
    pub scenarios: Vec<Scenario>,
}

/// A single named scenario within a suite.
#[derive(Debug, Clone, Deserialize, Serialize)]
pub struct Scenario {
    pub name: String,
    #[serde(default)]
    pub description: Option<String>,
    #[serde(default)]
    pub tags: Vec<String>,
    pub steps: Vec<ScenarioStep>,
}

/// One step in a multi-turn scenario.
#[derive(Debug, Clone, Deserialize, Serialize)]
pub struct ScenarioStep {
    pub user: String,
    #[serde(default)]
    pub expect: StepExpectation,
    #[serde(default)]
    pub on_outcome: Option<HashMap<String, OnOutcomeAction>>,
}

/// Partial assertions on a step's outcome. Only specified fields are checked.
#[derive(Debug, Clone, Default, Deserialize, Serialize)]
pub struct StepExpectation {
    pub outcome: Option<String>,
    pub chosen_verb: Option<String>,
    pub forced_verb: Option<String>,
    pub semreg_mode: Option<String>,
    pub selection_source: Option<String>,
    #[serde(default)]
    pub selection_source_in: Option<Vec<String>>,
    pub run_sheet_delta: Option<i32>,
    pub runnable_count: Option<usize>,
    pub sem_reg_denied_all: Option<bool>,
    pub semreg_unavailable: Option<bool>,
    #[serde(default)]
    pub trace: Option<TraceExpectation>,
    pub bypass_used: Option<String>,
    pub dsl_non_empty: Option<bool>,
}

/// Expected trace field values (partial).
#[derive(Debug, Clone, Default, Deserialize, Serialize)]
pub struct TraceExpectation {
    pub macro_semreg_checked: Option<bool>,
    #[serde(default)]
    pub macro_denied_verbs_non_empty: Option<bool>,
    pub telemetry_persisted: Option<bool>,
    pub dominant_entity_kind: Option<String>,
    pub entity_kind_filtered: Option<bool>,
}

/// Action to take when an interactive outcome occurs.
#[derive(Debug, Clone, Deserialize, Serialize)]
#[serde(untagged)]
pub enum OnOutcomeAction {
    ChooseIndex { choose_index: usize },
    ChooseVerbFqn { choose_verb_fqn: String },
    Reply { reply: String },
}

/// Policy expectations for the suite.
#[derive(Debug, Clone, Deserialize, Serialize)]
pub struct ModeExpectations {
    #[serde(default = "default_true")]
    pub strict_semreg: bool,
    #[serde(default = "default_true")]
    pub strict_single_pipeline: bool,
    #[serde(default)]
    pub allow_direct_dsl: bool,
    #[serde(default)]
    pub allow_raw_execute: bool,
}

impl Default for ModeExpectations {
    fn default() -> Self {
        Self {
            strict_semreg: true,
            strict_single_pipeline: true,
            allow_direct_dsl: false,
            allow_raw_execute: false,
        }
    }
}

fn default_true() -> bool {
    true
}

/// Session seed for scenario initialization.
#[derive(Debug, Clone, Default, Deserialize, Serialize)]
pub struct SessionSeed {
    #[serde(default)]
    pub scope: Option<String>,
    #[serde(default)]
    pub dominant_entity: Option<String>,
    #[serde(default)]
    pub actor: ActorSeed,
}

/// Actor configuration for scenario.
#[derive(Debug, Clone, Deserialize, Serialize)]
pub struct ActorSeed {
    #[serde(default = "default_actor_id")]
    pub actor_id: String,
    #[serde(default = "default_roles")]
    pub roles: Vec<String>,
    #[serde(default)]
    pub clearance: Option<String>,
}

impl Default for ActorSeed {
    fn default() -> Self {
        Self {
            actor_id: "test.user".into(),
            roles: vec!["viewer".into()],
            clearance: None,
        }
    }
}

fn default_actor_id() -> String {
    "test.user".into()
}
fn default_roles() -> Vec<String> {
    vec!["viewer".into()]
}

/// Load a single suite from a YAML file.
pub fn load_suite(path: &Path) -> anyhow::Result<ScenarioSuite> {
    let content = std::fs::read_to_string(path)?;
    let suite: ScenarioSuite = serde_yaml::from_str(&content)?;
    Ok(suite)
}

/// Load all suites from a directory (*.yaml files).
pub fn load_all_suites(dir: &Path) -> anyhow::Result<Vec<ScenarioSuite>> {
    let mut suites = Vec::new();
    for entry in std::fs::read_dir(dir)? {
        let entry = entry?;
        let path = entry.path();
        if path
            .extension()
            .map(|e| e == "yaml" || e == "yml")
            .unwrap_or(false)
        {
            match load_suite(&path) {
                Ok(suite) => suites.push(suite),
                Err(e) => {
                    tracing::warn!(path = %path.display(), error = %e, "Failed to load suite");
                }
            }
        }
    }
    suites.sort_by(|a, b| a.suite_id.cmp(&b.suite_id));
    Ok(suites)
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_deserialize_minimal_suite() {
        let yaml = r#"
name: "Test Suite"
suite_id: "test"
scenarios:
  - name: "basic"
    steps:
      - user: "hello"
        expect:
          outcome: "NoMatch"
"#;
        let suite: ScenarioSuite = serde_yaml::from_str(yaml).unwrap();
        assert_eq!(suite.name, "Test Suite");
        assert_eq!(suite.scenarios.len(), 1);
        assert_eq!(suite.scenarios[0].steps[0].user, "hello");
        assert_eq!(
            suite.scenarios[0].steps[0].expect.outcome.as_deref(),
            Some("NoMatch")
        );
    }

    #[test]
    fn test_deserialize_full_suite() {
        let yaml = r#"
name: "Full Suite"
suite_id: "full"
mode_expectations:
  strict_semreg: false
  allow_direct_dsl: true
session_seed:
  scope: "Allianz GI"
  actor:
    actor_id: "admin.user"
    roles: ["operator"]
scenarios:
  - name: "direct dsl"
    tags: ["bypass", "dsl"]
    steps:
      - user: "dsl:(cbu.create :name \"Acme\")"
        expect:
          outcome: "Ready"
          bypass_used: "direct_dsl"
          dsl_non_empty: true
        on_outcome:
          ClarifyVerb:
            choose_index: 1
"#;
        let suite: ScenarioSuite = serde_yaml::from_str(yaml).unwrap();
        assert!(!suite.mode_expectations.strict_semreg);
        assert!(suite.mode_expectations.allow_direct_dsl);
        assert_eq!(suite.session_seed.actor.roles, vec!["operator"]);
        let step = &suite.scenarios[0].steps[0];
        assert_eq!(step.expect.bypass_used.as_deref(), Some("direct_dsl"));
        assert!(step.on_outcome.is_some());
    }

    #[test]
    fn test_defaults_are_strict() {
        let yaml = r#"
name: "Defaults"
suite_id: "defaults"
scenarios: []
"#;
        let suite: ScenarioSuite = serde_yaml::from_str(yaml).unwrap();
        assert!(suite.mode_expectations.strict_semreg);
        assert!(suite.mode_expectations.strict_single_pipeline);
        assert!(!suite.mode_expectations.allow_direct_dsl);
        assert!(!suite.mode_expectations.allow_raw_execute);
    }

    #[test]
    fn test_step_expectation_defaults() {
        let exp = StepExpectation::default();
        assert!(exp.outcome.is_none());
        assert!(exp.chosen_verb.is_none());
        assert!(exp.run_sheet_delta.is_none());
    }
}
