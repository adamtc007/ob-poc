use axum::{
    extract::{Path, Query, State},
    http::StatusCode,
    response::{Html, Json},
    routing::{get, post},
    Router,
};
use serde::{Deserialize, Serialize};
use std::sync::Arc;
use tower::ServiceBuilder;
use tower_http::{
    cors::{Any, CorsLayer},
    services::ServeDir,
    trace::TraceLayer,
};
use tracing::{info, warn};

use ob_poc::{
    database::DslDomainRepository,
    dsl_manager::{DslManager, LayoutType, VisualizationOptions, DomainVisualizationOptions, StylingConfig},
    models::{DslDomain as Domain, DslVersion},
};

// Application state
#[derive(Clone)]
pub struct AppState {
    pub dsl_manager: Arc<DslManager>,
}

// API types
#[derive(Serialize, Deserialize)]
pub struct ApiResponse<T> {
    pub success: bool,
    pub data: Option<T>,
    pub error: Option<String>,
}

#[derive(Serialize, Deserialize)]
pub struct DomainInfo {
    pub domain_id: String,
    pub domain_name: String,
    pub description: Option<String>,
    pub active: bool,
    pub version_count: usize,
}

#[derive(Serialize, Deserialize)]
pub struct VersionInfo {
    pub version_id: String,
    pub version_number: i32,
    pub functional_state: Option<String>,
    pub compilation_status: String,
    pub created_at: String,
}

#[derive(Serialize, Deserialize)]
pub struct VisualizationRequest {
    pub layout_type: Option<String>,
    pub include_domain_context: Option<bool>,
    pub highlight_current_state: Option<bool>,
}

#[derive(Deserialize)]
pub struct VisualizationQuery {
    pub layout: Option<String>,
    pub domain_context: Option<bool>,
    pub highlight: Option<bool>,
}

#[tokio::main]
async fn main() -> Result<(), Box<dyn std::error::Error>> {
    // Initialize tracing
    tracing_subscriber::fmt()
        .with_env_filter("ob_poc_web_server=info,tower_http=debug")
        .init();

    // Load environment variables
    dotenvy::dotenv().ok();

    // Database connection
    let database_url = std::env::var("DATABASE_URL")
        .unwrap_or_else(|_| "postgresql://localhost:5432/ob-poc".to_string());

    info!("Connecting to database: {}", database_url);
    let pool = sqlx::PgPool::connect(&database_url).await?;

    // Initialize DSL Manager
    let repository = DslDomainRepository::new(pool);
    let dsl_manager = Arc::new(DslManager::new_with_defaults(repository));

    // Create application state
    let app_state = AppState { dsl_manager };

    // Build our application with routes
    let app = create_router(app_state);

    // Determine port
    let port = std::env::var("PORT")
        .unwrap_or_else(|_| "3000".to_string())
        .parse::<u16>()
        .unwrap_or(3000);

    let addr = format!("0.0.0.0:{}", port);
    info!("Starting server on {}", addr);

    // Start server
    let listener = tokio::net::TcpListener::bind(&addr).await?;
    axum::serve(listener, app).await?;

    Ok(())
}

fn create_router(state: AppState) -> Router {
    Router::new()
        // Serve static files (WASM app will go here)
        .nest_service("/", ServeDir::new("static"))

        // API routes
        .route("/api/health", get(health_check))
        .route("/api/domains", get(list_domains))
        .route("/api/domains/:domain_id/versions", get(list_domain_versions))
        .route("/api/domains/:domain_id/versions/:version_id/ast", get(get_ast_visualization))
        .route("/api/domains/:domain_id/versions/:version_id/visualize", get(get_domain_visualization))

        // Add middleware
        .layer(
            ServiceBuilder::new()
                .layer(TraceLayer::new_for_http())
                .layer(
                    CorsLayer::new()
                        .allow_origin(Any)
                        .allow_methods(Any)
                        .allow_headers(Any)
                )
        )
        .with_state(state)
}

// Health check endpoint
async fn health_check() -> Json<ApiResponse<String>> {
    Json(ApiResponse {
        success: true,
        data: Some("OK".to_string()),
        error: None,
    })
}

// List all domains
async fn list_domains(
    State(state): State<AppState>,
) -> Result<Json<ApiResponse<Vec<DomainInfo>>>, StatusCode> {
    match state.dsl_manager.list_domains(true).await {
        Ok(domains) => {
            let domain_infos: Vec<DomainInfo> = domains
                .into_iter()
                .map(|domain| DomainInfo {
                    domain_id: domain.domain_id.to_string(),
                    domain_name: domain.domain_name.clone(),
                    description: domain.description.clone(),
                    active: domain.active,
                    version_count: 0, // TODO: Get actual version count
                })
                .collect();

            Ok(Json(ApiResponse {
                success: true,
                data: Some(domain_infos),
                error: None,
            }))
        }
        Err(e) => {
            warn!("Failed to list domains: {:?}", e);
            Err(StatusCode::INTERNAL_SERVER_ERROR)
        }
    }
}

