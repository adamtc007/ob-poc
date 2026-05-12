//! V2 State Machine Types — REPL state model
//!
//! Defines the state machine:
//! ScopeGate → WorkspaceSelection → optional ConstellationMapSelection →
//! JourneySelection → InPack → Clarifying → SentencePlayback →
//! RunbookEditing → Executing
//!
//! Also defines `UserInputV2` — the conversational input model.
//! All answers are free-text `Message` input; no picker/form gates.

use std::collections::HashMap;

use chrono::{DateTime, Utc};
use serde::{Deserialize, Serialize};
use uuid::Uuid;

use crate::sem_os_runtime::constellation_runtime::HydratedConstellation;

// ---------------------------------------------------------------------------
// ReplStateV2 — session state machine
// ---------------------------------------------------------------------------

/// The states of the v2 REPL pipeline.
#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(tag = "state", rename_all = "snake_case")]
pub enum ReplStateV2 {
    /// Waiting for client/scope selection before any pack can start.
    ScopeGate {
        pending_input: Option<String>,
        /// Disambiguation candidates from a previous resolution attempt.
        #[serde(default, skip_serializing_if = "Option::is_none")]
        candidates: Option<Vec<super::bootstrap::BootstrapCandidate>>,
    },

    /// User has selected a client scope and must now choose a workspace.
    WorkspaceSelection { workspaces: Vec<WorkspaceOption> },

    /// CBU workspace selected; user must choose the structure constellation map
    /// before the DAG is hydrated.
    ConstellationMapSelection {
        options: Vec<ConstellationMapOption>,
    },

    /// User has scope, now choosing a journey pack.
    JourneySelection {
        candidates: Option<Vec<PackCandidate>>,
    },

    /// Inside an active pack — asking questions, matching verbs, building runbook.
    InPack {
        pack_id: String,
        required_slots_remaining: Vec<String>,
        last_proposal_id: Option<Uuid>,
    },

    /// Waiting for user to disambiguate a verb or entity.
    Clarifying {
        question: String,
        candidates: Vec<VerbCandidate>,
        original_input: String,
    },

    /// Showing a sentence for user to confirm or reject.
    SentencePlayback {
        sentence: String,
        verb: String,
        dsl: String,
        args: HashMap<String, String>,
    },

    /// Runbook exists and user is reviewing / editing it.
    RunbookEditing,

    /// Runbook is executing.
    Executing {
        runbook_id: Uuid,
        progress: ExecutionProgress,
    },
}

// ---------------------------------------------------------------------------
// Supporting types for state variants
// ---------------------------------------------------------------------------

/// A candidate pack for journey selection.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct PackCandidate {
    pub pack_id: String,
    pub pack_name: String,
    pub description: String,
    pub score: f32,
}

// Phase 3 slice 2c.2b (2026-05-12): WorkspaceKind / AgentMode / SubjectKind /
// WorkspaceRegistryEntry moved to ob-poc-envelope::session so the boundary
// tier (audit_chain, session_trace) can reference them without depending on
// the execution-tier repl module. Lateral re-export preserves all 46
// existing `crate::repl::types_v2::*` consumer paths unchanged.
pub use ob_poc_envelope::session::{AgentMode, SubjectKind, WorkspaceKind, WorkspaceRegistryEntry};

/// A selectable workspace option.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct WorkspaceOption {
    pub workspace: WorkspaceKind,
    pub label: String,
    pub description: String,
}

/// A selectable CBU structure constellation map.
#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq)]
pub struct ConstellationMapOption {
    pub constellation_map: String,
    pub constellation_family: String,
    pub label: String,
    pub description: String,
    pub jurisdiction: String,
}

/// Session scope anchored on a client group.
#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq)]
pub struct SessionScope {
    pub client_group_id: Uuid,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub client_group_name: Option<String>,
}

