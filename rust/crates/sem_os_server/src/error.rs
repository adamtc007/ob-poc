//! Error handling for the axum server.
//! Maps `SemOsError` to appropriate HTTP status codes and JSON error bodies.

use axum::{
    http::StatusCode,
    response::{IntoResponse, Response},
    Json,
};
use sem_os_core::error::SemOsError;
use serde_json::json;

/// Wrapper to convert `SemOsError` into an axum response.
pub struct AppError(SemOsError);

impl From<SemOsError> for AppError {
    fn from(e: SemOsError) -> Self {
        Self(e)
    }
}

impl IntoResponse for AppError {
    fn into_response(self) -> Response {
        let status = match &self.0 {
            SemOsError::NotFound(_) => StatusCode::NOT_FOUND,
            SemOsError::GateFailed(_) => StatusCode::UNPROCESSABLE_ENTITY,
            SemOsError::Unauthorized(_) => StatusCode::FORBIDDEN,
            SemOsError::Conflict(_) => StatusCode::CONFLICT,
            SemOsError::InvalidInput(_) => StatusCode::BAD_REQUEST,
            SemOsError::MigrationPending(_) => StatusCode::SERVICE_UNAVAILABLE,
            SemOsError::Internal(_) => StatusCode::INTERNAL_SERVER_ERROR,
        };

        let body = json!({
            "error": self.0.to_string(),
            "code": status.as_u16(),
        });

        (status, Json(body)).into_response()
    }
}
