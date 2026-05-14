//! ob-poc-domain — pure reference-data shapes for the business domains.
//!
//! ## Capability claim
//!
//! Owns the DTO surface for the business domains the platform speaks in:
//! booking principal + legal entity + booking location + rulesets, BODS
//! 0.4 / LEI spine, deal taxonomy (deal/product/rate-card/line), trading
//! profile, product/instrument taxonomies, ontology lifecycle stages,
//! derived-attribute snapshots, view-config / layout metadata, entity
//! linking. Where a DTO cluster has a small sqlx helper that doesn't
//! reach into ob-poc internals, the helper lives here too (so the type
//! and its loader stay together).
//!
//! ## Anti-charter
//!
//! - NOT domain SERVICES. The Postgres repositories with business logic,
//!   materialization, governance, transaction orchestration stay in
//!   `ob-poc::database::*` and `ob-poc::services::*`. This crate is for
//!   the reference-data shapes those services produce and consume.
//! - NOT cross-capability shared DTOs. Anything referenced by two or more
//!   capability crates belongs in `ob-poc-types` (per plan §6 decision 5).
//! - NOT Sage / Drafter / boundary types. Those are intent + contract, not
//!   business domain data.
//!
//! ## Public surface contract (post Phase 4)
//!
//! Top-level modules in this crate will be:
//! - `booking_principal_types` — legal entity / booking location /
//!   booking principal / ruleset / rule / service availability /
//!   evaluation result / contract pack / delivery plan / gap report.
//! - `bods_types` — BODS 0.4 schema (EntityIdentifier, GLEIF hierarchy,
//!   UboInterest, PersonPepStatus, …).
//! - `deal_types` — Deal taxonomy (Deal, Product, ProductLine,
//!   RateCard, RateCardLine, Participant, Contract, …).
//! - `trading_profile` — trading-profile AST + types + materialization.
//! - `taxonomy` — generic taxonomy combinators (Product/Instrument).
//! - `semtaxonomy` — entity-extraction layer.
//! - `ontology` — lifecycle stage / semantic stage definitions.
//! - `derived_attributes` — canonical derived-value snapshots + lineage.
//! - `view_config_service` — view-mode / node-type / layout config
//!   loader (sqlx::PgPool helper).
//! - `entity_linking` — mention extraction + resolver + snapshot.
//!
//! ## Dependency discipline
//!
//! Must depend only on `ob-poc-types` and primitives (`chrono`, `serde`,
//! `uuid`, `anyhow`, `thiserror`, `serde_json`, `serde_yaml`, `sha2`,
//! `unicode-normalization`). DB-coupled DTOs gate `sqlx` / `bigdecimal`
//! / `rust_decimal` behind the `database` feature. Must NOT depend on
//! `dsl-core`, `dsl-runtime`, `sem_os_*`, `ob-poc-boundary`,
//! `ob-poc-sage`, `ob-poc-journey`, or any execution-tier surface.
//!
//! Open question (re-evaluate at bed-in review per plan §9): if this
//! crate grows past ~2k LOC across unrelated domains, split into
//! per-domain crates (`ob-poc-deal`, `ob-poc-booking-principal`, …).
//!
//! ## Migration status (2026-05-13)
//!
//! This crate is the destination for Phase 4 of the capability-crate
//! restructure (`docs/todo/capability-crate-restructure-v1.md`). Phase 4
//! moves ten DTO modules out of `ob-poc-boundary::*` into this crate.
//! Helpers `advisory_lock` (paired with `derived_attributes`) follow
//! their primary consumer per plan §6 decision 3.

// Phase 4.1 (2026-05-13): pure-DTO modules relocated from ob-poc-boundary.
//   - booking_principal_types (485 LOC, no DB feature)
//   - bods_types (218 LOC, database feature; rust_decimal)
//   - deal_types (287 LOC, database feature; rust_decimal/bigdecimal)
// Callers reach these via `crate::api::*` / `crate::database::*` compat
// re-exports in ob-poc, now retargeted from ob_poc_boundary::* to
// ob_poc_domain::*. Boundary no longer hosts these modules.

// bods_types relocated to `ob-poc-bods` by split v1 Slice A1 (2026-05-14).
// booking_principal_types relocated to `ob-poc-booking-principal` by Slice A3.
// deal_types relocated to `ob-poc-deal` by split v1 Slice A2 (2026-05-14).

// Phase 4.2a (2026-05-13): independent self-contained domain modules.
//   - ontology (6 files, ~45 KB) — entity taxonomy + lifecycle (loads YAML),
//     only external dep is ob_poc_types::semantic_stage
//   - semtaxonomy (514 LOC) — entity-extraction layer, zero crate refs
// Both moved from ob-poc-boundary; compat re-exports in ob-poc::lib.rs
// retargeted from ob_poc_boundary::* to ob_poc_domain::*.
// ontology relocated to `ob-poc-ontology` by split v1 Slice B2 (2026-05-14).
// semtaxonomy relocated to `ob-poc-semtaxonomy` by split v1 Slice B1 (2026-05-14).

// Phase 4.2b (2026-05-13): paired move — taxonomy depends on
// view_config_service (single `use crate::view_config_service::*`
// in taxonomy/rules.rs). Both gated `database` because they carry
// sqlx::PgPool helpers.
//   - taxonomy (5,438 LOC, 11 active files + combinators submodule) —
//     generic taxonomy combinators (Product/Instrument), builder, stack,
//     rules engine. Materialization helpers gated `database`.
//   - view_config_service (1,032 LOC) — view-mode / node-type / layout
//     config loader. Pure sqlx::PgPool helpers.
#[cfg(feature = "database")]
pub mod taxonomy;
#[cfg(feature = "database")]
pub mod view_config_service;

// Phase 4.2c (2026-05-13): derived_attributes + advisory_lock paired
// move per plan §6 decision 3 ("Helpers like advisory_lock go with
// their primary consumer"). derived_attributes::repository imports
// `crate::advisory_lock::{advisory_xact_lock, lock_key}`.
//   - derived_attributes (~739 LOC, 2 files) — canonical derived-value
//     snapshots + lineage; sqlx::PgPool repository.
//   - advisory_lock (~90 LOC) — typeless pg advisory-lock helpers
//     (advisory_xact_lock, try_advisory_xact_lock, lock_key).
#[cfg(feature = "database")]
pub mod advisory_lock;
#[cfg(feature = "database")]
pub mod derived_attributes;

// Phase 4.2d (2026-05-13): final Phase 4 modules out of boundary into
// ob-poc-domain. Both are pure DTO trees with only `ob_poc_types::*`
// external deps (trading_profile carries TradingMatrix re-exports).
//   - trading_profile (~7 files) — AST + builder + DB ops + resolver
//     + validator.
//   - entity_linking (~7 files) — mention extraction + resolver +
//     snapshot + normalize + compiler + stub.
#[cfg(feature = "database")]
pub mod entity_linking;
#[cfg(feature = "database")]
pub mod trading_profile;
