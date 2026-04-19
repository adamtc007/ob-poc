//! Client Group Entity Context Operations
//!
//! These operations manage entity membership, roles, relationships, and shorthand tags
//! for client groups, enabling Candle-assisted semantic resolution from human language
//! to entity_ids.
//!
//! # Architecture
//!
//! - `client_group_entity` - Which entities belong to a client group
//! - `client_group_entity_roles` - Role assignments (via roles table FK)
//! - `client_group_relationship` - Provisional ownership/control edges
//! - `client_group_relationship_sources` - Multi-source lineage for trust-but-verify
//! - `client_group_entity_tag` - Human-readable shorthand labels
//! - `client_group_entity_tag_embedding` - Candle embeddings for semantic search
//!
//! # Verbs
//!
//! Entity Membership:
//! - `client-group.entity-add` - Add entity to group
//! - `client-group.entity-remove` - Remove entity from group
//! - `client-group.list-entities` - List entities in group
//!
//! Role Management:
//! - `client-group.assign-role` - Assign role to entity
//! - `client-group.remove-role` - Remove role assignment
//! - `client-group.list-roles` - List role assignments
//! - `client-group.list-parties` - List all parties with roles
//!
//! Relationship Management:
//! - `client-group.add-relationship` - Add ownership/control edge
//! - `client-group.list-relationships` - List relationships
//!
//! Ownership Sources:
//! - `client-group.add-ownership-source` - Add source/allegation
//! - `client-group.verify-ownership` - Mark source as verified
//! - `client-group.set-canonical` - Designate canonical source
//! - `client-group.list-unverified` - List unverified allegations
//! - `client-group.list-discrepancies` - List conflicting values
//!
//! Shorthand Tags:
//! - `client-group.tag-add` - Add shorthand tag to entity
//! - `client-group.tag-remove` - Remove tag
//! - `client-group.list-tags` - List tags
//!
//! Semantic Search:
//! - `client-group.search-entities` - Search entities by shorthand (Candle-assisted)
//!
//! Discovery:
//! - `client-group.discover-entities` - Find entities that might belong to group
//! - `client-group.confirm-entity` - Confirm suspected entity
//! - `client-group.reject-entity` - Reject suspected entity
//! - `client-group.start-discovery` - Start discovery workflow
//! - `client-group.complete-discovery` - Complete discovery workflow

use anyhow::Result;
use async_trait::async_trait;
use dsl_runtime_macros::register_custom_op;
use serde::{Deserialize, Serialize};
use serde_json::json;
use uuid::Uuid;

use super::CustomOperation;
use crate::dsl_v2::ast::VerbCall;
use crate::dsl_v2::executor::{ExecutionContext, ExecutionResult};

#[cfg(feature = "database")]
use sqlx::PgPool;

// =============================================================================
// RESULT TYPES
// =============================================================================

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct EntityMembershipListItem {
    pub entity_id: Uuid,
    pub entity_name: String,
    pub membership_type: String,
    pub added_by: String,
    pub tags: Vec<String>,
    pub created_at: String,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct TagResult {
    pub tag_id: Uuid,
    pub entity_id: Uuid,
    pub entity_name: String,
    pub tag: String,
    pub persona: Option<String>,
    pub source: String,
    pub confidence: f64,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct SearchResult {
    pub entity_id: Uuid,
    pub entity_name: String,
    pub matched_tag: String,
    pub confidence: f64,
    pub match_type: String,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct DiscoveryResult {
    pub entity_id: Uuid,
    pub entity_name: String,
    pub entity_type: Option<String>,
    pub match_reason: String,
    pub already_member: bool,
}

// =============================================================================
// HELPER FUNCTIONS
// =============================================================================

#[cfg(feature = "database")]
fn get_required_uuid(verb_call: &VerbCall, key: &str, ctx: &ExecutionContext) -> Result<Uuid> {
    let arg = verb_call
        .arguments
        .iter()
        .find(|a| a.key == key)
        .ok_or_else(|| anyhow::anyhow!("Missing required argument :{}", key))?;

    if let Some(ref_name) = arg.value.as_symbol() {
        let resolved = ctx
            .resolve(ref_name)
            .ok_or_else(|| anyhow::anyhow!("Unresolved reference @{}", ref_name))?;
        return Ok(resolved);
    }

    if let Some(uuid_val) = arg.value.as_uuid() {
        return Ok(uuid_val);
    }

    if let Some(str_val) = arg.value.as_string() {
        return Uuid::parse_str(str_val)
            .map_err(|e| anyhow::anyhow!("Invalid UUID for :{}: {}", key, e));
    }

    Err(anyhow::anyhow!(":{} must be a UUID or @reference", key))
}

#[cfg(feature = "database")]
fn get_optional_uuid(verb_call: &VerbCall, key: &str, ctx: &ExecutionContext) -> Option<Uuid> {
    let arg = verb_call.arguments.iter().find(|a| a.key == key)?;

    if let Some(ref_name) = arg.value.as_symbol() {
        return ctx.resolve(ref_name);
    }

    if let Some(uuid_val) = arg.value.as_uuid() {
        return Some(uuid_val);
    }

    if let Some(str_val) = arg.value.as_string() {
        return Uuid::parse_str(str_val).ok();
    }

    None
}

#[cfg(feature = "database")]
fn get_optional_string(verb_call: &VerbCall, key: &str) -> Option<String> {
    verb_call
        .arguments
        .iter()
        .find(|a| a.key == key)
        .and_then(|a| a.value.as_string().map(|s| s.to_string()))
}

#[cfg(feature = "database")]
fn get_required_string(verb_call: &VerbCall, key: &str) -> Result<String> {
    get_optional_string(verb_call, key)
        .ok_or_else(|| anyhow::anyhow!("Missing required argument :{}", key))
}

#[cfg(feature = "database")]
fn get_optional_bool(verb_call: &VerbCall, key: &str) -> Option<bool> {
    verb_call
        .arguments
        .iter()
        .find(|a| a.key == key)
        .and_then(|a| a.value.as_boolean())
}

#[cfg(feature = "database")]
fn get_optional_integer(verb_call: &VerbCall, key: &str) -> Option<i64> {
    verb_call
        .arguments
        .iter()
        .find(|a| a.key == key)
        .and_then(|a| a.value.as_integer())
}

#[cfg(feature = "database")]
fn get_optional_string_array(verb_call: &VerbCall, key: &str) -> Option<Vec<String>> {
    verb_call
        .arguments
        .iter()
        .find(|a| a.key == key)
        .and_then(|a| {
            a.value.as_list().map(|list| {
                list.iter()
                    .filter_map(|v| v.as_string().map(|s| s.to_string()))
                    .collect()
            })
        })
}

// =============================================================================
// ENTITY-ADD
// =============================================================================

/// Consolidated entity lifecycle verb — dispatches add/remove/confirm/reject
/// by action argument.
#[register_custom_op]
pub struct ClientGroupEntityManageOp;

#[async_trait]
impl CustomOperation for ClientGroupEntityManageOp {
    fn domain(&self) -> &'static str {
        "client-group"
    }
    fn verb(&self) -> &'static str {
        "entity-manage"
    }
    fn rationale(&self) -> &'static str {
        "Consolidated entity lifecycle — dispatches by action arg"
    }

    #[cfg(feature = "database")]
    async fn execute(
        &self,
        verb_call: &VerbCall,
        ctx: &mut ExecutionContext,
        pool: &PgPool,
    ) -> Result<ExecutionResult> {
        let action = super::helpers::extract_string_opt(verb_call, "action")
            .ok_or_else(|| anyhow::anyhow!(":action required (add|remove|confirm|reject)"))?;

        match action.as_str() {
            "add" => ClientGroupEntityAddOp.execute(verb_call, ctx, pool).await,
            "remove" => {
                ClientGroupEntityRemoveOp
                    .execute(verb_call, ctx, pool)
                    .await
            }
            "confirm" => {
                ClientGroupConfirmEntityOp
                    .execute(verb_call, ctx, pool)
                    .await
            }
            "reject" => {
                ClientGroupRejectEntityOp
                    .execute(verb_call, ctx, pool)
                    .await
            }
            other => Err(anyhow::anyhow!(
                "Unknown entity-manage action '{}'. Valid: add, remove, confirm, reject",
                other
            )),
        }
    }

    #[cfg(not(feature = "database"))]
    async fn execute(
        &self,
        _verb_call: &VerbCall,
        _ctx: &mut ExecutionContext,
    ) -> Result<ExecutionResult> {
        Err(anyhow::anyhow!(
            "Database feature required for client-group operations"
        ))
    }

    #[cfg(feature = "database")]
    async fn execute_json(
        &self,
        args: &serde_json::Value,
        ctx: &mut dsl_runtime::VerbExecutionContext,
        pool: &PgPool,
    ) -> Result<dsl_runtime::VerbExecutionOutcome> {
        let action = args
            .get("action")
            .and_then(|value| value.as_str())
            .ok_or_else(|| anyhow::anyhow!(":action required (add|remove|confirm|reject)"))?;

        match action {
            "add" => ClientGroupEntityAddOp.execute_json(args, ctx, pool).await,
            "remove" => {
                ClientGroupEntityRemoveOp
                    .execute_json(args, ctx, pool)
                    .await
            }
            "confirm" => {
                ClientGroupConfirmEntityOp
                    .execute_json(args, ctx, pool)
                    .await
            }
            "reject" => {
                ClientGroupRejectEntityOp
                    .execute_json(args, ctx, pool)
                    .await
            }
            other => Err(anyhow::anyhow!(
                "Unknown entity-manage action '{}'. Valid: add, remove, confirm, reject",
                other
            )),
        }
    }

    fn is_migrated(&self) -> bool {
        true
    }
}

#[register_custom_op]
pub struct ClientGroupEntityAddOp;

#[async_trait]
impl CustomOperation for ClientGroupEntityAddOp {
    fn domain(&self) -> &'static str {
        "client-group"
    }

    fn verb(&self) -> &'static str {
        "entity-add"
    }

    fn rationale(&self) -> &'static str {
        "Add an entity to a client group's membership universe"
    }

    #[cfg(feature = "database")]
    async fn execute(
        &self,
        verb_call: &VerbCall,
        ctx: &mut ExecutionContext,
        pool: &PgPool,
    ) -> Result<ExecutionResult> {
        let group_id = get_required_uuid(verb_call, "group-id", ctx)?;
        let entity_id = get_required_uuid(verb_call, "entity-id", ctx)?;
        let membership_type = get_optional_string(verb_call, "membership-type")
            .unwrap_or_else(|| "confirmed".to_string());
        let notes = get_optional_string(verb_call, "notes");

        let result = sqlx::query!(
            r#"
            INSERT INTO "ob-poc".client_group_entity
                (group_id, entity_id, membership_type, added_by, notes)
            VALUES ($1, $2, $3, 'manual', $4)
            ON CONFLICT (group_id, entity_id) DO UPDATE SET
                membership_type = EXCLUDED.membership_type,
                notes = COALESCE(EXCLUDED.notes, client_group_entity.notes),
                updated_at = now()
            RETURNING id
            "#,
            group_id,
            entity_id,
            membership_type,
            notes
        )
        .fetch_one(pool)
        .await?;

        Ok(ExecutionResult::Uuid(result.id))
    }

    #[cfg(not(feature = "database"))]
    async fn execute(
        &self,
        _verb_call: &VerbCall,
        _ctx: &mut ExecutionContext,
    ) -> Result<ExecutionResult> {
        Err(anyhow::anyhow!(
            "Database feature required for client-group operations"
        ))
    }

    #[cfg(feature = "database")]
    async fn execute_json(
        &self,
        args: &serde_json::Value,
        ctx: &mut dsl_runtime::VerbExecutionContext,
        pool: &PgPool,
    ) -> Result<dsl_runtime::VerbExecutionOutcome> {
        use super::helpers::{json_extract_string_opt, json_extract_uuid};
        let group_id = json_extract_uuid(args, ctx, "group-id")?;
        let entity_id = json_extract_uuid(args, ctx, "entity-id")?;
        let membership_type = json_extract_string_opt(args, "membership-type")
            .unwrap_or_else(|| "confirmed".to_string());
        let notes = json_extract_string_opt(args, "notes");
        let id: Uuid = sqlx::query_scalar(
            r#"INSERT INTO "ob-poc".client_group_entity
                (group_id, entity_id, membership_type, added_by, notes)
            VALUES ($1, $2, $3, 'manual', $4)
            ON CONFLICT (group_id, entity_id) DO UPDATE SET
                membership_type = EXCLUDED.membership_type,
                notes = COALESCE(EXCLUDED.notes, client_group_entity.notes),
                updated_at = now()
            RETURNING id"#,
        )
        .bind(group_id)
        .bind(entity_id)
        .bind(&membership_type)
        .bind(&notes)
        .fetch_one(pool)
        .await?;
        Ok(dsl_runtime::VerbExecutionOutcome::Uuid(id))
    }

    fn is_migrated(&self) -> bool {
        true
    }
}

// =============================================================================
// ENTITY-REMOVE
// =============================================================================

#[register_custom_op]
pub struct ClientGroupEntityRemoveOp;

