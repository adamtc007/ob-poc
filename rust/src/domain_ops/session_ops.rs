//! Session Operations - Astro Navigation Model
//!
//! Metaphor hierarchy:
//!   Universe  = All regions client operates in (global footprint)
//!   Galaxy    = Regional (LU, DE, IE) - may have multiple ManCos
//!   Cluster   = ManCo's controlled CBUs (gravitational grouping)
//!   System    = Single CBU (solar system container)
//!
//! # Verbs
//!
//! - `session.load-universe` - Load all CBUs (optionally filtered by client)
//! - `session.load-galaxy` - Load all CBUs in a jurisdiction (regional)
//! - `session.load-cluster` - Load all CBUs under a ManCo/governance controller
//! - `session.load-system` - Load a single CBU
//! - `session.unload-system` - Remove a CBU from session
//! - `session.clear` - Clear all CBUs
//! - `session.undo` - Undo last action
//! - `session.redo` - Redo undone action
//! - `session.info` - Get session info
//! - `session.list` - List loaded CBUs
//!
//! # Performance
//!
//! All mutations are sync, in-memory, <1µs.
//! DB queries (for CBU details) are async but don't block mutations.

use anyhow::Result;
use async_trait::async_trait;
use dsl_runtime_macros::register_custom_op;
use serde::{Deserialize, Serialize};
use serde_json::json;
use uuid::Uuid;

use super::CustomOperation;
use crate::session::UnifiedSession;

#[cfg(feature = "database")]
use sqlx::PgPool;

// =============================================================================
// RESULT TYPES
// =============================================================================

/// Summary of a CBU for list responses
#[derive(Debug, Clone, Serialize, Deserialize)]
#[cfg_attr(feature = "database", derive(sqlx::FromRow))]
pub struct CbuSummary {
    pub cbu_id: Uuid,
    pub name: String,
    pub jurisdiction: Option<String>,
}

/// Result of clearing session scope
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ClearResult {
    pub cleared: bool,
    pub count: usize,
}

/// Result of undo/redo operations
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct HistoryResult {
    pub success: bool,
    pub scope_size: usize,
    pub history_depth: usize,
    pub future_depth: usize,
}

/// Jurisdiction count for session info
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct JurisdictionCount {
    pub jurisdiction: String,
    pub count: i64,
}

/// Session info response
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct SessionInfo {
    pub session_id: Uuid,
    pub name: Option<String>,
    pub total_cbus: usize,
    pub jurisdictions: Vec<JurisdictionCount>,
    pub history_depth: usize,
    pub future_depth: usize,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct LoadUniverseResult {
    pub count_added: usize,
    pub total_loaded: usize,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct LoadGalaxyResult {
    pub jurisdiction: String,
    pub count_added: usize,
    pub total_loaded: usize,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct LoadClusterResult {
    pub manco_name: String,
    pub manco_entity_id: Uuid,
    pub jurisdiction: Option<String>,
    pub count_added: usize,
    pub total_loaded: usize,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct LoadSystemResult {
    pub cbu_id: Uuid,
    pub name: String,
    pub jurisdiction: Option<String>,
    pub total_loaded: usize,
    pub was_new: bool,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct UnloadSystemResult {
    pub cbu_id: Uuid,
    pub name: String,
    pub total_loaded: usize,
    pub was_present: bool,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct FilterJurisdictionResult {
    pub jurisdiction: String,
    pub count_kept: usize,
    pub count_removed: usize,
    pub total_loaded: usize,
}

// =============================================================================
// START SESSION
// =============================================================================

#[register_custom_op]
pub struct SessionStartOp;

#[async_trait]
impl CustomOperation for SessionStartOp {
    fn domain(&self) -> &'static str {
        "session"
    }

    fn verb(&self) -> &'static str {
        "start"
    }

    fn rationale(&self) -> &'static str {
        "Creates a fresh in-memory session state for DSL-driven session workflows"
    }

    #[cfg(feature = "database")]
    async fn execute_json(
        &self,
        args: &serde_json::Value,
        ctx: &mut dsl_runtime::VerbExecutionContext,
        _pool: &PgPool,
    ) -> Result<dsl_runtime::VerbExecutionOutcome> {
        use super::helpers::{ext_set_pending_session, json_extract_string, json_extract_string_opt};

        let mode = json_extract_string(args, "mode")?;
        let from = json_extract_string_opt(args, "from");

        let session = UnifiedSession::new();
        let session_id = session.id;
        ext_set_pending_session(ctx, session);

        Ok(dsl_runtime::VerbExecutionOutcome::Record(json!({
            "session_id": session_id,
            "mode": mode,
            "client_group_name": serde_json::Value::Null,
            "workspace": from,
        })))
    }

    fn is_migrated(&self) -> bool {
        true
    }
}


// =============================================================================
// LOAD-UNIVERSE (all CBUs, optionally filtered by client)
// =============================================================================

#[register_custom_op]
pub struct SessionLoadUniverseOp;

#[async_trait]
impl CustomOperation for SessionLoadUniverseOp {
    fn domain(&self) -> &'static str {
        "session"
    }

    fn verb(&self) -> &'static str {
        "load-universe"
    }

    fn rationale(&self) -> &'static str {
        "Loads all CBUs into the session (Universe = global footprint)"
    }
#[cfg(feature = "database")]
    async fn execute_json(
        &self,
        args: &serde_json::Value,
        ctx: &mut dsl_runtime::VerbExecutionContext,
        pool: &PgPool,
    ) -> Result<dsl_runtime::VerbExecutionOutcome> {
        use super::helpers::{
            ext_set_pending_session, ext_take_or_create_pending_session, json_extract_uuid_opt,
        };

        let client_id = json_extract_uuid_opt(args, ctx, "client-id");

        // Fetch all CBU IDs (optionally filtered by client/apex entity)
        let cbu_ids: Vec<Uuid> = if let Some(client_id) = client_id {
            sqlx::query_scalar!(
                r#"
                SELECT DISTINCT c.cbu_id as "cbu_id!"
                FROM "ob-poc".cbus c
                LEFT JOIN "ob-poc".cbu_groups g ON g.manco_entity_id = $1
                LEFT JOIN "ob-poc".cbu_group_members gm ON gm.group_id = g.group_id AND gm.cbu_id = c.cbu_id
                WHERE c.deleted_at IS NULL
                  AND (
                       c.commercial_client_entity_id = $1
                   OR gm.cbu_id IS NOT NULL
                  )
                "#,
                client_id
            )
            .fetch_all(pool)
            .await?
        } else {
            sqlx::query_scalar!(
                r#"SELECT cbu_id as "cbu_id!" FROM "ob-poc".cbus WHERE deleted_at IS NULL"#
            )
            .fetch_all(pool)
            .await?
        };

        let mut session = ext_take_or_create_pending_session(ctx);
        let count_added = session.load_cbus(cbu_ids);
        let total_loaded = session.cbu_count();
        ext_set_pending_session(ctx, session);

        let result = LoadUniverseResult {
            count_added,
            total_loaded,
        };

        Ok(dsl_runtime::VerbExecutionOutcome::Record(json!(result)))
    }

    fn is_migrated(&self) -> bool {
        true
    }
}


