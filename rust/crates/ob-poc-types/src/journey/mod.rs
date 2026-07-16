//! Journey pack DTOs hoisted from `ob-poc-boundary::journey::pack`.
//!
//! Phase 3C-prep of capability-crate restructure (2026-05-13).
//!
//! Per plan §6.5, cross-capability DTOs live in `ob-poc-types`. The pack
//! manifest types are consumed by:
//! - `ob-poc-journey` — owns the YAML loader (`load_packs_from_dir`,
//!   `PackLoadError`) and re-exports the type aliases from here.
//! - `ob-poc-boundary` — its `acp_registry_projection` builder reads
//!   `PackManifest` field-by-field to produce the typed Slice 1
//!   projection. Boundary must not depend on `ob-poc-journey` (plan §6
//!   decision 2), so these DTOs live here in the neutral types crate.
//! - `ob-poc` — the application layer consumes both surfaces.
//!
//! These are pure data shapes: serde derives, no IO, no crate-internal
//! deps. The YAML loader and `PackLoadError` stay with the integrator
//! crate that owns the catalogue source.

pub mod pack_candidate;
pub mod pack_types;