impl SessionScope {
    /// Sentinel UUID for infrastructure-only sessions (no client group).
    pub fn infrastructure_scope_id() -> Uuid {
        Uuid::nil()
    }

    /// Whether this scope represents an infrastructure session.
    pub fn is_infrastructure(&self) -> bool {
        self.client_group_id == Uuid::nil()
    }

    /// Create an infrastructure scope (no client group).
    pub fn infrastructure() -> Self {
        Self {
            client_group_id: Uuid::nil(),
            client_group_name: Some("SemOS Infrastructure".to_string()),
        }
    }
}

// AgentMode + SubjectKind relocated to ob-poc-envelope::session (see top-of-file note).

/// A lightweight subject reference for UI and feedback surfaces.
#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq)]
pub struct SubjectRef {
    pub kind: SubjectKind,
    pub id: Uuid,
}

/// Provisioning dependency attached to a handoff context.
#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq)]
pub struct ProvisioningDep {
    pub kind: String,
    pub reference: String,
}

/// Cross-workspace handoff payload carried alongside a working context.
#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq)]
pub struct HandoffContext {
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub source_deal_id: Option<Uuid>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub target_cbu_id: Option<Uuid>,
    pub handoff_id: Uuid,
    pub activation_path: String,
    #[serde(default)]
    pub provisioning_deps: Vec<ProvisioningDep>,
}

/// A scoped verb reference returned in hydrated workspace views.
#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq)]
pub struct VerbRef {
    pub verb_fqn: String,
    pub display_name: String,
}

/// Progress summary for the current constellation context.
#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq)]
pub struct ProgressSummary {
    pub total_slots: usize,
    pub completion_pct: u8,
    pub blocking_slots: usize,
}

/// A suggested action derived from the current scoped verb surface.
#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq)]
pub struct ActionHint {
    pub label: String,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub verb_fqn: Option<String>,
    pub action_type: String,
}

/// A workspace available to the user for navigation.
#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq)]
pub struct WorkspaceHint {
    pub workspace: WorkspaceKind,
    pub label: String,
    pub default_constellation_family: String,
    pub default_constellation_map: String,
}

/// Self-contained view of the hydrated working surface for one frame.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct WorkspaceStateView {
    pub workspace: WorkspaceKind,
    pub constellation_family: String,
    pub constellation_map: String,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub subject_ref: Option<SubjectRef>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub hydrated_constellation: Option<HydratedConstellation>,
    #[serde(default)]
    pub scoped_verb_surface: Vec<VerbRef>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub progress_summary: Option<ProgressSummary>,
    #[serde(default)]
    pub available_actions: Vec<ActionHint>,
}

