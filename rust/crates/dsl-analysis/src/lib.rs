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
//! - Does NOT execute user verbs against the DB. The hard line is no
//!   `TransactionScope`, no `CrudExecutionPort`, no `PgCrudExecutor`,
//!   no Pattern A domain ops. sqlx for read-only catalogue/registry
//!   loading at boot IS allowed — that's the analyser loading its own
//!   metadata, not verb execution (see ADR §2.2 clarification).
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
//! - Phase 2: `validation` — pure-types module (Diagnostic, Severity,
//!   SourceSpan, ValidationContext, ValidationResult, Suggestion,
//!   ValidatedProgram, ValidatedStatement). 927 LOC.
//! - Phase 3: `verb_registry` + `runtime_registry` + `catalogue_loader`
//!   + `entity_kind`. Registry cluster ~2,084 LOC. `entity_kind` joined
//!   from Phase 9 (paired-move dep of `runtime_registry`).
//! - Phase 4: `macros` (registry subset — schema, registry, conditions,
//!   variable, scope). 2,585 LOC. Expander stays in `ob-poc`.
//! - Phase 5: `ref_resolver` + `gateway_resolver` paired-move (835 LOC,
//!   async-trait + entity-gateway + tonic).
//! - Phase 6: `lsp_validator` (713 LOC).
//! - Phase 7 (current): `suggestions` + `planning_facade` (571 LOC).
//!   `suggestions::predict_next_steps` ranks verbs; `planning_facade`
//!   is the analyse-and-plan orchestrator (parses DSL, compiles to ops,
//!   runs DAG planning, returns diagnostics + executable plan). Both
//!   reach runtime_registry + verb_registry + dsl_core; all co-resident.
//!
//! Compat re-exported from `dsl-runtime` until Phase 11 cleanup.

pub mod catalogue_loader;
pub mod entity_kind;
pub mod gateway_resolver;
pub mod lsp_validator;
pub mod macros;
pub mod planning_facade;
pub mod ref_resolver;
pub mod runtime_registry;
pub mod suggestions;
pub mod validation;
pub mod verb_registry;
