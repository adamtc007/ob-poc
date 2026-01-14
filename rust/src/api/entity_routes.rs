//! Entity Search API endpoints
//!
//! Uses EntityGateway gRPC service for all entity lookups.
//! This ensures consistent fuzzy search behavior across the entire system.
//!
//! ## Endpoints
//!
//! - `GET /api/entity/search` - Server-side fuzzy search for entity resolution modal

use crate::dsl_v2::gateway_resolver::gateway_addr;
use axum::{extract::Query, http::StatusCode, response::Json, routing::get, Router};
use entity_gateway::proto::ob::gateway::v1::{
    entity_gateway_client::EntityGatewayClient, SearchMode, SearchRequest,
};
use serde::{Deserialize, Serialize};

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
}

fn default_limit() -> u32 {
    10
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
    };
    search_entities(Query(new_query)).await
}

// ============================================================================
// Router
// ============================================================================

/// Create router for entity endpoints
pub fn create_entity_router() -> Router {
    Router::new()
        // New endpoint per API contract
        .route("/api/entity/search", get(search_entities))
        // Legacy endpoint for backward compatibility
        .route("/api/entities/search", get(search_entities_legacy))
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
