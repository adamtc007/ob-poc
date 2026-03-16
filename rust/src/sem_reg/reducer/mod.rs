//! State reducer core types, parser, validation, and evaluation.

pub mod aggregate;
pub mod ast;
pub mod builtin;
pub mod error;
pub mod eval;
pub mod fetch;
pub mod overrides;
pub mod parser;
pub mod state_machine;
pub mod validate;
pub mod verbs;

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
pub use fetch::fetch_slot_overlays;
pub(crate) use fetch::fetch_slot_overlays_tx;
pub(crate) use overrides::get_active_override_tx;
pub use overrides::{
    create_override, get_active_override, list_active_overrides, revoke_override,
    CreateOverrideRequest, StateOverride,
};
pub use parser::{parse_condition_body, parse_literal, parse_value};
pub use state_machine::{
    compute_reducer_revision, load_state_machine, ConditionDef, ConsistencyCheckDef,
    OverlaySourceDef, ReducerDef, RuleDef, StateMachineDefinition, TransitionDef,
    ValidatedStateMachine,
};
pub use validate::validate_state_machine;
pub(crate) use verbs::build_eval_scope_tx;
pub use verbs::{
    diagnose_slot, handle_state_blocked_why, handle_state_check_consistency, handle_state_derive,
    handle_state_derive_all, handle_state_diagnose, handle_state_list_overrides,
    handle_state_override, handle_state_revoke_override, reduce_slot,
};