#[async_trait]
impl CustomOperation for ClientGroupEntityRemoveOp {
    fn domain(&self) -> &'static str {
        "client-group"
    }

    fn verb(&self) -> &'static str {
        "entity-remove"
    }

    fn rationale(&self) -> &'static str {
        "Remove an entity from a client group (or mark as historical)"
    }

    #[cfg(feature = "database")]
    async fn execute(
        &self,
        verb_call: &VerbCall,
        ctx: &mut ExecutionContext,
        pool: &PgPool,
    ) -> Result<ExecutionResult> {
        let group_id = get_required_uuid(verb_call, "group-id", ctx)?;
        let entity_id = get_required_uuid(verb_call, "entity-id", ctx)?;
        let mark_historical = get_optional_bool(verb_call, "mark-historical").unwrap_or(false);

        let affected = if mark_historical {
            sqlx::query!(
                r#"
                UPDATE "ob-poc".client_group_entity
                SET membership_type = 'historical', updated_at = now()
                WHERE group_id = $1 AND entity_id = $2
                "#,
                group_id,
                entity_id
            )
            .execute(pool)
            .await?
            .rows_affected()
        } else {
            sqlx::query!(
                r#"
                DELETE FROM "ob-poc".client_group_entity
                WHERE group_id = $1 AND entity_id = $2
                "#,
                group_id,
                entity_id
            )
            .execute(pool)
            .await?
            .rows_affected()
        };

        Ok(ExecutionResult::Record(json!({
            "removed": affected > 0,
            "mark_historical": mark_historical
        })))
    }

    #[cfg(not(feature = "database"))]
    async fn execute(
        &self,
        _verb_call: &VerbCall,
        _ctx: &mut ExecutionContext,
    ) -> Result<ExecutionResult> {
        Err(anyhow::anyhow!(
            "Database feature required for client-group operations"
        ))
    }

    #[cfg(feature = "database")]
    async fn execute_json(
        &self,
        args: &serde_json::Value,
        ctx: &mut dsl_runtime::VerbExecutionContext,
        pool: &PgPool,
    ) -> Result<dsl_runtime::VerbExecutionOutcome> {
        use super::helpers::{json_extract_bool_opt, json_extract_uuid};
        let group_id = json_extract_uuid(args, ctx, "group-id")?;
        let entity_id = json_extract_uuid(args, ctx, "entity-id")?;
        let mark_historical = json_extract_bool_opt(args, "mark-historical").unwrap_or(false);
        let affected = if mark_historical {
            sqlx::query(
                r#"UPDATE "ob-poc".client_group_entity
                SET membership_type = 'historical', updated_at = now()
                WHERE group_id = $1 AND entity_id = $2"#,
            )
            .bind(group_id)
            .bind(entity_id)
            .execute(pool)
            .await?
            .rows_affected()
        } else {
            sqlx::query(
                r#"DELETE FROM "ob-poc".client_group_entity
                WHERE group_id = $1 AND entity_id = $2"#,
            )
            .bind(group_id)
            .bind(entity_id)
            .execute(pool)
            .await?
            .rows_affected()
        };
        Ok(dsl_runtime::VerbExecutionOutcome::Record(
            json!({
                "removed": affected > 0,
                "mark_historical": mark_historical
            }),
        ))
    }

    fn is_migrated(&self) -> bool {
        true
    }
}

// =============================================================================
// LIST-ENTITIES
// =============================================================================

#[register_custom_op]
pub struct ClientGroupEntityListOp;

#[async_trait]
impl CustomOperation for ClientGroupEntityListOp {
    fn domain(&self) -> &'static str {
        "client-group"
    }

    fn verb(&self) -> &'static str {
        "list-entities"
    }

    fn rationale(&self) -> &'static str {
        "List all entities in a client group with their tags"
    }

    #[cfg(feature = "database")]
    async fn execute(
        &self,
        verb_call: &VerbCall,
        ctx: &mut ExecutionContext,
        pool: &PgPool,
    ) -> Result<ExecutionResult> {
        let group_id = get_required_uuid(verb_call, "group-id", ctx)?;
        let membership_type = get_optional_string(verb_call, "membership-type");
        let limit = get_optional_integer(verb_call, "limit").unwrap_or(100);

        let rows = sqlx::query!(
            r#"
            SELECT
                cge.entity_id,
                e.name as "entity_name!",
                cge.membership_type as "membership_type!",
                cge.added_by as "added_by!",
                cge.created_at as "created_at!",
                COALESCE(
                    (SELECT array_agg(cget.tag) FROM "ob-poc".client_group_entity_tag cget
                     WHERE cget.group_id = cge.group_id AND cget.entity_id = cge.entity_id),
                    ARRAY[]::TEXT[]
                ) as "tags!"
            FROM "ob-poc".client_group_entity cge
            JOIN "ob-poc".entities e ON e.entity_id = cge.entity_id
            WHERE cge.group_id = $1
              AND e.deleted_at IS NULL
              AND ($2::TEXT IS NULL OR cge.membership_type = $2)
            ORDER BY e.name
            LIMIT $3
            "#,
            group_id,
            membership_type,
            limit
        )
        .fetch_all(pool)
        .await?;

        let items: Vec<EntityMembershipListItem> = rows
            .into_iter()
            .map(|r| EntityMembershipListItem {
                entity_id: r.entity_id,
                entity_name: r.entity_name,
                membership_type: r.membership_type,
                added_by: r.added_by,
                tags: r.tags,
                created_at: r.created_at.to_rfc3339(),
            })
            .collect();

        Ok(ExecutionResult::RecordSet(
            items
                .iter()
                .map(serde_json::to_value)
                .collect::<Result<Vec<_>, _>>()?,
        ))
    }

    #[cfg(not(feature = "database"))]
    async fn execute(
        &self,
        _verb_call: &VerbCall,
        _ctx: &mut ExecutionContext,
    ) -> Result<ExecutionResult> {
        Err(anyhow::anyhow!(
            "Database feature required for client-group operations"
        ))
    }

    #[cfg(feature = "database")]
    async fn execute_json(
        &self,
        args: &serde_json::Value,
        ctx: &mut dsl_runtime::VerbExecutionContext,
        pool: &PgPool,
    ) -> Result<dsl_runtime::VerbExecutionOutcome> {
        use super::helpers::{json_extract_int_opt, json_extract_string_opt, json_extract_uuid};
        let group_id = json_extract_uuid(args, ctx, "group-id")?;
        let membership_type = json_extract_string_opt(args, "membership-type");
        let limit = json_extract_int_opt(args, "limit").unwrap_or(100);
        let rows: Vec<(
            Uuid,
            String,
            String,
            String,
            chrono::DateTime<chrono::Utc>,
            Vec<String>,
        )> = sqlx::query_as(
            r#"SELECT
                cge.entity_id,
                e.name,
                cge.membership_type,
                cge.added_by,
                cge.created_at,
                COALESCE(
                    (SELECT array_agg(cget.tag) FROM "ob-poc".client_group_entity_tag cget
                     WHERE cget.group_id = cge.group_id AND cget.entity_id = cge.entity_id),
                    ARRAY[]::TEXT[]
                )
            FROM "ob-poc".client_group_entity cge
            JOIN "ob-poc".entities e ON e.entity_id = cge.entity_id
            WHERE cge.group_id = $1
              AND e.deleted_at IS NULL
              AND ($2::TEXT IS NULL OR cge.membership_type = $2)
            ORDER BY e.name
            LIMIT $3"#,
        )
        .bind(group_id)
        .bind(&membership_type)
        .bind(limit)
        .fetch_all(pool)
        .await?;
        let items: Vec<serde_json::Value> = rows
            .into_iter()
            .map(|(entity_id, name, mtype, added_by, created_at, tags)| {
                json!({
                    "entity_id": entity_id,
                    "entity_name": name,
                    "membership_type": mtype,
                    "added_by": added_by,
                    "tags": tags,
                    "created_at": created_at.to_rfc3339(),
                })
            })
            .collect();
        Ok(dsl_runtime::VerbExecutionOutcome::RecordSet(
            items,
        ))
    }

    fn is_migrated(&self) -> bool {
        true
    }
}

// =============================================================================
// TAG-ADD
// =============================================================================

#[register_custom_op]
pub struct ClientGroupTagAddOp;

#[async_trait]
impl CustomOperation for ClientGroupTagAddOp {
    fn domain(&self) -> &'static str {
        "client-group"
    }

    fn verb(&self) -> &'static str {
        "tag-add"
    }

    fn rationale(&self) -> &'static str {
        "Add a shorthand tag to an entity for semantic search"
    }

    #[cfg(feature = "database")]
    async fn execute(
        &self,
        verb_call: &VerbCall,
        ctx: &mut ExecutionContext,
        pool: &PgPool,
    ) -> Result<ExecutionResult> {
        let group_id = get_required_uuid(verb_call, "group-id", ctx)?;
        let entity_id = get_required_uuid(verb_call, "entity-id", ctx)?;
        let tag = get_required_string(verb_call, "tag")?;
        let persona = get_optional_string(verb_call, "persona");

        // Ensure entity is a member of the group first
        sqlx::query!(
            r#"
            INSERT INTO "ob-poc".client_group_entity
                (group_id, entity_id, membership_type, added_by)
            VALUES ($1, $2, 'confirmed', 'tag_add')
            ON CONFLICT (group_id, entity_id) DO NOTHING
            "#,
            group_id,
            entity_id
        )
        .execute(pool)
        .await?;

        // Normalize tag
        let tag_norm: String =
            sqlx::query_scalar!(r#"SELECT "ob-poc".normalize_tag($1) as "tag_norm!""#, tag)
                .fetch_one(pool)
                .await?;

        // Insert tag
        let result = sqlx::query!(
            r#"
            INSERT INTO "ob-poc".client_group_entity_tag
                (group_id, entity_id, tag, tag_norm, persona, source, confidence)
            VALUES ($1, $2, $3, $4, $5, 'manual', 1.0)
            ON CONFLICT (group_id, entity_id, tag_norm, COALESCE(persona, ''))
            DO UPDATE SET
                confidence = GREATEST(client_group_entity_tag.confidence, 0.95),
                source = 'user_confirmed'
            RETURNING id
            "#,
            group_id,
            entity_id,
            tag,
            tag_norm,
            persona
        )
        .fetch_one(pool)
        .await?;

        // TODO: Queue embedding computation via background task
        // For now, embedding will be created by populate_embeddings binary

        Ok(ExecutionResult::Uuid(result.id))
    }

    #[cfg(not(feature = "database"))]
    async fn execute(
        &self,
        _verb_call: &VerbCall,
        _ctx: &mut ExecutionContext,
    ) -> Result<ExecutionResult> {
        Err(anyhow::anyhow!(
            "Database feature required for client-group operations"
        ))
    }

    #[cfg(feature = "database")]
    async fn execute_json(
        &self,
        args: &serde_json::Value,
        ctx: &mut dsl_runtime::VerbExecutionContext,
        pool: &PgPool,
    ) -> Result<dsl_runtime::VerbExecutionOutcome> {
        use super::helpers::{json_extract_string, json_extract_string_opt, json_extract_uuid};
        let group_id = json_extract_uuid(args, ctx, "group-id")?;
        let entity_id = json_extract_uuid(args, ctx, "entity-id")?;
        let tag = json_extract_string(args, "tag")?;
        let persona = json_extract_string_opt(args, "persona");
        sqlx::query(
            r#"INSERT INTO "ob-poc".client_group_entity
                (group_id, entity_id, membership_type, added_by)
            VALUES ($1, $2, 'confirmed', 'tag_add')
            ON CONFLICT (group_id, entity_id) DO NOTHING"#,
        )
        .bind(group_id)
        .bind(entity_id)
        .execute(pool)
        .await?;
        let tag_norm: String = sqlx::query_scalar(r#"SELECT "ob-poc".normalize_tag($1)"#)
            .bind(&tag)
            .fetch_one(pool)
            .await?;
        let id: Uuid = sqlx::query_scalar(
            r#"INSERT INTO "ob-poc".client_group_entity_tag
                (group_id, entity_id, tag, tag_norm, persona, source, confidence)
            VALUES ($1, $2, $3, $4, $5, 'manual', 1.0)
            ON CONFLICT (group_id, entity_id, tag_norm, COALESCE(persona, ''))
            DO UPDATE SET
                confidence = GREATEST(client_group_entity_tag.confidence, 0.95),
                source = 'user_confirmed'
            RETURNING id"#,
        )
        .bind(group_id)
        .bind(entity_id)
        .bind(&tag)
        .bind(&tag_norm)
        .bind(&persona)
        .fetch_one(pool)
        .await?;
        Ok(dsl_runtime::VerbExecutionOutcome::Uuid(id))
    }

    fn is_migrated(&self) -> bool {
        true
    }
}

// =============================================================================
// TAG-REMOVE
// =============================================================================

#[register_custom_op]
pub struct ClientGroupTagRemoveOp;

#[async_trait]
impl CustomOperation for ClientGroupTagRemoveOp {
    fn domain(&self) -> &'static str {
        "client-group"
    }

    fn verb(&self) -> &'static str {
        "tag-remove"
    }

    fn rationale(&self) -> &'static str {
        "Remove a shorthand tag"
    }

    #[cfg(feature = "database")]
    async fn execute(
        &self,
        verb_call: &VerbCall,
        ctx: &mut ExecutionContext,
        pool: &PgPool,
    ) -> Result<ExecutionResult> {
        let tag_id = get_required_uuid(verb_call, "tag-id", ctx)?;

        let affected = sqlx::query!(
            r#"DELETE FROM "ob-poc".client_group_entity_tag WHERE id = $1"#,
            tag_id
        )
        .execute(pool)
        .await?
        .rows_affected();

        Ok(ExecutionResult::Record(json!({
            "removed": affected > 0
        })))
    }

    #[cfg(not(feature = "database"))]
    async fn execute(
        &self,
        _verb_call: &VerbCall,
        _ctx: &mut ExecutionContext,
    ) -> Result<ExecutionResult> {
        Err(anyhow::anyhow!(
            "Database feature required for client-group operations"
        ))
    }

    #[cfg(feature = "database")]
    async fn execute_json(
        &self,
        args: &serde_json::Value,
        ctx: &mut dsl_runtime::VerbExecutionContext,
        pool: &PgPool,
    ) -> Result<dsl_runtime::VerbExecutionOutcome> {
        use super::helpers::json_extract_uuid;
        let tag_id = json_extract_uuid(args, ctx, "tag-id")?;
        let affected = sqlx::query(r#"DELETE FROM "ob-poc".client_group_entity_tag WHERE id = $1"#)
            .bind(tag_id)
            .execute(pool)
            .await?
            .rows_affected();
        Ok(dsl_runtime::VerbExecutionOutcome::Record(
            json!({ "removed": affected > 0 }),
        ))
    }

    fn is_migrated(&self) -> bool {
        true
    }
}

