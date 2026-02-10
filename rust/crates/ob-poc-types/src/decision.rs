//! Decision packet types for unified clarification UX
//!
//! The `DecisionPacket` system provides a single envelope for all types of
//! clarification: proposals, group/deal selection, verb/scope disambiguation,
//! and refusal. It includes confirm tokens for two-phase commit and audit
//! traces for regulated domains.

use serde::{Deserialize, Serialize};

use crate::{commands::default_true, ExecuteResult, VerbOption};

// ============================================================================
// DECISION PACKET - Unified Clarification UX
// ============================================================================

/// Kind of decision/clarification needed
#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq)]
#[serde(rename_all = "snake_case")]
pub enum DecisionKind {
    /// DSL ready, needs explicit confirm to execute
    Proposal,
    /// Missing/ambiguous client group anchor
    ClarifyGroup,
    /// Select deal from client's deals (0..n deals available)
    ClarifyDeal,
    /// Ambiguous verb intent (wraps VerbDisambiguationRequest)
    ClarifyVerb,
    /// Ambiguous scope/tier (wraps IntentTierRequest)
    ClarifyScope,
    /// Cannot safely proceed
    Refuse,
}

/// Unified decision packet - single source of truth for all clarifications
///
/// This envelope wraps existing clarification types (VerbDisambiguationRequest,
/// IntentTierRequest) and adds:
/// - Confirm token for two-phase commit
/// - Deterministic rendering via templates
/// - Audit trace for regulated domains
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct DecisionPacket {
    /// Unique packet ID (for confirm matching)
    pub packet_id: String,

    /// What kind of decision/clarification
    pub kind: DecisionKind,

    /// Session state snapshot
    pub session: SessionStateView,

    /// Original user utterance
    pub utterance: String,

    /// The clarification payload (varies by kind)
    pub payload: ClarificationPayload,

    /// Short question/prompt to show user
    pub prompt: String,

    /// Constrained choices (A/B/C style)
    pub choices: Vec<UserChoice>,

    /// Best plan preview (for Proposal kind)
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub best_plan: Option<PlanPreview>,

    /// Alternative plans
    #[serde(default, skip_serializing_if = "Vec::is_empty")]
    pub alternatives: Vec<PlanPreview>,

    /// Always true in regulated domain
    #[serde(default)]
    pub requires_confirm: bool,

    /// Token user must send to confirm (e.g., "CONFIRM p-abc123")
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub confirm_token: Option<String>,

    /// Audit trace
    pub trace: DecisionTrace,
}

/// Session state snapshot for decision context
#[derive(Debug, Clone, Serialize, Deserialize, Default)]
pub struct SessionStateView {
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub session_id: Option<uuid::Uuid>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub client_group_anchor: Option<String>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub client_group_name: Option<String>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub persona: Option<String>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub last_confirmed_verb: Option<String>,
}

/// A single choice option (A, B, C, etc.)
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct UserChoice {
    /// Choice ID: "A", "B", "C", "GLOBAL", "TYPE", etc.
    pub id: String,
    /// Display label
    pub label: String,
    /// One-line description
    pub description: String,
    /// True for escape hatches (TYPE, NARROW, etc.)
    #[serde(default)]
    pub is_escape: bool,
}

/// Clarification payload - varies by DecisionKind
#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(tag = "type", rename_all = "snake_case")]
pub enum ClarificationPayload {
    /// Ready to execute (contains DSL preview)
    Proposal(ProposalPayload),
    /// Client group selection
    Group(GroupClarificationPayload),
    /// Deal selection (after client group is set)
    Deal(DealClarificationPayload),
    /// Verb disambiguation (wraps existing type)
    Verb(VerbPayload),
    /// Scope/tier disambiguation (wraps existing type)
    Scope(ScopePayload),
    /// Cannot proceed
    Refuse(RefusePayload),
}

/// Proposal payload - DSL ready for execution
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ProposalPayload {
    /// Generated DSL source
    pub dsl_source: String,
    /// Human-readable summary
    pub summary: String,
    /// Affected entities preview
    #[serde(default, skip_serializing_if = "Vec::is_empty")]
    pub affected_entities: Vec<AffectedEntityPreview>,
    /// Effects preview
    pub effects: EffectsPreview,
    /// Warnings (if any)
    #[serde(default, skip_serializing_if = "Vec::is_empty")]
    pub warnings: Vec<String>,
}

/// Preview of an affected entity
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct AffectedEntityPreview {
    pub entity_id: String,
    pub canonical_name: String,
    pub entity_kind: String,
}

/// Effects preview for proposal
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct EffectsPreview {
    /// Read-only, write, or mixed
    pub mode: EffectMode,
    /// Short description of effects
    pub summary: String,
    /// Estimated affected entity count
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub affected_count: Option<usize>,
}

/// Effect mode for operation
#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq)]
#[serde(rename_all = "snake_case")]
pub enum EffectMode {
    ReadOnly,
    Write,
    Mixed,
}

/// Client group clarification payload
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct GroupClarificationPayload {
    /// Group options to choose from
    pub options: Vec<GroupOption>,
}

