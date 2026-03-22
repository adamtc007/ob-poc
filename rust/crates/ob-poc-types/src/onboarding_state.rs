//! Onboarding state view types for UI rendering.
//!
//! These types power the "where am I + what can I do" contextual verb picker.
//! The server computes the group's composite state (UBO, CBUs, cases, screenings,
//! docs) and projects it into a DAG of layers. Each layer shows:
//!   - Current state (complete / in-progress / not-started / blocked)
//!   - Forward verbs that advance the state (with suggested utterances)
//!   - Revert verbs that back up the state (undo at composite level)
//!   - Blocked verbs (with reasons and prerequisites)
//!
//! ## Key Design Principles
//!
//! 1. **Undo is composite-level, not factual.**
//!    You can revert a case status (REVIEW → ASSESSMENT) or withdraw a case,
//!    but you can't "undo" a company name — facts are either right or wrong.
//!    The revert_verbs carry the composite state backward.
//!
//! 2. **Suggested utterances must mirror the intent pipeline.**
//!    Every `suggested_utterance` here MUST resolve through the same
//!    HybridVerbSearcher pipeline the user's free-text would hit.
//!    Misalignment = the user clicks a suggestion and gets a different verb.
//!
//! 3. **Pruned by composite context.**
//!    The full verb taxonomy has 1,400+ verbs. This view is pruned to
//!    only the verbs relevant to the current group's composite state.
//!    No noise — only what moves the state forward or backward.
//!
//! 4. **Context reset for left-field utterances.**
//!    If the utterance is unrelated to the current group/composite,
//!    the server returns `context_reset_hint` suggesting a scope change.

use serde::{Deserialize, Serialize};

/// Full onboarding state view returned on every chat response.
///
/// Sent as `ChatResponse.onboarding_state` when a group is in scope.
/// Omitted when no group is loaded (bootstrap mode).
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct OnboardingStateView {
    /// Client group name (e.g., "Allianz Global Investors").
    pub group_name: Option<String>,

    /// Overall onboarding progress (0-100).
    pub overall_progress_pct: u8,

    /// The active layer index — where the user "is" right now.
    /// This is the lowest-index layer that is not yet complete.
    pub active_layer_index: u8,

    /// DAG layers in dependency order.
    /// Layer 0 must complete before Layer 1 can start, etc.
    pub layers: Vec<OnboardingLayer>,

    /// CBU-level detail cards (one per CBU in scope).
    pub cbu_cards: Vec<CbuStateCard>,

    /// Hint shown when utterance is unrelated to current composite context.
    /// E.g., "Your current scope is Allianz onboarding. Did you mean to
    /// switch groups or reset context?"
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub context_reset_hint: Option<ContextResetHint>,
}

/// Hint for when the user's utterance doesn't match the current group context.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ContextResetHint {
    /// Human-readable message.
    pub message: String,
    /// Verb to reset/switch context.
    pub reset_utterance: String,
    /// Verb FQN for the reset action.
    pub reset_verb_fqn: String,
}

/// A single layer in the onboarding DAG.
///
/// Layers represent sequential phases of the onboarding lifecycle:
///   0: Group Identity (UBO / ownership / control)
///   1: CBU Identification (revenue-generating units)
///   2: KYC Case Opening (per-CBU)
///   3: Screening (sanctions, PEP, adverse media)
///   4: Document Collection (per-entity requirements)
///   5: Tollgate / Approval
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct OnboardingLayer {
    /// Layer index (0-based, drives ordering).
    pub index: u8,

    /// Human-readable layer name (e.g., "Group Ownership").
    pub name: String,

    /// Short description of what this layer covers.
    pub description: String,

    /// Current state of this layer.
    pub state: LayerState,

    /// Progress within this layer (0-100).
    /// Derived from entity counts (e.g., 2/3 CBUs have cases = 67%).
    pub progress_pct: u8,

    /// Summary line (e.g., "2 of 3 CBUs screened").
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub summary: Option<String>,

    /// Forward verbs — advance the composite state.
    /// Ordered by relevance (highest boost first).
    /// These MUST resolve through the intent pipeline identically.
    pub forward_verbs: Vec<SuggestedVerb>,

    /// Revert verbs — move the composite state backward.
    /// E.g., "Withdraw the KYC case", "Reopen for review".
    /// Only present when the layer is InProgress or Complete.
    #[serde(default, skip_serializing_if = "Vec::is_empty")]
    pub revert_verbs: Vec<SuggestedVerb>,

    /// Verbs that are blocked at this layer (with reasons).
    #[serde(default, skip_serializing_if = "Vec::is_empty")]
    pub blocked_verbs: Vec<BlockedVerb>,

    /// Unreachable verbs — exist in the registry but can never fire
    /// given the current entity state. These are dead links.
    /// E.g., "screening.run" when the entity has been archived.
    #[serde(default, skip_serializing_if = "Vec::is_empty")]
    pub unreachable_verbs: Vec<UnreachableVerb>,
}

