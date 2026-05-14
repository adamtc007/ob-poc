//! ob-poc-derived-attributes — Derived-attribute snapshots and advisory-lock helpers.
//!
//! ## Capability claim
//!
//! Owns the canonical derived-attribute plane: derived_attribute_values
//! and derived_attribute_dependencies repositories, plus the small set
//! of pg advisory-lock helpers (`advisory_xact_lock`, `try_advisory_xact_lock`,
//! `lock_key`) that the repository uses. Paired per v1 plan §6 decision 3
//! ("helpers go with their primary consumer").
//!
//! ## Anti-charter
//!
//! - NOT derivation spec materialization (lives in sem_os_postgres).
//! - NOT the runtime evaluator.
//! - NOT CBU derived-value views (postgres view definitions).
