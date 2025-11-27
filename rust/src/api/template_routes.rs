//! Template API endpoints

use crate::templates::{FormTemplate, RenderError, TemplateRegistry, TemplateRenderer};
use axum::{
    extract::{Path, State},
    http::StatusCode,
    response::{IntoResponse, Json},
    routing::{get, post},
    Router,
};
use serde::{Deserialize, Serialize};
use std::collections::HashMap;
use std::sync::Arc;

#[derive(Clone)]
pub struct TemplateState {
    registry: Arc<TemplateRegistry>,
}

impl TemplateState {
    pub fn new() -> Self {
        Self {
            registry: Arc::new(TemplateRegistry::new()),
        }
    }
}

impl Default for TemplateState {
    fn default() -> Self {
        Self::new()
    }
}

// Response types
#[derive(Debug, Serialize)]
pub struct TemplateListResponse {
    pub templates: Vec<TemplateSummary>,
}

#[derive(Debug, Serialize)]
pub struct TemplateSummary {
    pub id: String,
    pub name: String,
    pub description: String,
    pub domain: String,
    pub tags: Vec<String>,
}

#[derive(Debug, Deserialize)]
pub struct RenderRequest {
    pub values: HashMap<String, serde_json::Value>,
}

#[derive(Debug, Serialize)]
pub struct RenderResponse {
    pub dsl: String,
    pub verb: String,
}

#[derive(Debug, Serialize)]
pub struct ErrorResponse {
    pub error: String,
}

/// GET /api/templates
async fn list_templates(State(state): State<TemplateState>) -> Json<TemplateListResponse> {
    let templates = state
        .registry
        .list()
        .iter()
        .map(|t| TemplateSummary {
            id: t.id.clone(),
            name: t.name.clone(),
            description: t.description.clone(),
            domain: t.domain.clone(),
            tags: t.tags.clone(),
        })
        .collect();

    Json(TemplateListResponse { templates })
}

/// GET /api/templates/:id
async fn get_template(
    State(state): State<TemplateState>,
    Path(id): Path<String>,
) -> Result<Json<FormTemplate>, StatusCode> {
    state
        .registry
        .get(&id)
        .cloned()
        .map(Json)
        .ok_or(StatusCode::NOT_FOUND)
}

/// POST /api/templates/:id/render
async fn render_template(
    State(state): State<TemplateState>,
    Path(id): Path<String>,
    Json(req): Json<RenderRequest>,
) -> Result<Json<RenderResponse>, impl IntoResponse> {
    let template = match state.registry.get(&id) {
        Some(t) => t,
        None => {
            return Err((
                StatusCode::NOT_FOUND,
                Json(ErrorResponse {
                    error: format!("Template '{}' not found", id),
                }),
            ))
        }
    };

    match TemplateRenderer::render(template, &req.values) {
        Ok(dsl) => Ok(Json(RenderResponse {
            dsl,
            verb: template.verb.clone(),
        })),
        Err(e) => Err((
            StatusCode::BAD_REQUEST,
            Json(ErrorResponse {
                error: render_error_message(&e),
            }),
        )),
    }
}

fn render_error_message(e: &RenderError) -> String {
    match e {
        RenderError::MissingRequired(slot) => format!("Missing required slot: {}", slot),
        RenderError::TypeMismatch {
            slot,
            expected,
            got,
        } => format!(
            "Type mismatch for slot '{}': expected {}, got {}",
            slot, expected, got
        ),
        RenderError::InvalidValue(msg) => format!("Invalid value: {}", msg),
    }
}

/// Create router for template endpoints
pub fn create_template_router() -> Router {
    let state = TemplateState::new();

    Router::new()
        .route("/api/templates", get(list_templates))
        .route("/api/templates/:id", get(get_template))
        .route("/api/templates/:id/render", post(render_template))
        .with_state(state)
}
