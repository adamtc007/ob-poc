//! Envelope construction + TOCTOU recheck + ACP session-input draft-mode selector.
//!
//! This is the boundary tier of the three-plane architecture: it builds and
//! verifies the typed envelopes that flow between the orchestrator and the
//! mutation/execution tier. It **must not** depend on any execution crate
//! (no `runbook`, no `sequencer`, no `domain_ops`, no `database`, no
//! `services`). The only allowed downstream is `ob-poc-types`.

pub mod acp_session_input_draft_mode;
pub mod envelope_builder;
pub mod llm_trace;
pub mod toctou_recheck;
pub mod workbook;
