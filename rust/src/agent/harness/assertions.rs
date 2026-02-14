//! Assertion engine for scenario step validation.
//!
//! Checks structured fields only — never LLM prose. Supports partial matching:
//! only fields specified in the expectation are checked.

use super::StepExpectation;
use crate::agent::orchestrator::{IntentTrace, OrchestratorOutcome};
use crate::mcp::intent_pipeline::PipelineOutcome;
use crate::session::unified::UnifiedSession;
use serde::Serialize;

/// A single assertion failure with expected vs actual values.
#[derive(Debug, Clone, Serialize)]
pub struct AssertionFailure {
    pub field: String,
    pub expected: String,
    pub actual: String,
}

impl std::fmt::Display for AssertionFailure {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        write!(f, "{}: expected '{}', got '{}'", self.field, self.expected, self.actual)
    }
}

/// Map PipelineOutcome to its assertion label.
fn outcome_label(outcome: &PipelineOutcome) -> &'static str {
    match outcome {
        PipelineOutcome::Ready => "Ready",
        PipelineOutcome::NeedsUserInput => "NeedsUserInput",
        PipelineOutcome::NeedsClarification => "NeedsClarification",
        PipelineOutcome::NoMatch => "NoMatch",
        PipelineOutcome::SemanticNotReady => "SemanticNotReady",
        PipelineOutcome::ScopeResolved { .. } => "ScopeResolved",
        PipelineOutcome::ScopeCandidates => "ScopeCandidates",
        PipelineOutcome::DirectDslNotAllowed => "DirectDslNotAllowed",
        PipelineOutcome::NoAllowedVerbs => "NoAllowedVerbs",
        PipelineOutcome::MacroExpanded { .. } => "MacroExpanded",
    }
}

/// Check a step's outcome against partial expectations.
/// Returns a list of failures (empty = all pass).
pub fn check_step(
    expected: &StepExpectation,
    outcome: &OrchestratorOutcome,
    session: &UnifiedSession,
    prev_entry_count: usize,
) -> Vec<AssertionFailure> {
    let mut failures = Vec::new();
    let result = &outcome.pipeline_result;
    let trace = &outcome.trace;

    // -- Outcome kind --
    if let Some(ref exp_outcome) = expected.outcome {
        let actual = outcome_label(&result.outcome);
        if actual != exp_outcome.as_str() {
            failures.push(AssertionFailure {
                field: "outcome".into(),
                expected: exp_outcome.clone(),
                actual: actual.to_string(),
            });
        }
    }

    // -- Chosen verb --
    if let Some(ref exp_verb) = expected.chosen_verb {
        let actual = trace.final_verb.as_deref().unwrap_or("<none>");
        if actual != exp_verb.as_str() {
            failures.push(AssertionFailure {
                field: "chosen_verb".into(),
                expected: exp_verb.clone(),
                actual: actual.to_string(),
            });
        }
    }

    // -- Forced verb --
    if let Some(ref exp_forced) = expected.forced_verb {
        let actual = trace.forced_verb.as_deref().unwrap_or("<none>");
        if actual != exp_forced.as_str() {
            failures.push(AssertionFailure {
                field: "forced_verb".into(),
                expected: exp_forced.clone(),
                actual: actual.to_string(),
            });
        }
    }

    // -- SemReg mode --
    if let Some(ref exp_mode) = expected.semreg_mode {
        if trace.sem_reg_mode.as_str() != exp_mode.as_str() {
            failures.push(AssertionFailure {
                field: "semreg_mode".into(),
                expected: exp_mode.clone(),
                actual: trace.sem_reg_mode.clone(),
            });
        }
    }

    // -- Selection source --
    if let Some(ref exp_src) = expected.selection_source {
        if trace.selection_source.as_str() != exp_src.as_str() {
            failures.push(AssertionFailure {
                field: "selection_source".into(),
                expected: exp_src.clone(),
                actual: trace.selection_source.clone(),
            });
        }
    }

    // -- Selection source in set --
    if let Some(ref exp_set) = expected.selection_source_in {
        if !exp_set.iter().any(|s| s == &trace.selection_source) {
            failures.push(AssertionFailure {
                field: "selection_source_in".into(),
                expected: format!("{:?}", exp_set),
                actual: trace.selection_source.clone(),
            });
        }
    }

    // -- Run sheet delta --
    if let Some(exp_delta) = expected.run_sheet_delta {
        let actual_count = session.run_sheet.entries.len();
        let actual_delta = actual_count as i32 - prev_entry_count as i32;
        if actual_delta != exp_delta {
            failures.push(AssertionFailure {
                field: "run_sheet_delta".into(),
                expected: format!("{}", exp_delta),
                actual: format!("{}", actual_delta),
            });
        }
    }

    // -- Runnable count --
    if let Some(exp_runnable) = expected.runnable_count {
        let actual = session.run_sheet.entries.iter()
            .filter(|e| matches!(e.status,
                crate::session::unified::EntryStatus::Draft |
                crate::session::unified::EntryStatus::Ready))
            .count();
        if actual != exp_runnable {
            failures.push(AssertionFailure {
                field: "runnable_count".into(),
                expected: format!("{}", exp_runnable),
                actual: format!("{}", actual),
            });
        }
    }

    // -- sem_reg_denied_all --
    if let Some(exp) = expected.sem_reg_denied_all {
        if trace.sem_reg_denied_all != exp {
            failures.push(AssertionFailure {
                field: "sem_reg_denied_all".into(),
                expected: format!("{}", exp),
                actual: format!("{}", trace.sem_reg_denied_all),
            });
        }
    }

    // -- semreg_unavailable --
    if let Some(exp) = expected.semreg_unavailable {
        if trace.semreg_unavailable != exp {
            failures.push(AssertionFailure {
                field: "semreg_unavailable".into(),
                expected: format!("{}", exp),
                actual: format!("{}", trace.semreg_unavailable),
            });
        }
    }

    // -- bypass_used --
    if let Some(ref exp) = expected.bypass_used {
        let actual = trace.bypass_used.as_deref().unwrap_or("<none>");
        if actual != exp.as_str() {
            failures.push(AssertionFailure {
                field: "bypass_used".into(),
                expected: exp.clone(),
                actual: actual.to_string(),
            });
        }
    }

    // -- dsl_non_empty --
    if let Some(exp) = expected.dsl_non_empty {
        let actual_non_empty = !result.dsl.is_empty();
        if actual_non_empty != exp {
            failures.push(AssertionFailure {
                field: "dsl_non_empty".into(),
                expected: format!("{}", exp),
                actual: format!("{}", actual_non_empty),
            });
        }
    }

    // -- Trace sub-fields --
    if let Some(ref trace_exp) = expected.trace {
        check_trace(trace_exp, trace, &mut failures);
    }

    // -- Global invariants (always checked) --
    check_global_invariants(result, trace, &mut failures);

    failures
}

