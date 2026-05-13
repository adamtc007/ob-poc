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
//! - NOT Sage / Coder / boundary types. Those are intent + contract, not
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

// Empty — Phase 4 fills this in.
