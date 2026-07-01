//! OB-POC - DSL v2 System
//!
//! This crate provides a unified S-expression DSL system with data-driven execution.
//!
//! ## Architecture
//! All DSL operations flow through dsl_v2:
//! DSL Source -> Parser (Nom) -> AST -> DslExecutor -> Database
//!
//! ## Quick Start
//!
//! ```rust,no_run
//! use ob_poc::dsl_v2::execution::{DslExecutor, ExecutionContext};
//! use ob_poc::dsl_v2::syntax::parse_program;
//!
//! let dsl = r#"(cbu.create :name "Test Fund" :jurisdiction "LU")"#;
//! let program = parse_program(dsl).unwrap();
//! // Execute with DslExecutor
//! ```

pub mod acp_runtime_context;
// Phase 3 slice 2a (2026-05-12): relocated to ob-poc-boundary crate; compat re-export.
pub use ob_poc_boundary::acp_session_input_draft_mode;
// Phase 3 slice 2d.5 (2026-05-12): mixed-purity split — pure boundary types
// (descriptors, registries, reports, outcomes, DealTransitionSpec) live in
// ob-poc-boundary; Repl-coupled async drivers stay in src/ and re-export the
// envelope surface at module top.
pub mod acp_state_anchor;

// Domain handlers for business logic
pub mod domains;

// Database integration (when enabled)
#[cfg(feature = "database")]
pub mod database;

// Services for database integration
#[cfg(feature = "database")]
pub mod services;

// DSL v2 - Unified S-expression DSL with data-driven execution
pub mod dsl_v2;

// Domain operations - custom verb handlers (extracted from dsl_v2 for faster builds)
#[cfg(feature = "database")]
pub mod domain_ops;

// REST API module (when server feature is enabled)
#[cfg(feature = "server")]
pub mod api;

// MCP server module (when mcp feature is enabled)
#[cfg(feature = "mcp")]
pub mod mcp;

// Graph visualization module
#[cfg(feature = "database")]
pub mod graph;

// Phase 5a composite-blocker #23 — stategraph relocated to dsl-runtime
// (alongside discovery_ops which is its sole consumer). Pure data + file-loading
// module with only `dsl_core::ConfigLoader` as a non-std dep —
// dsl_core is already a dsl-runtime dependency.

// Navigation module - Nom-based parser for graph navigation commands
#[cfg(feature = "database")]
pub mod navigation;

// Session module - unified session context for REPL + Graph + Viewport
#[cfg(feature = "database")]
pub mod session;

// Template system for DSL generation
pub mod templates;

// Traceability - first-class utterance trace persistence
pub mod traceability;

// Transitional Sem OS runtime surfaces
#[cfg(feature = "database")]
pub mod sem_os_runtime;

// Phase 5a composite #2 — `cross_workspace` relocated to
// `dsl-runtime::cross_workspace`. External callers reach it via
// `dsl_runtime::cross_workspace::*` directly.

// Loopback calibration harness
#[cfg(feature = "database")]
pub mod calibration;

// Phase 4 Slice B (Group 2) — `verification` module relocated to
// `dsl-runtime::verification`; consumer `verify_ops` moved alongside it.

// Lexicon module - In-memory vocabulary lookup for verb discovery
// Phase 3 slice 2j (2026-05-12): relocated to ob-poc-boundary; compat re-export.
pub use ob_poc_authoring::lexicon;

// Entity Linking module - In-memory entity resolution from utterances
// ob-poc-domain split v1 Slice B3 (2026-05-14): entity_linking now lives in
// `ob-poc-entity-linking`.
#[cfg(feature = "database")]
pub use ob_poc_entity_linking as entity_linking;

// Lookup module - Unified verb search + entity linking with verb-first ordering
#[cfg(feature = "database")]
pub mod lookup;

// GLEIF integration - LEI data enrichment and corporate tree traversal
#[cfg(feature = "database")]
pub mod gleif;

// Phase 4 Slice B (Group 3) — `bods` module relocated to
// `dsl-runtime::bods`; consumer `bods_ops` moved alongside it.

// Research macros - LLM + web search for structured discovery with human review
#[cfg(feature = "database")]
pub mod research;

// Agent learning infrastructure - continuous improvement from user interactions
#[cfg(feature = "database")]
pub mod agent;

// Service Resources Pipeline - CBU Service → Resource Discovery → Provisioning
#[cfg(feature = "database")]
pub mod service_resources;

