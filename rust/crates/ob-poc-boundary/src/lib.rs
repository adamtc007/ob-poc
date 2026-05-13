//! ob-poc-boundary — the typed boundary between Sage (intent understanding)
//! and the sequencer (execution).
//!
//! ## Capability claim
//!
//! Owns the typed contract that flows from the orchestrator to the mutation
//! tier: envelope construction, TOCTOU recheck, approval tokens, audit chain,
//! workbook DTOs, ACP discovery projection, gate policy, and session-input
//! draft-mode selection.
//!
//! ## Anti-charter
//!
//! This crate is NOT a catch-all for "things that compile cleanly without
//! execution-tier deps." If a module's job is intent classification, that
//! belongs in `ob-poc-sage`. Pack manifests belong in `ob-poc-journey`.
//! Domain DTOs belong in `ob-poc-domain`. Editor/authoring tools belong in
//! `ob-poc-authoring`. The boundary crate only holds the artefacts at the
//! Sage↔sequencer interface.
//!
//! ## Dependency discipline
//!
//! Must NOT depend on execution crates (`runbook`, `sequencer`, `domain_ops`,
//! `database`, `services`). May depend on `ob-poc-types` and
//! `ob-poc-diagnostics` (primitives), `dsl-core` (verb config metadata for
//! ACP projection), and `sem_os_core` (canonical SemOS domain types).
//!
//! ## Migration status (2026-05-13)
//!
//! This crate was renamed from `ob-poc-envelope` as part of the
//! capability-crate restructure documented in
//! `docs/todo/capability-crate-restructure-v1.md`. It currently still holds
//! modules that should move to the four new capability crates
//! (`ob-poc-sage`, `ob-poc-journey`, `ob-poc-domain`, `ob-poc-authoring`).
//! Phases 2–5 of the plan will perform those moves. Until then, do not add
//! to the misplaced module list — those modules are leaving.

pub mod acp;
pub mod acp_dag_semantic;
pub mod acp_facade;
pub mod acp_pack_context_envelope_v2;
pub mod acp_protocol;
pub mod acp_registry_projection;
pub mod acp_runtime_context;
pub mod acp_session_input_draft_mode;
pub mod acp_state_anchor;
#[cfg(feature = "database")]
pub mod advisory_lock;
pub mod clarify;
pub mod data_dictionary;
#[cfg(feature = "database")]
pub mod derived_attributes;
#[cfg(feature = "database")]
pub mod entity_linking;
#[cfg(feature = "database")]
pub mod feedback;
pub mod journey;
pub mod lexicon;
pub mod lint;
pub mod macros;
pub mod approval_token;
pub mod audit_chain;
#[cfg(feature = "database")]
pub mod bods_types;
pub mod booking_principal_types;
#[cfg(feature = "database")]
pub mod deal_types;
pub mod display_nouns;
pub mod dsl_coder;
pub mod envelope_builder;
pub mod kyc_dry_run;
pub mod language_pack;
pub mod llm_trace;
pub mod mutation_preflight;
pub mod ontology;
pub mod policy;
// Phase 2 of capability-crate restructure (2026-05-13) — sage subtree
// fully relocated to ob-poc-sage. The `sage::` submodule no longer
// exists in ob-poc-boundary.
pub mod semtaxonomy;
pub mod session;
pub mod session_trace;
pub mod taxonomy;
pub mod toctou_recheck;
pub mod traceability;
pub mod trading_profile;
#[cfg(feature = "database")]
pub mod view_config_service;
pub mod workbook;
pub mod workbook_diagnostics;
pub mod workbook_revision;
