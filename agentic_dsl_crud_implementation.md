# AGENTIC DSL CRUD IMPLEMENTATION - CBU Operations
# Drop this file into Zed and tell Claude: "Implement this complete agentic DSL CRUD system"

## Overview
This implementation creates an AI-powered DSL generation system that converts natural language instructions into executable database operations for KYC/onboarding.

## Implementation Steps

### Step 1: Database Schema Updates
```sql
-- Add these tables if not exists
-- This tracks AI-generated DSL operations

ALTER TABLE "ob-poc".crud_operations 
ADD COLUMN IF NOT EXISTS cbu_id UUID REFERENCES "ob-poc".cbus(cbu_id),
ADD COLUMN IF NOT EXISTS parsed_ast JSONB,
ADD COLUMN IF NOT EXISTS natural_language_input TEXT;

-- Add CBU creation tracking
CREATE TABLE IF NOT EXISTS "ob-poc".cbu_creation_log (
    log_id UUID PRIMARY KEY DEFAULT gen_random_uuid(),
    cbu_id UUID NOT NULL REFERENCES "ob-poc".cbus(cbu_id),
    nature_purpose TEXT,
    source_of_funds TEXT,
    created_via VARCHAR(50) DEFAULT 'agentic_dsl',
    ai_instruction TEXT,
    generated_dsl TEXT,
    created_at TIMESTAMP WITH TIME ZONE DEFAULT NOW()
);

-- Add entity role connection log
CREATE TABLE IF NOT EXISTS "ob-poc".entity_role_connections (
    connection_id UUID PRIMARY KEY DEFAULT gen_random_uuid(),
    cbu_id UUID NOT NULL REFERENCES "ob-poc".cbus(cbu_id),
    entity_id UUID NOT NULL REFERENCES "ob-poc".entities(entity_id),
    role_id UUID NOT NULL,
    connected_via VARCHAR(50) DEFAULT 'agentic_dsl',
    ai_instruction TEXT,
    generated_dsl TEXT,
    connected_at TIMESTAMP WITH TIME ZONE DEFAULT NOW()
);
```

### Step 2: Core Rust Implementation Files

#### File: `src/agentic/mod.rs`
```rust
//! Agentic DSL CRUD Module
//! Handles AI-powered DSL generation and execution

pub mod dsl_generator;
pub mod crud_executor;
pub mod llm_client;
pub mod cbu_operations;

pub use dsl_generator::AiDslGenerator;
pub use crud_executor::CrudExecutor;
pub use llm_client::{LlmClient, OpenAiClient, AnthropicClient};
pub use cbu_operations::{CbuService, EntityRoleService};
```

