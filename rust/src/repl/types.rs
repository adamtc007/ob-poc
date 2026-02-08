//! Shared types for REPL intent matching
//!
//! These types are the contract between IntentMatcher and consumers
//! (IntentService, ProposalEngine, OrchestratorV2, tests).
//!
//! V1-only types (ReplState, LedgerEntry, UserInput, ReplCommand, etc.)
//! were removed in Phase 6 when the V1 REPL was decommissioned.

use serde::{Deserialize, Serialize};
use uuid::Uuid;

// ============================================================================
// Intent Matching Types
// ============================================================================

/// Lightweight adapter DTO derived from [`ContextStack`].
///
/// `ContextStack` is the canonical context object (invariant P-1).
/// `MatchContext` exists solely for the [`IntentMatcher`] trait contract,
/// which requires a small, serializable subset of context for semantic
/// search and scoring.  The orchestrator builds `MatchContext` from
/// `ContextStack` via `build_match_context()` — never the reverse.
///
/// [`ContextStack`]: super::context_stack::ContextStack
/// [`IntentMatcher`]: super::intent_matcher::IntentMatcher
#[derive(Debug, Clone, Default, Serialize, Deserialize)]
pub struct MatchContext {
    /// Current client group (if set)
    #[serde(skip_serializing_if = "Option::is_none")]
    pub client_group_id: Option<Uuid>,

    /// Current client group name
    #[serde(skip_serializing_if = "Option::is_none")]
    pub client_group_name: Option<String>,

    /// Current scope context
    #[serde(skip_serializing_if = "Option::is_none")]
    pub scope: Option<ScopeContext>,

    /// Dominant entity from previous interactions
    #[serde(skip_serializing_if = "Option::is_none")]
    pub dominant_entity_id: Option<Uuid>,

    /// User ID (for personalized matching)
    #[serde(skip_serializing_if = "Option::is_none")]
    pub user_id: Option<Uuid>,

    /// Domain hint (e.g., "kyc", "trading")
    #[serde(skip_serializing_if = "Option::is_none")]
    pub domain_hint: Option<String>,

    /// Current bindings (symbol -> entity)
    #[serde(default)]
    pub bindings: Vec<(String, Uuid)>,
}

/// Scope context for matching
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ScopeContext {
    /// CBU IDs in scope
    pub cbu_ids: Vec<Uuid>,
    /// Scope description
    pub description: String,
}

/// Intent match result (returned from IntentMatcher)
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct IntentMatchResult {
    /// Match outcome
    pub outcome: MatchOutcome,

    /// Verb candidates (sorted by score, highest first)
    #[serde(default)]
    pub verb_candidates: Vec<VerbCandidate>,

    /// Entity mentions found in input
    #[serde(default)]
    pub entity_mentions: Vec<EntityMention>,

    /// Scope candidates (if scope selection needed)
    #[serde(skip_serializing_if = "Option::is_none")]
    pub scope_candidates: Option<Vec<ScopeCandidate>>,

    /// Generated DSL (if verb matched and args extracted)
    #[serde(skip_serializing_if = "Option::is_none")]
    pub generated_dsl: Option<String>,

    /// Unresolved entity references
    #[serde(default)]
    pub unresolved_refs: Vec<UnresolvedRef>,

    /// Debug information
    #[serde(skip_serializing_if = "Option::is_none")]
    pub debug: Option<MatchDebugInfo>,
}

/// Match outcome
#[derive(Debug, Clone, Serialize, Deserialize, PartialEq)]
#[serde(tag = "type", rename_all = "snake_case")]
pub enum MatchOutcome {
    /// Clear winner found
    Matched { verb: String, confidence: f32 },

    /// Multiple verbs matched with similar scores
    Ambiguous { margin: f32 },

    /// Need user to select scope/client group
    NeedsScopeSelection,

    /// Entity references need resolution
    NeedsEntityResolution,

    /// Need user to select client group (session context)
    NeedsClientGroup { options: Vec<ClientGroupOption> },

    /// Need user to select intent tier
    NeedsIntentTier { options: Vec<IntentTierOption> },

    /// No matching verb found
    NoMatch { reason: String },

    /// Direct DSL input (bypass intent matching)
    DirectDsl { source: String },
}

