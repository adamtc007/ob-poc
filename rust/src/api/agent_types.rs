//! Request and response types for the agent REST API.
//!
//! Extracted from agent_routes.rs for readability.

use serde::{Deserialize, Serialize};
use uuid::Uuid;

use crate::dsl_v2::{AtomicExecutionResult, BestEffortExecutionResult};
use crate::session::{
    MessageRole, SessionState, SubSessionType, UnifiedSession, UnresolvedRefInfo,
};

// ============================================================================
// DSL Request/Response Types
// ============================================================================

#[derive(Debug, Deserialize)]
pub struct ValidateDslRequest {
    pub dsl: String,
}

#[derive(Debug, Deserialize)]
pub struct GenerateDslRequest {
    pub instruction: String,
    pub domain: Option<String>,
}

#[derive(Debug, Serialize)]
pub struct GenerateDslResponse {
    pub dsl: Option<String>,
    pub explanation: Option<String>,
    pub error: Option<String>,
}

// ============================================================================
// Batch Operations Request/Response Types
// ============================================================================

/// Request to add products to multiple CBUs (server-side DSL generation)
#[derive(Debug, Deserialize)]
pub struct BatchAddProductsRequest {
    /// CBU IDs to add products to
    pub cbu_ids: Vec<Uuid>,
    /// Product codes to add (e.g., ["CUSTODY", "FUND_ACCOUNTING"])
    pub products: Vec<String>,
}

/// Result of adding a product to a single CBU
#[derive(Debug, Serialize)]
pub struct BatchProductResult {
    pub cbu_id: Uuid,
    pub product: String,
    pub success: bool,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub error: Option<String>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub services_added: Option<i32>,
}

/// Response from batch add products
#[derive(Debug, Serialize)]
pub struct BatchAddProductsResponse {
    pub total_operations: usize,
    pub success_count: usize,
    pub failure_count: usize,
    pub duration_ms: u64,
    pub results: Vec<BatchProductResult>,
}

// ============================================================================
// Validation Types
// ============================================================================

#[derive(Debug, Clone, Serialize)]
pub struct ValidationResult {
    pub valid: bool,
    pub errors: Vec<ValidationError>,
    pub warnings: Vec<String>,
}

#[derive(Debug, Clone, Serialize)]
pub struct ValidationError {
    pub line: Option<usize>,
    pub column: Option<usize>,
    pub message: String,
    pub suggestion: Option<String>,
}

// ============================================================================
// Domain/Vocabulary Types
// ============================================================================

#[derive(Debug, Serialize)]
pub struct DomainsResponse {
    pub domains: Vec<DomainInfo>,
    pub total_verbs: usize,
}

#[derive(Debug, Serialize)]
pub struct DomainInfo {
    pub name: String,
    pub description: String,
    pub verb_count: usize,
}

#[derive(Debug, Deserialize)]
pub struct VocabQuery {
    pub domain: Option<String>,
}

#[derive(Debug, Serialize)]
pub struct VocabResponse {
    pub verbs: Vec<VerbInfo>,
}

#[derive(Debug, Serialize)]
pub struct VerbInfo {
    pub domain: String,
    pub name: String,
    pub full_name: String,
    pub description: String,
    pub required_args: Vec<String>,
    pub optional_args: Vec<String>,
}

// ============================================================================
// Health Check Types
// ============================================================================

#[derive(Debug, Serialize)]
pub struct HealthResponse {
    pub status: String,
    pub version: String,
    pub verb_count: usize,
    pub domain_count: usize,
}

// ============================================================================
// Completion Request/Response Types (LSP-style via EntityGateway)
// ============================================================================

/// Request for entity completion
#[derive(Debug, Deserialize)]
pub struct CompleteRequest {
    /// The type of entity to complete: "cbu", "entity", "product", "role", "jurisdiction", etc.
    pub entity_type: String,
    /// The search query (partial text to match)
    pub query: String,
    /// Maximum number of results (default 10)
    #[serde(default = "default_limit")]
    pub limit: i32,
}

