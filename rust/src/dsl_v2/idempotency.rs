//! Idempotency support for DSL execution
//!
//! Ensures DSL programs can be re-run without side effects by tracking
//! executed statements and returning cached results for duplicates.
//!
//! ## Source Attribution
//!
//! Each execution is tagged with provenance information:
//! - `source`: Where the execution originated (api, cli, mcp, repl, batch, test)
//! - `request_id`: Correlation ID for distributed tracing
//! - `actor_id`: User or system that initiated the execution
//! - `actor_type`: Type of actor (user, system, agent, service)
//!
//! ## Atomic View State Recording
//!
//! When a view.* operation produces a ViewState, both the idempotency record
//! and the view state change are recorded atomically via a PostgreSQL function.
//! This ensures the audit trail is consistent even if the process crashes.

use anyhow::Result;
use serde::{Deserialize, Serialize};
use serde_json::Value as JsonValue;
use sha2::{Digest, Sha256};
use std::collections::HashMap;
use uuid::Uuid;

#[cfg(feature = "database")]
use sqlx::PgPool;

use super::executor::ExecutionResult;

/// Source of an execution - where did this request originate?
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize, Default)]
#[serde(rename_all = "snake_case")]
pub enum ExecutionSource {
    /// API request (REST/GraphQL)
    Api,
    /// Command-line interface
    Cli,
    /// MCP (Model Context Protocol) tool call
    Mcp,
    /// REPL interactive session
    Repl,
    /// Batch/bulk operation
    Batch,
    /// Test execution
    Test,
    /// Database migration
    Migration,
    /// Unknown source
    #[default]
    Unknown,
}

impl ExecutionSource {
    pub fn as_str(&self) -> &'static str {
        match self {
            Self::Api => "api",
            Self::Cli => "cli",
            Self::Mcp => "mcp",
            Self::Repl => "repl",
            Self::Batch => "batch",
            Self::Test => "test",
            Self::Migration => "migration",
            Self::Unknown => "unknown",
        }
    }
}

/// Actor type - what kind of entity initiated the execution?
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize, Default)]
#[serde(rename_all = "snake_case")]
pub enum ActorType {
    /// Human user
    #[default]
    User,
    /// Automated system
    System,
    /// AI agent
    Agent,
    /// External service
    Service,
}

impl ActorType {
    pub fn as_str(&self) -> &'static str {
        match self {
            Self::User => "user",
            Self::System => "system",
            Self::Agent => "agent",
            Self::Service => "service",
        }
    }
}

/// Source attribution for execution audit trail
#[derive(Debug, Clone, Default)]
pub struct SourceAttribution {
    /// Where the execution originated
    pub source: ExecutionSource,
    /// Correlation ID for distributed tracing
    pub request_id: Option<Uuid>,
    /// ID of the actor (user, system, etc.)
    pub actor_id: Option<Uuid>,
    /// Type of actor
    pub actor_type: ActorType,
}

impl SourceAttribution {
    pub fn new(source: ExecutionSource) -> Self {
        Self {
            source,
            ..Default::default()
        }
    }

    pub fn with_request_id(mut self, request_id: Uuid) -> Self {
        self.request_id = Some(request_id);
        self
    }

    pub fn with_actor(mut self, actor_id: Uuid, actor_type: ActorType) -> Self {
        self.actor_id = Some(actor_id);
        self.actor_type = actor_type;
        self
    }
}

/// Cached result from a previous execution
#[derive(Debug, Clone)]
pub struct CachedResult {
    pub result_type: String,
    pub result_id: Option<Uuid>,
    pub result_json: Option<JsonValue>,
    pub result_affected: Option<i64>,
}

impl CachedResult {
    /// Convert cached result back to ExecutionResult
    pub fn to_execution_result(&self) -> ExecutionResult {
        match self.result_type.as_str() {
            "uuid" => {
                if let Some(id) = self.result_id {
                    ExecutionResult::Uuid(id)
                } else {
                    ExecutionResult::Void
                }
            }
            "affected" => ExecutionResult::Affected(self.result_affected.unwrap_or(0) as u64),
            "record" => {
                ExecutionResult::Record(self.result_json.clone().unwrap_or(JsonValue::Null))
            }
            "recordset" => {
                if let Some(JsonValue::Array(arr)) = &self.result_json {
                    ExecutionResult::RecordSet(arr.clone())
                } else {
                    ExecutionResult::RecordSet(vec![])
                }
            }
            _ => ExecutionResult::Void,
        }
    }
}

