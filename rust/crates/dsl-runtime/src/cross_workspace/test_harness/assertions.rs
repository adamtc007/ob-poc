//! Per-step assertion checks: compare expected vs actual outcomes.

use crate::cross_workspace::derived_state::DerivedStateValue;
use crate::cross_workspace::gate_checker::GateViolation;
use crate::cross_workspace::hierarchy_cascade::CascadeAction;

use super::scenario::{
    ExpectedCascadeAction, ExpectedCondition, ExpectedDerivedValue, ExpectedViolation,
};

/// One assertion failure. The runner aggregates these into a per-step
/// failure list.
#[derive(Debug, Clone)]
pub struct AssertionFailure {
    pub field: String,
    pub expected: String,
    pub actual: String,
}

impl std::fmt::Display for AssertionFailure {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        write!(
            f,
            "  {}: expected {}, got {}",
            self.field, self.expected, self.actual
        )
    }
}

// ---------------------------------------------------------------------------
// Mode A — gate violations
// ---------------------------------------------------------------------------

/// Compare the actual GateViolation list against the expected list,
/// order-insensitive (matched by constraint_id). Any expected violation
/// not present in actual = failure; any actual violation not present in
/// expected = failure (unless the expected list is the explicit empty list,
/// which means "exactly zero violations").
pub fn check_violations(
    expected: &[ExpectedViolation],
    actual: &[GateViolation],
) -> Vec<AssertionFailure> {
    let mut failures = Vec::new();

    // Detect missing expected.
    for ev in expected {
        match actual.iter().find(|v| v.constraint_id == ev.constraint_id) {
            None => failures.push(AssertionFailure {
                field: format!("violation[{}]", ev.constraint_id),
                expected: "present".into(),
                actual: "absent".into(),
            }),
            Some(av) => {
                if let Some(ref expected_severity) = ev.severity {
                    if &av.severity != expected_severity {
                        failures.push(AssertionFailure {
                            field: format!("violation[{}].severity", ev.constraint_id),
                            expected: expected_severity.clone(),
                            actual: av.severity.clone(),
                        });
                    }
                }
                if let Some(ref expected_required) = ev.required_state {
                    if &av.required_state != expected_required {
                        failures.push(AssertionFailure {
                            field: format!("violation[{}].required_state", ev.constraint_id),
                            expected: format!("{:?}", expected_required),
                            actual: format!("{:?}", av.required_state),
                        });
                    }
                }
                if let Some(ref expected_actual) = ev.actual_state {
                    if &av.actual_state != expected_actual {
                        failures.push(AssertionFailure {
                            field: format!("violation[{}].actual_state", ev.constraint_id),
                            expected: format!("{:?}", expected_actual),
                            actual: format!("{:?}", av.actual_state),
                        });
                    }
                }
            }
        }
    }

    // Detect unexpected actual.
    for av in actual {
        if !expected.iter().any(|ev| ev.constraint_id == av.constraint_id) {
            failures.push(AssertionFailure {
                field: format!("violation[{}]", av.constraint_id),
                expected: "absent".into(),
                actual: format!("present (severity={}, message={})", av.severity, av.message),
            });
        }
    }

    failures
}

// ---------------------------------------------------------------------------
// Mode B — derived state value
// ---------------------------------------------------------------------------

pub fn check_derived_value(
    expected: &ExpectedDerivedValue,
    actual: &DerivedStateValue,
) -> Vec<AssertionFailure> {
    let mut failures = Vec::new();

    if expected.satisfied != actual.satisfied {
        failures.push(AssertionFailure {
            field: "derived.satisfied".into(),
            expected: expected.satisfied.to_string(),
            actual: actual.satisfied.to_string(),
        });
    }

    if let Some(ref expected_conditions) = expected.conditions {
        if expected_conditions.len() != actual.conditions.len() {
            failures.push(AssertionFailure {
                field: "derived.conditions.len".into(),
                expected: expected_conditions.len().to_string(),
                actual: actual.conditions.len().to_string(),
            });
        } else {
            for (i, (ec, ac)) in expected_conditions
                .iter()
                .zip(actual.conditions.iter())
                .enumerate()
            {
                check_condition(i, ec, ac, &mut failures);
            }
        }
    }

    failures
}

