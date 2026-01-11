//! Session scope management plugin operations.
//!
//! Controls the session scope (what data the user is viewing/operating on):
//! - Galaxy: All CBUs under an apex entity
//! - Book: Filtered subset of a galaxy
//! - CBU: Single CBU focus
//! - Jurisdiction: All CBUs in a jurisdiction
//! - Neighborhood: N hops from a focal entity
//!
//! Scope changes trigger viewport rebuild and agent context refresh.

use anyhow::Result;
use async_trait::async_trait;
use serde::{Deserialize, Serialize};
use serde_json::json;
use uuid::Uuid;

use super::CustomOperation;
use crate::dsl_v2::ast::VerbCall;
use crate::dsl_v2::executor::{ExecutionContext, ExecutionResult};
use crate::graph::GraphScope;

#[cfg(feature = "database")]
use sqlx::PgPool;

// ============================================================================
// Session Scope State Struct
// ============================================================================

/// Session scope state returned from navigation operations.
/// Uses Option<T> for all nullable database fields.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct SessionScopeState {
    pub scope_type: Option<String>,
    pub apex_entity_id: Option<Uuid>,
    pub apex_entity_name: Option<String>,
    pub cbu_id: Option<Uuid>,
    pub cbu_name: Option<String>,
    pub jurisdiction_code: Option<String>,
    pub focal_entity_id: Option<Uuid>,
    pub focal_entity_name: Option<String>,
    pub neighborhood_hops: Option<i32>,
    pub history_position: Option<i32>,
    pub active_cbu_ids: Option<Vec<Uuid>>,
}

impl SessionScopeState {
    /// Convert to GraphScope enum for pending scope change
    pub fn to_graph_scope(&self) -> GraphScope {
        match self.scope_type.as_deref() {
            Some("galaxy") | Some("book") => match (&self.apex_entity_id, &self.apex_entity_name) {
                (Some(id), Some(name)) => GraphScope::Book {
                    apex_entity_id: *id,
                    apex_name: name.clone(),
                },
                _ => GraphScope::Empty,
            },
            Some("cbu") => match (&self.cbu_id, &self.cbu_name) {
                (Some(id), Some(name)) => GraphScope::SingleCbu {
                    cbu_id: *id,
                    cbu_name: name.clone(),
                },
                _ => GraphScope::Empty,
            },
            Some("jurisdiction") => match &self.jurisdiction_code {
                Some(code) => GraphScope::Jurisdiction { code: code.clone() },
                None => GraphScope::Empty,
            },
            Some("neighborhood") => match &self.focal_entity_id {
                Some(id) => GraphScope::EntityNeighborhood {
                    entity_id: *id,
                    hops: self.neighborhood_hops.unwrap_or(2) as u32,
                },
                None => GraphScope::Empty,
            },
            _ => GraphScope::Empty,
        }
    }

    /// Check if currently in history (not at the latest scope)
    pub fn is_in_history(&self) -> bool {
        self.history_position.map(|p| p >= 0).unwrap_or(false)
    }

    /// Check if at the end of history (latest scope)
    pub fn is_at_end(&self) -> bool {
        self.history_position.map(|p| p < 0).unwrap_or(true)
    }

    /// Get active CBU count
    pub fn active_cbu_count(&self) -> usize {
        self.active_cbu_ids.as_ref().map(|v| v.len()).unwrap_or(0)
    }

    /// Convert to JSON response for back navigation
    pub fn to_back_response(&self, navigated: bool) -> serde_json::Value {
        json!({
            "success": true,
            "navigated": navigated,
            "scope_type": self.scope_type,
            "apex_entity_name": self.apex_entity_name,
            "cbu_name": self.cbu_name,
            "jurisdiction_code": self.jurisdiction_code,
            "focal_entity_name": self.focal_entity_name,
            "history_position": self.history_position,
            "message": if navigated { "Navigated back" } else { "Already at oldest history entry" },
            "active_cbu_count": self.active_cbu_count()
        })
    }

    /// Convert to JSON response for forward navigation
    pub fn to_forward_response(&self, navigated: bool) -> serde_json::Value {
        json!({
            "success": true,
            "navigated": navigated,
            "scope_type": self.scope_type,
            "apex_entity_name": self.apex_entity_name,
            "cbu_name": self.cbu_name,
            "jurisdiction_code": self.jurisdiction_code,
            "focal_entity_name": self.focal_entity_name,
            "history_position": self.history_position,
            "at_end": self.is_at_end(),
            "message": if navigated { "Navigated forward" } else { "Already at latest scope" },
            "active_cbu_count": self.active_cbu_count()
        })
    }
}

// ============================================================================
// Helper Functions
// ============================================================================

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
fn get_optional_integer(verb_call: &VerbCall, key: &str) -> Option<i32> {
    verb_call
        .arguments
        .iter()
        .find(|a| a.key == key)
        .and_then(|a| a.value.as_integer().map(|i| i as i32))
}

/// Extract string list argument from verb call
#[cfg(feature = "database")]
fn get_string_list(verb_call: &VerbCall, key: &str) -> Option<Vec<String>> {
    verb_call
        .arguments
        .iter()
        .find(|a| a.key == key)
        .and_then(|a| {
            a.value.as_list().map(|list| {
                list.iter()
                    .filter_map(|node| node.as_string().map(|s| s.to_string()))
                    .collect()
            })
        })
}

