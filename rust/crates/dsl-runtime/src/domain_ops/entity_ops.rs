//! Entity custom operations.
//!
//! Ghost entity lifecycle (ghost → identified → verified) and placeholder
//! entity lifecycle (pending → resolved → verified). Ghost/placeholder
//! semantics are stubs used during progressive refinement, document
//! extraction, and macro expansion where the real entity is not yet known.

use anyhow::Result;
use async_trait::async_trait;
use dsl_runtime_macros::register_custom_op;
use sqlx::PgPool;
use uuid::Uuid;

use crate::custom_op::CustomOperation;
use crate::domain_ops::helpers::{
    json_extract_string, json_extract_string_opt, json_extract_uuid, json_extract_uuid_opt,
};
use crate::execution::{VerbExecutionContext, VerbExecutionOutcome};
use crate::placeholder::{PlaceholderResolver, ResolvePlaceholderRequest};

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

    async fn execute_json(
        &self,
        args: &serde_json::Value,
        ctx: &mut VerbExecutionContext,
        pool: &PgPool,
    ) -> Result<VerbExecutionOutcome> {
        let name = json_extract_string(args, "name")?;
        let source = json_extract_string_opt(args, "source");
        let source_reference = json_extract_string_opt(args, "source-reference");

        let name_parts: Vec<&str> = name.split_whitespace().collect();
        let (first_name, last_name) = if name_parts.len() >= 2 {
            (name_parts[0].to_string(), name_parts[1..].join(" "))
        } else {
            (name.to_string(), String::new())
        };

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
            return Ok(VerbExecutionOutcome::Uuid(existing_id));
        }

        let type_row: (Uuid,) = sqlx::query_as(
            r#"SELECT entity_type_id FROM "ob-poc".entity_types WHERE name = 'PROPER_PERSON_NATURAL'"#,
        )
        .fetch_optional(pool)
        .await?
        .ok_or_else(|| anyhow::anyhow!("Entity type PROPER_PERSON_NATURAL not found"))?;

        let entity_id = Uuid::new_v4();

        sqlx::query(
            r#"INSERT INTO "ob-poc".entities (entity_id, entity_type_id, created_at, updated_at)
               VALUES ($1, $2, NOW(), NOW())"#,
        )
        .bind(entity_id)
        .bind(type_row.0)
        .execute(pool)
        .await?;

        let search_name = format!("{} {}", first_name, last_name).trim().to_string();

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

        if source.is_some() || source_reference.is_some() {
            tracing::info!(
                entity_id = %entity_id,
                source = ?source,
                source_reference = ?source_reference,
                "Created ghost entity"
            );
        }

        ctx.bind("entity", entity_id);
        Ok(VerbExecutionOutcome::Uuid(entity_id))
    }

    fn is_migrated(&self) -> bool {
        true
    }
}

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

    async fn execute_json(
        &self,
        args: &serde_json::Value,
        ctx: &mut VerbExecutionContext,
        pool: &PgPool,
    ) -> Result<VerbExecutionOutcome> {
        let entity_id = json_extract_uuid(args, ctx, "entity-id")?;

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

        if current_state != "GHOST" {
            return Err(anyhow::anyhow!(
                "Entity {} is already in state {} - cannot identify (only GHOST entities can be identified)",
                entity_id,
                current_state
            ));
        }

        let new_first_name = json_extract_string_opt(args, "first-name");
        let new_last_name = json_extract_string_opt(args, "last-name");
        let date_of_birth = json_extract_string_opt(args, "date-of-birth");
        let nationality = json_extract_string_opt(args, "nationality");
        let residence_address = json_extract_string_opt(args, "residence-address");
        let id_document_type = json_extract_string_opt(args, "id-document-type");
        let id_document_number = json_extract_string_opt(args, "id-document-number");

        let final_first_name = new_first_name.unwrap_or_else(|| first_name.unwrap_or_default());
        let final_last_name = new_last_name.unwrap_or_else(|| last_name.unwrap_or_default());

        let dob: Option<chrono::NaiveDate> = if let Some(dob_str) = &date_of_birth {
            Some(
                chrono::NaiveDate::parse_from_str(dob_str, "%Y-%m-%d").map_err(|e| {
                    anyhow::anyhow!("Invalid date-of-birth format (expected YYYY-MM-DD): {}", e)
                })?,
            )
        } else {
            None
        };

        let search_name = format!("{} {}", final_first_name, final_last_name)
            .trim()
            .to_string();

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
        Ok(VerbExecutionOutcome::Uuid(entity_id))
    }

    fn is_migrated(&self) -> bool {
        true
    }
}

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

    async fn execute_json(
        &self,
        args: &serde_json::Value,
        ctx: &mut VerbExecutionContext,
        pool: &PgPool,
    ) -> Result<VerbExecutionOutcome> {
        let entity_ref = json_extract_uuid_opt(args, ctx, "ref");
        let kind = json_extract_string(args, "kind")?;
        let cbu_id = json_extract_uuid(args, ctx, "cbu-id")?;
        let name_hint = json_extract_string_opt(args, "name-hint");

        let resolver = PlaceholderResolver::new(pool.clone());
        let (entity_id, is_placeholder) = resolver
            .ensure_or_placeholder(entity_ref, &kind, cbu_id, name_hint)
            .await?;

        ctx.bind("entity", entity_id);

        Ok(VerbExecutionOutcome::Record(serde_json::json!({
            "entity_id": entity_id,
            "is_placeholder": is_placeholder
        })))
    }

    fn is_migrated(&self) -> bool {
        true
    }
}

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

    async fn execute_json(
        &self,
        args: &serde_json::Value,
        ctx: &mut VerbExecutionContext,
        pool: &PgPool,
    ) -> Result<VerbExecutionOutcome> {
        let placeholder_id = json_extract_uuid(args, ctx, "placeholder-id")?;
        let resolved_entity_id = json_extract_uuid(args, ctx, "resolved-entity-id")?;
        let resolved_by =
            json_extract_string_opt(args, "resolved-by").unwrap_or_else(|| "system".to_string());

        let resolver = PlaceholderResolver::new(pool.clone());
        let result = resolver
            .resolve(ResolvePlaceholderRequest {
                placeholder_entity_id: placeholder_id,
                resolved_entity_id,
                resolved_by,
            })
            .await?;

        Ok(VerbExecutionOutcome::Record(serde_json::to_value(result)?))
    }

    fn is_migrated(&self) -> bool {
        true
    }
}

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

    async fn execute_json(
        &self,
        args: &serde_json::Value,
        ctx: &mut VerbExecutionContext,
        pool: &PgPool,
    ) -> Result<VerbExecutionOutcome> {
        let cbu_id = json_extract_uuid_opt(args, ctx, "cbu-id");

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

        Ok(VerbExecutionOutcome::RecordSet(records))
    }

    fn is_migrated(&self) -> bool {
        true
    }
}

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

    async fn execute_json(
        &self,
        args: &serde_json::Value,
        ctx: &mut VerbExecutionContext,
        pool: &PgPool,
    ) -> Result<VerbExecutionOutcome> {
        let cbu_id = json_extract_uuid(args, ctx, "cbu-id")?;

        let resolver = PlaceholderResolver::new(pool.clone());
        let summary = resolver.get_summary(cbu_id).await?;

        let by_kind: Vec<serde_json::Value> = summary
            .by_kind
            .into_iter()
            .map(|k| {
                serde_json::json!({
                    "kind": k.kind,
                    "count": k.count
                })
            })
            .collect();

        Ok(VerbExecutionOutcome::Record(serde_json::json!({
            "cbu_id": summary.cbu_id,
            "pending_count": summary.pending_count,
            "by_kind": by_kind
        })))
    }

    fn is_migrated(&self) -> bool {
        true
    }
}