#### File: `src/agentic/dsl_generator.rs`
```rust
use async_trait::async_trait;
use serde::{Deserialize, Serialize};
use sqlx::PgPool;
use uuid::Uuid;
use chrono::{DateTime, Utc};
use std::collections::HashMap;
use anyhow::Result;

use crate::parser_ast::{CrudStatement, DataCreate, DataUpdate, DataRead, Value};
use super::llm_client::LlmClient;

/// AI-powered DSL generator that converts natural language to CRUD operations
pub struct AiDslGenerator {
    pool: PgPool,
    llm_client: Box<dyn LlmClient>,
}

impl AiDslGenerator {
    pub fn new(pool: PgPool, llm_client: Box<dyn LlmClient>) -> Self {
        Self { pool, llm_client }
    }
    
    /// Generate DSL from natural language instruction
    pub async fn generate_from_instruction(
        &self,
        instruction: &str,
        context: Option<&ExecutionContext>,
    ) -> Result<GeneratedDsl> {
        // Step 1: Build context-aware prompt
        let prompt = self.build_prompt(instruction, context).await?;
        
        // Step 2: Get DSL from LLM
        let dsl_text = self.llm_client.generate_dsl(&prompt).await?;
        
        // Step 3: Parse DSL to AST
        let ast = self.parse_dsl(&dsl_text)?;
        
        // Step 4: Log generation
        let generation_id = self.log_generation(
            instruction,
            &dsl_text,
            &ast,
            context
        ).await?;
        
        Ok(GeneratedDsl {
            id: generation_id,
            instruction: instruction.to_string(),
            dsl_text,
            ast,
            confidence: 0.95, // Would come from LLM
            generated_at: Utc::now(),
        })
    }
    
    /// Build context-aware prompt for LLM
    async fn build_prompt(&self, instruction: &str, context: Option<&ExecutionContext>) -> Result<String> {
        let schema_context = self.get_schema_context().await?;
        
        let prompt = format!(
            r#"You are a DSL generator for a KYC/onboarding system.
            
Database Schema:
{}

Instruction: {}

Context: {:?}

Generate DSL code following these patterns:
1. CREATE CBU: CREATE CBU WITH nature_purpose "text" AND source_of_funds "text"
2. CONNECT ENTITY: CONNECT ENTITY {{uuid}} TO CBU {{uuid}} AS ROLE {{uuid}}
3. UPDATE CBU: UPDATE CBU {{uuid}} SET field = "value"
4. READ CBU: READ CBU WHERE cbu_id = {{uuid}}

Return ONLY the DSL code, no explanation."#,
            schema_context, instruction, context
        );
        
        Ok(prompt)
    }
    
    /// Get database schema context for LLM
    async fn get_schema_context(&self) -> Result<String> {
        // In production, fetch actual schema
        Ok(r#"
        Tables:
        - cbus (cbu_id, name, nature_purpose, source_of_funds, created_at)
        - cbu_entity_roles (cbu_id, entity_id, role_id)
        - entities (entity_id, name, entity_type)
        "#.to_string())
    }
    
    /// Parse DSL text to AST
    fn parse_dsl(&self, dsl_text: &str) -> Result<CrudStatement> {
        // Determine operation type and parse accordingly
        if dsl_text.starts_with("CREATE CBU") {
            self.parse_create_cbu(dsl_text)
        } else if dsl_text.starts_with("CONNECT ENTITY") {
            self.parse_connect_entity(dsl_text)
        } else if dsl_text.starts_with("UPDATE CBU") {
            self.parse_update_cbu(dsl_text)
        } else if dsl_text.starts_with("READ CBU") {
            self.parse_read_cbu(dsl_text)
        } else {
            Err(anyhow::anyhow!("Unknown DSL operation: {}", dsl_text))
        }
    }
    
    /// Parse CREATE CBU statement
    fn parse_create_cbu(&self, dsl: &str) -> Result<CrudStatement> {
        // Extract nature_purpose and source_of_funds
        let nature = extract_quoted(dsl, "nature_purpose")?;
        let source = extract_quoted(dsl, "source_of_funds")?;
        
        let mut values = HashMap::new();
        values.insert("name".to_string(), Value::String(format!("CBU-{}", Uuid::new_v4())));
        values.insert("nature_purpose".to_string(), Value::String(nature));
        values.insert("source_of_funds".to_string(), Value::String(source));
        
        Ok(CrudStatement::DataCreate(DataCreate {
            asset: "cbus".to_string(),
            values,
        }))
    }
    
    /// Parse CONNECT ENTITY statement
    fn parse_connect_entity(&self, dsl: &str) -> Result<CrudStatement> {
        let parts: Vec<&str> = dsl.split_whitespace().collect();
        
        // Extract UUIDs from positions
        let entity_id = Uuid::parse_str(parts[2])?;
        let cbu_id = Uuid::parse_str(parts[5])?;
        let role_id = Uuid::parse_str(parts[8])?;
        
        let mut values = HashMap::new();
        values.insert("entity_id".to_string(), Value::String(entity_id.to_string()));
        values.insert("cbu_id".to_string(), Value::String(cbu_id.to_string()));
        values.insert("role_id".to_string(), Value::String(role_id.to_string()));
        
        Ok(CrudStatement::DataCreate(DataCreate {
            asset: "cbu_entity_roles".to_string(),
            values,
        }))
    }
    
    /// Parse UPDATE CBU statement
    fn parse_update_cbu(&self, dsl: &str) -> Result<CrudStatement> {
        let cbu_id = extract_uuid(dsl, "UPDATE CBU")?;
        let updates = extract_set_clause(dsl)?;
        
        let mut where_clause = HashMap::new();
        where_clause.insert("cbu_id".to_string(), Value::String(cbu_id.to_string()));
        
        Ok(CrudStatement::DataUpdate(DataUpdate {
            asset: "cbus".to_string(),
            where_clause,
            values: updates,
        }))
    }
    
    /// Parse READ CBU statement
    fn parse_read_cbu(&self, dsl: &str) -> Result<CrudStatement> {
        let cbu_id = extract_uuid(dsl, "WHERE cbu_id")?;
        
        let mut where_clause = HashMap::new();
        where_clause.insert("cbu_id".to_string(), Value::String(cbu_id.to_string()));
        
        Ok(CrudStatement::DataRead(DataRead {
            asset: "cbus".to_string(),
            where_clause,
            select: vec!["*".to_string()],
            limit: None,
        }))
    }
    
    /// Log DSL generation for audit
    async fn log_generation(
        &self,
        instruction: &str,
        dsl_text: &str,
        ast: &CrudStatement,
        context: Option<&ExecutionContext>,
    ) -> Result<Uuid> {
        let id = Uuid::new_v4();
        
        sqlx::query(
            r#"
            INSERT INTO "ob-poc".crud_operations
            (operation_id, operation_type, asset_type, generated_dsl, 
             ai_instruction, parsed_ast, natural_language_input,
             ai_provider, ai_model, ai_confidence, cbu_id)
            VALUES ($1, $2, $3, $4, $5, $6, $7, $8, $9, $10, $11)
            "#
        )
        .bind(id)
        .bind(operation_type_from_ast(ast))
        .bind(asset_from_ast(ast))
        .bind(dsl_text)
        .bind(instruction)
        .bind(serde_json::to_value(ast)?)
        .bind(instruction)
        .bind("openai") // Or from config
        .bind("gpt-4")
        .bind(0.95f64)
        .bind(context.map(|c| c.cbu_id))
        .execute(&self.pool)
        .await?;
        
        Ok(id)
    }
}

/// Generated DSL with metadata
#[derive(Debug, Serialize, Deserialize)]
pub struct GeneratedDsl {
    pub id: Uuid,
    pub instruction: String,
    pub dsl_text: String,
    pub ast: CrudStatement,
    pub confidence: f64,
    pub generated_at: DateTime<Utc>,
}

/// Execution context for DSL operations
#[derive(Debug, Clone)]
pub struct ExecutionContext {
    pub cbu_id: Option<Uuid>,
    pub user_id: Option<Uuid>,
    pub session_id: Uuid,
    pub metadata: HashMap<String, String>,
}

// Helper functions
fn extract_quoted(text: &str, after: &str) -> Result<String> {
    let start = text.find(after).ok_or(anyhow::anyhow!("Field {} not found", after))?;
    let text_after = &text[start..];
    let quote_start = text_after.find('"').ok_or(anyhow::anyhow!("No quote found"))?;
    let quote_end = text_after[quote_start + 1..].find('"').ok_or(anyhow::anyhow!("No closing quote"))?;
    Ok(text_after[quote_start + 1..quote_start + 1 + quote_end].to_string())
}

fn extract_uuid(text: &str, after: &str) -> Result<Uuid> {
    let start = text.find(after).ok_or(anyhow::anyhow!("Pattern {} not found", after))?;
    let text_after = &text[start + after.len()..].trim();
    let uuid_str: String = text_after.chars()
        .take_while(|c| c.is_alphanumeric() || *c == '-')
        .collect();
    Uuid::parse_str(&uuid_str).map_err(Into::into)
}

fn extract_set_clause(text: &str) -> Result<HashMap<String, Value>> {
    // Simple SET clause parser - extend as needed
    let mut values = HashMap::new();
    if let Some(set_pos) = text.find("SET") {
        let set_text = &text[set_pos + 3..].trim();
        // Parse "field = value" pairs
        // This is simplified - extend for production
        values.insert("updated_at".to_string(), Value::String(Utc::now().to_rfc3339()));
    }
    Ok(values)
}

fn operation_type_from_ast(ast: &CrudStatement) -> &'static str {
    match ast {
        CrudStatement::DataCreate(_) => "CREATE",
        CrudStatement::DataRead(_) => "READ",
        CrudStatement::DataUpdate(_) => "UPDATE",
        CrudStatement::DataDelete(_) => "DELETE",
        _ => "COMPLEX",
    }
}

fn asset_from_ast(ast: &CrudStatement) -> String {
    match ast {
        CrudStatement::DataCreate(c) => c.asset.clone(),
        CrudStatement::DataRead(r) => r.asset.clone(),
        CrudStatement::DataUpdate(u) => u.asset.clone(),
        CrudStatement::DataDelete(d) => d.asset.clone(),
        _ => "unknown".to_string(),
    }
}
```