// =============================================================================
// LOAD-GALAXY (all CBUs in a jurisdiction/region)
// =============================================================================

#[register_custom_op]
pub struct SessionLoadGalaxyOp;

#[async_trait]
impl CustomOperation for SessionLoadGalaxyOp {
    fn domain(&self) -> &'static str {
        "session"
    }

    fn verb(&self) -> &'static str {
        "load-galaxy"
    }

    fn rationale(&self) -> &'static str {
        "Loads all CBUs in a jurisdiction (Galaxy = regional)"
    }
#[cfg(feature = "database")]
    async fn execute_json(
        &self,
        args: &serde_json::Value,
        ctx: &mut dsl_runtime::VerbExecutionContext,
        pool: &PgPool,
    ) -> Result<dsl_runtime::VerbExecutionOutcome> {
        use super::helpers::{
            ext_set_pending_session, ext_take_or_create_pending_session, json_extract_string,
        };

        let jurisdiction = json_extract_string(args, "jurisdiction")?;

        let cbu_ids: Vec<Uuid> = sqlx::query_scalar!(
            r#"SELECT cbu_id as "cbu_id!" FROM "ob-poc".cbus WHERE jurisdiction = $1 AND deleted_at IS NULL"#,
            jurisdiction
        )
        .fetch_all(pool)
        .await?;

        let mut session = ext_take_or_create_pending_session(ctx);
        let count_added = session.load_cbus(cbu_ids);
        let total_loaded = session.cbu_count();
        ext_set_pending_session(ctx, session);

        let result = LoadGalaxyResult {
            jurisdiction,
            count_added,
            total_loaded,
        };

        Ok(dsl_runtime::VerbExecutionOutcome::Record(json!(result)))
    }

    fn is_migrated(&self) -> bool {
        true
    }
}


// =============================================================================
// LOAD-CLUSTER (all CBUs under a GROUP apex entity)
// =============================================================================
// Loads CBUs via ownership hierarchy:
//   GROUP (apex) → ManCo → CBU (via share_links or manco_entity_id)
//
// Two resolution paths:
//   1. :client "Allianz" → client_group_id → anchor_entity_id (via resolve_client_group_anchor)
//   2. :apex-entity-id UUID → direct entity lookup
//
// The validation rule `one_of_required: [client, apex-entity-id]` ensures exactly one is provided.

#[register_custom_op]
pub struct SessionLoadClusterOp;

#[async_trait]
impl CustomOperation for SessionLoadClusterOp {
    fn domain(&self) -> &'static str {
        "session"
    }

    fn verb(&self) -> &'static str {
        "load-cluster"
    }

    fn rationale(&self) -> &'static str {
        "Loads all CBUs under a GROUP apex entity via ownership hierarchy"
    }
