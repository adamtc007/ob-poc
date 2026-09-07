//! The folds: control + determination, type registry.
//! Both are pure functions over the per-subject event stream.
//! `state = fold(events)`.
//!
//! `obligation` REMOVED (EOP-DD-UBO-CLEANOUT-001 T6 P2, 2026-09-07) — see
//! `lib.rs`'s module doc for the full reasoning.

pub(crate) mod control;
pub(crate) mod registry;
pub(crate) mod type_registry;
