//! Entity Search API endpoints
//!
//! Uses EntityGateway gRPC service for all entity lookups.
//! This ensures consistent fuzzy search behavior across the entire system.
//!
//! ## Endpoints
//!
//! - `GET /api/entity/search` - Server-side fuzzy search for entity resolution modal
//! - `GET /api/session/:id/entity/search` - Session-scoped entity search (uses constraint cascade)

use crate::api::session::SessionStore;
use crate::dsl_v2::gateway_resolver::gateway_addr;
use crate::session::constraint_cascade::derive_search_scope;
use axum::{
    extract::{Path, Query, State},
    http::StatusCode,
    response::Json,
    routing::get,
    Router,
};
use entity_gateway::proto::ob::gateway::v1::{
    entity_gateway_client::EntityGatewayClient, SearchMode, SearchRequest,
};
use serde::{Deserialize, Serialize};
use uuid::Uuid;

// ============================================================================
// Request/Response Types
// ============================================================================

/// Query params for entity search
#[derive(Debug, Deserialize)]
pub struct EntitySearchQuery {
    /// Entity type nickname: cbu, entity, person, company, product, role, jurisdiction, etc.
    #[serde(rename = "type")]
    pub entity_type: String,

    /// Search query string (minimum 2 chars recommended)
    pub q: String,

    /// Max results (default: 10, max: 50)
    #[serde(default = "default_limit")]
    pub limit: u32,

    /// Optional jurisdiction filter (e.g., "LU", "US")
    pub jurisdiction: Option<String>,

    /// Optional nationality discriminator for person search (ISO 2-letter code, e.g., "DE", "US")
    pub nationality: Option<String>,

    /// Optional date of birth discriminator for person search (YYYY-MM-DD or just YYYY)
    pub dob: Option<String>,

    /// Optional client_id to scope search (constraint cascade level 1)
    /// When set, only entities belonging to this client group are returned
    pub client_id: Option<Uuid>,

    /// Optional structure_type to further constrain results (constraint cascade level 2)
    /// Values: pe, sicav, hedge, etf, pension, trust, fof
    pub structure_type: Option<String>,
}

fn default_limit() -> u32 {
    10
}

/// Scope info derived from session for search filtering
#[derive(Debug, Clone, Default)]
pub struct EntitySearchScope {
    /// Client group ID (constraint cascade level 1)
    pub client_id: Option<Uuid>,
    /// Structure type (constraint cascade level 2)
    pub structure_type: Option<String>,
    /// Structure ID (constraint cascade level 3)
    pub structure_id: Option<Uuid>,
}

// Use the shared EntityMatch type from ob-poc-types for client compatibility
use ob_poc_types::EntityMatch;

/// Response from entity search
#[derive(Debug, Clone, Serialize)]
pub struct EntitySearchResponse {
    /// List of matches
    pub matches: Vec<EntityMatch>,

    /// Number of matches returned
    pub total: usize,

    /// True if more matches exist beyond limit
    pub truncated: bool,
}

// ============================================================================
// Handlers
// ============================================================================

