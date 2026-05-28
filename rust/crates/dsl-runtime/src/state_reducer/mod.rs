//! State reducer core types, parser, validation, and evaluation.

mod aggregate;
mod ast;
mod builtin;
mod error;
mod eval;
mod fetch;
mod overrides;
mod parser;
mod state_machine;
mod validate;
mod verbs;

pub use aggregate::evaluate_aggregate;
pub use ast::{
    AggFn, AggResult, BlockReason, BlockedVerb, BlockedWhyResult, CompareOp, ConditionBody,
    ConditionEvaluation, ConsistencyWarning, DerivationTrace, EvalScope, Expr, FieldValue, Literal,
    OverlayRow, OverrideInfo, Predicate, RuleEvaluation, ScopeData, SlotField, SlotOverlayData,
    SlotPredicate, SlotRecord, SlotReduceResult, Value,
};
pub use builtin::load_builtin_state_machine;
pub use error::{ReducerError, ReducerResult};
pub use eval::{evaluate_rules, ConditionEvaluator};
pub use fetch::{fetch_slot_overlays, fetch_slot_overlays_tx};
pub use overrides::{
    create_override, get_active_override, get_active_override_tx, list_active_overrides,
    revoke_override, CreateOverrideRequest, StateOverride,
};
pub use parser::{parse_condition_body, parse_literal, parse_value};
pub use state_machine::{
    ConditionDef, ConsistencyCheckDef, load_state_machine, OverlaySourceDef, ReducerDef, RuleDef,
    StateMachineDefinition, TransitionDef, ValidatedStateMachine,
};
pub use validate::validate_state_machine;
pub use verbs::{
    build_eval_scope_tx, diagnose_slot, handle_state_blocked_why, handle_state_check_consistency,
    handle_state_derive, handle_state_derive_all, handle_state_diagnose,
    handle_state_list_overrides, handle_state_override, handle_state_revoke_override, reduce_slot,
};
