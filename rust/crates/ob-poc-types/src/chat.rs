//! Chat API types
//!
//! Types for the agent chat interface including requests, responses,
//! debug/explainability info, DSL display segments, and verb match sources.

use serde::{Deserialize, Serialize};

use crate::{
    AgentCommand, AstStatement, DecisionPacket, DisambiguationRequest, DisambiguationResponse,
    IntentTierRequest, VerbDisambiguationRequest,
};

/// Chat message in conversation history
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ChatMessage {
    /// Unique message ID
    #[serde(default)]
    pub id: Option<String>,

    /// Message role
    pub role: ChatMessageRole,

    /// Message content
    pub content: String,

    /// Timestamp (ISO 8601 or DateTime)
    #[serde(default)]
    pub timestamp: Option<String>,

    /// Intents extracted from this message (if user message processed)
    #[serde(default)]
    pub intents: Option<serde_json::Value>,

    /// DSL generated from this message (if any) - server sends String, not DslState
    #[serde(default)]
    pub dsl: Option<String>,
}

/// Chat message role
#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum ChatMessageRole {
    User,
    Agent,
    System,
}

// ============================================================================
// CHAT API
// ============================================================================

/// Chat request from user - SINGLE source of truth for all chat endpoints
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ChatRequest {
    /// User's message
    pub message: String,
    /// Optional CBU context
    #[serde(default)]
    pub cbu_id: Option<String>, // UUID as string
    /// Optional disambiguation response (if responding to disambiguation request)
    #[serde(default)]
    pub disambiguation_response: Option<DisambiguationResponse>,
}

// ============================================================================
// VERB MATCH SOURCE (shared enum for evidence attribution)
// ============================================================================

/// Source of a verb match for explainability and debugging
#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq, Hash)]
#[serde(rename_all = "snake_case")]
pub enum VerbMatchSource {
    UserLearnedExact,
    UserLearnedSemantic,
    LearnedExact,
    LearnedSemantic,
    Semantic,
    DirectDsl,
    GlobalLearned,
    PatternEmbedding,
    Phonetic,
    Macro,
    /// Lexicon exact label match
    LexiconExact,
    /// Lexicon token overlap match
    LexiconToken,
    #[serde(other)]
    Unknown,
}

// ============================================================================
// CHAT DEBUG / EXPLAINABILITY (optional, gated by OB_CHAT_DEBUG=1)
// ============================================================================

/// Debug information included in chat response when OB_CHAT_DEBUG=1
#[derive(Debug, Clone, Serialize, Deserialize, Default)]
pub struct ChatDebugInfo {
    /// Verb matching details
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub verb_match: Option<VerbMatchDebug>,

    /// Entity resolution details (from EntityLinkingService)
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub entity_resolution: Option<EntityResolutionDebug>,
}

/// Entity resolution debug information from EntityLinkingService
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct EntityResolutionDebug {
    /// Snapshot hash used for resolution
    pub snapshot_hash: String,

    /// Entity count in snapshot
    pub entity_count: usize,

    /// Extracted entity mentions with candidates
    #[serde(default, skip_serializing_if = "Vec::is_empty")]
    pub mentions: Vec<EntityMentionDebug>,

    /// Dominant entity (if deterministic selection possible)
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub dominant_entity: Option<EntityCandidateDebug>,

    /// Expected entity kinds inferred from verb context
    #[serde(default, skip_serializing_if = "Vec::is_empty")]
    pub expected_kinds: Vec<String>,
}

/// A single entity mention with candidates
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct EntityMentionDebug {
    /// Character span in original utterance (start, end)
    pub span: (usize, usize),

    /// Original mention text
    pub text: String,

    /// Top candidates (sorted by score)
    pub candidates: Vec<EntityCandidateDebug>,

    /// Selected winner (if unambiguous)
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub selected_id: Option<String>,

    /// Confidence in selection
    pub confidence: f32,
}

/// A single entity candidate with score and evidence
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct EntityCandidateDebug {
    /// Entity UUID
    pub entity_id: String,

    /// Entity kind/type
    pub entity_kind: String,

    /// Canonical display name
    pub canonical_name: String,

    /// Match score (0.0-1.0)
    pub score: f32,

    /// Evidence explaining the score
    #[serde(default, skip_serializing_if = "Vec::is_empty")]
    pub evidence: Vec<String>,
}

/// Detailed verb matching information for debugging
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct VerbMatchDebug {
    /// The selected verb candidate (winner)
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub selected: Option<VerbCandidateDebug>,

    /// All candidates considered (including winner)
    #[serde(default, skip_serializing_if = "Vec::is_empty")]
    pub candidates: Vec<VerbCandidateDebug>,

    /// Selection policy used
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub policy: Option<VerbSelectionPolicyDebug>,
}

