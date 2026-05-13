//! Session-tier shared enums.
//!
//! Phase 3C-prep of capability-crate restructure (2026-05-13): the
//! definitions of `WorkspaceKind`, `SubjectKind`, `AgentMode`, and
//! `WorkspaceRegistryEntry` moved to `ob-poc-types::session` per plan
//! §6.5 — they cross capability boundaries (boundary + ob-poc-journey
//! pack manifest + ob-poc app machinery) so they belong with the
//! cross-crate primitives.
//!
//! This file is now a compat re-export: it preserves the
//! `ob_poc_boundary::session::*` path used by `acp_dag_semantic`,
//! `audit_chain`, `session_trace`, and the `src/repl/types_v2.rs` /
//! `src/repl/mod.rs` re-export shim that fans out to the rest of
//! ob-poc.

pub use ob_poc_types::session::*;