/// A single group option
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct GroupOption {
    /// Group/anchor ID
    pub id: String,
    /// Display alias/name
    pub alias: String,
    /// Confidence score
    pub score: f32,
    /// How it was found: alias, session, explicit
    pub method: String,
}

/// Deal clarification payload - select from client's deals
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct DealClarificationPayload {
    /// Client group context
    pub client_group_id: String,
    /// Client group name for display
    pub client_group_name: String,
    /// Deal options (empty = no deals, offer to create)
    pub deals: Vec<DealOption>,
    /// Whether user can create a new deal
    #[serde(default = "default_true")]
    pub can_create: bool,
}

/// A single deal option
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct DealOption {
    /// Deal ID
    pub deal_id: String,
    /// Deal name
    pub deal_name: String,
    /// Deal status (PROSPECT, QUALIFYING, NEGOTIATING, etc.)
    pub deal_status: String,
    /// Product count in deal
    pub product_count: i32,
    /// Brief summary
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub summary: Option<String>,
}

/// Verb clarification payload (wraps verb options)
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct VerbPayload {
    /// Verb options from VerbDisambiguationRequest
    pub options: Vec<VerbOption>,
    /// Context hint (e.g., "Scope is Goldman Sachs")
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub context_hint: Option<String>,
}

/// Scope/tier clarification payload
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ScopePayload {
    /// Scope options
    pub options: Vec<ScopeOption>,
    /// Context hint (e.g., "Verb is onboard_company")
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub context_hint: Option<String>,
}

/// A single scope option
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ScopeOption {
    /// Description of scope
    pub desc: String,
    /// How it was found: tag, semantic, hybrid
    pub method: String,
    /// Confidence score
    pub score: f32,
    /// Expected entity count
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub expect_count: Option<usize>,
    /// Sample entities (3-5)
    #[serde(default, skip_serializing_if = "Vec::is_empty")]
    pub sample: Vec<ScopeSample>,
    /// Snapshot ID if committed
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub snapshot_id: Option<String>,
}

/// Sample entity in scope
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ScopeSample {
    pub entity_id: String,
    pub canonical_name: String,
    pub entity_kind: String,
}

/// Refuse payload - cannot proceed
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct RefusePayload {
    /// Why we can't proceed
    pub reason: String,
    /// Suggestion for user
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub suggestion: Option<String>,
}

/// Plan preview (what will execute)
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct PlanPreview {
    /// Option ID: "A", "B", etc.
    pub option_id: String,
    /// Overall confidence
    pub confidence: f32,
    /// Selected verb
    pub verb_id: String,
    pub verb_label: String,
    /// Selected group (if applicable)
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub group: Option<GroupOption>,
    /// Selected scope
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub scope: Option<ScopeOption>,
    /// Parameter summary
    pub params_summary: String,
    /// Effects preview
    pub effects: EffectsPreview,
    /// DSL preview lines
    #[serde(default, skip_serializing_if = "Vec::is_empty")]
    pub dsl_preview: Vec<String>,
}

/// Audit trace for decision
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct DecisionTrace {
    /// Config version hash
    pub config_version: String,
    /// Entity snapshot hash
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub entity_snapshot_hash: Option<String>,
    /// Lexicon snapshot hash
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub lexicon_snapshot_hash: Option<String>,
    /// Semantic lane enabled?
    #[serde(default)]
    pub semantic_lane_enabled: bool,
    /// Embedding model ID
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub embedding_model_id: Option<String>,
    /// Verb margin score
    #[serde(default)]
    pub verb_margin: f32,
    /// Scope margin score
    #[serde(default)]
    pub scope_margin: f32,
    /// Kind margin score
    #[serde(default)]
    pub kind_margin: f32,
    /// Reason this decision kind was chosen
    pub decision_reason: String,
}

// ============================================================================
// DECISION REPLY - User responses to DecisionPacket
// ============================================================================

/// User's reply to a DecisionPacket
#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq)]
#[serde(tag = "action", rename_all = "snake_case")]
pub enum UserReply {
    /// User selected A/B/C (0-indexed)
    Select { index: usize },
    /// User confirmed a proposal
    Confirm {
        #[serde(default, skip_serializing_if = "Option::is_none")]
        token: Option<String>,
    },
    /// User typed exact text (entity, group, identifier)
    TypeExact { text: String },
    /// User wants to narrow scope
    Narrow { term: String },
    /// User wants more options
    More {
        #[serde(default, skip_serializing_if = "Option::is_none")]
        kind: Option<String>,
    },
    /// User cancelled
    Cancel,
}

/// Request to reply to a decision packet
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct DecisionReplyRequest {
    /// The packet ID being replied to
    pub packet_id: String,
    /// The user's reply
    pub reply: UserReply,
}

/// Response after handling a decision reply
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct DecisionReplyResponse {
    /// Next packet if clarification continues
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub next_packet: Option<Box<DecisionPacket>>,
    /// Execution result if confirmed and executed
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub execution_result: Option<ExecuteResult>,
    /// Message for UI
    pub message: String,
    /// Whether the decision is complete
    #[serde(default)]
    pub complete: bool,
}