// Compiled Runbook — sole executable truth (types + execution gate)
pub mod runbook;

// REPL module - Staged runbook with anti-hallucination guarantees
#[cfg(feature = "database")]
pub mod repl;

// Agentic Sequencer — the nine-stage dispatch contract (V&S §8). Phase 5b
// relocated the orchestrator here from `repl::orchestrator_v2`; the
// `ReplOrchestratorV2` struct is unchanged and continues to host the
// tollgate state machine. Future slices (5c/5e) split stage ownership
// across finer modules; 5b is a pure path move.
#[cfg(feature = "database")]
pub mod sequencer;

// Sequencer-side concrete `TransactionScope` impl (Phase 5c-prep). The
// trait lives in `dsl_runtime::tx`; `PgTransactionScope` wraps a
// `sqlx::Transaction` so the Sequencer can begin/commit a txn and pass
// a `&mut dyn TransactionScope` through stage-8 dispatch once plugin
// ops migrate their signatures in Phase 5c-migrate.
#[cfg(feature = "database")]
pub mod sequencer_tx;

// Phase 5b-deep — typed I/O + error contracts for the nine V&S stages.
// Scaffold only: the types are defined here and unit-tested for
// serde round-tripping; the actual extraction of `sequencer.rs`'s
// tollgate handlers into per-stage typed functions lands one stage
// at a time as `5b-deep-stage-N` slices. See `sequencer_stages.rs`
// header for the rationale.
pub mod sequencer_stages;

// Phase 5e — outbox drainer. Polls `public.outbox` for post-commit
// effects (maintenance subprocess spawn, narration synthesis, UI push,
// constellation broadcast, external HTTP notify), claims rows with
// `FOR UPDATE SKIP LOCKED` semantics, dispatches to per-effect-kind
// `AsyncOutboxConsumer` impls, and updates row status to done /
// failed_retryable / failed_terminal. See `outbox/mod.rs` for the
// full lifecycle model.
#[cfg(feature = "database")]
pub mod outbox;

// BPMN-Lite integration - gRPC client, workflow dispatch, job worker, event bridge
#[cfg(feature = "database")]
pub mod bpmn_integration;

// Journey module - Pack-guided REPL v2 (Journey Packs, sentence templates, unified runbook)
pub mod journey;

// Plan Builder — compilation pipeline decomposition (verb classifier, constraint gate, plan assembler)
pub mod plan_builder;

// Phase 4 Slice B (Group 4) — `document_bundles` module relocated to
// `dsl-runtime::document_bundles`; consumer `docs_bundle_ops` moved alongside it.

// Phase 4 Slice B (Group 1) — `placeholder` module relocated to
// `dsl-runtime::placeholder`; consumer `entity_ops` moved alongside it.

// Semantic Registry — immutable snapshot-based registry for the Semantic OS
#[cfg(feature = "database")]
pub mod sem_reg;

// Phase 4 Slice B (Group 9) — `state_reducer` module relocated to
// `dsl-runtime::state_reducer`; consumer `state_ops` moved alongside it.
// Constellation graph + resolver was removed/relocated alongside this.

// SemTaxonomy v2 — three-step rip-and-replace pipeline
#[cfg(feature = "database")]
pub mod semtaxonomy_v2;

// Sage — intent understanding layer (plane, polarity, domain — no verb FQNs)
pub mod sage;

// Core domain capabilities
pub use domains::{DomainHandler, DomainRegistry, DomainResult};

// Essential error types — re-exported from ob-poc-diagnostics so the
// crate-root API (`ob_poc::DSLError`, `ob_poc::ParseError`) stays stable.
pub use ob_poc_diagnostics::error::{DSLError, ParseError};

// DSL v2 types - unified S-expression DSL
pub use dsl_v2::execution::{
    DslExecutor, ExecutionContext, ExecutionResult as DslV2ExecutionResult, ReturnType,
};
pub use dsl_v2::{
    parse_program, parse_single_verb, Argument, AstNode, Literal, Program, Span, Statement,
    VerbCall,
};

// System info
pub use system_info as get_system_info;

/// System information module
pub mod system_info {
    /// Get system information
    pub fn get_system_info() -> String {
        format!(
            "OB-POC v{} - DSL v2 Architecture",
            env!("CARGO_PKG_VERSION")
        )
    }
}

#[cfg(test)]
mod integration_tests;
