//! Simplified Session Operations (Phase 6)
//!
//! 9 verbs instead of 20. Memory is truth, DB is backup.
//!
//! # Verbs
//!
//! - `session.load-cbu` - Load a CBU by ID
//! - `session.load-jurisdiction` - Load all CBUs in a jurisdiction
//! - `session.load-galaxy` - Load all CBUs under an apex entity
//! - `session.unload-cbu` - Remove a CBU from session
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
use serde_json::json;
use uuid::Uuid;

use super::CustomOperation;
use crate::dsl_v2::ast::VerbCall;
use crate::dsl_v2::executor::{ExecutionContext, ExecutionResult};
use crate::session::{
    CbuSummary, ClearResult, HistoryResult, JurisdictionCount, LoadCbuResult, LoadGalaxyResult,
    LoadJurisdictionResult, SessionInfo, UnloadCbuResult,
};

#[cfg(feature = "database")]
use sqlx::PgPool;

// =============================================================================
// HELPER FUNCTIONS
// =============================================================================

/// Extract a required UUID argument from verb call
#[cfg(feature = "database")]
fn get_required_uuid(verb_call: &VerbCall, key: &str, ctx: &ExecutionContext) -> Result<Uuid> {
    let arg = verb_call
        .arguments
        .iter()
        .find(|a| a.key == key)
        .ok_or_else(|| anyhow::anyhow!("Missing required argument :{}", key))?;

    // Try as symbol reference first
    if let Some(ref_name) = arg.value.as_symbol() {
        let resolved = ctx
            .resolve(ref_name)
            .ok_or_else(|| anyhow::anyhow!("Unresolved reference @{}", ref_name))?;
        return Ok(resolved);
    }

    // Try as UUID directly
    if let Some(uuid_val) = arg.value.as_uuid() {
        return Ok(uuid_val);
    }

    // Try as string (may be UUID string)
    if let Some(str_val) = arg.value.as_string() {
        return Uuid::parse_str(str_val)
            .map_err(|e| anyhow::anyhow!("Invalid UUID for :{}: {}", key, e));
    }

    Err(anyhow::anyhow!(":{} must be a UUID or @reference", key))
}

/// Extract an optional string argument from verb call
#[cfg(feature = "database")]
fn get_optional_string(verb_call: &VerbCall, key: &str) -> Option<String> {
    verb_call
        .arguments
        .iter()
        .find(|a| a.key == key)
        .and_then(|a| a.value.as_string().map(|s| s.to_string()))
}

/// Extract a required string argument from verb call
#[cfg(feature = "database")]
fn get_required_string(verb_call: &VerbCall, key: &str) -> Result<String> {
    get_optional_string(verb_call, key)
        .ok_or_else(|| anyhow::anyhow!("Missing required argument :{}", key))
}

/// Extract an optional integer argument from verb call
#[cfg(feature = "database")]
fn get_optional_integer(verb_call: &VerbCall, key: &str) -> Option<i64> {
    verb_call
        .arguments
        .iter()
        .find(|a| a.key == key)
        .and_then(|a| a.value.as_integer())
}

// =============================================================================
// LOAD-CBU
// =============================================================================

/// Load a single CBU into the session
pub struct SessionLoadCbuOp;