// =============================================================================
// LIST-TAGS
// =============================================================================

#[register_custom_op]
pub struct ClientGroupTagListOp;

#[async_trait]
impl CustomOperation for ClientGroupTagListOp {
    fn domain(&self) -> &'static str {
        "client-group"
    }

    fn verb(&self) -> &'static str {
        "list-tags"
    }

    fn rationale(&self) -> &'static str {
        "List shorthand tags for entities in a client group"
    }

    #[cfg(feature = "database")]
    async fn execute(
        &self,
        verb_call: &VerbCall,
        ctx: &mut ExecutionContext,
        pool: &PgPool,
    ) -> Result<ExecutionResult> {
        let group_id = get_required_uuid(verb_call, "group-id", ctx)?;
        let entity_id = get_optional_uuid(verb_call, "entity-id", ctx);
        let persona = get_optional_string(verb_call, "persona");

        let rows = sqlx::query!(
            r#"
            SELECT
                cget.id as tag_id,
                cget.entity_id,
                e.name as "entity_name!",
                cget.tag,
                cget.persona,
                cget.source,
                cget.confidence
            FROM "ob-poc".client_group_entity_tag cget
            JOIN "ob-poc".entities e ON e.entity_id = cget.entity_id
            WHERE cget.group_id = $1
              AND e.deleted_at IS NULL
              AND ($2::UUID IS NULL OR cget.entity_id = $2)
              AND ($3::TEXT IS NULL OR cget.persona IS NULL OR cget.persona = $3)
            ORDER BY e.name, cget.tag
            "#,
            group_id,
            entity_id,
            persona
        )
        .fetch_all(pool)
        .await?;

        let items: Vec<TagResult> = rows
            .into_iter()
            .map(|r| TagResult {
                tag_id: r.tag_id,
                entity_id: r.entity_id,
                entity_name: r.entity_name,
                tag: r.tag,
                persona: r.persona,
                source: r.source,
                confidence: r.confidence.unwrap_or(1.0),
            })
            .collect();

        Ok(ExecutionResult::RecordSet(
            items
                .iter()
                .map(serde_json::to_value)
                .collect::<Result<Vec<_>, _>>()?,
        ))
    }

    #[cfg(not(feature = "database"))]
    async fn execute(
        &self,
        _verb_call: &VerbCall,
        _ctx: &mut ExecutionContext,
    ) -> Result<ExecutionResult> {
        Err(anyhow::anyhow!(
            "Database feature required for client-group operations"
        ))
    }

    #[cfg(feature = "database")]
    async fn execute_json(
        &self,
        args: &serde_json::Value,
        ctx: &mut dsl_runtime::VerbExecutionContext,
        pool: &PgPool,
    ) -> Result<dsl_runtime::VerbExecutionOutcome> {
        use super::helpers::{json_extract_string_opt, json_extract_uuid, json_extract_uuid_opt};
        let group_id = json_extract_uuid(args, ctx, "group-id")?;
        let entity_id = json_extract_uuid_opt(args, ctx, "entity-id");
        let persona = json_extract_string_opt(args, "persona");
        let rows: Vec<(
            Uuid,
            Uuid,
            String,
            String,
            Option<String>,
            String,
            Option<f64>,
        )> = sqlx::query_as(
            r#"SELECT
                cget.id, cget.entity_id, e.name, cget.tag,
                cget.persona, cget.source, cget.confidence
            FROM "ob-poc".client_group_entity_tag cget
            JOIN "ob-poc".entities e ON e.entity_id = cget.entity_id
            WHERE cget.group_id = $1
              AND e.deleted_at IS NULL
              AND ($2::UUID IS NULL OR cget.entity_id = $2)
              AND ($3::TEXT IS NULL OR cget.persona IS NULL OR cget.persona = $3)
            ORDER BY e.name, cget.tag"#,
        )
        .bind(group_id)
        .bind(entity_id)
        .bind(&persona)
        .fetch_all(pool)
        .await?;
        let items: Vec<serde_json::Value> = rows
            .into_iter()
            .map(|(tag_id, eid, name, tag, persona, source, conf)| {
                json!({
                    "tag_id": tag_id, "entity_id": eid, "entity_name": name,
                    "tag": tag, "persona": persona, "source": source,
                    "confidence": conf.unwrap_or(1.0),
                })
            })
            .collect();
        Ok(dsl_runtime::VerbExecutionOutcome::RecordSet(
            items,
        ))
    }

    fn is_migrated(&self) -> bool {
        true
    }
}

// =============================================================================
// SEARCH-ENTITIES (Candle-assisted semantic search)
// =============================================================================

#[register_custom_op]
pub struct ClientGroupSearchOp;

#[async_trait]
impl CustomOperation for ClientGroupSearchOp {
    fn domain(&self) -> &'static str {
        "client-group"
    }

    fn verb(&self) -> &'static str {
        "search-entities"
    }

    fn rationale(&self) -> &'static str {
        "Semantic search for entities by shorthand tags (Candle-assisted)"
    }

    #[cfg(feature = "database")]
    async fn execute(
        &self,
        verb_call: &VerbCall,
        ctx: &mut ExecutionContext,
        pool: &PgPool,
    ) -> Result<ExecutionResult> {
        let group_id = get_required_uuid(verb_call, "group-id", ctx)?;
        let query = get_required_string(verb_call, "query")?;
        let persona = get_optional_string(verb_call, "persona");
        let limit = get_optional_integer(verb_call, "limit").unwrap_or(10) as i32;

        // Use the search_entity_tags function (text-based: exact + fuzzy)
        let rows = sqlx::query!(
            r#"
            SELECT
                entity_id as "entity_id!",
                entity_name as "entity_name!",
                tag as "tag!",
                confidence as "confidence!",
                match_type as "match_type!"
            FROM "ob-poc".search_entity_tags($1, $2, $3, $4, FALSE)
            "#,
            group_id,
            query,
            persona,
            limit
        )
        .fetch_all(pool)
        .await?;

        let items: Vec<SearchResult> = rows
            .into_iter()
            .map(|r| SearchResult {
                entity_id: r.entity_id,
                entity_name: r.entity_name,
                matched_tag: r.tag,
                confidence: r.confidence,
                match_type: r.match_type,
            })
            .collect();

        // TODO: If no text results, fall back to semantic search via EntityContextResolver
        // This requires Candle embedder integration

        Ok(ExecutionResult::RecordSet(
            items
                .iter()
                .map(serde_json::to_value)
                .collect::<Result<Vec<_>, _>>()?,
        ))
    }

    #[cfg(not(feature = "database"))]
    async fn execute(
        &self,
        _verb_call: &VerbCall,
        _ctx: &mut ExecutionContext,
    ) -> Result<ExecutionResult> {
        Err(anyhow::anyhow!(
            "Database feature required for client-group operations"
        ))
    }

    #[cfg(feature = "database")]
    async fn execute_json(
        &self,
        args: &serde_json::Value,
        ctx: &mut dsl_runtime::VerbExecutionContext,
        pool: &PgPool,
    ) -> Result<dsl_runtime::VerbExecutionOutcome> {
        use super::helpers::{
            json_extract_int_opt, json_extract_string, json_extract_string_opt, json_extract_uuid,
        };
        let group_id = json_extract_uuid(args, ctx, "group-id")?;
        let query = json_extract_string(args, "query")?;
        let persona = json_extract_string_opt(args, "persona");
        let limit = json_extract_int_opt(args, "limit").unwrap_or(10) as i32;
        let rows: Vec<(Uuid, String, String, f64, String)> = sqlx::query_as(
            r#"SELECT entity_id, entity_name, tag, confidence, match_type
            FROM "ob-poc".search_entity_tags($1, $2, $3, $4, FALSE)"#,
        )
        .bind(group_id)
        .bind(&query)
        .bind(&persona)
        .bind(limit)
        .fetch_all(pool)
        .await?;
        let items: Vec<serde_json::Value> = rows
            .into_iter()
            .map(|(eid, name, tag, conf, mt)| {
                json!({ "entity_id": eid, "entity_name": name, "matched_tag": tag,
                     "confidence": conf, "match_type": mt })
            })
            .collect();
        Ok(dsl_runtime::VerbExecutionOutcome::RecordSet(
            items,
        ))
    }

    fn is_migrated(&self) -> bool {
        true
    }
}

// =============================================================================
// DISCOVER-ENTITIES
// =============================================================================

#[register_custom_op]
pub struct ClientGroupDiscoverEntitiesOp;

#[async_trait]
impl CustomOperation for ClientGroupDiscoverEntitiesOp {
    fn domain(&self) -> &'static str {
        "client-group"
    }

    fn verb(&self) -> &'static str {
        "discover-entities"
    }

    fn rationale(&self) -> &'static str {
        "Discover entities that might belong to this client group (onboarding)"
    }

    #[cfg(feature = "database")]
    async fn execute(
        &self,
        verb_call: &VerbCall,
        ctx: &mut ExecutionContext,
        pool: &PgPool,
    ) -> Result<ExecutionResult> {
        let group_id = get_required_uuid(verb_call, "group-id", ctx)?;
        let search_terms = get_optional_string_array(verb_call, "search-terms");
        let jurisdiction = get_optional_string(verb_call, "jurisdiction");
        let auto_add = get_optional_bool(verb_call, "auto-add").unwrap_or(false);

        // Get group info for name matching
        let group = sqlx::query!(
            r#"SELECT canonical_name FROM "ob-poc".client_group WHERE id = $1"#,
            group_id
        )
        .fetch_one(pool)
        .await?;

        // Build search pattern from group name and additional terms
        let mut patterns = vec![format!("%{}%", group.canonical_name.to_lowercase())];
        if let Some(terms) = search_terms {
            for term in terms {
                patterns.push(format!("%{}%", term.to_lowercase()));
            }
        }

        // Find matching entities not already in group
        // Note: jurisdiction filter works via entity_limited_companies if needed in future
        let rows = sqlx::query!(
            r#"
            WITH already_members AS (
                SELECT entity_id FROM "ob-poc".client_group_entity WHERE group_id = $1
            )
            SELECT
                e.entity_id,
                e.name as "entity_name!",
                et.name as "entity_type?",
                'name_match'::TEXT as "match_reason!",
                EXISTS (SELECT 1 FROM already_members am WHERE am.entity_id = e.entity_id) as "already_member!"
            FROM "ob-poc".entities e
            LEFT JOIN "ob-poc".entity_types et ON et.entity_type_id = e.entity_type_id
            WHERE LOWER(e.name) LIKE ANY($2::TEXT[])
              AND e.deleted_at IS NULL
            ORDER BY e.name
            LIMIT 100
            "#,
            group_id,
            &patterns
        )
        .fetch_all(pool)
        .await?;

        // Log if jurisdiction filter was requested (for future implementation)
        if jurisdiction.is_some() {
            tracing::debug!(
                "Jurisdiction filter '{}' requested but not yet implemented for discover-entities",
                jurisdiction.as_deref().unwrap_or("")
            );
        }

        // Optionally auto-add as suspected
        if auto_add {
            for row in &rows {
                if !row.already_member {
                    sqlx::query!(
                        r#"
                        INSERT INTO "ob-poc".client_group_entity
                            (group_id, entity_id, membership_type, added_by, notes)
                        VALUES ($1, $2, 'suspected', 'discovery', 'Auto-added by discover-entities')
                        ON CONFLICT (group_id, entity_id) DO NOTHING
                        "#,
                        group_id,
                        row.entity_id
                    )
                    .execute(pool)
                    .await?;
                }
            }
        }

        let items: Vec<DiscoveryResult> = rows
            .into_iter()
            .map(|r| DiscoveryResult {
                entity_id: r.entity_id,
                entity_name: r.entity_name,
                entity_type: r.entity_type,
                match_reason: r.match_reason,
                already_member: r.already_member,
            })
            .collect();

        Ok(ExecutionResult::RecordSet(
            items
                .iter()
                .map(serde_json::to_value)
                .collect::<Result<Vec<_>, _>>()?,
        ))
    }

    #[cfg(not(feature = "database"))]
    async fn execute(
        &self,
        _verb_call: &VerbCall,
        _ctx: &mut ExecutionContext,
    ) -> Result<ExecutionResult> {
        Err(anyhow::anyhow!(
            "Database feature required for client-group operations"
        ))
    }

    #[cfg(feature = "database")]
    async fn execute_json(
        &self,
        args: &serde_json::Value,
        ctx: &mut dsl_runtime::VerbExecutionContext,
        pool: &PgPool,
    ) -> Result<dsl_runtime::VerbExecutionOutcome> {
        use super::helpers::{
            json_extract_bool_opt, json_extract_string_list_opt, json_extract_string_opt,
            json_extract_uuid,
        };
        let group_id = json_extract_uuid(args, ctx, "group-id")?;
        let search_terms = json_extract_string_list_opt(args, "search-terms");
        let jurisdiction = json_extract_string_opt(args, "jurisdiction");
        let auto_add = json_extract_bool_opt(args, "auto-add").unwrap_or(false);
        let group_name: String =
            sqlx::query_scalar(r#"SELECT canonical_name FROM "ob-poc".client_group WHERE id = $1"#)
                .bind(group_id)
                .fetch_one(pool)
                .await?;
        let mut patterns = vec![format!("%{}%", group_name.to_lowercase())];
        if let Some(terms) = search_terms {
            for term in terms {
                patterns.push(format!("%{}%", term.to_lowercase()));
            }
        }
        if jurisdiction.is_some() {
            tracing::debug!(
                "Jurisdiction filter requested but not yet implemented for discover-entities"
            );
        }
        let rows: Vec<(Uuid, String, Option<String>, String, bool)> = sqlx::query_as(
            r#"WITH already_members AS (
                SELECT entity_id FROM "ob-poc".client_group_entity WHERE group_id = $1
            )
            SELECT
                e.entity_id, e.name,
                et.name as entity_type,
                'name_match'::TEXT,
                EXISTS (SELECT 1 FROM already_members am WHERE am.entity_id = e.entity_id)
            FROM "ob-poc".entities e
            LEFT JOIN "ob-poc".entity_types et ON et.entity_type_id = e.entity_type_id
            WHERE LOWER(e.name) LIKE ANY($2::TEXT[])
              AND e.deleted_at IS NULL
            ORDER BY e.name LIMIT 100"#,
        )
        .bind(group_id)
        .bind(&patterns)
        .fetch_all(pool)
        .await?;
        if auto_add {
            for (eid, _, _, _, already) in &rows {
                if !already {
                    sqlx::query(
                        r#"INSERT INTO "ob-poc".client_group_entity
                            (group_id, entity_id, membership_type, added_by, notes)
                        VALUES ($1, $2, 'suspected', 'discovery', 'Auto-added by discover-entities')
                        ON CONFLICT (group_id, entity_id) DO NOTHING"#,
                    )
                    .bind(group_id)
                    .bind(eid)
                    .execute(pool)
                    .await?;
                }
            }
        }
        let items: Vec<serde_json::Value> = rows
            .into_iter()
            .map(|(eid, name, etype, reason, already)| {
                json!({ "entity_id": eid, "entity_name": name, "entity_type": etype,
                     "match_reason": reason, "already_member": already })
            })
            .collect();
        Ok(dsl_runtime::VerbExecutionOutcome::RecordSet(
            items,
        ))
    }

    fn is_migrated(&self) -> bool {
        true
    }
}

