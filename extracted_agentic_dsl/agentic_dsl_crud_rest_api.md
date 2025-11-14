# AGENTIC DSL CRUD - REST API Implementation
## Drop this into Zed Claude: "Implement this REST API for agentic DSL CRUD"

### Quick Start Commands:
```bash
# 1. Run SQL migrations
psql -d ob_poc -f migrations.sql

# 2. Build and run
cargo build --release
cargo run

# 3. Test with curl (see bottom of file)
```

### File 1: `src/api/agentic_api.rs`
```rust
//! REST API for Agentic DSL CRUD Operations
//! Natural Language → AI → DSL → Database

use axum::{
    extract::{Path, Query, State},
    http::StatusCode,
    response::Json,
    Router,
    routing::{get, post},
};
use serde::{Deserialize, Serialize};
use sqlx::PgPool;
use std::sync::Arc;
use uuid::Uuid;

// ============================================================================
// Request/Response Types
// ============================================================================

#[derive(Debug, Deserialize)]
pub struct CreateCbuRequest {
    /// Natural language instruction
    /// Example: "Create a CBU with Nature and Purpose 'Banking Services' and Source of funds 'Investment Returns'"
    pub instruction: String,
}

#[derive(Debug, Serialize)]
pub struct CreateCbuResponse {
    pub success: bool,
    pub cbu_id: Uuid,
    pub name: String,
    pub nature_purpose: String,
    pub source_of_funds: String,
    pub generated_dsl: String,
    pub message: String,
}

#[derive(Debug, Deserialize)]
pub struct ConnectEntityRequest {
    /// Natural language or structured instruction
    /// Example: "Connect entity {uuid} to CBU {uuid} as Director"
    pub instruction: String,
    /// Optional: If provided, will be injected into instruction
    pub entity_id: Option<Uuid>,
    pub cbu_id: Option<Uuid>,
    pub role_name: Option<String>,
}

#[derive(Debug, Serialize)]
pub struct ConnectEntityResponse {
    pub success: bool,
    pub connection_id: Uuid,
    pub entity_id: Uuid,
    pub cbu_id: Uuid,
    pub role_id: Uuid,
    pub generated_dsl: String,
    pub message: String,
}

#[derive(Debug, Serialize)]
pub struct CbuDetails {
    pub cbu_id: Uuid,
    pub name: String,
    pub nature_purpose: Option<String>,
    pub source_of_funds: Option<String>,
    pub created_at: chrono::DateTime<chrono::Utc>,
    pub entities: Vec<EntityConnection>,
}

#[derive(Debug, Serialize)]
pub struct EntityConnection {
    pub entity_id: Uuid,
    pub entity_name: String,
    pub role_name: String,
    pub connected_at: chrono::DateTime<chrono::Utc>,
}

// ============================================================================
// API Handlers
// ============================================================================

/// POST /api/agentic/cbu
/// Creates a new CBU from natural language instruction
pub async fn create_cbu_handler(
    State(state): State<AppState>,
    Json(request): Json<CreateCbuRequest>,
) -> Result<Json<CreateCbuResponse>, (StatusCode, String)> {
    // Parse instruction to extract nature/purpose and source
    let (nature, source) = parse_cbu_instruction(&request.instruction);
    
    // Generate CBU
    let cbu_id = Uuid::new_v4();
    let name = format!("CBU-{}", &cbu_id.to_string()[..8]);
    
    // Generate DSL for audit
    let generated_dsl = format!(
        r#"CREATE CBU WITH nature_purpose "{}" AND source_of_funds "{}""#,
        nature, source
    );
    
    // Execute database insertion
    let result = sqlx::query!(
        r#"
        INSERT INTO "ob-poc".cbus (cbu_id, name, nature_purpose, description)
        VALUES ($1, $2, $3, $4)
        RETURNING cbu_id
        "#,
        cbu_id,
        name,
        nature,
        source // Using description field for source_of_funds
    )
    .fetch_one(&state.pool)
    .await
    .map_err(|e| (StatusCode::INTERNAL_SERVER_ERROR, e.to_string()))?;
    
    // Log to crud_operations
    log_crud_operation(&state.pool, &request.instruction, &generated_dsl, Some(cbu_id)).await;
    
    Ok(Json(CreateCbuResponse {
        success: true,
        cbu_id: result.cbu_id,
        name: name.clone(),
        nature_purpose: nature,
        source_of_funds: source,
        generated_dsl,
        message: format!("Successfully created CBU '{}'", name),
    }))
}

/// POST /api/agentic/entity/connect
/// Connects an entity to a CBU with a specific role
pub async fn connect_entity_handler(
    State(state): State<AppState>,
    Json(request): Json<ConnectEntityRequest>,
) -> Result<Json<ConnectEntityResponse>, (StatusCode, String)> {
    // Parse or use provided values
    let (entity_id, cbu_id, role_id) = if let (Some(e), Some(c), Some(r)) = 
        (request.entity_id, request.cbu_id, request.role_name.as_ref()) {
        // Use provided values
        let role_id = get_or_create_role(&state.pool, r).await
            .map_err(|e| (StatusCode::INTERNAL_SERVER_ERROR, e.to_string()))?;
        (e, c, role_id)
    } else {
        // Parse from instruction
        parse_connect_instruction(&request.instruction)
            .map_err(|e| (StatusCode::BAD_REQUEST, e.to_string()))?
    };
    
    // Generate connection
    let connection_id = Uuid::new_v4();
    let generated_dsl = format!(
        "CONNECT ENTITY {} TO CBU {} AS ROLE {}",
        entity_id, cbu_id, role_id
    );
    
    // Execute
    sqlx::query!(
        r#"
        INSERT INTO "ob-poc".cbu_entity_roles 
        (cbu_entity_role_id, cbu_id, entity_id, role_id)
        VALUES ($1, $2, $3, $4)
        "#,
        connection_id,
        cbu_id,
        entity_id,
        role_id
    )
    .execute(&state.pool)
    .await
    .map_err(|e| (StatusCode::INTERNAL_SERVER_ERROR, e.to_string()))?;
    
    // Log operation
    log_crud_operation(&state.pool, &request.instruction, &generated_dsl, Some(cbu_id)).await;
    
    Ok(Json(ConnectEntityResponse {
        success: true,
        connection_id,
        entity_id,
        cbu_id,
        role_id,
        generated_dsl,
        message: format!("Successfully connected entity to CBU"),
    }))
}

/// GET /api/agentic/cbu/{id}
/// Retrieves CBU details including connected entities
pub async fn get_cbu_handler(
    State(state): State<AppState>,
    Path(cbu_id): Path<Uuid>,
) -> Result<Json<CbuDetails>, (StatusCode, String)> {
    // Get CBU details
    let cbu = sqlx::query!(
        r#"
        SELECT cbu_id, name, nature_purpose, description as source_of_funds, created_at
        FROM "ob-poc".cbus
        WHERE cbu_id = $1
        "#,
        cbu_id
    )
    .fetch_optional(&state.pool)
    .await
    .map_err(|e| (StatusCode::INTERNAL_SERVER_ERROR, e.to_string()))?
    .ok_or((StatusCode::NOT_FOUND, "CBU not found".to_string()))?;
    
    // Get connected entities
    let entities = sqlx::query!(
        r#"
        SELECT 
            er.entity_id,
            e.name as entity_name,
            'Director' as role_name, -- Simplified, should join with roles table
            er.created_at
        FROM "ob-poc".cbu_entity_roles er
        JOIN "ob-poc".entities e ON e.entity_id = er.entity_id
        WHERE er.cbu_id = $1
        "#,
        cbu_id
    )
    .fetch_all(&state.pool)
    .await
    .map_err(|e| (StatusCode::INTERNAL_SERVER_ERROR, e.to_string()))?;
    
    Ok(Json(CbuDetails {
        cbu_id: cbu.cbu_id,
        name: cbu.name,
        nature_purpose: cbu.nature_purpose,
        source_of_funds: cbu.source_of_funds,
        created_at: cbu.created_at.unwrap_or_else(chrono::Utc::now),
        entities: entities.into_iter().map(|e| EntityConnection {
            entity_id: e.entity_id,
            entity_name: e.entity_name,
            role_name: e.role_name,
            connected_at: e.created_at.unwrap_or_else(chrono::Utc::now),
        }).collect(),
    }))
}

/// GET /api/agentic/test
/// Test endpoint to verify API is running
pub async fn test_handler() -> &'static str {
    "Agentic DSL CRUD API is running!"
}

// ============================================================================
// Helper Functions
// ============================================================================

fn parse_cbu_instruction(instruction: &str) -> (String, String) {
    let lower = instruction.to_lowercase();
    
    // Extract nature and purpose
    let nature = if let Some(start) = lower.find("nature and purpose") {
        let after_nature = &instruction[start + 18..];
        if let Some(end) = after_nature.find("and source") {
            after_nature[..end].trim().trim_matches('"').to_string()
        } else {
            after_nature.trim().trim_matches('"').to_string()
        }
    } else {
        "General Banking Services".to_string()
    };
    
    // Extract source of funds
    let source = if let Some(start) = lower.find("source of funds") {
        instruction[start + 15..]
            .trim()
            .trim_matches('"')
            .to_string()
    } else {
        "Corporate Operations".to_string()
    };
    
    (nature, source)
}

fn parse_connect_instruction(instruction: &str) -> Result<(Uuid, Uuid, Uuid), String> {
    // Extract UUIDs from instruction
    let uuids: Vec<&str> = instruction
        .split_whitespace()
        .filter(|s| s.len() == 36 && s.contains('-'))
        .collect();
    
    if uuids.len() < 3 {
        return Err("Instruction must contain entity_id, cbu_id, and role_id UUIDs".to_string());
    }
    
    Ok((
        Uuid::parse_str(uuids[0]).map_err(|e| e.to_string())?,
        Uuid::parse_str(uuids[1]).map_err(|e| e.to_string())?,
        Uuid::parse_str(uuids[2]).map_err(|e| e.to_string())?,
    ))
}

async fn get_or_create_role(pool: &PgPool, role_name: &str) -> Result<Uuid, sqlx::Error> {
    // In production, lookup from roles table
    // For now, create deterministic UUID from role name
    Ok(Uuid::new_v5(&Uuid::NAMESPACE_DNS, role_name.as_bytes()))
}

async fn log_crud_operation(
    pool: &PgPool,
    instruction: &str,
    dsl: &str,
    cbu_id: Option<Uuid>,
) {
    let _ = sqlx::query!(
        r#"
        INSERT INTO "ob-poc".crud_operations
        (operation_id, operation_type, asset_type, generated_dsl, 
         ai_instruction, natural_language_input, cbu_id)
        VALUES ($1, $2, $3, $4, $5, $6, $7)
        "#,
        Uuid::new_v4(),
        "CREATE",
        "CBU",
        dsl,
        instruction,
        instruction,
        cbu_id
    )
    .execute(pool)
    .await;
}

// ============================================================================
// Application State
// ============================================================================

#[derive(Clone)]
pub struct AppState {
    pub pool: PgPool,
}

// ============================================================================
// Router Configuration
// ============================================================================

pub fn create_agentic_router(pool: PgPool) -> Router {
    let state = AppState { pool };
    
    Router::new()
        .route("/api/agentic/test", get(test_handler))
        .route("/api/agentic/cbu", post(create_cbu_handler))
        .route("/api/agentic/cbu/:id", get(get_cbu_handler))
        .route("/api/agentic/entity/connect", post(connect_entity_handler))
        .with_state(state)
}
```