pub fn default_limit() -> i32 {
    10
}

/// A single completion item
#[derive(Debug, Serialize)]
pub struct CompletionItem {
    /// The value to insert (UUID or code)
    pub value: String,
    /// Display label for the completion
    pub label: String,
    /// Additional detail (e.g., entity type, jurisdiction)
    pub detail: Option<String>,
    /// Relevance score (0.0-1.0)
    pub score: f32,
}

/// Response with completion items
#[derive(Debug, Serialize)]
pub struct CompleteResponse {
    pub items: Vec<CompletionItem>,
    pub total: usize,
}

// ============================================================================
// Session Watch Types (Long-Polling)
// ============================================================================

/// Query parameters for session watch endpoint
#[derive(Debug, Deserialize)]
pub struct WatchQuery {
    /// Timeout in milliseconds (default 30000, max 60000)
    #[serde(default = "default_watch_timeout")]
    pub timeout_ms: u64,
}

pub fn default_watch_timeout() -> u64 {
    30000
}

/// Query parameters for verb surface endpoint
#[derive(Debug, Deserialize)]
pub struct VerbSurfaceQuery {
    /// Filter to specific domain (e.g., "kyc", "cbu")
    #[serde(default)]
    pub domain: Option<String>,
    /// Include excluded verbs with prune reasons
    #[serde(default)]
    pub include_excluded: bool,
}

/// Response from session watch endpoint
#[derive(Debug, Serialize)]
pub struct WatchResponse {
    /// Session ID
    pub session_id: Uuid,
    /// Version number (incremented on each update)
    pub version: u64,
    /// Current scope path as string
    pub scope_path: String,
    /// Whether struct_mass has been computed
    pub has_mass: bool,
    /// Current effective view mode (if set)
    pub view_mode: Option<String>,
    /// Active CBU ID (if bound)
    pub active_cbu_id: Option<Uuid>,
    /// Timestamp of last update (RFC3339)
    pub updated_at: String,
    /// Whether this is the initial snapshot (no wait) or a change notification
    pub is_initial: bool,
    /// Session scope type (galaxy, book, cbu, jurisdiction, neighborhood, empty)
    pub scope_type: Option<String>,
    /// Whether scope data is fully loaded
    pub scope_loaded: bool,
}

impl WatchResponse {
    pub fn from_snapshot(
        snapshot: &crate::api::session_manager::SessionSnapshot,
        is_initial: bool,
    ) -> Self {
        // Extract scope type string from GraphScope
        let scope_type = snapshot.scope_definition.as_ref().map(|s| match s {
            crate::graph::GraphScope::Empty => "empty".to_string(),
            crate::graph::GraphScope::SingleCbu { .. } => "cbu".to_string(),
            crate::graph::GraphScope::Book { .. } => "book".to_string(),
            crate::graph::GraphScope::Jurisdiction { .. } => "jurisdiction".to_string(),
            crate::graph::GraphScope::EntityNeighborhood { .. } => "neighborhood".to_string(),
            crate::graph::GraphScope::Custom { .. } => "custom".to_string(),
        });

        Self {
            session_id: snapshot.session_id,
            version: snapshot.version,
            scope_path: snapshot.scope_path.clone(),
            has_mass: snapshot.has_mass,
            view_mode: snapshot.view_mode.clone(),
            active_cbu_id: snapshot.active_cbu_id,
            updated_at: snapshot.updated_at.to_rfc3339(),
            is_initial,
            scope_type,
            scope_loaded: snapshot.scope_loaded,
        }
    }
}

// ============================================================================
// Entity Reference Resolution Types
// ============================================================================

/// Identifies a specific EntityRef in the AST
#[derive(Debug, Deserialize)]
pub struct RefId {
    /// Index of statement in AST (0-based)
    pub statement_index: usize,
    /// Argument key containing the EntityRef (e.g., "entity-id")
    pub arg_key: String,
}

