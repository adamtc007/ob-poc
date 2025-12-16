//! Resolution API Types
//!
//! Types for entity resolution workflow - shared by server and WASM.
//!
//! ## Flow
//!
//! 1. POST /resolution/start - Extract unresolved refs from session DSL
//! 2. UI shows resolution panel with matches
//! 3. User/agent selects resolutions via /resolution/select
//! 4. POST /resolution/commit - Apply to AST and enable execution

use serde::{Deserialize, Serialize};
use std::collections::HashMap;

// ============================================================================
// RESOLUTION SESSION STATE
// ============================================================================

/// Resolution session state (returned from /resolution/start and other endpoints)
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ResolutionSessionResponse {
    /// Session ID (UUID as string)
    pub id: String,
    /// Resolution session ID (UUID as string)
    pub resolution_id: String,
    /// Current state
    pub state: ResolutionStateResponse,
    /// Refs needing user resolution
    pub unresolved: Vec<UnresolvedRefResponse>,
    /// Auto-resolved refs (exact match, reference data)
    pub auto_resolved: Vec<ResolvedRefResponse>,
    /// User-resolved refs
    pub resolved: Vec<ResolvedRefResponse>,
    /// Summary statistics
    pub summary: ResolutionSummary,
}

/// Resolution state enum
#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq)]
#[serde(rename_all = "snake_case")]
pub enum ResolutionStateResponse {
    /// User picking entities
    Resolving,
    /// All resolved, user reviewing
    Reviewing,
    /// Applied to AST
    Committed,
    /// Cancelled
    Cancelled,
}

/// Summary statistics for resolution progress
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ResolutionSummary {
    /// Total refs needing resolution
    pub total_refs: usize,
    /// Count resolved (auto + user)
    pub resolved_count: usize,
    /// Count with warnings
    pub warnings_count: usize,
    /// Count requiring review
    pub required_review_count: usize,
    /// True if all required refs resolved
    pub can_commit: bool,
}

// ============================================================================
// UNRESOLVED REFERENCE
// ============================================================================

/// A reference that needs resolution
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct UnresolvedRefResponse {
    /// Unique ref ID within this resolution session
    pub ref_id: String,
    /// Entity type (e.g., "cbu", "entity", "proper_person")
    pub entity_type: String,
    /// Entity subtype if applicable
    #[serde(default)]
    pub entity_subtype: Option<String>,
    /// Original search value from DSL
    pub search_value: String,
    /// Context about where this ref appears
    pub context: RefContext,
    /// Pre-fetched initial matches
    pub initial_matches: Vec<EntityMatchResponse>,
    /// Agent's suggested resolution (if confident)
    #[serde(default)]
    pub agent_suggestion: Option<EntityMatchResponse>,
    /// Reason for agent's suggestion
    #[serde(default)]
    pub suggestion_reason: Option<String>,
    /// Review requirement level
    pub review_requirement: ReviewRequirement,
    /// Discriminator fields from search schema (for UI)
    #[serde(default)]
    pub discriminator_fields: Vec<DiscriminatorField>,
}

/// Context about where a reference appears in DSL
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct RefContext {
    /// Statement index in DSL
    pub statement_index: usize,
    /// Verb (e.g., "cbu.assign-role")
    pub verb: String,
    /// Argument name (e.g., "entity-id")
    pub arg_name: String,
    /// DSL snippet for context
    #[serde(default)]
    pub dsl_snippet: Option<String>,
}

/// Discriminator field for search refinement
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct DiscriminatorField {
    /// Field name (e.g., "date_of_birth", "jurisdiction")
    pub name: String,
    /// Display label
    pub label: String,
    /// Selectivity (0.0-1.0, higher = more selective)
    pub selectivity: f32,
    /// Current value if known
    #[serde(default)]
    pub value: Option<String>,
}

/// Review requirement level
#[derive(Debug, Clone, Default, Serialize, Deserialize, PartialEq, Eq)]
#[serde(rename_all = "snake_case")]
pub enum ReviewRequirement {
    /// Auto-resolved, high confidence - can skip review
    #[default]
    Optional,
    /// Warnings present - review recommended
    Recommended,
    /// Low confidence, multiple close matches - must review
    Required,
}

// ============================================================================
// RESOLVED REFERENCE
// ============================================================================