#[cfg(feature = "database")]
    async fn execute_json(
        &self,
        args: &serde_json::Value,
        ctx: &mut dsl_runtime::VerbExecutionContext,
        pool: &PgPool,
    ) -> Result<dsl_runtime::VerbExecutionOutcome> {
        use super::helpers::{
            ext_set_pending_session, ext_take_or_create_pending_session, json_extract_string_opt,
            json_extract_uuid, json_extract_uuid_opt,
        };

        let jurisdiction = json_extract_string_opt(args, "jurisdiction");

        // Two-stage resolution: :client → client_group_id → anchor_entity_id
        // OR direct :apex-entity-id
        let apex_entity_id: Uuid = if let Some(client_group_id) =
            json_extract_uuid_opt(args, ctx, "client")
        {
            // Resolve client_group_id → anchor_entity_id via DB function
            // The function applies deterministic ordering:
            //   1. Exact jurisdiction match (if provided)
            //   2. Global fallback (jurisdiction = '')
            //   3. Priority (higher = preferred)
            //   4. Confidence (higher = preferred)
            //   5. UUID (tie-breaker)
            //
            // anchor_role = 'governance_controller' for session.load-cluster
            let anchor: Option<Uuid> = sqlx::query_scalar!(
                r#"
                SELECT anchor_entity_id as "anchor_entity_id!"
                FROM "ob-poc".resolve_client_group_anchor($1, 'governance_controller', COALESCE($2, ''))
                "#,
                client_group_id,
                jurisdiction.as_deref()
            )
            .fetch_optional(pool)
            .await?;

            anchor.ok_or_else(|| {
                anyhow::anyhow!(
                    "No anchor entity found for client group {} (jurisdiction: {:?})",
                    client_group_id,
                    jurisdiction
                )
            })?
        } else {
            // Direct apex-entity-id (validated by one_of_required)
            json_extract_uuid(args, ctx, "apex-entity-id")?
        };

        // Get apex entity name for response
        let apex_name: String = sqlx::query_scalar!(
            r#"SELECT name as "name!" FROM "ob-poc".entities WHERE entity_id = $1 AND deleted_at IS NULL"#,
            apex_entity_id
        )
        .fetch_optional(pool)
        .await?
        .unwrap_or_else(|| "Unknown".to_string());

        // Find all CBUs for this client group via client_group_entity.cbu_id
        //
        // Path: client_group → client_group_entity (cbu_id) → cbus
        //
        // The cbu_id is set by cbu.create when linking a fund entity.
        // This is the fast shorthand lookup - no tree walking required.
        //
        // We need the client_group_id to query. If we only have apex_entity_id,
        // we reverse-lookup the group via client_group_anchor.
        let client_group_id: Option<Uuid> = json_extract_uuid_opt(args, ctx, "client");

        let group_id: Uuid = if let Some(gid) = client_group_id {
            gid
        } else {
            // Reverse lookup: apex_entity_id → client_group via anchor
            sqlx::query_scalar(
                r#"
                SELECT group_id
                FROM "ob-poc".client_group_anchor
                WHERE anchor_entity_id = $1
                LIMIT 1
                "#,
            )
            .bind(apex_entity_id)
            .fetch_optional(pool)
            .await?
            .ok_or_else(|| {
                anyhow::anyhow!(
                    "No client group found for anchor entity {} - use client_group_entity for CBU lookup",
                    apex_entity_id
                )
            })?
        };

        let cbu_ids: Vec<Uuid> = sqlx::query_scalar(
            r#"
            SELECT DISTINCT cge.cbu_id
            FROM "ob-poc".client_group_entity cge
            JOIN "ob-poc".cbus c ON c.cbu_id = cge.cbu_id
            WHERE cge.group_id = $1
              AND cge.cbu_id IS NOT NULL
              AND cge.membership_type NOT IN ('historical', 'rejected')
              AND c.deleted_at IS NULL
              AND ($2::text IS NULL OR c.jurisdiction = $2)
            "#,
        )
        .bind(group_id)
        .bind(jurisdiction.as_deref())
        .fetch_all(pool)
        .await?;

        if cbu_ids.is_empty() {
            return Err(anyhow::anyhow!(
                "No CBUs found under '{}' ({})",
                apex_name,
                apex_entity_id
            ));
        }

        let mut session = ext_take_or_create_pending_session(ctx);
        session.name = Some(format!("{} Book", apex_name));
        let count_added = session.load_cbus(cbu_ids);
        let total_loaded = session.cbu_count();
        ext_set_pending_session(ctx, session);

        let result = LoadClusterResult {
            manco_name: apex_name,
            manco_entity_id: apex_entity_id,
            jurisdiction,
            count_added,
            total_loaded,
        };

        Ok(dsl_runtime::VerbExecutionOutcome::Record(json!(result)))
    }

    fn is_migrated(&self) -> bool {
        true
    }
}
// =============================================================================
// LOAD-SYSTEM (single CBU)
// =============================================================================

#[register_custom_op]
pub struct SessionLoadSystemOp;

#[async_trait]
impl CustomOperation for SessionLoadSystemOp {
    fn domain(&self) -> &'static str {
        "session"
    }

    fn verb(&self) -> &'static str {
        "load-system"
    }

    fn rationale(&self) -> &'static str {
        "Loads a single CBU into the session (System = solar system container)"
    }
#[cfg(feature = "database")]
    async fn execute_json(
        &self,
        args: &serde_json::Value,
        ctx: &mut dsl_runtime::VerbExecutionContext,
        pool: &PgPool,
    ) -> Result<dsl_runtime::VerbExecutionOutcome> {
        use super::helpers::{
            ext_set_pending_session, ext_take_or_create_pending_session, json_extract_uuid,
        };

        let cbu_id = json_extract_uuid(args, ctx, "cbu-id")?;

        let cbu = sqlx::query!(
            r#"SELECT cbu_id, name, jurisdiction
               FROM "ob-poc".cbus
               WHERE cbu_id = $1
                 AND deleted_at IS NULL"#,
            cbu_id
        )
        .fetch_optional(pool)
        .await?
        .ok_or_else(|| anyhow::anyhow!("CBU not found: {}", cbu_id))?;

        let mut session = ext_take_or_create_pending_session(ctx);
        let was_new = session.load_cbu(cbu_id);
        let total_loaded = session.cbu_count();
        ext_set_pending_session(ctx, session);

        let result = LoadSystemResult {
            cbu_id,
            name: cbu.name,
            jurisdiction: cbu.jurisdiction,
            total_loaded,
            was_new,
        };

        Ok(dsl_runtime::VerbExecutionOutcome::Record(json!(result)))
    }

    fn is_migrated(&self) -> bool {
        true
    }
}
// =============================================================================
// UNLOAD-SYSTEM (remove a CBU)
// =============================================================================

#[register_custom_op]
pub struct SessionUnloadSystemOp;

#[async_trait]
impl CustomOperation for SessionUnloadSystemOp {
    fn domain(&self) -> &'static str {
        "session"
    }

    fn verb(&self) -> &'static str {
        "unload-system"
    }

    fn rationale(&self) -> &'static str {
        "Removes a CBU from the session"
    }