/// Computes idempotency key from execution context
pub fn compute_idempotency_key(
    execution_id: Uuid,
    statement_index: usize,
    verb: &str,
    args: &HashMap<String, JsonValue>,
) -> String {
    let mut hasher = Sha256::new();

    // Add execution context
    hasher.update(execution_id.as_bytes());
    hasher.update(statement_index.to_le_bytes());
    hasher.update(verb.as_bytes());

    // Add canonical args (sorted keys for determinism)
    let args_hash = compute_args_hash(args);
    hasher.update(args_hash.as_bytes());

    format!("{:x}", hasher.finalize())
}

/// Compute hash of arguments for storage/debugging
pub fn compute_args_hash(args: &HashMap<String, JsonValue>) -> String {
    let mut hasher = Sha256::new();

    // Sort keys for deterministic hashing
    let mut keys: Vec<&String> = args.keys().collect();
    keys.sort();

    for key in keys {
        hasher.update(key.as_bytes());
        if let Some(value) = args.get(key) {
            // Use canonical JSON representation
            if let Ok(json_str) = serde_json::to_string(value) {
                hasher.update(json_str.as_bytes());
            }
        }
    }

    format!("{:x}", hasher.finalize())
}

/// Idempotency manager for DSL execution
#[cfg(feature = "database")]
pub struct IdempotencyManager {
    pool: PgPool,
}

#[cfg(feature = "database")]
impl IdempotencyManager {
    pub fn new(pool: PgPool) -> Self {
        Self { pool }
    }

    /// Check if a statement has already been executed
    pub async fn check(
        &self,
        execution_id: Uuid,
        statement_index: usize,
        verb: &str,
        args: &HashMap<String, JsonValue>,
    ) -> Result<Option<CachedResult>> {
        let key = compute_idempotency_key(execution_id, statement_index, verb, args);

        let row = sqlx::query_as::<_, (String, Option<Uuid>, Option<JsonValue>, Option<i64>)>(
            r#"SELECT result_type, result_id, result_json, result_affected
               FROM "ob-poc".dsl_idempotency
               WHERE idempotency_key = $1"#,
        )
        .bind(&key)
        .fetch_optional(&self.pool)
        .await?;

        Ok(row.map(
            |(result_type, result_id, result_json, result_affected)| CachedResult {
                result_type,
                result_id,
                result_json,
                result_affected,
            },
        ))
    }

    /// Record a successful execution for future idempotency checks
    ///
    /// The optional `verb_hash` parameter links this execution to a specific
    /// verb configuration version for audit trail purposes.
    ///
    /// Returns the idempotency key for downstream audit linkage (e.g., view state audit).
    pub async fn record(
        &self,
        execution_id: Uuid,
        statement_index: usize,
        verb: &str,
        args: &HashMap<String, JsonValue>,
        result: &ExecutionResult,
        verb_hash: Option<&[u8]>,
    ) -> Result<String> {
        let key = compute_idempotency_key(execution_id, statement_index, verb, args);
        let args_hash = compute_args_hash(args);

        let (result_type, result_id, result_json, result_affected) = match result {
            ExecutionResult::Uuid(id) => ("uuid", Some(*id), None, None),
            ExecutionResult::Affected(n) => ("affected", None, None, Some(*n as i64)),
            ExecutionResult::Record(json) => ("record", None, Some(json.clone()), None),
            ExecutionResult::RecordSet(arr) => {
                ("recordset", None, Some(JsonValue::Array(arr.clone())), None)
            }
            ExecutionResult::Void => ("void", None, None, None),
            ExecutionResult::EntityQuery(query_result) => {
                // Serialize entity query result as JSON for idempotency caching
                let json = serde_json::json!({
                    "items": query_result.items.iter().map(|(id, name)| {
                        serde_json::json!({"id": id.to_string(), "name": name})
                    }).collect::<Vec<_>>(),
                    "entity_type": query_result.entity_type,
                    "total_count": query_result.total_count,
                });
                ("entity_query", None, Some(json), None)
            }
            ExecutionResult::TemplateInvoked(invoke_result) => {
                // Serialize template invoke result as JSON
                let json = serde_json::json!({
                    "template_id": invoke_result.template_id,
                    "statements_executed": invoke_result.statements_executed,
                    "outputs": invoke_result.outputs.iter().map(|(k, v)| {
                        (k.clone(), v.to_string())
                    }).collect::<HashMap<String, String>>(),
                    "primary_entity_id": invoke_result.primary_entity_id.map(|id| id.to_string()),
                });
                (
                    "template_invoked",
                    invoke_result.primary_entity_id,
                    Some(json),
                    None,
                )
            }
            ExecutionResult::TemplateBatch(batch_result) => {
                // Serialize template batch result as JSON
                let json = serde_json::json!({
                    "template_id": batch_result.template_id,
                    "total_items": batch_result.total_items,
                    "success_count": batch_result.success_count,
                    "failure_count": batch_result.failure_count,
                    "primary_entity_ids": batch_result.primary_entity_ids.iter().map(|id| id.to_string()).collect::<Vec<_>>(),
                    "primary_entity_type": batch_result.primary_entity_type,
                    "aborted": batch_result.aborted,
                });
                (
                    "template_batch",
                    batch_result.primary_entity_ids.first().copied(),
                    Some(json),
                    None,
                )
            }
            ExecutionResult::BatchControl(control_result) => {
                // Serialize batch control result as JSON
                let json = serde_json::json!({
                    "operation": control_result.operation,
                    "success": control_result.success,
                    "status": control_result.status,
                    "message": control_result.message,
                });
                ("batch_control", None, Some(json), None)
            }
        };

        sqlx::query(
            r#"INSERT INTO "ob-poc".dsl_idempotency
               (idempotency_key, execution_id, statement_index, verb, args_hash,
                result_type, result_id, result_json, result_affected, verb_hash)
               VALUES ($1, $2, $3, $4, $5, $6, $7, $8, $9, $10)
               ON CONFLICT (idempotency_key) DO NOTHING"#,
        )
        .bind(&key)
        .bind(execution_id)
        .bind(statement_index as i32)
        .bind(verb)
        .bind(&args_hash)
        .bind(result_type)
        .bind(result_id)
        .bind(result_json)
        .bind(result_affected)
        .bind(verb_hash)
        .execute(&self.pool)
        .await?;

        Ok(key)
    }

