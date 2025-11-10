//! REST API Server for DSL Visualizer
//!
//! This module provides a REST API server that serves data to the egui visualizer
//! and other frontend applications. It exposes endpoints for browsing DSL instances
//! and retrieving AST data.

use crate::database::{
    DatabaseManager, DslInstance, DslInstanceRepository, PgDslInstanceRepository,
};
use crate::error::Error;
use axum::{
    extract::{Path, Query, State},
    http::StatusCode,
    response::Json,
    routing::get,
    Router,
};
use serde::{Deserialize, Serialize};
use serde_json::Value as JsonValue;
use std::collections::HashMap;
use std::sync::Arc;
use tower::ServiceBuilder;
use tower_http::cors::{Any, CorsLayer};
use tracing::{error, info};
use uuid::Uuid;

/// REST API server configuration
#[derive(Debug, Clone)]
pub struct RestApiConfig {
    pub host: String,
    pub port: u16,
}

impl Default for RestApiConfig {
    fn default() -> Self {
        Self {
            host: "127.0.0.1".to_string(),
            port: 8080,
        }
    }
}

/// Application state for the REST API
#[derive(Clone)]
pub struct AppState {
    pub dsl_repository: Arc<PgDslInstanceRepository>,
}

/// Query parameters for DSL listing
#[derive(Debug, Deserialize)]
pub struct DslListQuery {
    pub search: Option<String>,
    pub domain: Option<String>,
    pub limit: Option<i64>,
    pub offset: Option<i64>,
}

/// Response model for DSL entry in the list
#[derive(Debug, Serialize)]
pub struct DslEntryResponse {
    pub id: String,
    pub name: String,
    pub domain: String,
    pub version: u32,
    pub description: String,
    pub created_at: String,
    pub status: String,
}

/// Response model for DSL list
#[derive(Debug, Serialize)]
pub struct DslListResponse {
    pub entries: Vec<DslEntryResponse>,
    pub total_count: u32,
}

/// Response model for DSL content with AST
#[derive(Debug, Serialize)]
pub struct DslContentResponse {
    pub id: String,
    pub content: String,
    pub ast: AstNodeResponse,
    pub version: u32,
    pub domain: String,
    pub status: String,
}

/// Response model for AST nodes
#[derive(Debug, Serialize)]
pub struct AstNodeResponse {
    pub id: String,
    pub node_type: String,
    pub label: String,
    pub properties: HashMap<String, String>,
    pub children: Vec<AstNodeResponse>,
    pub position: Option<(f32, f32)>,
}

/// REST API server manager
pub struct RestApiServer {
    config: RestApiConfig,
    app_state: AppState,
}

impl RestApiServer {
    /// Create a new REST API server
    pub fn new(config: RestApiConfig, database_manager: Arc<DatabaseManager>) -> Self {
        let app_state = AppState {
            dsl_repository: Arc::new(database_manager.dsl_instance_repository()),
        };

        Self { config, app_state }
    }

    /// Start the REST API server
    pub async fn start(self) -> Result<(), Box<dyn std::error::Error + Send + Sync>> {
        let app = self.create_router();
        let addr = format!("{}:{}", self.config.host, self.config.port);

        info!("Starting REST API server on {}", addr);

        let listener = tokio::net::TcpListener::bind(&addr).await?;
        axum::serve(listener, app).await?;

        Ok(())
    }

    /// Create the router with all endpoints
    fn create_router(&self) -> Router {
        Router::new()
            .route("/api/dsls", get(list_dsls))
            .route("/api/dsls/:id/ast", get(get_dsl_ast))
            .route("/api/health", get(health_check))
            .layer(
                ServiceBuilder::new().layer(CorsLayer::new().allow_origin(Any).allow_methods(Any)),
            )
            .with_state(self.app_state.clone())
    }
}

/// Health check endpoint
async fn health_check() -> Result<Json<serde_json::Value>, (StatusCode, String)> {
    Ok(Json(serde_json::json!({
        "status": "healthy",
        "service": "dsl-visualizer-api",
        "timestamp": chrono::Utc::now().to_rfc3339()
    })))
}

/// List DSL instances with optional filtering
async fn list_dsls(
    State(state): State<AppState>,
    Query(params): Query<DslListQuery>,
) -> Result<Json<DslListResponse>, (StatusCode, String)> {
    info!(
        "Listing DSLs - search: {:?}, domain: {:?}, limit: {:?}, offset: {:?}",
        params.search, params.domain, params.limit, params.offset
    );

    let limit = params.limit.unwrap_or(50).min(100); // Cap at 100
    let offset = params.offset.unwrap_or(0).max(0);

    // Get DSL instances from database
    let instances = state
        .dsl_repository
        .list_instances(params.domain.as_deref(), Some(limit), Some(offset))
        .await
        .map_err(|e| {
            error!("Failed to list DSL instances: {}", e);
            (StatusCode::INTERNAL_SERVER_ERROR, e.to_string())
        })?;

    // Filter by search term if provided
    let filtered_instances: Vec<DslInstance> = if let Some(search_term) = &params.search {
        let search_lower = search_term.to_lowercase();
        instances
            .into_iter()
            .filter(|instance| {
                instance
                    .business_reference
                    .to_lowercase()
                    .contains(&search_lower)
                    || instance.domain_name.to_lowercase().contains(&search_lower)
            })
            .collect()
    } else {
        instances
    };

    // Convert to response format
    let entries: Vec<DslEntryResponse> = filtered_instances
        .into_iter()
        .map(|instance| DslEntryResponse {
            id: instance.instance_id.to_string(),
            name: instance.business_reference.clone(),
            domain: instance.domain_name.clone(),
            version: instance.current_version as u32,
            description: extract_description(&instance),
            created_at: instance.created_at.to_rfc3339(),
            status: instance.status.to_string(),
        })
        .collect();

    let total_count = entries.len() as u32;

    Ok(Json(DslListResponse {
        entries,
        total_count,
    }))
}

