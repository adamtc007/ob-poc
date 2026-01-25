//! Client Group Entity Context Operations
//!
//! These operations manage entity membership and shorthand tags for client groups,
//! enabling Candle-assisted semantic resolution from human language to entity_ids.
//!
//! # Architecture
//!
//! - `client_group_entity` - Which entities belong to a client group
//! - `client_group_entity_tag` - Human-readable shorthand labels
//! - `client_group_entity_tag_embedding` - Candle embeddings for semantic search
//!
//! # Verbs
//!
//! Entity Membership:
//! - `client-group.entity-add` - Add entity to group
//! - `client-group.entity-remove` - Remove entity from group
//! - `client-group.entity-list` - List entities in group
//!
//! Shorthand Tags:
//! - `client-group.tag-add` - Add shorthand tag to entity
//! - `client-group.tag-remove` - Remove tag
//! - `client-group.tag-list` - List tags
//!
//! Semantic Search:
//! - `client-group.search` - Search entities by shorthand (Candle-assisted)
//!
//! Discovery:
//! - `client-group.discover-entities` - Find entities that might belong to group
//! - `client-group.confirm-entity` - Confirm suspected entity
//! - `client-group.reject-entity` - Reject suspected entity

use anyhow::Result;
use async_trait::async_trait;
use ob_poc_macros::register_custom_op;
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

#[allow(dead_code)] // Future use for membership operations
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct EntityMembershipResult {
    pub id: Uuid,
    pub group_id: Uuid,
    pub entity_id: Uuid,
    pub entity_name: String,
    pub membership_type: String,
}

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
}

// =============================================================================
// ENTITY-LIST
// =============================================================================

#[register_custom_op]
pub struct ClientGroupEntityListOp;

#[async_trait]
impl CustomOperation for ClientGroupEntityListOp {
    fn domain(&self) -> &'static str {
        "client-group"
    }

    fn verb(&self) -> &'static str {
        "entity-list"
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
                .map(|i| serde_json::to_value(i).unwrap())
                .collect(),
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
}

// =============================================================================
// TAG-LIST
// =============================================================================

#[register_custom_op]
pub struct ClientGroupTagListOp;

#[async_trait]
impl CustomOperation for ClientGroupTagListOp {
    fn domain(&self) -> &'static str {
        "client-group"
    }

    fn verb(&self) -> &'static str {
        "tag-list"
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
                .map(|i| serde_json::to_value(i).unwrap())
                .collect(),
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
}

// =============================================================================
// SEARCH (Candle-assisted semantic search)
// =============================================================================

#[register_custom_op]
pub struct ClientGroupSearchOp;

#[async_trait]
impl CustomOperation for ClientGroupSearchOp {
    fn domain(&self) -> &'static str {
        "client-group"
    }

    fn verb(&self) -> &'static str {
        "search"
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
                .map(|i| serde_json::to_value(i).unwrap())
                .collect(),
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
                .map(|i| serde_json::to_value(i).unwrap())
                .collect(),
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
}