#[async_trait]
impl CustomOperation for SessionLoadCbuOp {
    fn domain(&self) -> &'static str {
        "session"
    }

    fn verb(&self) -> &'static str {
        "load-cbu"
    }

    fn rationale(&self) -> &'static str {
        "Loads a CBU into the session by ID"
    }

    #[cfg(feature = "database")]
    async fn execute(
        &self,
        verb_call: &VerbCall,
        ctx: &mut ExecutionContext,
        pool: &PgPool,
    ) -> Result<ExecutionResult> {
        let cbu_id = get_required_uuid(verb_call, "cbu-id", ctx)?;

        // Fetch CBU details from DB
        let cbu = sqlx::query!(
            r#"
            SELECT cbu_id, name, jurisdiction
            FROM "ob-poc".cbus
            WHERE cbu_id = $1
            "#,
            cbu_id
        )
        .fetch_optional(pool)
        .await?
        .ok_or_else(|| anyhow::anyhow!("CBU not found: {}", cbu_id))?;

        // Get or create session from context
        // NOTE: Session is stored in pending_session field and propagated by caller
        let session = ctx.get_or_create_cbu_session_mut();
        let was_new = session.load_cbu(cbu_id);

        let result = LoadCbuResult {
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
// LOAD-JURISDICTION
// =============================================================================

/// Load all CBUs in a jurisdiction
pub struct SessionLoadJurisdictionOp;

#[async_trait]
impl CustomOperation for SessionLoadJurisdictionOp {
    fn domain(&self) -> &'static str {
        "session"
    }

    fn verb(&self) -> &'static str {
        "load-jurisdiction"
    }

    fn rationale(&self) -> &'static str {
        "Loads all CBUs in a jurisdiction into the session"
    }

    #[cfg(feature = "database")]
    async fn execute(
        &self,
        verb_call: &VerbCall,
        ctx: &mut ExecutionContext,
        pool: &PgPool,
    ) -> Result<ExecutionResult> {
        let jurisdiction = get_required_string(verb_call, "jurisdiction")?;

        // Fetch all CBU IDs in the jurisdiction
        let cbu_ids: Vec<Uuid> = sqlx::query_scalar!(
            r#"
            SELECT cbu_id
            FROM "ob-poc".cbus
            WHERE jurisdiction = $1
            "#,
            jurisdiction
        )
        .fetch_all(pool)
        .await?;

        let session = ctx.get_or_create_cbu_session_mut();
        let count_added = session.load_many(cbu_ids);

        let result = LoadJurisdictionResult {
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
// LOAD-GALAXY
// =============================================================================

/// Load all CBUs under an apex entity
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
        "Loads all CBUs under an apex entity (commercial client group)"
    }

    #[cfg(feature = "database")]
    async fn execute(
        &self,
        verb_call: &VerbCall,
        ctx: &mut ExecutionContext,
        pool: &PgPool,
    ) -> Result<ExecutionResult> {
        let apex_entity_id = get_required_uuid(verb_call, "apex-entity-id", ctx)?;

        // Get apex name
        let apex_name: String = sqlx::query_scalar!(
            r#"
            SELECT name
            FROM "ob-poc".entities
            WHERE entity_id = $1
            "#,
            apex_entity_id
        )
        .fetch_optional(pool)
        .await?
        .ok_or_else(|| anyhow::anyhow!("Apex entity not found: {}", apex_entity_id))?;

        // Find all CBUs under this apex via commercial_client_entity_id
        let cbu_ids: Vec<Uuid> = sqlx::query_scalar!(
            r#"
            SELECT cbu_id
            FROM "ob-poc".cbus
            WHERE commercial_client_entity_id = $1
            "#,
            apex_entity_id
        )
        .fetch_all(pool)
        .await?;

        let session = ctx.get_or_create_cbu_session_mut();
        let count_added = session.load_many(cbu_ids);

        let result = LoadGalaxyResult {
            apex_name,
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
// UNLOAD-CBU
// =============================================================================

/// Unload a CBU from the session
pub struct SessionUnloadCbuOp;

#[async_trait]
impl CustomOperation for SessionUnloadCbuOp {
    fn domain(&self) -> &'static str {
        "session"
    }

    fn verb(&self) -> &'static str {
        "unload-cbu"
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

        // Get CBU name for response (optional, may not exist)
        let name: String = sqlx::query_scalar!(
            r#"SELECT name FROM "ob-poc".cbus WHERE cbu_id = $1"#,
            cbu_id
        )
        .fetch_optional(pool)
        .await?
        .unwrap_or_default();

        let session = ctx.get_or_create_cbu_session_mut();
        let was_present = session.unload_cbu(cbu_id);

        let result = UnloadCbuResult {
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
// CLEAR
// =============================================================================

/// Clear all CBUs from the session
pub struct SessionClearOp2;

#[async_trait]
impl CustomOperation for SessionClearOp2 {
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

        let result = ClearResult { count_removed };

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
// UNDO
// =============================================================================

/// Undo last session action
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

/// Redo previously undone action
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

/// Get session info
pub struct SessionInfoOp2;

#[async_trait]
impl CustomOperation for SessionInfoOp2 {
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

        // Get jurisdiction breakdown
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

/// List CBUs in the session
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

/// Register all v2 session operations with the registry
pub fn register_session_ops_v2(registry: &mut crate::dsl_v2::custom_ops::CustomOperationRegistry) {
    use std::sync::Arc;

    registry.register(Arc::new(SessionLoadCbuOp));
    registry.register(Arc::new(SessionLoadJurisdictionOp));
    registry.register(Arc::new(SessionLoadGalaxyOp));
    registry.register(Arc::new(SessionUnloadCbuOp));
    registry.register(Arc::new(SessionClearOp2));
    registry.register(Arc::new(SessionUndoOp));
    registry.register(Arc::new(SessionRedoOp));
    registry.register(Arc::new(SessionInfoOp2));
    registry.register(Arc::new(SessionListOp));
}