/// Verb candidate from search
#[derive(Debug, Clone, Serialize, Deserialize, PartialEq)]
pub struct VerbCandidate {
    /// Fully qualified verb name (e.g., "session.load-galaxy")
    pub verb_fqn: String,
    /// Verb description
    pub description: String,
    /// Match score (0.0-1.0)
    pub score: f32,
    /// Example phrase
    #[serde(skip_serializing_if = "Option::is_none")]
    pub example: Option<String>,
    /// Domain
    #[serde(skip_serializing_if = "Option::is_none")]
    pub domain: Option<String>,
}

/// Entity mention found in input
#[derive(Debug, Clone, Serialize, Deserialize, PartialEq)]
pub struct EntityMention {
    /// Text span in original input
    pub text: String,
    /// Character offset start
    pub start: usize,
    /// Character offset end
    pub end: usize,
    /// Resolved entity ID (if unambiguous)
    #[serde(skip_serializing_if = "Option::is_none")]
    pub entity_id: Option<Uuid>,
    /// Resolved entity name
    #[serde(skip_serializing_if = "Option::is_none")]
    pub entity_name: Option<String>,
    /// Entity kind (company, person, etc.)
    #[serde(skip_serializing_if = "Option::is_none")]
    pub entity_kind: Option<String>,
    /// Confidence score
    pub confidence: f32,
}

/// Scope candidate for selection
#[derive(Debug, Clone, Serialize, Deserialize, PartialEq)]
pub struct ScopeCandidate {
    /// Unique ID for this option
    pub id: String,
    /// Display name
    pub name: String,
    /// Description
    pub description: String,
    /// Number of CBUs in this scope
    #[serde(skip_serializing_if = "Option::is_none")]
    pub cbu_count: Option<usize>,
}

/// Unresolved entity reference
#[derive(Debug, Clone, Serialize, Deserialize, PartialEq)]
pub struct UnresolvedRef {
    /// Reference ID (for tracking)
    pub ref_id: String,
    /// Original text
    pub text: String,
    /// Expected entity type
    #[serde(skip_serializing_if = "Option::is_none")]
    pub expected_kind: Option<String>,
    /// Candidate entities
    pub candidates: Vec<EntityCandidate>,
}

/// Entity candidate for resolution
#[derive(Debug, Clone, Serialize, Deserialize, PartialEq)]
pub struct EntityCandidate {
    /// Entity ID
    pub entity_id: Uuid,
    /// Entity name
    pub name: String,
    /// Entity kind
    #[serde(skip_serializing_if = "Option::is_none")]
    pub kind: Option<String>,
    /// Match score
    pub score: f32,
}

/// Client group option for selection
#[derive(Debug, Clone, Serialize, Deserialize, PartialEq)]
pub struct ClientGroupOption {
    /// Group ID
    pub group_id: Uuid,
    /// Group name
    pub name: String,
    /// Description
    #[serde(skip_serializing_if = "Option::is_none")]
    pub description: Option<String>,
    /// Number of CBUs in this group
    #[serde(skip_serializing_if = "Option::is_none")]
    pub cbu_count: Option<usize>,
}

/// Intent tier option (high-level categorization)
#[derive(Debug, Clone, Serialize, Deserialize, PartialEq)]
pub struct IntentTierOption {
    /// Option ID
    pub id: String,
    /// Label (e.g., "Navigate & View")
    pub label: String,
    /// Description
    pub description: String,
    /// Hint about related verbs
    #[serde(skip_serializing_if = "Option::is_none")]
    pub hint: Option<String>,
    /// Number of verbs in this category
    #[serde(skip_serializing_if = "Option::is_none")]
    pub verb_count: Option<usize>,
}

/// Debug information for intent matching
#[derive(Debug, Clone, Default, Serialize, Deserialize)]
pub struct MatchDebugInfo {
    /// Time spent on each stage (ms)
    #[serde(default)]
    pub timing: Vec<(String, u64)>,
    /// Search tier that produced the match
    #[serde(skip_serializing_if = "Option::is_none")]
    pub search_tier: Option<String>,
    /// Entity linking details
    #[serde(skip_serializing_if = "Option::is_none")]
    pub entity_linking: Option<serde_json::Value>,
    /// Additional notes
    #[serde(default)]
    pub notes: Vec<String>,
}