/// One entry in the workspace stack.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct WorkspaceFrame {
    pub workspace: WorkspaceKind,
    pub constellation_family: String,
    pub constellation_map: String,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub subject_kind: Option<SubjectKind>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub subject_id: Option<Uuid>,
    pub session_scope: SessionScope,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub hydrated_state: Option<WorkspaceStateView>,
    pub pushed_at: DateTime<Utc>,
    #[serde(default)]
    pub stale: bool,
    /// Number of write operations (verb executions) since this frame was pushed.
    #[serde(default)]
    pub writes_since_push: u32,
    /// Whether this frame was pushed as a peek (read-only workspace glance).
    #[serde(default)]
    pub is_peek: bool,
    /// Verb FQNs from the last narration's `suggested_next`.
    /// Used as a boost signal in `HybridVerbSearcher` (+0.05 score bias).
    /// Cleared on workspace/scope change.
    #[serde(default, skip_serializing_if = "Vec::is_empty")]
    pub narration_hot_verbs: Vec<String>,

    /// Cached constellation verb index — two-way (noun, action) → verb lookup.
    /// Rebuilt on each constellation hydration. Transient (not serialized).
    #[serde(skip)]
    pub constellation_verb_index:
        Option<std::sync::Arc<crate::agent::constellation_verb_index::ConstellationVerbIndex>>,

    /// Cached stale shared fact refs for this workspace frame.
    /// Populated during hydration, used by pre-REPL staleness check and narration.
    /// Transient (not serialized).
    #[serde(skip)]
    pub stale_shared_facts: Vec<dsl_runtime::cross_workspace::fact_refs::StaleSharedFactRef>,

    // --- Constraint cascade (workspace-scoped working context) ---
    // Each workspace frame carries its own constraint state.
    // Switching workspaces (push/pop) preserves per-workspace context.
    // Synced from ExecutionContext.pending_session after verb execution.
    /// Current structure context (e.g., fund structure ID).
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub current_structure_id: Option<Uuid>,

    /// Current structure display name.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub current_structure_name: Option<String>,

    /// Current KYC case ID.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub current_case_id: Option<Uuid>,

    /// Current trading mandate ID.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub current_mandate_id: Option<Uuid>,

    /// Deal context for deal workspace.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub deal_id: Option<Uuid>,

    /// Deal display name.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub deal_name: Option<String>,

    // --- Viewport state (observation frame, NOT resource truth) ---
    // These fields describe how the user is observing the DAG.
    // They do NOT affect the DAG, do NOT trigger rehydration.
    // Mutated by nav verbs (nav.drill, nav.set-lens, etc.).
    /// Current Observatory view level.
    /// Viewport state — does not affect DAG, does not trigger rehydration.
    #[serde(default = "default_view_level")]
    pub view_level: ob_poc_types::galaxy::ViewLevel,

    /// Current focus slot path within the constellation (e.g., "cbu.kyc.screening").
    /// Viewport state — selects which part of the DAG is in focus.
    /// If this changes to a slot in a DIFFERENT constellation/CBU, that IS a
    /// materialization boundary crossing and requires rehydration.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub focus_slot_path: Option<String>,

    /// Navigation history for this workspace frame (viewport snapshots).
    /// Viewport state — back/forward restores previous observation frame.
    #[serde(default, skip_serializing_if = "Vec::is_empty")]
    pub nav_snapshots: Vec<ViewportSnapshot>,

    /// Current position in nav_snapshots (for back/forward).
    #[serde(default)]
    pub nav_cursor: usize,
}

fn default_view_level() -> ob_poc_types::galaxy::ViewLevel {
    ob_poc_types::galaxy::ViewLevel::System
}

/// Lightweight viewport snapshot for navigation history (back/forward).
/// Captures viewport state only — NOT DAG state.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ViewportSnapshot {
    pub view_level: ob_poc_types::galaxy::ViewLevel,
    pub focus_slot_path: Option<String>,
    pub timestamp: DateTime<Utc>,
}

impl WorkspaceFrame {
    /// Create a new frame using workspace defaults.
    ///
    /// # Examples
    /// ```rust
    /// use ob_poc::repl::types_v2::{SessionScope, WorkspaceFrame, WorkspaceKind};
    /// use uuid::Uuid;
    ///
    /// let frame = WorkspaceFrame::new(
    ///     WorkspaceKind::Deal,
    ///     SessionScope { client_group_id: Uuid::nil(), client_group_name: None },
    /// );
    /// assert_eq!(frame.constellation_family, "commercial");
    /// assert_eq!(frame.writes_since_push, 0);
    /// assert!(!frame.is_peek);
    /// ```
    pub fn new(workspace: WorkspaceKind, session_scope: SessionScope) -> Self {
        let registry = workspace.registry_entry();
        Self {
            workspace,
            constellation_family: registry.default_constellation_family.to_string(),
            constellation_map: registry.default_constellation_map.to_string(),
            subject_kind: registry.subject_kinds.first().cloned(),
            subject_id: None,
            session_scope,
            hydrated_state: None,
            pushed_at: Utc::now(),
            stale: false,
            writes_since_push: 0,
            is_peek: false,
            narration_hot_verbs: Vec::new(),
            constellation_verb_index: None,
            stale_shared_facts: Vec::new(),
            current_structure_id: None,
            current_structure_name: None,
            current_case_id: None,
            current_mandate_id: None,
            deal_id: None,
            deal_name: None,
            view_level: default_view_level(),
            focus_slot_path: None,
            nav_snapshots: Vec::new(),
            nav_cursor: 0,
        }
    }
}

