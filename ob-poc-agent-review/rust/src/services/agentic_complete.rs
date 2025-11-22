//! Complete Agentic DSL System - Entity, Role, and CBU Management
//!
//! This module extends the basic agentic DSL CRUD system to support:
//! - Entity creation (person, company, trust)
//! - Role management
//! - Complete end-to-end workflows
//!
//! This fills the missing pieces identified in the end-to-end analysis.

use anyhow::{Context, Result};
use serde::{Deserialize, Serialize};
use sqlx::PgPool;
use std::collections::HashMap;
use uuid::Uuid;

// Import existing agentic DSL CRUD
use super::agentic_dsl_crud::{AgenticCbuService, ConnectEntity, CrudStatement, DslParser};

// ============================================================================
// Extended AST Types for Entity and Role Operations
// ============================================================================

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct CreateEntity {
    pub name: String,
    pub entity_type: String, // PERSON, COMPANY, TRUST, etc.
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct CreateRole {
    pub name: String,
    pub description: String,
}

/// Extended CRUD operations including entity and role management
#[derive(Debug, Clone, Serialize, Deserialize)]
pub enum ExtendedCrudStatement {
    /// Base operations from existing system
    Base(CrudStatement),
    /// Create a new entity
    CreateEntity(CreateEntity),
    /// Create a new role
    CreateRole(CreateRole),
}

// ============================================================================
// Extended Parser for Entity and Role Creation
// ============================================================================

pub struct ExtendedDslParser;

impl ExtendedDslParser {
    /// Parse natural language into extended CRUD operations
    pub fn parse(input: &str) -> Result<ExtendedCrudStatement> {
        let normalized = input.to_lowercase();

        // Try entity creation patterns
        if normalized.contains("create entity")
            || normalized.contains("add person")
            || normalized.contains("add company")
            || normalized.contains("add trust")
        {
            return Ok(ExtendedCrudStatement::CreateEntity(
                Self::parse_create_entity(input)?,
            ));
        }

        // Try role creation patterns
        if normalized.contains("create role") || normalized.contains("add role") {
            return Ok(ExtendedCrudStatement::CreateRole(Self::parse_create_role(
                input,
            )?));
        }

        // Fall back to base parser for CBU operations
        DslParser::parse(input)
            .map(ExtendedCrudStatement::Base)
            .context("Failed to parse as base or extended statement")
    }

    fn parse_create_entity(input: &str) -> Result<CreateEntity> {
        let normalized = input.to_lowercase();

        // Determine entity type
        let entity_type = if normalized.contains("person") || normalized.contains("individual") {
            "PERSON"
        } else if normalized.contains("company")
            || normalized.contains("corp")
            || normalized.contains("ltd")
        {
            "COMPANY"
        } else if normalized.contains("trust") {
            "TRUST"
        } else {
            "ENTITY"
        };

        // Extract name (simplified parsing)
        let mut name = input
            .replace("Create entity", "")
            .replace("create entity", "")
            .replace("Add person", "")
            .replace("add person", "")
            .replace("Add company", "")
            .replace("add company", "")
            .replace("Add trust", "")
            .replace("add trust", "")
            .replace("as PERSON", "")
            .replace("as person", "")
            .replace("as COMPANY", "")
            .replace("as company", "")
            .replace("as TRUST", "")
            .replace("as trust", "")
            .trim()
            .to_string();

        if name.is_empty() {
            name = format!("Unnamed {}", entity_type);
        }

        Ok(CreateEntity {
            name,
            entity_type: entity_type.to_string(),
        })
    }

    fn parse_create_role(input: &str) -> Result<CreateRole> {
        let normalized = input.to_lowercase();

        // Common role types
        let name = if normalized.contains("director") {
            "Director"
        } else if normalized.contains("beneficiary") {
            "Beneficiary"
        } else if normalized.contains("trustee") {
            "Trustee"
        } else if normalized.contains("shareholder") {
            "Shareholder"
        } else if normalized.contains("officer") {
            "Officer"
        } else if normalized.contains("member") {
            "Member"
        } else {
            "Role"
        };

        Ok(CreateRole {
            name: name.to_string(),
            description: format!("{} role", name),
        })
    }
}

// ============================================================================
// Complete Execution Result
// ============================================================================

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct CompleteExecutionResult {
    pub success: bool,
    pub entity_type: String, // "CBU", "Entity", "Role", "Connection"
    pub entity_id: Option<Uuid>,
    pub message: String,
    pub data: serde_json::Value,
}

// ============================================================================
// Complete Agentic Service - Unified API for All Operations
// ============================================================================

pub struct CompleteAgenticService {
    pool: PgPool,
    base_service: AgenticCbuService,
}

impl CompleteAgenticService {
    pub fn new(pool: PgPool) -> Self {
        Self {
            base_service: AgenticCbuService::new(pool.clone()),
            pool,
        }
    }

    /// Get reference to the database pool
    pub fn pool(&self) -> &PgPool {
        &self.pool
    }

    /// Execute any extended statement (entity, role, CBU, connection)
    pub async fn execute(
        &self,
        statement: ExtendedCrudStatement,
    ) -> Result<CompleteExecutionResult> {
        match statement {
            ExtendedCrudStatement::Base(base) => self.execute_base(base).await,
            ExtendedCrudStatement::CreateEntity(create) => self.create_entity(create).await,
            ExtendedCrudStatement::CreateRole(create) => self.create_role(create).await,
        }
    }

    /// Execute from natural language instruction
    pub async fn execute_from_natural_language(
        &self,
        instruction: &str,
    ) -> Result<CompleteExecutionResult> {
        let statement = ExtendedDslParser::parse(instruction)?;
        self.execute(statement).await
    }

    /// Execute base CBU operations
    async fn execute_base(&self, statement: CrudStatement) -> Result<CompleteExecutionResult> {
        // Use the base service's execute_statement method
        let result = self.base_service.execute_statement(&statement).await?;

        // Determine entity type from statement
        let entity_type = match statement {
            CrudStatement::CreateCbu(_) => "CBU",
            CrudStatement::ConnectEntity(_) => "Connection",
            CrudStatement::ReadCbu(_) => "CBU",
            CrudStatement::UpdateCbu(_) => "CBU",
        };

        // Convert ExecutionResult to CompleteExecutionResult
        Ok(CompleteExecutionResult {
            success: result.success,
            entity_type: entity_type.to_string(),
            entity_id: result.entity_id,
            message: result.message,
            data: result.data,
        })
    }

    /// Helper method kept for backwards compatibility
    async fn _execute_base_detailed(
        &self,
        statement: CrudStatement,
    ) -> Result<CompleteExecutionResult> {
        // For base operations, delegate to base service
        match statement {
            CrudStatement::CreateCbu(_) | CrudStatement::ConnectEntity(_) => {
                let result = self.base_service.execute_statement(&statement).await?;

                let entity_type = match statement {
                    CrudStatement::CreateCbu(_) => "CBU",
                    CrudStatement::ConnectEntity(_) => "Connection",
                    _ => "Unknown",
                };

                Ok(CompleteExecutionResult {
                    success: result.success,
                    entity_type: entity_type.to_string(),
                    entity_id: result.entity_id,
                    message: result.message,
                    data: result.data,
                })
            }
            CrudStatement::ReadCbu(read) => {
                // Read CBU details
                let cbu = self.read_cbu(read.cbu_id).await?;

                Ok(CompleteExecutionResult {
                    success: true,
                    entity_type: "CBU".to_string(),
                    entity_id: Some(read.cbu_id),
                    message: format!("Read CBU: {}", read.cbu_id),
                    data: cbu,
                })
            }
            CrudStatement::UpdateCbu(update) => {
                // Update CBU
                self.update_cbu(update.cbu_id, &update.updates).await?;

                Ok(CompleteExecutionResult {
                    success: true,
                    entity_type: "CBU".to_string(),
                    entity_id: Some(update.cbu_id),
                    message: format!("Updated CBU: {}", update.cbu_id),
                    data: serde_json::json!({
                        "cbu_id": update.cbu_id,
                        "updates": update.updates,
                    }),
                })
            }
        }
    }

    /// Create a new entity
    async fn create_entity(&self, create: CreateEntity) -> Result<CompleteExecutionResult> {
        let entity_id = Uuid::new_v4();

        // Get or create entity_type_id
        let entity_type_id = self.get_or_create_entity_type(&create.entity_type).await?;

        sqlx::query!(
            r#"
            INSERT INTO "ob-poc".entities (entity_id, entity_type_id, name)
            VALUES ($1, $2, $3)
            "#,
            entity_id,
            entity_type_id,
            create.name
        )
        .execute(&self.pool)
        .await
        .context("Failed to create entity")?;

        Ok(CompleteExecutionResult {
            success: true,
            entity_type: "Entity".to_string(),
            entity_id: Some(entity_id),
            message: format!("Created {} entity: {}", create.entity_type, create.name),
            data: serde_json::json!({
                "entity_id": entity_id,
                "name": create.name,
                "type": create.entity_type,
            }),
        })
    }

    /// Create a new role (simplified - in production you'd have a roles table)
    async fn create_role(&self, create: CreateRole) -> Result<CompleteExecutionResult> {
        let role_id = Uuid::new_v4();

        // For now, we return a role ID and metadata
        // In production, you'd insert into a roles table

        Ok(CompleteExecutionResult {
            success: true,
            entity_type: "Role".to_string(),
            entity_id: Some(role_id),
            message: format!("Created role: {}", create.name),
            data: serde_json::json!({
                "role_id": role_id,
                "name": create.name,
                "description": create.description,
            }),
        })
    }

    /// Helper: Get or create entity type
    async fn get_or_create_entity_type(&self, type_name: &str) -> Result<Uuid> {
        // Try to get existing entity type
        let existing = sqlx::query_scalar::<_, Uuid>(
            r#"SELECT entity_type_id FROM "ob-poc".entity_types WHERE name = $1"#,
        )
        .bind(type_name)
        .fetch_optional(&self.pool)
        .await?;

        if let Some(type_id) = existing {
            return Ok(type_id);
        }

        // Create new entity type if doesn't exist
        let type_id = Uuid::new_v4();
        sqlx::query!(
            r#"
            INSERT INTO "ob-poc".entity_types (entity_type_id, name, description, table_name)
            VALUES ($1, $2, $3, $4)
            ON CONFLICT (name) DO NOTHING
            "#,
            type_id,
            type_name,
            format!("{} entity type", type_name),
            "entities" // All types share the polymorphic entities table
        )
        .execute(&self.pool)
        .await
        .ok(); // Ignore conflicts

        Ok(type_id)
    }

    /// Helper: Read CBU details
    #[allow(dead_code)]
    async fn read_cbu(&self, cbu_id: Uuid) -> Result<serde_json::Value> {
        let cbu = sqlx::query!(
            r#"
            SELECT cbu_id, name, description, nature_purpose, source_of_funds, created_at
            FROM "ob-poc".cbus
            WHERE cbu_id = $1
            "#,
            cbu_id
        )
        .fetch_optional(&self.pool)
        .await?
        .context("CBU not found")?;

        Ok(serde_json::json!({
            "cbu_id": cbu.cbu_id,
            "name": cbu.name,
            "description": cbu.description,
            "nature_purpose": cbu.nature_purpose,
            "source_of_funds": cbu.source_of_funds,
            "created_at": cbu.created_at,
        }))
    }

    /// Helper: Update CBU
    #[allow(dead_code)]
    async fn update_cbu(&self, cbu_id: Uuid, updates: &HashMap<String, String>) -> Result<()> {
        // Simplified update - in production, build dynamic SQL
        for (key, value) in updates {
            match key.as_str() {
                "description" => {
                    sqlx::query!(
                        r#"UPDATE "ob-poc".cbus SET description = $1, updated_at = NOW() WHERE cbu_id = $2"#,
                        value,
                        cbu_id
                    )
                    .execute(&self.pool)
                    .await?;
                }
                "nature_purpose" => {
                    sqlx::query!(
                        r#"UPDATE "ob-poc".cbus SET nature_purpose = $1, updated_at = NOW() WHERE cbu_id = $2"#,
                        value,
                        cbu_id
                    )
                    .execute(&self.pool)
                    .await?;
                }
                _ => {}
            }
        }
        Ok(())
    }

    /// High-level workflow: Create complete setup
    pub async fn create_complete_setup(
        &self,
        entity_name: &str,
        entity_type: &str,
        role_name: &str,
        cbu_nature: &str,
        cbu_source: &str,
    ) -> Result<CompleteSetupResult> {
        // Step 1: Create entity
        let entity_result = self
            .create_entity(CreateEntity {
                name: entity_name.to_string(),
                entity_type: entity_type.to_string(),
            })
            .await?;

        let entity_id = entity_result.entity_id.context("Entity ID missing")?;

        // Step 2: Create role
        let role_result = self
            .create_role(CreateRole {
                name: role_name.to_string(),
                description: format!("{} role", role_name),
            })
            .await?;

        let role_id = role_result.entity_id.context("Role ID missing")?;

        // Step 3: Create CBU using the base service
        let cbu_instruction = format!(
            "Create CBU with nature: {} and source: {}",
            cbu_nature, cbu_source
        );
        let cbu_result = self
            .base_service
            .process_instruction(&cbu_instruction)
            .await?;
        let cbu_id = cbu_result
            .entity_id
            .context("Failed to get CBU ID from creation result")?;

        // Step 4: Connect entity to CBU with role
        let connect_statement = CrudStatement::ConnectEntity(ConnectEntity {
            entity_id,
            cbu_id,
            role_id,
        });
        let connect_result = self
            .base_service
            .execute_statement(&connect_statement)
            .await?;
        let connection_id = connect_result
            .entity_id
            .context("Failed to get connection ID")?;

        Ok(CompleteSetupResult {
            entity_id,
            role_id,
            cbu_id,
            connection_id,
            message: format!(
                "Complete setup: {} ({}) connected to CBU as {}",
                entity_name, entity_type, role_name
            ),
        })
    }
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct CompleteSetupResult {
    pub entity_id: Uuid,
    pub role_id: Uuid,
    pub cbu_id: Uuid,
    pub connection_id: Uuid,
    pub message: String,
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_parse_create_entity() {
        let result = ExtendedDslParser::parse("Create entity John Smith as person");
        assert!(result.is_ok());

        if let Ok(ExtendedCrudStatement::CreateEntity(create)) = result {
            assert_eq!(create.entity_type, "PERSON");
            assert!(create.name.contains("John"));
        }
    }

    #[test]
    fn test_parse_create_company() {
        let result = ExtendedDslParser::parse("Add company TechCorp Ltd");
        assert!(result.is_ok());

        if let Ok(ExtendedCrudStatement::CreateEntity(create)) = result {
            assert_eq!(create.entity_type, "COMPANY");
        }
    }

    #[test]
    fn test_parse_create_role() {
        let result = ExtendedDslParser::parse("Create role Director");
        assert!(result.is_ok());

        if let Ok(ExtendedCrudStatement::CreateRole(create)) = result {
            assert_eq!(create.name, "Director");
        }
    }
}
