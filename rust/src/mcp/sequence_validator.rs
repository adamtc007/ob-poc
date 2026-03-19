//! Sequence Validator — Three-Valued Prereq Validation for Macro Sequences
//!
//! Validates that a sequence of macros (from a ScenarioIndex `macro_sequence` route)
//! forms a coherent pipeline where each macro's prerequisites are satisfied by
//! either the current session state or by earlier macros in the sequence.
//!
//! ## Three-Valued Result
//!
//! Each macro in the sequence gets one of:
//! - **Pass**: All prerequisites are met (by session state or prior macros)
//! - **Fail**: A prerequisite cannot be met — lists what's missing and what could fix it
//! - **Deferred**: Prerequisites depend on runtime arguments that can't be checked statically
//!
//! ## Usage
//!
//! Called by the ScenarioIndex when a `macro_sequence` route is selected, to verify
//! the sequence is feasible before presenting it to the user.

use std::collections::{HashMap, HashSet};

use crate::dsl_v2::macros::{MacroPrereq, MacroRegistry, MacroSchema};

// ─── Core Types ────────────────────────────────────────────────────────────

/// Result of checking a single prerequisite.
#[derive(Debug, Clone)]
pub enum PrereqCheck {
    /// Prerequisite is satisfied (by current state or earlier macro in sequence).
    Pass,

    /// Prerequisite cannot be satisfied statically.
    Fail {
        /// Human-readable description of the missing prerequisite.
        missing: String,
        /// Macro FQNs from the registry that could satisfy this prerequisite.
        satisfied_by: Vec<String>,
    },

    /// Prerequisite depends on runtime state that can't be evaluated statically.
    /// This is NOT a failure — it means the system should accept the sequence
    /// and check the prereq at execution time.
    Deferred {
        /// Description of what runtime information is needed.
        reason: String,
    },
}

/// Validation result for a single macro in the sequence.
#[derive(Debug, Clone)]
pub struct MacroValidation {
    /// The macro FQN being validated.
    pub macro_fqn: String,
    /// Position in the sequence (0-indexed).
    pub position: usize,
    /// Overall check result for this macro.
    pub check: PrereqCheck,
    /// Individual prereq results (one per prereq on the macro).
    pub prereq_details: Vec<PrereqDetail>,
}

/// Detail for a single prerequisite check.
#[derive(Debug, Clone)]
pub struct PrereqDetail {
    /// Human-readable description of the prerequisite.
    pub description: String,
    /// Check result.
    pub check: PrereqCheck,
}

/// Result of validating an entire macro sequence.
#[derive(Debug, Clone)]
pub struct SequenceValidationResult {
    /// Per-macro validation results, in sequence order.
    pub validations: Vec<MacroValidation>,
    /// Whether the overall sequence is feasible.
    pub feasible: bool,
    /// Number of macros that passed.
    pub pass_count: usize,
    /// Number of macros that failed.
    pub fail_count: usize,
    /// Number of macros with deferred checks.
    pub deferred_count: usize,
}

// ─── Simulated State ───────────────────────────────────────────────────────

/// Tracks accumulated state during sequence validation.
/// Simulates what would happen if each macro in the sequence executed successfully.
struct SimulatedState {
    /// State flags that are set (key → true).
    state_flags: HashSet<String>,
    /// Verbs (macro FQNs) that have been "completed" by earlier sequence entries.
    completed_verbs: HashSet<String>,
}

impl SimulatedState {
    /// Create from current session state.
    fn new(current_state: &HashSet<String>, completed_verbs: &HashSet<String>) -> Self {
        Self {
            state_flags: current_state.clone(),
            completed_verbs: completed_verbs.clone(),
        }
    }

    /// Check if a state flag is set.
    fn has_flag(&self, key: &str) -> bool {
        self.state_flags.contains(key)
    }

    /// Check if a verb has been completed.
    fn has_completed(&self, verb: &str) -> bool {
        self.completed_verbs.contains(verb)
    }