/// Request envelope for navigation resolution.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ConstellationContextRef {
    pub session_id: Uuid,
    pub client_group_id: Uuid,
    pub workspace: WorkspaceKind,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub constellation_family: Option<String>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub constellation_map: Option<String>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub subject_kind: Option<SubjectKind>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub subject_id: Option<Uuid>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub handoff_context: Option<HandoffContext>,
}

/// Resolved context after defaults and subject resolution are applied.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ResolvedConstellationContext {
    pub session_id: Uuid,
    pub client_group_id: Uuid,
    pub workspace: WorkspaceKind,
    pub constellation_family: String,
    pub constellation_map: String,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub subject_kind: Option<SubjectKind>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub subject_id: Option<Uuid>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub handoff_context: Option<HandoffContext>,
    pub session_scope: SessionScope,
    pub agent_mode: AgentMode,
}

/// Feedback returned with session-scoped navigation responses.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct SessionFeedback {
    pub stack_depth: usize,
    pub tos: WorkspaceStateView,
    pub tos_is_peek: bool,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub previous_workspace: Option<WorkspaceKind>,
    pub stale_warning: bool,
    /// Stale shared fact references in the current workspace (cross-workspace consistency).
    /// Non-empty when the workspace is operating against superseded shared attribute versions.
    #[serde(default, skip_serializing_if = "Vec::is_empty")]
    pub stale_shared_facts: Vec<dsl_runtime::cross_workspace::fact_refs::StaleSharedFactRef>,
    #[serde(default)]
    pub scoped_verb_surface: Vec<VerbRef>,
    #[serde(default)]
    pub available_workspaces: Vec<WorkspaceHint>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub pending_verb: Option<VerbRef>,
    pub conversation_mode: ConversationMode,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub entity_resolution: Option<SessionEntityResolutionFeedback>,
}

/// Entity-resolution evidence visible on Sage session feedback.
#[derive(Debug, Clone, Serialize, Deserialize, PartialEq)]
pub struct SessionEntityResolutionFeedback {
    pub snapshot_hash: String,
    pub snapshot_version: u32,
    pub entity_count: usize,
    #[serde(default)]
    pub expected_kinds: Vec<String>,
    pub entities_resolved: bool,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub dominant_entity: Option<SessionEntityCandidateFeedback>,
    #[serde(default)]
    pub mentions: Vec<SessionEntityMentionFeedback>,
}

/// One mention resolved by the Sage entity-linking service.
#[derive(Debug, Clone, Serialize, Deserialize, PartialEq)]
pub struct SessionEntityMentionFeedback {
    pub span: (usize, usize),
    pub text: String,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub selected_id: Option<Uuid>,
    pub confidence: f32,
    #[serde(default)]
    pub candidates: Vec<SessionEntityCandidateFeedback>,
}

/// One candidate considered during entity resolution.
#[derive(Debug, Clone, Serialize, Deserialize, PartialEq)]
pub struct SessionEntityCandidateFeedback {
    pub entity_id: Uuid,
    pub entity_kind: String,
    pub canonical_name: String,
    pub score: f32,
    #[serde(default)]
    pub evidence: Vec<String>,
}