/// Request to resolve an EntityRef in the session AST
#[derive(Debug, Deserialize)]
pub struct ResolveRefRequest {
    /// Session containing the AST
    pub session_id: Uuid,
    /// Location of the EntityRef to resolve
    pub ref_id: RefId,
    /// Primary key from entity search (UUID or code)
    pub resolved_key: String,
}

/// Statistics about EntityRef resolution in the AST
#[derive(Debug, Serialize)]
pub struct ResolutionStats {
    /// Total EntityRef nodes in AST
    pub total_refs: i32,
    /// Remaining unresolved refs
    pub unresolved_count: i32,
}

// ============================================================================
// Span-Based Resolution Types (Issue K)
// ============================================================================

/// Request to resolve an EntityRef by span-based ref_id (Issue K)
///
/// Uses span-based ref_id format ("stmt_idx:start-end") for precise targeting
/// of refs in lists and maps. Includes dsl_hash to prevent stale commits.
#[derive(Debug, Deserialize)]
pub struct ResolveByRefIdRequest {
    /// Session containing the AST
    pub session_id: Uuid,
    /// Span-based ref_id (e.g., "0:15-30")
    pub ref_id: String,
    /// Primary key from entity search (UUID)
    pub resolved_key: String,
    /// Hash of DSL this resolution applies to (prevents race conditions)
    pub dsl_hash: String,
}

/// Response from resolving by ref_id (Issue K)
#[derive(Debug, Serialize)]
pub struct ResolveByRefIdResponse {
    /// Whether the update succeeded
    pub success: bool,
    /// Updated DSL with resolved ref
    pub dsl: String,
    /// New hash for the updated DSL
    pub dsl_hash: String,
    /// Remaining unresolved refs (so UI can continue without round-trip)
    pub remaining_unresolved: Vec<RemainingUnresolvedRef>,
    /// Whether all refs are now resolved
    pub fully_resolved: bool,
    /// Error message if failed
    #[serde(skip_serializing_if = "Option::is_none")]
    pub error: Option<String>,
}

/// Info about an unresolved ref (for ResolveByRefIdResponse)
#[derive(Debug, Clone, Serialize)]
pub struct RemainingUnresolvedRef {
    /// Argument key (e.g., "entity-id")
    pub param_name: String,
    /// Search text (e.g., "John Smith")
    pub search_value: String,
    /// Entity type (e.g., "entity", "cbu")
    pub entity_type: String,
    /// Search column (e.g., "name")
    #[serde(skip_serializing_if = "Option::is_none")]
    pub search_column: Option<String>,
    /// Span-based ref_id (e.g., "0:15-30")
    pub ref_id: String,
}

/// Response from resolving an EntityRef
#[derive(Debug, Serialize)]
pub struct ResolveRefResponse {
    /// Whether the update succeeded
    pub success: bool,
    /// DSL source re-rendered from updated AST (DSL + AST are a tuple pair)
    pub dsl_source: Option<String>,
    /// Full refreshed AST with updated triplet
    pub ast: Option<Vec<crate::dsl_v2::ast::Statement>>,
    /// Resolution statistics
    pub resolution_stats: ResolutionStats,
    /// True if all refs resolved (ready to execute)
    pub can_execute: bool,
    /// Error message if failed
    pub error: Option<String>,
    /// Error code for programmatic handling
    pub code: Option<String>,
}

// ============================================================================
// Onboarding Request/Response Types
// ============================================================================

/// Request to generate onboarding DSL from natural language
#[derive(Debug, Deserialize)]
pub struct OnboardingRequest {
    /// Natural language description of the onboarding request
    pub description: String,
    /// Whether to execute the DSL after generation
    #[serde(default)]
    pub execute: bool,
}