### File 2: `src/main.rs` (Add to existing or create new)
```rust
use axum::{Router, Server};
use sqlx::PgPool;
use std::net::SocketAddr;
use tower_http::cors::CorsLayer;

mod api;
use api::agentic_api;

#[tokio::main]
async fn main() -> Result<(), Box<dyn std::error::Error>> {
    // Initialize logging
    tracing_subscriber::fmt::init();
    
    // Database connection
    let database_url = std::env::var("DATABASE_URL")
        .unwrap_or_else(|_| "postgresql://localhost:5432/ob-poc".to_string());
    let pool = PgPool::connect(&database_url).await?;
    
    // Build application
    let app = Router::new()
        .merge(agentic_api::create_agentic_router(pool))
        .layer(CorsLayer::permissive()); // Enable CORS for testing
    
    // Start server
    let addr = SocketAddr::from(([127, 0, 0, 1], 3000));
    println!("🚀 Agentic DSL CRUD API running on http://{}", addr);
    
    Server::bind(&addr)
        .serve(app.into_make_service())
        .await?;
    
    Ok(())
}
```

### File 3: `migrations.sql`
```sql
-- Run these migrations first

-- Ensure tables exist
CREATE TABLE IF NOT EXISTS "ob-poc".cbus (
    cbu_id UUID PRIMARY KEY DEFAULT gen_random_uuid(),
    name VARCHAR(255) NOT NULL,
    nature_purpose TEXT,
    description TEXT, -- We use this for source_of_funds
    created_at TIMESTAMP WITH TIME ZONE DEFAULT NOW(),
    updated_at TIMESTAMP WITH TIME ZONE DEFAULT NOW()
);

CREATE TABLE IF NOT EXISTS "ob-poc".entities (
    entity_id UUID PRIMARY KEY DEFAULT gen_random_uuid(),
    name VARCHAR(255) NOT NULL,
    entity_type VARCHAR(50),
    created_at TIMESTAMP WITH TIME ZONE DEFAULT NOW()
);

CREATE TABLE IF NOT EXISTS "ob-poc".cbu_entity_roles (
    cbu_entity_role_id UUID PRIMARY KEY DEFAULT gen_random_uuid(),
    cbu_id UUID NOT NULL REFERENCES "ob-poc".cbus(cbu_id) ON DELETE CASCADE,
    entity_id UUID NOT NULL REFERENCES "ob-poc".entities(entity_id) ON DELETE CASCADE,
    role_id UUID NOT NULL,
    created_at TIMESTAMP WITH TIME ZONE DEFAULT NOW()
);

CREATE TABLE IF NOT EXISTS "ob-poc".crud_operations (
    operation_id UUID PRIMARY KEY DEFAULT gen_random_uuid(),
    operation_type VARCHAR(20) NOT NULL,
    asset_type VARCHAR(50) NOT NULL,
    entity_table_name VARCHAR(100),
    generated_dsl TEXT NOT NULL,
    ai_instruction TEXT NOT NULL,
    natural_language_input TEXT,
    parsed_ast JSONB,
    affected_records JSONB DEFAULT '[]'::jsonb,
    execution_status VARCHAR(20) DEFAULT 'PENDING',
    ai_confidence NUMERIC(3,2),
    ai_provider VARCHAR(50),
    ai_model VARCHAR(100),
    execution_time_ms INTEGER,
    error_message TEXT,
    created_by VARCHAR(255) DEFAULT 'agentic_system',
    created_at TIMESTAMP WITH TIME ZONE DEFAULT NOW(),
    completed_at TIMESTAMP WITH TIME ZONE,
    rows_affected INTEGER DEFAULT 0,
    transaction_id UUID,
    parent_operation_id UUID,
    cbu_id UUID REFERENCES "ob-poc".cbus(cbu_id)
);

-- Create indexes
CREATE INDEX IF NOT EXISTS idx_cbu_entity_roles_cbu ON "ob-poc".cbu_entity_roles(cbu_id);
CREATE INDEX IF NOT EXISTS idx_cbu_entity_roles_entity ON "ob-poc".cbu_entity_roles(entity_id);
CREATE INDEX IF NOT EXISTS idx_crud_operations_cbu ON "ob-poc".crud_operations(cbu_id);
```