// =============================================================================
// CONFIRM-ENTITY
// =============================================================================

#[register_custom_op]
pub struct ClientGroupConfirmEntityOp;

#[async_trait]
impl CustomOperation for ClientGroupConfirmEntityOp {
    fn domain(&self) -> &'static str {
        "client-group"
    }

    fn verb(&self) -> &'static str {
        "confirm-entity"
    }

    fn rationale(&self) -> &'static str {
        "Confirm a suspected entity as belonging to the client group"
    }

    #[cfg(feature = "database")]
    async fn execute(
        &self,
        verb_call: &VerbCall,
        ctx: &mut ExecutionContext,
        pool: &PgPool,
    ) -> Result<ExecutionResult> {
        let group_id = get_required_uuid(verb_call, "group-id", ctx)?;
        let entity_id = get_required_uuid(verb_call, "entity-id", ctx)?;
        let tags = get_optional_string_array(verb_call, "tags");

        // Update membership to confirmed
        let affected = sqlx::query!(
            r#"
            UPDATE "ob-poc".client_group_entity
            SET membership_type = 'confirmed',
                added_by = 'user_confirmed',
                updated_at = now()
            WHERE group_id = $1 AND entity_id = $2
            "#,
            group_id,
            entity_id
        )
        .execute(pool)
        .await?
        .rows_affected();

        // If no existing membership, create one
        if affected == 0 {
            sqlx::query!(
                r#"
                INSERT INTO "ob-poc".client_group_entity
                    (group_id, entity_id, membership_type, added_by)
                VALUES ($1, $2, 'confirmed', 'user_confirmed')
                "#,
                group_id,
                entity_id
            )
            .execute(pool)
            .await?;
        }

        // Add initial tags if provided
        if let Some(tag_list) = tags {
            for tag in tag_list {
                let tag_norm: String =
                    sqlx::query_scalar!(r#"SELECT "ob-poc".normalize_tag($1) as "tag_norm!""#, tag)
                        .fetch_one(pool)
                        .await?;

                sqlx::query!(
                    r#"
                    INSERT INTO "ob-poc".client_group_entity_tag
                        (group_id, entity_id, tag, tag_norm, source, confidence)
                    VALUES ($1, $2, $3, $4, 'user_confirmed', 1.0)
                    ON CONFLICT DO NOTHING
                    "#,
                    group_id,
                    entity_id,
                    tag,
                    tag_norm
                )
                .execute(pool)
                .await?;
            }
        }

        Ok(ExecutionResult::Record(json!({
            "confirmed": true,
            "entity_id": entity_id
        })))
    }

    #[cfg(not(feature = "database"))]
    async fn execute(
        &self,
        _verb_call: &VerbCall,
        _ctx: &mut ExecutionContext,
    ) -> Result<ExecutionResult> {
        Err(anyhow::anyhow!(
            "Database feature required for client-group operations"
        ))
    }

    #[cfg(feature = "database")]
    async fn execute_json(
        &self,
        args: &serde_json::Value,
        ctx: &mut dsl_runtime::VerbExecutionContext,
        pool: &PgPool,
    ) -> Result<dsl_runtime::VerbExecutionOutcome> {
        use super::helpers::{json_extract_string_list_opt, json_extract_uuid};
        let group_id = json_extract_uuid(args, ctx, "group-id")?;
        let entity_id = json_extract_uuid(args, ctx, "entity-id")?;
        let tags = json_extract_string_list_opt(args, "tags");
        let affected = sqlx::query(
            r#"UPDATE "ob-poc".client_group_entity
            SET membership_type = 'confirmed', added_by = 'user_confirmed', updated_at = now()
            WHERE group_id = $1 AND entity_id = $2"#,
        )
        .bind(group_id)
        .bind(entity_id)
        .execute(pool)
        .await?
        .rows_affected();
        if affected == 0 {
            sqlx::query(
                r#"INSERT INTO "ob-poc".client_group_entity
                    (group_id, entity_id, membership_type, added_by)
                VALUES ($1, $2, 'confirmed', 'user_confirmed')"#,
            )
            .bind(group_id)
            .bind(entity_id)
            .execute(pool)
            .await?;
        }
        if let Some(tag_list) = tags {
            for tag in tag_list {
                let tag_norm: String = sqlx::query_scalar(r#"SELECT "ob-poc".normalize_tag($1)"#)
                    .bind(&tag)
                    .fetch_one(pool)
                    .await?;
                sqlx::query(
                    r#"INSERT INTO "ob-poc".client_group_entity_tag
                        (group_id, entity_id, tag, tag_norm, source, confidence)
                    VALUES ($1, $2, $3, $4, 'user_confirmed', 1.0)
                    ON CONFLICT DO NOTHING"#,
                )
                .bind(group_id)
                .bind(entity_id)
                .bind(&tag)
                .bind(&tag_norm)
                .execute(pool)
                .await?;
            }
        }
        Ok(dsl_runtime::VerbExecutionOutcome::Record(
            json!({ "confirmed": true, "entity_id": entity_id }),
        ))
    }

    fn is_migrated(&self) -> bool {
        true
    }
}

// =============================================================================
// REJECT-ENTITY
// =============================================================================

#[register_custom_op]
pub struct ClientGroupRejectEntityOp;

#[async_trait]
impl CustomOperation for ClientGroupRejectEntityOp {
    fn domain(&self) -> &'static str {
        "client-group"
    }

    fn verb(&self) -> &'static str {
        "reject-entity"
    }

    fn rationale(&self) -> &'static str {
        "Reject a suspected entity as not belonging to the client group"
    }

    #[cfg(feature = "database")]
    async fn execute(
        &self,
        verb_call: &VerbCall,
        ctx: &mut ExecutionContext,
        pool: &PgPool,
    ) -> Result<ExecutionResult> {
        let group_id = get_required_uuid(verb_call, "group-id", ctx)?;
        let entity_id = get_required_uuid(verb_call, "entity-id", ctx)?;
        let reason = get_optional_string(verb_call, "reason");

        // Remove membership (could also mark as rejected with reason)
        let affected = sqlx::query!(
            r#"
            DELETE FROM "ob-poc".client_group_entity
            WHERE group_id = $1 AND entity_id = $2
            "#,
            group_id,
            entity_id
        )
        .execute(pool)
        .await?
        .rows_affected();

        // Also remove any tags
        sqlx::query!(
            r#"
            DELETE FROM "ob-poc".client_group_entity_tag
            WHERE group_id = $1 AND entity_id = $2
            "#,
            group_id,
            entity_id
        )
        .execute(pool)
        .await?;

        Ok(ExecutionResult::Record(json!({
            "rejected": affected > 0,
            "reason": reason
        })))
    }

    #[cfg(not(feature = "database"))]
    async fn execute(
        &self,
        _verb_call: &VerbCall,
        _ctx: &mut ExecutionContext,
    ) -> Result<ExecutionResult> {
        Err(anyhow::anyhow!(
            "Database feature required for client-group operations"
        ))
    }

    #[cfg(feature = "database")]
    async fn execute_json(
        &self,
        args: &serde_json::Value,
        ctx: &mut dsl_runtime::VerbExecutionContext,
        pool: &PgPool,
    ) -> Result<dsl_runtime::VerbExecutionOutcome> {
        use super::helpers::{json_extract_string_opt, json_extract_uuid};
        let group_id = json_extract_uuid(args, ctx, "group-id")?;
        let entity_id = json_extract_uuid(args, ctx, "entity-id")?;
        let reason = json_extract_string_opt(args, "reason");
        let affected = sqlx::query(
            r#"DELETE FROM "ob-poc".client_group_entity WHERE group_id = $1 AND entity_id = $2"#,
        )
        .bind(group_id)
        .bind(entity_id)
        .execute(pool)
        .await?
        .rows_affected();
        sqlx::query(
            r#"DELETE FROM "ob-poc".client_group_entity_tag WHERE group_id = $1 AND entity_id = $2"#,
        ).bind(group_id).bind(entity_id).execute(pool).await?;
        Ok(dsl_runtime::VerbExecutionOutcome::Record(
            json!({ "rejected": affected > 0, "reason": reason }),
        ))
    }

    fn is_migrated(&self) -> bool {
        true
    }
}

