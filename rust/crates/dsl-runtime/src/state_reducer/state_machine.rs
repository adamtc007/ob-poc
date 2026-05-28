//! State machine definition + leaf type re-exports.
//!
//! ## Schema-authority alignment (walk-by audit, 2026-05-13)
//!
//! The 6 leaf types `TransitionDef`, `ReducerDef`,
//! `OverlaySourceDef`, `ConditionDef`, `RuleDef`,
//! `ConsistencyCheckDef` are re-exported from
//! `sem_os_core::state_machine_def` — they had byte-identical
//! shapes and derives in the dsl-runtime-local definitions
//! before, surfaced as `xtask audit` bucket-1 entries that the
//! walk-by check identified as bucket-3-class drift.
//!
//! `StateMachineDefinition` (this file) stays local because it
//! genuinely differs from `sem_os_ontology::state_machine_def::
//! StateMachineDefBody` — the body type carries `fqn: String`
//! and an `Option<ReducerDef>` (registry-snapshot shape), while
//! the runtime engine here loads YAML where `reducer` is always
//! present and there is no fqn (the YAML filename is the
//! identifier).
//!
//! `ValidatedStateMachine` (further down) stays local — it is
//! the post-validation runtime artefact, not a schema-authority
//! type.

use std::collections::HashMap;

use serde::{Deserialize, Serialize};
use sha2::{Digest, Sha256};

use super::ast::ConditionBody;
use super::error::ReducerResult;
use super::validate::validate_state_machine;

pub use sem_os_ontology::state_machine_def::{
    OverlaySourceDef, ReducerDef, RuleDef, TransitionDef,
};

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
/// use dsl_runtime::load_state_machine;
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
/// use dsl_runtime::compute_reducer_revision;
///
/// assert_eq!(compute_reducer_revision("demo").len(), 16);
/// ```
pub(crate) fn compute_reducer_revision(state_machine_yaml: &str) -> String {
    let hash = Sha256::digest(state_machine_yaml.as_bytes());
    hex::encode(&hash[..8])
}
