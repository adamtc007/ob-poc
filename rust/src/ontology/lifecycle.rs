//! Lifecycle state machine utilities.
//!
//! Provides functions for validating state transitions and querying valid states.
//!
//! ## Public API (used by production code)
//! - `is_valid_transition` - Check if a state transition is allowed
//! - `valid_next_states` - Get valid next states from current state
//! - `is_valid_state` - Check if a state is valid for a lifecycle
//!
//! ## Test Utilities (used by tests only, marked #[allow(dead_code)])
//! - `validate_required_state` - Validate entity is in required state
//! - `validate_transition` - Validate state transition with detailed result
//! - `is_terminal_state` - Check if state has no outbound transitions
//! - `terminal_states` - Get all terminal states for a lifecycle

use crate::ontology::types::EntityLifecycle;

/// Check if a transition from `from_state` to `to_state` is valid.
pub fn is_valid_transition(lifecycle: &EntityLifecycle, from_state: &str, to_state: &str) -> bool {
    lifecycle.is_valid_transition(from_state, to_state)
}

/// Get valid next states from a given state.
pub fn valid_next_states<'a>(lifecycle: &'a EntityLifecycle, from_state: &str) -> Vec<&'a str> {
    lifecycle.valid_next_states(from_state)
}

/// Check if a state is valid for this lifecycle.
pub fn is_valid_state(lifecycle: &EntityLifecycle, state: &str) -> bool {
    lifecycle.is_valid_state(state)
}

/// Result of a lifecycle validation check.
#[derive(Debug, Clone)]
#[allow(dead_code)]
pub struct LifecycleValidation {
    /// Whether the validation passed
    pub valid: bool,
    /// Current state of the entity
    pub current_state: String,
    /// Required states for the operation
    pub required_states: Vec<String>,
    /// Target state after operation (if applicable)
    pub target_state: Option<String>,
    /// Error message if validation failed
    pub error: Option<String>,
}

impl LifecycleValidation {
    /// Create a successful validation result.
    pub fn success(current_state: &str) -> Self {
        Self {
            valid: true,
            current_state: current_state.to_string(),
            required_states: vec![],
            target_state: None,
            error: None,
        }
    }

    /// Create a failed validation result.
    pub fn failure(
        current_state: &str,
        required_states: Vec<String>,
        target_state: Option<String>,
        error: &str,
    ) -> Self {
        Self {
            valid: false,
            current_state: current_state.to_string(),
            required_states,
            target_state,
            error: Some(error.to_string()),
        }
    }
}

/// Validate that an entity is in one of the required states.
#[allow(dead_code)]
pub fn validate_required_state(
    _lifecycle: &EntityLifecycle,
    current_state: &str,
    required_states: &[String],
) -> LifecycleValidation {
    if required_states.is_empty() {
        return LifecycleValidation::success(current_state);
    }

    if required_states.iter().any(|s| s == current_state) {
        LifecycleValidation::success(current_state)
    } else {
        LifecycleValidation::failure(
            current_state,
            required_states.to_vec(),
            None,
            &format!(
                "Entity is in state '{}' but must be in one of: {:?}",
                current_state, required_states
            ),
        )
    }
}

/// Validate that a state transition is allowed.
#[allow(dead_code)]
pub fn validate_transition(
    lifecycle: &EntityLifecycle,
    current_state: &str,
    target_state: &str,
) -> LifecycleValidation {
    if lifecycle.is_valid_transition(current_state, target_state) {
        LifecycleValidation {
            valid: true,
            current_state: current_state.to_string(),
            required_states: vec![current_state.to_string()],
            target_state: Some(target_state.to_string()),
            error: None,
        }
    } else {
        let valid_targets = lifecycle.valid_next_states(current_state);
        LifecycleValidation::failure(
            current_state,
            vec![current_state.to_string()],
            Some(target_state.to_string()),
            &format!(
                "Cannot transition from '{}' to '{}'. Valid transitions: {:?}",
                current_state, target_state, valid_targets
            ),
        )
    }
}

/// Check if an entity is in a terminal state (no valid transitions out).
#[allow(dead_code)]
pub fn is_terminal_state(lifecycle: &EntityLifecycle, state: &str) -> bool {
    lifecycle.valid_next_states(state).is_empty()
}

/// Get all terminal states for a lifecycle.
#[allow(dead_code)]
pub fn terminal_states(lifecycle: &EntityLifecycle) -> Vec<&str> {
    lifecycle
        .states
        .iter()
        .filter(|s| is_terminal_state(lifecycle, s))
        .map(|s| s.as_str())
        .collect()
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::ontology::types::StateTransition;

    fn test_lifecycle() -> EntityLifecycle {
        EntityLifecycle {
            status_column: "status".to_string(),
            states: vec![
                "DRAFT".to_string(),
                "ACTIVE".to_string(),
                "TERMINATED".to_string(),
            ],
            transitions: vec![
                StateTransition {
                    from: "DRAFT".to_string(),
                    to: vec!["ACTIVE".to_string()],
                },
                StateTransition {
                    from: "ACTIVE".to_string(),
                    to: vec!["TERMINATED".to_string()],
                },
                StateTransition {
                    from: "TERMINATED".to_string(),
                    to: vec![],
                },
            ],
            initial_state: "DRAFT".to_string(),
        }
    }

    #[test]
    fn test_valid_transition() {
        let lc = test_lifecycle();
        assert!(is_valid_transition(&lc, "DRAFT", "ACTIVE"));
        assert!(!is_valid_transition(&lc, "ACTIVE", "DRAFT"));
    }

    #[test]
    fn test_terminal_state() {
        let lc = test_lifecycle();
        assert!(is_terminal_state(&lc, "TERMINATED"));
        assert!(!is_terminal_state(&lc, "DRAFT"));
    }

    #[test]
    fn test_validate_required_state() {
        let lc = test_lifecycle();
        let result =
            validate_required_state(&lc, "ACTIVE", &["ACTIVE".to_string(), "DRAFT".to_string()]);
        assert!(result.valid);

        let result = validate_required_state(&lc, "TERMINATED", &["ACTIVE".to_string()]);
        assert!(!result.valid);
    }
}
