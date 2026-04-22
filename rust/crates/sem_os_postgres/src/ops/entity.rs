//! Entity domain verbs (6 plugin verbs) — SemOS-side YAML-first
//! re-implementation of the plugin subset of
//! `rust/config/verbs/entity.yaml`.
//!
//! Two lifecycles share the domain:
//! - **Ghost entity** (`ghost`, `identify`) — person entity
//!   created in GHOST state with minimal attrs, later transitioned
//!   to IDENTIFIED.
//! - **Placeholder** (`ensure-or-placeholder`, `resolve-placeholder`,
//!   `list-placeholders`, `placeholder-summary`) — stub entity used
//!   during progressive refinement / macro expansion; resolved to a
//!   real entity later via `PlaceholderResolver`.

use anyhow::{anyhow, Result};
use async_trait::async_trait;
use serde_json::{json, Value};
use uuid::Uuid;

use dsl_runtime::domain_ops::helpers::{
    json_extract_string, json_extract_string_opt, json_extract_uuid, json_extract_uuid_opt,
};
use dsl_runtime::placeholder::{PlaceholderResolver, ResolvePlaceholderRequest};
use dsl_runtime::tx::TransactionScope;
use dsl_runtime::{VerbExecutionContext, VerbExecutionOutcome};

use super::SemOsVerbOp;

// ── entity.ghost ──────────────────────────────────────────────────────────────

pub struct Ghost;

#[async_trait]
impl SemOsVerbOp for Ghost {
    fn fqn(&self) -> &str {
        "entity.ghost"
    }
    async fn execute(
        &self,
        args: &Value,
        ctx: &mut VerbExecutionContext,
        scope: &mut dyn TransactionScope,
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
        .fetch_optional(scope.executor())
        .await?;

        if let Some(existing_id) = existing {
            ctx.bind("entity", existing_id);
            return Ok(VerbExecutionOutcome::Uuid(existing_id));
        }

        let type_row: Option<(Uuid,)> = sqlx::query_as(
            r#"SELECT entity_type_id FROM "ob-poc".entity_types WHERE name = 'PROPER_PERSON_NATURAL'"#,
        )
        .fetch_optional(scope.executor())
        .await?;
        let type_row = type_row.ok_or_else(|| anyhow!("Entity type PROPER_PERSON_NATURAL not found"))?;

        let entity_id = Uuid::new_v4();

        sqlx::query(
            r#"INSERT INTO "ob-poc".entities (entity_id, entity_type_id, created_at, updated_at)
               VALUES ($1, $2, NOW(), NOW())"#,
        )
        .bind(entity_id)
        .bind(type_row.0)
        .execute(scope.executor())
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
        .execute(scope.executor())
        .await?;

        if source.is_some() || source_reference.is_some() {
            tracing::info!(
                entity_id = %entity_id,
                source = ?source,
                source_reference = ?source_reference,
                "Created ghost entity"
            );
        }

        // Phase C.3 (F7 follow-on, 2026-04-22): emit PendingStateAdvance
        // via the shared `emit_pending_state_advance` helper. Only on
        // genuine creation — the idempotent early-return at line 66
        // bypasses this code.
        dsl_runtime::domain_ops::helpers::emit_pending_state_advance(
            ctx,
            entity_id,
            "entity:ghost",
            "entity/identity",
            "entity.ghost — new ghost proper person",
        );

        ctx.bind("entity", entity_id);
        Ok(VerbExecutionOutcome::Uuid(entity_id))
    }
}

// ── entity.identify ───────────────────────────────────────────────────────────

pub struct Identify;

