//! Entity custom operations
//!
//! Ghost entity lifecycle operations for progressive refinement.

use anyhow::Result;
use async_trait::async_trait;

use super::CustomOperation;
use crate::dsl_v2::ast::VerbCall;
use crate::dsl_v2::executor::{ExecutionContext, ExecutionResult};

#[cfg(feature = "database")]
use sqlx::PgPool;

#[cfg(feature = "database")]
use super::helpers;

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
