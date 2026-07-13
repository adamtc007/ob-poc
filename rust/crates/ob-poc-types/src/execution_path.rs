//! `ExecutionPath` — which of the four RR-2 admission ingress points a verb
//! dispatch reached the control plane through (G3:
//! `EOP-DESIGN-CONTROLPLANE-G3-ENFORCEMENT-DIMENSION-001`).
//!
//! Threaded end-to-end from each ingress point so `EnforcedVerbs`
//! (`ob-poc::agent::control_plane_envelope_store`) can express "graduate
//! this verb on Path A only," matching the graduation runbook's §3
//! per-path order literally (AD-2(b)).
//!
//! Lives here, not in `ob-poc` or `ob-poc-control-plane`: it is a
//! values-only boundary type both `dsl-runtime` (the `VerbExecutionPort`
//! trait signature) and `ob-poc`/`ob-poc-web` (the four ingress call
//! sites) need, and all of those crates already depend on `ob-poc-types`
//! — zero new crate edges, same reasoning as `EnvelopeHandle`
//! (`envelope_handle.rs`'s module doc).

/// Which of the four RR-2 admission ingress points a verb dispatch
/// reached the control plane through.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, serde::Serialize, serde::Deserialize)]
pub enum ExecutionPath {
    /// Sequencer/runbook step dispatch — `VerbExecutionPortStepExecutor`
    /// → `execute_verb_admitting_envelope` (`step_executor_bridge.rs`).
    RunbookSequencer,
    /// Direct dsl_v2 dispatch NOT reached via `WorkflowDispatcher` —
    /// `RealDslExecutor::execute`/`execute_in_scope`, covering the MCP
    /// `dsl_execute` tool, the legacy raw-execute route, batch/sheet
    /// executors, and the no-BPMN `executor_v2` fallback.
    DslDirect,
    /// `WorkflowDispatcher`'s Direct-routed branch — the `RealDslExecutor`
    /// instance wrapped exclusively by a `WorkflowDispatcher`.
    WorkflowDispatched,
    /// Federated bus — `ObPocVerbAdapter::execute` → `execute_verb_admitting_envelope`
    /// (`bus_runtime.rs`).
    BusFederated,
}

impl ExecutionPath {
    /// The single-letter code used in `OB_POC_CONTROL_PLANE_ENFORCE_VERBS`'s
    /// `verb[:tag(|tag)*]` grammar (G3 §3(c)).
    pub fn as_letter(self) -> &'static str {
        match self {
            ExecutionPath::RunbookSequencer => "A",
            ExecutionPath::DslDirect => "B",
            ExecutionPath::WorkflowDispatched => "C",
            ExecutionPath::BusFederated => "D",
        }
    }

    /// Parses a single letter from the env-var grammar. `None` for any
    /// unrecognised letter — callers must fail the WHOLE config on this,
    /// not silently skip the entry (G3 §3(c)'s fail-closed rule).
    pub fn from_letter(letter: &str) -> Option<Self> {
        match letter {
            "A" => Some(ExecutionPath::RunbookSequencer),
            "B" => Some(ExecutionPath::DslDirect),
            "C" => Some(ExecutionPath::WorkflowDispatched),
            "D" => Some(ExecutionPath::BusFederated),
            _ => None,
        }
    }

    /// All four variants, for exhaustive test loops.
    pub const ALL: [ExecutionPath; 4] = [
        ExecutionPath::RunbookSequencer,
        ExecutionPath::DslDirect,
        ExecutionPath::WorkflowDispatched,
        ExecutionPath::BusFederated,
    ];
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn letter_roundtrip_is_total_over_all_variants() {
        for path in ExecutionPath::ALL {
            let letter = path.as_letter();
            assert_eq!(ExecutionPath::from_letter(letter), Some(path));
        }
    }

    #[test]
    fn unrecognised_letter_is_none() {
        assert_eq!(ExecutionPath::from_letter("Z"), None);
        assert_eq!(ExecutionPath::from_letter(""), None);
        assert_eq!(ExecutionPath::from_letter("a"), None);
    }
}
