//! Membership rule body — re-export of the canonical definition
//! in `sem_os_core::membership`.
//!
//! Bucket-3 migration (2026-05-13): the rich per-field docstrings
//! that lived here were lifted into `sem_os_core::membership`, and
//! this module now re-exports the canonical types. Keeps
//! `sem_os_core` as the single schema authority per V&S §O7 /
//! ADN §7.3 without losing the prose contract.

pub use sem_os_core::membership::*;
