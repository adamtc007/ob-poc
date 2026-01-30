//! Entity custom operations
//!
//! Ghost entity lifecycle operations for progressive refinement.
//! Placeholder entity operations for deferred resolution.

use anyhow::Result;
use async_trait::async_trait;
use ob_poc_macros::register_custom_op;

use super::CustomOperation;
use crate::dsl_v2::ast::VerbCall;
use crate::dsl_v2::executor::{ExecutionContext, ExecutionResult};

#[cfg(feature = "database")]
use sqlx::PgPool;

#[cfg(feature = "database")]
use super::helpers;

#[cfg(feature = "database")]
use crate::placeholder::{PlaceholderResolver, ResolvePlaceholderRequest};

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
#[register_custom_op]
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
#[register_custom_op]
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

// =============================================================================
// PLACEHOLDER ENTITY OPERATIONS
// =============================================================================

/// Ensure an entity exists or create a placeholder for later resolution.
///
/// This is the core operation used by structure macros when a service provider
/// entity reference is not yet known. It:
/// - Returns the existing entity if `ref` is provided and valid
/// - Creates a placeholder entity if `ref` is empty or not found
///
/// Placeholders are stub entity records with:
/// - `placeholder_status = 'pending'`
/// - `placeholder_kind` set to the role (depositary, auditor, etc.)
/// - `placeholder_created_for` pointing to the CBU
#[register_custom_op]
pub struct EntityEnsureOrPlaceholderOp;

#[async_trait]
impl CustomOperation for EntityEnsureOrPlaceholderOp {
    fn domain(&self) -> &'static str {
        "entity"
    }
    fn verb(&self) -> &'static str {
        "ensure-or-placeholder"
    }
    fn rationale(&self) -> &'static str {
        "Creates placeholder entities for deferred resolution in macro expansion; requires state management"
    }

    #[cfg(feature = "database")]
    async fn execute(
        &self,
        verb_call: &VerbCall,
        ctx: &mut ExecutionContext,
        pool: &PgPool,
    ) -> Result<ExecutionResult> {
        use serde_json::json;

        // Extract arguments - ref is optional
        let entity_ref = helpers::extract_uuid_opt(verb_call, ctx, "ref");
        let kind = helpers::extract_string(verb_call, "kind")?;
        let cbu_id = helpers::extract_uuid(verb_call, ctx, "cbu-id")?;
        let name_hint = helpers::extract_string_opt(verb_call, "name-hint");

        // Use PlaceholderResolver
        let resolver = PlaceholderResolver::new(pool.clone());
        let (entity_id, is_placeholder) = resolver
            .ensure_or_placeholder(entity_ref, &kind, cbu_id, name_hint)
            .await?;

        ctx.bind("entity", entity_id);

        Ok(ExecutionResult::Record(json!({
            "entity_id": entity_id,
            "is_placeholder": is_placeholder
        })))
    }

    #[cfg(not(feature = "database"))]
    async fn execute(
        &self,
        _verb_call: &VerbCall,
        _ctx: &mut ExecutionContext,
    ) -> Result<ExecutionResult> {
        use serde_json::json;
        Ok(ExecutionResult::Record(json!({
            "entity_id": uuid::Uuid::new_v4(),
            "is_placeholder": true
        })))
    }
}

/// Resolve a placeholder entity to a real entity.
///
/// This transfers any role assignments from the placeholder to the real entity
/// and marks the placeholder as resolved.
#[register_custom_op]
pub struct EntityResolvePlaceholderOp;

#[async_trait]
impl CustomOperation for EntityResolvePlaceholderOp {
    fn domain(&self) -> &'static str {
        "entity"
    }
    fn verb(&self) -> &'static str {
        "resolve-placeholder"
    }
    fn rationale(&self) -> &'static str {
        "Resolves placeholder to real entity with role transfer; requires transaction and state management"
    }

    #[cfg(feature = "database")]
    async fn execute(
        &self,
        verb_call: &VerbCall,
        ctx: &mut ExecutionContext,
        pool: &PgPool,
    ) -> Result<ExecutionResult> {
        use serde_json::json;

        let placeholder_id = helpers::extract_uuid(verb_call, ctx, "placeholder-id")?;
        let resolved_entity_id = helpers::extract_uuid(verb_call, ctx, "resolved-entity-id")?;
        let resolved_by = helpers::extract_string_opt(verb_call, "resolved-by")
            .unwrap_or_else(|| "system".to_string());

        let resolver = PlaceholderResolver::new(pool.clone());
        let result = resolver
            .resolve(ResolvePlaceholderRequest {
                placeholder_entity_id: placeholder_id,
                resolved_entity_id,
                resolved_by,
            })
            .await?;

        Ok(ExecutionResult::Record(json!({
            "placeholder_entity_id": result.placeholder_entity_id,
            "resolved_to_entity_id": result.resolved_to_entity_id,
            "status": result.status.to_string(),
            "roles_transferred": result.roles_transferred
        })))
    }

    #[cfg(not(feature = "database"))]
    async fn execute(
        &self,
        _verb_call: &VerbCall,
        _ctx: &mut ExecutionContext,
    ) -> Result<ExecutionResult> {
        use serde_json::json;
        Ok(ExecutionResult::Record(json!({
            "placeholder_entity_id": uuid::Uuid::new_v4(),
            "resolved_to_entity_id": uuid::Uuid::new_v4(),
            "status": "resolved",
            "roles_transferred": 0
        })))
    }
}