/// Response from onboarding DSL generation
#[derive(Debug, Serialize)]
pub struct OnboardingResponse {
    /// Generated DSL code
    pub dsl: Option<String>,
    /// Explanation of what was generated
    pub explanation: Option<String>,
    /// Validation result
    pub validation: Option<ValidationResult>,
    /// Execution result (if execute=true)
    pub execution: Option<OnboardingExecutionResult>,
    /// Error message if generation failed
    pub error: Option<String>,
}

/// Result of executing onboarding DSL
#[derive(Debug, Serialize)]
pub struct OnboardingExecutionResult {
    pub success: bool,
    pub cbu_id: Option<Uuid>,
    pub resource_count: usize,
    pub delivery_count: usize,
    pub errors: Vec<String>,
}

/// Outcome of DSL execution - either atomic (all-or-nothing) or best-effort (partial success)
///
/// This enum captures the execution strategy result, allowing the caller to handle
/// different outcomes appropriately (e.g., rollback vs partial success).
#[derive(Debug)]
pub(crate) enum ExecutionOutcome {
    /// Atomic execution result (all steps in single transaction)
    Atomic(AtomicExecutionResult),
    /// Best-effort execution result (continues on failure)
    BestEffort(BestEffortExecutionResult),
}

// ============================================================================
// Sub-Session Types
// ============================================================================

/// Request to create a sub-session
#[derive(Debug, Deserialize)]
pub struct CreateSubSessionRequest {
    /// Type of sub-session to create
    pub session_type: CreateSubSessionType,
}

/// Sub-session type for API (simplified for JSON)
#[derive(Debug, Deserialize)]
#[serde(tag = "type", rename_all = "snake_case")]
pub enum CreateSubSessionType {
    /// Resolution sub-session with unresolved refs
    Resolution {
        /// Unresolved refs to resolve
        unresolved_refs: Vec<UnresolvedRefInfo>,
        /// Parent DSL statement index
        parent_dsl_index: usize,
    },
    /// Research sub-session
    Research {
        /// Target entity ID (optional)
        target_entity_id: Option<Uuid>,
        /// Research type
        research_type: String,
    },
    /// Review sub-session
    Review {
        /// DSL to review
        pending_dsl: String,
    },
}

/// Response from creating a sub-session
#[derive(Debug, Serialize)]
pub struct CreateSubSessionResponse {
    /// New sub-session ID
    pub session_id: Uuid,
    /// Parent session ID
    pub parent_id: Uuid,
    /// Inherited symbol names (for display)
    pub inherited_symbols: Vec<String>,
    /// Sub-session type
    pub session_type: String,
}

/// Response for sub-session state
#[derive(Debug, Serialize)]
pub struct SubSessionStateResponse {
    pub session_id: Uuid,
    pub parent_id: Option<Uuid>,
    pub session_type: String,
    pub state: String,
    pub messages: Vec<SubSessionMessage>,
    /// Resolution-specific state
    #[serde(skip_serializing_if = "Option::is_none")]
    pub resolution: Option<ResolutionState>,
}

#[derive(Debug, Serialize)]
pub struct SubSessionMessage {
    pub role: String,
    pub content: String,
    pub timestamp: chrono::DateTime<chrono::Utc>,
}

#[derive(Debug, Serialize)]
pub struct ResolutionState {
    pub total_refs: usize,
    pub current_index: usize,
    pub resolved_count: usize,
    pub current_ref: Option<UnresolvedRefInfo>,
    pub pending_refs: Vec<UnresolvedRefInfo>,
}

