use super::state_machine::ValidatedStateMachine;
use super::{load_state_machine, ReducerResult};
use std::path::PathBuf;

const ENTITY_KYC_LIFECYCLE_YAML: &str =
    include_str!("../../config/sem_os_seeds/state_machines/entity_kyc_lifecycle.yaml");

const KYC_CASE_LIFECYCLE_YAML: &str =
    include_str!("../../config/sem_os_seeds/state_machines/kyc_case_lifecycle.yaml");

const UBO_EPISTEMIC_LIFECYCLE_YAML: &str =
    include_str!("../../config/sem_os_seeds/state_machines/ubo_epistemic_lifecycle.yaml");

/// Load one of the built-in reducer state machines used by the state verbs.
///
/// # Examples
/// ```rust
/// use ob_poc::state_reducer::load_builtin_state_machine;
///
/// let machine = load_builtin_state_machine("entity_kyc_lifecycle").unwrap();
/// assert_eq!(machine.name, "entity_kyc_lifecycle");
/// ```
pub fn load_builtin_state_machine(name: &str) -> ReducerResult<ValidatedStateMachine> {
    match name {
        "entity_kyc_lifecycle" => load_state_machine(ENTITY_KYC_LIFECYCLE_YAML),
        "kyc_case_lifecycle" => load_state_machine(KYC_CASE_LIFECYCLE_YAML),
        "ubo_epistemic_lifecycle" => load_state_machine(UBO_EPISTEMIC_LIFECYCLE_YAML),
        other => load_builtin_state_machine_from_disk(other),
    }
}

fn load_builtin_state_machine_from_disk(name: &str) -> ReducerResult<ValidatedStateMachine> {
    let path = PathBuf::from(env!("CARGO_MANIFEST_DIR"))
        .join("config/sem_os_seeds/state_machines")
        .join(format!("{name}.yaml"));

    let yaml = std::fs::read_to_string(&path).map_err(|_| {
        super::ReducerError::Validation(format!("unknown built-in state machine '{name}'"))
    })?;

    load_state_machine(&yaml)
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn loads_kyc_case_lifecycle_builtin() {
        let machine = load_builtin_state_machine("kyc_case_lifecycle").unwrap();
        assert_eq!(machine.name, "kyc_case_lifecycle");
        assert!(machine.states.iter().any(|state| state == "review"));
    }
}