#[cfg(feature = "database")]
    async fn execute_json(
        &self,
        args: &serde_json::Value,
        ctx: &mut dsl_runtime::VerbExecutionContext,
        pool: &PgPool,
    ) -> Result<dsl_runtime::VerbExecutionOutcome> {
        use super::helpers::{
            ext_set_pending_session, ext_take_or_create_pending_session, json_extract_uuid,
        };

        let cbu_id = json_extract_uuid(args, ctx, "cbu-id")?;

        let name: String = sqlx::query_scalar!(
            r#"SELECT name FROM "ob-poc".cbus WHERE cbu_id = $1 AND deleted_at IS NULL"#,
            cbu_id
        )
        .fetch_optional(pool)
        .await?
        .unwrap_or_default();

        let mut session = ext_take_or_create_pending_session(ctx);
        let was_present = session.unload_cbu(cbu_id);
        let total_loaded = session.cbu_count();
        ext_set_pending_session(ctx, session);

        let result = UnloadSystemResult {
            cbu_id,
            name,
            total_loaded,
            was_present,
        };

        Ok(dsl_runtime::VerbExecutionOutcome::Record(json!(result)))
    }

    fn is_migrated(&self) -> bool {
        true
    }
}
// =============================================================================
// FILTER-JURISDICTION (narrow scope)
// =============================================================================

#[register_custom_op]
pub struct SessionFilterJurisdictionOp;

#[async_trait]
impl CustomOperation for SessionFilterJurisdictionOp {
    fn domain(&self) -> &'static str {
        "session"
    }

    fn verb(&self) -> &'static str {
        "filter-jurisdiction"
    }

    fn rationale(&self) -> &'static str {
        "Narrows session scope to only CBUs in a specific jurisdiction"
    }
#[cfg(feature = "database")]
    async fn execute_json(
        &self,
        args: &serde_json::Value,
        ctx: &mut dsl_runtime::VerbExecutionContext,
        pool: &PgPool,
    ) -> Result<dsl_runtime::VerbExecutionOutcome> {
        use super::helpers::{
            ext_set_pending_session, ext_take_or_create_pending_session, json_extract_string,
        };

        let jurisdiction = json_extract_string(args, "jurisdiction")?;

        let mut session = ext_take_or_create_pending_session(ctx);
        let before_count = session.cbu_count();
        let current_cbu_ids = session.cbu_ids_vec();

        if current_cbu_ids.is_empty() {
            ext_set_pending_session(ctx, session);
            return Ok(dsl_runtime::VerbExecutionOutcome::Record(json!(
                FilterJurisdictionResult {
                    jurisdiction,
                    count_kept: 0,
                    count_removed: 0,
                    total_loaded: 0,
                }
            )));
        }

        let matching_cbu_ids: Vec<Uuid> = sqlx::query_scalar!(
            r#"SELECT cbu_id as "cbu_id!" FROM "ob-poc".cbus
               WHERE cbu_id = ANY($1)
                 AND jurisdiction = $2
                 AND deleted_at IS NULL"#,
            &current_cbu_ids,
            &jurisdiction
        )
        .fetch_all(pool)
        .await?;

        session.clear_cbus_with_history();
        let count_kept = session.load_cbus(matching_cbu_ids);
        let count_removed = before_count - count_kept;
        let total_loaded = session.cbu_count();
        ext_set_pending_session(ctx, session);

        let result = FilterJurisdictionResult {
            jurisdiction,
            count_kept,
            count_removed,
            total_loaded,
        };

        Ok(dsl_runtime::VerbExecutionOutcome::Record(json!(result)))
    }

    fn is_migrated(&self) -> bool {
        true
    }
}


// =============================================================================
// CLEAR
// =============================================================================

#[register_custom_op]
pub struct SessionClearOp;

#[async_trait]
impl CustomOperation for SessionClearOp {
    fn domain(&self) -> &'static str {
        "session"
    }

    fn verb(&self) -> &'static str {
        "clear"
    }

    fn rationale(&self) -> &'static str {
        "Clears all CBUs from the session"
    }
#[cfg(feature = "database")]
    async fn execute_json(
        &self,
        _args: &serde_json::Value,
        ctx: &mut dsl_runtime::VerbExecutionContext,
        _pool: &PgPool,
    ) -> Result<dsl_runtime::VerbExecutionOutcome> {
        use super::helpers::{ext_set_pending_session, ext_take_or_create_pending_session};

        let mut session = ext_take_or_create_pending_session(ctx);
        let count = session.clear_cbus_with_history();
        ext_set_pending_session(ctx, session);

        Ok(dsl_runtime::VerbExecutionOutcome::Record(json!(
            ClearResult {
                cleared: true,
                count,
            }
        )))
    }

    fn is_migrated(&self) -> bool {
        true
    }
}
// =============================================================================
// UNDO
// =============================================================================

#[register_custom_op]
pub struct SessionUndoOp;

#[async_trait]
impl CustomOperation for SessionUndoOp {
    fn domain(&self) -> &'static str {
        "session"
    }

    fn verb(&self) -> &'static str {
        "undo"
    }

    fn rationale(&self) -> &'static str {
        "Undoes the last session action"
    }
#[cfg(feature = "database")]
    async fn execute_json(
        &self,
        _args: &serde_json::Value,
        ctx: &mut dsl_runtime::VerbExecutionContext,
        _pool: &PgPool,
    ) -> Result<dsl_runtime::VerbExecutionOutcome> {
        use super::helpers::{ext_set_pending_session, ext_take_or_create_pending_session};

        let mut session = ext_take_or_create_pending_session(ctx);
        let success = session.undo_cbu();
        let scope_size = session.cbu_count();
        let history_depth = session.cbu_history_depth();
        let future_depth = session.cbu_future_depth();
        ext_set_pending_session(ctx, session);

        let result = HistoryResult {
            success,
            scope_size,
            history_depth,
            future_depth,
        };

        Ok(dsl_runtime::VerbExecutionOutcome::Record(json!(result)))
    }

    fn is_migrated(&self) -> bool {
        true
    }
}
// =============================================================================
// REDO
// =============================================================================

#[register_custom_op]
pub struct SessionRedoOp;

#[async_trait]
impl CustomOperation for SessionRedoOp {
    fn domain(&self) -> &'static str {
        "session"
    }

    fn verb(&self) -> &'static str {
        "redo"
    }

    fn rationale(&self) -> &'static str {
        "Redoes the last undone action"
    }