impl From<&crate::lookup::LookupResult> for SessionEntityResolutionFeedback {
    fn from(result: &crate::lookup::LookupResult) -> Self {
        let mentions = result
            .entities
            .iter()
            .map(|resolution| {
                let candidates = resolution
                    .candidates
                    .iter()
                    .take(3)
                    .map(SessionEntityCandidateFeedback::from)
                    .collect();
                SessionEntityMentionFeedback {
                    span: resolution.mention_span,
                    text: resolution.mention_text.clone(),
                    selected_id: resolution.selected,
                    confidence: resolution.confidence,
                    candidates,
                }
            })
            .collect();

        let dominant_entity =
            result
                .dominant_entity
                .as_ref()
                .map(|dominant| SessionEntityCandidateFeedback {
                    entity_id: dominant.entity_id,
                    entity_kind: dominant.entity_kind.clone(),
                    canonical_name: dominant.canonical_name.clone(),
                    score: dominant.confidence,
                    evidence: Vec::new(),
                });

        Self {
            snapshot_hash: result.entity_snapshot.hash.clone(),
            snapshot_version: result.entity_snapshot.version,
            entity_count: result.entity_snapshot.entity_count,
            expected_kinds: result.expected_kinds.clone(),
            entities_resolved: result.entities_resolved,
            dominant_entity,
            mentions,
        }
    }
}

impl From<&crate::entity_linking::EntityCandidate> for SessionEntityCandidateFeedback {
    fn from(candidate: &crate::entity_linking::EntityCandidate) -> Self {
        Self {
            entity_id: candidate.entity_id,
            entity_kind: candidate.entity_kind.clone(),
            canonical_name: candidate.canonical_name.clone(),
            score: candidate.score,
            evidence: candidate
                .evidence
                .iter()
                .map(|evidence| format!("{evidence:?}"))
                .collect(),
        }
    }
}

/// Semantic IR frame used before deterministic or probabilistic resolution.
#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq)]
pub struct UtteranceFrame {
    pub action_phrase: String,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub target_workspace_hint: Option<WorkspaceKind>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub subject_hint: Option<String>,
    pub conversation_mode: ConversationMode,
    pub scope_cue: ScopeCue,
    pub temporal_cue: TemporalCue,
}

impl UtteranceFrame {
    /// Build a deterministic utterance frame from free text cues.
    ///
    /// # Examples
    /// ```rust
    /// use ob_poc::repl::types_v2::{ConversationMode, UtteranceFrame};
    ///
    /// let frame = UtteranceFrame::from_message("show me the current KYC case");
    /// assert_eq!(frame.conversation_mode, ConversationMode::Inspect);
    /// ```
    pub fn from_message(message: &str) -> Self {
        let normalized = message.trim().to_lowercase();
        Self {
            action_phrase: normalized.clone(),
            target_workspace_hint: WorkspaceKind::from_hint(&normalized),
            subject_hint: extract_subject_hint(&normalized),
            conversation_mode: ConversationMode::classify(&normalized),
            scope_cue: ScopeCue::classify(&normalized),
            temporal_cue: TemporalCue::classify(&normalized),
        }
    }
}

/// High-level conversational mode used to select stack operations.
#[derive(Debug, Clone, Copy, Default, Serialize, Deserialize, PartialEq, Eq)]
#[serde(rename_all = "snake_case")]
pub enum ConversationMode {
    #[default]
    Inspect,
    Navigate,
    Compare,
    Prepare,
    Mutate,
    Confirm,
    Return,
}