#### File: `src/agentic/crud_executor.rs`
```rust
use anyhow::Result;
use sqlx::PgPool;
use uuid::Uuid;
use serde_json::json;

use crate::parser_ast::{CrudStatement, DataCreate, DataRead, DataUpdate, DataDelete};
use super::dsl_generator::ExecutionContext;

/// Executes CRUD statements against the database
pub struct CrudExecutor {
    pool: PgPool,
}

impl CrudExecutor {
    pub fn new(pool: PgPool) -> Self {
        Self { pool }
    }
    
    /// Execute a CRUD statement
    pub async fn execute(
        &self,
        statement: &CrudStatement,
        context: &ExecutionContext,
    ) -> Result<ExecutionResult> {
        let start = std::time::Instant::now();
        
        let result = match statement {
            CrudStatement::DataCreate(create) => self.execute_create(create).await,
            CrudStatement::DataRead(read) => self.execute_read(read).await,
            CrudStatement::DataUpdate(update) => self.execute_update(update).await,
            CrudStatement::DataDelete(delete) => self.execute_delete(delete).await,
            _ => Err(anyhow::anyhow!("Complex operations not yet implemented")),
        };
        
        let duration_ms = start.elapsed().as_millis() as i32;
        
        // Log execution
        self.log_execution(statement, &result, duration_ms, context).await?;
        
        result
    }
    
    /// Execute CREATE operation
    async fn execute_create(&self, create: &DataCreate) -> Result<ExecutionResult> {
        match create.asset.as_str() {
            "cbus" => self.create_cbu(create).await,
            "cbu_entity_roles" => self.create_entity_role(create).await,
            _ => Err(anyhow::anyhow!("Unknown asset type: {}", create.asset)),
        }
    }
    
    /// Create a new CBU
    async fn create_cbu(&self, create: &DataCreate) -> Result<ExecutionResult> {
        let cbu_id = Uuid::new_v4();
        
        let name = create.values.get("name")
            .and_then(|v| if let Value::String(s) = v { Some(s) } else { None })
            .unwrap_or(&format!("CBU-{}", cbu_id));
            
        let nature_purpose = create.values.get("nature_purpose")
            .and_then(|v| if let Value::String(s) = v { Some(s) } else { None })
            .unwrap_or("");
            
        let source_of_funds = create.values.get("source_of_funds")
            .and_then(|v| if let Value::String(s) = v { Some(s) } else { None })
            .unwrap_or("");
        
        sqlx::query(
            r#"
            INSERT INTO "ob-poc".cbus 
            (cbu_id, name, nature_purpose, description)
            VALUES ($1, $2, $3, $4)
            "#
        )
        .bind(cbu_id)
        .bind(name)
        .bind(nature_purpose)
        .bind(source_of_funds) // Using description field for source_of_funds
        .execute(&self.pool)
        .await?;
        
        // Log CBU creation
        sqlx::query(
            r#"
            INSERT INTO "ob-poc".cbu_creation_log
            (cbu_id, nature_purpose, source_of_funds, generated_dsl)
            VALUES ($1, $2, $3, $4)
            "#
        )
        .bind(cbu_id)
        .bind(nature_purpose)
        .bind(source_of_funds)
        .bind(format!("{:?}", create))
        .execute(&self.pool)
        .await?;
        
        Ok(ExecutionResult {
            success: true,
            affected_rows: 1,
            returned_id: Some(cbu_id),
            data: Some(json!({
                "cbu_id": cbu_id,
                "name": name,
                "nature_purpose": nature_purpose,
                "source_of_funds": source_of_funds,
            })),
            error: None,
        })
    }
    
    /// Create entity-role connection
    async fn create_entity_role(&self, create: &DataCreate) -> Result<ExecutionResult> {
        let connection_id = Uuid::new_v4();
        
        let entity_id = create.values.get("entity_id")
            .and_then(|v| if let Value::String(s) = v { Uuid::parse_str(s).ok() } else { None })
            .ok_or(anyhow::anyhow!("Invalid entity_id"))?;
            
        let cbu_id = create.values.get("cbu_id")
            .and_then(|v| if let Value::String(s) = v { Uuid::parse_str(s).ok() } else { None })
            .ok_or(anyhow::anyhow!("Invalid cbu_id"))?;
            
        let role_id = create.values.get("role_id")
            .and_then(|v| if let Value::String(s) = v { Uuid::parse_str(s).ok() } else { None })
            .ok_or(anyhow::anyhow!("Invalid role_id"))?;
        
        sqlx::query(
            r#"
            INSERT INTO "ob-poc".cbu_entity_roles
            (cbu_entity_role_id, cbu_id, entity_id, role_id)
            VALUES ($1, $2, $3, $4)
            "#
        )
        .bind(connection_id)
        .bind(cbu_id)
        .bind(entity_id)
        .bind(role_id)
        .execute(&self.pool)
        .await?;
        
        Ok(ExecutionResult {
            success: true,
            affected_rows: 1,
            returned_id: Some(connection_id),
            data: Some(json!({
                "connection_id": connection_id,
                "entity_id": entity_id,
                "cbu_id": cbu_id,
                "role_id": role_id,
            })),
            error: None,
        })
    }
    
    /// Execute READ operation
    async fn execute_read(&self, read: &DataRead) -> Result<ExecutionResult> {
        // Implement based on asset type
        match read.asset.as_str() {
            "cbus" => {
                let cbu_id = read.where_clause.get("cbu_id")
                    .and_then(|v| if let Value::String(s) = v { Uuid::parse_str(s).ok() } else { None })
                    .ok_or(anyhow::anyhow!("Invalid cbu_id"))?;
                
                let row = sqlx::query!(
                    r#"
                    SELECT cbu_id, name, nature_purpose, description, created_at
                    FROM "ob-poc".cbus
                    WHERE cbu_id = $1
                    "#,
                    cbu_id
                )
                .fetch_optional(&self.pool)
                .await?;
                
                Ok(ExecutionResult {
                    success: true,
                    affected_rows: if row.is_some() { 1 } else { 0 },
                    returned_id: None,
                    data: row.map(|r| json!({
                        "cbu_id": r.cbu_id,
                        "name": r.name,
                        "nature_purpose": r.nature_purpose,
                        "description": r.description,
                        "created_at": r.created_at,
                    })),
                    error: None,
                })
            }
            _ => Err(anyhow::anyhow!("Unknown asset type for READ: {}", read.asset)),
        }
    }
    
    /// Execute UPDATE operation
    async fn execute_update(&self, update: &DataUpdate) -> Result<ExecutionResult> {
        // Implement UPDATE logic based on asset
        // Similar pattern to CREATE and READ
        todo!("Implement UPDATE execution")
    }
    
    /// Execute DELETE operation
    async fn execute_delete(&self, delete: &DataDelete) -> Result<ExecutionResult> {
        // Implement DELETE logic based on asset
        todo!("Implement DELETE execution")
    }
    
    /// Log execution results
    async fn log_execution(
        &self,
        statement: &CrudStatement,
        result: &Result<ExecutionResult>,
        duration_ms: i32,
        context: &ExecutionContext,
    ) -> Result<()> {
        let success = result.is_ok();
        let error_msg = result.as_ref().err().map(|e| e.to_string());
        let rows_affected = result.as_ref().ok().map(|r| r.affected_rows as i32).unwrap_or(0);
        
        sqlx::query(
            r#"
            UPDATE "ob-poc".crud_operations
            SET execution_status = $1,
                execution_time_ms = $2,
                error_message = $3,
                rows_affected = $4,
                completed_at = NOW()
            WHERE operation_id = $5
            "#
        )
        .bind(if success { "SUCCESS" } else { "FAILED" })
        .bind(duration_ms)
        .bind(error_msg)
        .bind(rows_affected)
        .bind(context.session_id)
        .execute(&self.pool)
        .await?;
        
        Ok(())
    }
}

/// Result of CRUD execution
#[derive(Debug)]
pub struct ExecutionResult {
    pub success: bool,
    pub affected_rows: usize,
    pub returned_id: Option<Uuid>,
    pub data: Option<serde_json::Value>,
    pub error: Option<String>,
}

// Import Value enum from parser_ast
use crate::parser_ast::Value;
```

