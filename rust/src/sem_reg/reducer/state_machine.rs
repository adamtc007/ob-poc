use std::collections::HashMap;

use serde::{Deserialize, Serialize};
use sha2::{Digest, Sha256};

use super::ast::ConditionBody;
use super::error::ReducerResult;
use super::validate::validate_state_machine;

/// State machine definition loaded from YAML.
#[derive(Debug, Clone, Deserialize, Serialize)]
pub struct StateMachineDefinition {
    pub state_machine: String,
    pub description: Option<String>,
    pub states: Vec<String>,
    pub initial: String,
    pub transitions: Vec<TransitionDef>,
    pub reducer: ReducerDef,
}

/// Transition definition.
#[derive(Debug, Clone, Deserialize, Serialize)]
pub struct TransitionDef {
    pub from: String,
    pub to: String,
    pub verbs: Vec<String>,
}

/// Reducer section of the state machine.
#[derive(Debug, Clone, Deserialize, Serialize)]
pub struct ReducerDef {
    pub overlay_sources: HashMap<String, OverlaySourceDef>,
    pub conditions: HashMap<String, ConditionDef>,
    pub rules: Vec<RuleDef>,
}

/// Overlay source definition used for validation.
#[derive(Debug, Clone, Deserialize, Serialize)]
pub struct OverlaySourceDef {
    pub table: String,
    pub join: String,
    pub provides: Vec<String>,
    pub cardinality: Option<String>,
}

/// Condition definition.
#[derive(Debug, Clone, Deserialize, Serialize)]
pub struct ConditionDef {
    pub expr: String,
    pub description: Option<String>,
    #[serde(default)]
    pub parameterized: bool,
}

/// Reducer rule definition.
#[derive(Debug, Clone, Deserialize, Serialize, PartialEq, Eq)]
pub struct RuleDef {
    pub state: String,
    #[serde(default)]
    pub requires: Vec<String>,
    #[serde(default)]
    pub excludes: Vec<String>,
    pub consistency_check: Option<ConsistencyCheckDef>,
}

/// Consistency warning definition.
#[derive(Debug, Clone, Deserialize, Serialize, PartialEq, Eq)]
pub struct ConsistencyCheckDef {
    pub warn_unless: String,
    pub warning: String,
}

/// State machine after parse + validation.
#[derive(Debug, Clone)]
pub struct ValidatedStateMachine {
    pub name: String,
    pub states: Vec<String>,
    pub initial: String,
    pub transitions: Vec<TransitionDef>,
    pub conditions: HashMap<String, ConditionBody>,
    pub eval_order: Vec<String>,
    pub rules: Vec<RuleDef>,
    pub overlay_sources: HashMap<String, OverlaySourceDef>,
    pub reducer_revision: String,
}

/// Load and validate a reducer state machine from YAML.
///
/// # Examples
/// ```rust
/// use ob_poc::sem_reg::reducer::load_state_machine;
///
/// let yaml = r#"
/// state_machine: demo
/// states: [empty]
/// initial: empty
/// transitions: []
/// reducer:
///   overlay_sources: {}
///   conditions: {}
///   rules:
///     - state: empty
///       requires: []
/// "#;
/// let machine = load_state_machine(yaml).unwrap();
/// assert_eq!(machine.name, "demo");
/// ```
pub fn load_state_machine(yaml: &str) -> ReducerResult<ValidatedStateMachine> {
    let definition: StateMachineDefinition =
        serde_yaml::from_str(yaml).map_err(|err| super::error::ReducerError::Other(err.into()))?;
    let mut validated = validate_state_machine(&definition)?;
    validated.reducer_revision = compute_reducer_revision(yaml);
    Ok(validated)
}

/// Compute the reducer revision hash from YAML content.
///
/// # Examples
/// ```rust
/// use ob_poc::sem_reg::reducer::compute_reducer_revision;
///
/// assert_eq!(compute_reducer_revision("demo").len(), 16);
/// ```
pub fn compute_reducer_revision(state_machine_yaml: &str) -> String {
    let hash = Sha256::digest(state_machine_yaml.as_bytes());
    hex::encode(&hash[..8])
}