impl ConversationMode {
    /// Classify a conversational mode from simple lexical cues.
    ///
    /// # Examples
    /// ```rust
    /// use ob_poc::repl::types_v2::ConversationMode;
    ///
    /// assert_eq!(ConversationMode::classify("compare this cbu with that one"), ConversationMode::Compare);
    /// ```
    pub fn classify(message: &str) -> Self {
        let msg = message.trim().to_lowercase();
        if matches!(msg.as_str(), "yes" | "confirm" | "approved" | "do it") {
            return Self::Confirm;
        }
        if msg.contains("go back") || msg.contains("return") || msg == "back" {
            return Self::Return;
        }
        if msg.contains("compare") || msg.contains("versus") {
            return Self::Compare;
        }
        if msg.contains("switch to")
            || msg.contains("go to ")
            || msg.contains("open the ")
            || msg.contains("take me to")
        {
            return Self::Navigate;
        }
        if msg.contains("would")
            || msg.contains("could")
            || msg.contains("can you")
            || msg.ends_with('?')
        {
            return Self::Prepare;
        }
        if msg.starts_with("create ")
            || msg.starts_with("add ")
            || msg.starts_with("update ")
            || msg.starts_with("remove ")
            || msg.starts_with("delete ")
            || msg.starts_with("activate ")
            || msg.starts_with("provision ")
        {
            return Self::Mutate;
        }
        Self::Inspect
    }
}

/// Scope cue used in utterance decomposition.
#[derive(Debug, Clone, Copy, Serialize, Deserialize, PartialEq, Eq)]
#[serde(rename_all = "snake_case")]
pub enum ScopeCue {
    Here,
    There,
    Across,
    Unspecified,
}

impl ScopeCue {
    /// Classify a scope cue from lexical markers.
    ///
    /// # Examples
    /// ```rust
    /// use ob_poc::repl::types_v2::ScopeCue;
    ///
    /// assert_eq!(ScopeCue::classify("show me this cbu"), ScopeCue::Here);
    /// ```
    pub fn classify(message: &str) -> Self {
        let msg = message.to_lowercase();
        if msg.contains("across") || msg.contains("compare") {
            return Self::Across;
        }
        if msg.contains("that ") || msg.contains("there") || msg.contains("other workspace") {
            return Self::There;
        }
        if msg.contains("this ") || msg.contains("current") || msg.contains("here") {
            return Self::Here;
        }
        Self::Unspecified
    }
}

/// Temporal cue used in utterance decomposition.
#[derive(Debug, Clone, Copy, Serialize, Deserialize, PartialEq, Eq)]
#[serde(rename_all = "snake_case")]
pub enum TemporalCue {
    Now,
    Before,
    Back,
    Unspecified,
}

impl TemporalCue {
    /// Classify a temporal cue from lexical markers.
    ///
    /// # Examples
    /// ```rust
    /// use ob_poc::repl::types_v2::TemporalCue;
    ///
    /// assert_eq!(TemporalCue::classify("go back to the deal"), TemporalCue::Back);
    /// ```
    pub fn classify(message: &str) -> Self {
        let msg = message.to_lowercase();
        if msg.contains("go back") || msg == "back" || msg.contains("return") {
            return Self::Back;
        }
        if msg.contains("before") || msg.contains("previous") {
            return Self::Before;
        }
        if msg.contains("now") || msg.contains("current") {
            return Self::Now;
        }
        Self::Unspecified
    }
}

// `WorkspaceKind::from_hint` relocated to ob-poc-envelope::session (see top-of-file note).

fn extract_subject_hint(message: &str) -> Option<String> {
    let subject_markers = [" for ", " on ", " about ", " regarding "];
    for marker in subject_markers {
        if let Some((_, tail)) = message.split_once(marker) {
            let trimmed = tail.trim();
            if !trimmed.is_empty() {
                return Some(trimmed.to_string());
            }
        }
    }
    None
}

// WorkspaceRegistryEntry relocated to ob-poc-envelope::session (see top-of-file note).

/// A candidate verb for clarification.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct VerbCandidate {
    pub verb_fqn: String,
    pub description: String,
    pub score: f32,
}

/// Progress of runbook execution.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ExecutionProgress {
    pub total_steps: usize,
    pub completed_steps: usize,
    pub failed_steps: usize,
    pub parked_steps: usize,
    pub current_step: Option<Uuid>,
    pub parked_entry_id: Option<Uuid>,
}