/// Get session ID from context, or generate a new one
#[cfg(feature = "database")]
fn get_session_id(ctx: &ExecutionContext) -> Uuid {
    ctx.session_id.unwrap_or_else(Uuid::new_v4)
}

// ============================================================================
// Scope Setting Operations
// ============================================================================

/// Set scope to galaxy (all CBUs under apex entity)
pub struct SessionSetGalaxyOp;

#[async_trait]
impl CustomOperation for SessionSetGalaxyOp {
    fn domain(&self) -> &'static str {
        "session"
    }

    fn verb(&self) -> &'static str {
        "set-galaxy"
    }

    fn rationale(&self) -> &'static str {
        "Sets session scope to all CBUs under an apex entity (commercial client)"
    }

    #[cfg(feature = "database")]
    async fn execute(
        &self,
        verb_call: &VerbCall,
        ctx: &mut ExecutionContext,
        pool: &PgPool,
    ) -> Result<ExecutionResult> {
        let session_id = get_session_id(ctx);
        let apex_entity_id = get_required_uuid(verb_call, "apex-entity-id", ctx)?;

        // Call the database function to set galaxy scope
        let row = sqlx::query!(
            r#"SELECT * FROM "ob-poc".set_scope_galaxy($1, $2)"#,
            session_id,
            apex_entity_id
        )
        .fetch_one(pool)
        .await
        .map_err(|e| anyhow::anyhow!("Failed to set galaxy scope: {}", e))?;

        // Set pending scope change for propagation to session layer
        let apex_name = row.apex_entity_name.clone().unwrap_or_default();
        ctx.set_pending_scope_change(GraphScope::Book {
            apex_entity_id,
            apex_name: apex_name.clone(),
        });

        Ok(ExecutionResult::Record(json!({
            "scope_type": "galaxy",
            "apex_entity_id": apex_entity_id,
            "apex_entity_name": apex_name,
            "total_cbus": row.total_cbus,
            "total_entities": row.total_entities,
            "session_id": session_id
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

/// Set scope to book (filtered subset of galaxy)
pub struct SessionSetBookOp;

#[async_trait]
impl CustomOperation for SessionSetBookOp {
    fn domain(&self) -> &'static str {
        "session"
    }

    fn verb(&self) -> &'static str {
        "set-book"
    }

    fn rationale(&self) -> &'static str {
        "Sets session scope to filtered subset of a galaxy"
    }

    #[cfg(feature = "database")]
    async fn execute(
        &self,
        verb_call: &VerbCall,
        ctx: &mut ExecutionContext,
        pool: &PgPool,
    ) -> Result<ExecutionResult> {
        let session_id = get_session_id(ctx);
        let apex_entity_id = get_required_uuid(verb_call, "apex-entity-id", ctx)?;

        // Build filters JSON
        let mut filters = serde_json::Map::new();
        if let Some(jurisdictions) = get_string_list(verb_call, "jurisdictions") {
            filters.insert(
                "jurisdictions".to_string(),
                serde_json::Value::Array(
                    jurisdictions
                        .into_iter()
                        .map(serde_json::Value::String)
                        .collect(),
                ),
            );
        }
        if let Some(entity_types) = get_string_list(verb_call, "entity-types") {
            filters.insert(
                "entity_types".to_string(),
                serde_json::Value::Array(
                    entity_types
                        .into_iter()
                        .map(serde_json::Value::String)
                        .collect(),
                ),
            );
        }
        if let Some(cbu_types) = get_string_list(verb_call, "cbu-types") {
            filters.insert(
                "cbu_types".to_string(),
                serde_json::Value::Array(
                    cbu_types
                        .into_iter()
                        .map(serde_json::Value::String)
                        .collect(),
                ),
            );
        }
        let filters_json = serde_json::Value::Object(filters);

        let row = sqlx::query!(
            r#"SELECT * FROM "ob-poc".set_scope_book($1, $2, $3)"#,
            session_id,
            apex_entity_id,
            filters_json
        )
        .fetch_one(pool)
        .await
        .map_err(|e| anyhow::anyhow!("Failed to set book scope: {}", e))?;

        // Set pending scope change for propagation to session layer
        let apex_name = row.apex_entity_name.clone().unwrap_or_default();
        ctx.set_pending_scope_change(GraphScope::Book {
            apex_entity_id,
            apex_name: apex_name.clone(),
        });

        Ok(ExecutionResult::Record(json!({
            "scope_type": "book",
            "apex_entity_id": apex_entity_id,
            "apex_entity_name": apex_name,
            "filters": row.scope_filters,
            "total_cbus": row.total_cbus,
            "session_id": session_id
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

/// Set scope to single CBU
pub struct SessionSetCbuOp;

#[async_trait]
impl CustomOperation for SessionSetCbuOp {
    fn domain(&self) -> &'static str {
        "session"
    }

    fn verb(&self) -> &'static str {
        "set-cbu"
    }

    fn rationale(&self) -> &'static str {
        "Sets session scope to a single CBU"
    }

    #[cfg(feature = "database")]
    async fn execute(
        &self,
        verb_call: &VerbCall,
        ctx: &mut ExecutionContext,
        pool: &PgPool,
    ) -> Result<ExecutionResult> {
        let session_id = get_session_id(ctx);
        let cbu_id = get_required_uuid(verb_call, "cbu-id", ctx)?;

        let row = sqlx::query!(
            r#"SELECT * FROM "ob-poc".set_scope_cbu($1, $2)"#,
            session_id,
            cbu_id
        )
        .fetch_one(pool)
        .await
        .map_err(|e| anyhow::anyhow!("Failed to set CBU scope: {}", e))?;

        // Set pending scope change for propagation to session layer
        let cbu_name = row.cbu_name.clone().unwrap_or_default();
        ctx.set_pending_scope_change(GraphScope::SingleCbu {
            cbu_id,
            cbu_name: cbu_name.clone(),
        });

        Ok(ExecutionResult::Record(json!({
            "scope_type": "cbu",
            "cbu_id": cbu_id,
            "cbu_name": cbu_name,
            "total_entities": row.total_entities,
            "session_id": session_id
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

/// Set scope to jurisdiction
pub struct SessionSetJurisdictionOp;

#[async_trait]
impl CustomOperation for SessionSetJurisdictionOp {
    fn domain(&self) -> &'static str {
        "session"
    }

    fn verb(&self) -> &'static str {
        "set-jurisdiction"
    }

    fn rationale(&self) -> &'static str {
        "Sets session scope to all CBUs in a jurisdiction"
    }

    #[cfg(feature = "database")]
    async fn execute(
        &self,
        verb_call: &VerbCall,
        ctx: &mut ExecutionContext,
        pool: &PgPool,
    ) -> Result<ExecutionResult> {
        let session_id = get_session_id(ctx);
        let jurisdiction_code = get_required_string(verb_call, "jurisdiction-code")?;

        let row = sqlx::query!(
            r#"SELECT * FROM "ob-poc".set_scope_jurisdiction($1, $2)"#,
            session_id,
            jurisdiction_code
        )
        .fetch_one(pool)
        .await
        .map_err(|e| anyhow::anyhow!("Failed to set jurisdiction scope: {}", e))?;

        // Set pending scope change for propagation to session layer
        ctx.set_pending_scope_change(GraphScope::Jurisdiction {
            code: jurisdiction_code.clone(),
        });

        Ok(ExecutionResult::Record(json!({
            "scope_type": "jurisdiction",
            "jurisdiction_code": jurisdiction_code,
            "total_cbus": row.total_cbus,
            "session_id": session_id
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

/// Set scope to entity neighborhood
pub struct SessionSetNeighborhoodOp;

#[async_trait]
impl CustomOperation for SessionSetNeighborhoodOp {
    fn domain(&self) -> &'static str {
        "session"
    }

    fn verb(&self) -> &'static str {
        "set-neighborhood"
    }

    fn rationale(&self) -> &'static str {
        "Sets session scope to N hops from a focal entity"
    }

    #[cfg(feature = "database")]
    async fn execute(
        &self,
        verb_call: &VerbCall,
        ctx: &mut ExecutionContext,
        pool: &PgPool,
    ) -> Result<ExecutionResult> {
        let session_id = get_session_id(ctx);
        let entity_id = get_required_uuid(verb_call, "entity-id", ctx)?;
        let hops = get_optional_integer(verb_call, "hops").unwrap_or(2);

        let row = sqlx::query!(
            r#"SELECT * FROM "ob-poc".set_scope_neighborhood($1, $2, $3)"#,
            session_id,
            entity_id,
            hops
        )
        .fetch_one(pool)
        .await
        .map_err(|e| anyhow::anyhow!("Failed to set neighborhood scope: {}", e))?;

        // Set pending scope change for propagation to session layer
        ctx.set_pending_scope_change(GraphScope::EntityNeighborhood {
            entity_id,
            hops: hops as u32,
        });

        Ok(ExecutionResult::Record(json!({
            "scope_type": "neighborhood",
            "focal_entity_id": entity_id,
            "focal_entity_name": row.focal_entity_name,
            "hops": hops,
            "session_id": session_id
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

// ============================================================================
// Cursor Operations
// ============================================================================

/// Set cursor (focus) to a specific entity
pub struct SessionFocusOp;

#[async_trait]
impl CustomOperation for SessionFocusOp {
    fn domain(&self) -> &'static str {
        "session"
    }

    fn verb(&self) -> &'static str {
        "focus"
    }

    fn rationale(&self) -> &'static str {
        "Sets cursor to a specific entity within the current scope"
    }

    #[cfg(feature = "database")]
    async fn execute(
        &self,
        verb_call: &VerbCall,
        ctx: &mut ExecutionContext,
        pool: &PgPool,
    ) -> Result<ExecutionResult> {
        let session_id = get_session_id(ctx);
        let entity_id = get_required_uuid(verb_call, "entity-id", ctx)?;

        let row = sqlx::query!(
            r#"SELECT * FROM "ob-poc".set_scope_cursor($1, $2)"#,
            session_id,
            entity_id
        )
        .fetch_one(pool)
        .await
        .map_err(|e| anyhow::anyhow!("Failed to set cursor: {}", e))?;

        Ok(ExecutionResult::Record(json!({
            "cursor_entity_id": entity_id,
            "cursor_entity_name": row.cursor_entity_name,
            "scope_type": row.scope_type,
            "session_id": session_id
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

/// Clear cursor (unfocus)
pub struct SessionClearFocusOp;

#[async_trait]
impl CustomOperation for SessionClearFocusOp {
    fn domain(&self) -> &'static str {
        "session"
    }

    fn verb(&self) -> &'static str {
        "clear-focus"
    }

    fn rationale(&self) -> &'static str {
        "Clears the cursor (unfocus)"
    }

    #[cfg(feature = "database")]
    async fn execute(
        &self,
        _verb_call: &VerbCall,
        ctx: &mut ExecutionContext,
        pool: &PgPool,
    ) -> Result<ExecutionResult> {
        let session_id = get_session_id(ctx);

        let result = sqlx::query!(
            r#"
            UPDATE "ob-poc".session_scopes
            SET cursor_entity_id = NULL,
                cursor_entity_name = NULL,
                updated_at = NOW()
            WHERE session_id = $1
            "#,
            session_id
        )
        .execute(pool)
        .await
        .map_err(|e| anyhow::anyhow!("Failed to clear cursor: {}", e))?;

        Ok(ExecutionResult::Affected(result.rows_affected()))
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

// ============================================================================
// Query Operations
// ============================================================================

/// Get current scope information
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
        "Gets current scope information"
    }

    #[cfg(feature = "database")]
    async fn execute(
        &self,
        _verb_call: &VerbCall,
        ctx: &mut ExecutionContext,
        pool: &PgPool,
    ) -> Result<ExecutionResult> {
        let session_id = get_session_id(ctx);

        // Ensure scope exists
        sqlx::query_scalar!(
            r#"SELECT "ob-poc".get_or_create_session_scope($1, NULL)"#,
            session_id
        )
        .fetch_one(pool)
        .await?;

        let row = sqlx::query!(
            r#"
            SELECT * FROM "ob-poc".v_current_session_scope
            WHERE session_id = $1
            "#,
            session_id
        )
        .fetch_optional(pool)
        .await
        .map_err(|e| anyhow::anyhow!("Failed to get scope info: {}", e))?;

        match row {
            Some(r) => Ok(ExecutionResult::Record(json!({
                "session_id": session_id,
                "scope_type": r.scope_type,
                "scope_display": r.scope_display,
                "cursor_display": r.cursor_display,
                "apex_entity_id": r.apex_entity_id,
                "apex_entity_name": r.apex_entity_name,
                "cbu_id": r.cbu_id,
                "cbu_name": r.cbu_name,
                "jurisdiction_code": r.jurisdiction_code,
                "focal_entity_id": r.focal_entity_id,
                "focal_entity_name": r.focal_entity_name,
                "cursor_entity_id": r.cursor_entity_id,
                "cursor_entity_name": r.cursor_entity_name,
                "total_cbus": r.total_cbus,
                "total_entities": r.total_entities,
                "is_expired": r.is_expired
            }))),
            None => Ok(ExecutionResult::Record(json!({
                "session_id": session_id,
                "scope_type": "empty",
                "scope_display": "No scope set",
                "total_cbus": 0,
                "total_entities": 0
            }))),
        }
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

/// List CBUs in current scope
pub struct SessionListCbusOp;

#[async_trait]
impl CustomOperation for SessionListCbusOp {
    fn domain(&self) -> &'static str {
        "session"
    }

    fn verb(&self) -> &'static str {
        "list-cbus"
    }

    fn rationale(&self) -> &'static str {
        "Lists CBUs in the current scope"
    }

    #[cfg(feature = "database")]
    async fn execute(
        &self,
        verb_call: &VerbCall,
        ctx: &mut ExecutionContext,
        pool: &PgPool,
    ) -> Result<ExecutionResult> {
        let session_id = get_session_id(ctx);
        let limit = get_optional_integer(verb_call, "limit").unwrap_or(100) as i64;

        // Get current scope
        let scope = sqlx::query!(
            r#"
            SELECT scope_type, apex_entity_id, cbu_id, jurisdiction_code
            FROM "ob-poc".session_scopes
            WHERE session_id = $1
            "#,
            session_id
        )
        .fetch_optional(pool)
        .await?;

        // Build dynamic query based on scope type
        let cbus: Vec<serde_json::Value> = match scope {
            Some(s) => {
                match (
                    s.scope_type.as_str(),
                    &s.apex_entity_id,
                    &s.cbu_id,
                    &s.jurisdiction_code,
                ) {
                    ("galaxy" | "book", Some(apex_id), _, _) => {
                        let rows = sqlx::query!(
                            r#"
                            SELECT cbu_id, name, jurisdiction, client_type
                            FROM "ob-poc".cbus
                            WHERE commercial_client_entity_id = $1
                            ORDER BY name
                            LIMIT $2
                            "#,
                            apex_id,
                            limit
                        )
                        .fetch_all(pool)
                        .await?;
                        rows.into_iter()
                            .map(|c| json!({"cbu_id": c.cbu_id, "name": c.name, "jurisdiction": c.jurisdiction, "client_type": c.client_type}))
                            .collect()
                    }
                    ("cbu", _, Some(cbu_id), _) => {
                        let rows = sqlx::query!(
                            r#"
                            SELECT cbu_id, name, jurisdiction, client_type
                            FROM "ob-poc".cbus
                            WHERE cbu_id = $1
                            "#,
                            cbu_id
                        )
                        .fetch_all(pool)
                        .await?;
                        rows.into_iter()
                            .map(|c| json!({"cbu_id": c.cbu_id, "name": c.name, "jurisdiction": c.jurisdiction, "client_type": c.client_type}))
                            .collect()
                    }
                    ("jurisdiction", _, _, Some(jurisdiction)) => {
                        let rows = sqlx::query!(
                            r#"
                            SELECT cbu_id, name, jurisdiction, client_type
                            FROM "ob-poc".cbus
                            WHERE jurisdiction = $1
                            ORDER BY name
                            LIMIT $2
                            "#,
                            jurisdiction,
                            limit
                        )
                        .fetch_all(pool)
                        .await?;
                        rows.into_iter()
                            .map(|c| json!({"cbu_id": c.cbu_id, "name": c.name, "jurisdiction": c.jurisdiction, "client_type": c.client_type}))
                            .collect()
                    }
                    _ => vec![],
                }
            }
            None => vec![],
        };

        let count = cbus.len();
        Ok(ExecutionResult::Record(json!({
            "cbus": cbus,
            "count": count
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

// ============================================================================
// Clear/Reset Operations
// ============================================================================

/// Clear scope (reset to empty)
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
        "Clears the session scope (resets to empty)"
    }

    #[cfg(feature = "database")]
    async fn execute(
        &self,
        _verb_call: &VerbCall,
        ctx: &mut ExecutionContext,
        pool: &PgPool,
    ) -> Result<ExecutionResult> {
        let session_id = get_session_id(ctx);

        let _row = sqlx::query!(r#"SELECT * FROM "ob-poc".clear_scope($1)"#, session_id)
            .fetch_optional(pool)
            .await
            .map_err(|e| anyhow::anyhow!("Failed to clear scope: {}", e))?;

        // Set pending scope change for propagation to session layer
        ctx.set_pending_scope_change(GraphScope::Empty);

        Ok(ExecutionResult::Affected(1))
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

// ============================================================================
// Navigation Operations (Back/Forward)
// ============================================================================

/// Navigate back to previous scope
pub struct SessionBackOp;

#[async_trait]
impl CustomOperation for SessionBackOp {
    fn domain(&self) -> &'static str {
        "session"
    }

    fn verb(&self) -> &'static str {
        "back"
    }

    fn rationale(&self) -> &'static str {
        "Navigates back to previous scope in history"
    }

    #[cfg(feature = "database")]
    async fn execute(
        &self,
        _verb_call: &VerbCall,
        ctx: &mut ExecutionContext,
        pool: &PgPool,
    ) -> Result<ExecutionResult> {
        let session_id = get_session_id(ctx);

        // Call the navigate_back PL/pgSQL function
        let row = sqlx::query_as!(
            SessionScopeState,
            r#"
            SELECT
                scope_type,
                apex_entity_id,
                apex_entity_name,
                cbu_id,
                cbu_name,
                jurisdiction_code,
                focal_entity_id,
                focal_entity_name,
                neighborhood_hops,
                history_position,
                active_cbu_ids
            FROM "ob-poc".navigate_back($1)
            "#,
            session_id
        )
        .fetch_optional(pool)
        .await?;

        match row {
            Some(state) => {
                let navigated = state.is_in_history();

                // Only set pending scope change if we actually navigated
                if navigated {
                    ctx.set_pending_scope_change(state.to_graph_scope());
                }

                Ok(ExecutionResult::Record(state.to_back_response(navigated)))
            }
            None => Ok(ExecutionResult::Record(json!({
                "success": false,
                "navigated": false,
                "message": "Session not found",
                "session_id": session_id
            }))),
        }
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

/// Navigate forward (undo back)
pub struct SessionForwardOp;

#[async_trait]
impl CustomOperation for SessionForwardOp {
    fn domain(&self) -> &'static str {
        "session"
    }

    fn verb(&self) -> &'static str {
        "forward"
    }

    fn rationale(&self) -> &'static str {
        "Navigates forward in history (undo back)"
    }

    #[cfg(feature = "database")]
    async fn execute(
        &self,
        _verb_call: &VerbCall,
        ctx: &mut ExecutionContext,
        pool: &PgPool,
    ) -> Result<ExecutionResult> {
        let session_id = get_session_id(ctx);

        // Call the navigate_forward PL/pgSQL function
        let row = sqlx::query_as!(
            SessionScopeState,
            r#"
            SELECT
                scope_type,
                apex_entity_id,
                apex_entity_name,
                cbu_id,
                cbu_name,
                jurisdiction_code,
                focal_entity_id,
                focal_entity_name,
                neighborhood_hops,
                history_position,
                active_cbu_ids
            FROM "ob-poc".navigate_forward($1)
            "#,
            session_id
        )
        .fetch_optional(pool)
        .await?;

        match row {
            Some(state) => {
                let navigated = !state.is_at_end();

                // Only set pending scope change if we actually navigated
                if navigated {
                    ctx.set_pending_scope_change(state.to_graph_scope());
                }

                Ok(ExecutionResult::Record(
                    state.to_forward_response(navigated),
                ))
            }
            None => Ok(ExecutionResult::Record(json!({
                "success": false,
                "navigated": false,
                "message": "Session not found",
                "session_id": session_id
            }))),
        }
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

// ============================================================================
// Bookmark Operations
// ============================================================================

/// Save current scope as a named bookmark
pub struct SessionSaveBookmarkOp;

#[async_trait]
impl CustomOperation for SessionSaveBookmarkOp {
    fn domain(&self) -> &'static str {
        "session"
    }

    fn verb(&self) -> &'static str {
        "save-bookmark"
    }

    fn rationale(&self) -> &'static str {
        "Saves current scope as a named bookmark"
    }

    #[cfg(feature = "database")]
    async fn execute(
        &self,
        verb_call: &VerbCall,
        ctx: &mut ExecutionContext,
        pool: &PgPool,
    ) -> Result<ExecutionResult> {
        let session_id = get_session_id(ctx);
        let name = get_required_string(verb_call, "name")?;
        let description = get_optional_string(verb_call, "description");

        // Get current scope as snapshot
        let scope = sqlx::query!(
            r#"
            SELECT scope_type, apex_entity_id, apex_entity_name,
                   cbu_id, cbu_name, jurisdiction_code,
                   focal_entity_id, focal_entity_name, neighborhood_hops,
                   scope_filters, cursor_entity_id, cursor_entity_name
            FROM "ob-poc".session_scopes
            WHERE session_id = $1
            "#,
            session_id
        )
        .fetch_optional(pool)
        .await?;

        let snapshot = match scope {
            Some(s) => json!({
                "scope_type": s.scope_type,
                "apex_entity_id": s.apex_entity_id,
                "apex_entity_name": s.apex_entity_name,
                "cbu_id": s.cbu_id,
                "cbu_name": s.cbu_name,
                "jurisdiction_code": s.jurisdiction_code,
                "focal_entity_id": s.focal_entity_id,
                "focal_entity_name": s.focal_entity_name,
                "neighborhood_hops": s.neighborhood_hops,
                "scope_filters": s.scope_filters,
                "cursor_entity_id": s.cursor_entity_id,
                "cursor_entity_name": s.cursor_entity_name
            }),
            None => json!({"scope_type": "empty"}),
        };

        let bookmark_id = sqlx::query_scalar!(
            r#"
            INSERT INTO "ob-poc".session_bookmarks
                (session_id, name, description, scope_snapshot)
            VALUES ($1, $2, $3, $4)
            ON CONFLICT (COALESCE(user_id, session_id), name)
            DO UPDATE SET
                scope_snapshot = EXCLUDED.scope_snapshot,
                description = EXCLUDED.description,
                last_used_at = NOW()
            RETURNING bookmark_id
            "#,
            session_id,
            name,
            description.as_deref(),
            snapshot
        )
        .fetch_one(pool)
        .await
        .map_err(|e| anyhow::anyhow!("Failed to save bookmark: {}", e))?;

        Ok(ExecutionResult::Uuid(bookmark_id))
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

/// Load scope from a saved bookmark
pub struct SessionLoadBookmarkOp;

#[async_trait]
impl CustomOperation for SessionLoadBookmarkOp {
    fn domain(&self) -> &'static str {
        "session"
    }

    fn verb(&self) -> &'static str {
        "load-bookmark"
    }

    fn rationale(&self) -> &'static str {
        "Loads scope from a saved bookmark"
    }

    #[cfg(feature = "database")]
    async fn execute(
        &self,
        verb_call: &VerbCall,
        ctx: &mut ExecutionContext,
        pool: &PgPool,
    ) -> Result<ExecutionResult> {
        let session_id = get_session_id(ctx);
        let name = get_required_string(verb_call, "name")?;

        let bookmark = sqlx::query!(
            r#"
            SELECT bookmark_id, scope_snapshot
            FROM "ob-poc".session_bookmarks
            WHERE (session_id = $1 OR user_id IS NOT NULL)
              AND name = $2
            ORDER BY session_id = $1 DESC
            LIMIT 1
            "#,
            session_id,
            name
        )
        .fetch_optional(pool)
        .await?;

        match bookmark {
            Some(b) => {
                // Update use count
                sqlx::query!(
                    r#"
                    UPDATE "ob-poc".session_bookmarks
                    SET use_count = use_count + 1,
                        last_used_at = NOW()
                    WHERE bookmark_id = $1
                    "#,
                    b.bookmark_id
                )
                .execute(pool)
                .await?;

                // Return the snapshot (caller should apply it)
                // scope_snapshot is JSONB NOT NULL, so it's always present
                Ok(ExecutionResult::Record(b.scope_snapshot))
            }
            None => Err(anyhow::anyhow!("Bookmark '{}' not found", name)),
        }
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

/// List saved bookmarks
pub struct SessionListBookmarksOp;

#[async_trait]
impl CustomOperation for SessionListBookmarksOp {
    fn domain(&self) -> &'static str {
        "session"
    }

    fn verb(&self) -> &'static str {
        "list-bookmarks"
    }

    fn rationale(&self) -> &'static str {
        "Lists saved bookmarks"
    }

    #[cfg(feature = "database")]
    async fn execute(
        &self,
        _verb_call: &VerbCall,
        ctx: &mut ExecutionContext,
        pool: &PgPool,
    ) -> Result<ExecutionResult> {
        let session_id = get_session_id(ctx);

        let bookmarks = sqlx::query!(
            r#"
            SELECT bookmark_id, name, description,
                   scope_snapshot->>'scope_type' as scope_type,
                   use_count, last_used_at, created_at
            FROM "ob-poc".session_bookmarks
            WHERE session_id = $1 OR user_id IS NOT NULL
            ORDER BY last_used_at DESC NULLS LAST, name
            "#,
            session_id
        )
        .fetch_all(pool)
        .await?;

        let list: Vec<serde_json::Value> = bookmarks
            .into_iter()
            .map(|b| {
                json!({
                    "bookmark_id": b.bookmark_id,
                    "name": b.name,
                    "description": b.description,
                    "scope_type": b.scope_type,
                    "use_count": b.use_count,
                    "last_used_at": b.last_used_at,
                    "created_at": b.created_at
                })
            })
            .collect();

        Ok(ExecutionResult::Record(json!({
            "bookmarks": list,
            "count": list.len()
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

/// Delete a saved bookmark
pub struct SessionDeleteBookmarkOp;

#[async_trait]
impl CustomOperation for SessionDeleteBookmarkOp {
    fn domain(&self) -> &'static str {
        "session"
    }

    fn verb(&self) -> &'static str {
        "delete-bookmark"
    }

    fn rationale(&self) -> &'static str {
        "Deletes a saved bookmark"
    }

    #[cfg(feature = "database")]
    async fn execute(
        &self,
        verb_call: &VerbCall,
        ctx: &mut ExecutionContext,
        pool: &PgPool,
    ) -> Result<ExecutionResult> {
        let session_id = get_session_id(ctx);
        let name = get_required_string(verb_call, "name")?;

        let result = sqlx::query!(
            r#"
            DELETE FROM "ob-poc".session_bookmarks
            WHERE session_id = $1 AND name = $2
            "#,
            session_id,
            name
        )
        .execute(pool)
        .await
        .map_err(|e| anyhow::anyhow!("Failed to delete bookmark: {}", e))?;

        Ok(ExecutionResult::Affected(result.rows_affected()))
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

// ============================================================================
// Multi-CBU Set Operations
// ============================================================================

/// Add a CBU to the active set
pub struct SessionAddCbuOp;

#[async_trait]
impl CustomOperation for SessionAddCbuOp {
    fn domain(&self) -> &'static str {
        "session"
    }

    fn verb(&self) -> &'static str {
        "add-cbu"
    }

    fn rationale(&self) -> &'static str {
        "Adds a CBU to the active CBU set"
    }

    #[cfg(feature = "database")]
    async fn execute(
        &self,
        verb_call: &VerbCall,
        ctx: &mut ExecutionContext,
        pool: &PgPool,
    ) -> Result<ExecutionResult> {
        let session_id = get_session_id(ctx);
        let cbu_id = get_required_uuid(verb_call, "cbu-id", ctx)?;

        let row = sqlx::query!(
            r#"
            SELECT active_cbu_ids
            FROM "ob-poc".add_cbu_to_set($1, $2)
            "#,
            session_id,
            cbu_id
        )
        .fetch_optional(pool)
        .await?;

        match row {
            Some(r) => {
                let count = r.active_cbu_ids.as_ref().map(|v| v.len()).unwrap_or(0);
                Ok(ExecutionResult::Record(json!({
                    "success": true,
                    "cbu_id": cbu_id,
                    "active_cbu_count": count,
                    "active_cbu_ids": r.active_cbu_ids
                })))
            }
            None => Ok(ExecutionResult::Record(json!({
                "success": false,
                "message": "Session not found"
            }))),
        }
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

/// Remove a CBU from the active set
pub struct SessionRemoveCbuOp;

#[async_trait]
impl CustomOperation for SessionRemoveCbuOp {
    fn domain(&self) -> &'static str {
        "session"
    }

    fn verb(&self) -> &'static str {
        "remove-cbu"
    }

    fn rationale(&self) -> &'static str {
        "Removes a CBU from the active CBU set"
    }

    #[cfg(feature = "database")]
    async fn execute(
        &self,
        verb_call: &VerbCall,
        ctx: &mut ExecutionContext,
        pool: &PgPool,
    ) -> Result<ExecutionResult> {
        let session_id = get_session_id(ctx);
        let cbu_id = get_required_uuid(verb_call, "cbu-id", ctx)?;

        let row = sqlx::query!(
            r#"
            SELECT active_cbu_ids
            FROM "ob-poc".remove_cbu_from_set($1, $2)
            "#,
            session_id,
            cbu_id
        )
        .fetch_optional(pool)
        .await?;

        match row {
            Some(r) => {
                let count = r.active_cbu_ids.as_ref().map(|v| v.len()).unwrap_or(0);
                Ok(ExecutionResult::Record(json!({
                    "success": true,
                    "removed_cbu_id": cbu_id,
                    "active_cbu_count": count,
                    "active_cbu_ids": r.active_cbu_ids
                })))
            }
            None => Ok(ExecutionResult::Record(json!({
                "success": false,
                "message": "Session not found"
            }))),
        }
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

/// Clear the active CBU set
pub struct SessionClearCbuSetOp;

#[async_trait]
impl CustomOperation for SessionClearCbuSetOp {
    fn domain(&self) -> &'static str {
        "session"
    }

    fn verb(&self) -> &'static str {
        "clear-cbu-set"
    }

    fn rationale(&self) -> &'static str {
        "Clears all CBUs from the active set"
    }

    #[cfg(feature = "database")]
    async fn execute(
        &self,
        _verb_call: &VerbCall,
        ctx: &mut ExecutionContext,
        pool: &PgPool,
    ) -> Result<ExecutionResult> {
        let session_id = get_session_id(ctx);

        let row = sqlx::query!(
            r#"
            SELECT active_cbu_ids
            FROM "ob-poc".clear_cbu_set($1)
            "#,
            session_id
        )
        .fetch_optional(pool)
        .await?;

        match row {
            Some(_) => Ok(ExecutionResult::Record(json!({
                "success": true,
                "active_cbu_count": 0,
                "active_cbu_ids": []
            }))),
            None => Ok(ExecutionResult::Record(json!({
                "success": false,
                "message": "Session not found"
            }))),
        }
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

/// List active CBUs in the set
pub struct SessionListActiveCbusOp;

#[async_trait]
impl CustomOperation for SessionListActiveCbusOp {
    fn domain(&self) -> &'static str {
        "session"
    }

    fn verb(&self) -> &'static str {
        "list-active-cbus"
    }

    fn rationale(&self) -> &'static str {
        "Lists all CBUs in the active set with details"
    }

    #[cfg(feature = "database")]
    async fn execute(
        &self,
        _verb_call: &VerbCall,
        ctx: &mut ExecutionContext,
        pool: &PgPool,
    ) -> Result<ExecutionResult> {
        let session_id = get_session_id(ctx);

        // Get active CBU IDs from session scope
        let scope = sqlx::query!(
            r#"
            SELECT active_cbu_ids
            FROM "ob-poc".session_scopes
            WHERE session_id = $1
            "#,
            session_id
        )
        .fetch_optional(pool)
        .await?;

        let cbu_ids = scope.and_then(|s| s.active_cbu_ids).unwrap_or_default();

        if cbu_ids.is_empty() {
            return Ok(ExecutionResult::Record(json!({
                "count": 0,
                "cbus": []
            })));
        }

        // Get CBU details
        let cbus = sqlx::query!(
            r#"
            SELECT cbu_id, name, jurisdiction, client_type
            FROM "ob-poc".cbus
            WHERE cbu_id = ANY($1)
            ORDER BY name
            "#,
            &cbu_ids
        )
        .fetch_all(pool)
        .await?;

        let list: Vec<serde_json::Value> = cbus
            .into_iter()
            .map(|c| {
                json!({
                    "cbu_id": c.cbu_id,
                    "name": c.name,
                    "jurisdiction": c.jurisdiction,
                    "client_type": c.client_type
                })
            })
            .collect();

        Ok(ExecutionResult::Record(json!({
            "count": list.len(),
            "cbus": list
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

// ============================================================================
// Registration
// ============================================================================

/// Register all session operations with the registry
pub fn register_session_ops(registry: &mut crate::dsl_v2::custom_ops::CustomOperationRegistry) {
    use std::sync::Arc;

    // Scope setting
    registry.register(Arc::new(SessionSetGalaxyOp));
    registry.register(Arc::new(SessionSetBookOp));
    registry.register(Arc::new(SessionSetCbuOp));
    registry.register(Arc::new(SessionSetJurisdictionOp));
    registry.register(Arc::new(SessionSetNeighborhoodOp));

    // Cursor
    registry.register(Arc::new(SessionFocusOp));
    registry.register(Arc::new(SessionClearFocusOp));

    // Navigation
    registry.register(Arc::new(SessionBackOp));
    registry.register(Arc::new(SessionForwardOp));

    // Query
    registry.register(Arc::new(SessionInfoOp));
    registry.register(Arc::new(SessionListCbusOp));

    // Clear
    registry.register(Arc::new(SessionClearOp));

    // Bookmarks
    registry.register(Arc::new(SessionSaveBookmarkOp));
    registry.register(Arc::new(SessionLoadBookmarkOp));
    registry.register(Arc::new(SessionListBookmarksOp));
    registry.register(Arc::new(SessionDeleteBookmarkOp));

    // Multi-CBU Set
    registry.register(Arc::new(SessionAddCbuOp));
    registry.register(Arc::new(SessionRemoveCbuOp));
    registry.register(Arc::new(SessionClearCbuSetOp));
    registry.register(Arc::new(SessionListActiveCbusOp));
}