/// A resolved reference
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ResolvedRefResponse {
    /// Unique ref ID
    pub ref_id: String,
    /// Entity type
    pub entity_type: String,
    /// Original search value
    pub original_search: String,
    /// Resolved primary key (UUID as string)
    pub resolved_key: String,
    /// Display name of resolved entity
    pub display: String,
    /// Key discriminators for display (e.g., jurisdiction, DOB)
    #[serde(default)]
    pub discriminators: HashMap<String, String>,
    /// Entity status (active, inactive, etc.)
    pub entity_status: EntityStatus,
    /// Warnings about this resolution
    #[serde(default)]
    pub warnings: Vec<ResolutionWarning>,
    /// Number of alternative matches
    pub alternative_count: usize,
    /// Confidence score (0.0-1.0)
    pub confidence: f32,
    /// Has user reviewed this resolution
    pub reviewed: bool,
    /// Was this changed from initial suggestion
    pub changed_from_original: bool,
    /// How this was resolved
    pub resolution_method: ResolutionMethod,
}

/// How a reference was resolved
#[derive(Debug, Clone, Default, Serialize, Deserialize, PartialEq, Eq)]
#[serde(rename_all = "snake_case")]
pub enum ResolutionMethod {
    /// Exact match (reference data, unique identifier)
    Exact,
    /// Auto-resolved with high confidence
    Auto,
    /// Agent suggestion accepted
    AgentSuggestion,
    /// User selected from options
    #[default]
    UserSelected,
    /// User searched and selected
    UserSearched,
}

/// Entity status
#[derive(Debug, Clone, Default, Serialize, Deserialize, PartialEq, Eq)]
#[serde(rename_all = "snake_case")]
pub enum EntityStatus {
    Active,
    Inactive,
    Pending,
    #[default]
    Unknown,
}

/// Warning about a resolution
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ResolutionWarning {
    /// Warning severity
    pub severity: WarningSeverity,
    /// Warning code
    pub code: String,
    /// Human-readable message
    pub message: String,
}

/// Warning severity level
#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq)]
#[serde(rename_all = "snake_case")]
pub enum WarningSeverity {
    Info,
    Warning,
    Error,
}

// ============================================================================
// ENTITY MATCH
// ============================================================================

/// A matching entity for selection
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct EntityMatchResponse {
    /// Entity ID (UUID as string)
    pub id: String,
    /// Display name
    pub display: String,
    /// Entity type
    pub entity_type: String,
    /// Match score (0.0-1.0)
    pub score: f32,
    /// Key discriminators
    #[serde(default)]
    pub discriminators: HashMap<String, String>,
    /// Entity status
    pub status: EntityStatus,
    /// Additional context
    #[serde(default)]
    pub context: Option<String>,
}

// ============================================================================
// API REQUESTS
// ============================================================================

/// Request to start resolution for a session's DSL
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct StartResolutionRequest {
    /// Optional: only resolve specific refs
    #[serde(default)]
    pub ref_ids: Option<Vec<String>>,
}

/// Request to search for entity matches
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ResolutionSearchRequest {
    /// Which ref we're searching for
    pub ref_id: String,
    /// Search query
    pub query: String,
    /// Optional discriminator values to refine search
    #[serde(default)]
    pub discriminators: HashMap<String, String>,
    /// Max results to return
    #[serde(default)]
    pub limit: Option<usize>,
}

/// Response from resolution search
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ResolutionSearchResponse {
    /// Matching entities
    pub matches: Vec<EntityMatchResponse>,
    /// Total count before limit
    pub total: usize,
    /// Whether results were truncated
    pub truncated: bool,
}

/// Request to select a resolution
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct SelectResolutionRequest {
    /// Which ref to resolve
    pub ref_id: String,
    /// Selected entity key (UUID as string)
    pub resolved_key: String,
}

/// Response from selecting a resolution
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct SelectResolutionResponse {
    pub success: bool,
    /// Updated resolution session state
    pub session: ResolutionSessionResponse,
}

/// Request to confirm (mark as reviewed) a resolution
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ConfirmResolutionRequest {
    /// Which ref to confirm
    pub ref_id: String,
}

/// Request to confirm all high-confidence resolutions
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ConfirmAllRequest {
    /// Only confirm refs with this minimum confidence
    #[serde(default)]
    pub min_confidence: Option<f32>,
}

/// Response from committing resolutions to AST
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct CommitResolutionResponse {
    pub success: bool,
    /// Updated DSL with resolved refs
    #[serde(default)]
    pub dsl_source: Option<String>,
    /// Message
    pub message: String,
    /// Errors if any
    #[serde(default)]
    pub errors: Vec<String>,
}