/// List pending placeholder entities.
#[register_custom_op]
pub struct EntityListPlaceholdersOp;

#[async_trait]
impl CustomOperation for EntityListPlaceholdersOp {
    fn domain(&self) -> &'static str {
        "entity"
    }
    fn verb(&self) -> &'static str {
        "list-placeholders"
    }
    fn rationale(&self) -> &'static str {
        "Lists placeholders with details from custom view; requires join logic"
    }

    #[cfg(feature = "database")]
    async fn execute(
        &self,
        verb_call: &VerbCall,
        ctx: &mut ExecutionContext,
        pool: &PgPool,
    ) -> Result<ExecutionResult> {
        let cbu_id = helpers::extract_uuid_opt(verb_call, ctx, "cbu-id");

        let resolver = PlaceholderResolver::new(pool.clone());

        let placeholders = if let Some(cbu_id) = cbu_id {
            resolver.list_pending_for_cbu(cbu_id).await?
        } else {
            resolver.list_all_pending().await?
        };

        let records: Vec<serde_json::Value> = placeholders
            .into_iter()
            .map(|p| {
                serde_json::json!({
                    "entity_id": p.placeholder.entity_id,
                    "status": p.placeholder.status.to_string(),
                    "kind": p.placeholder.kind,
                    "cbu_id": p.placeholder.created_for_cbu_id,
                    "entity_name": p.entity_name,
                    "cbu_name": p.cbu_name,
                    "kind_label": p.kind_label,
                    "created_at": p.placeholder.created_at.to_rfc3339()
                })
            })
            .collect();

        Ok(ExecutionResult::RecordSet(records))
    }

    #[cfg(not(feature = "database"))]
    async fn execute(
        &self,
        _verb_call: &VerbCall,
        _ctx: &mut ExecutionContext,
    ) -> Result<ExecutionResult> {
        Ok(ExecutionResult::RecordSet(vec![]))
    }
}

/// Get placeholder summary statistics for a CBU.
#[register_custom_op]
pub struct EntityPlaceholderSummaryOp;

#[async_trait]
impl CustomOperation for EntityPlaceholderSummaryOp {
    fn domain(&self) -> &'static str {
        "entity"
    }
    fn verb(&self) -> &'static str {
        "placeholder-summary"
    }
    fn rationale(&self) -> &'static str {
        "Aggregates placeholder stats with grouping; requires custom query"
    }

    #[cfg(feature = "database")]
    async fn execute(
        &self,
        verb_call: &VerbCall,
        ctx: &mut ExecutionContext,
        pool: &PgPool,
    ) -> Result<ExecutionResult> {
        use serde_json::json;

        let cbu_id = helpers::extract_uuid(verb_call, ctx, "cbu-id")?;

        let resolver = PlaceholderResolver::new(pool.clone());
        let summary = resolver.get_summary(cbu_id).await?;

        let by_kind: Vec<serde_json::Value> = summary
            .by_kind
            .into_iter()
            .map(|k| {
                json!({
                    "kind": k.kind,
                    "count": k.count
                })
            })
            .collect();

        Ok(ExecutionResult::Record(json!({
            "cbu_id": summary.cbu_id,
            "pending_count": summary.pending_count,
            "by_kind": by_kind
        })))
    }

    #[cfg(not(feature = "database"))]
    async fn execute(
        &self,
        _verb_call: &VerbCall,
        _ctx: &mut ExecutionContext,
    ) -> Result<ExecutionResult> {
        use serde_json::json;
        Ok(ExecutionResult::Record(json!({
            "cbu_id": uuid::Uuid::new_v4(),
            "pending_count": 0,
            "by_kind": []
        })))
    }
}
