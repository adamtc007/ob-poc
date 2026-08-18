//! Typed failure modes. Every rejection names the exact slot/state/transition
//! that triggered it — no silent drops.

use thiserror::Error;

/// A `dag_taxonomies` shape this compiler does not (yet) translate.
///
/// These are not bugs to fix reactively — they're the deliberate "fail
/// loud" boundary described in the crate's design doc. Each variant is a
/// real, observed shape in `kyc_dag.yaml`.
#[derive(Debug, Clone, PartialEq, Eq, Error)]
pub enum ShapeError {
    #[error("slot '{slot_id}' not found in this DAG")]
    SlotNotFound { slot_id: String },

    #[error("slot '{slot_id}' has no state machine (stateless slot)")]
    SlotHasNoStateMachine { slot_id: String },

    #[error(
        "slot '{slot_id}' declares a state_machine reference ('{reference}') instead of a \
         structured block — nothing to compile"
    )]
    SlotStateMachineIsReference { slot_id: String, reference: String },

    #[error("slot '{slot_id}' has no entry state (state_machine.states has no entry: true)")]
    NoEntryState { slot_id: String },

    #[error(
        "slot '{slot_id}' has {count} entry states ({states}) — exactly one is required"
    )]
    MultipleEntryStates {
        slot_id: String,
        count: usize,
        states: String,
    },

    #[error(
        "transition '{from} -> {to}' in slot '{slot_id}' has a list-valued `via` ({verbs}) — \
         ambiguous whether these verbs are alternative (OR) or joint (AND) triggers, not \
         supported"
    )]
    TransitionViaIsList {
        slot_id: String,
        from: String,
        to: String,
        verbs: String,
    },

    #[error("transition '{from} -> {to}' in slot '{slot_id}' has no `via` verb")]
    TransitionViaMissing {
        slot_id: String,
        from: String,
        to: String,
    },

    #[error(
        "transition '{from} -> {to}' in slot '{slot_id}' has `via: {raw}`, which is not a verb \
         FQN (free-text annotation, not a callable verb)"
    )]
    TransitionViaNotAVerb {
        slot_id: String,
        from: String,
        to: String,
        raw: String,
    },

    #[error(
        "transition (via '{via}') in slot '{slot_id}' names unknown `to` state '{to}' — not \
         declared in state_machine.states"
    )]
    TransitionToUnknownState {
        slot_id: String,
        via: String,
        to: String,
    },

    #[error(
        "transition '{from_raw} -> {to}' (via '{via}') in slot '{slot_id}' has a `from` value \
         this compiler cannot resolve to real state id(s): {from_raw}"
    )]
    TransitionFromUnresolvable {
        slot_id: String,
        from_raw: String,
        to: String,
        via: String,
    },

    #[error(
        "state '{state_id}' in slot '{slot_id}' declares `awaits` AND has outgoing transitions \
         — a state resolves via exactly one mechanism"
    )]
    AwaitsAndTransitionsBothPresent { slot_id: String, state_id: String },

    #[error(
        "state '{state_id}' in slot '{slot_id}' awaits slot '{await_slot}', which is not \
         declared in this DAG"
    )]
    AwaitTargetSlotNotFound {
        slot_id: String,
        state_id: String,
        await_slot: String,
    },

    #[error(
        "state '{state_id}' in slot '{slot_id}' awaits case outcome '{outcome}', which is not \
         a declared terminal state of slot '{await_slot}'"
    )]
    AwaitCaseNotATerminalState {
        slot_id: String,
        state_id: String,
        await_slot: String,
        outcome: String,
    },

    #[error(
        "state '{state_id}' in slot '{slot_id}' awaits slot '{await_slot}', whose terminal \
         state(s) {missing} have no matching arm in `cases` — the switch is not exhaustive"
    )]
    AwaitCasesNotExhaustive {
        slot_id: String,
        state_id: String,
        await_slot: String,
        missing: String,
    },

    #[error(
        "state '{state_id}' in slot '{slot_id}' has an awaits case (outcome '{outcome}') whose \
         `to` names unknown state '{to}' — not declared in state_machine.states"
    )]
    AwaitCaseTargetUnknown {
        slot_id: String,
        state_id: String,
        outcome: String,
        to: String,
    },
}

/// Top-level compile failure: either the source shape was rejected before
/// any DSL text was emitted ([`ShapeError`]), or the emitted DSL text was
/// rejected by the real `dsl-bpmn-frontend`/`dsl-lowering` pipeline
/// (structurally invalid BPMN — unreachable node, unterminated path, etc).
#[derive(Debug, Error)]
pub enum DagToBpmnError {
    #[error(transparent)]
    Shape(#[from] ShapeError),

    #[error("emitted DSL template was rejected by the compile pipeline: {0}")]
    Pipeline(#[source] anyhow::Error),
}