#### File: `src/agentic/llm_client.rs`
```rust
use async_trait::async_trait;
use anyhow::Result;
use serde::{Deserialize, Serialize};

/// Trait for LLM clients
#[async_trait]
pub trait LlmClient: Send + Sync {
    async fn generate_dsl(&self, prompt: &str) -> Result<String>;
}

/// OpenAI client implementation
pub struct OpenAiClient {
    api_key: String,
    model: String,
}

impl OpenAiClient {
    pub fn new(api_key: String) -> Self {
        Self {
            api_key,
            model: "gpt-4".to_string(),
        }
    }
}

#[async_trait]
impl LlmClient for OpenAiClient {
    async fn generate_dsl(&self, prompt: &str) -> Result<String> {
        let client = reqwest::Client::new();
        
        let request = OpenAiRequest {
            model: self.model.clone(),
            messages: vec![
                Message {
                    role: "system".to_string(),
                    content: "You are a DSL generator. Return only DSL code, no explanation.".to_string(),
                },
                Message {
                    role: "user".to_string(),
                    content: prompt.to_string(),
                },
            ],
            temperature: 0.2, // Low temperature for deterministic output
            max_tokens: 500,
        };
        
        let response = client
            .post("https://api.openai.com/v1/chat/completions")
            .header("Authorization", format!("Bearer {}", self.api_key))
            .json(&request)
            .send()
            .await?
            .json::<OpenAiResponse>()
            .await?;
        
        Ok(response.choices[0].message.content.clone())
    }
}

/// Anthropic Claude client
pub struct AnthropicClient {
    api_key: String,
    model: String,
}

impl AnthropicClient {
    pub fn new(api_key: String) -> Self {
        Self {
            api_key,
            model: "claude-3-opus-20240229".to_string(),
        }
    }
}

#[async_trait]
impl LlmClient for AnthropicClient {
    async fn generate_dsl(&self, prompt: &str) -> Result<String> {
        let client = reqwest::Client::new();
        
        let request = AnthropicRequest {
            model: self.model.clone(),
            max_tokens: 500,
            messages: vec![
                AnthropicMessage {
                    role: "user".to_string(),
                    content: prompt.to_string(),
                },
            ],
        };
        
        let response = client
            .post("https://api.anthropic.com/v1/messages")
            .header("x-api-key", &self.api_key)
            .header("anthropic-version", "2023-06-01")
            .json(&request)
            .send()
            .await?
            .json::<AnthropicResponse>()
            .await?;
        
        Ok(response.content[0].text.clone())
    }
}

// Request/Response structures for OpenAI
#[derive(Serialize)]
struct OpenAiRequest {
    model: String,
    messages: Vec<Message>,
    temperature: f32,
    max_tokens: i32,
}

#[derive(Serialize, Deserialize)]
struct Message {
    role: String,
    content: String,
}

#[derive(Deserialize)]
struct OpenAiResponse {
    choices: Vec<Choice>,
}

#[derive(Deserialize)]
struct Choice {
    message: Message,
}

// Request/Response structures for Anthropic
#[derive(Serialize)]
struct AnthropicRequest {
    model: String,
    max_tokens: i32,
    messages: Vec<AnthropicMessage>,
}

#[derive(Serialize)]
struct AnthropicMessage {
    role: String,
    content: String,
}

#[derive(Deserialize)]
struct AnthropicResponse {
    content: Vec<AnthropicContent>,
}

#[derive(Deserialize)]
struct AnthropicContent {
    text: String,
}
```