/// A single verb candidate with evidence
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct VerbCandidateDebug {
    pub verb: String,
    pub score: f32,

    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub primary_source: Option<VerbMatchSource>,

    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub matched_phrase: Option<String>,

    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub description: Option<String>,

    /// Evidence from multiple search channels
    #[serde(default, skip_serializing_if = "Vec::is_empty")]
    pub evidence: Vec<VerbEvidenceDebug>,
}

/// Evidence from a single search channel
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct VerbEvidenceDebug {
    pub source: VerbMatchSource,
    pub score: f32,
    pub matched_phrase: String,
}

/// Verb selection policy information
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct VerbSelectionPolicyDebug {
    pub algorithm: String,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub accept_threshold: Option<f32>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub ambiguity_margin: Option<f32>,
}

// ============================================================================
// VERB PROFILES (structured verb universe returned on every chat response)
// ============================================================================

/// Full profile of a verb available in the current session context.
/// Includes s-expression signature and typed argument details.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct VerbProfile {
    /// Fully-qualified name (e.g., "cbu.create")
    pub fqn: String,
    /// Domain (e.g., "cbu")
    pub domain: String,
    /// Human-readable description
    pub description: String,
    /// S-expression usage signature (e.g., "(cbu.create :name <string> [:kind <string>])")
    pub sexpr: String,
    /// Typed argument details
    pub args: Vec<VerbArgProfile>,
    /// Whether all preconditions are met for this verb
    pub preconditions_met: bool,
    /// Governance tier (e.g., "governed", "operational")
    pub governance_tier: String,
}

/// Profile of a single verb argument.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct VerbArgProfile {
    /// Argument name (e.g., "name", "kind")
    pub name: String,
    /// Type label (e.g., "string", "uuid", "Entity")
    pub arg_type: String,
    /// Whether this argument is required
    pub required: bool,
    /// Valid enum values, if constrained
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub valid_values: Option<Vec<String>>,
    /// Description of the argument
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub description: Option<String>,
}

/// Chat response from agent
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ChatResponse {
    /// Agent's text message
    pub message: String,

    /// DSL state - the SINGLE source of truth for all DSL content
    #[serde(default)]
    pub dsl: Option<crate::DslState>,

    /// Session state after this response
    #[serde(default)]
    pub session_state: SessionStateEnum,

    /// UI commands to execute (show CBU, highlight entity, etc.)
    #[serde(default)]
    pub commands: Option<Vec<AgentCommand>>,

    /// Disambiguation request if agent needs user to resolve ambiguous entities
    #[serde(default)]
    pub disambiguation_request: Option<DisambiguationRequest>,

    /// Verb disambiguation request if multiple verbs match user input
    /// When present, UI should show clickable buttons for verb selection
    /// User's selection = gold-standard training data for learning loop
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub verb_disambiguation: Option<VerbDisambiguationRequest>,

    /// Intent tier request if user input matches multiple action categories
    /// When present, UI should show tier selection before verb disambiguation
    /// This is a higher-level question: "Are you navigating, creating, modifying, etc.?"
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub intent_tier: Option<IntentTierRequest>,

    /// Unresolved entity references needing resolution (post-DSL parsing)
    /// When present, UI should trigger resolution modal before execution
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub unresolved_refs: Option<Vec<crate::resolution::UnresolvedRefResponse>>,

    /// Index of current ref being resolved (if in resolution state)
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub current_ref_index: Option<usize>,

    /// Hash of current DSL for resolution commit verification (Issue K)
    /// UI must pass this back when resolving refs to prevent stale commits
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub dsl_hash: Option<String>,

    /// Decision packet for client group or deal selection
    /// When present, UI should show decision prompt with choices
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub decision: Option<DecisionPacket>,

    /// Constrained verb universe for this session context.
    /// Populated on every response — same data as /commands but structured.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub available_verbs: Option<Vec<VerbProfile>>,

    /// SessionVerbSurface fingerprint for this turn.
    /// Format: "vs1:<sha256-hex>". Changes when the visible verb set changes.
    /// Enables UI to detect surface drift and refresh VerbBrowser.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub surface_fingerprint: Option<String>,
}

/// Verb surface response for the REST endpoint.
/// Returns the full SessionVerbSurface with governance metadata.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct VerbSurfaceResponse {
    /// Visible verbs with governance metadata
    pub verbs: Vec<VerbSurfaceEntry>,
    /// Total verbs in registry before filtering
    pub total_registry: usize,
    /// Final visible count
    pub final_count: usize,
    /// Surface fingerprint (format: "vs1:<sha256-hex>")
    pub surface_fingerprint: String,
    /// Fail policy applied
    pub fail_policy: String,
    /// Filter summary showing progressive narrowing at each stage
    pub filter_summary: VerbSurfaceFilterSummary,
    /// Excluded verbs with prune reasons (only if include_excluded=true)
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub excluded: Option<Vec<VerbSurfaceExcludedEntry>>,
}