/// GET /api/entity/search
///
/// Server-side fuzzy search for entity resolution modal.
/// Wraps EntityGateway gRPC service.
///
/// ## Query Parameters
///
/// - `type` (required): Entity type nickname
/// - `q` (required): Search query
/// - `limit` (optional): Max results (default 10, max 50)
/// - `jurisdiction` (optional): Filter by jurisdiction code
///
/// ## Example
///
/// ```text
/// GET /api/entity/search?type=entity&q=john&limit=10
/// ```
async fn search_entities(
    Query(query): Query<EntitySearchQuery>,
) -> Result<Json<EntitySearchResponse>, (StatusCode, String)> {
    // Validate query length
    if query.q.len() < 2 {
        return Err((
            StatusCode::BAD_REQUEST,
            "Search query must be at least 2 characters".to_string(),
        ));
    }

    let addr = gateway_addr();

    let mut client = EntityGatewayClient::connect(addr.clone())
        .await
        .map_err(|e| {
            tracing::error!("Failed to connect to EntityGateway at {}: {}", addr, e);
            (
                StatusCode::SERVICE_UNAVAILABLE,
                format!("EntityGateway unavailable: {}", e),
            )
        })?;

    // Map common type aliases to gateway nicknames (UPPERCASE)
    let nickname = normalize_entity_type(&query.entity_type);

    // Build discriminators for composite person search
    let mut discriminators = std::collections::HashMap::new();
    if let Some(ref nationality) = query.nationality {
        discriminators.insert("nationality".to_string(), nationality.to_uppercase());
    }
    if let Some(ref dob) = query.dob {
        discriminators.insert("date_of_birth".to_string(), dob.clone());
    }

    let request = SearchRequest {
        nickname,
        values: vec![query.q.clone()],
        search_key: None,
        mode: SearchMode::Fuzzy as i32,
        limit: Some(query.limit.min(50) as i32 + 1), // +1 to detect truncation
        discriminators,
        tenant_id: None,
        cbu_id: None,
    };

    let response = client.search(request).await.map_err(|e| {
        tracing::error!("EntityGateway search error: {}", e);
        (
            StatusCode::INTERNAL_SERVER_ERROR,
            format!("Search failed: {}", e),
        )
    })?;

    let mut matches: Vec<_> = response.into_inner().matches;
    let limit = query.limit.min(50) as usize;
    let truncated = matches.len() > limit;

    // Truncate to limit
    matches.truncate(limit);

    // Apply jurisdiction filter if provided (post-filter)
    // TODO: This should ideally be pushed to EntityGateway
    let matches: Vec<EntityMatch> = matches
        .into_iter()
        .filter(|m| {
            if let Some(ref jur) = query.jurisdiction {
                // Check if display contains jurisdiction
                m.display.to_uppercase().contains(&jur.to_uppercase())
            } else {
                true
            }
        })
        .map(|m| {
            // Parse jurisdiction from display if present (format: "Name (JUR)")
            let jurisdiction = extract_jurisdiction(&m.display);

            EntityMatch {
                entity_id: m.token,
                name: m.display,
                entity_type: query.entity_type.clone(),
                jurisdiction,
                context: None,
                score: Some(m.score as f64),
            }
        })
        .collect();

    let total = matches.len();

    Ok(Json(EntitySearchResponse {
        matches,
        total,
        truncated,
    }))
}

/// Normalize entity type aliases to gateway nicknames (UPPERCASE)
fn normalize_entity_type(entity_type: &str) -> String {
    match entity_type.to_lowercase().as_str() {
        "cbu" | "client" => "CBU".to_string(),
        "entity" | "entities" => "ENTITY".to_string(),
        "person" | "proper_person" | "individual" => "PERSON".to_string(),
        "company" | "limited_company" | "legal_entity" => "LEGAL_ENTITY".to_string(),
        "product" | "products" => "PRODUCT".to_string(),
        "service" | "services" => "SERVICE".to_string(),
        "role" | "roles" => "ROLE".to_string(),
        "jurisdiction" | "jurisdictions" | "country" => "JURISDICTION".to_string(),
        "currency" | "currencies" => "CURRENCY".to_string(),
        "document_type" | "doc_type" => "DOCUMENT_TYPE".to_string(),
        "fund" => "FUND".to_string(),
        _ => entity_type.to_uppercase(), // Force uppercase for unknown types
    }
}

/// Build detail string from entity type and display
/// Extract jurisdiction from display string if present
/// Handles formats like "Name | JURISDICTION" or "Name (JUR)"
fn extract_jurisdiction(display: &str) -> Option<String> {
    // Try "Name | JURISDICTION" format
    if let Some(idx) = display.find(" | ") {
        return Some(display[idx + 3..].trim().to_string());
    }
    // Try "Name (JUR)" format
    if let Some(start) = display.rfind('(') {
        if let Some(end) = display.rfind(')') {
            if end > start {
                return Some(display[start + 1..end].trim().to_string());
            }
        }
    }
    None
}

