//! Entity custom operations
//!
//! Operations for entity creation that require dynamic type/table mapping.
//! Also includes ghost entity lifecycle operations for progressive refinement.

use anyhow::Result;
use async_trait::async_trait;

use super::CustomOperation;
use crate::dsl_v2::ast::VerbCall;
use crate::dsl_v2::executor::{ExecutionContext, ExecutionResult};

#[cfg(feature = "database")]
use sqlx::PgPool;

#[cfg(feature = "database")]
use super::helpers;

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

/// Rename a person entity (only allowed in GHOST or IDENTIFIED states)
///
/// Once an entity is VERIFIED, their identity is confirmed by official documents
/// and the name cannot be changed. This prevents accidental or malicious name
/// changes after verification has been completed.
///
/// Use cases for renaming:
/// - Correcting typos in ghost entities discovered from documents
/// - Updating names during identification when more accurate info is available
/// - Handling name changes (marriage, legal name change) before verification
pub struct EntityRenameOp;

#[async_trait]
impl CustomOperation for EntityRenameOp {
    fn domain(&self) -> &'static str {
        "entity"
    }
    fn verb(&self) -> &'static str {
        "rename"
    }
    fn rationale(&self) -> &'static str {
        "Renames person entity with state-based restrictions; blocks rename after VERIFIED state"
    }

    #[cfg(feature = "database")]
    async fn execute(
        &self,
        verb_call: &VerbCall,
        ctx: &mut ExecutionContext,
        pool: &PgPool,
    ) -> Result<ExecutionResult> {
        // Get entity ID (supports @symbol or direct UUID)
        let entity_id = helpers::extract_uuid(verb_call, ctx, "entity-id")?;

        // Get new name (required)
        let new_name = helpers::extract_string(verb_call, "name")?;

        // Get current state
        let current: Option<(String,)> = sqlx::query_as(
            r#"SELECT person_state
               FROM "ob-poc".entity_proper_persons
               WHERE entity_id = $1"#,
        )
        .bind(entity_id)
        .fetch_optional(pool)
        .await?;

        let (current_state,) = current.ok_or_else(|| {
            anyhow::anyhow!("Entity {} not found in entity_proper_persons", entity_id)
        })?;

        // Block rename for VERIFIED entities
        if current_state == "VERIFIED" {
            return Err(anyhow::anyhow!(
                "Entity {} is VERIFIED - name cannot be changed after verification. \
                 The identity has been confirmed by official documents.",
                entity_id
            ));
        }

        // Split new name into first/last
        let name_parts: Vec<&str> = new_name.split_whitespace().collect();
        let (first_name, last_name) = if name_parts.len() >= 2 {
            (name_parts[0].to_string(), name_parts[1..].join(" "))
        } else {
            (new_name.clone(), String::new())
        };

        // Build search_name
        let search_name = format!("{} {}", first_name, last_name).trim().to_string();

        // Update name
        sqlx::query(
            r#"UPDATE "ob-poc".entity_proper_persons SET
                first_name = $2,
                last_name = $3,
                search_name = $4,
                updated_at = NOW()
               WHERE entity_id = $1"#,
        )
        .bind(entity_id)
        .bind(&first_name)
        .bind(&last_name)
        .bind(&search_name)
        .execute(pool)
        .await?;

        tracing::info!(
            entity_id = %entity_id,
            new_name = %new_name,
            state = %current_state,
            "Entity renamed"
        );

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

// =============================================================================
// GHOST ENTITY LIFECYCLE OPERATIONS
// =============================================================================

/// Create a ghost person entity (name only, minimal attributes)
///
/// Ghost entities represent discovered persons with only a name - e.g., from:
/// - UBO ownership chain traversal
/// - Document extraction (mentioned as shareholder, director, etc.)
/// - External registry lookups
///
/// Ghost entities:
/// - Have person_state = 'GHOST'
/// - Cannot proceed to KYC screening until identified
/// - Display with 👻 indicator in visualization
pub struct EntityGhostOp;

#[async_trait]
impl CustomOperation for EntityGhostOp {
    fn domain(&self) -> &'static str {
        "entity"
    }
    fn verb(&self) -> &'static str {
        "ghost"
    }
    fn rationale(&self) -> &'static str {
        "Creates person entity in GHOST state with minimal attributes; requires state management"
    }

    #[cfg(feature = "database")]
    async fn execute(
        &self,
        verb_call: &VerbCall,
        ctx: &mut ExecutionContext,
        pool: &PgPool,
    ) -> Result<ExecutionResult> {
        use uuid::Uuid;

        // Extract required name
        let name = helpers::extract_string(verb_call, "name")?;

        // Extract optional source tracking
        let source = helpers::extract_string_opt(verb_call, "source");
        let source_reference = helpers::extract_string_opt(verb_call, "source-reference");

        // Split name into first/last
        let name_parts: Vec<&str> = name.split_whitespace().collect();
        let (first_name, last_name) = if name_parts.len() >= 2 {
            (name_parts[0].to_string(), name_parts[1..].join(" "))
        } else {
            (name.to_string(), String::new())
        };

        // Check for existing ghost with same name (idempotency)
        let existing: Option<Uuid> = sqlx::query_scalar(
            r#"SELECT entity_id FROM "ob-poc".entity_proper_persons
               WHERE first_name = $1 AND last_name = $2 AND person_state = 'GHOST'
               LIMIT 1"#,
        )
        .bind(&first_name)
        .bind(&last_name)
        .fetch_optional(pool)
        .await?;

        if let Some(existing_id) = existing {
            ctx.bind("entity", existing_id);
            return Ok(ExecutionResult::Uuid(existing_id));
        }

        // Look up PROPER_PERSON entity type
        let type_row = sqlx::query!(
            r#"SELECT entity_type_id FROM "ob-poc".entity_types WHERE name = 'PROPER_PERSON_NATURAL'"#
        )
        .fetch_optional(pool)
        .await?
        .ok_or_else(|| anyhow::anyhow!("Entity type PROPER_PERSON_NATURAL not found"))?;

        let entity_id = Uuid::new_v4();

        // Insert into base entities table
        sqlx::query!(
            r#"INSERT INTO "ob-poc".entities (entity_id, entity_type_id, created_at, updated_at)
               VALUES ($1, $2, NOW(), NOW())"#,
            entity_id,
            type_row.entity_type_id
        )
        .execute(pool)
        .await?;

        // Build search_name for indexing
        let search_name = format!("{} {}", first_name, last_name).trim().to_string();

        // Insert into proper_persons with GHOST state
        // Use dynamic query to handle optional source fields
        sqlx::query(
            r#"INSERT INTO "ob-poc".entity_proper_persons
               (entity_id, first_name, last_name, search_name, person_state, created_at, updated_at)
               VALUES ($1, $2, $3, $4, 'GHOST', NOW(), NOW())"#,
        )
        .bind(entity_id)
        .bind(&first_name)
        .bind(&last_name)
        .bind(&search_name)
        .execute(pool)
        .await?;

        // Log source if provided (useful for audit trail)
        if source.is_some() || source_reference.is_some() {
            tracing::info!(
                entity_id = %entity_id,
                source = ?source,
                source_reference = ?source_reference,
                "Created ghost entity"
            );
        }

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

/// Add identifying attributes to a ghost entity, transitioning to IDENTIFIED state
///
/// This verb takes a ghost entity and adds identifying attributes:
/// - Date of birth
/// - Nationality
/// - Country of residence
/// - ID document details (passport, SSN, etc.)
///
/// Once identified, the entity can proceed to KYC screening.
pub struct EntityIdentifyOp;

#[async_trait]
impl CustomOperation for EntityIdentifyOp {
    fn domain(&self) -> &'static str {
        "entity"
    }
    fn verb(&self) -> &'static str {
        "identify"
    }
    fn rationale(&self) -> &'static str {
        "Transitions entity from GHOST to IDENTIFIED state with validation; requires state machine logic"
    }

    #[cfg(feature = "database")]
    async fn execute(
        &self,
        verb_call: &VerbCall,
        ctx: &mut ExecutionContext,
        pool: &PgPool,
    ) -> Result<ExecutionResult> {
        // Get entity ID (supports @symbol or direct UUID)
        let entity_id = helpers::extract_uuid(verb_call, ctx, "entity-id")?;

        // Get current state to validate transition
        let current: Option<(String, Option<String>, Option<String>)> = sqlx::query_as(
            r#"SELECT person_state, first_name, last_name
               FROM "ob-poc".entity_proper_persons
               WHERE entity_id = $1"#,
        )
        .bind(entity_id)
        .fetch_optional(pool)
        .await?;

        let (current_state, first_name, last_name) = current.ok_or_else(|| {
            anyhow::anyhow!("Entity {} not found in entity_proper_persons", entity_id)
        })?;

        // Only GHOST entities can be identified (IDENTIFIED/VERIFIED are already past this stage)
        if current_state != "GHOST" {
            return Err(anyhow::anyhow!(
                "Entity {} is already in state {} - cannot identify (only GHOST entities can be identified)",
                entity_id,
                current_state
            ));
        }

        // Extract optional identifying attributes
        let new_first_name = helpers::extract_string_opt(verb_call, "first-name");
        let new_last_name = helpers::extract_string_opt(verb_call, "last-name");
        let date_of_birth = helpers::extract_string_opt(verb_call, "date-of-birth");
        let nationality = helpers::extract_string_opt(verb_call, "nationality");
        let residence_address = helpers::extract_string_opt(verb_call, "residence-address");
        let id_document_type = helpers::extract_string_opt(verb_call, "id-document-type");
        let id_document_number = helpers::extract_string_opt(verb_call, "id-document-number");

        // Use provided names or existing ones
        let final_first_name = new_first_name.unwrap_or_else(|| first_name.unwrap_or_default());
        let final_last_name = new_last_name.unwrap_or_else(|| last_name.unwrap_or_default());

        // Parse date of birth if provided
        let dob: Option<chrono::NaiveDate> = if let Some(dob_str) = &date_of_birth {
            Some(
                chrono::NaiveDate::parse_from_str(dob_str, "%Y-%m-%d").map_err(|e| {
                    anyhow::anyhow!("Invalid date-of-birth format (expected YYYY-MM-DD): {}", e)
                })?,
            )
        } else {
            None
        };

        // Build search_name
        let search_name = format!("{} {}", final_first_name, final_last_name)
            .trim()
            .to_string();

        // Update entity with identifying attributes and transition to IDENTIFIED
        sqlx::query(
            r#"UPDATE "ob-poc".entity_proper_persons SET
                first_name = $2,
                last_name = $3,
                search_name = $4,
                date_of_birth = COALESCE($5, date_of_birth),
                nationality = COALESCE($6, nationality),
                residence_address = COALESCE($7, residence_address),
                id_document_type = COALESCE($8, id_document_type),
                id_document_number = COALESCE($9, id_document_number),
                person_state = 'IDENTIFIED',
                updated_at = NOW()
               WHERE entity_id = $1"#,
        )
        .bind(entity_id)
        .bind(&final_first_name)
        .bind(&final_last_name)
        .bind(&search_name)
        .bind(dob)
        .bind(nationality.as_deref())
        .bind(residence_address.as_deref())
        .bind(id_document_type.as_deref())
        .bind(id_document_number.as_deref())
        .execute(pool)
        .await?;

        tracing::info!(
            entity_id = %entity_id,
            "Entity transitioned from GHOST to IDENTIFIED"
        );

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