// =============================================================================
// ROLE MANAGEMENT RESULT TYPES
// =============================================================================

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct RoleAssignmentResult {
    pub id: Uuid,
    pub entity_id: Uuid,
    pub entity_name: String,
    pub role_id: Uuid,
    pub role_name: String,
    pub target_entity_id: Option<Uuid>,
    pub target_entity_name: Option<String>,
    pub effective_from: Option<String>,
    pub effective_to: Option<String>,
    pub source: String,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct PartyResult {
    pub entity_id: Uuid,
    pub entity_name: String,
    pub membership_type: String,
    pub roles: Vec<String>,
    pub role_categories: Vec<String>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct RelationshipResult {
    pub id: Uuid,
    pub parent_entity_id: Uuid,
    pub parent_name: String,
    pub child_entity_id: Uuid,
    pub child_name: String,
    pub relationship_kind: String,
    pub canonical_ownership_pct: Option<f64>,
    pub source_count: i64,
    pub review_status: String,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct UnverifiedAllegationResult {
    pub relationship_id: Uuid,
    pub parent_name: String,
    pub child_name: String,
    pub alleged_pct: Option<f64>,
    pub source_document_ref: Option<String>,
    pub source_document_date: Option<String>,
    pub verification_count: i64,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct DiscrepancyResult {
    pub relationship_id: Uuid,
    pub parent_name: String,
    pub child_name: String,
    pub sources: Vec<String>,
    pub ownership_values: Vec<f64>,
    pub ownership_spread: f64,
    pub alleged_pct: Option<f64>,
    pub verified_pct: Option<f64>,
}

// =============================================================================
// ASSIGN-ROLE
// =============================================================================

#[register_custom_op]
pub struct ClientGroupAssignRoleOp;

#[async_trait]
impl CustomOperation for ClientGroupAssignRoleOp {
    fn domain(&self) -> &'static str {
        "client-group"
    }

    fn verb(&self) -> &'static str {
        "assign-role"
    }

    fn rationale(&self) -> &'static str {
        "Assign a role to an entity within the client group context"
    }

    #[cfg(feature = "database")]
    async fn execute(
        &self,
        verb_call: &VerbCall,
        ctx: &mut ExecutionContext,
        pool: &PgPool,
    ) -> Result<ExecutionResult> {
        let group_id = get_required_uuid(verb_call, "group-id", ctx)?;
        let entity_id = get_required_uuid(verb_call, "entity-id", ctx)?;
        let role_id = get_required_uuid(verb_call, "role-id", ctx)?;
        let target_entity_id = get_optional_uuid(verb_call, "target-entity-id", ctx);
        let effective_from = get_optional_string(verb_call, "effective-from");
        let source =
            get_optional_string(verb_call, "source").unwrap_or_else(|| "manual".to_string());

        // Get the cge_id for the entity membership
        let cge = sqlx::query!(
            r#"
            SELECT id FROM "ob-poc".client_group_entity
            WHERE group_id = $1 AND entity_id = $2
            "#,
            group_id,
            entity_id
        )
        .fetch_optional(pool)
        .await?;

        let cge_id = match cge {
            Some(row) => row.id,
            None => {
                // Auto-add entity to group if not present
                let result = sqlx::query!(
                    r#"
                    INSERT INTO "ob-poc".client_group_entity
                        (group_id, entity_id, membership_type, added_by)
                    VALUES ($1, $2, 'in_group', 'role_assignment')
                    RETURNING id
                    "#,
                    group_id,
                    entity_id
                )
                .fetch_one(pool)
                .await?;
                result.id
            }
        };

        // Parse effective_from date
        let eff_from: Option<chrono::NaiveDate> = effective_from
            .as_ref()
            .and_then(|s| chrono::NaiveDate::parse_from_str(s, "%Y-%m-%d").ok());

        // Insert role assignment
        let result = sqlx::query!(
            r#"
            INSERT INTO "ob-poc".client_group_entity_roles
                (cge_id, role_id, target_entity_id, effective_from, assigned_by)
            VALUES ($1, $2, $3, $4, $5)
            ON CONFLICT (cge_id, role_id, COALESCE(target_entity_id, '00000000-0000-0000-0000-000000000000'))
            DO UPDATE SET
                effective_from = COALESCE(EXCLUDED.effective_from, client_group_entity_roles.effective_from),
                assigned_by = EXCLUDED.assigned_by,
                updated_at = NOW()
            RETURNING id
            "#,
            cge_id,
            role_id,
            target_entity_id,
            eff_from,
            source
        )
        .fetch_one(pool)
        .await?;

        Ok(ExecutionResult::Uuid(result.id))
    }

    #[cfg(not(feature = "database"))]
    async fn execute(
        &self,
        _verb_call: &VerbCall,
        _ctx: &mut ExecutionContext,
    ) -> Result<ExecutionResult> {
        Err(anyhow::anyhow!(
            "Database feature required for client-group operations"
        ))
    }

    #[cfg(feature = "database")]
    async fn execute_json(
        &self,
        args: &serde_json::Value,
        ctx: &mut dsl_runtime::VerbExecutionContext,
        pool: &PgPool,
    ) -> Result<dsl_runtime::VerbExecutionOutcome> {
        use super::helpers::{json_extract_string_opt, json_extract_uuid, json_extract_uuid_opt};
        let group_id = json_extract_uuid(args, ctx, "group-id")?;
        let entity_id = json_extract_uuid(args, ctx, "entity-id")?;
        let role_id = json_extract_uuid(args, ctx, "role-id")?;
        let target_entity_id = json_extract_uuid_opt(args, ctx, "target-entity-id");
        let effective_from = json_extract_string_opt(args, "effective-from");
        let source =
            json_extract_string_opt(args, "source").unwrap_or_else(|| "manual".to_string());
        let cge_id: Uuid = match sqlx::query_scalar::<_, Uuid>(
            r#"SELECT id FROM "ob-poc".client_group_entity WHERE group_id = $1 AND entity_id = $2"#,
        )
        .bind(group_id)
        .bind(entity_id)
        .fetch_optional(pool)
        .await?
        {
            Some(id) => id,
            None => {
                sqlx::query_scalar(
                    r#"INSERT INTO "ob-poc".client_group_entity
                    (group_id, entity_id, membership_type, added_by)
                VALUES ($1, $2, 'in_group', 'role_assignment') RETURNING id"#,
                )
                .bind(group_id)
                .bind(entity_id)
                .fetch_one(pool)
                .await?
            }
        };
        let eff_from: Option<chrono::NaiveDate> = effective_from
            .as_ref()
            .and_then(|s| chrono::NaiveDate::parse_from_str(s, "%Y-%m-%d").ok());
        let id: Uuid = sqlx::query_scalar(
            r#"INSERT INTO "ob-poc".client_group_entity_roles
                (cge_id, role_id, target_entity_id, effective_from, assigned_by)
            VALUES ($1, $2, $3, $4, $5)
            ON CONFLICT (cge_id, role_id, COALESCE(target_entity_id, '00000000-0000-0000-0000-000000000000'))
            DO UPDATE SET
                effective_from = COALESCE(EXCLUDED.effective_from, client_group_entity_roles.effective_from),
                assigned_by = EXCLUDED.assigned_by, updated_at = NOW()
            RETURNING id"#,
        ).bind(cge_id).bind(role_id).bind(target_entity_id).bind(eff_from).bind(&source)
        .fetch_one(pool).await?;
        Ok(dsl_runtime::VerbExecutionOutcome::Uuid(id))
    }

    fn is_migrated(&self) -> bool {
        true
    }
}

// =============================================================================
// REMOVE-ROLE
// =============================================================================

#[register_custom_op]
pub struct ClientGroupRemoveRoleOp;

#[async_trait]
impl CustomOperation for ClientGroupRemoveRoleOp {
    fn domain(&self) -> &'static str {
        "client-group"
    }

    fn verb(&self) -> &'static str {
        "remove-role"
    }

    fn rationale(&self) -> &'static str {
        "Remove a role assignment from an entity"
    }

    #[cfg(feature = "database")]
    async fn execute(
        &self,
        verb_call: &VerbCall,
        ctx: &mut ExecutionContext,
        pool: &PgPool,
    ) -> Result<ExecutionResult> {
        let role_assignment_id = get_required_uuid(verb_call, "role-assignment-id", ctx)?;

        // Soft delete by setting effective_to
        let affected = sqlx::query!(
            r#"
            UPDATE "ob-poc".client_group_entity_roles
            SET effective_to = CURRENT_DATE, updated_at = NOW()
            WHERE id = $1 AND effective_to IS NULL
            "#,
            role_assignment_id
        )
        .execute(pool)
        .await?
        .rows_affected();

        Ok(ExecutionResult::Affected(affected))
    }

    #[cfg(not(feature = "database"))]
    async fn execute(
        &self,
        _verb_call: &VerbCall,
        _ctx: &mut ExecutionContext,
    ) -> Result<ExecutionResult> {
        Err(anyhow::anyhow!(
            "Database feature required for client-group operations"
        ))
    }

    #[cfg(feature = "database")]
    async fn execute_json(
        &self,
        args: &serde_json::Value,
        ctx: &mut dsl_runtime::VerbExecutionContext,
        pool: &PgPool,
    ) -> Result<dsl_runtime::VerbExecutionOutcome> {
        use super::helpers::json_extract_uuid;
        let role_assignment_id = json_extract_uuid(args, ctx, "role-assignment-id")?;
        let affected = sqlx::query(
            r#"UPDATE "ob-poc".client_group_entity_roles
            SET effective_to = CURRENT_DATE, updated_at = NOW()
            WHERE id = $1 AND effective_to IS NULL"#,
        )
        .bind(role_assignment_id)
        .execute(pool)
        .await?
        .rows_affected();
        Ok(dsl_runtime::VerbExecutionOutcome::Affected(
            affected,
        ))
    }

    fn is_migrated(&self) -> bool {
        true
    }
}

// =============================================================================
// LIST-ROLES
// =============================================================================

#[register_custom_op]
pub struct ClientGroupListRolesOp;

#[async_trait]
impl CustomOperation for ClientGroupListRolesOp {
    fn domain(&self) -> &'static str {
        "client-group"
    }

    fn verb(&self) -> &'static str {
        "list-roles"
    }

    fn rationale(&self) -> &'static str {
        "List role assignments for entities in a client group"
    }

    #[cfg(feature = "database")]
    async fn execute(
        &self,
        verb_call: &VerbCall,
        ctx: &mut ExecutionContext,
        pool: &PgPool,
    ) -> Result<ExecutionResult> {
        let group_id = get_required_uuid(verb_call, "group-id", ctx)?;
        let entity_id = get_optional_uuid(verb_call, "entity-id", ctx);
        let role_id = get_optional_uuid(verb_call, "role-id", ctx);

        let rows = sqlx::query!(
            r#"
            SELECT
                cer.id,
                cge.entity_id,
                e.name as "entity_name!",
                cer.role_id,
                r.name as "role_name!",
                cer.target_entity_id,
                te.name as target_entity_name,
                cer.effective_from,
                cer.effective_to,
                cer.assigned_by as "source!"
            FROM "ob-poc".client_group_entity_roles cer
            JOIN "ob-poc".client_group_entity cge ON cge.id = cer.cge_id
            JOIN "ob-poc".entities e ON e.entity_id = cge.entity_id
            JOIN "ob-poc".roles r ON r.role_id = cer.role_id
            LEFT JOIN "ob-poc".entities te ON te.entity_id = cer.target_entity_id
            WHERE cge.group_id = $1
              AND e.deleted_at IS NULL
              AND (te.entity_id IS NULL OR te.deleted_at IS NULL)
              AND ($2::UUID IS NULL OR cge.entity_id = $2)
              AND ($3::UUID IS NULL OR cer.role_id = $3)
              AND (cer.effective_to IS NULL OR cer.effective_to > CURRENT_DATE)
            ORDER BY e.name, r.name
            "#,
            group_id,
            entity_id,
            role_id
        )
        .fetch_all(pool)
        .await?;

        let items: Vec<RoleAssignmentResult> = rows
            .into_iter()
            .map(|r| RoleAssignmentResult {
                id: r.id,
                entity_id: r.entity_id,
                entity_name: r.entity_name,
                role_id: r.role_id,
                role_name: r.role_name,
                target_entity_id: r.target_entity_id,
                target_entity_name: Some(r.target_entity_name).filter(|s| !s.is_empty()),
                effective_from: r.effective_from.map(|d| d.to_string()),
                effective_to: r.effective_to.map(|d| d.to_string()),
                source: r.source,
            })
            .collect();

        Ok(ExecutionResult::RecordSet(
            items
                .iter()
                .map(serde_json::to_value)
                .collect::<Result<Vec<_>, _>>()?,
        ))
    }

    #[cfg(not(feature = "database"))]
    async fn execute(
        &self,
        _verb_call: &VerbCall,
        _ctx: &mut ExecutionContext,
    ) -> Result<ExecutionResult> {
        Err(anyhow::anyhow!(
            "Database feature required for client-group operations"
        ))
    }

    #[cfg(feature = "database")]
    async fn execute_json(
        &self,
        args: &serde_json::Value,
        ctx: &mut dsl_runtime::VerbExecutionContext,
        pool: &PgPool,
    ) -> Result<dsl_runtime::VerbExecutionOutcome> {
        use super::helpers::{json_extract_uuid, json_extract_uuid_opt};
        let group_id = json_extract_uuid(args, ctx, "group-id")?;
        let entity_id = json_extract_uuid_opt(args, ctx, "entity-id");
        let role_id = json_extract_uuid_opt(args, ctx, "role-id");
        let rows = sqlx::query(
            r#"SELECT
                cer.id, cge.entity_id, e.name as entity_name,
                cer.role_id, r.name as role_name,
                cer.target_entity_id, te.name as target_entity_name,
                cer.effective_from, cer.effective_to, cer.assigned_by as source
            FROM "ob-poc".client_group_entity_roles cer
            JOIN "ob-poc".client_group_entity cge ON cge.id = cer.cge_id
            JOIN "ob-poc".entities e ON e.entity_id = cge.entity_id
            JOIN "ob-poc".roles r ON r.role_id = cer.role_id
            LEFT JOIN "ob-poc".entities te ON te.entity_id = cer.target_entity_id
            WHERE cge.group_id = $1
              AND e.deleted_at IS NULL
              AND (te.entity_id IS NULL OR te.deleted_at IS NULL)
              AND ($2::UUID IS NULL OR cge.entity_id = $2)
              AND ($3::UUID IS NULL OR cer.role_id = $3)
              AND (cer.effective_to IS NULL OR cer.effective_to > CURRENT_DATE)
            ORDER BY e.name, r.name"#,
        )
        .bind(group_id)
        .bind(entity_id)
        .bind(role_id)
        .fetch_all(pool)
        .await?;
        use sqlx::Row;
        let items: Vec<serde_json::Value> = rows.iter().map(|r| {
            json!({
                "id": r.get::<Uuid, _>("id"),
                "entity_id": r.get::<Uuid, _>("entity_id"),
                "entity_name": r.get::<String, _>("entity_name"),
                "role_id": r.get::<Uuid, _>("role_id"),
                "role_name": r.get::<String, _>("role_name"),
                "target_entity_id": r.get::<Option<Uuid>, _>("target_entity_id"),
                "target_entity_name": r.get::<Option<String>, _>("target_entity_name"),
                "effective_from": r.get::<Option<chrono::NaiveDate>, _>("effective_from").map(|d| d.to_string()),
                "effective_to": r.get::<Option<chrono::NaiveDate>, _>("effective_to").map(|d| d.to_string()),
                "source": r.get::<String, _>("source"),
            })
        }).collect();
        Ok(dsl_runtime::VerbExecutionOutcome::RecordSet(
            items,
        ))
    }

    fn is_migrated(&self) -> bool {
        true
    }
}

// =============================================================================
// LIST-PARTIES
// =============================================================================

#[register_custom_op]
pub struct ClientGroupPartiesOp;