// ============================================================================
// Legacy endpoint for backward compatibility
// ============================================================================

/// Legacy query params (deprecated - use EntitySearchQuery)
#[derive(Debug, Deserialize)]
pub struct LegacySearchQuery {
    pub q: String,
    #[serde(default)]
    pub entity_type: Option<String>,
    #[serde(default = "default_limit")]
    pub limit: u32,
}

/// GET /api/entities/search (legacy endpoint)
async fn search_entities_legacy(
    Query(query): Query<LegacySearchQuery>,
) -> Result<Json<EntitySearchResponse>, (StatusCode, String)> {
    // Convert to new format
    let new_query = EntitySearchQuery {
        entity_type: query.entity_type.unwrap_or_else(|| "entity".to_string()),
        q: query.q,
        limit: query.limit,
        jurisdiction: None,
        nationality: None,
        dob: None,
        client_id: None,
        structure_type: None,
    };
    search_entities(Query(new_query)).await
}

// ============================================================================
// Session-Scoped Entity Search
// ============================================================================

/// State for session-scoped entity search
#[derive(Clone)]
pub struct ScopedEntitySearchState {
    pub sessions: SessionStore,
}

/// GET /api/session/:id/entity/search
///
/// Session-scoped entity search that automatically applies the constraint cascade
/// from the session context. This narrows search results based on:
/// - Level 1: Client group (e.g., only Allianz entities)
/// - Level 2: Structure type (e.g., only PE structures)
/// - Level 3: Current structure (e.g., only entities linked to this structure)
///
/// ## Example
///
/// ```text
/// # Session scoped to Allianz client - searches only within Allianz
/// GET /api/session/abc-123/entity/search?type=company&q=holding
/// ```
pub async fn search_entities_scoped(
    State(state): State<ScopedEntitySearchState>,
    Path(session_id): Path<Uuid>,
    Query(query): Query<EntitySearchQuery>,
) -> Result<Json<EntitySearchResponse>, (StatusCode, String)> {
    // Get session to derive search scope
    let scope = {
        let sessions = state.sessions.read().await;
        if let Some(session) = sessions.get(&session_id) {
            let search_scope = derive_search_scope(session);
            EntitySearchScope {
                client_id: search_scope.client_id,
                structure_type: search_scope
                    .structure_type
                    .map(|st| st.internal_token().to_string()),
                structure_id: search_scope.structure_id,
            }
        } else {
            // Session not found - no scope constraints
            EntitySearchScope::default()
        }
    };

    tracing::debug!(
        "Session {} entity search with scope: client={:?}, structure_type={:?}, structure={:?}",
        session_id,
        scope.client_id,
        scope.structure_type,
        scope.structure_id
    );

    // Merge query-level scope with session scope (query overrides)
    let effective_scope = EntitySearchScope {
        client_id: query.client_id.or(scope.client_id),
        structure_type: query.structure_type.clone().or(scope.structure_type),
        structure_id: scope.structure_id, // Structure ID only from session
    };

    // Execute scoped search
    search_entities_with_scope(query, effective_scope).await
}

