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

pub mod taxonomy;
pub mod view_config_service;
