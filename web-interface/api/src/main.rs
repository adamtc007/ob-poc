//! OB-POC Web API Server
//!
//! This is the REST API server for the OB-POC web interface, providing HTTP endpoints
//! for entity CRUD operations, AI integration, and transaction management.

use axum::{
    extract::{Path, Query, State},
    http::{header, Method, StatusCode},
    middleware::{self, Next},
    response::{IntoResponse, Json, Response},
    routing::{get, post, put, delete},
    Router,
};
use serde::{Deserialize, Serialize};
use sqlx::PgPool;
use std::{collections::HashMap, env, net::SocketAddr, sync::Arc, time::Instant};
use tower::ServiceBuilder;
use tower_cors::{Any, CorsLayer};
use tower_http::trace::TraceLayer;
use tracing::{error, info, warn, debug};
use uuid::Uuid;

// Re-exports from the main ob-poc library
use ob_poc::{
    ai::{crud_prompt_builder::CrudPromptBuilder, rag_system::CrudRagSystem},
    services::{EntityCrudService, EntityTransactionManager},
};

/// Application state shared across all handlers
#[derive(Clone)]
pub struct AppState {
    /// Database connection pool
    pub pool: PgPool,
    /// Entity CRUD service with AI integration
    pub entity_service: Arc<EntityCrudService>,
    /// Transaction manager for batch operations
    pub transaction_manager: Arc<EntityTransactionManager>,
    /// Configuration
    pub config: ApiConfig,
}

/// API server configuration
#[derive(Debug, Clone)]
pub struct ApiConfig {
    pub host: String,
    pub port: u16,
    pub cors_origins: Vec<String>,
    pub enable_ai: bool,
    pub max_request_size: usize,
}

/// Standard API response wrapper
#[derive(Serialize)]
pub struct ApiResponse<T> {
    pub success: bool,
    pub data: Option<T>,
    pub error: Option<String>,
    pub timestamp: chrono::DateTime<chrono::Utc>,
    pub request_id: Option<String>,
}

/// Error response for API endpoints
#[derive(Serialize)]
pub struct ErrorResponse {
    pub code: String,
    pub message: String,
    pub details: Option<serde_json::Value>,
}

/// Entity creation request
#[derive(Debug, Deserialize)]
pub struct CreateEntityRequest {
    pub entity_type: String,
    pub name: String,
    pub instruction: Option<String>,
    pub data: HashMap<String, serde_json::Value>,
    pub link_to_cbu: Option<Uuid>,
}

/// Entity search request
#[derive(Debug, Deserialize)]
pub struct SearchEntitiesRequest {
    pub entity_type: Option<String>,
    pub name_contains: Option<String>,
    pub filters: Option<HashMap<String, serde_json::Value>>,
    pub limit: Option<i32>,
    pub offset: Option<i32>,
}

/// Entity response
#[derive(Debug, Serialize)]
pub struct EntityResponse {
    pub id: Uuid,
    pub entity_type: String,
    pub name: String,
    pub data: serde_json::Value,
    pub created_at: chrono::DateTime<chrono::Utc>,
    pub updated_at: chrono::DateTime<chrono::Utc>,
}

/// AI DSL generation request
#[derive(Debug, Deserialize)]
pub struct GenerateDslRequest {
    pub instruction: String,
    pub entity_type: String,
    pub operation_type: String,
    pub context: Option<HashMap<String, serde_json::Value>>,
}

/// AI DSL generation response
#[derive(Debug, Serialize)]
pub struct GenerateDslResponse {
    pub dsl_content: String,
    pub confidence: f64,
    pub provider_used: String,
    pub explanation: String,
}

/// Health check response
#[derive(Serialize)]
pub struct HealthResponse {
    pub status: String,
    pub timestamp: chrono::DateTime<chrono::Utc>,
    pub database: String,
    pub ai_services: HashMap<String, String>,
    pub uptime_seconds: u64,
}