#[async_trait]
impl CustomOperation for ClientGroupPartiesOp {
    fn domain(&self) -> &'static str {
        "client-group"
    }

    fn verb(&self) -> &'static str {
        "list-parties"
    }

    fn rationale(&self) -> &'static str {
        "List all parties (entities with roles) in a client group"
    }

    #[cfg(feature = "database")]
    async fn execute(
        &self,
        verb_call: &VerbCall,
        ctx: &mut ExecutionContext,
        pool: &PgPool,
    ) -> Result<ExecutionResult> {
        let group_id = get_required_uuid(verb_call, "group-id", ctx)?;
        let role_category = get_optional_string(verb_call, "role-category");
        let include_external = get_optional_bool(verb_call, "include-external").unwrap_or(true);

        let rows = sqlx::query!(
            r#"
            SELECT
                cge.entity_id,
                e.name as "entity_name!",
                cge.membership_type as "membership_type!",
                COALESCE(
                    array_agg(DISTINCT r.name ORDER BY r.name) FILTER (WHERE r.name IS NOT NULL),
                    ARRAY[]::TEXT[]
                ) as "roles!",
                COALESCE(
                    array_agg(DISTINCT r.role_category ORDER BY r.role_category) FILTER (WHERE r.role_category IS NOT NULL),
                    ARRAY[]::TEXT[]
                ) as "role_categories!"
            FROM "ob-poc".client_group_entity cge
            JOIN "ob-poc".entities e ON e.entity_id = cge.entity_id
            LEFT JOIN "ob-poc".client_group_entity_roles cer ON cer.cge_id = cge.id
                AND (cer.effective_to IS NULL OR cer.effective_to > CURRENT_DATE)
            LEFT JOIN "ob-poc".roles r ON r.role_id = cer.role_id
            WHERE cge.group_id = $1
              AND e.deleted_at IS NULL
              AND cge.membership_type != 'historical'
              AND ($2 OR cge.membership_type = 'in_group')
              AND ($3::TEXT IS NULL OR r.role_category = $3)
            GROUP BY cge.entity_id, e.name, cge.membership_type
            HAVING COUNT(cer.id) > 0 OR $3::TEXT IS NULL
            ORDER BY
                CASE cge.membership_type WHEN 'in_group' THEN 0 ELSE 1 END,
                e.name
            "#,
            group_id,
            include_external,
            role_category
        )
        .fetch_all(pool)
        .await?;

        let items: Vec<PartyResult> = rows
            .into_iter()
            .map(|r| PartyResult {
                entity_id: r.entity_id,
                entity_name: r.entity_name,
                membership_type: r.membership_type,
                roles: r.roles,
                role_categories: r.role_categories,
            })
            .collect();

        Ok(ExecutionResult::RecordSet(
            items
                .iter()
                .map(serde_json::to_value)
                .collect::<Result<Vec<_>, _>>()?,
        ))
    }

    #[cfg(not(feature = "database"))]
    async fn execute(
        &self,
        _verb_call: &VerbCall,
        _ctx: &mut ExecutionContext,
    ) -> Result<ExecutionResult> {
        Err(anyhow::anyhow!(
            "Database feature required for client-group operations"
        ))
    }

    #[cfg(feature = "database")]
    async fn execute_json(
        &self,
        args: &serde_json::Value,
        ctx: &mut dsl_runtime::VerbExecutionContext,
        pool: &PgPool,
    ) -> Result<dsl_runtime::VerbExecutionOutcome> {
        use super::helpers::{json_extract_bool_opt, json_extract_string_opt, json_extract_uuid};
        let group_id = json_extract_uuid(args, ctx, "group-id")?;
        let role_category = json_extract_string_opt(args, "role-category");
        let include_external = json_extract_bool_opt(args, "include-external").unwrap_or(true);
        let rows = sqlx::query(
            r#"SELECT
                cge.entity_id, e.name as entity_name,
                cge.membership_type,
                COALESCE(
                    array_agg(DISTINCT r.name ORDER BY r.name) FILTER (WHERE r.name IS NOT NULL),
                    ARRAY[]::TEXT[]
                ) as roles,
                COALESCE(
                    array_agg(DISTINCT r.role_category ORDER BY r.role_category) FILTER (WHERE r.role_category IS NOT NULL),
                    ARRAY[]::TEXT[]
                ) as role_categories
            FROM "ob-poc".client_group_entity cge
            JOIN "ob-poc".entities e ON e.entity_id = cge.entity_id
            LEFT JOIN "ob-poc".client_group_entity_roles cer ON cer.cge_id = cge.id
                AND (cer.effective_to IS NULL OR cer.effective_to > CURRENT_DATE)
            LEFT JOIN "ob-poc".roles r ON r.role_id = cer.role_id
            WHERE cge.group_id = $1
              AND e.deleted_at IS NULL
              AND cge.membership_type != 'historical'
              AND ($2 OR cge.membership_type = 'in_group')
              AND ($3::TEXT IS NULL OR r.role_category = $3)
            GROUP BY cge.entity_id, e.name, cge.membership_type
            HAVING COUNT(cer.id) > 0 OR $3::TEXT IS NULL
            ORDER BY
                CASE cge.membership_type WHEN 'in_group' THEN 0 ELSE 1 END, e.name"#,
        ).bind(group_id).bind(include_external).bind(&role_category)
        .fetch_all(pool).await?;
        use sqlx::Row;
        let items: Vec<serde_json::Value> = rows
            .iter()
            .map(|r| {
                json!({
                    "entity_id": r.get::<Uuid, _>("entity_id"),
                    "entity_name": r.get::<String, _>("entity_name"),
                    "membership_type": r.get::<String, _>("membership_type"),
                    "roles": r.get::<Vec<String>, _>("roles"),
                    "role_categories": r.get::<Vec<String>, _>("role_categories"),
                })
            })
            .collect();
        Ok(dsl_runtime::VerbExecutionOutcome::RecordSet(
            items,
        ))
    }

    fn is_migrated(&self) -> bool {
        true
    }
}

// =============================================================================
// ADD-RELATIONSHIP
// =============================================================================

#[register_custom_op]
pub struct ClientGroupAddRelationshipOp;

#[async_trait]
impl CustomOperation for ClientGroupAddRelationshipOp {
    fn domain(&self) -> &'static str {
        "client-group"
    }

    fn verb(&self) -> &'static str {
        "add-relationship"
    }

    fn rationale(&self) -> &'static str {
        "Add a provisional ownership/control edge between entities"
    }

    #[cfg(feature = "database")]
    async fn execute(
        &self,
        verb_call: &VerbCall,
        ctx: &mut ExecutionContext,
        pool: &PgPool,
    ) -> Result<ExecutionResult> {
        let group_id = get_required_uuid(verb_call, "group-id", ctx)?;
        let parent_entity_id = get_required_uuid(verb_call, "parent-entity-id", ctx)?;
        let child_entity_id = get_required_uuid(verb_call, "child-entity-id", ctx)?;
        let relationship_kind = get_optional_string(verb_call, "relationship-kind")
            .unwrap_or_else(|| "ownership".to_string());
        let effective_from = get_optional_string(verb_call, "effective-from");

        let eff_from: Option<chrono::NaiveDate> = effective_from
            .as_ref()
            .and_then(|s| chrono::NaiveDate::parse_from_str(s, "%Y-%m-%d").ok());

        let result = sqlx::query!(
            r#"
            INSERT INTO "ob-poc".client_group_relationship
                (group_id, parent_entity_id, child_entity_id, relationship_kind, effective_from)
            VALUES ($1, $2, $3, $4, $5)
            ON CONFLICT (group_id, parent_entity_id, child_entity_id, relationship_kind)
            DO UPDATE SET
                effective_from = COALESCE(EXCLUDED.effective_from, client_group_relationship.effective_from),
                updated_at = NOW()
            RETURNING id
            "#,
            group_id,
            parent_entity_id,
            child_entity_id,
            relationship_kind,
            eff_from
        )
        .fetch_one(pool)
        .await?;

        Ok(ExecutionResult::Uuid(result.id))
    }

    #[cfg(not(feature = "database"))]
    async fn execute(
        &self,
        _verb_call: &VerbCall,
        _ctx: &mut ExecutionContext,
    ) -> Result<ExecutionResult> {
        Err(anyhow::anyhow!(
            "Database feature required for client-group operations"
        ))
    }

    #[cfg(feature = "database")]
    async fn execute_json(
        &self,
        args: &serde_json::Value,
        ctx: &mut dsl_runtime::VerbExecutionContext,
        pool: &PgPool,
    ) -> Result<dsl_runtime::VerbExecutionOutcome> {
        use super::helpers::{json_extract_string_opt, json_extract_uuid};
        let group_id = json_extract_uuid(args, ctx, "group-id")?;
        let parent_entity_id = json_extract_uuid(args, ctx, "parent-entity-id")?;
        let child_entity_id = json_extract_uuid(args, ctx, "child-entity-id")?;
        let relationship_kind = json_extract_string_opt(args, "relationship-kind")
            .unwrap_or_else(|| "ownership".to_string());
        let effective_from = json_extract_string_opt(args, "effective-from");
        let eff_from: Option<chrono::NaiveDate> = effective_from
            .as_ref()
            .and_then(|s| chrono::NaiveDate::parse_from_str(s, "%Y-%m-%d").ok());
        let id: Uuid = sqlx::query_scalar(
            r#"INSERT INTO "ob-poc".client_group_relationship
                (group_id, parent_entity_id, child_entity_id, relationship_kind, effective_from)
            VALUES ($1, $2, $3, $4, $5)
            ON CONFLICT (group_id, parent_entity_id, child_entity_id, relationship_kind)
            DO UPDATE SET
                effective_from = COALESCE(EXCLUDED.effective_from, client_group_relationship.effective_from),
                updated_at = NOW()
            RETURNING id"#,
        ).bind(group_id).bind(parent_entity_id).bind(child_entity_id).bind(&relationship_kind).bind(eff_from)
        .fetch_one(pool).await?;
        Ok(dsl_runtime::VerbExecutionOutcome::Uuid(id))
    }

    fn is_migrated(&self) -> bool {
        true
    }
}

// =============================================================================
// LIST-RELATIONSHIPS
// =============================================================================

#[register_custom_op]
pub struct ClientGroupListRelationshipsOp;

#[async_trait]
impl CustomOperation for ClientGroupListRelationshipsOp {
    fn domain(&self) -> &'static str {
        "client-group"
    }

    fn verb(&self) -> &'static str {
        "list-relationships"
    }

    fn rationale(&self) -> &'static str {
        "List ownership/control relationships in a client group"
    }

    #[cfg(feature = "database")]
    async fn execute(
        &self,
        verb_call: &VerbCall,
        ctx: &mut ExecutionContext,
        pool: &PgPool,
    ) -> Result<ExecutionResult> {
        let group_id = get_required_uuid(verb_call, "group-id", ctx)?;
        let entity_id = get_optional_uuid(verb_call, "entity-id", ctx);
        let relationship_kind = get_optional_string(verb_call, "relationship-kind");

        let rows = sqlx::query!(
            r#"
            SELECT
                r.id,
                r.parent_entity_id,
                pe.name as "parent_name!",
                r.child_entity_id,
                ce.name as "child_name!",
                r.relationship_kind as "relationship_kind!",
                r.review_status as "review_status!",
                (SELECT s.ownership_pct FROM "ob-poc".client_group_relationship_sources s
                 WHERE s.relationship_id = r.id
                 ORDER BY s.is_canonical DESC, s.confidence_score DESC NULLS LAST
                 LIMIT 1) as canonical_ownership_pct,
                (SELECT COUNT(*) FROM "ob-poc".client_group_relationship_sources s
                 WHERE s.relationship_id = r.id) as "source_count!"
            FROM "ob-poc".client_group_relationship r
            JOIN "ob-poc".entities pe ON pe.entity_id = r.parent_entity_id
            JOIN "ob-poc".entities ce ON ce.entity_id = r.child_entity_id
            WHERE r.group_id = $1
              AND pe.deleted_at IS NULL
              AND ce.deleted_at IS NULL
              AND ($2::UUID IS NULL OR r.parent_entity_id = $2 OR r.child_entity_id = $2)
              AND ($3::TEXT IS NULL OR r.relationship_kind = $3)
            ORDER BY pe.name, ce.name
            "#,
            group_id,
            entity_id,
            relationship_kind
        )
        .fetch_all(pool)
        .await?;

        let items: Vec<RelationshipResult> = rows
            .into_iter()
            .map(|r| RelationshipResult {
                id: r.id,
                parent_entity_id: r.parent_entity_id,
                parent_name: r.parent_name,
                child_entity_id: r.child_entity_id,
                child_name: r.child_name,
                relationship_kind: r.relationship_kind,
                canonical_ownership_pct: r
                    .canonical_ownership_pct
                    .map(|d| d.to_string().parse().unwrap_or(0.0)),
                source_count: r.source_count,
                review_status: r.review_status,
            })
            .collect();

        Ok(ExecutionResult::RecordSet(
            items
                .iter()
                .map(serde_json::to_value)
                .collect::<Result<Vec<_>, _>>()?,
        ))
    }

    #[cfg(not(feature = "database"))]
    async fn execute(
        &self,
        _verb_call: &VerbCall,
        _ctx: &mut ExecutionContext,
    ) -> Result<ExecutionResult> {
        Err(anyhow::anyhow!(
            "Database feature required for client-group operations"
        ))
    }

    #[cfg(feature = "database")]
    async fn execute_json(
        &self,
        args: &serde_json::Value,
        ctx: &mut dsl_runtime::VerbExecutionContext,
        pool: &PgPool,
    ) -> Result<dsl_runtime::VerbExecutionOutcome> {
        use super::helpers::{json_extract_string_opt, json_extract_uuid, json_extract_uuid_opt};
        let group_id = json_extract_uuid(args, ctx, "group-id")?;
        let entity_id = json_extract_uuid_opt(args, ctx, "entity-id");
        let relationship_kind = json_extract_string_opt(args, "relationship-kind");
        let rows = sqlx::query(
            r#"SELECT
                r.id, r.parent_entity_id, pe.name as parent_name,
                r.child_entity_id, ce.name as child_name,
                r.relationship_kind, r.review_status,
                (SELECT s.ownership_pct FROM "ob-poc".client_group_relationship_sources s
                 WHERE s.relationship_id = r.id
                 ORDER BY s.is_canonical DESC, s.confidence_score DESC NULLS LAST
                 LIMIT 1) as canonical_ownership_pct,
                (SELECT COUNT(*) FROM "ob-poc".client_group_relationship_sources s
                 WHERE s.relationship_id = r.id) as source_count
            FROM "ob-poc".client_group_relationship r
            JOIN "ob-poc".entities pe ON pe.entity_id = r.parent_entity_id
            JOIN "ob-poc".entities ce ON ce.entity_id = r.child_entity_id
            WHERE r.group_id = $1
              AND pe.deleted_at IS NULL AND ce.deleted_at IS NULL
              AND ($2::UUID IS NULL OR r.parent_entity_id = $2 OR r.child_entity_id = $2)
              AND ($3::TEXT IS NULL OR r.relationship_kind = $3)
            ORDER BY pe.name, ce.name"#,
        )
        .bind(group_id)
        .bind(entity_id)
        .bind(&relationship_kind)
        .fetch_all(pool)
        .await?;
        use sqlx::Row;
        let items: Vec<serde_json::Value> = rows.iter().map(|r| {
            let pct: Option<bigdecimal::BigDecimal> = r.get("canonical_ownership_pct");
            json!({
                "id": r.get::<Uuid, _>("id"),
                "parent_entity_id": r.get::<Uuid, _>("parent_entity_id"),
                "parent_name": r.get::<String, _>("parent_name"),
                "child_entity_id": r.get::<Uuid, _>("child_entity_id"),
                "child_name": r.get::<String, _>("child_name"),
                "relationship_kind": r.get::<String, _>("relationship_kind"),
                "canonical_ownership_pct": pct.map(|d| d.to_string().parse::<f64>().unwrap_or(0.0)),
                "source_count": r.get::<i64, _>("source_count"),
                "review_status": r.get::<String, _>("review_status"),
            })
        }).collect();
        Ok(dsl_runtime::VerbExecutionOutcome::RecordSet(
            items,
        ))
    }

    fn is_migrated(&self) -> bool {
        true
    }
}