#### File: `src/agentic/cbu_operations.rs`
```rust
use anyhow::Result;
use sqlx::PgPool;
use uuid::Uuid;
use serde::{Deserialize, Serialize};

/// High-level CBU operations service
pub struct CbuService {
    pool: PgPool,
    dsl_generator: super::AiDslGenerator,
    crud_executor: super::CrudExecutor,
}

impl CbuService {
    pub fn new(
        pool: PgPool,
        dsl_generator: super::AiDslGenerator,
        crud_executor: super::CrudExecutor,
    ) -> Self {
        Self { pool, dsl_generator, crud_executor }
    }
    
    /// Create CBU from natural language
    pub async fn create_from_instruction(&self, instruction: &str) -> Result<CbuCreationResult> {
        // Generate DSL from instruction
        let generated = self.dsl_generator
            .generate_from_instruction(instruction, None)
            .await?;
        
        // Execute the generated DSL
        let context = super::dsl_generator::ExecutionContext {
            cbu_id: None,
            user_id: None,
            session_id: generated.id,
            metadata: Default::default(),
        };
        
        let result = self.crud_executor
            .execute(&generated.ast, &context)
            .await?;
        
        Ok(CbuCreationResult {
            cbu_id: result.returned_id.unwrap(),
            instruction,
            generated_dsl: generated.dsl_text,
            success: result.success,
        })
    }
}

#[derive(Debug, Serialize, Deserialize)]
pub struct CbuCreationResult {
    pub cbu_id: Uuid,
    pub instruction: String,
    pub generated_dsl: String,
    pub success: bool,
}

/// Service for managing entity-role connections
pub struct EntityRoleService {
    pool: PgPool,
    dsl_generator: super::AiDslGenerator,
    crud_executor: super::CrudExecutor,
}

impl EntityRoleService {
    pub fn new(
        pool: PgPool,
        dsl_generator: super::AiDslGenerator,
        crud_executor: super::CrudExecutor,
    ) -> Self {
        Self { pool, dsl_generator, crud_executor }
    }
    
    /// Connect entity to CBU with role from natural language
    pub async fn connect_from_instruction(
        &self,
        instruction: &str,
        cbu_id: Uuid,
    ) -> Result<ConnectionResult> {
        let context = super::dsl_generator::ExecutionContext {
            cbu_id: Some(cbu_id),
            user_id: None,
            session_id: Uuid::new_v4(),
            metadata: Default::default(),
        };
        
        // Generate DSL
        let generated = self.dsl_generator
            .generate_from_instruction(instruction, Some(&context))
            .await?;
        
        // Execute
        let result = self.crud_executor
            .execute(&generated.ast, &context)
            .await?;
        
        Ok(ConnectionResult {
            connection_id: result.returned_id.unwrap(),
            cbu_id,
            instruction,
            generated_dsl: generated.dsl_text,
            success: result.success,
        })
    }
}

#[derive(Debug, Serialize, Deserialize)]
pub struct ConnectionResult {
    pub connection_id: Uuid,
    pub cbu_id: Uuid,
    pub instruction: String,
    pub generated_dsl: String,
    pub success: bool,
}
```