    /// Apply the effects of a macro executing successfully.
    fn apply_macro(&mut self, fqn: &str, schema: &MacroSchema) {
        // Mark the macro itself as completed
        self.completed_verbs.insert(fqn.to_string());

        // Apply sets_state
        for set_state in &schema.sets_state {
            if set_state.value.as_bool().unwrap_or(false) {
                self.state_flags.insert(set_state.key.clone());
            }
        }
    }
}

// ─── Reverse Index ─────────────────────────────────────────────────────────

/// Maps state keys and verb completions to macros that produce them.
/// Used to generate `satisfied_by` suggestions on failures.
struct ProducerIndex {
    /// state_key → [macro FQNs that set this state]
    state_producers: HashMap<String, Vec<String>>,
    // verb_fqn → macro FQN (trivially, executing a macro "produces" its own completion)
    // But more useful: which macros unlock other macros via sets_state
}

impl ProducerIndex {
    fn build(registry: &MacroRegistry) -> Self {
        let mut state_producers: HashMap<String, Vec<String>> = HashMap::new();

        for (fqn, schema) in registry.all() {
            for set_state in &schema.sets_state {
                if set_state.value.as_bool().unwrap_or(false) {
                    state_producers
                        .entry(set_state.key.clone())
                        .or_default()
                        .push(fqn.clone());
                }
            }
        }

        Self { state_producers }
    }

    /// Find macros that could satisfy a missing state flag.
    fn find_state_producers(&self, key: &str) -> Vec<String> {
        self.state_producers.get(key).cloned().unwrap_or_default()
    }
}

// ─── Validation Logic ──────────────────────────────────────────────────────

/// Validate a macro sequence against the current session state.
///
/// Walks the sequence in order, checking each macro's prerequisites against
/// the accumulated simulated state. After checking, applies the macro's
/// effects (sets_state, completion) to the simulated state for subsequent macros.
///
/// # Arguments
///
/// * `macros` - Ordered list of macro FQNs in the sequence.
/// * `registry` - The macro registry (for looking up prereqs and sets_state).
/// * `current_state_flags` - State flags currently set in the session.
/// * `completed_verbs` - Verbs already completed in the session.
///
/// # Returns
///
/// A `SequenceValidationResult` with per-macro validation and overall feasibility.
pub fn validate_macro_sequence(
    macros: &[String],
    registry: &MacroRegistry,
    current_state_flags: &HashSet<String>,
    completed_verbs: &HashSet<String>,
) -> SequenceValidationResult {
    let producer_index = ProducerIndex::build(registry);
    let mut sim_state = SimulatedState::new(current_state_flags, completed_verbs);
    let mut validations = Vec::with_capacity(macros.len());

    for (position, fqn) in macros.iter().enumerate() {
        let validation = match registry.get(fqn) {
            Some(schema) => {
                let (check, details) =
                    validate_single_macro(fqn, schema, &sim_state, &producer_index);

                // If pass or deferred, apply effects optimistically
                if matches!(check, PrereqCheck::Pass | PrereqCheck::Deferred { .. }) {
                    sim_state.apply_macro(fqn, schema);
                }

                MacroValidation {
                    macro_fqn: fqn.clone(),
                    position,
                    check,
                    prereq_details: details,
                }
            }
            None => MacroValidation {
                macro_fqn: fqn.clone(),
                position,
                check: PrereqCheck::Fail {
                    missing: format!("Macro '{}' not found in registry", fqn),
                    satisfied_by: vec![],
                },
                prereq_details: vec![],
            },
        };

        validations.push(validation);
    }

    // Compute summary
    let pass_count = validations
        .iter()
        .filter(|v| matches!(v.check, PrereqCheck::Pass))
        .count();
    let fail_count = validations
        .iter()
        .filter(|v| matches!(v.check, PrereqCheck::Fail { .. }))
        .count();
    let deferred_count = validations
        .iter()
        .filter(|v| matches!(v.check, PrereqCheck::Deferred { .. }))
        .count();

    SequenceValidationResult {
        validations,
        feasible: fail_count == 0,
        pass_count,
        fail_count,
        deferred_count,
    }
}