/// State of an onboarding layer.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum LayerState {
    /// Layer is complete — all entities in this phase are done.
    Complete,
    /// Layer is actively in progress — some entities done, some pending.
    InProgress,
    /// Layer has not started — no entities in this phase yet.
    NotStarted,
    /// Layer is blocked — depends on a prior layer that isn't complete.
    Blocked,
}

/// A verb suggestion attached to a DAG layer node.
///
/// This is what the UI renders as a clickable action on the timeline.
/// Clicking it should submit the `suggested_utterance` as a chat message.
///
/// **Critical invariant:** `suggested_utterance` MUST resolve through
/// `HybridVerbSearcher.search()` to the same `verb_fqn`. If it doesn't,
/// the user clicks the suggestion and gets a different verb = broken UX.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct SuggestedVerb {
    /// Fully qualified verb name (e.g., "kyc-case.create").
    pub verb_fqn: String,

    /// Human-readable label (e.g., "Open KYC Case").
    pub label: String,

    /// Natural language phrase the user can say to invoke this verb.
    /// This is the key UX element — clicking it submits this as input.
    ///
    /// **Must be pipeline-aligned:** this phrase should be the verb's
    /// top invocation phrase so the intent pipeline resolves it correctly.
    pub suggested_utterance: String,

    /// Why this verb is suggested right now (state-derived reason).
    pub reason: String,

    /// Relevance boost (0.0 to 0.20) — higher = more prominent in UI.
    pub boost: f32,

    /// Direction of state change.
    pub direction: VerbDirection,

    /// Governance tier (e.g., "governed", "operational").
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub governance_tier: Option<String>,
}

/// Direction of a verb's state change effect.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum VerbDirection {
    /// Advances the onboarding state forward.
    Forward,
    /// Reverts the onboarding state backward (composite-level undo).
    Revert,
    /// Reads/queries state without changing it.
    Query,
}

/// A blocked verb with explanation.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct BlockedVerb {
    /// Fully qualified verb name.
    pub verb_fqn: String,

    /// Human-readable label.
    pub label: String,

    /// Why this verb is blocked.
    pub reason: String,

    /// What must happen first (e.g., "Complete screening for all CBUs").
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub prerequisite: Option<String>,

    /// Utterance that would unblock this verb (the prerequisite action).
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub unblock_utterance: Option<String>,
}

/// A verb that exists in the registry but is unreachable given current state.
///
/// This is a governance/hygiene signal — dead links in the verb surface.
/// The UI can render these as greyed-out with a "why" tooltip.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct UnreachableVerb {
    /// Fully qualified verb name.
    pub verb_fqn: String,
    /// Why this verb can never fire.
    pub reason: String,
}

/// Per-CBU state card for the onboarding view.
///
/// Shows the lifecycle position of a single CBU within the group.
/// The entity context (what it is) + the state node (where it is)
/// + the verbs (what moves it).
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct CbuStateCard {
    /// CBU identifier.
    pub cbu_id: String,

    /// CBU name (e.g., "Allianz Dynamic Commodities").
    pub cbu_name: Option<String>,

    /// Lifecycle state (e.g., "DISCOVERED", "VALIDATED", "ACTIVE").
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub lifecycle_state: Option<String>,

    /// Onboarding progress for this CBU (0-100).
    pub progress_pct: u8,

    /// Per-phase status flags — the entity's composite state.
    pub phases: CbuPhaseStatus,

    /// Forward action — the single most impactful verb to advance this CBU.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub next_action: Option<SuggestedVerb>,

    /// Revert action — the composite-level undo for this CBU.
    /// E.g., "Withdraw KYC case" or "Reopen for discovery".
    /// Only present when there's something to revert.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub revert_action: Option<SuggestedVerb>,
}

/// Per-CBU phase completion status.
///
/// Each field is a simple state indicator — the DAG layer view
/// provides the full verb/reason detail.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct CbuPhaseStatus {
    /// KYC case exists?
    pub has_case: bool,
    /// KYC case status (e.g., "INTAKE", "APPROVED").
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub case_status: Option<String>,
    /// Screening started?
    pub has_screening: bool,
    /// All screenings clear?
    pub screening_complete: bool,
    /// Document coverage (0.0 = none, 1.0 = full).
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub document_coverage_pct: Option<f64>,
}
