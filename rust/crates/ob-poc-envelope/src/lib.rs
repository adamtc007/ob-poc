//! Envelope construction + TOCTOU recheck + ACP session-input draft-mode selector.
//!
//! This is the boundary tier of the three-plane architecture: it builds and
//! verifies the typed envelopes that flow between the orchestrator and the
//! mutation/execution tier. It **must not** depend on any execution crate
//! (no `runbook`, no `sequencer`, no `domain_ops`, no `database`, no
//! `services`). The only allowed downstream is `ob-poc-types`.

pub mod acp_session_input_draft_mode;
pub mod approval_token;
pub mod audit_chain;
pub mod dsl_coder;
pub mod envelope_builder;
pub mod kyc_dry_run;
pub mod language_pack;
pub mod llm_trace;
pub mod mutation_preflight;
pub mod session;
pub mod session_trace;
pub mod toctou_recheck;
pub mod workbook;
pub mod workbook_diagnostics;
pub mod workbook_revision;
