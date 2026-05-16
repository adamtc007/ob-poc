//! ob-poc-diagnostics — error types and telemetry helpers for OB-POC.
//!
//! Per audit §5 Rule 4: "Diagnostics depend on no one downstream." This crate
//! must remain a leaf in the workspace dependency graph — it depends only on
//! external crates (nom, thiserror, serde, anyhow) and never on another
//! ob-poc workspace crate.
//!
//! Phase 3 Slice 1a (2026-05-12) — scaffold + `error.rs` extracted from
//! ob-poc. Subsequent slices will land `events/` and additional telemetry
//! helpers.

pub mod error;
pub mod events;

pub use error::{DSLError, ParseError};