#[cfg(feature = "database")]
    async fn execute_json(
        &self,
        _args: &serde_json::Value,
        ctx: &mut dsl_runtime::VerbExecutionContext,
        _pool: &PgPool,
    ) -> Result<dsl_runtime::VerbExecutionOutcome> {
        use super::helpers::{ext_set_pending_session, ext_take_or_create_pending_session};

        let mut session = ext_take_or_create_pending_session(ctx);
        let success = session.redo_cbu();
        let scope_size = session.cbu_count();
        let history_depth = session.cbu_history_depth();
        let future_depth = session.cbu_future_depth();
        ext_set_pending_session(ctx, session);

        let result = HistoryResult {
            success,
            scope_size,
            history_depth,
            future_depth,
        };

        Ok(dsl_runtime::VerbExecutionOutcome::Record(json!(result)))
    }

    fn is_migrated(&self) -> bool {
        true
    }
}
// =============================================================================
// INFO
// =============================================================================

#[register_custom_op]
pub struct SessionInfoOp;

#[async_trait]
impl CustomOperation for SessionInfoOp {
    fn domain(&self) -> &'static str {
        "session"
    }

    fn verb(&self) -> &'static str {
        "info"
    }

    fn rationale(&self) -> &'static str {
        "Gets session info including jurisdiction breakdown"
    }
#[cfg(feature = "database")]
    async fn execute_json(
        &self,
        _args: &serde_json::Value,
        ctx: &mut dsl_runtime::VerbExecutionContext,
        pool: &PgPool,
    ) -> Result<dsl_runtime::VerbExecutionOutcome> {
        use super::helpers::{ext_set_pending_session, ext_take_or_create_pending_session};

        let session = ext_take_or_create_pending_session(ctx);
        let cbu_ids = session.cbu_ids_vec();

        let jurisdictions: Vec<JurisdictionCount> = if cbu_ids.is_empty() {
            vec![]
        } else {
            sqlx::query!(
                r#"
                SELECT jurisdiction, COUNT(*) as count
                FROM "ob-poc".cbus
                WHERE cbu_id = ANY($1)
                  AND deleted_at IS NULL
                GROUP BY jurisdiction
                ORDER BY count DESC
                "#,
                &cbu_ids
            )
            .fetch_all(pool)
            .await?
            .into_iter()
            .map(|r| JurisdictionCount {
                jurisdiction: r.jurisdiction.unwrap_or_default(),
                count: r.count.unwrap_or(0),
            })
            .collect()
        };

        let result = SessionInfo {
            session_id: session.id,
            name: session.name.clone(),
            total_cbus: session.cbu_count(),
            jurisdictions,
            history_depth: session.cbu_history_depth(),
            future_depth: session.cbu_future_depth(),
        };
        ext_set_pending_session(ctx, session);

        Ok(dsl_runtime::VerbExecutionOutcome::Record(json!(result)))
    }

    fn is_migrated(&self) -> bool {
        true
    }
}


// =============================================================================
// LIST
// =============================================================================

#[register_custom_op]
pub struct SessionListOp;

#[async_trait]
impl CustomOperation for SessionListOp {
    fn domain(&self) -> &'static str {
        "session"
    }

    fn verb(&self) -> &'static str {
        "list"
    }

    fn rationale(&self) -> &'static str {
        "Lists CBUs currently loaded in the session"
    }
#[cfg(feature = "database")]
    async fn execute_json(
        &self,
        args: &serde_json::Value,
        ctx: &mut dsl_runtime::VerbExecutionContext,
        pool: &PgPool,
    ) -> Result<dsl_runtime::VerbExecutionOutcome> {
        use super::helpers::{
            ext_set_pending_session, ext_take_or_create_pending_session, json_extract_int_opt,
            json_extract_string_opt,
        };

        let limit = json_extract_int_opt(args, "limit").unwrap_or(100);
        let jurisdiction_filter = json_extract_string_opt(args, "jurisdiction");

        let session = ext_take_or_create_pending_session(ctx);
        let cbu_ids = session.cbu_ids_vec();
        let total_in_session = session.cbu_count();

        let cbus: Vec<CbuSummary> = if cbu_ids.is_empty() {
            vec![]
        } else {
            sqlx::query_as!(
                CbuSummary,
                r#"
                SELECT cbu_id, name, jurisdiction
                FROM "ob-poc".cbus
                WHERE cbu_id = ANY($1)
                  AND deleted_at IS NULL
                  AND ($2::text IS NULL OR jurisdiction = $2)
                ORDER BY name
                LIMIT $3
                "#,
                &cbu_ids,
                jurisdiction_filter.as_deref(),
                limit
            )
            .fetch_all(pool)
            .await?
        };

        ext_set_pending_session(ctx, session);

        let count = cbus.len();
        Ok(dsl_runtime::VerbExecutionOutcome::Record(json!({
            "cbus": cbus,
            "count": count,
            "total_in_session": total_in_session
        })))
    }

    fn is_migrated(&self) -> bool {
        true
    }
}
// =============================================================================
// SET-CLIENT (Client Group Context for Entity Resolution)
// =============================================================================

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct SetClientResult {
    pub group_id: Option<Uuid>,
    pub group_name: Option<String>,
    pub entity_count: i64,
    pub candidates: Vec<ClientGroupCandidate>,
    pub resolved: bool,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ClientGroupCandidate {
    pub group_id: Uuid,
    pub group_name: String,
    pub confidence: f64,
}

#[register_custom_op]
pub struct SessionSetClientOp;

#[async_trait]
impl CustomOperation for SessionSetClientOp {
    fn domain(&self) -> &'static str {
        "session"
    }

    fn verb(&self) -> &'static str {
        "set-client"
    }

    fn rationale(&self) -> &'static str {
        "Sets client group context for entity resolution"
    }