/// Validate a single macro's prerequisites against the simulated state.
fn validate_single_macro(
    _fqn: &str,
    schema: &MacroSchema,
    sim_state: &SimulatedState,
    producer_index: &ProducerIndex,
) -> (PrereqCheck, Vec<PrereqDetail>) {
    if schema.prereqs.is_empty() {
        return (PrereqCheck::Pass, vec![]);
    }

    let mut details = Vec::new();
    let mut has_fail = false;
    let mut has_deferred = false;

    for prereq in &schema.prereqs {
        let detail = check_prereq(prereq, sim_state, producer_index);
        match &detail.check {
            PrereqCheck::Fail { .. } => has_fail = true,
            PrereqCheck::Deferred { .. } => has_deferred = true,
            PrereqCheck::Pass => {}
        }
        details.push(detail);
    }

    let overall = if has_fail {
        // Find the first fail for the overall message
        let first_fail = details
            .iter()
            .find(|d| matches!(d.check, PrereqCheck::Fail { .. }))
            .unwrap();
        first_fail.check.clone()
    } else if has_deferred {
        PrereqCheck::Deferred {
            reason: "Some prerequisites require runtime evaluation".to_string(),
        }
    } else {
        PrereqCheck::Pass
    };

    (overall, details)
}

/// Check a single prerequisite against the simulated state.
fn check_prereq(
    prereq: &MacroPrereq,
    sim_state: &SimulatedState,
    producer_index: &ProducerIndex,
) -> PrereqDetail {
    match prereq {
        MacroPrereq::StateExists { key } => {
            if sim_state.has_flag(key) {
                PrereqDetail {
                    description: format!("State '{}' exists", key),
                    check: PrereqCheck::Pass,
                }
            } else {
                PrereqDetail {
                    description: format!("State '{}' must exist", key),
                    check: PrereqCheck::Fail {
                        missing: format!("State '{}' not set", key),
                        satisfied_by: producer_index.find_state_producers(key),
                    },
                }
            }
        }

        MacroPrereq::VerbCompleted { verb } => {
            if sim_state.has_completed(verb) {
                PrereqDetail {
                    description: format!("Verb '{}' completed", verb),
                    check: PrereqCheck::Pass,
                }
            } else {
                PrereqDetail {
                    description: format!("Verb '{}' must be completed", verb),
                    check: PrereqCheck::Fail {
                        missing: format!("Verb '{}' not completed", verb),
                        satisfied_by: vec![verb.clone()],
                    },
                }
            }
        }

        MacroPrereq::AnyOf { conditions } => {
            // Pass if ANY sub-condition passes
            let mut any_pass = false;
            let mut all_missing = Vec::new();
            let mut all_satisfied_by = Vec::new();
            let mut has_deferred = false;

            for sub in conditions {
                let sub_detail = check_prereq(sub, sim_state, producer_index);
                match &sub_detail.check {
                    PrereqCheck::Pass => {
                        any_pass = true;
                        break;
                    }
                    PrereqCheck::Fail {
                        missing,
                        satisfied_by,
                    } => {
                        all_missing.push(missing.clone());
                        all_satisfied_by.extend(satisfied_by.iter().cloned());
                    }
                    PrereqCheck::Deferred { .. } => {
                        has_deferred = true;
                    }
                }
            }

            if any_pass {
                PrereqDetail {
                    description: "Any-of condition satisfied".to_string(),
                    check: PrereqCheck::Pass,
                }
            } else if has_deferred {
                PrereqDetail {
                    description: "Any-of condition requires runtime evaluation".to_string(),
                    check: PrereqCheck::Deferred {
                        reason: "Some alternatives require runtime evaluation".to_string(),
                    },
                }
            } else {
                // Deduplicate satisfied_by
                let mut unique: Vec<String> = all_satisfied_by;
                unique.sort();
                unique.dedup();

                PrereqDetail {
                    description: "None of the alternative conditions are met".to_string(),
                    check: PrereqCheck::Fail {
                        missing: all_missing.join("; "),
                        satisfied_by: unique,
                    },
                }
            }
        }

        MacroPrereq::FactExists { predicate } => {
            // Facts are runtime-evaluated — always defer
            PrereqDetail {
                description: format!("Fact '{}' must exist", predicate),
                check: PrereqCheck::Deferred {
                    reason: format!(
                        "Fact '{}' requires runtime evaluation (depends on execution context)",
                        predicate
                    ),
                },
            }
        }
    }
}