// List versions for a domain
async fn list_domain_versions(
    Path(domain_id): Path<String>,
    State(state): State<AppState>,
) -> Result<Json<ApiResponse<Vec<VersionInfo>>>, StatusCode> {
    match uuid::Uuid::parse_str(&domain_id) {
        Ok(domain_uuid) => {
            match state.dsl_manager.list_domain_versions(domain_uuid, None).await {
                Ok(versions) => {
                    let version_infos: Vec<VersionInfo> = versions
                        .into_iter()
                        .map(|version| VersionInfo {
                            version_id: version.version_id.to_string(),
                            version_number: version.version_number,
                            functional_state: version.functional_state.clone(),
                            compilation_status: format!("{:?}", version.compilation_status),
                            created_at: version.created_at.to_rfc3339(),
                        })
                        .collect();

                    Ok(Json(ApiResponse {
                        success: true,
                        data: Some(version_infos),
                        error: None,
                    }))
                }
                Err(e) => {
                    warn!("Failed to list domain versions: {:?}", e);
                    Err(StatusCode::INTERNAL_SERVER_ERROR)
                }
            }
        }
        Err(_) => Err(StatusCode::BAD_REQUEST),
    }
}

// Get AST visualization for a specific version
async fn get_ast_visualization(
    Path((domain_id, version_id)): Path<(String, String)>,
    Query(query): Query<VisualizationQuery>,
    State(state): State<AppState>,
) -> Result<Json<ApiResponse<serde_json::Value>>, StatusCode> {
    match (uuid::Uuid::parse_str(&domain_id), uuid::Uuid::parse_str(&version_id)) {
        (Ok(domain_uuid), Ok(version_uuid)) => {
            // Set up visualization options based on query parameters
            let layout_type = match query.layout.as_deref() {
                Some("tree") => LayoutType::Tree,
                Some("graph") => LayoutType::Graph,
                Some("hierarchical") => LayoutType::Hierarchical,
                _ => LayoutType::Tree,
            };

            let options = VisualizationOptions {
                layout: layout_type,
                include_compilation_info: true,
                include_domain_context: query.domain_context.unwrap_or(true),
                styling: StylingConfig::default(),
                filters: None,
            };

            match state.dsl_manager.generate_ast_visualization(version_uuid, &options).await {
                Ok(visualization) => {
                    // Convert to JSON
                    match serde_json::to_value(&visualization) {
                        Ok(json_value) => Ok(Json(ApiResponse {
                            success: true,
                            data: Some(json_value),
                            error: None,
                        })),
                        Err(e) => {
                            warn!("Failed to serialize visualization: {:?}", e);
                            Err(StatusCode::INTERNAL_SERVER_ERROR)
                        }
                    }
                }
                Err(e) => {
                    warn!("Failed to generate AST visualization: {:?}", e);
                    Err(StatusCode::INTERNAL_SERVER_ERROR)
                }
            }
        }
        _ => Err(StatusCode::BAD_REQUEST),
    }
}

// Get domain-specific visualization
async fn get_domain_visualization(
    Path((domain_id, version_id)): Path<(String, String)>,
    Query(query): Query<VisualizationQuery>,
    State(state): State<AppState>,
) -> Result<Json<ApiResponse<serde_json::Value>>, StatusCode> {
    match (uuid::Uuid::parse_str(&domain_id), uuid::Uuid::parse_str(&version_id)) {
        (Ok(domain_uuid), Ok(version_uuid)) => {
            // First get the domain name for domain-specific visualization
            match state.dsl_manager.get_domain_by_id(domain_uuid).await {
                Ok(Some(domain)) => {
                    let layout_type = match query.layout.as_deref() {
                        Some("tree") => LayoutType::Tree,
                        Some("graph") => LayoutType::Graph,
                        Some("hierarchical") => LayoutType::Hierarchical,
                        _ => LayoutType::Tree,
                    };

                    let domain_options = DomainVisualizationOptions {
                        base_options: VisualizationOptions {
                            layout: layout_type,
                            include_compilation_info: true,
                            include_domain_context: true,
                            styling: StylingConfig::default(),
                            filters: None,
                        },
                        highlight_current_state: query.highlight.unwrap_or(true),
                        show_state_transitions: true,
                        include_domain_metrics: true,
                        show_workflow_progression: true,
                        emphasize_critical_paths: true,
                        domain_specific_styling: true,
                    };

                    match state.dsl_manager.generate_domain_enhanced_visualization(
                        version_uuid,
                        &domain.domain_name,
                        &domain_options,
                    ).await {
                        Ok(visualization) => {
                            match serde_json::to_value(&visualization) {
                                Ok(json_value) => Ok(Json(ApiResponse {
                                    success: true,
                                    data: Some(json_value),
                                    error: None,
                                })),
                                Err(e) => {
                                    warn!("Failed to serialize domain visualization: {:?}", e);
                                    Err(StatusCode::INTERNAL_SERVER_ERROR)
                                }
                            }
                        }
                        Err(e) => {
                            warn!("Failed to generate domain visualization: {:?}", e);
                            Err(StatusCode::INTERNAL_SERVER_ERROR)
                        }
                    }
                }
                Ok(None) => Err(StatusCode::NOT_FOUND),
                Err(e) => {
                    warn!("Failed to get domain: {:?}", e);
                    Err(StatusCode::INTERNAL_SERVER_ERROR)
                }
            }
        }
        _ => Err(StatusCode::BAD_REQUEST),
    }
}

// Tests removed during refactor consolidation
