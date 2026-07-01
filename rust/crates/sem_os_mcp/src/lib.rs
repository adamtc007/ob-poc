//! sem_os_mcp — Semantic OS MCP server.
//!
//! ## Capability claim
//!
//! Owns the MCP-fronted SemOS knowledge surface (V&S §6.6 / §9
//! item 10). Exposes:
//!
//! - **entity_resolve** — natural-language fragment → candidate
//!   entity matches against the substrate's resolution surface.
//! - **active_verb_surface_at_state** — session-aware verb set
//!   that's legal right now, given workspace + constellation +
//!   entity-state snapshot. Substitutes the agent's pack-allowlist
//!   approximation with the substrate's ABAC + lifecycle-pruned
//!   surface.
//! - **pack_catalogue** — read-only walk of the pack catalogue for
//!   compound-intent matching. Phase 3 macro / pack catalogue
//!   surface.
//! - **fsm_transitions** — lifecycle-FSM transition options at a
//!   given state node.
//! - **constellation_walk** — slot-tree projection consumable by
//!   the Sage planning loop's hydration step.
//!
//! Every tool is read-only by construction. Mutation flows
//! exclusively through the workbook approval + compiled-runbook
//! gate in `ob-poc-boundary` — there is no MCP write path here.
//!
//! ## Phase 4 migration status (2026-05-13)
//!
//! Phase 4.1 (this commit): empty skeleton — workspace member,
//! charter docs, dep wall, `deny(unreachable_pub)` from day one.
//! Builds clean with zero exports.
//!
//! Subsequent Phase 4 slices populate:
//!
//! - **4.2**: the five knowledge tools above + the
//!   `sem_os_mcp` binary launcher.
//! - **4.3**: `ob-poc-agent` switches its
//!   `SemOsKnowledgeClient` / `ConstellationHydrator`
//!   implementations to MCP-backed clients pointed at this
//!   server.
//!
//! ## Anti-charter
//!
//! - NOT the application-side MCP server (REPL-facing verb
//!   search, learning, etc.). Those stay in `rust/src/mcp/` under
//!   the `ob-poc` application crate.
//! - NOT a mutation surface. Read-only projections only; the
//!   workbook approval + compiled-runbook gate in
//!   `ob-poc-boundary` is the sole mutation authority.
//! - NOT a transport — the `sem_os_client::SemOsClient` trait is
//!   the canonical API into SemOS. This crate is a **second
//!   front-end** over that trait, parallel to `sem_os_server`
//!   (REST) and `InProcessClient` (direct).
//!
//! ## Dependency discipline
//!
//! - Depends on `sem_os_client`, `sem_os_core`, `ob-poc-types` for
//!   cross-capability DTOs.
//! - Must NOT depend on `ob-poc` — the SemOS MCP surface is
//!   substrate-side, not application-side.
//! - Tools that need DB access reach through
//!   `sem_os_client` → `sem_os_postgres`. No direct sqlx usage
//!   in this crate.

/// Minimal JSON-RPC 2.0 primitives — Phase 4.2a.
pub mod protocol;

/// Knowledge-tool trait + name-keyed registry — Phase 4.2a.
pub mod tools;

/// Server dispatcher — `initialize` / `tools/list` /
/// `tools/invoke`. Phase 4.2a.
pub mod server;

/// SemOS substrate bridge trait — narrow surface the knowledge
/// tools delegate through. Spike ships `NullBridge`; Phase 4.3
/// adapts `sem_os_client::SemOsClient` to this surface.
pub mod bridge;

/// Concrete `KnowledgeTool` implementations — Phase 4.2b onwards.
pub mod tool_impls;
