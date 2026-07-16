//! ob-poc-taxonomy — Generic taxonomy combinators + view-config loader.
//!
//! Relocated from `ob_poc_domain::{taxonomy, view_config_service}` by
//! ob-poc-domain split v1 Slice C2 (2026-05-14). The two paired because
//! `taxonomy::rules` imports `view_config_service::*`.
//!
//! ## Public re-exports
//!
//! `crate::taxonomy::*` — generic combinators, builder, stack, rules engine.
//! `crate::view_config_service::*` — view-mode / node-type / layout loader.
#![deny(unreachable_pub)]

pub mod taxonomy;
// Unconditionally sqlx-backed (view-mode/node-type/layout loader queries
// against PgPool) — gated to keep the crate buildable at
// --no-default-features (2026-07-13 E5 fix, membrane check §6.2).
// `taxonomy::rules`'s own use of it is already correctly
// #[cfg(feature = "database")]-gated internally.
#[cfg(feature = "database")]
pub mod view_config_service;

#[cfg(test)]
mod integration_tests;