#[tokio::main]
async fn main() -> Result<(), Box<dyn std::error::Error>> {
    // Initialize tracing
    tracing_subscriber::fmt::init();

    info!("🚀 Starting OB-POC Web API Server");

    // Load environment variables
    dotenvy::dotenv().ok();

    // Load configuration
    let config = ApiConfig {
        host: env::var("API_HOST").unwrap_or_else(|_| "0.0.0.0".to_string()),
        port: env::var("API_PORT")
            .unwrap_or_else(|_| "3001".to_string())
            .parse()
            .unwrap_or(3001),
        cors_origins: env::var("CORS_ORIGINS")
            .unwrap_or_else(|_| "http://localhost:3000".to_string())
            .split(',')
            .map(|s| s.trim().to_string())
            .collect(),
        enable_ai: env::var("OPENAI_API_KEY").is_ok() || env::var("GEMINI_API_KEY").is_ok(),
        max_request_size: 1024 * 1024, // 1MB
    };

    info!("⚙️  Configuration:");
    info!("   Host: {}:{}", config.host, config.port);
    info!("   CORS Origins: {:?}", config.cors_origins);
    info!("   AI Enabled: {}", config.enable_ai);

    // Initialize database connection
    let database_url = env::var("DATABASE_URL")
        .expect("DATABASE_URL environment variable must be set");

    info!("🗄️  Connecting to database...");
    let pool = PgPool::connect(&database_url).await?;

    // Test database connection
    sqlx::query("SELECT 1")
        .fetch_one(&pool)
        .await
        .map_err(|e| {
            error!("Database connection failed: {}", e);
            e
        })?;

    info!("✅ Database connection successful");

    // Initialize services
    info!("🔧 Initializing services...");

    let rag_system = CrudRagSystem::new();
    let prompt_builder = CrudPromptBuilder::new();
    let entity_service = EntityCrudService::new(
        pool.clone(),
        rag_system.clone(),
        prompt_builder.clone(),
        None,
    );
    let transaction_manager = EntityTransactionManager::new(
        pool.clone(),
        entity_service.clone(),
        None,
    );

    let app_state = AppState {
        pool,
        entity_service: Arc::new(entity_service),
        transaction_manager: Arc::new(transaction_manager),
        config: config.clone(),
    };

    // Create CORS layer
    let cors = CorsLayer::new()
        .allow_methods([Method::GET, Method::POST, Method::PUT, Method::DELETE])
        .allow_headers([header::CONTENT_TYPE, header::AUTHORIZATION])
        .allow_origin(Any);

    // Build the application router
    let app = Router::new()
        // Health check
        .route("/health", get(health_check))
        .route("/", get(root))

        // Entity management endpoints
        .route("/api/entities", post(create_entity))
        .route("/api/entities", get(search_entities))
        .route("/api/entities/:id", get(get_entity))
        .route("/api/entities/:id", put(update_entity))
        .route("/api/entities/:id", delete(delete_entity))

        // AI integration endpoints
        .route("/api/ai/generate-dsl", post(generate_dsl))
        .route("/api/ai/validate-dsl", post(validate_dsl))

        // Transaction management endpoints
        .route("/api/transactions", post(create_transaction))
        .route("/api/transactions/:id", get(get_transaction))
        .route("/api/transactions/:id/status", get(get_transaction_status))

        // Add middleware
        .layer(
            ServiceBuilder::new()
                .layer(TraceLayer::new_for_http())
                .layer(middleware::from_fn(request_logging_middleware))
                .layer(cors)
        )
        .with_state(app_state);

    // Start the server
    let addr = SocketAddr::from(([0, 0, 0, 0], config.port));
    info!("🌐 Server starting on http://{}", addr);

    let listener = tokio::net::TcpListener::bind(addr).await?;
    info!("✅ Server ready to accept connections");

    axum::serve(listener, app).await?;

    Ok(())
}

/// Root endpoint - API information
async fn root() -> impl IntoResponse {
    let response = ApiResponse {
        success: true,
        data: Some(serde_json::json!({
            "service": "OB-POC Web API",
            "version": "1.0.0",
            "description": "REST API for agentic entity CRUD operations",
            "endpoints": {
                "health": "GET /health",
                "entities": "GET|POST|PUT|DELETE /api/entities",
                "ai": "POST /api/ai/generate-dsl",
                "transactions": "GET|POST /api/transactions"
            }
        })),
        error: None,
        timestamp: chrono::Utc::now(),
        request_id: None,
    };

    Json(response)
}

