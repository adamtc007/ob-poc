//! Request and response types for the agent REST API.
//!
//! Extracted from agent_routes.rs for readability.

use serde::{Deserialize, Serialize};
use uuid::Uuid;

use crate::session::{
    MessageRole, SessionState, SubSessionType, UnifiedSession, UnresolvedRefInfo,
};

// ============================================================================
// Domain/Vocabulary Types
// ============================================================================

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
// Session Watch Types (Long-Polling)
// ============================================================================

/// Query parameters for session watch endpoint
#[derive(Debug, Deserialize)]
pub(crate) struct WatchQuery {
    /// Timeout in milliseconds (default 30000, max 60000)
    #[serde(default = "default_watch_timeout")]
    pub timeout_ms: u64,
}

pub(crate) fn default_watch_timeout() -> u64 {
    30000
}

/// Query parameters for verb surface endpoint
#[derive(Debug, Deserialize)]
pub(crate) struct VerbSurfaceQuery {
    /// Filter to specific domain (e.g., "kyc", "cbu")
    #[serde(default)]
    pub domain: Option<String>,
    /// Include excluded verbs with prune reasons
    #[serde(default)]
    pub include_excluded: bool,
}

/// Response from session watch endpoint
#[derive(Debug, Serialize)]
pub(crate) struct WatchResponse {
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
    pub(crate) fn from_snapshot(
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
// Sub-Session Types
// ============================================================================

/// Request to create a sub-session
#[derive(Debug, Deserialize)]
pub(crate) struct CreateSubSessionRequest {
    /// Type of sub-session to create
    pub session_type: CreateSubSessionType,
}

/// Sub-session type for API (simplified for JSON)
#[derive(Debug, Deserialize)]
#[serde(tag = "type", rename_all = "snake_case")]
pub(crate) enum CreateSubSessionType {
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
pub(crate) struct CreateSubSessionResponse {
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
pub(crate) struct SubSessionStateResponse {
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
pub(crate) struct SubSessionMessage {
    pub role: String,
    pub content: String,
    pub timestamp: chrono::DateTime<chrono::Utc>,
}

#[derive(Debug, Serialize)]
pub(crate) struct ResolutionState {
    pub total_refs: usize,
    pub current_index: usize,
    pub resolved_count: usize,
    pub current_ref: Option<UnresolvedRefInfo>,
    pub pending_refs: Vec<UnresolvedRefInfo>,
}

impl SubSessionStateResponse {
    pub(crate) fn from_session(session: &UnifiedSession) -> Self {
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

/// Request to complete a resolution sub-session
#[derive(Debug, Deserialize)]
pub(crate) struct CompleteSubSessionRequest {
    /// Whether to apply resolutions to parent
    #[serde(default = "default_true")]
    pub apply: bool,
}

pub(crate) fn default_true() -> bool {
    true
}

/// Response from completing a sub-session
#[derive(Debug, Serialize)]
pub(crate) struct CompleteSubSessionResponse {
    pub success: bool,
    pub resolutions_applied: usize,
    pub message: String,
}

// ============================================================================
// Binding/Focus/ViewMode Types
// ============================================================================

/// Request to set a binding in a session
#[derive(Debug, Deserialize)]
pub(crate) struct SetBindingRequest {
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
pub(crate) fn deserialize_uuid_string<'de, D>(deserializer: D) -> Result<Uuid, D::Error>
where
    D: serde::Deserializer<'de>,
{
    let s: String = serde::Deserialize::deserialize(deserializer)?;
    Uuid::parse_str(&s).map_err(serde::de::Error::custom)
}

/// Response from setting a binding
#[derive(Debug, Serialize)]
pub(crate) struct SetBindingResponse {
    pub success: bool,
    pub binding_name: String,
    pub bindings: std::collections::HashMap<String, Uuid>,
}

/// Request to set stage focus in a session
#[derive(Debug, Deserialize)]
pub(crate) struct SetFocusRequest {
    /// The stage code to focus on (e.g., "KYC_REVIEW")
    /// Pass None or empty string to clear focus
    #[serde(default)]
    pub stage_code: Option<String>,
}

/// Response from setting stage focus
#[derive(Debug, Serialize)]
pub(crate) struct SetFocusResponse {
    pub success: bool,
    /// The stage that is now focused (None if cleared)
    pub stage_code: Option<String>,
    /// Stage name for display
    pub stage_name: Option<String>,
    /// Verbs relevant to this stage (for agent filtering)
    pub relevant_verbs: Vec<String>,
}

// ============================================================================
// Additional Types
// ============================================================================

#[derive(Debug, Deserialize)]
pub(crate) struct ExecuteDslRequest {
    /// DSL source to execute. If None/missing, uses the session's run-sheet draft entries.
    #[serde(default)]
    pub dsl: Option<String>,
}

// NOTE: Direct /execute endpoint removed - use /api/session/:id/execute instead
// All DSL execution now requires a session for proper binding persistence and audit trail
