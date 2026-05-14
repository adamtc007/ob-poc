//! Session-tier shared types.
//!
//! Currently this module hosts the session "kinds" — workspace, subject,
//! agent-mode enums plus the workspace registry entry — hoisted from
//! `ob-poc-boundary::session` in Phase 3C-prep (2026-05-13) per plan
//! §6.5. They cross capability boundaries (boundary's `acp_dag_semantic`
//! and `audit_chain`, ob-poc-journey's pack manifest, ob-poc app session
//! machinery) so they belong with the cross-crate primitives.
//!
//! Historical note: the file `session/mod.rs` at HEAD before this slice
//! declared `mod context; mod manager; mod scope;` and `pub use`d their
//! contents (`ScopePath`, `SessionContext`, `SessionManager`,
//! `SessionSnapshot`, `FilterSet`, `ViewMode`). None of those files
//! existed in the tree — the module was orphaned code never compiled
//! because `pub mod session;` was missing from `lib.rs`. Activating the
//! module surfaced the rot. The dangling declarations have been
//! removed; if any of the documented runtime types are ever resurrected
//! they should land here as new files.

pub mod kinds;

pub use kinds::{AgentMode, SubjectKind, WorkspaceKind, WorkspaceRegistryEntry};