/// Health check endpoint
async fn health_check(State(state): State<AppState>) -> impl IntoResponse {
    let start_time = Instant::now();

    // Test database connectivity
    let db_status = match sqlx::query("SELECT 1").fetch_one(&state.pool).await {
        Ok(_) => "healthy".to_string(),
        Err(e) => {
            warn!("Database health check failed: {}", e);
            "unhealthy".to_string()
        }
    };

    // Check AI service availability
    let mut ai_services = HashMap::new();
    ai_services.insert(
        "openai".to_string(),
        if env::var("OPENAI_API_KEY").is_ok() {
            "configured".to_string()
        } else {
            "not_configured".to_string()
        }
    );
    ai_services.insert(
        "gemini".to_string(),
        if env::var("GEMINI_API_KEY").is_ok() {
            "configured".to_string()
        } else {
            "not_configured".to_string()
        }
    );

    let health = HealthResponse {
        status: if db_status == "healthy" { "healthy" } else { "unhealthy" }.to_string(),
        timestamp: chrono::Utc::now(),
        database: db_status,
        ai_services,
        uptime_seconds: start_time.elapsed().as_secs(),
    };

    let status_code = if health.status == "healthy" {
        StatusCode::OK
    } else {
        StatusCode::SERVICE_UNAVAILABLE
    };

    (status_code, Json(ApiResponse {
        success: health.status == "healthy",
        data: Some(health),
        error: None,
        timestamp: chrono::Utc::now(),
        request_id: None,
    }))
}

/// Create a new entity
async fn create_entity(
    State(_state): State<AppState>,
    Json(request): Json<CreateEntityRequest>,
) -> impl IntoResponse {
    info!("Creating entity: {} ({})", request.name, request.entity_type);

    // For now, return a mock response - in production this would use the entity service
    let entity_id = Uuid::new_v4();
    let now = chrono::Utc::now();

    let entity = EntityResponse {
        id: entity_id,
        entity_type: request.entity_type,
        name: request.name,
        data: serde_json::json!(request.data),
        created_at: now,
        updated_at: now,
    };

    Json(ApiResponse {
        success: true,
        data: Some(entity),
        error: None,
        timestamp: chrono::Utc::now(),
        request_id: Some(Uuid::new_v4().to_string()),
    })
}

/// Search entities
async fn search_entities(
    State(_state): State<AppState>,
    Query(params): Query<SearchEntitiesRequest>,
) -> impl IntoResponse {
    info!("Searching entities with filters: {:?}", params);

    // Mock response - in production would query database
    let entities = vec![
        EntityResponse {
            id: Uuid::new_v4(),
            entity_type: "partnership".to_string(),
            name: "Example Partnership LP".to_string(),
            data: serde_json::json!({
                "jurisdiction": "US-DE",
                "partnership_type": "Limited Liability"
            }),
            created_at: chrono::Utc::now(),
            updated_at: chrono::Utc::now(),
        }
    ];

    Json(ApiResponse {
        success: true,
        data: Some(entities),
        error: None,
        timestamp: chrono::Utc::now(),
        request_id: Some(Uuid::new_v4().to_string()),
    })
}

/// Get specific entity
async fn get_entity(
    State(_state): State<AppState>,
    Path(id): Path<Uuid>,
) -> impl IntoResponse {
    info!("Getting entity: {}", id);

    // Mock response
    let entity = EntityResponse {
        id,
        entity_type: "partnership".to_string(),
        name: "Example Entity".to_string(),
        data: serde_json::json!({}),
        created_at: chrono::Utc::now(),
        updated_at: chrono::Utc::now(),
    };

    Json(ApiResponse {
        success: true,
        data: Some(entity),
        error: None,
        timestamp: chrono::Utc::now(),
        request_id: Some(Uuid::new_v4().to_string()),
    })
}

/// Update entity
async fn update_entity(
    State(_state): State<AppState>,
    Path(id): Path<Uuid>,
    Json(updates): Json<HashMap<String, serde_json::Value>>,
) -> impl IntoResponse {
    info!("Updating entity {}: {:?}", id, updates);

    // Mock response
    let entity = EntityResponse {
        id,
        entity_type: "partnership".to_string(),
        name: "Updated Entity".to_string(),
        data: serde_json::json!(updates),
        created_at: chrono::Utc::now() - chrono::Duration::hours(1),
        updated_at: chrono::Utc::now(),
    };

    Json(ApiResponse {
        success: true,
        data: Some(entity),
        error: None,
        timestamp: chrono::Utc::now(),
        request_id: Some(Uuid::new_v4().to_string()),
    })
}

/// Delete entity
async fn delete_entity(
    State(_state): State<AppState>,
    Path(id): Path<Uuid>,
) -> impl IntoResponse {
    info!("Deleting entity: {}", id);

    Json(ApiResponse {
        success: true,
        data: Some(serde_json::json!({"deleted": true, "id": id})),
        error: None,
        timestamp: chrono::Utc::now(),
        request_id: Some(Uuid::new_v4().to_string()),
    })
}

