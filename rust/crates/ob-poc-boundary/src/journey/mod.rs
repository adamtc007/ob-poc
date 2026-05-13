//! Journey-pack manifest types relocated from ob-poc's `src/journey/`.
//!
//! - Slice 2d.2 (2026-05-12): `pack` — pack manifest types for ACP discovery
//!   surface projection.
//! - Slice 2u (2026-05-13): `handoff` — `PackHandoff` context-forwarding DTO
//!   used by `repl::session_v2` and `sequencer`; `pack_state` — pack lifecycle
//!   FSM (`Dormant → Active → Suspended → Completed`). Both are pure DTOs +
//!   FSM with zero internal-crate deps.
//!
//! The remaining journey modules (router, playback, template instantiation,
//! pack manager) stay in ob-poc because they reach into the REPL execution
//! tier (`repl::runbook`, `repl::types_v2`).
//!
//! Phase 3C of capability-crate restructure (paused 2026-05-13): a move
//! into ob-poc-journey was attempted but pack.rs is still needed by
//! `acp_registry_projection.rs` (a second site with the same deep
//! coupling pattern that `acp_dag_semantic` had before Phase 3B). Phase
//! 3C cannot land until that second site is refactored to consume a
//! boundary-owned projection (Phase 3C-followup). For now journey/*
//! stays in boundary.

pub mod handoff;
pub mod pack;
pub mod pack_state;