// ─── Tests ─────────────────────────────────────────────────────────────────

#[cfg(test)]
mod tests {
    use super::*;

    /// Build a minimal MacroSchema with the given prereqs and sets_state.
    fn test_schema(prereqs: Vec<MacroPrereq>, sets_state: Vec<(&str, bool)>) -> MacroSchema {
        use crate::dsl_v2::macros::{
            ArgStyle, MacroArgs, MacroKind, MacroRouting, MacroTarget, MacroUi, SetState,
        };

        MacroSchema {
            id: None,
            kind: MacroKind::Macro,
            tier: None,
            aliases: vec![],
            taxonomy: None,
            ui: MacroUi {
                label: String::new(),
                description: String::new(),
                target_label: String::new(),
            },
            routing: MacroRouting {
                mode_tags: vec![],
                operator_domain: None,
            },
            target: MacroTarget {
                operates_on: String::new(),
                produces: None,
                allowed_structure_types: vec![],
            },
            args: MacroArgs {
                style: ArgStyle::Keyworded,
                required: HashMap::new(),
                optional: HashMap::new(),
            },
            required_roles: vec![],
            optional_roles: vec![],
            docs_bundle: None,
            prereqs,
            expands_to: vec![],
            sets_state: sets_state
                .into_iter()
                .map(|(k, v)| SetState {
                    key: k.to_string(),
                    value: serde_json::Value::Bool(v),
                })
                .collect(),
            unlocks: vec![],
        }
    }

    /// Build a test MacroRegistry from a list of (fqn, prereqs, sets_state).
    #[allow(clippy::type_complexity)]
    fn test_registry(entries: Vec<(&str, Vec<MacroPrereq>, Vec<(&str, bool)>)>) -> MacroRegistry {
        let mut registry = MacroRegistry::new();
        for (fqn, prereqs, sets_state) in entries {
            registry.add(fqn.to_string(), test_schema(prereqs, sets_state));
        }
        registry
    }

    // --- Sequence validation ---

    #[test]
    fn test_empty_sequence_passes() {
        let registry = test_registry(vec![]);
        let result = validate_macro_sequence(&[], &registry, &HashSet::new(), &HashSet::new());
        assert!(result.feasible);
        assert_eq!(result.pass_count, 0);
        assert_eq!(result.fail_count, 0);
    }

    #[test]
    fn test_single_macro_no_prereqs() {
        let registry = test_registry(vec![(
            "structure.setup",
            vec![],
            vec![("structure.exists", true)],
        )]);

        let result = validate_macro_sequence(
            &["structure.setup".to_string()],
            &registry,
            &HashSet::new(),
            &HashSet::new(),
        );

        assert!(result.feasible);
        assert_eq!(result.pass_count, 1);
    }

    #[test]
    fn test_sequence_chain_passes() {
        // structure.setup (no prereqs, sets structure.exists)
        // case.open (requires structure.exists, sets case.exists)
        let registry = test_registry(vec![
            ("structure.setup", vec![], vec![("structure.exists", true)]),
            (
                "case.open",
                vec![MacroPrereq::StateExists {
                    key: "structure.exists".to_string(),
                }],
                vec![("case.exists", true)],
            ),
        ]);

        let result = validate_macro_sequence(
            &["structure.setup".to_string(), "case.open".to_string()],
            &registry,
            &HashSet::new(),
            &HashSet::new(),
        );

        assert!(result.feasible);
        assert_eq!(result.pass_count, 2);
        assert_eq!(result.fail_count, 0);
    }

