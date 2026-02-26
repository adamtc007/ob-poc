//! Stewardship Agent — Changeset Layer (Phase 0) + Show Loop (Phase 1).
//!
//! Implements the stewardship agent architecture per spec §2–§11:
//! - **Phase 0 (Changeset Layer)**: Draft changesets, guardrails G01–G15,
//!   basis records, conflict resolution, templates, idempotency, impact analysis,
//!   17 MCP tools for changeset lifecycle management.
//! - **Phase 1 (Show Loop)**: Focus state, viewport engine (4 of 8 viewports),
//!   SSE transport, 6 MCP tools for visualization.
//!
//! All stewardship data lives in the `stewardship` schema (separate from `sem_reg`)
//! for boundary visibility. Draft snapshots live in `sem_reg.snapshots` with
//! `status = 'draft'` and are grouped by changeset UUID via `snapshot_set_id`.

// Phase 0: Changeset Layer
pub mod guardrails;
pub mod idempotency;
pub mod impact;
pub mod store;
pub mod templates;
pub mod tools_phase0;
pub mod types;

// Phase 1: Show Loop
pub mod focus;
pub mod show_loop;
pub mod tools_phase1;

// Re-export core Phase 0 types
pub use guardrails::{evaluate_all_guardrails, has_blocking_guardrails, has_warning_guardrails};
pub use idempotency::{check_idempotency, record_idempotency, with_idempotency, IdempotencyCheck};
pub use impact::{
    compute_changeset_impact, AffectedConsumer, AffectedSnapshot, ChangesetImpactReport,
    ImpactType, RiskLevel, RiskSummary,
};
pub use store::StewardshipStore;
pub use templates::{instantiate_template, validate_template};
pub use tools_phase0::{dispatch_phase0_tool, phase0_tool_specs};
pub use tools_phase1::{dispatch_phase1_tool, phase1_tool_specs};
pub use types::{
    BasisClaim, BasisKind, BasisRecord, ChangesetAction, ChangesetEntryRow, ChangesetRow,
    ChangesetStatus, ConflictRecord, ConflictStrategy, GuardrailId, GuardrailResult,
    GuardrailSeverity, ReviewDisposition, SemanticVersion, StewardshipEventType, StewardshipRecord,
    StewardshipTemplate, TemplateItem, TemplateStatus, VerbImplementationBinding,
};

// Phase 1 types (re-exported for convenience)
pub use types::{
    FocusState, ObjectRef, OverlayMode, ShowPacket, TaxonomyFocus, ViewportKind, ViewportManifest,
    ViewportModel, ViewportSpec, ViewportStatus, WorkbenchPacket, WorkbenchPayload,
};
