//! dsl-analysis — the analyser plane of the three-plane architecture.
//!
//! ## Capability claim
//!
//! Reads DSL text + verb metadata and produces diagnostics, suggestions,
//! and executable plans. Hosts the verb registry, the macro registry,
//! ref/gateway resolvers, the LSP-facing semantic validator, and the
//! `analyse_and_plan` orchestrator. Owns nothing that touches a database
//! at runtime.
//!
//! ## Anti-charter
//!
//! - Does NOT execute verbs. The execution plane is `dsl-runtime`.
//! - Does NOT hold a `PgPool`, a `TransactionScope`, or any sqlx surface.
//! - Does NOT implement `VerbExecutionPort` or `CrudExecutionPort`.
//! - Does NOT host runtime services or Pattern A domain ops.
//! - Does NOT host the macro EXPANDER — only the macro REGISTRY.
//!   The expander reaches `UnifiedSession` + `sem_os_obpoc_adapter`
//!   and stays in `ob-poc`.
//!
//! ## Dependency discipline
//!
//! May depend on `dsl-core` (parser, AST, compiler, DAG primitives),
//! `ob-templates` (template loader), and `ob-poc-types` (cross-capability
//! DTOs such as the plan output). MUST NOT depend on `dsl-runtime`,
//! `sem_os_postgres`, `sqlx`, or any execution-tier surface.
//!
//! ## Migration status (2026-05-14)
//!
//! Phase 2 of the split described in `docs/todo/dsl-runtime-split-v1.md`.
//! Modules land in Phases 2–9 via `git mv` from `dsl-runtime/src/`.
//!
//! - Phase 2 (current): `validation` — pure-types module (Diagnostic,
//!   Severity, SourceSpan, ValidationContext, ValidationResult,
//!   Suggestion, ValidatedProgram, ValidatedStatement). 927 LOC, zero
//!   internal crate refs at the source. Compat re-exported from
//!   `dsl-runtime` until Phase 11 cleanup.

pub mod validation;
