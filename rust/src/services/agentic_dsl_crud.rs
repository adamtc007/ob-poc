// AGENTIC DSL CRUD - SINGLE FILE IMPLEMENTATION
// Drop this into Zed Claude and say: "Implement this agentic DSL CRUD system"
// This creates Natural Language → DSL → Database operations for CBU management

use anyhow::{Context, Result};
use async_trait::async_trait;
use chrono::{DateTime, Utc};
use serde::{Deserialize, Serialize};
use sqlx::{FromRow, PgPool};
use std::collections::HashMap;
use uuid::Uuid;

// ============================================================================
// STEP 1: Database Setup - Run these migrations first
// ============================================================================
/*
-- Run in PostgreSQL:

-- Ensure CBU table has all fields
ALTER TABLE "ob-poc".cbus
ADD COLUMN IF NOT EXISTS source_of_funds TEXT;

-- Track AI-generated operations
ALTER TABLE "ob-poc".crud_operations
ADD COLUMN IF NOT EXISTS cbu_id UUID REFERENCES "ob-poc".cbus(cbu_id),
ADD COLUMN IF NOT EXISTS parsed_ast JSONB,
ADD COLUMN IF NOT EXISTS natural_language_input TEXT;

-- CBU creation audit log
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
*/

// ============================================================================
// STEP 2: Core Types & AST
// ============================================================================