/// Response from cancelling resolution
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct CancelResolutionResponse {
    pub success: bool,
    pub message: String,
}

// ============================================================================
// AGENT INTEGRATION
// ============================================================================

/// Agent context info about current resolution
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ResolutionContextInfo {
    /// Resolution state
    pub state: ResolutionStateResponse,
    /// Count of unresolved refs
    pub unresolved_count: usize,
    /// Count of resolved refs
    pub resolved_count: usize,
    /// Brief summaries of unresolved refs (for agent context)
    pub unresolved_summaries: Vec<String>,
    /// Whether user can commit now
    pub can_commit: bool,
}

/// Chat response indicating resolution is required
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ResolutionRequiredPayload {
    /// Resolution session ID
    pub resolution_id: String,
    /// Count of refs needing resolution
    pub unresolved_count: usize,
    /// Agent's message about the resolution
    pub agent_message: String,
    /// Whether agent made suggestions
    pub suggestions_made: bool,
}

// ============================================================================
// IMPL HELPERS
// ============================================================================

impl Default for ResolutionSummary {
    fn default() -> Self {
        Self {
            total_refs: 0,
            resolved_count: 0,
            warnings_count: 0,
            required_review_count: 0,
            can_commit: true,
        }
    }
}

impl ResolutionSummary {
    pub fn new(
        total_refs: usize,
        resolved_count: usize,
        warnings_count: usize,
        required_review_count: usize,
    ) -> Self {
        Self {
            total_refs,
            resolved_count,
            warnings_count,
            required_review_count,
            can_commit: resolved_count == total_refs && required_review_count == 0,
        }
    }
}

// ============================================================================
// TESTS
// ============================================================================

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn resolution_state_serializes_snake_case() {
        let state = ResolutionStateResponse::Resolving;
        let json = serde_json::to_string(&state).unwrap();
        assert_eq!(json, r#""resolving""#);
    }

    #[test]
    fn review_requirement_serializes_snake_case() {
        let req = ReviewRequirement::Required;
        let json = serde_json::to_string(&req).unwrap();
        assert_eq!(json, r#""required""#);
    }

    #[test]
    fn resolution_session_roundtrip() {
        let session = ResolutionSessionResponse {
            id: "sess-123".to_string(),
            resolution_id: "res-456".to_string(),
            state: ResolutionStateResponse::Resolving,
            unresolved: vec![],
            auto_resolved: vec![],
            resolved: vec![],
            summary: ResolutionSummary::default(),
        };

        let json = serde_json::to_string(&session).unwrap();
        let parsed: ResolutionSessionResponse = serde_json::from_str(&json).unwrap();

        assert_eq!(parsed.id, session.id);
        assert_eq!(parsed.resolution_id, session.resolution_id);
        assert_eq!(parsed.state, session.state);
    }

    #[test]
    fn unresolved_ref_with_matches() {
        let unresolved = UnresolvedRefResponse {
            ref_id: "ref-1".to_string(),
            entity_type: "proper_person".to_string(),
            entity_subtype: None,
            search_value: "John Smith".to_string(),
            context: RefContext {
                statement_index: 0,
                verb: "cbu.assign-role".to_string(),
                arg_name: "entity-id".to_string(),
                dsl_snippet: None,
            },
            initial_matches: vec![EntityMatchResponse {
                id: "uuid-123".to_string(),
                display: "John Smith".to_string(),
                entity_type: "proper_person".to_string(),
                score: 0.95,
                discriminators: HashMap::from([("nationality".to_string(), "GB".to_string())]),
                status: EntityStatus::Active,
                context: None,
            }],
            agent_suggestion: None,
            suggestion_reason: None,
            review_requirement: ReviewRequirement::Optional,
            discriminator_fields: vec![],
        };

        let json = serde_json::to_string(&unresolved).unwrap();
        assert!(json.contains("John Smith"));
        assert!(json.contains("proper_person"));
    }

    #[test]
    fn summary_can_commit_logic() {
        // All resolved, no required reviews = can commit
        let summary = ResolutionSummary::new(5, 5, 0, 0);
        assert!(summary.can_commit);

        // Not all resolved = cannot commit
        let summary = ResolutionSummary::new(5, 3, 0, 0);
        assert!(!summary.can_commit);

        // Has required reviews = cannot commit
        let summary = ResolutionSummary::new(5, 5, 1, 2);
        assert!(!summary.can_commit);
    }
}