/// Get DSL content and AST for a specific instance
async fn get_dsl_ast(
    State(state): State<AppState>,
    Path(instance_id): Path<String>,
) -> Result<Json<DslContentResponse>, (StatusCode, String)> {
    info!("Getting DSL AST for instance: {}", instance_id);

    // Parse instance ID
    let instance_uuid = Uuid::parse_str(&instance_id).map_err(|_| {
        (
            StatusCode::BAD_REQUEST,
            "Invalid instance ID format".to_string(),
        )
    })?;

    // Get the DSL instance
    let instance = state
        .dsl_repository
        .get_instance(instance_uuid)
        .await
        .map_err(|e| {
            error!("Failed to get DSL instance {}: {}", instance_id, e);
            (StatusCode::INTERNAL_SERVER_ERROR, e.to_string())
        })?
        .ok_or_else(|| {
            (
                StatusCode::NOT_FOUND,
                format!("DSL instance {} not found", instance_id),
            )
        })?;

    // Get the latest version of the DSL
    let version = state
        .dsl_repository
        .get_latest_version(instance_uuid)
        .await
        .map_err(|e| {
            error!("Failed to get latest version for {}: {}", instance_id, e);
            (StatusCode::INTERNAL_SERVER_ERROR, e.to_string())
        })?
        .ok_or_else(|| {
            (
                StatusCode::NOT_FOUND,
                format!("No versions found for DSL instance {}", instance_id),
            )
        })?;

    // Get AST nodes if available
    let ast_nodes = if let Some(version_id) = Some(version.version_id) {
        state
            .dsl_repository
            .get_ast_nodes_by_version(version_id)
            .await
            .map_err(|e| {
                error!("Failed to get AST nodes for version {}: {}", version_id, e);
                (StatusCode::INTERNAL_SERVER_ERROR, e.to_string())
            })?
    } else {
        vec![]
    };

    // Convert AST to tree structure
    let ast_tree = build_ast_tree(&ast_nodes, &version.ast_json);

    Ok(Json(DslContentResponse {
        id: instance.instance_id.to_string(),
        content: version.dsl_content.clone(),
        ast: ast_tree,
        version: version.version_number as u32,
        domain: instance.domain_name.clone(),
        status: instance.status.to_string(),
    }))
}

/// Extract a description from the DSL instance metadata or generate one
fn extract_description(instance: &DslInstance) -> String {
    if let Some(metadata) = &instance.metadata {
        if let Some(description) = metadata.get("description") {
            if let Some(desc_str) = description.as_str() {
                return desc_str.to_string();
            }
        }
    }

    // Generate a default description
    format!(
        "{} DSL instance (v{})",
        instance.domain_name, instance.current_version
    )
}

/// Build AST tree structure from flat AST nodes and JSON AST
fn build_ast_tree(
    ast_nodes: &[crate::database::AstNode],
    ast_json: &Option<JsonValue>,
) -> AstNodeResponse {
    // If we have structured AST JSON, use that
    if let Some(json_ast) = ast_json {
        return convert_json_ast_to_response(json_ast, None);
    }

    // Otherwise, build from flat AST nodes
    if ast_nodes.is_empty() {
        return create_default_ast_root();
    }

    // Find root node (no parent)
    let root_node = ast_nodes
        .iter()
        .find(|node| node.parent_node_id.is_none())
        .unwrap_or(&ast_nodes[0]);

    build_ast_node_tree(root_node, ast_nodes)
}

