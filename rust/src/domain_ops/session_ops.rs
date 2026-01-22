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
use serde::{Deserialize, Serialize};
use serde_json::json;
use uuid::Uuid;

use super::CustomOperation;
use crate::dsl_v2::ast::VerbCall;
use crate::dsl_v2::executor::{ExecutionContext, ExecutionResult};
use crate::session::{CbuSummary, ClearResult, HistoryResult, JurisdictionCount, SessionInfo};

#[cfg(feature = "database")]
use sqlx::PgPool;

// =============================================================================
// RESULT TYPES
// =============================================================================

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

        let session = ctx.get_or_create_cbu_session_mut();
        let count_added = session.load_many(cbu_ids);

        let result = LoadUniverseResult {
            count_added,
            total_loaded: session.count(),
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

        let session = ctx.get_or_create_cbu_session_mut();
        let count_added = session.load_many(cbu_ids);

        let result = LoadGalaxyResult {
            jurisdiction,
            count_added,
            total_loaded: session.count(),
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
// LOAD-CLUSTER (all CBUs under a ManCo/governance controller)
// =============================================================================

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
        "Loads all CBUs for a client by client_label (e.g., 'allianz', 'blackrock')"
    }

    #[cfg(feature = "database")]
    async fn execute(
        &self,
        verb_call: &VerbCall,
        ctx: &mut ExecutionContext,
        pool: &PgPool,
    ) -> Result<ExecutionResult> {
        let client = get_required_string(verb_call, "client")?;
        let jurisdiction = get_optional_string(verb_call, "jurisdiction");

        // Normalize client label to lowercase for matching
        let client_label = client.to_lowercase();

        // Query directly on cbus.client_label (no join needed)
        let cbu_ids: Vec<Uuid> = sqlx::query_scalar!(
            r#"
            SELECT cbu_id as "cbu_id!"
            FROM "ob-poc".cbus
            WHERE LOWER(client_label) = $1
              AND ($2::text IS NULL OR jurisdiction = $2)
            "#,
            client_label,
            jurisdiction.as_deref()
        )
        .fetch_all(pool)
        .await?;

        if cbu_ids.is_empty() {
            return Err(anyhow::anyhow!(
                "No CBUs found for client '{}'{}",
                client,
                jurisdiction
                    .as_ref()
                    .map(|j| format!(" in jurisdiction {}", j))
                    .unwrap_or_default()
            ));
        }

        let session = ctx.get_or_create_cbu_session_mut();
        let count_added = session.load_many(cbu_ids);

        let result = LoadClusterResult {
            manco_name: client.clone(),   // Use client label as the name
            manco_entity_id: Uuid::nil(), // No single entity ID anymore
            jurisdiction,
            count_added,
            total_loaded: session.count(),
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

        let session = ctx.get_or_create_cbu_session_mut();
        let was_new = session.load_cbu(cbu_id);

        let result = LoadSystemResult {
            cbu_id,
            name: cbu.name,
            jurisdiction: cbu.jurisdiction,
            total_loaded: session.count(),
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

        let session = ctx.get_or_create_cbu_session_mut();
        let was_present = session.unload_cbu(cbu_id);

        let result = UnloadSystemResult {
            cbu_id,
            name,
            total_loaded: session.count(),
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

        let session = ctx.get_or_create_cbu_session_mut();
        let before_count = session.count();
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

        session.clear();
        let count_kept = session.load_many(matching_cbu_ids);
        let count_removed = before_count - count_kept;

        let result = FilterJurisdictionResult {
            jurisdiction,
            count_kept,
            count_removed,
            total_loaded: session.count(),
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
        let session = ctx.get_or_create_cbu_session_mut();
        let count_removed = session.clear();
        Ok(ExecutionResult::Record(json!(ClearResult {
            count_removed
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
        let session = ctx.get_or_create_cbu_session_mut();
        let success = session.undo();

        let result = HistoryResult {
            success,
            total_loaded: session.count(),
            history_depth: session.history_depth(),
            future_depth: session.future_depth(),
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
        let session = ctx.get_or_create_cbu_session_mut();
        let success = session.redo();

        let result = HistoryResult {
            success,
            total_loaded: session.count(),
            history_depth: session.history_depth(),
            future_depth: session.future_depth(),
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
        let session = ctx.get_or_create_cbu_session_mut();
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
            total_cbus: session.count(),
            jurisdictions,
            history_depth: session.history_depth(),
            future_depth: session.future_depth(),
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

        let session = ctx.get_or_create_cbu_session_mut();
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
            "total_in_session": session.count()
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

/// Register all session operations with the registry (Astro model)
pub fn register_session_ops_v2(registry: &mut crate::domain_ops::CustomOperationRegistry) {
    use std::sync::Arc;

    // Load verbs (Universe → Galaxy → Cluster → System)
    registry.register(Arc::new(SessionLoadUniverseOp));
    registry.register(Arc::new(SessionLoadGalaxyOp));
    registry.register(Arc::new(SessionLoadClusterOp));
    registry.register(Arc::new(SessionLoadSystemOp));

    // Unload/filter
    registry.register(Arc::new(SessionUnloadSystemOp));
    registry.register(Arc::new(SessionFilterJurisdictionOp));
    registry.register(Arc::new(SessionClearOp));

    // History
    registry.register(Arc::new(SessionUndoOp));
    registry.register(Arc::new(SessionRedoOp));

    // Query
    registry.register(Arc::new(SessionInfoOp));
    registry.register(Arc::new(SessionListOp));
}