#[cfg(feature = "database")]
    async fn execute_json(
        &self,
        args: &serde_json::Value,
        ctx: &mut dsl_runtime::VerbExecutionContext,
        pool: &PgPool,
    ) -> Result<dsl_runtime::VerbExecutionOutcome> {
        use super::helpers::json_extract_string;

        let client = json_extract_string(args, "client")?;
        let client_norm = client.to_lowercase().trim().to_string();

        // Search for matching client groups via aliases
        let matches = sqlx::query!(
            r#"
            SELECT
                cg.id as group_id,
                cg.canonical_name as "group_name!",
                cga.confidence as "confidence!",
                (cga.alias_norm = $1) as "exact_match!"
            FROM "ob-poc".client_group_alias cga
            JOIN "ob-poc".client_group cg ON cg.id = cga.group_id
            WHERE cga.alias_norm = $1
               OR cga.alias_norm ILIKE '%' || $1 || '%'
               OR similarity(cga.alias_norm, $1) > 0.3
            ORDER BY
                (cga.alias_norm = $1) DESC,
                cga.confidence DESC,
                similarity(cga.alias_norm, $1) DESC
            LIMIT 5
            "#,
            client_norm
        )
        .fetch_all(pool)
        .await?;

        if matches.is_empty() {
            // No match found
            return Ok(dsl_runtime::VerbExecutionOutcome::Record(json!(
                SetClientResult {
                    group_id: None,
                    group_name: None,
                    entity_count: 0,
                    candidates: vec![],
                    resolved: false,
                }
            )));
        }

        // Check if we have a clear winner (exact match or high confidence with gap)
        let top = &matches[0];
        let has_clear_winner = top.exact_match
            || (matches.len() == 1)
            || (matches.len() > 1 && (top.confidence - matches[1].confidence) > 0.10);

        if has_clear_winner {
            // Set the client context in session
            let group_id = top.group_id;
            let group_name = top.group_name.clone();

            // Get entity count for this group
            let entity_count: i64 = sqlx::query_scalar!(
                r#"
                SELECT COUNT(*) as "count!"
                FROM "ob-poc".client_group_entity
                WHERE group_id = $1 AND membership_type != 'historical'
                "#,
                group_id
            )
            .fetch_one(pool)
            .await?;

            // Store in session context via extensions side-channel (keys
            // `client_group_id` / `client_group_name` are read back into the
            // `ExecutionContext` in `to_dsl_context_pub`).
            if !ctx.extensions.is_object() {
                ctx.extensions = serde_json::Value::Object(serde_json::Map::new());
            }
            let ext = ctx.extensions.as_object_mut().unwrap();
            ext.insert(
                "client_group_id".to_string(),
                serde_json::Value::String(group_id.to_string()),
            );
            ext.insert(
                "client_group_name".to_string(),
                serde_json::Value::String(group_name.clone()),
            );

            return Ok(dsl_runtime::VerbExecutionOutcome::Record(json!(
                SetClientResult {
                    group_id: Some(group_id),
                    group_name: Some(group_name),
                    entity_count,
                    candidates: vec![],
                    resolved: true,
                }
            )));
        }

        // Ambiguous - return top 3 candidates for user selection
        let candidates: Vec<ClientGroupCandidate> = matches
            .into_iter()
            .take(3)
            .map(|m| ClientGroupCandidate {
                group_id: m.group_id,
                group_name: m.group_name,
                confidence: m.confidence,
            })
            .collect();

        Ok(dsl_runtime::VerbExecutionOutcome::Record(json!(
            SetClientResult {
                group_id: None,
                group_name: None,
                entity_count: 0,
                candidates,
                resolved: false,
            }
        )))
    }

    fn is_migrated(&self) -> bool {
        true
    }
}


// =============================================================================
// SET-PERSONA (Persona Context for Tag Filtering)
// =============================================================================

#[register_custom_op]
pub struct SessionSetPersonaOp;

#[async_trait]
impl CustomOperation for SessionSetPersonaOp {
    fn domain(&self) -> &'static str {
        "session"
    }

    fn verb(&self) -> &'static str {
        "set-persona"
    }

    fn rationale(&self) -> &'static str {
        "Sets persona context for tag filtering (kyc, trading, ops, onboarding)"
    }
#[cfg(feature = "database")]
    async fn execute_json(
        &self,
        args: &serde_json::Value,
        ctx: &mut dsl_runtime::VerbExecutionContext,
        _pool: &PgPool,
    ) -> Result<dsl_runtime::VerbExecutionOutcome> {
        use super::helpers::json_extract_string;

        let persona = json_extract_string(args, "persona")?;

        // Validate persona
        let valid_personas = ["kyc", "trading", "ops", "onboarding"];
        let persona_lower = persona.to_lowercase();

        if !valid_personas.contains(&persona_lower.as_str()) {
            return Err(anyhow::anyhow!(
                "Invalid persona '{}'. Valid options: {:?}",
                persona,
                valid_personas
            ));
        }

        // Store in session context via extensions side-channel (key `persona`
        // is read back into the `ExecutionContext` in `to_dsl_context_pub`).
        if !ctx.extensions.is_object() {
            ctx.extensions = serde_json::Value::Object(serde_json::Map::new());
        }
        ctx.extensions.as_object_mut().unwrap().insert(
            "persona".to_string(),
            serde_json::Value::String(persona_lower.clone()),
        );

        Ok(dsl_runtime::VerbExecutionOutcome::Record(json!({
            "persona": persona_lower,
            "set": true
        })))
    }

    fn is_migrated(&self) -> bool {
        true
    }
}
// =============================================================================
// SET-STRUCTURE (Macro Expansion Target)
// =============================================================================

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct SetStructureResult {
    pub structure_id: Uuid,
    pub structure_name: String,
    pub structure_type: Option<String>,
}

#[register_custom_op]
pub struct SessionSetStructureOp;

#[async_trait]
impl CustomOperation for SessionSetStructureOp {
    fn domain(&self) -> &'static str {
        "session"
    }

    fn verb(&self) -> &'static str {
        "set-structure"
    }

    fn rationale(&self) -> &'static str {
        "Sets current structure (CBU) context for subsequent operations"
    }
