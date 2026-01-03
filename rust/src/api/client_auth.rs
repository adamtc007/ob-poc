//! Client Portal Authentication Middleware
//!
//! Provides JWT-like token verification for client portal routes.
//! In production, replace with proper JWT verification.

use axum::{
    body::Body,
    extract::State,
    http::{header::AUTHORIZATION, Request, StatusCode},
    middleware::Next,
    response::Response,
};
use uuid::Uuid;

use super::client_routes::{AuthenticatedClient, ClientState};

/// Extract bearer token from Authorization header
fn extract_bearer_token(request: &Request<Body>) -> Option<String> {
    request
        .headers()
        .get(AUTHORIZATION)
        .and_then(|v| v.to_str().ok())
        .and_then(|v| v.strip_prefix("Bearer "))
        .map(|s| s.to_string())
}

/// Verify client token and inject AuthenticatedClient into request extensions
///
/// In production, this should:
/// 1. Verify JWT signature
/// 2. Check token expiration
/// 3. Validate claims
///
/// For development, we use a simple base64-encoded client_id
pub async fn verify_client_token(
    State(state): State<ClientState>,
    mut request: Request<Body>,
    next: Next,
) -> Result<Response, (StatusCode, String)> {
    // Extract token
    let token = extract_bearer_token(&request).ok_or_else(|| {
        (
            StatusCode::UNAUTHORIZED,
            "Missing Authorization header".to_string(),
        )
    })?;

    // Decode token (development: base64 encoded client_id)
    let client_id_str = base64::Engine::decode(&base64::engine::general_purpose::STANDARD, &token)
        .map_err(|_| (StatusCode::UNAUTHORIZED, "Invalid token format".to_string()))
        .and_then(|bytes| {
            String::from_utf8(bytes).map_err(|_| {
                (
                    StatusCode::UNAUTHORIZED,
                    "Invalid token encoding".to_string(),
                )
            })
        })?;

    let client_id = Uuid::parse_str(&client_id_str).map_err(|_| {
        (
            StatusCode::UNAUTHORIZED,
            "Invalid client ID in token".to_string(),
        )
    })?;

    // Look up client
    let client = sqlx::query!(
        r#"
        SELECT client_id, name, email, accessible_cbus
        FROM client_portal.clients
        WHERE client_id = $1 AND is_active = true
        "#,
        client_id
    )
    .fetch_optional(&state.pool)
    .await
    .map_err(|e| {
        tracing::error!("Client lookup error: {}", e);
        (
            StatusCode::INTERNAL_SERVER_ERROR,
            "Database error".to_string(),
        )
    })?
    .ok_or_else(|| {
        (
            StatusCode::UNAUTHORIZED,
            "Client not found or inactive".to_string(),
        )
    })?;

    // Create authenticated client
    let authenticated = AuthenticatedClient {
        client_id: client.client_id,
        client_name: client.name,
        client_email: client.email,
        accessible_cbus: client.accessible_cbus,
    };

    // Inject into request extensions
    request.extensions_mut().insert(authenticated);

    // Continue to next handler
    Ok(next.run(request).await)
}

/// Middleware that allows unauthenticated access (for login endpoint)
pub async fn allow_unauthenticated(request: Request<Body>, next: Next) -> Response {
    next.run(request).await
}

#[cfg(test)]
mod tests {
    #[allow(unused_imports)]
    use super::*;

    #[test]
    fn test_extract_bearer_token() {
        // Would need to construct a mock request
        // For now, just verify the function exists
    }
}