/// A verb entry in the verb surface response.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct VerbSurfaceEntry {
    pub fqn: String,
    pub domain: String,
    pub action: String,
    pub description: String,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub governance_tier: Option<String>,
    pub lifecycle_eligible: bool,
    pub rank_boost: f64,
}

/// Filter summary showing how many verbs survived each stage.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct VerbSurfaceFilterSummary {
    pub total_registry: usize,
    pub after_agent_mode: usize,
    pub after_workflow: usize,
    pub after_semreg: usize,
    pub after_lifecycle: usize,
    pub after_actor: usize,
    pub final_count: usize,
}

/// An excluded verb with structured prune reasons.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct VerbSurfaceExcludedEntry {
    pub fqn: String,
    pub reasons: Vec<VerbSurfacePruneReason>,
}

/// A single prune reason explaining why a verb was excluded.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct VerbSurfacePruneReason {
    pub layer: String,
    pub reason: String,
}

/// Session state enum for typed responses - matches server's SessionState
#[derive(Debug, Clone, Default, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum SessionStateEnum {
    /// Just created, awaiting scope selection (client/CBU set)
    #[default]
    New,
    /// Scope is set (client/CBU set selected), ready for operations
    Scoped,
    PendingValidation,
    ReadyToExecute,
    Executing,
    Executed,
    Closed,
}

/// SSE stream event - tagged enum for discrimination
#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(tag = "type", rename_all = "snake_case")]
pub enum ChatStreamEvent {
    /// Text chunk from agent
    Chunk { content: String },
    /// DSL source generated
    Dsl { source: String },
    /// AST parsed from DSL
    Ast { statements: Vec<AstStatement> },
    /// Agent command (show CBU, highlight entity, etc.)
    Command { action: AgentCommand },
    /// Stream complete
    Done { session_id: String },
    /// Error occurred
    Error { message: String },
}

// ============================================================================
// DSL DISPLAY SEGMENTS (for rich rendering with inline binding info)
// ============================================================================

/// A segment of DSL text for rich rendering
/// The UI receives pre-segmented DSL and renders each segment appropriately
#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(tag = "type", rename_all = "snake_case")]
pub enum DslDisplaySegment {
    /// Plain text (keywords, punctuation, literals)
    Text { content: String },

    /// A binding reference (@symbol) with resolved info
    Binding {
        /// The symbol name (without @)
        symbol: String,
        /// Resolved display name (e.g., "Apex Capital")
        display_name: Option<String>,
        /// Entity type (e.g., "cbu", "proper_person")
        entity_type: Option<String>,
        /// Resolved UUID (if resolved)
        entity_id: Option<String>,
        /// Whether this binding is editable/clickable
        editable: bool,
        /// Byte offset in source for click handling
        source_offset: usize,
    },

    /// An unresolved entity reference that needs resolution
    UnresolvedRef {
        /// The search/display value from DSL
        search_value: String,
        /// Expected entity type from verb schema
        entity_type: String,
        /// Argument name (e.g., ":cbu-id")
        arg_name: String,
        /// Reference ID for resolution API
        ref_id: String,
        /// Byte offset in source
        source_offset: usize,
    },

    /// A comment
    Comment { content: String },

    /// Newline/whitespace (preserved for layout)
    Whitespace { content: String },
}

/// Enriched DSL for display - raw source plus segmented view
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct EnrichedDsl {
    /// Raw DSL source (for editing mode)
    pub source: String,
    /// Segmented view for rich rendering
    pub segments: Vec<DslDisplaySegment>,
    /// Summary of bindings used
    pub binding_summary: Vec<BindingSummary>,
    /// Whether all references are resolved
    pub fully_resolved: bool,
}

/// Summary of a binding for the context panel
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct BindingSummary {
    /// Symbol name (without @)
    pub symbol: String,
    /// Display name
    pub display_name: String,
    /// Entity type
    pub entity_type: String,
    /// UUID
    pub entity_id: String,
    /// Is this the active/primary binding (e.g., active_cbu)
    pub is_primary: bool,
}

// ============================================================================
// CHAT V2 (extended response with disambiguation)
// ============================================================================

/// Extended chat response that can include disambiguation
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ChatResponseV2 {
    /// Agent message
    pub message: String,
    /// Response type
    #[serde(flatten)]
    pub payload: ChatPayload,
    /// Session state
    pub session_state: String,
}

/// Chat response payload - either ready DSL or needs disambiguation
#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(tag = "status", rename_all = "snake_case")]
pub enum ChatPayload {
    /// DSL is ready (no ambiguity or already resolved)
    Ready {
        dsl_source: String,
        #[serde(default)]
        ast: Option<Vec<AstStatement>>,
        can_execute: bool,
        #[serde(default)]
        commands: Option<Vec<AgentCommand>>,
    },
    /// Needs user disambiguation before generating DSL
    NeedsDisambiguation {
        disambiguation: DisambiguationRequest,
    },
    /// Just a message, no DSL
    Message {
        #[serde(default)]
        commands: Option<Vec<AgentCommand>>,
    },
}