/// Convert JSON AST to response format recursively
fn convert_json_ast_to_response(
    json_ast: &JsonValue,
    position: Option<(f32, f32)>,
) -> AstNodeResponse {
    match json_ast {
        JsonValue::Object(obj) => {
            let node_type = obj
                .get("type")
                .and_then(|v| v.as_str())
                .unwrap_or("unknown")
                .to_string();

            let label = obj
                .get("value")
                .and_then(|v| v.as_str())
                .or_else(|| obj.get("name").and_then(|v| v.as_str()))
                .unwrap_or(&node_type)
                .to_string();

            let mut properties = HashMap::new();
            for (key, value) in obj {
                if !["type", "children", "value", "name"].contains(&key.as_str()) {
                    properties.insert(key.clone(), format!("{}", value));
                }
            }

            let children = obj
                .get("children")
                .and_then(|v| v.as_array())
                .map(|arr| {
                    arr.iter()
                        .enumerate()
                        .map(|(i, child)| {
                            let child_pos =
                                position.map(|(x, y)| (x + 50.0, y + 100.0 + i as f32 * 30.0));
                            convert_json_ast_to_response(child, child_pos)
                        })
                        .collect()
                })
                .unwrap_or_default();

            AstNodeResponse {
                id: uuid::Uuid::new_v4().to_string(),
                node_type,
                label,
                properties,
                children,
                position,
            }
        }
        JsonValue::Array(arr) => {
            let children = arr
                .iter()
                .enumerate()
                .map(|(i, child)| {
                    let child_pos = position.map(|(x, y)| (x, y + i as f32 * 30.0));
                    convert_json_ast_to_response(child, child_pos)
                })
                .collect();

            AstNodeResponse {
                id: uuid::Uuid::new_v4().to_string(),
                node_type: "array".to_string(),
                label: "Array".to_string(),
                properties: HashMap::new(),
                children,
                position,
            }
        }
        JsonValue::String(s) => AstNodeResponse {
            id: uuid::Uuid::new_v4().to_string(),
            node_type: "string".to_string(),
            label: s.clone(),
            properties: HashMap::new(),
            children: vec![],
            position,
        },
        other => AstNodeResponse {
            id: uuid::Uuid::new_v4().to_string(),
            node_type: "value".to_string(),
            label: format!("{}", other),
            properties: HashMap::new(),
            children: vec![],
            position,
        },
    }
}

/// Build AST tree from flat database nodes
fn build_ast_node_tree(
    node: &crate::database::AstNode,
    all_nodes: &[crate::database::AstNode],
) -> AstNodeResponse {
    let children: Vec<AstNodeResponse> = all_nodes
        .iter()
        .filter(|child| child.parent_node_id == Some(node.node_id))
        .map(|child| build_ast_node_tree(child, all_nodes))
        .collect();

    let mut properties = HashMap::new();
    if let Some(ref value) = node.node_value {
        properties.insert("value".to_string(), value.to_string());
    }
    if let Some(ref key) = node.node_key {
        properties.insert("key".to_string(), key.clone());
    }
    properties.insert("depth".to_string(), node.depth.to_string());
    properties.insert("path".to_string(), node.path.clone());

    AstNodeResponse {
        id: node.node_id.to_string(),
        node_type: node.node_type.to_string(),
        label: node.node_key.clone().unwrap_or_else(|| {
            node.node_value
                .as_ref()
                .and_then(|v| v.as_str())
                .unwrap_or(&node.node_type.to_string())
                .to_string()
        }),
        properties,
        children,
        position: None, // Position will be calculated by the visualizer
    }
}

/// Create a default AST root when no data is available
fn create_default_ast_root() -> AstNodeResponse {
    AstNodeResponse {
        id: uuid::Uuid::new_v4().to_string(),
        node_type: "root".to_string(),
        label: "Empty DSL".to_string(),
        properties: HashMap::new(),
        children: vec![],
        position: Some((200.0, 100.0)),
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use serde_json::json;

    #[test]
    fn test_convert_json_ast_to_response() {
        let json_ast = json!({
            "type": "program",
            "name": "test-program",
            "children": [
                {
                    "type": "verb",
                    "value": "validate"
                },
                {
                    "type": "attribute",
                    "value": "customer.email"
                }
            ]
        });

        let ast = convert_json_ast_to_response(&json_ast, Some((100.0, 50.0)));

        assert_eq!(ast.node_type, "program");
        assert_eq!(ast.label, "test-program");
        assert_eq!(ast.children.len(), 2);
        assert_eq!(ast.children[0].node_type, "verb");
        assert_eq!(ast.children[0].label, "validate");
        assert_eq!(ast.position, Some((100.0, 50.0)));
    }

    #[test]
    fn test_extract_description() {
        let mut instance = DslInstance {
            instance_id: Uuid::new_v4(),
            domain_name: "test-domain".to_string(),
            business_reference: "TEST-001".to_string(),
            current_version: 1,
            status: crate::database::InstanceStatus::Created,
            created_at: chrono::Utc::now(),
            updated_at: chrono::Utc::now(),
            metadata: None,
        };

        // Test without metadata
        let desc = extract_description(&instance);
        assert_eq!(desc, "test-domain DSL instance (v1)");

        // Test with metadata description
        instance.metadata = Some(json!({
            "description": "Custom description"
        }));
        let desc = extract_description(&instance);
        assert_eq!(desc, "Custom description");
    }

    #[test]
    fn test_create_default_ast_root() {
        let ast = create_default_ast_root();
        assert_eq!(ast.node_type, "root");
        assert_eq!(ast.label, "Empty DSL");
        assert_eq!(ast.children.len(), 0);
        assert!(ast.position.is_some());
    }
}