// =============================================================================
// ADD-OWNERSHIP-SOURCE
// =============================================================================

#[cfg(feature = "database")]
fn get_optional_bigdecimal(verb_call: &VerbCall, key: &str) -> Option<bigdecimal::BigDecimal> {
    use std::str::FromStr;
    verb_call
        .arguments
        .iter()
        .find(|a| a.key == key)
        .and_then(|a| {
            if let Some(d) = a.value.as_decimal() {
                bigdecimal::BigDecimal::from_str(&d.to_string()).ok()
            } else if let Some(s) = a.value.as_string() {
                bigdecimal::BigDecimal::from_str(s).ok()
            } else {
                a.value.as_integer().map(bigdecimal::BigDecimal::from)
            }
        })
}

#[register_custom_op]
pub struct ClientGroupAddOwnershipSourceOp;

#[async_trait]
impl CustomOperation for ClientGroupAddOwnershipSourceOp {
    fn domain(&self) -> &'static str {
        "client-group"
    }

    fn verb(&self) -> &'static str {
        "add-ownership-source"
    }

    fn rationale(&self) -> &'static str {
        "Add an ownership source/allegation for a relationship"
    }

    #[cfg(feature = "database")]
    async fn execute(
        &self,
        verb_call: &VerbCall,
        ctx: &mut ExecutionContext,
        pool: &PgPool,
    ) -> Result<ExecutionResult> {
        let relationship_id = get_required_uuid(verb_call, "relationship-id", ctx)?;
        let source = get_required_string(verb_call, "source")?;
        let source_type = get_optional_string(verb_call, "source-type")
            .unwrap_or_else(|| "discovery".to_string());
        let ownership_pct = get_optional_bigdecimal(verb_call, "ownership-pct");
        let voting_pct = get_optional_bigdecimal(verb_call, "voting-pct");
        let control_pct = get_optional_bigdecimal(verb_call, "control-pct");
        let source_document_ref = get_optional_string(verb_call, "source-document-ref");
        let source_document_date = get_optional_string(verb_call, "source-document-date");
        let verifies_source_id = get_optional_uuid(verb_call, "verifies-source-id", ctx);

        let doc_date: Option<chrono::NaiveDate> = source_document_date
            .as_ref()
            .and_then(|s| chrono::NaiveDate::parse_from_str(s, "%Y-%m-%d").ok());

        // Get confidence score based on source
        let confidence_bd: bigdecimal::BigDecimal = sqlx::query_scalar!(
            r#"SELECT "ob-poc".get_source_confidence($1) as "conf!""#,
            source
        )
        .fetch_one(pool)
        .await?;
        let result = sqlx::query!(
            r#"
            INSERT INTO "ob-poc".client_group_relationship_sources
                (relationship_id, source, source_type, ownership_pct, voting_pct, control_pct,
                 source_document_ref, source_document_date, verifies_source_id, confidence_score)
            VALUES ($1, $2, $3, $4, $5, $6, $7, $8, $9, $10)
            RETURNING id
            "#,
            relationship_id,
            source,
            source_type,
            ownership_pct,
            voting_pct,
            control_pct,
            source_document_ref,
            doc_date,
            verifies_source_id,
            confidence_bd
        )
        .fetch_one(pool)
        .await?;

        Ok(ExecutionResult::Uuid(result.id))
    }

    #[cfg(not(feature = "database"))]
    async fn execute(
        &self,
        _verb_call: &VerbCall,
        _ctx: &mut ExecutionContext,
    ) -> Result<ExecutionResult> {
        Err(anyhow::anyhow!(
            "Database feature required for client-group operations"
        ))
    }

    #[cfg(feature = "database")]
    async fn execute_json(
        &self,
        args: &serde_json::Value,
        ctx: &mut dsl_runtime::VerbExecutionContext,
        pool: &PgPool,
    ) -> Result<dsl_runtime::VerbExecutionOutcome> {
        use super::helpers::{
            json_extract_string, json_extract_string_opt, json_extract_uuid, json_extract_uuid_opt,
        };
        use std::str::FromStr;
        let relationship_id = json_extract_uuid(args, ctx, "relationship-id")?;
        let source = json_extract_string(args, "source")?;
        let source_type =
            json_extract_string_opt(args, "source-type").unwrap_or_else(|| "discovery".to_string());
        let ownership_pct = json_extract_string_opt(args, "ownership-pct")
            .and_then(|s| bigdecimal::BigDecimal::from_str(&s).ok());
        let voting_pct = json_extract_string_opt(args, "voting-pct")
            .and_then(|s| bigdecimal::BigDecimal::from_str(&s).ok());
        let control_pct = json_extract_string_opt(args, "control-pct")
            .and_then(|s| bigdecimal::BigDecimal::from_str(&s).ok());
        let source_document_ref = json_extract_string_opt(args, "source-document-ref");
        let source_document_date = json_extract_string_opt(args, "source-document-date");
        let verifies_source_id = json_extract_uuid_opt(args, ctx, "verifies-source-id");
        let doc_date: Option<chrono::NaiveDate> = source_document_date
            .as_ref()
            .and_then(|s| chrono::NaiveDate::parse_from_str(s, "%Y-%m-%d").ok());
        let confidence_bd: bigdecimal::BigDecimal =
            sqlx::query_scalar(r#"SELECT "ob-poc".get_source_confidence($1)"#)
                .bind(&source)
                .fetch_one(pool)
                .await?;
        let id: Uuid = sqlx::query_scalar(
            r#"INSERT INTO "ob-poc".client_group_relationship_sources
                (relationship_id, source, source_type, ownership_pct, voting_pct, control_pct,
                 source_document_ref, source_document_date, verifies_source_id, confidence_score)
            VALUES ($1, $2, $3, $4, $5, $6, $7, $8, $9, $10)
            RETURNING id"#,
        )
        .bind(relationship_id)
        .bind(&source)
        .bind(&source_type)
        .bind(&ownership_pct)
        .bind(&voting_pct)
        .bind(&control_pct)
        .bind(&source_document_ref)
        .bind(doc_date)
        .bind(verifies_source_id)
        .bind(confidence_bd)
        .fetch_one(pool)
        .await?;
        Ok(dsl_runtime::VerbExecutionOutcome::Uuid(id))
    }

    fn is_migrated(&self) -> bool {
        true
    }
}

// =============================================================================
// VERIFY-OWNERSHIP
// =============================================================================

#[register_custom_op]
pub struct ClientGroupVerifyOwnershipOp;

#[async_trait]
impl CustomOperation for ClientGroupVerifyOwnershipOp {
    fn domain(&self) -> &'static str {
        "client-group"
    }

    fn verb(&self) -> &'static str {
        "verify-ownership"
    }

    fn rationale(&self) -> &'static str {
        "Mark an ownership source as verified"
    }

    #[cfg(feature = "database")]
    async fn execute(
        &self,
        verb_call: &VerbCall,
        ctx: &mut ExecutionContext,
        pool: &PgPool,
    ) -> Result<ExecutionResult> {
        let source_id = get_required_uuid(verb_call, "source-id", ctx)?;
        let verified_by = get_optional_string(verb_call, "verified-by");
        let notes = get_optional_string(verb_call, "notes");

        let affected = sqlx::query!(
            r#"
            UPDATE "ob-poc".client_group_relationship_sources
            SET verification_status = 'verified',
                verified_by = $2,
                verified_at = NOW(),
                verification_notes = $3,
                updated_at = NOW()
            WHERE id = $1
            "#,
            source_id,
            verified_by,
            notes
        )
        .execute(pool)
        .await?
        .rows_affected();

        Ok(ExecutionResult::Affected(affected))
    }

    #[cfg(not(feature = "database"))]
    async fn execute(
        &self,
        _verb_call: &VerbCall,
        _ctx: &mut ExecutionContext,
    ) -> Result<ExecutionResult> {
        Err(anyhow::anyhow!(
            "Database feature required for client-group operations"
        ))
    }

    #[cfg(feature = "database")]
    async fn execute_json(
        &self,
        args: &serde_json::Value,
        ctx: &mut dsl_runtime::VerbExecutionContext,
        pool: &PgPool,
    ) -> Result<dsl_runtime::VerbExecutionOutcome> {
        use super::helpers::{json_extract_string_opt, json_extract_uuid};
        let source_id = json_extract_uuid(args, ctx, "source-id")?;
        let verified_by = json_extract_string_opt(args, "verified-by");
        let notes = json_extract_string_opt(args, "notes");
        let affected = sqlx::query(
            r#"UPDATE "ob-poc".client_group_relationship_sources
            SET verification_status = 'verified', verified_by = $2,
                verified_at = NOW(), verification_notes = $3, updated_at = NOW()
            WHERE id = $1"#,
        )
        .bind(source_id)
        .bind(&verified_by)
        .bind(&notes)
        .execute(pool)
        .await?
        .rows_affected();
        Ok(dsl_runtime::VerbExecutionOutcome::Affected(
            affected,
        ))
    }

    fn is_migrated(&self) -> bool {
        true
    }
}

// =============================================================================
// SET-CANONICAL
// =============================================================================

#[register_custom_op]
pub struct ClientGroupSetCanonicalOp;

#[async_trait]
impl CustomOperation for ClientGroupSetCanonicalOp {
    fn domain(&self) -> &'static str {
        "client-group"
    }

    fn verb(&self) -> &'static str {
        "set-canonical"
    }

    fn rationale(&self) -> &'static str {
        "Designate a source as the canonical value for a relationship"
    }

    #[cfg(feature = "database")]
    async fn execute(
        &self,
        verb_call: &VerbCall,
        ctx: &mut ExecutionContext,
        pool: &PgPool,
    ) -> Result<ExecutionResult> {
        let source_id = get_required_uuid(verb_call, "source-id", ctx)?;
        let notes = get_optional_string(verb_call, "notes");

        // First, clear any existing canonical for this relationship
        sqlx::query!(
            r#"
            UPDATE "ob-poc".client_group_relationship_sources
            SET is_canonical = false, updated_at = NOW()
            WHERE relationship_id = (
                SELECT relationship_id FROM "ob-poc".client_group_relationship_sources WHERE id = $1
            ) AND is_canonical = true
            "#,
            source_id
        )
        .execute(pool)
        .await?;

        // Set new canonical
        let affected = sqlx::query!(
            r#"
            UPDATE "ob-poc".client_group_relationship_sources
            SET is_canonical = true,
                canonical_set_at = NOW(),
                canonical_notes = $2,
                updated_at = NOW()
            WHERE id = $1
            "#,
            source_id,
            notes
        )
        .execute(pool)
        .await?
        .rows_affected();

        Ok(ExecutionResult::Affected(affected))
    }

    #[cfg(not(feature = "database"))]
    async fn execute(
        &self,
        _verb_call: &VerbCall,
        _ctx: &mut ExecutionContext,
    ) -> Result<ExecutionResult> {
        Err(anyhow::anyhow!(
            "Database feature required for client-group operations"
        ))
    }

    #[cfg(feature = "database")]
    async fn execute_json(
        &self,
        args: &serde_json::Value,
        ctx: &mut dsl_runtime::VerbExecutionContext,
        pool: &PgPool,
    ) -> Result<dsl_runtime::VerbExecutionOutcome> {
        use super::helpers::{json_extract_string_opt, json_extract_uuid};
        let source_id = json_extract_uuid(args, ctx, "source-id")?;
        let notes = json_extract_string_opt(args, "notes");
        sqlx::query(
            r#"UPDATE "ob-poc".client_group_relationship_sources
            SET is_canonical = false, updated_at = NOW()
            WHERE relationship_id = (
                SELECT relationship_id FROM "ob-poc".client_group_relationship_sources WHERE id = $1
            ) AND is_canonical = true"#,
        )
        .bind(source_id)
        .execute(pool)
        .await?;
        let affected = sqlx::query(
            r#"UPDATE "ob-poc".client_group_relationship_sources
            SET is_canonical = true, canonical_set_at = NOW(), canonical_notes = $2, updated_at = NOW()
            WHERE id = $1"#,
        ).bind(source_id).bind(&notes).execute(pool).await?.rows_affected();
        Ok(dsl_runtime::VerbExecutionOutcome::Affected(
            affected,
        ))
    }

    fn is_migrated(&self) -> bool {
        true
    }
}

// =============================================================================
// LIST-UNVERIFIED
// =============================================================================

#[register_custom_op]
pub struct ClientGroupListUnverifiedOp;