    /// Clear idempotency records for an execution (for testing/reset)
    pub async fn clear_execution(&self, execution_id: Uuid) -> Result<u64> {
        let result = sqlx::query(r#"DELETE FROM "ob-poc".dsl_idempotency WHERE execution_id = $1"#)
            .bind(execution_id)
            .execute(&self.pool)
            .await?;

        Ok(result.rows_affected())
    }

    /// Atomically record execution AND view state change in a single transaction
    ///
    /// This is the preferred method when a view.* operation produces a ViewState.
    /// It ensures both the idempotency record and view state audit are committed
    /// together, preventing inconsistency if the process crashes between writes.
    ///
    /// Returns (idempotency_key, view_state_change_id, was_cached)
    #[allow(clippy::too_many_arguments)]
    pub async fn record_with_view_state(
        &self,
        execution_id: Uuid,
        statement_index: usize,
        verb: &str,
        args: &HashMap<String, JsonValue>,
        result: &ExecutionResult,
        verb_hash: Option<&[u8]>,
        attribution: &SourceAttribution,
        view_state: Option<&crate::session::ViewState>,
        session_id: Option<Uuid>,
    ) -> Result<AtomicRecordResult> {
        let key = compute_idempotency_key(execution_id, statement_index, verb, args);
        let args_hash = compute_args_hash(args);

        let (result_type, result_id, result_json, result_affected) =
            Self::execution_result_to_db(result);

        // Prepare view state params if provided
        let (view_taxonomy, view_selection, view_refinements, view_stack_depth, view_snapshot) =
            if let Some(vs) = view_state {
                (
                    Some(serde_json::to_value(&vs.context)?),
                    Some(vs.selection.clone()),
                    Some(serde_json::to_value(&vs.refinements)?),
                    Some(vs.stack.depth() as i32),
                    Some(serde_json::to_value(vs)?),
                )
            } else {
                (None, None, None, None, None)
            };

        // Call atomic PostgreSQL function
        let row = sqlx::query_as::<_, (String, Option<Uuid>, bool)>(
            r#"SELECT * FROM "ob-poc".record_execution_with_view_state(
                $1, $2, $3, $4, $5, $6, $7, $8, $9, $10,
                $11, $12, $13, $14,
                $15, $16, $17, $18, $19, $20
            )"#,
        )
        .bind(&key)
        .bind(execution_id)
        .bind(statement_index as i32)
        .bind(verb)
        .bind(&args_hash)
        .bind(result_type)
        .bind(result_id)
        .bind(&result_json)
        .bind(result_affected)
        .bind(verb_hash)
        // Source attribution
        .bind(attribution.source.as_str())
        .bind(attribution.request_id)
        .bind(attribution.actor_id)
        .bind(attribution.actor_type.as_str())
        // View state (optional)
        .bind(session_id)
        .bind(&view_taxonomy)
        .bind(&view_selection)
        .bind(&view_refinements)
        .bind(view_stack_depth)
        .bind(&view_snapshot)
        .fetch_one(&self.pool)
        .await?;

        Ok(AtomicRecordResult {
            idempotency_key: row.0,
            view_state_change_id: row.1,
            was_cached: row.2,
        })
    }

