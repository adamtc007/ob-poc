//! Entity custom operations
//!
//! Operations for entity creation that require dynamic type/table mapping.

use anyhow::Result;
use async_trait::async_trait;

use super::CustomOperation;
use crate::dsl_v2::ast::VerbCall;
use crate::dsl_v2::executor::{ExecutionContext, ExecutionResult};

#[cfg(feature = "database")]
use sqlx::PgPool;

/// Generic entity creation with type dispatch
///
/// Rationale: Requires mapping :type argument to entity_type and selecting
/// the correct extension table (proper_persons, limited_companies, etc.)
pub struct EntityCreateOp;

#[async_trait]
impl CustomOperation for EntityCreateOp {
    fn domain(&self) -> &'static str {
        "entity"
    }
    fn verb(&self) -> &'static str {
        "create"
    }
    fn rationale(&self) -> &'static str {
        "Requires mapping :type to entity_type and selecting correct extension table"
    }

    #[cfg(feature = "database")]
    async fn execute(
        &self,
        verb_call: &VerbCall,
        ctx: &mut ExecutionContext,
        pool: &PgPool,
    ) -> Result<ExecutionResult> {
        use uuid::Uuid;

        // Extract entity type
        let entity_type = verb_call
            .arguments
            .iter()
            .find(|a| a.key == "type")
            .and_then(|a| a.value.as_string())
            .ok_or_else(|| anyhow::anyhow!("Missing :type argument"))?;

        // Extract name
        let name = verb_call
            .arguments
            .iter()
            .find(|a| a.key == "name")
            .and_then(|a| a.value.as_string())
            .ok_or_else(|| anyhow::anyhow!("Missing :name argument"))?;

        // Map type string to entity_type_name and extension table
        let (entity_type_name, extension_table) = match entity_type {
            "natural-person" => ("PROPER_PERSON_NATURAL", "entity_proper_persons"),
            "limited-company" => ("LIMITED_COMPANY_PRIVATE", "entity_limited_companies"),
            "partnership" => ("PARTNERSHIP_LIMITED", "entity_partnerships"),
            "trust" => ("TRUST_DISCRETIONARY", "entity_trusts"),
            _ => return Err(anyhow::anyhow!("Unknown entity type: {}", entity_type)),
        };

        // Idempotency: Check for existing entity with same name
        // For proper_persons, we split name and check first_name + last_name
        // For companies/partnerships/trusts, we check the name column directly
        let existing_entity_id: Option<Uuid> = match extension_table {
            "entity_proper_persons" => {
                let name_parts: Vec<&str> = name.split_whitespace().collect();
                let (first_name, last_name) = if name_parts.len() >= 2 {
                    (name_parts[0].to_string(), name_parts[1..].join(" "))
                } else {
                    (name.to_string(), "".to_string())
                };
                sqlx::query_scalar(
                    r#"SELECT entity_id FROM "ob-poc".entity_proper_persons
                       WHERE first_name = $1 AND last_name = $2
                       LIMIT 1"#,
                )
                .bind(&first_name)
                .bind(&last_name)
                .fetch_optional(pool)
                .await?
            }
            "entity_limited_companies" => {
                sqlx::query_scalar(
                    r#"SELECT entity_id FROM "ob-poc".entity_limited_companies
                       WHERE company_name = $1
                       LIMIT 1"#,
                )
                .bind(name)
                .fetch_optional(pool)
                .await?
            }
            "entity_partnerships" => {
                sqlx::query_scalar(
                    r#"SELECT entity_id FROM "ob-poc".entity_partnerships
                       WHERE partnership_name = $1
                       LIMIT 1"#,
                )
                .bind(name)
                .fetch_optional(pool)
                .await?
            }
            "entity_trusts" => {
                sqlx::query_scalar(
                    r#"SELECT entity_id FROM "ob-poc".entity_trusts
                       WHERE trust_name = $1
                       LIMIT 1"#,
                )
                .bind(name)
                .fetch_optional(pool)
                .await?
            }
            _ => None,
        };

        // If entity already exists, return existing ID
        if let Some(existing_id) = existing_entity_id {
            ctx.bind("entity", existing_id);
            return Ok(ExecutionResult::Uuid(existing_id));
        }

        // Look up entity type ID
        let type_row = sqlx::query!(
            r#"SELECT entity_type_id FROM "ob-poc".entity_types WHERE name = $1"#,
            entity_type_name
        )
        .fetch_optional(pool)
        .await?
        .ok_or_else(|| anyhow::anyhow!("Entity type not found: {}", entity_type_name))?;

        let entity_type_id = type_row.entity_type_id;
        let entity_id = Uuid::new_v4();

        // Insert into base entities table
        sqlx::query!(
            r#"INSERT INTO "ob-poc".entities (entity_id, entity_type_id, created_at, updated_at)
               VALUES ($1, $2, NOW(), NOW())"#,
            entity_id,
            entity_type_id
        )
        .execute(pool)
        .await?;

        // Insert into extension table based on type
        match extension_table {
            "entity_proper_persons" => {
                // Split name into first/last for proper_persons
                let name_parts: Vec<&str> = name.split_whitespace().collect();
                let (first_name, last_name) = if name_parts.len() >= 2 {
                    (name_parts[0].to_string(), name_parts[1..].join(" "))
                } else {
                    (name.to_string(), "".to_string())
                };

                sqlx::query!(
                    r#"INSERT INTO "ob-poc".entity_proper_persons (entity_id, first_name, last_name)
                       VALUES ($1, $2, $3)"#,
                    entity_id,
                    first_name,
                    last_name
                )
                .execute(pool)
                .await?;
            }
            "entity_limited_companies" => {
                sqlx::query!(
                    r#"INSERT INTO "ob-poc".entity_limited_companies (entity_id, company_name)
                       VALUES ($1, $2)"#,
                    entity_id,
                    name
                )
                .execute(pool)
                .await?;
            }
            "entity_partnerships" => {
                sqlx::query!(
                    r#"INSERT INTO "ob-poc".entity_partnerships (entity_id, partnership_name)
                       VALUES ($1, $2)"#,
                    entity_id,
                    name
                )
                .execute(pool)
                .await?;
            }
            "entity_trusts" => {
                sqlx::query!(
                    r#"INSERT INTO "ob-poc".entity_trusts (entity_id, trust_name)
                       VALUES ($1, $2)"#,
                    entity_id,
                    name
                )
                .execute(pool)
                .await?;
            }
            _ => {
                return Err(anyhow::anyhow!(
                    "Unknown extension table: {}",
                    extension_table
                ))
            }
        }

        // Bind to context
        ctx.bind("entity", entity_id);

        Ok(ExecutionResult::Uuid(entity_id))
    }

    #[cfg(not(feature = "database"))]
    async fn execute(
        &self,
        _verb_call: &VerbCall,
        _ctx: &mut ExecutionContext,
    ) -> Result<ExecutionResult> {
        Ok(ExecutionResult::Uuid(uuid::Uuid::new_v4()))
    }
}