#[cfg(feature = "database")]
    async fn execute_json(
        &self,
        args: &serde_json::Value,
        ctx: &mut dsl_runtime::VerbExecutionContext,
        pool: &PgPool,
    ) -> Result<dsl_runtime::VerbExecutionOutcome> {
        use super::helpers::{
            ext_set_pending_session, ext_take_or_create_pending_session, json_extract_string_opt,
            json_extract_uuid,
        };
        use crate::session::unified::StructureType;

        let structure_id = json_extract_uuid(args, ctx, "structure-id")?;
        let structure_type_str = json_extract_string_opt(args, "structure-type");

        // Fetch CBU details
        let cbu = sqlx::query!(
            r#"SELECT cbu_id, name, jurisdiction
               FROM "ob-poc".cbus
               WHERE cbu_id = $1
                 AND deleted_at IS NULL"#,
            structure_id
        )
        .fetch_optional(pool)
        .await?
        .ok_or_else(|| anyhow::anyhow!("Structure (CBU) not found: {}", structure_id))?;

        // Parse structure type if provided
        let structure_type = structure_type_str
            .as_ref()
            .and_then(|s| StructureType::from_internal(s));

        // Update session context
        let mut session = ext_take_or_create_pending_session(ctx);

        if let Some(st) = structure_type {
            session.set_current_structure(structure_id, cbu.name.clone(), st);
        } else {
            // Default to PE if not specified
            session.set_current_structure(structure_id, cbu.name.clone(), StructureType::Pe);
        }

        // Set DAG state flag
        session.set_dag_flag("structure.selected", true);
        session.set_dag_flag("structure.exists", true);

        ext_set_pending_session(ctx, session);

        let result = SetStructureResult {
            structure_id,
            structure_name: cbu.name,
            structure_type: structure_type_str,
        };

        Ok(dsl_runtime::VerbExecutionOutcome::Record(json!(result)))
    }

    fn is_migrated(&self) -> bool {
        true
    }
}
// =============================================================================
// SET-CASE (Macro Expansion Target)
// =============================================================================

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct SetCaseResult {
    pub case_id: Uuid,
    pub case_reference: String,
    pub status: String,
}

#[register_custom_op]
pub struct SessionSetCaseOp;

#[async_trait]
impl CustomOperation for SessionSetCaseOp {
    fn domain(&self) -> &'static str {
        "session"
    }

    fn verb(&self) -> &'static str {
        "set-case"
    }

    fn rationale(&self) -> &'static str {
        "Sets current KYC case context for subsequent operations"
    }
#[cfg(feature = "database")]
    async fn execute_json(
        &self,
        args: &serde_json::Value,
        ctx: &mut dsl_runtime::VerbExecutionContext,
        pool: &PgPool,
    ) -> Result<dsl_runtime::VerbExecutionOutcome> {
        use super::helpers::{
            ext_set_pending_session, ext_take_or_create_pending_session, json_extract_uuid,
        };

        let case_id = json_extract_uuid(args, ctx, "case-id")?;

        // Fetch KYC case details from "ob-poc".cases
        let case = sqlx::query!(
            r#"SELECT case_id, status, case_type FROM "ob-poc".cases WHERE case_id = $1"#,
            case_id
        )
        .fetch_optional(pool)
        .await?
        .ok_or_else(|| anyhow::anyhow!("KYC case not found: {}", case_id))?;

        // Generate display name from case_id and type
        let display_name = format!(
            "Case {} ({})",
            &case_id.to_string()[..8],
            case.case_type.as_deref().unwrap_or("NEW_CLIENT")
        );

        // Update session context
        let mut session = ext_take_or_create_pending_session(ctx);
        session.set_current_case(case_id, display_name.clone());

        // Set DAG state flags
        session.set_dag_flag("case.selected", true);
        session.set_dag_flag("case.exists", true);

        ext_set_pending_session(ctx, session);

        let result = SetCaseResult {
            case_id,
            case_reference: display_name,
            status: case.status,
        };

        Ok(dsl_runtime::VerbExecutionOutcome::Record(json!(result)))
    }

    fn is_migrated(&self) -> bool {
        true
    }
}
// =============================================================================
// SET-MANDATE (Macro Expansion Target)
// =============================================================================

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct SetMandateResult {
    pub mandate_id: Uuid,
    pub mandate_name: String,
    pub structure_id: Option<Uuid>,
}

#[register_custom_op]
pub struct SessionSetMandateOp;

#[async_trait]
impl CustomOperation for SessionSetMandateOp {
    fn domain(&self) -> &'static str {
        "session"
    }

    fn verb(&self) -> &'static str {
        "set-mandate"
    }

    fn rationale(&self) -> &'static str {
        "Sets current mandate (trading profile) context for subsequent operations"
    }
#[cfg(feature = "database")]
    async fn execute_json(
        &self,
        args: &serde_json::Value,
        ctx: &mut dsl_runtime::VerbExecutionContext,
        pool: &PgPool,
    ) -> Result<dsl_runtime::VerbExecutionOutcome> {
        use super::helpers::{
            ext_set_pending_session, ext_take_or_create_pending_session, json_extract_uuid,
        };

        let mandate_id = json_extract_uuid(args, ctx, "mandate-id")?;

        // Fetch trading profile details from cbu_trading_profiles
        let profile = sqlx::query!(
            r#"SELECT profile_id, cbu_id, status, version FROM "ob-poc".cbu_trading_profiles WHERE profile_id = $1"#,
            mandate_id
        )
        .fetch_optional(pool)
        .await?
        .ok_or_else(|| anyhow::anyhow!("Mandate (trading profile) not found: {}", mandate_id))?;

        // Generate display name from profile_id and version
        let display_name = format!(
            "Profile {} v{}",
            &profile.profile_id.to_string()[..8],
            profile.version
        );

        // Update session context
        let mut session = ext_take_or_create_pending_session(ctx);
        session.set_current_mandate(mandate_id, display_name.clone());

        // Set DAG state flag
        session.set_dag_flag("mandate.selected", true);

        ext_set_pending_session(ctx, session);

        let result = SetMandateResult {
            mandate_id,
            mandate_name: display_name,
            structure_id: profile.cbu_id,
        };

        Ok(dsl_runtime::VerbExecutionOutcome::Record(json!(result)))
    }

    fn is_migrated(&self) -> bool {
        true
    }
}
// =============================================================================
// Deal Taxonomy Navigation Operations
// =============================================================================