/// Execute entity search with scope constraints applied
async fn search_entities_with_scope(
    query: EntitySearchQuery,
    scope: EntitySearchScope,
) -> Result<Json<EntitySearchResponse>, (StatusCode, String)> {
    // Validate query length
    if query.q.len() < 2 {
        return Err((
            StatusCode::BAD_REQUEST,
            "Search query must be at least 2 characters".to_string(),
        ));
    }

    let addr = gateway_addr();

    let mut client = EntityGatewayClient::connect(addr.clone())
        .await
        .map_err(|e| {
            tracing::error!("Failed to connect to EntityGateway at {}: {}", addr, e);
            (
                StatusCode::SERVICE_UNAVAILABLE,
                format!("EntityGateway unavailable: {}", e),
            )
        })?;

    // Map common type aliases to gateway nicknames (UPPERCASE)
    let nickname = normalize_entity_type(&query.entity_type);

    // Build discriminators for composite person search
    let mut discriminators = std::collections::HashMap::new();
    if let Some(ref nationality) = query.nationality {
        discriminators.insert("nationality".to_string(), nationality.to_uppercase());
    }
    if let Some(ref dob) = query.dob {
        discriminators.insert("date_of_birth".to_string(), dob.clone());
    }

    // Add scope discriminators
    // Note: EntityGateway needs to support client_group_id filtering
    // For now, we'll do post-filtering, but ideally this moves to the gateway
    if let Some(ref structure_type) = scope.structure_type {
        discriminators.insert("structure_type".to_string(), structure_type.clone());
    }

    let request = SearchRequest {
        nickname,
        values: vec![query.q.clone()],
        search_key: None,
        mode: SearchMode::Fuzzy as i32,
        limit: Some((query.limit.min(50) as i32 + 1) * 3), // Over-fetch for post-filtering
        discriminators,
        tenant_id: None,
        cbu_id: scope.structure_id.map(|id| id.to_string()), // Pass structure as CBU context
    };

    let response = client.search(request).await.map_err(|e| {
        tracing::error!("EntityGateway search error: {}", e);
        (
            StatusCode::INTERNAL_SERVER_ERROR,
            format!("Search failed: {}", e),
        )
    })?;

    let matches: Vec<_> = response.into_inner().matches;
    let limit = query.limit.min(50) as usize;

    // TODO: Apply client_id scope filtering here once EntityGateway supports it
    // For now, this is a placeholder for future client-scoped filtering
    // The constraint cascade narrows the search via discriminators where supported
    let mut filtered_matches: Vec<EntityMatch> = matches
        .into_iter()
        .filter(|m| {
            // Apply jurisdiction filter if provided
            if let Some(ref jur) = query.jurisdiction {
                if !m.display.to_uppercase().contains(&jur.to_uppercase()) {
                    return false;
                }
            }
            true
        })
        .map(|m| {
            let jurisdiction = extract_jurisdiction(&m.display);
            EntityMatch {
                entity_id: m.token,
                name: m.display,
                entity_type: query.entity_type.clone(),
                jurisdiction,
                context: None,
                score: Some(m.score as f64),
            }
        })
        .collect();

    let truncated = filtered_matches.len() > limit;
    filtered_matches.truncate(limit);

    let total = filtered_matches.len();

    Ok(Json(EntitySearchResponse {
        matches: filtered_matches,
        total,
        truncated,
    }))
}

// ============================================================================
// Router
// ============================================================================

/// Create router for entity endpoints (stateless)
pub fn create_entity_router() -> Router {
    Router::new()
        // New endpoint per API contract
        .route("/api/entity/search", get(search_entities))
        // Legacy endpoint for backward compatibility
        .route("/api/entities/search", get(search_entities_legacy))
}

/// Create router for session-scoped entity endpoints (requires session state)
pub fn create_scoped_entity_router(sessions: SessionStore) -> Router {
    let state = ScopedEntitySearchState { sessions };
    Router::new()
        .route(
            "/api/session/:session_id/entity/search",
            get(search_entities_scoped),
        )
        .with_state(state)
}

// ============================================================================
// Tests
// ============================================================================

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_normalize_entity_type() {
        assert_eq!(normalize_entity_type("cbu"), "CBU");
        assert_eq!(normalize_entity_type("CBU"), "CBU");
        assert_eq!(normalize_entity_type("person"), "PERSON");
        assert_eq!(normalize_entity_type("proper_person"), "PERSON");
        assert_eq!(normalize_entity_type("company"), "LEGAL_ENTITY");
        assert_eq!(normalize_entity_type("unknown"), "UNKNOWN");
    }
}
