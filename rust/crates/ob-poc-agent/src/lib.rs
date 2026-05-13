//! ob-poc-agent — Sage ACP runtime.
//!
//! ## Capability claim
//!
//! Owns the Sage-ACP-server runtime that an editor (Zed, JetBrains)
//! launches over stdio. Holds:
//!
//! - The Motivated Sage planning loop (GoalFrame, frontier computation,
//!   blocker detection, motivation prompt assembly, GoalProposalTrace
//!   emission).
//! - In-memory SemOS indices (scoped verb surface, pack catalogue, NOM
//!   equivalent, AffinityGraph) loaded at session start via
//!   `sem_os_client` and refreshed via dirty-flag propagation.
//! - The LLM call site (BYOK — Anthropic, OpenAI). Constrained-composition
//!   discipline: the LLM selects from sanctioned macros/packs/verbs, not
//!   free-text DSL.
//! - The LSP-shaped channel client to the REPL (runbook as textDocument,
//!   `validate-only` + `validate-and-execute` methods, `publishDiagnostics`
//!   as the feedback channel).
//! - The MCP client to the SemOS knowledge surface (entity resolution,
//!   active verb surface at state, macro/pack catalogue, FSM transitions,
//!   constellation walk).
//! - Audit emission (local JSONL + OTLP exporter).
//!
//! ## Anti-charter
//!
//! - NOT the ACP discovery / projection surface (the read-only pack /
//!   policy / context envelope projection that ACP editors observe). That
//!   lives in `ob-poc-boundary::acp_*`. The agent constructs the
//!   `AcpJsonRpcAgent` dispatcher with injected runtime deps; it does not
//!   re-implement the projection.
//! - NOT the Drafter type vocabulary (lives in `ob-poc-sage`).
//! - NOT the validator/executor (lives in `ob-poc::repl`, served to the
//!   agent over the LSP-shaped channel).
//! - NOT the SemOS substrate or registry mutation authority (lives in
//!   `sem_os_*`).
//!
//! ## Dependency discipline
//!
//! Depends on `ob-poc-types`, `ob-poc-diagnostics`, `ob-poc-sage`,
//! `ob-poc-boundary`, `dsl-runtime`, `sem_os_client`, `sem_os_core`, plus
//! primitives (`tokio`, `serde`, `chrono`, `uuid`, `tracing`, `anyhow`,
//! `thiserror`). Must NOT depend on `ob-poc` — the Sage ACP capability is
//! intended to ship as a standalone productisable artefact (V&S §3, R5).
//! Engines that live in `ob-poc` (`llm_sage`, `valid_verb_set`,
//! `deterministic`) are reached via trait abstractions defined in
//! `ob-poc-sage`; concrete impls live in `ob-poc`; the binary integrator
//! wires the impl into the agent at startup.
//!
//! ## Migration status (2026-05-13)
//!
//! Phase 2 of the Sage ACP capability plan
//! (`/Users/adamtc007/.claude/plans/context-ref-file-users-adamtc007-downlo-serialized-blum.md`).
//! Phase 2.1 (this commit) creates the empty skeleton. Subsequent slices
//! fill in:
//!   - 2.2: engine traits in ob-poc-sage
//!   - 2.3: trait impls in ob-poc
//!   - 2.4: relocate `rust/src/bin/ob_poc_acp.rs` here as `bin/sage-acp`
//!   - 2.5: in-memory SemOS index loader
//!   - 2.6: ACP `initialize` + `prompt` handlers wired to a planning loop
//!   - 2.7: LSP-shaped REPL channel client (consumes `dsl-lsp`)
//!   - 2.8: SemOS knowledge query trait + temporary impl
//!   - 2.9: audit emission (JSONL)
//!   - 2.10: hard-coded GoalFrame for the spike

/// In-memory SemOS knowledge snapshot for a session. Phase 2.5 — see
/// `index.rs` for the planning loop's read view + the spike disk
/// loader. The substrate-backed loader lands in Phase 4 once
/// `sem_os_mcp` exists.
pub mod index;

/// Motivated Sage goal frame — typed goal state the agent tracks
/// across a session. Phase 3.1 (C-01 / C-02 / C-03) — see
/// `goal_frame.rs` for the lifecycle FSM. The `GoalFrameStore`
/// (Phase 3.1b) keys frames by session id.
pub mod goal_frame;

/// Sage planning loop — Phase 2.6. Takes a raw utterance + a
/// `SessionIndex` and returns a constrained-composition draft (verb
/// FQN bounded to the pack allowlist). LLM call site is optional so
/// the spike runs offline.
pub mod planning;

/// ACP `session/prompt` interception — Phase 2.6 wiring. Routes
/// editor prompt requests through the planning loop and falls
/// through to the boundary `AcpJsonRpcAgent` for everything else.
pub mod prompt_handler;

/// LSP-shaped client surface to the REPL validator — Phase 2.7.
/// Drafts emitted by the planning loop are validated through this
/// channel before they reach the editor. The spike runs parse-only
/// in-process; Phase 4 swaps for a real out-of-process LSP client.
pub mod repl_channel;

/// SemOS knowledge query surface — Phase 2.8. Trait the planning
/// loop calls to reach entity resolution / active verb surface /
/// macro and pack catalogue / FSM transitions. Phase 2 ships a stub
/// impl; Phase 4 introduces `sem_os_mcp` as the production transport.
pub mod knowledge;

/// Audit emission — Phase 2.9. JSONL sink for replay-grade prompt
/// records. Phase 5.3 adds the OTLP companion sink per V&S §6.9.
pub mod audit;