### Testing with cURL:

```bash
# 1. Test API is running
curl http://localhost:3000/api/agentic/test

# 2. Create a CBU from natural language
curl -X POST http://localhost:3000/api/agentic/cbu \
  -H "Content-Type: application/json" \
  -d '{
    "instruction": "Create a CBU with Nature and Purpose \"Investment Banking Services for High Net Worth Individuals\" and Source of funds \"Private Equity Returns and Investment Portfolio\""
  }'

# Response:
# {
#   "success": true,
#   "cbu_id": "123e4567-e89b-12d3-a456-426614174000",
#   "name": "CBU-123e4567",
#   "nature_purpose": "Investment Banking Services for High Net Worth Individuals",
#   "source_of_funds": "Private Equity Returns and Investment Portfolio",
#   "generated_dsl": "CREATE CBU WITH nature_purpose \"...\" AND source_of_funds \"...\"",
#   "message": "Successfully created CBU 'CBU-123e4567'"
# }

# 3. Create a test entity
curl -X POST http://localhost:3000/api/entities \
  -H "Content-Type: application/json" \
  -d '{
    "name": "John Smith",
    "entity_type": "PERSON"
  }'

# 4. Connect entity to CBU
curl -X POST http://localhost:3000/api/agentic/entity/connect \
  -H "Content-Type: application/json" \
  -d '{
    "entity_id": "entity-uuid-here",
    "cbu_id": "cbu-uuid-here",
    "role_name": "Director"
  }'

# 5. Get CBU details with connected entities
curl http://localhost:3000/api/agentic/cbu/123e4567-e89b-12d3-a456-426614174000

# Response:
# {
#   "cbu_id": "123e4567-e89b-12d3-a456-426614174000",
#   "name": "CBU-123e4567",
#   "nature_purpose": "Investment Banking Services...",
#   "source_of_funds": "Private Equity Returns...",
#   "created_at": "2024-11-14T16:30:00Z",
#   "entities": [
#     {
#       "entity_id": "entity-uuid",
#       "entity_name": "John Smith",
#       "role_name": "Director",
#       "connected_at": "2024-11-14T16:31:00Z"
#     }
#   ]
# }
```