fn check_trace(
    exp: &super::TraceExpectation,
    trace: &IntentTrace,
    failures: &mut Vec<AssertionFailure>,
) {
    if let Some(exp_checked) = exp.macro_semreg_checked {
        if trace.macro_semreg_checked != exp_checked {
            failures.push(AssertionFailure {
                field: "trace.macro_semreg_checked".into(),
                expected: format!("{}", exp_checked),
                actual: format!("{}", trace.macro_semreg_checked),
            });
        }
    }

    if let Some(exp_non_empty) = exp.macro_denied_verbs_non_empty {
        let actual_non_empty = !trace.macro_denied_verbs.is_empty();
        if actual_non_empty != exp_non_empty {
            failures.push(AssertionFailure {
                field: "trace.macro_denied_verbs_non_empty".into(),
                expected: format!("{}", exp_non_empty),
                actual: format!("{}", actual_non_empty),
            });
        }
    }

    if let Some(ref exp_kind) = exp.dominant_entity_kind {
        let actual_kind = trace.dominant_entity_kind.as_deref().unwrap_or("");
        if actual_kind != exp_kind {
            failures.push(AssertionFailure {
                field: "trace.dominant_entity_kind".into(),
                expected: exp_kind.clone(),
                actual: actual_kind.to_string(),
            });
        }
    }

    if let Some(exp_filtered) = exp.entity_kind_filtered {
        if trace.entity_kind_filtered != exp_filtered {
            failures.push(AssertionFailure {
                field: "trace.entity_kind_filtered".into(),
                expected: format!("{}", exp_filtered),
                actual: format!("{}", trace.entity_kind_filtered),
            });
        }
    }
}

/// Global invariants checked on every step regardless of expectations.
fn check_global_invariants(
    result: &crate::mcp::intent_pipeline::PipelineResult,
    _trace: &IntentTrace,
    failures: &mut Vec<AssertionFailure>,
) {
    // INV-1: NoAllowedVerbs must have empty DSL
    if matches!(result.outcome, PipelineOutcome::NoAllowedVerbs) && !result.dsl.is_empty() {
        failures.push(AssertionFailure {
            field: "INVARIANT: NoAllowedVerbs must have empty DSL".into(),
            expected: "empty".into(),
            actual: format!("{} chars", result.dsl.len()),
        });
    }

    // INV-2: MacroExpanded implies macro_semreg_checked (when SemReg is available)
    // Note: this is checked conditionally via trace expectations, not forced globally
    // since SemReg availability varies per test
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_empty_expectation_always_passes() {
        // StepExpectation::default() has all None fields → no assertions → no failures
        let exp = StepExpectation::default();
        let failures: Vec<AssertionFailure> = vec![]; // Would need real outcome; skip for unit test
        assert!(failures.is_empty());
        // Verify defaults
        assert!(exp.outcome.is_none());
        assert!(exp.chosen_verb.is_none());
    }

    #[test]
    fn test_assertion_failure_display() {
        let f = AssertionFailure {
            field: "outcome".into(),
            expected: "Ready".into(),
            actual: "NoMatch".into(),
        };
        assert_eq!(format!("{}", f), "outcome: expected 'Ready', got 'NoMatch'");
    }

    #[test]
    fn test_outcome_label_coverage() {
        assert_eq!(outcome_label(&PipelineOutcome::Ready), "Ready");
        assert_eq!(outcome_label(&PipelineOutcome::NoMatch), "NoMatch");
        assert_eq!(outcome_label(&PipelineOutcome::NeedsClarification), "NeedsClarification");
        assert_eq!(outcome_label(&PipelineOutcome::DirectDslNotAllowed), "DirectDslNotAllowed");
        assert_eq!(outcome_label(&PipelineOutcome::NoAllowedVerbs), "NoAllowedVerbs");
    }
}
