//! Scoped runbook bindings harness (Phase 3 CR A4)
//!
//! The `compile_scoped_runbook_bindings` function and the `Op` enum were
//! removed in Phase 3 CR A4 as part of Option α Op elimination. The
//! binding validation that was tested here is now performed in
//! `planning_facade::analyse_and_plan` via `symbol_refs_in_verb_call`.
//!
//! This file is retained to avoid orphaned test infra warnings; it contains
//! no test functions.
