//! ob-poc-derived-attributes — Derived-attribute snapshots and advisory-lock helpers.
//!
//! Relocated from `ob_poc_domain::{derived_attributes, advisory_lock}` by
//! ob-poc-domain split v1 Slice C1 (2026-05-14). The two modules pair
//! because `derived_attributes::repository` consumes `advisory_lock::{
//! advisory_xact_lock, lock_key}` — co-located per v1 plan §6 decision 3
//! ("helpers go with their primary consumer").
//!
//! ## Public re-exports
//!
//! `crate::advisory_lock::*` — the pg advisory-lock helpers.
//! `crate::derived_attributes::*` — the canonical derived-value plane.
#![deny(unreachable_pub)]

pub mod advisory_lock;
pub mod derived_attributes;
