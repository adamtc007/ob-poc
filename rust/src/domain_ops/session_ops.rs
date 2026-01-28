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
fn get_optional_integer(verb_call: &VerbCall, key: &str) -> Option<i64> {
    verb_call
        .arguments
        .iter()
        .find(|a| a.key == key)
        .and_then(|a| a.value.as_integer())
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
    async fn execute(
        &self,
        verb_call: &VerbCall,
        ctx: &mut ExecutionContext,
        pool: &PgPool,
    ) -> Result<ExecutionResult> {
        let client_id = get_optional_uuid(verb_call, "client-id", ctx);

        // Fetch all CBU IDs (optionally filtered by client/apex entity)
        let cbu_ids: Vec<Uuid> = if let Some(client_id) = client_id {
            sqlx::query_scalar!(
                r#"
                SELECT DISTINCT c.cbu_id as "cbu_id!"
                FROM "ob-poc".cbus c
                LEFT JOIN "ob-poc".cbu_groups g ON g.manco_entity_id = $1
                LEFT JOIN "ob-poc".cbu_group_members gm ON gm.group_id = g.group_id AND gm.cbu_id = c.cbu_id
                WHERE c.commercial_client_entity_id = $1
                   OR gm.cbu_id IS NOT NULL
                "#,
                client_id
            )
            .fetch_all(pool)
            .await?
        } else {
            sqlx::query_scalar!(r#"SELECT cbu_id as "cbu_id!" FROM "ob-poc".cbus"#)
                .fetch_all(pool)
                .await?
        };

        let session = ctx.get_or_create_session_mut();
        let count_added = session.load_cbus(cbu_ids);

        let result = LoadUniverseResult {
            count_added,
            total_loaded: session.cbu_count(),
        };

        Ok(ExecutionResult::Record(json!(result)))
    }

    #[cfg(not(feature = "database"))]
    async fn execute(
        &self,
        _verb_call: &VerbCall,
        _ctx: &mut ExecutionContext,
    ) -> Result<ExecutionResult> {
        Err(anyhow::anyhow!(
            "Database feature required for session operations"
        ))
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
    async fn execute(
        &self,
        verb_call: &VerbCall,
        ctx: &mut ExecutionContext,
        pool: &PgPool,
    ) -> Result<ExecutionResult> {
        let jurisdiction = get_required_string(verb_call, "jurisdiction")?;

        let cbu_ids: Vec<Uuid> = sqlx::query_scalar!(
            r#"SELECT cbu_id as "cbu_id!" FROM "ob-poc".cbus WHERE jurisdiction = $1"#,
            jurisdiction
        )
        .fetch_all(pool)
        .await?;

        let session = ctx.get_or_create_session_mut();
        let count_added = session.load_cbus(cbu_ids);

        let result = LoadGalaxyResult {
            jurisdiction,
            count_added,
            total_loaded: session.cbu_count(),
        };

        Ok(ExecutionResult::Record(json!(result)))
    }

    #[cfg(not(feature = "database"))]
    async fn execute(
        &self,
        _verb_call: &VerbCall,
        _ctx: &mut ExecutionContext,
    ) -> Result<ExecutionResult> {
        Err(anyhow::anyhow!(
            "Database feature required for session operations"
        ))
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
    async fn execute(
        &self,
        verb_call: &VerbCall,
        ctx: &mut ExecutionContext,
        pool: &PgPool,
    ) -> Result<ExecutionResult> {
        let jurisdiction = get_optional_string(verb_call, "jurisdiction");

        // Two-stage resolution: :client → client_group_id → anchor_entity_id
        // OR direct :apex-entity-id
        let apex_entity_id: Uuid = if let Some(client_group_id) =
            get_optional_uuid(verb_call, "client", ctx)
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
            get_required_uuid(verb_call, "apex-entity-id", ctx)?
        };

        // Get apex entity name for response
        let apex_name: String = sqlx::query_scalar!(
            r#"SELECT name as "name!" FROM "ob-poc".entities WHERE entity_id = $1"#,
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
        let client_group_id: Option<Uuid> = get_optional_uuid(verb_call, "client", ctx);

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

        let session = ctx.get_or_create_session_mut();
        session.name = Some(format!("{} Book", apex_name));
        let count_added = session.load_cbus(cbu_ids);

        let result = LoadClusterResult {
            manco_name: apex_name,
            manco_entity_id: apex_entity_id,
            jurisdiction,
            count_added,
            total_loaded: session.cbu_count(),
        };

        Ok(ExecutionResult::Record(json!(result)))
    }

    #[cfg(not(feature = "database"))]
    async fn execute(
        &self,
        _verb_call: &VerbCall,
        _ctx: &mut ExecutionContext,
    ) -> Result<ExecutionResult> {
        Err(anyhow::anyhow!(
            "Database feature required for session operations"
        ))
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
    async fn execute(
        &self,
        verb_call: &VerbCall,
        ctx: &mut ExecutionContext,
        pool: &PgPool,
    ) -> Result<ExecutionResult> {
        let cbu_id = get_required_uuid(verb_call, "cbu-id", ctx)?;

        let cbu = sqlx::query!(
            r#"SELECT cbu_id, name, jurisdiction FROM "ob-poc".cbus WHERE cbu_id = $1"#,
            cbu_id
        )
        .fetch_optional(pool)
        .await?
        .ok_or_else(|| anyhow::anyhow!("CBU not found: {}", cbu_id))?;

        let session = ctx.get_or_create_session_mut();
        let was_new = session.load_cbu(cbu_id);

        let result = LoadSystemResult {
            cbu_id,
            name: cbu.name,
            jurisdiction: cbu.jurisdiction,
            total_loaded: session.cbu_count(),
            was_new,
        };

        Ok(ExecutionResult::Record(json!(result)))
    }

    #[cfg(not(feature = "database"))]
    async fn execute(
        &self,
        _verb_call: &VerbCall,
        _ctx: &mut ExecutionContext,
    ) -> Result<ExecutionResult> {
        Err(anyhow::anyhow!(
            "Database feature required for session operations"
        ))
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
    async fn execute(
        &self,
        verb_call: &VerbCall,
        ctx: &mut ExecutionContext,
        pool: &PgPool,
    ) -> Result<ExecutionResult> {
        let cbu_id = get_required_uuid(verb_call, "cbu-id", ctx)?;

        let name: String = sqlx::query_scalar!(
            r#"SELECT name FROM "ob-poc".cbus WHERE cbu_id = $1"#,
            cbu_id
        )
        .fetch_optional(pool)
        .await?
        .unwrap_or_default();

        let session = ctx.get_or_create_session_mut();
        let was_present = session.unload_cbu(cbu_id);

        let result = UnloadSystemResult {
            cbu_id,
            name,
            total_loaded: session.cbu_count(),
            was_present,
        };

        Ok(ExecutionResult::Record(json!(result)))
    }

    #[cfg(not(feature = "database"))]
    async fn execute(
        &self,
        _verb_call: &VerbCall,
        _ctx: &mut ExecutionContext,
    ) -> Result<ExecutionResult> {
        Err(anyhow::anyhow!(
            "Database feature required for session operations"
        ))
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
    async fn execute(
        &self,
        verb_call: &VerbCall,
        ctx: &mut ExecutionContext,
        pool: &PgPool,
    ) -> Result<ExecutionResult> {
        let jurisdiction = get_required_string(verb_call, "jurisdiction")?;

        let session = ctx.get_or_create_session_mut();
        let before_count = session.cbu_count();
        let current_cbu_ids = session.cbu_ids_vec();

        if current_cbu_ids.is_empty() {
            return Ok(ExecutionResult::Record(json!(FilterJurisdictionResult {
                jurisdiction,
                count_kept: 0,
                count_removed: 0,
                total_loaded: 0,
            })));
        }

        let matching_cbu_ids: Vec<Uuid> = sqlx::query_scalar!(
            r#"SELECT cbu_id as "cbu_id!" FROM "ob-poc".cbus WHERE cbu_id = ANY($1) AND jurisdiction = $2"#,
            &current_cbu_ids,
            &jurisdiction
        )
        .fetch_all(pool)
        .await?;

        session.clear_cbus_with_history();
        let count_kept = session.load_cbus(matching_cbu_ids);
        let count_removed = before_count - count_kept;

        let result = FilterJurisdictionResult {
            jurisdiction,
            count_kept,
            count_removed,
            total_loaded: session.cbu_count(),
        };

        Ok(ExecutionResult::Record(json!(result)))
    }

    #[cfg(not(feature = "database"))]
    async fn execute(
        &self,
        _verb_call: &VerbCall,
        _ctx: &mut ExecutionContext,
    ) -> Result<ExecutionResult> {
        Err(anyhow::anyhow!(
            "Database feature required for session operations"
        ))
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
    async fn execute(
        &self,
        _verb_call: &VerbCall,
        ctx: &mut ExecutionContext,
        _pool: &PgPool,
    ) -> Result<ExecutionResult> {
        let session = ctx.get_or_create_session_mut();
        let count = session.clear_cbus_with_history();
        Ok(ExecutionResult::Record(json!(ClearResult {
            cleared: true,
            count
        })))
    }

    #[cfg(not(feature = "database"))]
    async fn execute(
        &self,
        _verb_call: &VerbCall,
        _ctx: &mut ExecutionContext,
    ) -> Result<ExecutionResult> {
        Err(anyhow::anyhow!(
            "Database feature required for session operations"
        ))
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
    async fn execute(
        &self,
        _verb_call: &VerbCall,
        ctx: &mut ExecutionContext,
        _pool: &PgPool,
    ) -> Result<ExecutionResult> {
        let session = ctx.get_or_create_session_mut();
        let success = session.undo_cbu();

        let result = HistoryResult {
            success,
            scope_size: session.cbu_count(),
            history_depth: session.cbu_history_depth(),
            future_depth: session.cbu_future_depth(),
        };

        Ok(ExecutionResult::Record(json!(result)))
    }

    #[cfg(not(feature = "database"))]
    async fn execute(
        &self,
        _verb_call: &VerbCall,
        _ctx: &mut ExecutionContext,
    ) -> Result<ExecutionResult> {
        Err(anyhow::anyhow!(
            "Database feature required for session operations"
        ))
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
    async fn execute(
        &self,
        _verb_call: &VerbCall,
        ctx: &mut ExecutionContext,
        _pool: &PgPool,
    ) -> Result<ExecutionResult> {
        let session = ctx.get_or_create_session_mut();
        let success = session.redo_cbu();

        let result = HistoryResult {
            success,
            scope_size: session.cbu_count(),
            history_depth: session.cbu_history_depth(),
            future_depth: session.cbu_future_depth(),
        };

        Ok(ExecutionResult::Record(json!(result)))
    }

    #[cfg(not(feature = "database"))]
    async fn execute(
        &self,
        _verb_call: &VerbCall,
        _ctx: &mut ExecutionContext,
    ) -> Result<ExecutionResult> {
        Err(anyhow::anyhow!(
            "Database feature required for session operations"
        ))
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
    async fn execute(
        &self,
        _verb_call: &VerbCall,
        ctx: &mut ExecutionContext,
        pool: &PgPool,
    ) -> Result<ExecutionResult> {
        let session = ctx.get_or_create_session_mut();
        let cbu_ids = session.cbu_ids_vec();

        let jurisdictions: Vec<JurisdictionCount> = if cbu_ids.is_empty() {
            vec![]
        } else {
            sqlx::query!(
                r#"
                SELECT jurisdiction, COUNT(*) as count
                FROM "ob-poc".cbus
                WHERE cbu_id = ANY($1)
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

        Ok(ExecutionResult::Record(json!(result)))
    }

    #[cfg(not(feature = "database"))]
    async fn execute(
        &self,
        _verb_call: &VerbCall,
        _ctx: &mut ExecutionContext,
    ) -> Result<ExecutionResult> {
        Err(anyhow::anyhow!(
            "Database feature required for session operations"
        ))
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
    async fn execute(
        &self,
        verb_call: &VerbCall,
        ctx: &mut ExecutionContext,
        pool: &PgPool,
    ) -> Result<ExecutionResult> {
        let limit = get_optional_integer(verb_call, "limit").unwrap_or(100) as i64;
        let jurisdiction_filter = get_optional_string(verb_call, "jurisdiction");

        let session = ctx.get_or_create_session_mut();
        let cbu_ids = session.cbu_ids_vec();

        let cbus: Vec<CbuSummary> = if cbu_ids.is_empty() {
            vec![]
        } else {
            sqlx::query_as!(
                CbuSummary,
                r#"
                SELECT cbu_id, name, jurisdiction
                FROM "ob-poc".cbus
                WHERE cbu_id = ANY($1)
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

        Ok(ExecutionResult::Record(json!({
            "cbus": cbus,
            "count": cbus.len(),
            "total_in_session": session.cbu_count()
        })))
    }

    #[cfg(not(feature = "database"))]
    async fn execute(
        &self,
        _verb_call: &VerbCall,
        _ctx: &mut ExecutionContext,
    ) -> Result<ExecutionResult> {
        Err(anyhow::anyhow!(
            "Database feature required for session operations"
        ))
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
    async fn execute(
        &self,
        verb_call: &VerbCall,
        ctx: &mut ExecutionContext,
        pool: &PgPool,
    ) -> Result<ExecutionResult> {
        let client = get_required_string(verb_call, "client")?;
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
            return Ok(ExecutionResult::Record(json!(SetClientResult {
                group_id: None,
                group_name: None,
                entity_count: 0,
                candidates: vec![],
                resolved: false,
            })));
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

            // Store in session context
            ctx.set_client_group_id(Some(group_id));
            ctx.set_client_group_name(Some(group_name.clone()));

            return Ok(ExecutionResult::Record(json!(SetClientResult {
                group_id: Some(group_id),
                group_name: Some(group_name),
                entity_count,
                candidates: vec![],
                resolved: true,
            })));
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

        Ok(ExecutionResult::Record(json!(SetClientResult {
            group_id: None,
            group_name: None,
            entity_count: 0,
            candidates,
            resolved: false,
        })))
    }

    #[cfg(not(feature = "database"))]
    async fn execute(
        &self,
        _verb_call: &VerbCall,
        _ctx: &mut ExecutionContext,
    ) -> Result<ExecutionResult> {
        Err(anyhow::anyhow!(
            "Database feature required for session operations"
        ))
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
    async fn execute(
        &self,
        verb_call: &VerbCall,
        ctx: &mut ExecutionContext,
        _pool: &PgPool,
    ) -> Result<ExecutionResult> {
        let persona = get_required_string(verb_call, "persona")?;

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

        // Store in session context
        ctx.set_persona(Some(persona_lower.clone()));

        Ok(ExecutionResult::Record(json!({
            "persona": persona_lower,
            "set": true
        })))
    }

    #[cfg(not(feature = "database"))]
    async fn execute(
        &self,
        _verb_call: &VerbCall,
        _ctx: &mut ExecutionContext,
    ) -> Result<ExecutionResult> {
        Err(anyhow::anyhow!(
            "Database feature required for session operations"
        ))
    }
}

// =============================================================================
// REGISTRATION
// =============================================================================