    /// Convert ExecutionResult to database columns
    fn execution_result_to_db(
        result: &ExecutionResult,
    ) -> (&'static str, Option<Uuid>, Option<JsonValue>, Option<i64>) {
        match result {
            ExecutionResult::Uuid(id) => ("uuid", Some(*id), None, None),
            ExecutionResult::Affected(n) => ("affected", None, None, Some(*n as i64)),
            ExecutionResult::Record(json) => ("record", None, Some(json.clone()), None),
            ExecutionResult::RecordSet(arr) => {
                ("recordset", None, Some(JsonValue::Array(arr.clone())), None)
            }
            ExecutionResult::Void => ("void", None, None, None),
            ExecutionResult::EntityQuery(query_result) => {
                let json = serde_json::json!({
                    "items": query_result.items.iter().map(|(id, name)| {
                        serde_json::json!({"id": id.to_string(), "name": name})
                    }).collect::<Vec<_>>(),
                    "entity_type": query_result.entity_type,
                    "total_count": query_result.total_count,
                });
                ("entity_query", None, Some(json), None)
            }
            ExecutionResult::TemplateInvoked(invoke_result) => {
                let json = serde_json::json!({
                    "template_id": invoke_result.template_id,
                    "statements_executed": invoke_result.statements_executed,
                    "outputs": invoke_result.outputs.iter().map(|(k, v)| {
                        (k.clone(), v.to_string())
                    }).collect::<HashMap<String, String>>(),
                    "primary_entity_id": invoke_result.primary_entity_id.map(|id| id.to_string()),
                });
                (
                    "template_invoked",
                    invoke_result.primary_entity_id,
                    Some(json),
                    None,
                )
            }
            ExecutionResult::TemplateBatch(batch_result) => {
                let json = serde_json::json!({
                    "template_id": batch_result.template_id,
                    "total_items": batch_result.total_items,
                    "success_count": batch_result.success_count,
                    "failure_count": batch_result.failure_count,
                    "primary_entity_ids": batch_result.primary_entity_ids.iter().map(|id| id.to_string()).collect::<Vec<_>>(),
                    "primary_entity_type": batch_result.primary_entity_type,
                    "aborted": batch_result.aborted,
                });
                (
                    "template_batch",
                    batch_result.primary_entity_ids.first().copied(),
                    Some(json),
                    None,
                )
            }
            ExecutionResult::BatchControl(control_result) => {
                let json = serde_json::json!({
                    "operation": control_result.operation,
                    "success": control_result.success,
                    "status": control_result.status,
                    "message": control_result.message,
                });
                ("batch_control", None, Some(json), None)
            }
        }
    }
}

/// Result of atomic execution + view state recording
#[derive(Debug, Clone)]
pub struct AtomicRecordResult {
    /// The idempotency key for this execution
    pub idempotency_key: String,
    /// The view state change ID (if view state was recorded)
    pub view_state_change_id: Option<Uuid>,
    /// Whether this was a cached result (already executed)
    pub was_cached: bool,
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_compute_idempotency_key_deterministic() {
        let execution_id = Uuid::parse_str("550e8400-e29b-41d4-a716-446655440000").unwrap();
        let mut args = HashMap::new();
        args.insert("name".to_string(), JsonValue::String("Test".to_string()));
        args.insert(
            "jurisdiction".to_string(),
            JsonValue::String("US".to_string()),
        );

        let key1 = compute_idempotency_key(execution_id, 0, "cbu.ensure", &args);
        let key2 = compute_idempotency_key(execution_id, 0, "cbu.ensure", &args);

        assert_eq!(key1, key2);
    }

    #[test]
    fn test_different_statement_index_different_key() {
        let execution_id = Uuid::parse_str("550e8400-e29b-41d4-a716-446655440000").unwrap();
        let args = HashMap::new();

        let key1 = compute_idempotency_key(execution_id, 0, "cbu.ensure", &args);
        let key2 = compute_idempotency_key(execution_id, 1, "cbu.ensure", &args);

        assert_ne!(key1, key2);
    }

    #[test]
    fn test_args_order_independence() {
        let mut args1 = HashMap::new();
        args1.insert("a".to_string(), JsonValue::String("1".to_string()));
        args1.insert("b".to_string(), JsonValue::String("2".to_string()));

        let mut args2 = HashMap::new();
        args2.insert("b".to_string(), JsonValue::String("2".to_string()));
        args2.insert("a".to_string(), JsonValue::String("1".to_string()));

        let hash1 = compute_args_hash(&args1);
        let hash2 = compute_args_hash(&args2);

        assert_eq!(hash1, hash2);
    }
}