impl ExecutionProgress {
    pub fn new(total_steps: usize) -> Self {
        Self {
            total_steps,
            completed_steps: 0,
            failed_steps: 0,
            parked_steps: 0,
            current_step: None,
            parked_entry_id: None,
        }
    }
}

// ---------------------------------------------------------------------------
// UserInputV2 — conversational model, not typed forms
// ---------------------------------------------------------------------------

/// All input variants the v2 REPL accepts.
///
/// Design rule: conversation-first. All answers are accepted as free-text
/// `Message` input. Structured variants exist only for explicit UI actions
/// (button clicks, picker selections), never as correctness gates.
#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(tag = "type", rename_all = "snake_case")]
pub enum UserInputV2 {
    /// Free-text conversational input — the primary input mode.
    Message { content: String },

    /// User confirmed a sentence or runbook.
    Confirm,

    /// User rejected a proposed sentence.
    Reject,

    /// User edited a specific field on a runbook entry.
    Edit {
        step_id: Uuid,
        field: String,
        value: String,
    },

    /// Explicit REPL command.
    Command { command: ReplCommandV2 },

    /// User explicitly selected a pack by ID.
    SelectPack { pack_id: String },

    /// User selected a verb from disambiguation options.
    SelectVerb {
        verb_fqn: String,
        original_input: String,
    },

    /// User selected a proposal from the ranked list (Phase 3).
    SelectProposal { proposal_id: Uuid },

    /// User selected an entity to resolve an ambiguous reference.
    SelectEntity {
        ref_id: String,
        entity_id: Uuid,
        entity_name: String,
    },

    /// User selected a scope (client group / CBU set).
    SelectScope { group_id: Uuid, group_name: String },

    /// User selected a workspace after scope resolution.
    SelectWorkspace { workspace: WorkspaceKind },

    /// User selected a CBU structure constellation map.
    SelectConstellationMap { constellation_map: String },

    /// User approves a human-gated entry.
    Approve {
        entry_id: Uuid,
        approved_by: Option<String>,
    },

    /// User rejects a human-gated entry.
    RejectGate {
        entry_id: Uuid,
        reason: Option<String>,
    },
}

/// REPL commands available to the user.
#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum ReplCommandV2 {
    /// Execute the runbook.
    Run,
    /// Undo the last action.
    Undo,
    /// Redo the last undone action.
    Redo,
    /// Clear the runbook.
    Clear,
    /// Cancel the current operation.
    Cancel,
    /// Show session info.
    Info,
    /// Show help.
    Help,
    /// Remove a specific runbook entry.
    Remove(Uuid),
    /// Reorder runbook entries.
    Reorder(Vec<Uuid>),
    /// Disable a specific runbook entry (skip during execution).
    Disable(Uuid),
    /// Enable a previously disabled entry.
    Enable(Uuid),
    /// Toggle disabled state on an entry.
    Toggle(Uuid),
    /// Show status of parked entries.
    Status,
    /// Resume a parked entry (by entry_id) — for internal use after signal.
    Resume(Uuid),
}

