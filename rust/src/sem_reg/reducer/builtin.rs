use super::state_machine::ValidatedStateMachine;
use super::{load_state_machine, ReducerResult};

const ENTITY_KYC_LIFECYCLE_YAML: &str =
    include_str!("../../../config/sem_os_seeds/state_machines/entity_kyc_lifecycle.yaml");

const UBO_EPISTEMIC_LIFECYCLE_YAML: &str =
    include_str!("../../../config/sem_os_seeds/state_machines/ubo_epistemic_lifecycle.yaml");

/// Load one of the built-in reducer state machines used by the state verbs.
///
/// # Examples
/// ```rust
/// use ob_poc::sem_reg::reducer::load_builtin_state_machine;
///
/// let machine = load_builtin_state_machine("entity_kyc_lifecycle").unwrap();
/// assert_eq!(machine.name, "entity_kyc_lifecycle");
/// ```
pub fn load_builtin_state_machine(name: &str) -> ReducerResult<ValidatedStateMachine> {
    match name {
        "entity_kyc_lifecycle" => load_state_machine(ENTITY_KYC_LIFECYCLE_YAML),
        "ubo_epistemic_lifecycle" => load_state_machine(UBO_EPISTEMIC_LIFECYCLE_YAML),
        other => Err(super::ReducerError::Validation(format!(
            "unknown built-in state machine '{other}'"
        ))),
    }
}