impl SubSessionStateResponse {
    pub fn from_session(session: &UnifiedSession) -> Self {
        let session_type = match &session.sub_session_type {
            SubSessionType::Root => "root",
            SubSessionType::Resolution(_) => "resolution",
            SubSessionType::Research(_) => "research",
            SubSessionType::Review(_) => "review",
            SubSessionType::Correction(_) => "correction",
        }
        .to_string();

        let state = match session.state {
            SessionState::New => "new",
            SessionState::Scoped => "scoped",
            SessionState::PendingValidation => "pending_validation",
            SessionState::ReadyToExecute => "ready_to_execute",
            SessionState::Executing => "executing",
            SessionState::Executed => "executed",
            SessionState::Closed => "closed",
        }
        .to_string();

        let messages = session
            .messages
            .iter()
            .map(|m| SubSessionMessage {
                role: match m.role {
                    MessageRole::User => "user",
                    MessageRole::Agent => "agent",
                    MessageRole::System => "system",
                }
                .to_string(),
                content: m.content.clone(),
                timestamp: m.timestamp,
            })
            .collect();

        let resolution = if let SubSessionType::Resolution(r) = &session.sub_session_type {
            Some(ResolutionState {
                total_refs: r.unresolved_refs.len(),
                current_index: r.current_ref_index,
                resolved_count: r.resolutions.len(),
                current_ref: r.unresolved_refs.get(r.current_ref_index).cloned(),
                pending_refs: r
                    .unresolved_refs
                    .iter()
                    .skip(r.current_ref_index + 1)
                    .cloned()
                    .collect(),
            })
        } else {
            None
        };

        Self {
            session_id: session.id,
            parent_id: session.parent_session_id,
            session_type,
            state,
            messages,
            resolution,
        }
    }
}

/// Request for sub-session chat
#[derive(Debug, Deserialize)]
pub struct SubSessionChatRequest {
    pub message: String,
}

/// Request to complete a resolution sub-session
#[derive(Debug, Deserialize)]
pub struct CompleteSubSessionRequest {
    /// Whether to apply resolutions to parent
    #[serde(default = "default_true")]
    pub apply: bool,
}

pub fn default_true() -> bool {
    true
}

/// Response from completing a sub-session
#[derive(Debug, Serialize)]
pub struct CompleteSubSessionResponse {
    pub success: bool,
    pub resolutions_applied: usize,
    pub message: String,
}

// ============================================================================
// Binding/Focus/ViewMode Types
// ============================================================================

/// Request to set a binding in a session
#[derive(Debug, Deserialize)]
pub struct SetBindingRequest {
    /// The binding name (without @)
    pub name: String,
    /// The UUID to bind (accepts string for TypeScript compat)
    #[serde(deserialize_with = "deserialize_uuid_string")]
    pub id: Uuid,
    /// Entity type (e.g., "cbu", "entity", "case")
    pub entity_type: String,
    /// Human-readable display name
    pub display_name: String,
}

/// Deserialize UUID from string
pub fn deserialize_uuid_string<'de, D>(deserializer: D) -> Result<Uuid, D::Error>
where
    D: serde::Deserializer<'de>,
{
    let s: String = serde::Deserialize::deserialize(deserializer)?;
    Uuid::parse_str(&s).map_err(serde::de::Error::custom)
}

/// Response from setting a binding
#[derive(Debug, Serialize)]
pub struct SetBindingResponse {
    pub success: bool,
    pub binding_name: String,
    pub bindings: std::collections::HashMap<String, Uuid>,
}

/// Request to set stage focus in a session
#[derive(Debug, Deserialize)]
pub struct SetFocusRequest {
    /// The stage code to focus on (e.g., "KYC_REVIEW")
    /// Pass None or empty string to clear focus
    #[serde(default)]
    pub stage_code: Option<String>,
}

/// Response from setting stage focus
#[derive(Debug, Serialize)]
pub struct SetFocusResponse {
    pub success: bool,
    /// The stage that is now focused (None if cleared)
    pub stage_code: Option<String>,
    /// Stage name for display
    pub stage_name: Option<String>,
    /// Verbs relevant to this stage (for agent filtering)
    pub relevant_verbs: Vec<String>,
}

// ============================================================================
// DSL Parse Types
// ============================================================================

