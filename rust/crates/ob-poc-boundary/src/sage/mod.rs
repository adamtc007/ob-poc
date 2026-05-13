//! Transitional sage sub-module — holds the remaining DB-coupled
//! `session_context` until Phase 2B of the capability-crate restructure
//! relocates it to `ob-poc-sage`.
//!
//! Phase 2A (2026-05-13) moved the eight pure-type sage modules
//! (plane, polarity, context, outcome, coder_result, verb_resolve_types,
//! disposition, pre_classify) to `ob-poc-sage` directly. They no longer
//! live under `ob-poc-boundary::sage::*`. Consumers reach them through
//! the compat re-export chain in `ob_poc::sage::*`, which now points at
//! `ob_poc_sage::*`.

#[cfg(feature = "database")]
pub mod session_context;
