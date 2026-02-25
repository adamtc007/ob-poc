//! JWT middleware for the Semantic OS server.
//!
//! Extracts `Authorization: Bearer <token>` header, validates the JWT signature,
//! calls `Principal::from_jwt_claims()`, and injects `Principal` into request extensions.
//! Returns 401 if token is missing or invalid.

use axum::{
    extract::Request,
    http::{header, StatusCode},
    middleware::Next,
    response::{IntoResponse, Response},
    Json,
};
use jsonwebtoken::{decode, DecodingKey, Validation};
use sem_os_core::principal::{JwtClaims, Principal};
use serde_json::json;

/// Shared state for JWT validation.
#[derive(Clone)]
pub struct JwtConfig {
    pub decoding_key: DecodingKey,
    pub validation: Validation,
}

impl JwtConfig {
    /// Create from a symmetric secret (HS256).
    pub fn from_secret(secret: &[u8]) -> Self {
        let mut validation = Validation::default();
        validation.validate_exp = false; // relax for dev — tighten in production
        validation.required_spec_claims.clear();
        Self {
            decoding_key: DecodingKey::from_secret(secret),
            validation,
        }
    }
}

/// Axum middleware layer that validates JWT and injects `Principal`.
pub async fn jwt_auth(mut req: Request, next: Next) -> Result<Response, Response> {
    let jwt_config = req
        .extensions()
        .get::<JwtConfig>()
        .cloned()
        .ok_or_else(|| {
            (
                StatusCode::INTERNAL_SERVER_ERROR,
                Json(json!({"error": "JWT config not initialized"})),
            )
                .into_response()
        })?;

    let auth_header = req
        .headers()
        .get(header::AUTHORIZATION)
        .and_then(|v| v.to_str().ok())
        .ok_or_else(|| {
            (
                StatusCode::UNAUTHORIZED,
                Json(json!({"error": "missing Authorization header"})),
            )
                .into_response()
        })?;

    let token = auth_header.strip_prefix("Bearer ").ok_or_else(|| {
        (
            StatusCode::UNAUTHORIZED,
            Json(
                json!({"error": "invalid Authorization header format — expected 'Bearer <token>'"}),
            ),
        )
            .into_response()
    })?;

    let token_data = decode::<JwtClaims>(token, &jwt_config.decoding_key, &jwt_config.validation)
        .map_err(|e| {
        tracing::warn!("JWT validation failed: {e}");
        (
            StatusCode::UNAUTHORIZED,
            Json(json!({"error": format!("invalid token: {e}")})),
        )
            .into_response()
    })?;

    let principal = Principal::from_jwt_claims(&token_data.claims).map_err(|e| {
        (
            StatusCode::UNAUTHORIZED,
            Json(json!({"error": format!("invalid claims: {e}")})),
        )
            .into_response()
    })?;

    req.extensions_mut().insert(principal);

    Ok(next.run(req).await)
}