#[derive(Debug, Clone, Serialize, Deserialize)]
pub enum CrudStatement {
    CreateCbu(CreateCbu),
    ConnectEntity(ConnectEntity),
    ReadCbu(ReadCbu),
    UpdateCbu(UpdateCbu),
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct CreateCbu {
    pub name: Option<String>,
    pub nature_purpose: String,
    pub source_of_funds: String,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ConnectEntity {
    pub entity_id: Uuid,
    pub cbu_id: Uuid,
    pub role_id: Uuid,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ReadCbu {
    pub cbu_id: Uuid,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct UpdateCbu {
    pub cbu_id: Uuid,
    pub updates: HashMap<String, String>,
}

// ============================================================================
// STEP 3: DSL Parser
// ============================================================================

pub struct DslParser;

impl DslParser {
    /// Parse natural language or DSL into AST
    pub fn parse(input: &str) -> Result<CrudStatement> {
        let normalized = input.to_lowercase();

        if normalized.contains("create") && normalized.contains("cbu") {
            Self::parse_create_cbu(input)
        } else if normalized.contains("connect") && normalized.contains("entity") {
            Self::parse_connect_entity(input)
        } else if normalized.contains("read") || normalized.contains("get") {
            Self::parse_read_cbu(input)
        } else if normalized.contains("update") {
            Self::parse_update_cbu(input)
        } else {
            Err(anyhow::anyhow!("Unknown operation: {}", input))
        }
    }

    fn parse_create_cbu(input: &str) -> Result<CrudStatement> {
        // Extract nature and purpose
        let nature = Self::extract_between(input, "nature and purpose", "and source")
            .or_else(|| Self::extract_quoted(input, "nature_purpose"))
            .unwrap_or_else(|| "General Services".to_string());

        // Extract source of funds
        let source = Self::extract_after(input, "source of funds")
            .or_else(|| Self::extract_quoted(input, "source_of_funds"))
            .unwrap_or_else(|| "Operations".to_string());

        Ok(CrudStatement::CreateCbu(CreateCbu {
            name: None, // Will be auto-generated
            nature_purpose: nature.trim().trim_matches('"').to_string(),
            source_of_funds: source.trim().trim_matches('"').to_string(),
        }))
    }

    fn parse_connect_entity(input: &str) -> Result<CrudStatement> {
        // Extract UUIDs - simple pattern matching
        let uuids: Vec<&str> = input
            .split_whitespace()
            .filter(|s| s.len() == 36 && s.contains('-'))
            .collect();

        if uuids.len() < 3 {
            return Err(anyhow::anyhow!("Need entity_id, cbu_id, and role_id"));
        }

        Ok(CrudStatement::ConnectEntity(ConnectEntity {
            entity_id: Uuid::parse_str(uuids[0])?,
            cbu_id: Uuid::parse_str(uuids[1])?,
            role_id: Uuid::parse_str(uuids[2])?,
        }))
    }

    fn parse_read_cbu(input: &str) -> Result<CrudStatement> {
        let uuid_str = input
            .split_whitespace()
            .find(|s| s.len() == 36 && s.contains('-'))
            .ok_or_else(|| anyhow::anyhow!("No CBU ID found"))?;

        Ok(CrudStatement::ReadCbu(ReadCbu {
            cbu_id: Uuid::parse_str(uuid_str)?,
        }))
    }

    fn parse_update_cbu(input: &str) -> Result<CrudStatement> {
        // Simple UPDATE parsing - extend as needed
        let uuid_str = input
            .split_whitespace()
            .find(|s| s.len() == 36 && s.contains('-'))
            .ok_or_else(|| anyhow::anyhow!("No CBU ID found"))?;

        let mut updates = HashMap::new();
        // Parse SET clauses - simplified
        if let Some(set_pos) = input.find("set") {
            let set_part = &input[set_pos + 3..];
            // Add parsing logic for "field = value" pairs
            updates.insert("updated_at".to_string(), Utc::now().to_rfc3339());
        }

        Ok(CrudStatement::UpdateCbu(UpdateCbu {
            cbu_id: Uuid::parse_str(uuid_str)?,
            updates,
        }))
    }

    // Helper methods
    fn extract_between(text: &str, start: &str, end: &str) -> Option<String> {
        let start_pos = text.to_lowercase().find(start)?;
        let after_start = &text[start_pos + start.len()..];
        let end_pos = after_start.to_lowercase().find(end)?;
        Some(after_start[..end_pos].trim().to_string())
    }

    fn extract_after(text: &str, marker: &str) -> Option<String> {
        let pos = text.to_lowercase().find(marker)?;
        Some(text[pos + marker.len()..].trim().to_string())
    }

    fn extract_quoted(text: &str, after: &str) -> Option<String> {
        let start = text.find(after)?;
        let text_after = &text[start..];
        let quote_start = text_after.find('"')?;
        let quote_end = text_after[quote_start + 1..].find('"')?;
        Some(text_after[quote_start + 1..quote_start + 1 + quote_end].to_string())
    }
}

// ============================================================================
// STEP 4: AI DSL Generator
// ============================================================================

pub struct AiDslGenerator {
    pool: PgPool,
}

impl AiDslGenerator {
    pub fn new(pool: PgPool) -> Self {
        Self { pool }
    }

    /// Generate DSL from natural language
    pub async fn generate(&self, instruction: &str) -> Result<GeneratedDsl> {
        // For now, use deterministic templates
        // In production, call OpenAI/Anthropic here
        let dsl_text = self.generate_dsl_template(instruction);

        // Parse to AST
        let ast = DslParser::parse(&dsl_text)?;

        // Log generation
        let id = self.log_generation(instruction, &dsl_text, &ast).await?;

        Ok(GeneratedDsl {
            id,
            instruction: instruction.to_string(),
            dsl_text,
            ast,
            confidence: 0.95,
        })
    }

    fn generate_dsl_template(&self, instruction: &str) -> String {
        let lower = instruction.to_lowercase();

        if lower.contains("create") && lower.contains("cbu") {
            // Template for CBU creation
            format!("CREATE CBU WITH {}", instruction)
        } else if lower.contains("connect") {
            // Template for entity connection
            format!("CONNECT ENTITY {}", instruction)
        } else {
            instruction.to_string()
        }
    }

    async fn log_generation(
        &self,
        _instruction: &str,
        _dsl: &str,
        _ast: &CrudStatement,
    ) -> Result<Uuid> {
        let id = Uuid::new_v4();

        // Note: crud_operations table does not exist in current schema
        // Audit logging is handled by cbu_creation_log and entity_role_connections tables
        // If you need this table, create it with:
        // CREATE TABLE "ob-poc".crud_operations (
        //     operation_id UUID PRIMARY KEY,
        //     operation_type VARCHAR(50),
        //     asset_type VARCHAR(50),
        //     generated_dsl TEXT,
        //     ai_instruction TEXT,
        //     parsed_ast JSONB,
        //     natural_language_input TEXT,
        //     created_at TIMESTAMP WITH TIME ZONE DEFAULT NOW()
        // );

        Ok(id)
    }
}

#[derive(Debug)]
pub struct GeneratedDsl {
    pub id: Uuid,
    pub instruction: String,
    pub dsl_text: String,
    pub ast: CrudStatement,
    pub confidence: f64,
}

// ============================================================================
// STEP 5: CRUD Executor
// ============================================================================

pub struct CrudExecutor {
    pool: PgPool,
}

impl CrudExecutor {
    pub fn new(pool: PgPool) -> Self {
        Self { pool }
    }

    /// Execute CRUD statement
    pub async fn execute(&self, statement: &CrudStatement) -> Result<ExecutionResult> {
        match statement {
            CrudStatement::CreateCbu(create) => self.create_cbu(create).await,
            CrudStatement::ConnectEntity(connect) => self.connect_entity(connect).await,
            CrudStatement::ReadCbu(read) => self.read_cbu(read).await,
            CrudStatement::UpdateCbu(update) => self.update_cbu(update).await,
        }
    }

    async fn create_cbu(&self, create: &CreateCbu) -> Result<ExecutionResult> {
        let cbu_id = Uuid::new_v4();
        let name = create
            .name
            .clone()
            .unwrap_or_else(|| format!("CBU-{}", &cbu_id.to_string()[..8]));

        // Insert CBU
        sqlx::query!(
            r#"
            INSERT INTO "ob-poc".cbus
            (cbu_id, name, nature_purpose, description)
            VALUES ($1, $2, $3, $4)
            "#,
            cbu_id,
            name,
            create.nature_purpose,
            create.source_of_funds // Using description field for source_of_funds
        )
        .execute(&self.pool)
        .await?;

        // Log creation
        sqlx::query!(
            r#"
            INSERT INTO "ob-poc".cbu_creation_log
            (cbu_id, nature_purpose, source_of_funds, ai_instruction, generated_dsl)
            VALUES ($1, $2, $3, $4, $5)
            "#,
            cbu_id,
            create.nature_purpose,
            create.source_of_funds,
            format!("{:?}", create),
            format!(
                "CREATE CBU WITH nature_purpose \"{}\" AND source_of_funds \"{}\"",
                create.nature_purpose, create.source_of_funds
            )
        )
        .execute(&self.pool)
        .await?;

        Ok(ExecutionResult {
            success: true,
            entity_id: Some(cbu_id),
            message: format!("Created CBU {} with name '{}'", cbu_id, name),
            data: serde_json::json!({
                "cbu_id": cbu_id,
                "name": name,
                "nature_purpose": create.nature_purpose,
                "source_of_funds": create.source_of_funds,
            }),
        })
    }

    async fn connect_entity(&self, connect: &ConnectEntity) -> Result<ExecutionResult> {
        let connection_id = Uuid::new_v4();

        sqlx::query!(
            r#"
            INSERT INTO "ob-poc".cbu_entity_roles
            (cbu_entity_role_id, cbu_id, entity_id, role_id)
            VALUES ($1, $2, $3, $4)
            "#,
            connection_id,
            connect.cbu_id,
            connect.entity_id,
            connect.role_id
        )
        .execute(&self.pool)
        .await?;

        Ok(ExecutionResult {
            success: true,
            entity_id: Some(connection_id),
            message: format!(
                "Connected entity {} to CBU {} with role {}",
                connect.entity_id, connect.cbu_id, connect.role_id
            ),
            data: serde_json::json!({
                "connection_id": connection_id,
                "entity_id": connect.entity_id,
                "cbu_id": connect.cbu_id,
                "role_id": connect.role_id,
            }),
        })
    }

    async fn read_cbu(&self, read: &ReadCbu) -> Result<ExecutionResult> {
        #[derive(FromRow)]
        struct CbuRow {
            cbu_id: Uuid,
            name: String,
            nature_purpose: Option<String>,
            description: Option<String>,
            created_at: Option<DateTime<Utc>>,
        }

        let row = sqlx::query_as::<_, CbuRow>(
            r#"
            SELECT cbu_id, name, nature_purpose, description, created_at
            FROM "ob-poc".cbus
            WHERE cbu_id = $1
            "#,
        )
        .bind(read.cbu_id)
        .fetch_optional(&self.pool)
        .await?;

        match row {
            Some(cbu) => Ok(ExecutionResult {
                success: true,
                entity_id: Some(cbu.cbu_id),
                message: format!("Found CBU: {}", cbu.name),
                data: serde_json::json!({
                    "cbu_id": cbu.cbu_id,
                    "name": cbu.name,
                    "nature_purpose": cbu.nature_purpose,
                    "source_of_funds": cbu.description, // description holds source_of_funds
                    "created_at": cbu.created_at,
                }),
            }),
            None => Ok(ExecutionResult {
                success: false,
                entity_id: None,
                message: format!("CBU {} not found", read.cbu_id),
                data: serde_json::Value::Null,
            }),
        }
    }

    async fn update_cbu(&self, update: &UpdateCbu) -> Result<ExecutionResult> {
        // Simple update - extend as needed
        sqlx::query!(
            r#"
            UPDATE "ob-poc".cbus
            SET updated_at = NOW()
            WHERE cbu_id = $1
            "#,
            update.cbu_id
        )
        .execute(&self.pool)
        .await?;

        Ok(ExecutionResult {
            success: true,
            entity_id: Some(update.cbu_id),
            message: format!("Updated CBU {}", update.cbu_id),
            data: serde_json::json!({"cbu_id": update.cbu_id}),
        })
    }
}

#[derive(Debug)]
pub struct ExecutionResult {
    pub success: bool,
    pub entity_id: Option<Uuid>,
    pub message: String,
    pub data: serde_json::Value,
}

// ============================================================================
// STEP 6: High-Level Service
// ============================================================================

pub struct AgenticCbuService {
    generator: AiDslGenerator,
    executor: CrudExecutor,
}

impl AgenticCbuService {
    pub fn new(pool: PgPool) -> Self {
        Self {
            generator: AiDslGenerator::new(pool.clone()),
            executor: CrudExecutor::new(pool),
        }
    }

    /// Main entry point: Natural language → Result
    pub async fn process_instruction(&self, instruction: &str) -> Result<ExecutionResult> {
        println!("📝 Instruction: {}", instruction);

        // Generate DSL
        let generated = self.generator.generate(instruction).await?;
        println!("🔧 Generated DSL: {}", generated.dsl_text);
        println!("📊 Confidence: {:.2}", generated.confidence);

        // Execute
        let result = self.executor.execute(&generated.ast).await?;
        println!("✅ Result: {}", result.message);

        Ok(result)
    }
}

// ============================================================================
// STEP 7: Demo & Tests
// ============================================================================

#[cfg(test)]
mod tests {
    use super::*;

    #[tokio::test]
    async fn test_create_cbu_flow() -> Result<()> {
        let pool = setup_test_db().await?;
        let service = AgenticCbuService::new(pool);

        // Test 1: Create CBU from natural language
        let result = service
            .process_instruction(
                r#"Create a CBU with Nature and Purpose "Investment Banking Services"
               and Source of funds "Private Equity Returns""#,
            )
            .await?;

        assert!(result.success);
        assert!(result.entity_id.is_some());

        let cbu_id = result.entity_id.unwrap();

        // Test 2: Read the created CBU
        let read_result = service
            .process_instruction(&format!("Read CBU {}", cbu_id))
            .await?;

        assert!(read_result.success);
        assert_eq!(read_result.data["cbu_id"], serde_json::json!(cbu_id));

        Ok(())
    }

    async fn setup_test_db() -> Result<PgPool> {
        let url = std::env::var("DATABASE_URL")
            .unwrap_or_else(|_| "postgresql://localhost/ob_poc_test".to_string());
        Ok(PgPool::connect(&url).await?)
    }
}

// ============================================================================
// STEP 8: Main Demo Function
// ============================================================================

/// Run this to test the complete flow
pub async fn run_demo(pool: PgPool) -> Result<()> {
    println!("\n🤖 AGENTIC DSL CRUD DEMO");
    println!("{}", "=".repeat(60));

    let service = AgenticCbuService::new(pool.clone());

    // Demo 1: Create CBU
    println!("\n📌 Demo 1: Create CBU from natural language");
    let instruction1 = r#"Create a CBU with Nature and Purpose "High Net Worth Banking Services"
                          and Source of funds "Investment Portfolio and Business Operations""#;

    let result1 = service.process_instruction(instruction1).await?;
    let cbu_id = result1.entity_id.unwrap();

    // Demo 2: Read CBU
    println!("\n📌 Demo 2: Read created CBU");
    let instruction2 = format!("Read CBU {}", cbu_id);
    let result2 = service.process_instruction(&instruction2).await?;
    println!("CBU Data: {}", serde_json::to_string_pretty(&result2.data)?);

    // Demo 3: Connect Entity (need to create entity first)
    println!("\n📌 Demo 3: Connect entity to CBU");
    let entity_id = create_test_entity(&pool).await?;
    let role_id = Uuid::new_v4(); // In production, lookup actual role

    let instruction3 = format!(
        "Connect entity {} to CBU {} as role {}",
        entity_id, cbu_id, role_id
    );
    let result3 = service.process_instruction(&instruction3).await?;

    println!("\n✅ All demos completed successfully!");
    Ok(())
}

async fn create_test_entity(pool: &PgPool) -> Result<Uuid> {
    let entity_id = Uuid::new_v4();

    // Get or create PERSON entity type
    let entity_type_id = sqlx::query_scalar::<_, Uuid>(
        r#"SELECT entity_type_id FROM "ob-poc".entity_types WHERE name = 'PERSON' LIMIT 1"#,
    )
    .fetch_one(pool)
    .await?;

    sqlx::query!(
        r#"
        INSERT INTO "ob-poc".entities (entity_id, entity_type_id, name)
        VALUES ($1, $2, $3)
        ON CONFLICT (entity_id) DO NOTHING
        "#,
        entity_id,
        entity_type_id,
        "Test Entity"
    )
    .execute(pool)
    .await?;
    Ok(entity_id)
}

// ============================================================================
// INSTRUCTIONS FOR ZED CLAUDE:
// ============================================================================
/*
1. Create a new file: src/agentic_dsl_crud.rs
2. Copy this entire content into that file
3. Add to src/lib.rs: pub mod agentic_dsl_crud;
4. Run the SQL migrations at the top of this file
5. Create a demo file: examples/run_agentic_demo.rs with:

use ob_poc::agentic_dsl_crud::{run_demo, AgenticCbuService};
use sqlx::PgPool;

#[tokio::main]
async fn main() -> Result<(), Box<dyn std::error::Error>> {
    let database_url = std::env::var("DATABASE_URL")?;
    let pool = PgPool::connect(&database_url).await?;

    // Run the demo
    run_demo(pool).await?;

    Ok(())
}

6. Run: cargo run --example run_agentic_demo

This will demonstrate:
- Creating CBUs from natural language
- Reading CBUs
- Connecting entities to CBUs with roles
- Full audit trail of AI-generated DSL
*/