### Cargo.toml additions:
```toml
[dependencies]
axum = "0.6"
tokio = { version = "1", features = ["full"] }
sqlx = { version = "0.7", features = ["postgres", "runtime-tokio-native-tls", "uuid", "chrono"] }
serde = { version = "1", features = ["derive"] }
serde_json = "1"
uuid = { version = "1", features = ["v4", "v5", "serde"] }
chrono = { version = "0.4", features = ["serde"] }
tower-http = { version = "0.4", features = ["cors"] }
tracing = "0.1"
tracing-subscriber = "0.3"
```

### Environment variables (.env):
```env
DATABASE_URL=postgresql://username:password@localhost:5432/ob-poc
RUST_LOG=info
```

## Next Steps:

1. **Add OpenAI/Anthropic Integration**: Replace the simple parsing with actual LLM calls
2. **Add Authentication**: Secure the endpoints with JWT or API keys
3. **Add Validation**: Validate CBU nature/purpose against business rules
4. **Add Batch Operations**: Support creating multiple CBUs at once
5. **Add Workflow Triggers**: Trigger onboarding workflows on CBU creation
6. **Add WebSocket Support**: Real-time updates on DSL generation progress

**Instructions for Zed Claude:**
"Implement this REST API for agentic DSL CRUD operations. Start with the migrations, then create the API module, and finally test with the provided cURL commands."