/// Result type for load-deal
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct LoadDealResult {
    pub deal_id: Uuid,
    pub deal_name: String,
    pub deal_status: String,
    pub client_group_name: Option<String>,
    pub product_count: i32,
    pub rate_card_count: i32,
}

/// Result type for unload-deal
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct UnloadDealResult {
    pub previous_deal_id: Option<Uuid>,
    pub previous_deal_name: Option<String>,
}

// -----------------------------------------------------------------------------
// session.load-deal
// -----------------------------------------------------------------------------

#[register_custom_op]
pub struct SessionLoadDealOp;

#[async_trait]
impl CustomOperation for SessionLoadDealOp {
    fn domain(&self) -> &'static str {
        "session"
    }

    fn verb(&self) -> &'static str {
        "load-deal"
    }

    fn rationale(&self) -> &'static str {
        "Sets deal context in session for taxonomy visualization"
    }
#[cfg(feature = "database")]
    async fn execute_json(
        &self,
        args: &serde_json::Value,
        ctx: &mut dsl_runtime::VerbExecutionContext,
        pool: &PgPool,
    ) -> Result<dsl_runtime::VerbExecutionOutcome> {
        use super::helpers::{
            ext_set_pending_session, ext_take_or_create_pending_session, json_extract_string_opt,
            json_extract_uuid_opt,
        };

        // Get deal_id from args (either direct UUID or resolved from deal_name)
        let deal_id = json_extract_uuid_opt(args, ctx, "deal-id");
        let deal_name_arg = json_extract_string_opt(args, "deal-name");

        let deal_id = match (deal_id, deal_name_arg) {
            (Some(id), _) => id,
            (None, Some(name)) => {
                // Search for deal by name
                let deal = sqlx::query!(
                    r#"
                    SELECT deal_id FROM "ob-poc".deals
                    WHERE deal_name ILIKE '%' || $1 || '%'
                    ORDER BY
                        CASE WHEN deal_name ILIKE $1 THEN 0
                             WHEN deal_name ILIKE $1 || '%' THEN 1
                             ELSE 2 END,
                        deal_name
                    LIMIT 1
                    "#,
                    name
                )
                .fetch_optional(pool)
                .await?
                .ok_or_else(|| anyhow::anyhow!("No deal found matching: {}", name))?;
                deal.deal_id
            }
            (None, None) => {
                return Err(anyhow::anyhow!(
                    "Either :deal-id or :deal-name must be provided"
                ));
            }
        };

        // Fetch deal details
        let deal = sqlx::query!(
            r#"
            SELECT
                d.deal_id,
                d.deal_name,
                d.deal_status,
                cg.canonical_name as "client_group_name?",
                COALESCE((SELECT COUNT(*) FROM "ob-poc".deal_products WHERE deal_id = d.deal_id), 0)::int as "product_count!",
                COALESCE((SELECT COUNT(*) FROM "ob-poc".deal_rate_cards WHERE deal_id = d.deal_id), 0)::int as "rate_card_count!"
            FROM "ob-poc".deals d
            LEFT JOIN "ob-poc".client_group cg ON cg.id = d.primary_client_group_id
            WHERE d.deal_id = $1
            "#,
            deal_id
        )
        .fetch_optional(pool)
        .await?
        .ok_or_else(|| anyhow::anyhow!("Deal not found: {}", deal_id))?;

        // Update session context
        let mut session = ext_take_or_create_pending_session(ctx);
        session.context.deal_id = Some(deal.deal_id);
        session.context.deal_name = Some(deal.deal_name.clone());
        ext_set_pending_session(ctx, session);

        let result = LoadDealResult {
            deal_id: deal.deal_id,
            deal_name: deal.deal_name,
            deal_status: deal.deal_status,
            client_group_name: deal.client_group_name.clone(),
            product_count: deal.product_count,
            rate_card_count: deal.rate_card_count,
        };

        Ok(dsl_runtime::VerbExecutionOutcome::Record(json!(result)))
    }

    fn is_migrated(&self) -> bool {
        true
    }
}
// -----------------------------------------------------------------------------
// session.unload-deal
// -----------------------------------------------------------------------------

#[register_custom_op]
pub struct SessionUnloadDealOp;

#[async_trait]
impl CustomOperation for SessionUnloadDealOp {
    fn domain(&self) -> &'static str {
        "session"
    }

    fn verb(&self) -> &'static str {
        "unload-deal"
    }

    fn rationale(&self) -> &'static str {
        "Clears deal context from session"
    }
#[cfg(feature = "database")]
    async fn execute_json(
        &self,
        _args: &serde_json::Value,
        ctx: &mut dsl_runtime::VerbExecutionContext,
        _pool: &PgPool,
    ) -> Result<dsl_runtime::VerbExecutionOutcome> {
        use super::helpers::{ext_set_pending_session, ext_take_or_create_pending_session};

        let mut session = ext_take_or_create_pending_session(ctx);

        // Capture previous values before clearing
        let previous_deal_id = session.context.deal_id.take();
        let previous_deal_name = session.context.deal_name.take();

        ext_set_pending_session(ctx, session);

        let result = UnloadDealResult {
            previous_deal_id,
            previous_deal_name,
        };

        Ok(dsl_runtime::VerbExecutionOutcome::Record(json!(result)))
    }

    fn is_migrated(&self) -> bool {
        true
    }
}


// =============================================================================
// REGISTRATION
// =============================================================================