### Step 3: Integration Test & Demo

#### File: `examples/agentic_cbu_demo.rs`
```rust
//! Agentic CBU Creation Demo
//! Run with: cargo run --example agentic_cbu_demo

use ob_poc::agentic::{AiDslGenerator, CrudExecutor, CbuService, OpenAiClient};
use sqlx::PgPool;
use std::env;

#[tokio::main]
async fn main() -> Result<(), Box<dyn std::error::Error>> {
    // Setup
    println!("🤖 Agentic DSL CRUD Demo\n");
    println!("=" .repeat(60));
    
    // Database connection
    let database_url = env::var("DATABASE_URL")?;
    let pool = PgPool::connect(&database_url).await?;
    
    // LLM client (use OpenAI or Anthropic)
    let api_key = env::var("OPENAI_API_KEY")?;
    let llm_client = Box::new(OpenAiClient::new(api_key));
    
    // Initialize services
    let dsl_generator = AiDslGenerator::new(pool.clone(), llm_client);
    let crud_executor = CrudExecutor::new(pool.clone());
    let cbu_service = CbuService::new(pool.clone(), dsl_generator, crud_executor);
    
    // DEMO 1: Create CBU from natural language
    println!("\n📝 Test 1: Create CBU from natural language");
    println!("-" .repeat(40));
    
    let instruction = r#"Create a CBU with Nature and Purpose "Banking Services for High Net Worth Individuals" 
                        and Source of funds "Investment Returns and Business Operations""#;
    
    println!("Instruction: {}", instruction);
    println!("Generating DSL...");
    
    let result = cbu_service.create_from_instruction(instruction).await?;
    
    println!("✅ Generated DSL: {}", result.generated_dsl);
    println!("✅ Created CBU: {}", result.cbu_id);
    
    // DEMO 2: Connect entity to CBU
    println!("\n📝 Test 2: Connect entity to CBU");
    println!("-" .repeat(40));
    
    // First create a test entity
    let entity_id = create_test_entity(&pool).await?;
    let role_id = get_or_create_role(&pool, "Director").await?;
    
    let instruction2 = format!(
        "Connect entity {} to CBU {} as role {}",
        entity_id, result.cbu_id, role_id
    );
    
    println!("Instruction: {}", instruction2);
    
    let entity_service = EntityRoleService::new(
        pool.clone(),
        dsl_generator.clone(),
        crud_executor.clone()
    );
    
    let connection_result = entity_service
        .connect_from_instruction(&instruction2, result.cbu_id)
        .await?;
    
    println!("✅ Generated DSL: {}", connection_result.generated_dsl);
    println!("✅ Created connection: {}", connection_result.connection_id);
    
    // DEMO 3: Query the created CBU
    println!("\n📝 Test 3: Query created CBU");
    println!("-" .repeat(40));
    
    let query_instruction = format!("Read CBU where cbu_id = {}", result.cbu_id);
    let query_dsl = dsl_generator
        .generate_from_instruction(&query_instruction, None)
        .await?;
    
    println!("Query DSL: {}", query_dsl.dsl_text);
    
    let query_result = crud_executor.execute(&query_dsl.ast, &context).await?;
    println!("✅ CBU Data: {}", serde_json::to_string_pretty(&query_result.data)?);
    
    println!("\n🎉 Demo completed successfully!");
    
    Ok(())
}

async fn create_test_entity(pool: &PgPool) -> Result<Uuid, Box<dyn std::error::Error>> {
    let entity_id = Uuid::new_v4();
    sqlx::query(
        r#"INSERT INTO "ob-poc".entities (entity_id, name, entity_type) 
           VALUES ($1, $2, $3)"#
    )
    .bind(entity_id)
    .bind("John Doe")
    .bind("PERSON")
    .execute(pool)
    .await?;
    Ok(entity_id)
}

async fn get_or_create_role(pool: &PgPool, role_name: &str) -> Result<Uuid, Box<dyn std::error::Error>> {
    // Check if role exists or create new one
    // Simplified for demo
    Ok(Uuid::new_v4())
}
```