/// Request to parse DSL source into AST
#[derive(Debug, Deserialize)]
pub struct ParseDslRequest {
    /// DSL source text to parse
    pub dsl: String,
    /// Optional session ID to store the parsed AST
    pub session_id: Option<Uuid>,
}

/// A missing required argument
#[derive(Debug, Clone, Serialize)]
pub struct MissingArg {
    /// Statement index (0-based)
    pub statement_index: usize,
    /// Argument name (e.g., "name", "jurisdiction")
    pub arg_name: String,
    /// Verb that requires this arg
    pub verb: String,
}

#[derive(Debug, Serialize)]
pub struct ParseDslResponse {
    /// Whether parsing succeeded
    pub success: bool,
    /// Pipeline stage reached
    pub stage: PipelineStage,
    /// DSL source (echoed back)
    pub dsl_source: String,
    /// Parsed AST (if parse succeeded)
    pub ast: Option<Vec<crate::dsl_v2::ast::Statement>>,
    /// Unresolved EntityRefs requiring resolution
    pub unresolved_refs: Vec<UnresolvedRef>,
    /// Missing required arguments (from CSG validation)
    pub missing_args: Vec<MissingArg>,
    /// Validation errors (non-missing-arg errors)
    pub validation_errors: Vec<String>,
    /// Parse error (if failed)
    pub error: Option<String>,
}

// ============================================================================
// Discriminator Types
// ============================================================================

/// Request to parse natural language into discriminators
#[derive(Debug, Deserialize)]
pub struct ParseDiscriminatorsRequest {
    /// The natural language input to parse
    pub input: String,
    /// Optional entity type context
    pub entity_type: Option<String>,
}

/// Parsed discriminators for entity resolution
#[derive(Debug, Serialize, Default)]
pub struct ParsedDiscriminators {
    /// Nationality code (e.g., "GB", "US")
    #[serde(skip_serializing_if = "Option::is_none")]
    pub nationality: Option<String>,
    /// Year of birth
    #[serde(skip_serializing_if = "Option::is_none")]
    pub dob_year: Option<i32>,
    /// Full date of birth
    #[serde(skip_serializing_if = "Option::is_none")]
    pub dob: Option<String>,
    /// Role (e.g., "DIRECTOR", "UBO")
    #[serde(skip_serializing_if = "Option::is_none")]
    pub role: Option<String>,
    /// Associated entity name
    #[serde(skip_serializing_if = "Option::is_none")]
    pub associated_entity: Option<String>,
    /// Jurisdiction code
    #[serde(skip_serializing_if = "Option::is_none")]
    pub jurisdiction: Option<String>,
    /// Selection index (e.g., "first", "second", "1", "2")
    #[serde(skip_serializing_if = "Option::is_none")]
    pub selection_index: Option<usize>,
}

/// Response from discriminator parsing
#[derive(Debug, Serialize)]
pub struct ParseDiscriminatorsResponse {
    pub success: bool,
    pub discriminators: ParsedDiscriminators,
    /// Whether input appears to be a selection (number or ordinal)
    pub is_selection: bool,
    /// The original input
    pub input: String,
    /// Unrecognized parts of the input
    pub unrecognized: Vec<String>,
}

// ============================================================================
// Entity Mention Extraction Types
// ============================================================================

/// Request for entity mention extraction
#[derive(Debug, Deserialize)]
pub struct ExtractEntitiesRequest {
    /// The utterance to extract entity mentions from
    pub utterance: String,
    /// Optional: limit entity kinds to these values (e.g., ["company", "fund"])
    #[serde(default)]
    pub expected_kinds: Option<Vec<String>>,
    /// Optional: context concepts for boosting (e.g., ["otc", "trading"])
    #[serde(default)]
    pub context_concepts: Option<Vec<String>>,
    /// Maximum candidates per mention (default: 5)
    #[serde(default = "default_mention_limit")]
    pub limit: usize,
}

pub fn default_mention_limit() -> usize {
    5
}