    #[test]
    fn test_sequence_wrong_order_fails() {
        let registry = test_registry(vec![
            ("structure.setup", vec![], vec![("structure.exists", true)]),
            (
                "case.open",
                vec![MacroPrereq::StateExists {
                    key: "structure.exists".to_string(),
                }],
                vec![("case.exists", true)],
            ),
        ]);

        // case.open BEFORE structure.setup — should fail
        let result = validate_macro_sequence(
            &["case.open".to_string(), "structure.setup".to_string()],
            &registry,
            &HashSet::new(),
            &HashSet::new(),
        );

        assert!(!result.feasible);
        assert_eq!(result.fail_count, 1);

        // The first entry (case.open) should fail
        assert!(matches!(
            result.validations[0].check,
            PrereqCheck::Fail { .. }
        ));
        // The second (structure.setup) should pass
        assert!(matches!(result.validations[1].check, PrereqCheck::Pass));
    }

    #[test]
    fn test_session_state_satisfies_prereq() {
        let registry = test_registry(vec![(
            "case.open",
            vec![MacroPrereq::StateExists {
                key: "structure.exists".to_string(),
            }],
            vec![("case.exists", true)],
        )]);

        let mut current_state = HashSet::new();
        current_state.insert("structure.exists".to_string());

        let result = validate_macro_sequence(
            &["case.open".to_string()],
            &registry,
            &current_state,
            &HashSet::new(),
        );

        assert!(result.feasible);
        assert_eq!(result.pass_count, 1);
    }

    #[test]
    fn test_verb_completed_prereq() {
        let registry = test_registry(vec![
            ("screening.full", vec![], vec![]),
            (
                "screening.review",
                vec![MacroPrereq::VerbCompleted {
                    verb: "screening.full".to_string(),
                }],
                vec![],
            ),
        ]);

        let result = validate_macro_sequence(
            &["screening.full".to_string(), "screening.review".to_string()],
            &registry,
            &HashSet::new(),
            &HashSet::new(),
        );

        assert!(result.feasible);
        assert_eq!(result.pass_count, 2);
    }

    #[test]
    fn test_verb_completed_from_session() {
        let registry = test_registry(vec![(
            "screening.review",
            vec![MacroPrereq::VerbCompleted {
                verb: "screening.full".to_string(),
            }],
            vec![],
        )]);

        let mut completed = HashSet::new();
        completed.insert("screening.full".to_string());

        let result = validate_macro_sequence(
            &["screening.review".to_string()],
            &registry,
            &HashSet::new(),
            &completed,
        );

        assert!(result.feasible);
    }

    #[test]
    fn test_fact_exists_is_deferred() {
        let registry = test_registry(vec![(
            "approval.submit",
            vec![MacroPrereq::FactExists {
                predicate: "jurisdiction_approved".to_string(),
            }],
            vec![],
        )]);

        let result = validate_macro_sequence(
            &["approval.submit".to_string()],
            &registry,
            &HashSet::new(),
            &HashSet::new(),
        );

        // Facts are always deferred — not a failure
        assert!(result.feasible);
        assert_eq!(result.deferred_count, 1);
        assert_eq!(result.fail_count, 0);
    }

    #[test]
    fn test_any_of_one_satisfied() {
        let registry = test_registry(vec![
            ("case.open", vec![], vec![("case.exists", true)]),
            (
                "case.submit",
                vec![MacroPrereq::AnyOf {
                    conditions: vec![
                        MacroPrereq::VerbCompleted {
                            verb: "case.open".to_string(),
                        },
                        MacroPrereq::VerbCompleted {
                            verb: "case.add-party".to_string(),
                        },
                    ],
                }],
                vec![],
            ),
        ]);

        let result = validate_macro_sequence(
            &["case.open".to_string(), "case.submit".to_string()],
            &registry,
            &HashSet::new(),
            &HashSet::new(),
        );

        assert!(result.feasible);
        assert_eq!(result.pass_count, 2);
    }