fn check_condition(
    index: usize,
    expected: &ExpectedCondition,
    actual: &crate::cross_workspace::derived_state::ConditionResult,
    failures: &mut Vec<AssertionFailure>,
) {
    if expected.satisfied != actual.satisfied {
        failures.push(AssertionFailure {
            field: format!("derived.conditions[{}].satisfied", index),
            expected: expected.satisfied.to_string(),
            actual: actual.satisfied.to_string(),
        });
    }
    if let Some(ref needle) = expected.description_contains {
        if !actual.description.contains(needle) {
            failures.push(AssertionFailure {
                field: format!("derived.conditions[{}].description", index),
                expected: format!("contains '{}'", needle),
                actual: actual.description.clone(),
            });
        }
    }
}

// ---------------------------------------------------------------------------
// Mode C — cascade actions
// ---------------------------------------------------------------------------

/// Compare cascade actions, order-insensitive. The `child_entity` in
/// expected is a UUID string already resolved by the runner.
pub fn check_cascade_actions(
    expected: &[ExpectedCascadeAction],
    actual: &[CascadeAction],
    resolve_alias: impl Fn(&str) -> Option<uuid::Uuid>,
) -> Vec<AssertionFailure> {
    let mut failures = Vec::new();

    for ea in expected {
        let expected_child_id = match resolve_alias(&ea.child_entity) {
            Some(id) => id,
            None => {
                failures.push(AssertionFailure {
                    field: "cascade.child_entity".into(),
                    expected: ea.child_entity.clone(),
                    actual: "<unresolved alias>".into(),
                });
                continue;
            }
        };
        let matched = actual.iter().find(|a| {
            a.child_workspace == ea.child_workspace
                && a.child_slot == ea.child_slot
                && a.child_entity_id == expected_child_id
        });
        match matched {
            None => failures.push(AssertionFailure {
                field: format!(
                    "cascade[{}.{}.{}]",
                    ea.child_workspace, ea.child_slot, ea.child_entity
                ),
                expected: "present".into(),
                actual: "absent".into(),
            }),
            Some(a) => {
                if a.target_state != ea.target_state {
                    failures.push(AssertionFailure {
                        field: format!(
                            "cascade[{}.{}.{}].target_state",
                            ea.child_workspace, ea.child_slot, ea.child_entity
                        ),
                        expected: ea.target_state.clone(),
                        actual: a.target_state.clone(),
                    });
                }
                if let Some(ref expected_severity) = ea.severity {
                    if &a.severity != expected_severity {
                        failures.push(AssertionFailure {
                            field: format!(
                                "cascade[{}.{}.{}].severity",
                                ea.child_workspace, ea.child_slot, ea.child_entity
                            ),
                            expected: expected_severity.clone(),
                            actual: a.severity.clone(),
                        });
                    }
                }
            }
        }
    }

    // Detect unexpected actual cascade actions.
    for aa in actual {
        let matched = expected.iter().any(|ea| {
            resolve_alias(&ea.child_entity)
                .map(|id| {
                    aa.child_workspace == ea.child_workspace
                        && aa.child_slot == ea.child_slot
                        && aa.child_entity_id == id
                })
                .unwrap_or(false)
        });
        if !matched {
            failures.push(AssertionFailure {
                field: format!(
                    "cascade[{}.{}.{}]",
                    aa.child_workspace, aa.child_slot, aa.child_entity_id
                ),
                expected: "absent".into(),
                actual: format!("present (target_state={})", aa.target_state),
            });
        }
    }

    failures
}