#[async_trait]
impl SemOsVerbOp for Identify {
    fn fqn(&self) -> &str {
        "entity.identify"
    }
    async fn execute(
        &self,
        args: &Value,
        ctx: &mut VerbExecutionContext,
        scope: &mut dyn TransactionScope,
    ) -> Result<VerbExecutionOutcome> {
        let entity_id = json_extract_uuid(args, ctx, "entity-id")?;

        let current: Option<(String, Option<String>, Option<String>)> = sqlx::query_as(
            r#"SELECT person_state, first_name, last_name
               FROM "ob-poc".entity_proper_persons
               WHERE entity_id = $1"#,
        )
        .bind(entity_id)
        .fetch_optional(scope.executor())
        .await?;

        let (current_state, first_name, last_name) = current
            .ok_or_else(|| anyhow!("Entity {} not found in entity_proper_persons", entity_id))?;

        if current_state != "GHOST" {
            return Err(anyhow!(
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

        let dob: Option<chrono::NaiveDate> = date_of_birth
            .as_deref()
            .map(|s| {
                chrono::NaiveDate::parse_from_str(s, "%Y-%m-%d")
                    .map_err(|e| anyhow!("Invalid date-of-birth format (expected YYYY-MM-DD): {}", e))
            })
            .transpose()?;

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
        .execute(scope.executor())
        .await?;

        tracing::info!(entity_id = %entity_id, "Entity transitioned from GHOST to IDENTIFIED");
        ctx.bind("entity", entity_id);
        Ok(VerbExecutionOutcome::Uuid(entity_id))
    }
}

// ── entity.ensure-or-placeholder ──────────────────────────────────────────────

pub struct EnsureOrPlaceholder;

#[async_trait]
impl SemOsVerbOp for EnsureOrPlaceholder {
    fn fqn(&self) -> &str {
        "entity.ensure-or-placeholder"
    }
    async fn execute(
        &self,
        args: &Value,
        ctx: &mut VerbExecutionContext,
        scope: &mut dyn TransactionScope,
    ) -> Result<VerbExecutionOutcome> {
        let entity_ref = json_extract_uuid_opt(args, ctx, "ref");
        let kind = json_extract_string(args, "kind")?;
        let cbu_id = json_extract_uuid(args, ctx, "cbu-id")?;
        let name_hint = json_extract_string_opt(args, "name-hint");

        let resolver = PlaceholderResolver::new(scope.pool().clone());
        let (entity_id, is_placeholder) = resolver
            .ensure_or_placeholder(entity_ref, &kind, cbu_id, name_hint)
            .await?;

        ctx.bind("entity", entity_id);
        Ok(VerbExecutionOutcome::Record(json!({
            "entity_id": entity_id,
            "is_placeholder": is_placeholder,
        })))
    }
}

// ── entity.resolve-placeholder ────────────────────────────────────────────────

pub struct ResolvePlaceholder;

#[async_trait]
impl SemOsVerbOp for ResolvePlaceholder {
    fn fqn(&self) -> &str {
        "entity.resolve-placeholder"
    }
    async fn execute(
        &self,
        args: &Value,
        ctx: &mut VerbExecutionContext,
        scope: &mut dyn TransactionScope,
    ) -> Result<VerbExecutionOutcome> {
        let placeholder_id = json_extract_uuid(args, ctx, "placeholder-id")?;
        let resolved_entity_id = json_extract_uuid(args, ctx, "resolved-entity-id")?;
        let resolved_by =
            json_extract_string_opt(args, "resolved-by").unwrap_or_else(|| "system".to_string());

        let resolver = PlaceholderResolver::new(scope.pool().clone());
        let result = resolver
            .resolve(ResolvePlaceholderRequest {
                placeholder_entity_id: placeholder_id,
                resolved_entity_id,
                resolved_by,
            })
            .await?;

        Ok(VerbExecutionOutcome::Record(serde_json::to_value(result)?))
    }
}

// ── entity.list-placeholders ──────────────────────────────────────────────────

pub struct ListPlaceholders;

#[async_trait]
impl SemOsVerbOp for ListPlaceholders {
    fn fqn(&self) -> &str {
        "entity.list-placeholders"
    }
    async fn execute(
        &self,
        args: &Value,
        ctx: &mut VerbExecutionContext,
        scope: &mut dyn TransactionScope,
    ) -> Result<VerbExecutionOutcome> {
        let cbu_id = json_extract_uuid_opt(args, ctx, "cbu-id");

        let resolver = PlaceholderResolver::new(scope.pool().clone());
        let placeholders = if let Some(cbu_id) = cbu_id {
            resolver.list_pending_for_cbu(cbu_id).await?
        } else {
            resolver.list_all_pending().await?
        };

        let records: Vec<Value> = placeholders
            .into_iter()
            .map(|p| {
                json!({
                    "entity_id": p.placeholder.entity_id,
                    "status": p.placeholder.status.to_string(),
                    "kind": p.placeholder.kind,
                    "cbu_id": p.placeholder.created_for_cbu_id,
                    "entity_name": p.entity_name,
                    "cbu_name": p.cbu_name,
                    "kind_label": p.kind_label,
                    "created_at": p.placeholder.created_at.to_rfc3339(),
                })
            })
            .collect();

        Ok(VerbExecutionOutcome::RecordSet(records))
    }
}

// ── entity.placeholder-summary ────────────────────────────────────────────────

pub struct PlaceholderSummary;

#[async_trait]
impl SemOsVerbOp for PlaceholderSummary {
    fn fqn(&self) -> &str {
        "entity.placeholder-summary"
    }
    async fn execute(
        &self,
        args: &Value,
        ctx: &mut VerbExecutionContext,
        scope: &mut dyn TransactionScope,
    ) -> Result<VerbExecutionOutcome> {
        let cbu_id = json_extract_uuid(args, ctx, "cbu-id")?;

        let resolver = PlaceholderResolver::new(scope.pool().clone());
        let summary = resolver.get_summary(cbu_id).await?;

        let by_kind: Vec<Value> = summary
            .by_kind
            .into_iter()
            .map(|k| json!({"kind": k.kind, "count": k.count}))
            .collect();

        Ok(VerbExecutionOutcome::Record(json!({
            "cbu_id": summary.cbu_id,
            "pending_count": summary.pending_count,
            "by_kind": by_kind,
        })))
    }
}