### Step 4: REST API Endpoints

#### File: `src/api/agentic_handlers.rs`
```rust
use axum::{
    extract::{Path, State},
    response::Json,
    http::StatusCode,
};
use serde::{Deserialize, Serialize};
use uuid::Uuid;

use crate::agentic::CbuService;

#[derive(Deserialize)]
pub struct CreateCbuRequest {
    pub instruction: String,
}

#[derive(Serialize)]
pub struct CreateCbuResponse {
    pub cbu_id: Uuid,
    pub generated_dsl: String,
    pub success: bool,
}

/// POST /api/agentic/cbu/create
pub async fn create_cbu_agentic(
    State(cbu_service): State<Arc<CbuService>>,
    Json(request): Json<CreateCbuRequest>,
) -> Result<Json<CreateCbuResponse>, (StatusCode, String)> {
    let result = cbu_service
        .create_from_instruction(&request.instruction)
        .await
        .map_err(|e| (StatusCode::INTERNAL_SERVER_ERROR, e.to_string()))?;
    
    Ok(Json(CreateCbuResponse {
        cbu_id: result.cbu_id,
        generated_dsl: result.generated_dsl,
        success: result.success,
    }))
}

#[derive(Deserialize)]
pub struct ConnectEntityRequest {
    pub instruction: String,
    pub cbu_id: Uuid,
}

/// POST /api/agentic/entity/connect
pub async fn connect_entity_agentic(
    State(entity_service): State<Arc<EntityRoleService>>,
    Json(request): Json<ConnectEntityRequest>,
) -> Result<Json<ConnectionResponse>, (StatusCode, String)> {
    let result = entity_service
        .connect_from_instruction(&request.instruction, request.cbu_id)
        .await
        .map_err(|e| (StatusCode::INTERNAL_SERVER_ERROR, e.to_string()))?;
    
    Ok(Json(ConnectionResponse {
        connection_id: result.connection_id,
        success: result.success,
    }))
}
```