#[async_trait]
impl CustomOperation for ClientGroupListUnverifiedOp {
    fn domain(&self) -> &'static str {
        "client-group"
    }

    fn verb(&self) -> &'static str {
        "list-unverified"
    }

    fn rationale(&self) -> &'static str {
        "List unverified ownership allegations needing review"
    }

    #[cfg(feature = "database")]
    async fn execute(
        &self,
        verb_call: &VerbCall,
        ctx: &mut ExecutionContext,
        pool: &PgPool,
    ) -> Result<ExecutionResult> {
        let group_id = get_required_uuid(verb_call, "group-id", ctx)?;
        let limit = get_optional_integer(verb_call, "limit").unwrap_or(50);

        let rows = sqlx::query!(
            r#"
            SELECT
                relationship_id as "relationship_id!",
                parent_name as "parent_name!",
                child_name as "child_name!",
                alleged_pct,
                source_document_ref,
                source_document_date,
                verification_count as "verification_count!"
            FROM "ob-poc".v_cgr_unverified_allegations
            WHERE group_id = $1
            LIMIT $2
            "#,
            group_id,
            limit
        )
        .fetch_all(pool)
        .await?;

        let items: Vec<UnverifiedAllegationResult> = rows
            .into_iter()
            .map(|r| UnverifiedAllegationResult {
                relationship_id: r.relationship_id,
                parent_name: r.parent_name,
                child_name: r.child_name,
                alleged_pct: r.alleged_pct.map(|d| d.to_string().parse().unwrap_or(0.0)),
                source_document_ref: r.source_document_ref,
                source_document_date: r.source_document_date.map(|d| d.to_string()),
                verification_count: r.verification_count,
            })
            .collect();

        Ok(ExecutionResult::RecordSet(
            items
                .iter()
                .map(serde_json::to_value)
                .collect::<Result<Vec<_>, _>>()?,
        ))
    }

    #[cfg(not(feature = "database"))]
    async fn execute(
        &self,
        _verb_call: &VerbCall,
        _ctx: &mut ExecutionContext,
    ) -> Result<ExecutionResult> {
        Err(anyhow::anyhow!(
            "Database feature required for client-group operations"
        ))
    }

    #[cfg(feature = "database")]
    async fn execute_json(
        &self,
        args: &serde_json::Value,
        ctx: &mut dsl_runtime::VerbExecutionContext,
        pool: &PgPool,
    ) -> Result<dsl_runtime::VerbExecutionOutcome> {
        use super::helpers::{json_extract_int_opt, json_extract_uuid};
        let group_id = json_extract_uuid(args, ctx, "group-id")?;
        let limit = json_extract_int_opt(args, "limit").unwrap_or(50);
        let rows = sqlx::query(
            r#"SELECT relationship_id, parent_name, child_name,
                   alleged_pct, source_document_ref, source_document_date, verification_count
            FROM "ob-poc".v_cgr_unverified_allegations
            WHERE group_id = $1 LIMIT $2"#,
        )
        .bind(group_id)
        .bind(limit)
        .fetch_all(pool)
        .await?;
        use sqlx::Row;
        let items: Vec<serde_json::Value> = rows.iter().map(|r| {
            let pct: Option<bigdecimal::BigDecimal> = r.get("alleged_pct");
            json!({
                "relationship_id": r.get::<Uuid, _>("relationship_id"),
                "parent_name": r.get::<String, _>("parent_name"),
                "child_name": r.get::<String, _>("child_name"),
                "alleged_pct": pct.map(|d| d.to_string().parse::<f64>().unwrap_or(0.0)),
                "source_document_ref": r.get::<Option<String>, _>("source_document_ref"),
                "source_document_date": r.get::<Option<chrono::NaiveDate>, _>("source_document_date").map(|d| d.to_string()),
                "verification_count": r.get::<i64, _>("verification_count"),
            })
        }).collect();
        Ok(dsl_runtime::VerbExecutionOutcome::RecordSet(
            items,
        ))
    }

    fn is_migrated(&self) -> bool {
        true
    }
}

// =============================================================================
// LIST-DISCREPANCIES
// =============================================================================

#[register_custom_op]
pub struct ClientGroupListDiscrepanciesOp;

#[async_trait]
impl CustomOperation for ClientGroupListDiscrepanciesOp {
    fn domain(&self) -> &'static str {
        "client-group"
    }

    fn verb(&self) -> &'static str {
        "list-discrepancies"
    }

    fn rationale(&self) -> &'static str {
        "List ownership relationships with conflicting source values"
    }

    #[cfg(feature = "database")]
    async fn execute(
        &self,
        verb_call: &VerbCall,
        ctx: &mut ExecutionContext,
        pool: &PgPool,
    ) -> Result<ExecutionResult> {
        let group_id = get_required_uuid(verb_call, "group-id", ctx)?;
        let min_spread = get_optional_bigdecimal(verb_call, "min-spread")
            .unwrap_or_else(|| bigdecimal::BigDecimal::from(1)); // 1.0

        let rows = sqlx::query!(
            r#"
            SELECT
                r.id as "relationship_id!",
                pe.name as "parent_name!",
                ce.name as "child_name!",
                d.sources as "sources!",
                d.ownership_values as "ownership_values!",
                d.ownership_spread as "ownership_spread!",
                d.alleged_pct,
                d.verified_pct
            FROM "ob-poc".v_cgr_discrepancies d
            JOIN "ob-poc".client_group_relationship r ON r.parent_entity_id = d.parent_entity_id
                AND r.child_entity_id = d.child_entity_id AND r.group_id = d.group_id
            JOIN "ob-poc".entities pe ON pe.entity_id = d.parent_entity_id
            JOIN "ob-poc".entities ce ON ce.entity_id = d.child_entity_id
            WHERE d.group_id = $1
              AND pe.deleted_at IS NULL
              AND ce.deleted_at IS NULL
              AND d.ownership_spread >= $2
            ORDER BY d.ownership_spread DESC
            "#,
            group_id,
            min_spread
        )
        .fetch_all(pool)
        .await?;

        let items: Vec<DiscrepancyResult> = rows
            .into_iter()
            .map(|r| DiscrepancyResult {
                relationship_id: r.relationship_id,
                parent_name: r.parent_name,
                child_name: r.child_name,
                sources: r.sources,
                ownership_values: r
                    .ownership_values
                    .into_iter()
                    .map(|d| d.to_string().parse().unwrap_or(0.0))
                    .collect(),
                ownership_spread: r.ownership_spread.to_string().parse().unwrap_or(0.0),
                alleged_pct: r.alleged_pct.map(|d| d.to_string().parse().unwrap_or(0.0)),
                verified_pct: r.verified_pct.map(|d| d.to_string().parse().unwrap_or(0.0)),
            })
            .collect();

        Ok(ExecutionResult::RecordSet(
            items
                .iter()
                .map(serde_json::to_value)
                .collect::<Result<Vec<_>, _>>()?,
        ))
    }

    #[cfg(not(feature = "database"))]
    async fn execute(
        &self,
        _verb_call: &VerbCall,
        _ctx: &mut ExecutionContext,
    ) -> Result<ExecutionResult> {
        Err(anyhow::anyhow!(
            "Database feature required for client-group operations"
        ))
    }

    #[cfg(feature = "database")]
    async fn execute_json(
        &self,
        args: &serde_json::Value,
        ctx: &mut dsl_runtime::VerbExecutionContext,
        pool: &PgPool,
    ) -> Result<dsl_runtime::VerbExecutionOutcome> {
        use super::helpers::{json_extract_string_opt, json_extract_uuid};
        use std::str::FromStr;
        let group_id = json_extract_uuid(args, ctx, "group-id")?;
        let min_spread = json_extract_string_opt(args, "min-spread")
            .and_then(|s| bigdecimal::BigDecimal::from_str(&s).ok())
            .unwrap_or_else(|| bigdecimal::BigDecimal::from(1));
        let rows = sqlx::query(
            r#"SELECT
                r.id as relationship_id, pe.name as parent_name, ce.name as child_name,
                d.sources, d.ownership_values, d.ownership_spread,
                d.alleged_pct, d.verified_pct
            FROM "ob-poc".v_cgr_discrepancies d
            JOIN "ob-poc".client_group_relationship r ON r.parent_entity_id = d.parent_entity_id
                AND r.child_entity_id = d.child_entity_id AND r.group_id = d.group_id
            JOIN "ob-poc".entities pe ON pe.entity_id = d.parent_entity_id
            JOIN "ob-poc".entities ce ON ce.entity_id = d.child_entity_id
            WHERE d.group_id = $1 AND pe.deleted_at IS NULL AND ce.deleted_at IS NULL
              AND d.ownership_spread >= $2
            ORDER BY d.ownership_spread DESC"#,
        )
        .bind(group_id)
        .bind(&min_spread)
        .fetch_all(pool)
        .await?;
        use sqlx::Row;
        let items: Vec<serde_json::Value> = rows.iter().map(|r| {
            let spread: bigdecimal::BigDecimal = r.get("ownership_spread");
            let vals: Vec<bigdecimal::BigDecimal> = r.get("ownership_values");
            let alleged: Option<bigdecimal::BigDecimal> = r.get("alleged_pct");
            let verified: Option<bigdecimal::BigDecimal> = r.get("verified_pct");
            json!({
                "relationship_id": r.get::<Uuid, _>("relationship_id"),
                "parent_name": r.get::<String, _>("parent_name"),
                "child_name": r.get::<String, _>("child_name"),
                "sources": r.get::<Vec<String>, _>("sources"),
                "ownership_values": vals.iter().map(|d| d.to_string().parse::<f64>().unwrap_or(0.0)).collect::<Vec<f64>>(),
                "ownership_spread": spread.to_string().parse::<f64>().unwrap_or(0.0),
                "alleged_pct": alleged.map(|d| d.to_string().parse::<f64>().unwrap_or(0.0)),
                "verified_pct": verified.map(|d| d.to_string().parse::<f64>().unwrap_or(0.0)),
            })
        }).collect();
        Ok(dsl_runtime::VerbExecutionOutcome::RecordSet(
            items,
        ))
    }

    fn is_migrated(&self) -> bool {
        true
    }
}

// =============================================================================
// START-DISCOVERY
// =============================================================================

#[register_custom_op]
pub struct ClientGroupStartDiscoveryOp;

#[async_trait]
impl CustomOperation for ClientGroupStartDiscoveryOp {
    fn domain(&self) -> &'static str {
        "client-group"
    }

    fn verb(&self) -> &'static str {
        "start-discovery"
    }

    fn rationale(&self) -> &'static str {
        "Start discovery process for a client group"
    }

    #[cfg(feature = "database")]
    async fn execute(
        &self,
        verb_call: &VerbCall,
        ctx: &mut ExecutionContext,
        pool: &PgPool,
    ) -> Result<ExecutionResult> {
        let group_id = get_required_uuid(verb_call, "group-id", ctx)?;
        let source =
            get_optional_string(verb_call, "source").unwrap_or_else(|| "manual".to_string());
        let root_lei = get_optional_string(verb_call, "root-lei");

        let affected = sqlx::query!(
            r#"
            UPDATE "ob-poc".client_group
            SET discovery_status = 'in_progress',
                discovery_started_at = NOW(),
                discovery_source = $2,
                discovery_root_lei = $3,
                updated_at = NOW()
            WHERE id = $1
            "#,
            group_id,
            source,
            root_lei
        )
        .execute(pool)
        .await?
        .rows_affected();

        Ok(ExecutionResult::Affected(affected))
    }

    #[cfg(not(feature = "database"))]
    async fn execute(
        &self,
        _verb_call: &VerbCall,
        _ctx: &mut ExecutionContext,
    ) -> Result<ExecutionResult> {
        Err(anyhow::anyhow!(
            "Database feature required for client-group operations"
        ))
    }

    #[cfg(feature = "database")]
    async fn execute_json(
        &self,
        args: &serde_json::Value,
        ctx: &mut dsl_runtime::VerbExecutionContext,
        pool: &PgPool,
    ) -> Result<dsl_runtime::VerbExecutionOutcome> {
        use super::helpers::{json_extract_string_opt, json_extract_uuid};
        let group_id = json_extract_uuid(args, ctx, "group-id")?;
        let source =
            json_extract_string_opt(args, "source").unwrap_or_else(|| "manual".to_string());
        let root_lei = json_extract_string_opt(args, "root-lei");
        let affected = sqlx::query(
            r#"UPDATE "ob-poc".client_group
            SET discovery_status = 'in_progress', discovery_started_at = NOW(),
                discovery_source = $2, discovery_root_lei = $3, updated_at = NOW()
            WHERE id = $1"#,
        )
        .bind(group_id)
        .bind(&source)
        .bind(&root_lei)
        .execute(pool)
        .await?
        .rows_affected();
        Ok(dsl_runtime::VerbExecutionOutcome::Affected(
            affected,
        ))
    }

    fn is_migrated(&self) -> bool {
        true
    }
}

// =============================================================================
// COMPLETE-DISCOVERY
// =============================================================================

#[register_custom_op]
pub struct ClientGroupCompleteDiscoveryOp;

#[async_trait]
impl CustomOperation for ClientGroupCompleteDiscoveryOp {
    fn domain(&self) -> &'static str {
        "client-group"
    }

    fn verb(&self) -> &'static str {
        "complete-discovery"
    }

    fn rationale(&self) -> &'static str {
        "Mark discovery process as complete"
    }

    #[cfg(feature = "database")]
    async fn execute(
        &self,
        verb_call: &VerbCall,
        ctx: &mut ExecutionContext,
        pool: &PgPool,
    ) -> Result<ExecutionResult> {
        let group_id = get_required_uuid(verb_call, "group-id", ctx)?;
        let _notes = get_optional_string(verb_call, "notes");

        let affected = sqlx::query!(
            r#"
            UPDATE "ob-poc".client_group
            SET discovery_status = 'complete',
                discovery_completed_at = NOW(),
                updated_at = NOW()
            WHERE id = $1
            "#,
            group_id
        )
        .execute(pool)
        .await?
        .rows_affected();

        Ok(ExecutionResult::Affected(affected))
    }

    #[cfg(not(feature = "database"))]
    async fn execute(
        &self,
        _verb_call: &VerbCall,
        _ctx: &mut ExecutionContext,
    ) -> Result<ExecutionResult> {
        Err(anyhow::anyhow!(
            "Database feature required for client-group operations"
        ))
    }

    #[cfg(feature = "database")]
    async fn execute_json(
        &self,
        args: &serde_json::Value,
        ctx: &mut dsl_runtime::VerbExecutionContext,
        pool: &PgPool,
    ) -> Result<dsl_runtime::VerbExecutionOutcome> {
        use super::helpers::{json_extract_string_opt, json_extract_uuid};
        let group_id = json_extract_uuid(args, ctx, "group-id")?;
        let _notes = json_extract_string_opt(args, "notes");
        let affected = sqlx::query(
            r#"UPDATE "ob-poc".client_group
            SET discovery_status = 'complete', discovery_completed_at = NOW(), updated_at = NOW()
            WHERE id = $1"#,
        )
        .bind(group_id)
        .execute(pool)
        .await?
        .rows_affected();
        Ok(dsl_runtime::VerbExecutionOutcome::Affected(
            affected,
        ))
    }

    fn is_migrated(&self) -> bool {
        true
    }
}