/// Generate DSL from natural language
async fn generate_dsl(
    State(state): State<AppState>,
    Json(request): Json<GenerateDslRequest>,
) -> impl IntoResponse {
    info!("Generating DSL: {}", request.instruction);

    if !state.config.enable_ai {
        let response = GenerateDslResponse {
            dsl_content: format!(
                "(data.{} :asset \"{}\" :values {{:name \"Generated Entity\"}})",
                request.operation_type, request.entity_type
            ),
            confidence: 0.7,
            provider_used: "fallback".to_string(),
            explanation: "AI services not configured, using pattern-based generation".to_string(),
        };

        return Json(ApiResponse {
            success: true,
            data: Some(response),
            error: None,
            timestamp: chrono::Utc::now(),
            request_id: Some(Uuid::new_v4().to_string()),
        });
    }

    // Mock AI response - in production would call actual AI service
    let response = GenerateDslResponse {
        dsl_content: format!(
            "(data.{} :asset \"{}\" :values {{:name \"{}\"}})",
            request.operation_type,
            request.entity_type,
            request.instruction.replace("Create ", "").replace("Update ", "")
        ),
        confidence: 0.95,
        provider_used: "openai".to_string(),
        explanation: "Generated DSL based on natural language instruction".to_string(),
    };

    Json(ApiResponse {
        success: true,
        data: Some(response),
        error: None,
        timestamp: chrono::Utc::now(),
        request_id: Some(Uuid::new_v4().to_string()),
    })
}

/// Validate DSL syntax
async fn validate_dsl(
    State(_state): State<AppState>,
    Json(dsl_content): Json<String>,
) -> impl IntoResponse {
    info!("Validating DSL: {}", dsl_content);

    let is_valid = dsl_content.starts_with('(') && dsl_content.ends_with(')');
    let validation_result = serde_json::json!({
        "valid": is_valid,
        "errors": if is_valid { Vec::<String>::new() } else { vec!["Invalid S-expression syntax".to_string()] },
        "warnings": Vec::<String>::new()
    });

    Json(ApiResponse {
        success: true,
        data: Some(validation_result),
        error: None,
        timestamp: chrono::Utc::now(),
        request_id: Some(Uuid::new_v4().to_string()),
    })
}

/// Create batch transaction
async fn create_transaction(
    State(_state): State<AppState>,
    Json(operations): Json<serde_json::Value>,
) -> impl IntoResponse {
    info!("Creating transaction with operations: {:?}", operations);

    let transaction_id = Uuid::new_v4();
    let result = serde_json::json!({
        "transaction_id": transaction_id,
        "status": "pending",
        "operations_count": 0,
        "created_at": chrono::Utc::now()
    });

    Json(ApiResponse {
        success: true,
        data: Some(result),
        error: None,
        timestamp: chrono::Utc::now(),
        request_id: Some(Uuid::new_v4().to_string()),
    })
}

/// Get transaction details
async fn get_transaction(
    State(_state): State<AppState>,
    Path(id): Path<Uuid>,
) -> impl IntoResponse {
    info!("Getting transaction: {}", id);

    let result = serde_json::json!({
        "transaction_id": id,
        "status": "completed",
        "operations_completed": 3,
        "operations_failed": 0,
        "execution_time_ms": 150
    });

    Json(ApiResponse {
        success: true,
        data: Some(result),
        error: None,
        timestamp: chrono::Utc::now(),
        request_id: Some(Uuid::new_v4().to_string()),
    })
}

/// Get transaction status
async fn get_transaction_status(
    State(_state): State<AppState>,
    Path(id): Path<Uuid>,
) -> impl IntoResponse {
    info!("Getting transaction status: {}", id);

    let result = serde_json::json!({
        "transaction_id": id,
        "status": "completed",
        "progress_percent": 100
    });

    Json(ApiResponse {
        success: true,
        data: Some(result),
        error: None,
        timestamp: chrono::Utc::now(),
        request_id: Some(Uuid::new_v4().to_string()),
    })
}

/// Request logging middleware
async fn request_logging_middleware(
    request: axum::extract::Request,
    next: Next,
) -> Result<Response, StatusCode> {
    let method = request.method().clone();
    let uri = request.uri().clone();
    let start_time = Instant::now();

    let response = next.run(request).await;

    let duration = start_time.elapsed();
    let status = response.status();

    debug!(
        "{} {} - {} - {:?}",
        method,
        uri,
        status,
        duration
    );

    Ok(response)
}