    #[test]
    fn test_any_of_none_satisfied() {
        let registry = test_registry(vec![(
            "case.submit",
            vec![MacroPrereq::AnyOf {
                conditions: vec![
                    MacroPrereq::VerbCompleted {
                        verb: "case.open".to_string(),
                    },
                    MacroPrereq::VerbCompleted {
                        verb: "case.add-party".to_string(),
                    },
                ],
            }],
            vec![],
        )]);

        let result = validate_macro_sequence(
            &["case.submit".to_string()],
            &registry,
            &HashSet::new(),
            &HashSet::new(),
        );

        assert!(!result.feasible);
        assert_eq!(result.fail_count, 1);
    }

    #[test]
    fn test_unknown_macro_fails() {
        let registry = test_registry(vec![]);

        let result = validate_macro_sequence(
            &["nonexistent.macro".to_string()],
            &registry,
            &HashSet::new(),
            &HashSet::new(),
        );

        assert!(!result.feasible);
        assert_eq!(result.fail_count, 1);
        match &result.validations[0].check {
            PrereqCheck::Fail { missing, .. } => {
                assert!(missing.contains("not found"));
            }
            _ => panic!("Expected Fail for unknown macro"),
        }
    }

    #[test]
    fn test_satisfied_by_suggestions() {
        let registry = test_registry(vec![
            ("structure.setup", vec![], vec![("structure.exists", true)]),
            (
                "case.open",
                vec![MacroPrereq::StateExists {
                    key: "structure.exists".to_string(),
                }],
                vec![],
            ),
        ]);

        // case.open alone — fails with suggestion to run structure.setup
        let result = validate_macro_sequence(
            &["case.open".to_string()],
            &registry,
            &HashSet::new(),
            &HashSet::new(),
        );

        assert!(!result.feasible);
        match &result.validations[0].check {
            PrereqCheck::Fail { satisfied_by, .. } => {
                assert!(satisfied_by.contains(&"structure.setup".to_string()));
            }
            _ => panic!("Expected Fail with satisfied_by"),
        }
    }

    #[test]
    fn test_three_step_chain() {
        let registry = test_registry(vec![
            ("structure.setup", vec![], vec![("structure.exists", true)]),
            (
                "case.open",
                vec![MacroPrereq::StateExists {
                    key: "structure.exists".to_string(),
                }],
                vec![("case.exists", true)],
            ),
            (
                "screening.start",
                vec![MacroPrereq::StateExists {
                    key: "case.exists".to_string(),
                }],
                vec![],
            ),
        ]);

        let result = validate_macro_sequence(
            &[
                "structure.setup".to_string(),
                "case.open".to_string(),
                "screening.start".to_string(),
            ],
            &registry,
            &HashSet::new(),
            &HashSet::new(),
        );

        assert!(result.feasible);
        assert_eq!(result.pass_count, 3);
    }

    #[test]
    fn test_deferred_does_not_block() {
        // A mix of pass and deferred should be feasible
        let registry = test_registry(vec![
            ("step.one", vec![], vec![("step.one.done", true)]),
            (
                "step.two",
                vec![
                    MacroPrereq::StateExists {
                        key: "step.one.done".to_string(),
                    },
                    MacroPrereq::FactExists {
                        predicate: "runtime_check".to_string(),
                    },
                ],
                vec![],
            ),
        ]);

        let result = validate_macro_sequence(
            &["step.one".to_string(), "step.two".to_string()],
            &registry,
            &HashSet::new(),
            &HashSet::new(),
        );

        // Deferred does not make it infeasible
        assert!(result.feasible);
        assert_eq!(result.deferred_count, 1);
        assert_eq!(result.fail_count, 0);
    }
}