// ---------------------------------------------------------------------------
// Tests
// ---------------------------------------------------------------------------

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_state_serialization_roundtrip() {
        let state = ReplStateV2::InPack {
            pack_id: "onboarding-request".to_string(),
            required_slots_remaining: vec!["products".to_string(), "jurisdiction".to_string()],
            last_proposal_id: Some(Uuid::new_v4()),
        };

        let json = serde_json::to_string(&state).unwrap();
        let deserialized: ReplStateV2 = serde_json::from_str(&json).unwrap();

        match deserialized {
            ReplStateV2::InPack {
                pack_id,
                required_slots_remaining,
                ..
            } => {
                assert_eq!(pack_id, "onboarding-request");
                assert_eq!(required_slots_remaining.len(), 2);
            }
            _ => panic!("Wrong state variant"),
        }
    }

    #[test]
    fn test_input_message_serialization() {
        let input = UserInputV2::Message {
            content: "Add IRS product".to_string(),
        };
        let json = serde_json::to_string(&input).unwrap();
        assert!(json.contains("\"type\":\"message\""));
        assert!(json.contains("Add IRS product"));
    }

    #[test]
    fn test_input_confirm_serialization() {
        let input = UserInputV2::Confirm;
        let json = serde_json::to_string(&input).unwrap();
        assert!(json.contains("\"type\":\"confirm\""));
    }

    #[test]
    fn test_input_select_pack_serialization() {
        let input = UserInputV2::SelectPack {
            pack_id: "onboarding-request".to_string(),
        };
        let json = serde_json::to_string(&input).unwrap();
        assert!(json.contains("\"type\":\"select_pack\""));
        assert!(json.contains("onboarding-request"));
    }

    #[test]
    fn instrument_matrix_defaults_to_trading_streetside() {
        let entry = WorkspaceKind::InstrumentMatrix.registry_entry();

        assert_eq!(entry.default_constellation_family, "trading_streetside");
        assert_eq!(entry.default_constellation_map, "trading.streetside");
        assert!(entry.constellation_families.contains(&"trading_streetside"));
    }

    #[test]
    fn test_input_command_run() {
        let input = UserInputV2::Command {
            command: ReplCommandV2::Run,
        };
        let json = serde_json::to_string(&input).unwrap();
        let deserialized: UserInputV2 = serde_json::from_str(&json).unwrap();
        match deserialized {
            UserInputV2::Command {
                command: ReplCommandV2::Run,
            } => {}
            _ => panic!("Wrong variant"),
        }
    }

    #[test]
    fn test_input_command_remove() {
        let id = Uuid::new_v4();
        let input = UserInputV2::Command {
            command: ReplCommandV2::Remove(id),
        };
        let json = serde_json::to_string(&input).unwrap();
        let deserialized: UserInputV2 = serde_json::from_str(&json).unwrap();
        match deserialized {
            UserInputV2::Command {
                command: ReplCommandV2::Remove(rid),
            } => assert_eq!(rid, id),
            _ => panic!("Wrong variant"),
        }
    }

    #[test]
    fn test_execution_progress() {
        let mut progress = ExecutionProgress::new(5);
        assert_eq!(progress.total_steps, 5);
        assert_eq!(progress.completed_steps, 0);

        progress.completed_steps = 3;
        progress.current_step = Some(Uuid::new_v4());
        assert_eq!(progress.completed_steps, 3);
    }

    #[test]
    fn test_all_state_variants_serialize() {
        let states: Vec<ReplStateV2> = vec![
            ReplStateV2::ScopeGate {
                pending_input: Some("allianz".to_string()),
                candidates: None,
            },
            ReplStateV2::JourneySelection {
                candidates: Some(vec![PackCandidate {
                    pack_id: "test".to_string(),
                    pack_name: "Test".to_string(),
                    description: "desc".to_string(),
                    score: 0.9,
                }]),
            },
            ReplStateV2::InPack {
                pack_id: "test".to_string(),
                required_slots_remaining: vec![],
                last_proposal_id: None,
            },
            ReplStateV2::Clarifying {
                question: "Which verb?".to_string(),
                candidates: vec![],
                original_input: "load".to_string(),
            },
            ReplStateV2::SentencePlayback {
                sentence: "Create Allianz Lux CBU".to_string(),
                verb: "cbu.create".to_string(),
                dsl: "(cbu.create :name \"Allianz Lux\")".to_string(),
                args: HashMap::new(),
            },
            ReplStateV2::RunbookEditing,
            ReplStateV2::Executing {
                runbook_id: Uuid::new_v4(),
                progress: ExecutionProgress::new(3),
            },
        ];

        for state in &states {
            let json = serde_json::to_string(state).unwrap();
            let _: ReplStateV2 = serde_json::from_str(&json).unwrap();
        }
    }
}
