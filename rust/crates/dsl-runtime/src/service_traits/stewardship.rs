//! Stewardship dispatch (SemReg Phase 0 + Phase 1 MCP tools).
//!
//! Stewardship tools mutate the Semantic Registry — changesets, focus state,
//! guardrails, impact analysis, idempotency, review lifecycle, viewport
//! manifests. The concrete tool surfaces live in
//! `ob_poc::sem_reg::stewardship` (phase 0 = changeset layer, phase 1 =
//! show loop / viewport engine). Plugin ops that relocated to
//! `dsl-runtime::domain_ops::{sem_os_focus_ops, sem_os_governance_ops,
//! sem_os_changeset_ops}` consume the trait below rather than reaching
//! into sem_reg directly.
//!
//! # Trait surface
//!
//! Every tool has the same shape: a name + JSON args + actor principal,
//! returning a success/failure outcome with JSON data. That uniformity
//! lets a single dispatch method represent the full ~25-tool surface.
//! The concrete per-tool routing stays in ob-poc, dispatched by name.
//!
//! # Why single method instead of per-tool methods
//!
//! The three consumer op files use a macro (`focus_op!`,
//! `governance_op!`, `changeset_op!`) that constructs ~17 op structs
//! each of which delegates to the SAME helper with a different
//! `tool_name` literal. Per-tool trait methods would require 17+ method
//! definitions with identical signatures — ceremony without benefit.
//! Single-method dispatch mirrors the existing
//! `sem_reg::stewardship::dispatch_phase0_tool` / `dispatch_phase1_tool`
//! cascade directly.

use async_trait::async_trait;
use sem_os_core::principal::Principal;
use serde::{Deserialize, Serialize};

/// Outcome of a stewardship tool invocation. Projection of the internal
/// `SemRegToolResult` to the plane-crossing boundary (plane crossings
/// take data, not backend-specific structures).
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct StewardshipOutcome {
    /// True if the tool succeeded. False indicates a business-level
    /// rejection — a guardrail blocked the changeset, a review is
    /// not authorised, etc. — not a system error.
    pub success: bool,
    /// Tool-specific response payload.
    pub data: serde_json::Value,
    /// Human-readable failure message when `success == false`. `None`
    /// on success.
    pub message: Option<String>,
}

/// Dispatcher for stewardship Phase 0 (changeset layer) and Phase 1
/// (show loop / viewport) MCP tools.
///
/// Implementations look the tool up by name, run it against the
/// registry, and return the outcome. Unknown tool names resolve to
/// `Ok(None)` so callers can chain dispatchers; runtime errors (DB,
/// serialization, actor-auth failure) return `Err`.
#[async_trait]
pub trait StewardshipDispatch: Send + Sync {
    /// Dispatch a tool by name with pre-extracted JSON args and the
    /// invoking principal.
    ///
    /// Returns:
    /// - `Ok(Some(outcome))` — tool recognised; outcome reports
    ///   business success or rejection.
    /// - `Ok(None)` — tool name does not match any Phase 0 or Phase 1
    ///   stewardship tool. Callers that chain multiple dispatchers use
    ///   this to fall through.
    /// - `Err(_)` — system-level failure (DB error, invalid args shape,
    ///   actor lookup failed, etc.).
    async fn dispatch(
        &self,
        tool_name: &str,
        args: &serde_json::Value,
        principal: &Principal,
    ) -> anyhow::Result<Option<StewardshipOutcome>>;
}