## Testing Instructions

### 1. Run Database Migrations
```bash
psql -d ob-poc -f step1_migrations.sql
```

### 2. Build and Test
```bash
cargo build --lib
cargo test agentic
cargo run --example agentic_cbu_demo
```

### 3. Test REST API
```bash
# Create CBU
curl -X POST http://localhost:3000/api/agentic/cbu/create \
  -H "Content-Type: application/json" \
  -d '{
    "instruction": "Create a CBU with Nature and Purpose \"Investment Banking Services\" and Source of funds \"Private Equity Returns\""
  }'

# Connect Entity
curl -X POST http://localhost:3000/api/agentic/entity/connect \
  -H "Content-Type: application/json" \
  -d '{
    "instruction": "Connect entity 123e4567-e89b-12d3-a456-426614174000 as Director",
    "cbu_id": "generated-cbu-id-here"
  }'
```

## Environment Variables
```env
DATABASE_URL=postgresql://user:pass@localhost:5432/ob-poc
OPENAI_API_KEY=sk-...
# OR
ANTHROPIC_API_KEY=sk-ant-...
```

## Cargo.toml Dependencies
```toml
[dependencies]
# Async runtime
tokio = { version = "1", features = ["full"] }
async-trait = "0.1"

# Database
sqlx = { version = "0.7", features = ["postgres", "uuid", "chrono", "json"] }

# Web framework
axum = "0.6"

# Serialization
serde = { version = "1", features = ["derive"] }
serde_json = "1"

# HTTP client for LLM APIs
reqwest = { version = "0.11", features = ["json"] }

# Error handling
anyhow = "1"
thiserror = "1"

# UUID
uuid = { version = "1", features = ["v4", "serde"] }

# Date/time
chrono = { version = "0.4", features = ["serde"] }

# Parsing (if using nom)
nom = "7"
```

## Implementation Notes

1. **LLM Integration**: Replace the mock LLM with real OpenAI/Anthropic clients
2. **DSL Parser**: Extend the parser for more complex operations
3. **Error Handling**: Add comprehensive error handling and validation
4. **Caching**: Consider caching generated DSL for similar instructions
5. **Security**: Add authentication and authorization checks
6. **Monitoring**: Add metrics for DSL generation success rates

## Next Steps

After implementing this base system:
1. Add more complex DSL operations (JOINs, aggregations)
2. Implement workflow orchestration based on DSL
3. Add RAG for better context understanding
4. Create UI for visual DSL generation
5. Add versioning for generated DSL

**Tell Zed Claude**: "Implement this complete agentic DSL CRUD system following the steps in order. Start with the database migrations, then implement each Rust file, and finally test with the demo."