/// A candidate entity match
#[derive(Debug, Serialize)]
pub struct EntityCandidateResponse {
    pub entity_id: String,
    pub entity_kind: String,
    pub canonical_name: String,
    pub score: f32,
    pub evidence: Vec<EvidenceResponse>,
}

/// Evidence for a match (stable wire format)
#[derive(Debug, Serialize)]
pub struct EvidenceResponse {
    pub kind: String,
    pub details: serde_json::Value,
}

/// A single entity mention extracted from the utterance
#[derive(Debug, Serialize)]
pub struct EntityMentionResponse {
    /// Character span in original utterance (start, end)
    pub span: (usize, usize),
    /// The text that was matched
    pub text: String,
    /// Candidate entities (sorted by score)
    pub candidates: Vec<EntityCandidateResponse>,
    /// Selected entity ID (if unambiguous)
    pub selected_id: Option<String>,
    /// Confidence in selection
    pub confidence: f32,
}

/// Response from entity mention extraction
#[derive(Debug, Serialize)]
pub struct ExtractEntitiesResponse {
    /// Snapshot metadata for cache invalidation
    pub snapshot_hash: String,
    pub snapshot_version: u32,
    pub entity_count: usize,
    /// Extracted mentions
    pub mentions: Vec<EntityMentionResponse>,
    /// Dominant entity (if a clear winner exists across mentions)
    pub dominant_entity: Option<EntityCandidateResponse>,
    /// Dominant entity kind (for verb boosting)
    pub dominant_kind: Option<String>,
}

// ============================================================================
// Additional Types
// ============================================================================

#[derive(Debug, Deserialize)]
pub struct ExecuteDslRequest {
    /// DSL source to execute. If None/missing, uses session's assembled_dsl.
    #[serde(default)]
    pub dsl: Option<String>,
}

// NOTE: Direct /execute endpoint removed - use /api/session/:id/execute instead
// All DSL execution now requires a session for proper binding persistence and audit trail

/// Pipeline stage indicating where processing stopped
#[derive(Debug, Clone, Serialize, Deserialize, PartialEq)]
#[serde(rename_all = "lowercase")]
pub enum PipelineStage {
    /// DSL source received but parse failed
    Draft,
    /// Parse succeeded - AST exists, tokens valid
    /// May have unresolved EntityRefs
    Parsed,
    /// AST has unresolved EntityRefs requiring user/agent resolution
    /// UI should show search popup for each unresolved ref
    Resolving,
    /// All EntityRefs resolved - ready for lint
    Resolved,
    /// CSG linter passed - dataflow valid
    Linted,
    /// Compile succeeded - execution plan ready
    Compiled,
    /// Execute succeeded - DB mutations committed
    Executed,
}

/// An unresolved EntityRef that needs user/agent resolution
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct UnresolvedRef {
    /// Statement index in AST (0-based)
    pub statement_index: usize,
    /// Argument key containing the EntityRef
    pub arg_key: String,
    /// Entity type for search (e.g., "cbu", "entity", "product")
    pub entity_type: String,
    /// The search text entered by user
    pub search_text: String,
}

// ============================================================================
// Learning/Feedback Types
// ============================================================================

/// Request to report a user correction (for learning loop)
#[derive(Debug, Deserialize)]
pub struct ReportCorrectionRequest {
    /// Session ID where correction occurred
    pub session_id: Uuid,
    /// Original user message that triggered DSL generation
    #[serde(default)]
    pub original_message: Option<String>,
    /// DSL generated by the agent
    pub generated_dsl: String,
    /// DSL after user correction (what was actually executed)
    pub corrected_dsl: String,
}

/// Response from reporting a correction
#[derive(Debug, Serialize)]
pub struct ReportCorrectionResponse {
    /// Whether the correction was recorded
    pub recorded: bool,
    /// Event ID for tracking
    #[serde(skip_serializing_if = "Option::is_none")]
    pub event_id: Option<i64>,
}
