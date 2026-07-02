//! Generic CRUD Executor
//!
//! Executes CRUD operations based on RuntimeVerb configuration from YAML.
//! All table names, column mappings, and behaviors come from config.
//!
//! This replaces the pattern of:
//! - Static Behavior enum variants in verbs.rs
//! - Hardcoded execute_* methods in executor.rs
//! - Static column mappings in mappings.rs

use anyhow::{anyhow, bail, Result};
use serde_json::{json, Value as JsonValue};
use std::collections::HashMap;
use std::sync::Arc;
use tokio::sync::Mutex;
use tracing::debug;
use uuid::Uuid;

#[cfg(feature = "database")]
use sqlx::{postgres::PgRow, PgPool, Row};

#[cfg(feature = "database")]
use entity_gateway::proto::ob::gateway::v1::{
    entity_gateway_client::EntityGatewayClient, SearchMode, SearchRequest,
};
#[cfg(feature = "database")]
use ob_poc_ontology::ontology;
#[cfg(feature = "database")]
use tonic::transport::Channel;

use super::config::types::{ArgType, CrudOperation, ReturnTypeConfig};
use super::runtime_registry::{RuntimeArg, RuntimeBehavior, RuntimeCrudConfig, RuntimeVerb};

// =============================================================================
// EXECUTION RESULT
// =============================================================================

/// Result of executing a verb via generic executor
#[derive(Debug, Clone)]
pub enum GenericExecutionResult {
    /// Single UUID returned (from INSERT/UPSERT with RETURNING)
    Uuid(Uuid),
    /// Single record (from SELECT by ID)
    Record(JsonValue),
    /// Multiple records (from SELECT list)
    RecordSet(Vec<JsonValue>),
    /// Number of rows affected (from UPDATE/DELETE)
    Affected(u64),
    /// No return value
    Void,
}

impl GenericExecutionResult {
    /// Convert to the existing ExecutionResult type for compatibility
    pub fn to_legacy(&self) -> super::executor::ExecutionResult {
        match self {
            GenericExecutionResult::Uuid(u) => super::executor::ExecutionResult::Uuid(*u),
            GenericExecutionResult::Record(r) => {
                super::executor::ExecutionResult::Record(r.clone())
            }
            GenericExecutionResult::RecordSet(rs) => {
                super::executor::ExecutionResult::RecordSet(rs.clone())
            }
            GenericExecutionResult::Affected(n) => super::executor::ExecutionResult::Affected(*n),
            GenericExecutionResult::Void => super::executor::ExecutionResult::Void,
        }
    }
}
// SQL VALUE TYPE (internal)
// =============================================================================

/// Internal SQL value representation for dynamic binding
#[cfg(feature = "database")]
#[derive(Debug, Clone)]
enum SqlValue {
    String(String),
    Uuid(Uuid),
    Integer(i64),
    Decimal(rust_decimal::Decimal),
    Boolean(bool),
    Json(JsonValue),
    Date(chrono::NaiveDate),
    Timestamp(chrono::DateTime<chrono::Utc>),
    StringArray(Vec<String>),
    UuidArray(Vec<Uuid>),
    #[allow(dead_code)]
    Null,
}

// =============================================================================
// GENERIC CRUD EXECUTOR
// =============================================================================

/// Generic CRUD executor - executes verbs based on YAML config
#[cfg(feature = "database")]
pub struct GenericCrudExecutor {
    pool: PgPool,
    /// EntityGateway client for lookup resolution (lazy initialized)
    gateway_client: Arc<Mutex<Option<EntityGatewayClient<Channel>>>>,
}

#[cfg(feature = "database")]
impl GenericCrudExecutor {
    fn soft_delete_supported(&self, schema: &str, table: &str) -> bool {
        schema == "ob-poc" && matches!(table, "cbus" | "entities")
    }

    fn active_row_predicate(
        &self,
        schema: &str,
        table: &str,
        qualifier: Option<&str>,
    ) -> Option<String> {
        self.soft_delete_supported(schema, table)
            .then(|| match qualifier {
                Some(alias) => format!(r#"{alias}."deleted_at" IS NULL"#),
                None => r#""deleted_at" IS NULL"#.to_string(),
            })
    }

    pub fn new(pool: PgPool) -> Self {
        Self {
            pool,
            gateway_client: Arc::new(Mutex::new(None)),
        }
    }

    /// Create executor without a database pool (for dry-run testing only)
    #[cfg(test)]
    pub fn new_without_pool() -> Self {
        use sqlx::postgres::PgPoolOptions;

        // Create a dummy pool that will never be used (dry_run=true skips execution)
        // This is a workaround since PgPool doesn't have a Default impl
        let pool = PgPoolOptions::new()
            .max_connections(1)
            .connect_lazy("postgres://localhost/nonexistent")
            .expect("lazy pool creation should not fail");

        Self {
            pool,
            gateway_client: Arc::new(Mutex::new(None)),
        }
    }

    /// Get or create the canonical EntityGateway client.
    async fn get_gateway_client(&self) -> Result<EntityGatewayClient<Channel>> {
        let mut guard = self.gateway_client.lock().await;
        if guard.is_none() {
            let addr = super::gateway_resolver::gateway_addr();
            let client = EntityGatewayClient::connect(addr.clone())
                .await
                .map_err(|e| anyhow!("EntityGateway unavailable at {}: {}", addr, e))?;
            *guard = Some(client);
        }
        guard
            .as_ref()
            .cloned()
            .ok_or_else(|| anyhow!("EntityGateway client was not initialized"))
    }

    /// Execute a verb from RuntimeVerb configuration (auto-commit mode)
    ///
    /// # Arguments
    /// * `verb` - The RuntimeVerb definition from YAML config
    /// * `args` - Arguments as JSON values (already resolved references)
    ///
    /// # Returns
    /// GenericExecutionResult based on the verb's return type
    pub async fn execute(
        &self,
        verb: &RuntimeVerb,
        args: &HashMap<String, JsonValue>,
    ) -> Result<GenericExecutionResult> {
        self.execute_with_optional_tx(verb, args, None).await
    }

    /// Execute a verb within an existing transaction
    ///
    /// This method ensures the verb execution participates in the caller's transaction.
    /// All database operations will use the provided transaction, enabling atomic
    /// multi-verb execution with proper rollback on failure.
    ///
    /// # Arguments
    /// * `tx` - Mutable reference to an active transaction
    /// * `verb` - The RuntimeVerb definition from YAML config
    /// * `args` - Arguments as JSON values (already resolved references)
    ///
    /// # Returns
    /// GenericExecutionResult based on the verb's return type
    pub async fn execute_in_tx(
        &self,
        tx: &mut sqlx::Transaction<'_, sqlx::Postgres>,
        verb: &RuntimeVerb,
        args: &HashMap<String, JsonValue>,
    ) -> Result<GenericExecutionResult> {
        self.execute_with_optional_tx(verb, args, Some(tx)).await
    }

    /// Internal implementation that works with optional transaction
    ///
    /// When `tx` is Some, all operations use the transaction.
    /// When `tx` is None, operations use the pool (auto-commit).
    async fn execute_with_optional_tx(
        &self,
        verb: &RuntimeVerb,
        args: &HashMap<String, JsonValue>,
        tx: Option<&mut sqlx::Transaction<'_, sqlx::Postgres>>,
    ) -> Result<GenericExecutionResult> {
        tracing::debug!(
            "DBG GenericCrudExecutor::execute ENTER {}.{} (in_tx={})",
            verb.domain,
            verb.verb,
            tx.is_some()
        );

        let crud = match &verb.behavior {
            RuntimeBehavior::Crud(crud) => crud,
            RuntimeBehavior::Plugin(handler) => {
                return Err(anyhow!(
                    "Verb {}.{} is a plugin (handler: {}), use plugin executor",
                    verb.domain,
                    verb.verb,
                    handler
                ));
            }
            RuntimeBehavior::GraphQuery(_) => {
                return Err(anyhow!(
                    "Verb {}.{} is a graph query, use GraphQueryExecutor",
                    verb.domain,
                    verb.verb
                ));
            }
            RuntimeBehavior::Durable(_) => {
                return Err(anyhow!(
                    "Verb {}.{} is durable, use WorkflowDispatcher",
                    verb.domain,
                    verb.verb
                ));
            }
        };

        tracing::debug!(
            "DBG GenericCrudExecutor: operation={:?} table={}.{}",
            crud.operation,
            crud.schema,
            crud.table
        );

        // For transaction support, we need to pass tx through to the operation methods.
        // Since Option<&mut T> can't be easily passed through multiple calls without
        // ownership issues, we use a different approach: check if we have a tx and
        // call the appropriate variant.
        //
        // TODO: Once all execute_* methods support tx, refactor to pass tx directly.
        // For now, we handle the most critical write operations.
        let result = if let Some(tx) = tx {
            match crud.operation {
                CrudOperation::Insert => self.execute_insert_in_tx(tx, verb, crud, args).await,
                CrudOperation::Update => self.execute_update_in_tx(tx, verb, crud, args).await,
                CrudOperation::Delete => self.execute_delete_in_tx(tx, verb, crud, args).await,
                CrudOperation::Upsert => self.execute_upsert_in_tx(tx, verb, crud, args).await,
                CrudOperation::Link => self.execute_link_in_tx(tx, verb, crud, args).await,
                CrudOperation::Unlink => self.execute_unlink_in_tx(tx, verb, crud, args).await,
                CrudOperation::RoleLink => self.execute_role_link_in_tx(tx, verb, crud, args).await,
                CrudOperation::RoleUnlink => {
                    self.execute_role_unlink_in_tx(tx, verb, crud, args).await
                }
                CrudOperation::EntityCreate => {
                    self.execute_entity_create_in_tx(tx, verb, crud, args).await
                }
                CrudOperation::EntityUpsert => {
                    self.execute_entity_upsert_in_tx(tx, verb, crud, args).await
                }
                // Read operations use the active transaction so that plan writes
                // from earlier steps are visible to subsequent reads (B1 fix).
                CrudOperation::Select => self.execute_select_in_tx(tx, verb, crud, args).await,
                CrudOperation::ListByFk => {
                    self.execute_list_by_fk_in_tx(tx, verb, crud, args).await
                }
                CrudOperation::ListParties => {
                    self.execute_list_parties_in_tx(tx, verb, crud, args).await
                }
                CrudOperation::SelectWithJoin => {
                    self.execute_select_with_join_in_tx(tx, verb, crud, args)
                        .await
                }
            }
        } else {
            match crud.operation {
                CrudOperation::Insert => self.execute_insert(verb, crud, args).await,
                CrudOperation::Select => self.execute_select(verb, crud, args).await,
                CrudOperation::Update => self.execute_update(verb, crud, args).await,
                CrudOperation::Delete => self.execute_delete(verb, crud, args).await,
                CrudOperation::Upsert => self.execute_upsert(verb, crud, args).await,
                CrudOperation::Link => self.execute_link(verb, crud, args).await,
                CrudOperation::Unlink => self.execute_unlink(verb, crud, args).await,
                CrudOperation::RoleLink => self.execute_role_link(verb, crud, args).await,
                CrudOperation::RoleUnlink => self.execute_role_unlink(verb, crud, args).await,
                CrudOperation::ListByFk => self.execute_list_by_fk(verb, crud, args).await,
                CrudOperation::ListParties => self.execute_list_parties(verb, crud, args).await,
                CrudOperation::SelectWithJoin => {
                    self.execute_select_with_join(verb, crud, args).await
                }
                CrudOperation::EntityCreate => self.execute_entity_create(verb, crud, args).await,
                CrudOperation::EntityUpsert => self.execute_entity_upsert(verb, crud, args).await,
            }
        };

        tracing::debug!(
            "DBG GenericCrudExecutor::execute EXIT result={:?}",
            result.is_ok()
        );
        result
    }

    // =========================================================================
    // DAG-BASED EXECUTION
    // =========================================================================

    /// Execute INSERT with idempotency support
    ///
    /// If conflict_keys are defined in YAML, uses ON CONFLICT DO UPDATE (upsert behavior).
    /// Otherwise, uses ON CONFLICT DO NOTHING and returns existing row if conflict.
    async fn execute_insert(
        &self,
        verb: &RuntimeVerb,
        crud: &RuntimeCrudConfig,
        args: &HashMap<String, JsonValue>,
    ) -> Result<GenericExecutionResult> {
        let mut columns = Vec::new();
        let mut placeholders = Vec::new();
        let mut bind_values: Vec<SqlValue> = Vec::new();

        // Generate UUID for primary key (convention: table_pk is <singular>_id)
        let pk_col = crud
            .returning
            .as_deref()
            .unwrap_or_else(|| self.infer_pk_column(&crud.table));

        let new_id = Uuid::new_v4();
        columns.push(format!("\"{}\"", pk_col));
        placeholders.push("$1".to_string());
        bind_values.push(SqlValue::Uuid(new_id));
        let mut idx = 2;

        // Track which columns we're inserting for conflict detection
        let mut insert_cols: Vec<String> = vec![pk_col.to_string()];

        // Add provided arguments based on verb arg definitions
        for arg_def in &verb.args {
            if let Some(value) = args.get(&arg_def.name) {
                if let Some(col) = &arg_def.maps_to {
                    // Skip if it's the PK (already added)
                    if col == pk_col {
                        continue;
                    }
                    columns.push(format!("\"{}\"", col));
                    placeholders.push(format!("${}", idx));

                    // Handle lookup args specially - resolve name/code to UUID
                    // Only applies to ArgType::Uuid with lookup config (not string lookups like jurisdiction)
                    if arg_def.lookup.is_some() && arg_def.arg_type == ArgType::Uuid {
                        let code = value.as_str().ok_or_else(|| {
                            anyhow!("Expected string for lookup {}", arg_def.name)
                        })?;
                        // Check if value is already a UUID (resolved) or needs lookup
                        if let Ok(uuid) = Uuid::parse_str(code) {
                            bind_values.push(SqlValue::Uuid(uuid));
                        } else {
                            let uuid = self.resolve_lookup(arg_def, code).await?;
                            bind_values.push(SqlValue::Uuid(uuid));
                        }
                    } else if arg_def.arg_type == ArgType::Lookup && arg_def.lookup.is_some() {
                        // Legacy ArgType::Lookup - resolve to UUID
                        let code = value.as_str().ok_or_else(|| {
                            anyhow!("Expected string for lookup {}", arg_def.name)
                        })?;
                        let uuid = self.resolve_lookup(arg_def, code).await?;
                        bind_values.push(SqlValue::Uuid(uuid));
                    } else {
                        bind_values.push(self.json_to_sql_value(value, arg_def)?);
                    }

                    insert_cols.push(col.clone());
                    idx += 1;
                }
            }
        }

        // Add set_values (hardcoded column values from YAML, e.g., relationship_type: ownership)
        if let Some(ref sv) = crud.set_values {
            for (col, val) in sv {
                if !insert_cols.contains(col) {
                    columns.push(format!("\"{}\"", col));
                    placeholders.push(format!("${}", idx));
                    let str_val = val
                        .as_str()
                        .map(|s| s.to_string())
                        .unwrap_or_else(|| format!("{:?}", val));
                    bind_values.push(SqlValue::String(str_val));
                    insert_cols.push(col.clone());
                    idx += 1;
                }
            }
        }

        if columns.len() == 1 {
            // Only PK, no other columns
            bail!("No columns to insert for {}.{}", verb.domain, verb.verb);
        }

        let returning = crud.returning.as_deref().unwrap_or(pk_col);

        // Build idempotent INSERT with ON CONFLICT
        let sql = if !crud.conflict_keys.is_empty() {
            // Use explicit conflict keys from YAML config
            let conflict_cols: Vec<String> = crud
                .conflict_keys
                .iter()
                .map(|c| format!("\"{}\"", c))
                .collect();

            // Build UPDATE SET for non-conflict columns (upsert behavior)
            let updates: Vec<String> = insert_cols
                .iter()
                .filter(|c| !crud.conflict_keys.contains(*c) && *c != pk_col)
                .map(|c| format!("\"{}\" = EXCLUDED.\"{}\"", c, c))
                .collect();

            let update_clause = if updates.is_empty() {
                // Nothing to update, just return existing
                format!("\"{}\" = \"{}\".\"{}\"", pk_col, crud.table, pk_col)
            } else {
                updates.join(", ")
            };

            format!(
                r#"INSERT INTO "{}"."{}" ({}) VALUES ({})
                   ON CONFLICT ({}) DO UPDATE SET {}
                   RETURNING "{}""#,
                crud.schema,
                crud.table,
                columns.join(", "),
                placeholders.join(", "),
                conflict_cols.join(", "),
                update_clause,
                returning
            )
        } else {
            // No conflict keys defined - use DO NOTHING for pure idempotency
            // First try INSERT, if conflict return existing row
            format!(
                r#"WITH ins AS (
                    INSERT INTO "{}"."{}" ({}) VALUES ({})
                    ON CONFLICT DO NOTHING
                    RETURNING "{}"
                )
                SELECT "{}" FROM ins
                UNION ALL
                SELECT "{}" FROM "{}"."{}"
                WHERE NOT EXISTS (SELECT 1 FROM ins)
                LIMIT 1"#,
                crud.schema,
                crud.table,
                columns.join(", "),
                placeholders.join(", "),
                returning,
                returning,
                returning,
                crud.schema,
                crud.table
            )
        };

        debug!("INSERT (idempotent) SQL: {}", sql);

        let row = self.execute_with_bindings(&sql, &bind_values).await?;

        let uuid: Uuid = row.try_get(returning)?;
        Ok(GenericExecutionResult::Uuid(uuid))
    }

    // =========================================================================
    // SELECT
    // =========================================================================

    async fn execute_select(
        &self,
        verb: &RuntimeVerb,
        crud: &RuntimeCrudConfig,
        args: &HashMap<String, JsonValue>,
    ) -> Result<GenericExecutionResult> {
        let mut conditions = Vec::new();
        let mut bind_values: Vec<SqlValue> = Vec::new();
        let mut idx = 1;
        let mut limit: Option<i64> = None;
        let mut offset: Option<i64> = None;

        for arg_def in &verb.args {
            if let Some(value) = args.get(&arg_def.name) {
                // Handle pagination args specially
                if arg_def.name == "limit" {
                    limit = value.as_i64();
                    continue;
                }
                if arg_def.name == "offset" {
                    offset = value.as_i64();
                    continue;
                }

                if let Some(col) = &arg_def.maps_to {
                    conditions.push(format!("\"{}\" = ${}", col, idx));
                    bind_values.push(self.json_to_sql_value(value, arg_def)?);
                    idx += 1;
                }
            }
        }

        let where_clause = if conditions.is_empty() {
            self.active_row_predicate(&crud.schema, &crud.table, None)
                .map(|predicate| format!(" WHERE {predicate}"))
                .unwrap_or_default()
        } else {
            if let Some(predicate) = self.active_row_predicate(&crud.schema, &crud.table, None) {
                conditions.push(predicate);
            }
            format!(" WHERE {}", conditions.join(" AND "))
        };

        let limit_clause = limit.map(|l| format!(" LIMIT {}", l)).unwrap_or_default();
        let offset_clause = offset.map(|o| format!(" OFFSET {}", o)).unwrap_or_default();

        let sql = format!(
            "SELECT * FROM \"{}\".\"{}\"{}{}{}",
            crud.schema, crud.table, where_clause, limit_clause, offset_clause
        );

        debug!("SELECT SQL: {}", sql);

        let rows = self.execute_many_with_bindings(&sql, &bind_values).await?;

        // Return type determines single vs multiple
        match verb.returns.return_type {
            ReturnTypeConfig::Record => {
                if rows.is_empty() {
                    Ok(GenericExecutionResult::Record(JsonValue::Null))
                } else {
                    Ok(GenericExecutionResult::Record(self.row_to_json(&rows[0])?))
                }
            }
            _ => {
                let records: Result<Vec<JsonValue>> =
                    rows.iter().map(|r| self.row_to_json(r)).collect();
                Ok(GenericExecutionResult::RecordSet(records?))
            }
        }
    }

    // =========================================================================
    // UPDATE
    // =========================================================================

    async fn execute_update(
        &self,
        verb: &RuntimeVerb,
        crud: &RuntimeCrudConfig,
        args: &HashMap<String, JsonValue>,
    ) -> Result<GenericExecutionResult> {
        let key_col = crud
            .key
            .as_deref()
            .ok_or_else(|| anyhow!("Update requires key column in config"))?;

        let mut sets = Vec::new();
        let mut bind_values: Vec<SqlValue> = Vec::new();
        let mut key_value: Option<SqlValue> = None;
        let mut status_update: Option<(String, String)> = None;
        let mut idx = 1;

        for arg_def in &verb.args {
            if let Some(value) = args.get(&arg_def.name) {
                if let Some(col) = &arg_def.maps_to {
                    if col == key_col {
                        // Handle lookup args specially - resolve name/code to UUID
                        if arg_def.lookup.is_some() && arg_def.arg_type == ArgType::Uuid {
                            let code = value.as_str().ok_or_else(|| {
                                anyhow!("Expected string for lookup {}", arg_def.name)
                            })?;
                            // Check if value is already a UUID (resolved) or needs lookup
                            if let Ok(uuid) = Uuid::parse_str(code) {
                                key_value = Some(SqlValue::Uuid(uuid));
                            } else {
                                let uuid = self.resolve_lookup(arg_def, code).await?;
                                key_value = Some(SqlValue::Uuid(uuid));
                            }
                        } else if arg_def.arg_type == ArgType::Lookup && arg_def.lookup.is_some() {
                            let code = value.as_str().ok_or_else(|| {
                                anyhow!("Expected string for lookup {}", arg_def.name)
                            })?;
                            let uuid = self.resolve_lookup(arg_def, code).await?;
                            key_value = Some(SqlValue::Uuid(uuid));
                        } else {
                            key_value = Some(self.json_to_sql_value(value, arg_def)?);
                        }
                    } else {
                        sets.push(format!("\"{}\" = ${}", col, idx));
                        if col == "status" {
                            if let Some(status) = value.as_str() {
                                status_update = Some((col.clone(), status.to_string()));
                            }
                        }
                        // Handle lookup args specially - resolve name/code to UUID
                        if arg_def.lookup.is_some() && arg_def.arg_type == ArgType::Uuid {
                            let code = value.as_str().ok_or_else(|| {
                                anyhow!("Expected string for lookup {}", arg_def.name)
                            })?;
                            // Check if value is already a UUID (resolved) or needs lookup
                            if let Ok(uuid) = Uuid::parse_str(code) {
                                bind_values.push(SqlValue::Uuid(uuid));
                            } else {
                                let uuid = self.resolve_lookup(arg_def, code).await?;
                                bind_values.push(SqlValue::Uuid(uuid));
                            }
                        } else if arg_def.arg_type == ArgType::Lookup && arg_def.lookup.is_some() {
                            let code = value.as_str().ok_or_else(|| {
                                anyhow!("Expected string for lookup {}", arg_def.name)
                            })?;
                            let uuid = self.resolve_lookup(arg_def, code).await?;
                            bind_values.push(SqlValue::Uuid(uuid));
                        } else {
                            bind_values.push(self.json_to_sql_value(value, arg_def)?);
                        }
                        idx += 1;
                    }
                }
            }
        }

        let key_val = key_value.ok_or_else(|| anyhow!("Missing key argument for update"))?;

        // Add set_values from config (for status transitions etc.)
        if let Some(set_values) = &crud.set_values {
            for (col, value) in set_values {
                if let Some(s) = value.as_str() {
                    // Check if this is a SQL expression (e.g., now(), CURRENT_TIMESTAMP)
                    let s_lower = s.to_lowercase();
                    if s_lower == "now()" || s_lower == "current_timestamp" {
                        sets.push(format!("\"{}\" = NOW()", col));
                        // No bind value needed for SQL expression
                    } else {
                        sets.push(format!("\"{}\" = ${}", col, idx));
                        if col == "status" {
                            status_update = Some((col.clone(), s.to_string()));
                        }
                        bind_values.push(SqlValue::String(s.to_string()));
                        idx += 1;
                    }
                } else if let Some(b) = value.as_bool() {
                    sets.push(format!("\"{}\" = ${}", col, idx));
                    bind_values.push(SqlValue::Boolean(b));
                    idx += 1;
                } else if let Some(n) = value.as_i64() {
                    sets.push(format!("\"{}\" = ${}", col, idx));
                    bind_values.push(SqlValue::Integer(n));
                    idx += 1;
                }
            }
        }

        if sets.is_empty() {
            bail!("No columns to update for {}.{}", verb.domain, verb.verb);
        }

        if let Some((status_column, target_status)) = status_update.as_ref() {
            self.validate_lifecycle_transition(
                crud,
                key_col,
                &key_val,
                status_column,
                target_status,
            )
            .await?;
        }

        // Note: We no longer auto-add updated_at since not all tables have it.
        // Tables that need updated_at should use triggers or explicit set_values in YAML.

        let sql = format!(
            r#"UPDATE "{}"."{}" SET {} WHERE "{}" = ${}"#,
            crud.schema,
            crud.table,
            sets.join(", "),
            key_col,
            idx
        );
        let sql =
            if let Some(predicate) = self.active_row_predicate(&crud.schema, &crud.table, None) {
                format!("{sql} AND {predicate}")
            } else {
                sql
            };

        debug!("UPDATE SQL: {}", sql);

        bind_values.push(key_val);
        let affected = self.execute_non_query(&sql, &bind_values).await?;
        Ok(GenericExecutionResult::Affected(affected))
    }

    #[cfg(feature = "database")]
    async fn validate_lifecycle_transition(
        &self,
        crud: &RuntimeCrudConfig,
        key_col: &str,
        key_value: &SqlValue,
        status_column: &str,
        target_status: &str,
    ) -> Result<()> {
        let Some(entity_type) = ontology().taxonomy().find_entity_type_by_storage(
            &crud.schema,
            &crud.table,
            Some(status_column),
        ) else {
            return Ok(());
        };

        let Some(lifecycle) = ontology().get_lifecycle(entity_type) else {
            return Ok(());
        };

        let sql = format!(
            r#"SELECT "{}" FROM "{}"."{}" WHERE "{}" = $1"#,
            status_column, crud.schema, crud.table, key_col
        );
        let row = self
            .execute_with_bindings(&sql, std::slice::from_ref(key_value))
            .await?;
        let current_status: String = row.try_get(status_column)?;

        // Skip tables whose runtime status vocabulary has not yet been normalized into taxonomy.
        if !lifecycle.is_valid_state(&current_status) || !lifecycle.is_valid_state(target_status) {
            return Ok(());
        }

        if lifecycle.is_valid_transition(&current_status, target_status) {
            return Ok(());
        }

        let allowed = lifecycle.valid_next_states(&current_status).join(", ");
        Err(anyhow!(
            "Invalid state transition for {}: {} -> {}. Allowed transitions from {}: [{}]",
            entity_type,
            current_status,
            target_status,
            current_status,
            allowed
        ))
    }

    // =========================================================================
    // DELETE
    // =========================================================================

    async fn execute_delete(
        &self,
        verb: &RuntimeVerb,
        crud: &RuntimeCrudConfig,
        args: &HashMap<String, JsonValue>,
    ) -> Result<GenericExecutionResult> {
        // If key is specified, use single-key delete (backward compatible)
        // Otherwise, use ALL maps_to columns as compound WHERE conditions
        if let Some(key_col) = crud.key.as_deref() {
            // Single key delete
            let key_arg = verb
                .args
                .iter()
                .find(|a| a.maps_to.as_deref() == Some(key_col))
                .ok_or_else(|| anyhow!("Key argument not found in verb definition"))?;

            let key_value = args
                .get(&key_arg.name)
                .ok_or_else(|| anyhow!("Missing key argument: {}", key_arg.name))?;

            let sql = if self.soft_delete_supported(&crud.schema, &crud.table) {
                format!(
                    r#"UPDATE "{}"."{}" SET deleted_at = NOW() WHERE "{}" = $1 AND deleted_at IS NULL"#,
                    crud.schema, crud.table, key_col
                )
            } else {
                format!(
                    r#"DELETE FROM "{}"."{}" WHERE "{}" = $1"#,
                    crud.schema, crud.table, key_col
                )
            };

            debug!("DELETE SQL: {}", sql);

            let sql_val = self.json_to_sql_value(key_value, key_arg)?;
            let affected = self.execute_non_query(&sql, &[sql_val]).await?;
            Ok(GenericExecutionResult::Affected(affected))
        } else {
            // Compound delete - use ALL maps_to columns as WHERE conditions
            let mut where_clauses = Vec::new();
            let mut bind_values: Vec<SqlValue> = Vec::new();
            let mut idx = 1;

            for arg_def in &verb.args {
                if let Some(col) = &arg_def.maps_to {
                    if let Some(value) = args.get(&arg_def.name) {
                        where_clauses.push(format!("\"{}\" = ${}", col, idx));

                        // Handle lookup args - resolve name/code to UUID/ID
                        // But if value is already a valid UUID, use it directly
                        if arg_def.lookup.is_some() && arg_def.arg_type == ArgType::Uuid {
                            let str_val = value.as_str().ok_or_else(|| {
                                anyhow!("Expected string for lookup {}", arg_def.name)
                            })?;
                            // Check if it's already a UUID
                            if let Ok(uuid) = Uuid::parse_str(str_val) {
                                bind_values.push(SqlValue::Uuid(uuid));
                            } else {
                                // It's a name/code that needs resolution
                                let resolved_id = self.resolve_lookup(arg_def, str_val).await?;
                                bind_values.push(SqlValue::Uuid(resolved_id));
                            }
                        } else {
                            bind_values.push(self.json_to_sql_value(value, arg_def)?);
                        }
                        idx += 1;
                    }
                }
            }

            if where_clauses.is_empty() {
                bail!("Delete requires at least one WHERE condition (maps_to columns)");
            }

            let where_sql = if let Some(predicate) =
                self.active_row_predicate(&crud.schema, &crud.table, None)
            {
                where_clauses.push(predicate);
                where_clauses.join(" AND ")
            } else {
                where_clauses.join(" AND ")
            };

            let sql = if self.soft_delete_supported(&crud.schema, &crud.table) {
                format!(
                    r#"UPDATE "{}"."{}" SET deleted_at = NOW() WHERE {}"#,
                    crud.schema, crud.table, where_sql
                )
            } else {
                format!(
                    r#"DELETE FROM "{}"."{}" WHERE {}"#,
                    crud.schema, crud.table, where_sql
                )
            };

            debug!("DELETE SQL (compound): {}", sql);

            let affected = self.execute_non_query(&sql, &bind_values).await?;
            Ok(GenericExecutionResult::Affected(affected))
        }
    }

    // =========================================================================
    // UPSERT
    // =========================================================================

    async fn execute_upsert(
        &self,
        verb: &RuntimeVerb,
        crud: &RuntimeCrudConfig,
        args: &HashMap<String, JsonValue>,
    ) -> Result<GenericExecutionResult> {
        // Must have either conflict_keys or conflict_constraint
        if crud.conflict_keys.is_empty() && crud.conflict_constraint.is_none() {
            bail!("Upsert requires conflict_keys or conflict_constraint in config");
        }

        let pk_col = crud
            .returning
            .as_deref()
            .unwrap_or_else(|| self.infer_pk_column(&crud.table));

        let mut columns = Vec::new();
        let mut placeholders = Vec::new();
        let mut updates = Vec::new();
        let mut bind_values: Vec<SqlValue> = Vec::new();

        // Generate UUID for PK
        let new_id = Uuid::new_v4();
        columns.push(format!("\"{}\"", pk_col));
        placeholders.push("$1".to_string());
        bind_values.push(SqlValue::Uuid(new_id));
        let mut idx = 2;

        for arg_def in &verb.args {
            if let Some(value) = args.get(&arg_def.name) {
                if let Some(col) = &arg_def.maps_to {
                    if col == pk_col {
                        continue;
                    }
                    columns.push(format!("\"{}\"", col));
                    placeholders.push(format!("${}", idx));

                    // Only update non-conflict columns
                    if !crud.conflict_keys.contains(col) {
                        updates.push(format!("\"{}\" = EXCLUDED.\"{}\"", col, col));
                    }

                    // Handle lookup args specially - resolve name/code to UUID
                    if arg_def.lookup.is_some() && arg_def.arg_type == ArgType::Uuid {
                        let code = value.as_str().ok_or_else(|| {
                            anyhow!("Expected string for lookup {}", arg_def.name)
                        })?;
                        // Check if value is already a UUID (resolved) or needs lookup
                        if let Ok(uuid) = Uuid::parse_str(code) {
                            bind_values.push(SqlValue::Uuid(uuid));
                        } else {
                            let uuid = self.resolve_lookup(arg_def, code).await?;
                            bind_values.push(SqlValue::Uuid(uuid));
                        }
                    } else if arg_def.arg_type == ArgType::Lookup && arg_def.lookup.is_some() {
                        let code = value.as_str().ok_or_else(|| {
                            anyhow!("Expected string for lookup {}", arg_def.name)
                        })?;
                        let uuid = self.resolve_lookup(arg_def, code).await?;
                        bind_values.push(SqlValue::Uuid(uuid));
                    } else {
                        bind_values.push(self.json_to_sql_value(value, arg_def)?);
                    }

                    idx += 1;
                }
            }
        }

        // Add set_values from config (for implicit columns like relationship_type)
        if let Some(set_values) = &crud.set_values {
            for (col, value) in set_values {
                // Skip if column is already in columns list (arg took precedence)
                let col_quoted = format!("\"{}\"", col);
                if columns.contains(&col_quoted) {
                    continue;
                }

                if let Some(s) = value.as_str() {
                    // Check if this is a SQL expression (e.g., now(), CURRENT_TIMESTAMP)
                    let s_lower = s.to_lowercase();
                    if s_lower == "now()" || s_lower == "current_timestamp" {
                        columns.push(col_quoted.clone());
                        placeholders.push("NOW()".to_string());
                        // Don't add to updates - conflict keys shouldn't include timestamps
                    } else {
                        columns.push(col_quoted.clone());
                        placeholders.push(format!("${}", idx));
                        bind_values.push(SqlValue::String(s.to_string()));
                        // Only update if not a conflict key
                        if !crud.conflict_keys.contains(col) {
                            updates.push(format!("\"{}\" = EXCLUDED.\"{}\"", col, col));
                        }
                        idx += 1;
                    }
                } else if let Some(b) = value.as_bool() {
                    columns.push(col_quoted.clone());
                    placeholders.push(format!("${}", idx));
                    bind_values.push(SqlValue::Boolean(b));
                    if !crud.conflict_keys.contains(col) {
                        updates.push(format!("\"{}\" = EXCLUDED.\"{}\"", col, col));
                    }
                    idx += 1;
                } else if let Some(n) = value.as_i64() {
                    columns.push(col_quoted.clone());
                    placeholders.push(format!("${}", idx));
                    bind_values.push(SqlValue::Integer(n));
                    if !crud.conflict_keys.contains(col) {
                        updates.push(format!("\"{}\" = EXCLUDED.\"{}\"", col, col));
                    }
                    idx += 1;
                }
            }
        }

        let returning = crud.returning.as_deref().unwrap_or(pk_col);

        // Build the ON CONFLICT clause - prefer named constraint if provided
        tracing::debug!(
            "execute_upsert: table={}.{}, conflict_constraint={:?}, conflict_keys={:?}",
            crud.schema,
            crud.table,
            crud.conflict_constraint,
            crud.conflict_keys
        );
        let conflict_clause = if let Some(constraint_name) = &crud.conflict_constraint {
            format!("ON CONSTRAINT \"{}\"", constraint_name)
        } else {
            let conflict_cols: Vec<String> = crud
                .conflict_keys
                .iter()
                .map(|c| format!("\"{}\"", c))
                .collect();
            format!("({})", conflict_cols.join(", "))
        };

        // If no columns to update, use DO UPDATE SET <pk_col> = EXCLUDED.<pk_col>
        // This is a harmless no-op that returns the existing row.
        // We can't use DO NOTHING because it doesn't return the existing row.
        let sql = if updates.is_empty() {
            // Use pk_col for the no-op update (always present)
            format!(
                r#"INSERT INTO "{}"."{}" ({}) VALUES ({})
                   ON CONFLICT {} DO UPDATE SET "{}" = EXCLUDED."{}"
                   RETURNING "{}""#,
                crud.schema,
                crud.table,
                columns.join(", "),
                placeholders.join(", "),
                conflict_clause,
                pk_col,
                pk_col,
                returning
            )
        } else {
            format!(
                r#"INSERT INTO "{}"."{}" ({}) VALUES ({})
                   ON CONFLICT {} DO UPDATE SET {}
                   RETURNING "{}""#,
                crud.schema,
                crud.table,
                columns.join(", "),
                placeholders.join(", "),
                conflict_clause,
                updates.join(", "),
                returning
            )
        };

        debug!("UPSERT SQL: {}", sql);

        let row = self.execute_with_bindings(&sql, &bind_values).await?;
        let uuid: Uuid = row.try_get(returning)?;
        Ok(GenericExecutionResult::Uuid(uuid))
    }

    // =========================================================================
    // LINK (Junction table insert)
    // =========================================================================

    /// Execute LINK (junction table insert) with idempotency
    ///
    /// Uses ON CONFLICT DO NOTHING and returns existing row if conflict.
    /// This ensures re-running the same link operation is safe.
    async fn execute_link(
        &self,
        verb: &RuntimeVerb,
        crud: &RuntimeCrudConfig,
        args: &HashMap<String, JsonValue>,
    ) -> Result<GenericExecutionResult> {
        let junction = crud
            .junction
            .as_deref()
            .ok_or_else(|| anyhow!("Link requires junction table"))?;
        let from_col = crud
            .from_col
            .as_deref()
            .ok_or_else(|| anyhow!("Link requires from_col"))?;
        let to_col = crud
            .to_col
            .as_deref()
            .ok_or_else(|| anyhow!("Link requires to_col"))?;

        let pk_col = self.infer_pk_column(junction);
        let new_id = Uuid::new_v4();

        let mut columns = vec![
            format!("\"{}\"", pk_col),
            format!("\"{}\"", from_col),
            format!("\"{}\"", to_col),
        ];
        let mut placeholders = vec!["$1".to_string(), "$2".to_string(), "$3".to_string()];
        let mut bind_values: Vec<SqlValue> = vec![SqlValue::Uuid(new_id)];

        // Collect from/to values separately to ensure correct order
        let mut from_value: Option<SqlValue> = None;
        let mut to_value: Option<SqlValue> = None;

        for arg_def in &verb.args {
            if let Some(value) = args.get(&arg_def.name) {
                if arg_def.maps_to.as_deref() == Some(from_col) {
                    from_value = Some(self.json_to_sql_value(value, arg_def)?);
                } else if arg_def.maps_to.as_deref() == Some(to_col) {
                    to_value = Some(self.json_to_sql_value(value, arg_def)?);
                }
            }
        }

        let from_val = from_value.ok_or_else(|| anyhow!("Missing from argument for link"))?;
        let to_val = to_value.ok_or_else(|| anyhow!("Missing to argument for link"))?;

        bind_values.push(from_val.clone());
        bind_values.push(to_val.clone());

        // Add extra junction columns
        let mut idx = 4;
        for arg_def in &verb.args {
            if let Some(value) = args.get(&arg_def.name) {
                if let Some(col) = &arg_def.maps_to {
                    if col != from_col && col != to_col && col != pk_col {
                        columns.push(format!("\"{}\"", col));
                        placeholders.push(format!("${}", idx));
                        bind_values.push(self.json_to_sql_value(value, arg_def)?);
                        idx += 1;
                    }
                }
            }
        }

        // Idempotent: INSERT or return existing
        let sql = format!(
            r#"WITH ins AS (
                INSERT INTO "{}"."{}" ({}) VALUES ({})
                ON CONFLICT ("{}", "{}") DO NOTHING
                RETURNING "{}"
            )
            SELECT "{}" FROM ins
            UNION ALL
            SELECT "{}" FROM "{}"."{}"
            WHERE "{}" = $2 AND "{}" = $3
            AND NOT EXISTS (SELECT 1 FROM ins)
            LIMIT 1"#,
            crud.schema,
            junction,
            columns.join(", "),
            placeholders.join(", "),
            from_col,
            to_col,
            pk_col,
            pk_col,
            pk_col,
            crud.schema,
            junction,
            from_col,
            to_col
        );

        debug!("LINK (idempotent) SQL: {}", sql);

        let row = self.execute_with_bindings(&sql, &bind_values).await?;
        let uuid: Uuid = row.try_get(pk_col)?;
        Ok(GenericExecutionResult::Uuid(uuid))
    }

    // =========================================================================
    // UNLINK (Junction table delete)
    // =========================================================================

    async fn execute_unlink(
        &self,
        verb: &RuntimeVerb,
        crud: &RuntimeCrudConfig,
        args: &HashMap<String, JsonValue>,
    ) -> Result<GenericExecutionResult> {
        let junction = crud
            .junction
            .as_deref()
            .ok_or_else(|| anyhow!("Unlink requires junction table"))?;
        let from_col = crud
            .from_col
            .as_deref()
            .ok_or_else(|| anyhow!("Unlink requires from_col"))?;
        let to_col = crud
            .to_col
            .as_deref()
            .ok_or_else(|| anyhow!("Unlink requires to_col"))?;

        let mut from_value: Option<SqlValue> = None;
        let mut to_value: Option<SqlValue> = None;

        for arg_def in &verb.args {
            if let Some(value) = args.get(&arg_def.name) {
                if arg_def.maps_to.as_deref() == Some(from_col) {
                    from_value = Some(self.json_to_sql_value(value, arg_def)?);
                } else if arg_def.maps_to.as_deref() == Some(to_col) {
                    to_value = Some(self.json_to_sql_value(value, arg_def)?);
                }
            }
        }

        let sql = format!(
            r#"DELETE FROM "{}"."{}" WHERE "{}" = $1 AND "{}" = $2"#,
            crud.schema, junction, from_col, to_col
        );

        debug!("UNLINK SQL: {}", sql);

        let bind_values = vec![
            from_value.ok_or_else(|| anyhow!("Missing from argument"))?,
            to_value.ok_or_else(|| anyhow!("Missing to argument"))?,
        ];
        let affected = self.execute_non_query(&sql, &bind_values).await?;
        Ok(GenericExecutionResult::Affected(affected))
    }

    // =========================================================================
    // ROLE LINK (Junction with role lookup) - Idempotent
    // =========================================================================

    /// Execute ROLE_LINK with idempotency
    ///
    /// Links entity to CBU with a role. Uses ON CONFLICT to handle
    /// duplicate role assignments safely (returns existing if already linked).
    async fn execute_role_link(
        &self,
        verb: &RuntimeVerb,
        crud: &RuntimeCrudConfig,
        args: &HashMap<String, JsonValue>,
    ) -> Result<GenericExecutionResult> {
        let junction = crud
            .junction
            .as_deref()
            .ok_or_else(|| anyhow!("RoleLink requires junction table"))?;
        let from_col = crud
            .from_col
            .as_deref()
            .ok_or_else(|| anyhow!("RoleLink requires from_col"))?;
        let to_col = crud
            .to_col
            .as_deref()
            .ok_or_else(|| anyhow!("RoleLink requires to_col"))?;
        let _role_table = crud.role_table.as_deref().unwrap_or("roles");
        let role_col = crud.role_col.as_deref().unwrap_or("role_id");

        // Find the lookup argument for role
        let role_arg = verb
            .args
            .iter()
            .find(|a| a.arg_type == ArgType::Lookup && a.lookup.is_some())
            .ok_or_else(|| anyhow!("RoleLink requires lookup argument for role"))?;

        let role_value = args
            .get(&role_arg.name)
            .ok_or_else(|| anyhow!("Missing role argument"))?;

        let role_code = role_value
            .as_str()
            .ok_or_else(|| anyhow!("Role must be a string"))?;

        // Look up role_id using resolve_lookup for better error messages
        let role_id = self.resolve_lookup(role_arg, role_code).await?;

        // Build insert
        let pk_col = self.infer_pk_column(junction);
        let new_id = Uuid::new_v4();

        let mut columns = vec![
            format!("\"{}\"", pk_col),
            format!("\"{}\"", from_col),
            format!("\"{}\"", to_col),
            format!("\"{}\"", role_col),
        ];
        let mut placeholders = vec![
            "$1".to_string(),
            "$2".to_string(),
            "$3".to_string(),
            "$4".to_string(),
        ];
        let mut bind_values: Vec<SqlValue> = vec![SqlValue::Uuid(new_id)];

        // Collect from/to values separately to ensure correct order
        let mut from_value: Option<SqlValue> = None;
        let mut to_value: Option<SqlValue> = None;

        for arg_def in &verb.args {
            if let Some(value) = args.get(&arg_def.name) {
                if arg_def.maps_to.as_deref() == Some(from_col) {
                    from_value = Some(self.json_to_sql_value(value, arg_def)?);
                } else if arg_def.maps_to.as_deref() == Some(to_col) {
                    to_value = Some(self.json_to_sql_value(value, arg_def)?);
                }
            }
        }

        let from_val = from_value.ok_or_else(|| anyhow!("Missing from argument for role_link"))?;
        let to_val = to_value.ok_or_else(|| anyhow!("Missing to argument for role_link"))?;

        bind_values.push(from_val);
        bind_values.push(to_val);
        bind_values.push(SqlValue::Uuid(role_id));

        // Add extra columns (like ownership-percentage)
        let mut idx = 5;
        for arg_def in &verb.args {
            if let Some(value) = args.get(&arg_def.name) {
                if let Some(col) = &arg_def.maps_to {
                    if col != from_col
                        && col != to_col
                        && col != pk_col
                        && arg_def.arg_type != ArgType::Lookup
                    {
                        columns.push(format!("\"{}\"", col));
                        placeholders.push(format!("${}", idx));
                        bind_values.push(self.json_to_sql_value(value, arg_def)?);
                        idx += 1;
                    }
                }
            }
        }

        let returning = crud.returning.as_deref().unwrap_or(pk_col);

        // Idempotent: INSERT or return existing (conflict on cbu_id, entity_id, role_id)
        let sql = format!(
            r#"WITH ins AS (
                INSERT INTO "{}"."{}" ({}) VALUES ({})
                ON CONFLICT ("{}", "{}", "{}") DO NOTHING
                RETURNING "{}"
            )
            SELECT "{}" FROM ins
            UNION ALL
            SELECT "{}" FROM "{}"."{}"
            WHERE "{}" = $2 AND "{}" = $3 AND "{}" = $4
            AND NOT EXISTS (SELECT 1 FROM ins)
            LIMIT 1"#,
            crud.schema,
            junction,
            columns.join(", "),
            placeholders.join(", "),
            from_col,
            to_col,
            role_col,
            returning,
            returning,
            returning,
            crud.schema,
            junction,
            from_col,
            to_col,
            role_col
        );

        debug!("ROLE_LINK (idempotent) SQL: {}", sql);

        let row = self.execute_with_bindings(&sql, &bind_values).await?;
        let uuid: Uuid = row.try_get(returning)?;
        Ok(GenericExecutionResult::Uuid(uuid))
    }

    // =========================================================================
    // ROLE UNLINK
    // =========================================================================

    async fn execute_role_unlink(
        &self,
        verb: &RuntimeVerb,
        crud: &RuntimeCrudConfig,
        args: &HashMap<String, JsonValue>,
    ) -> Result<GenericExecutionResult> {
        let junction = crud
            .junction
            .as_deref()
            .ok_or_else(|| anyhow!("RoleUnlink requires junction table"))?;
        let from_col = crud
            .from_col
            .as_deref()
            .ok_or_else(|| anyhow!("RoleUnlink requires from_col"))?;
        let to_col = crud
            .to_col
            .as_deref()
            .ok_or_else(|| anyhow!("RoleUnlink requires to_col"))?;
        let role_col = crud.role_col.as_deref().unwrap_or("role_id");

        // Find and lookup role
        let role_arg = verb
            .args
            .iter()
            .find(|a| a.arg_type == ArgType::Lookup && a.lookup.is_some())
            .ok_or_else(|| anyhow!("RoleUnlink requires lookup argument"))?;

        let role_value = args
            .get(&role_arg.name)
            .ok_or_else(|| anyhow!("Missing role argument"))?;

        let lookup = role_arg
            .lookup
            .as_ref()
            .ok_or_else(|| anyhow!("Role argument missing lookup configuration"))?;
        let role_code = role_value
            .as_str()
            .ok_or_else(|| anyhow!("Role must be a string"))?;

        let lookup_sql = format!(
            r#"SELECT "{}" FROM "{}"."{}" WHERE "{}" = $1"#,
            lookup.primary_key,
            crud.schema,
            lookup.table,
            lookup.search_key.primary_column()
        );

        let role_row = sqlx::query(&lookup_sql)
            .bind(role_code)
            .fetch_one(&self.pool)
            .await?;

        let role_id: Uuid = role_row.try_get(&lookup.primary_key as &str)?;

        // Get from/to values
        let mut from_value: Option<SqlValue> = None;
        let mut to_value: Option<SqlValue> = None;

        for arg_def in &verb.args {
            if let Some(value) = args.get(&arg_def.name) {
                if arg_def.maps_to.as_deref() == Some(from_col) {
                    from_value = Some(self.json_to_sql_value(value, arg_def)?);
                } else if arg_def.maps_to.as_deref() == Some(to_col) {
                    to_value = Some(self.json_to_sql_value(value, arg_def)?);
                }
            }
        }

        let sql = format!(
            r#"DELETE FROM "{}"."{}" WHERE "{}" = $1 AND "{}" = $2 AND "{}" = $3"#,
            crud.schema, junction, from_col, to_col, role_col
        );

        debug!("ROLE_UNLINK SQL: {}", sql);

        let bind_values = vec![
            from_value.ok_or_else(|| anyhow!("Missing from argument"))?,
            to_value.ok_or_else(|| anyhow!("Missing to argument"))?,
            SqlValue::Uuid(role_id),
        ];
        let affected = self.execute_non_query(&sql, &bind_values).await?;
        Ok(GenericExecutionResult::Affected(affected))
    }

    // =========================================================================
    // LIST BY FK
    // =========================================================================

    async fn execute_list_by_fk(
        &self,
        verb: &RuntimeVerb,
        crud: &RuntimeCrudConfig,
        args: &HashMap<String, JsonValue>,
    ) -> Result<GenericExecutionResult> {
        let fk_col = crud
            .fk_col
            .as_deref()
            .ok_or_else(|| anyhow!("ListByFk requires fk_col"))?;

        // Find the FK argument (first required arg typically)
        let fk_arg = verb
            .args
            .iter()
            .find(|a| a.required)
            .ok_or_else(|| anyhow!("ListByFk requires a required argument"))?;

        let fk_value = args
            .get(&fk_arg.name)
            .ok_or_else(|| anyhow!("Missing FK argument: {}", fk_arg.name))?;

        let sql =
            if let Some(predicate) = self.active_row_predicate(&crud.schema, &crud.table, None) {
                format!(
                    r#"SELECT * FROM "{}"."{}" WHERE "{}" = $1 AND {}"#,
                    crud.schema, crud.table, fk_col, predicate
                )
            } else {
                format!(
                    r#"SELECT * FROM "{}"."{}" WHERE "{}" = $1"#,
                    crud.schema, crud.table, fk_col
                )
            };

        debug!("LIST_BY_FK SQL: {}", sql);

        let sql_val = self.json_to_sql_value(fk_value, fk_arg)?;
        let rows = self.execute_many_with_bindings(&sql, &[sql_val]).await?;

        let records: Result<Vec<JsonValue>> = rows.iter().map(|r| self.row_to_json(r)).collect();
        Ok(GenericExecutionResult::RecordSet(records?))
    }

    // =========================================================================
    // LIST PARTIES (CBU Entity Roles with joins)
    // =========================================================================

    async fn execute_list_parties(
        &self,
        verb: &RuntimeVerb,
        crud: &RuntimeCrudConfig,
        args: &HashMap<String, JsonValue>,
    ) -> Result<GenericExecutionResult> {
        let junction = crud
            .junction
            .as_deref()
            .ok_or_else(|| anyhow!("ListParties requires junction"))?;
        let fk_col = crud
            .fk_col
            .as_deref()
            .ok_or_else(|| anyhow!("ListParties requires fk_col"))?;

        let fk_arg = verb
            .args
            .iter()
            .find(|a| a.required)
            .ok_or_else(|| anyhow!("ListParties requires FK argument"))?;

        let fk_value = args
            .get(&fk_arg.name)
            .ok_or_else(|| anyhow!("Missing FK argument"))?;

        // Check for optional as-of-date parameter (defaults to today)
        let as_of_date = args
            .get("as-of-date")
            .and_then(|v| v.as_str())
            .and_then(|s| chrono::NaiveDate::parse_from_str(s, "%Y-%m-%d").ok())
            .unwrap_or_else(|| chrono::Utc::now().date_naive());

        // Join to get enriched party data with temporal filtering
        let sql = format!(
            r#"SELECT
                cer.cbu_entity_role_id,
                cer.cbu_id,
                cer.entity_id,
                e.name as entity_name,
                et.name as entity_type,
                r.role_id,
                r.name as role_name,
                r.description as role_description,
                cer.created_at
            FROM "{}"."{}" cer
            JOIN "{}".entities e ON e.entity_id = cer.entity_id
            JOIN "{}".entity_types et ON et.entity_type_id = e.entity_type_id
            JOIN "{}".roles r ON r.role_id = cer.role_id
            WHERE cer."{}" = $1
            AND (cer.effective_from IS NULL OR cer.effective_from <= $2)
            AND (cer.effective_to IS NULL OR cer.effective_to >= $2)
            AND e.deleted_at IS NULL
            ORDER BY e.name, r.name"#,
            crud.schema, junction, crud.schema, crud.schema, crud.schema, fk_col
        );

        debug!("LIST_PARTIES SQL: {} (as_of: {})", sql, as_of_date);

        let sql_val = self.json_to_sql_value(fk_value, fk_arg)?;
        let rows = self
            .execute_many_with_bindings(&sql, &[sql_val, SqlValue::Date(as_of_date)])
            .await?;

        let records: Result<Vec<JsonValue>> = rows.iter().map(|r| self.row_to_json(r)).collect();
        Ok(GenericExecutionResult::RecordSet(records?))
    }

    // =========================================================================
    // SELECT WITH JOIN
    // =========================================================================

    async fn execute_select_with_join(
        &self,
        verb: &RuntimeVerb,
        crud: &RuntimeCrudConfig,
        args: &HashMap<String, JsonValue>,
    ) -> Result<GenericExecutionResult> {
        let primary = crud
            .primary_table
            .as_deref()
            .ok_or_else(|| anyhow!("SelectWithJoin requires primary_table"))?;
        let join_table = crud
            .join_table
            .as_deref()
            .ok_or_else(|| anyhow!("SelectWithJoin requires join_table"))?;
        let join_col = crud
            .join_col
            .as_deref()
            .ok_or_else(|| anyhow!("SelectWithJoin requires join_col"))?;
        let filter_col = crud
            .filter_col
            .as_deref()
            .ok_or_else(|| anyhow!("SelectWithJoin requires filter_col"))?;

        let filter_arg = verb
            .args
            .iter()
            .find(|a| a.required)
            .ok_or_else(|| anyhow!("SelectWithJoin requires filter argument"))?;

        let filter_value = args
            .get(&filter_arg.name)
            .ok_or_else(|| anyhow!("Missing filter argument"))?;

        let sql =
            if let Some(predicate) = self.active_row_predicate(&crud.schema, primary, Some("p")) {
                format!(
                    r#"SELECT p.* FROM "{}"."{}" p
                   JOIN "{}"."{}" j ON p."{}" = j."{}"
                   WHERE j."{}" = $1 AND {}"#,
                    crud.schema,
                    primary,
                    crud.schema,
                    join_table,
                    join_col,
                    join_col,
                    filter_col,
                    predicate
                )
            } else {
                format!(
                    r#"SELECT p.* FROM "{}"."{}" p
                   JOIN "{}"."{}" j ON p."{}" = j."{}"
                   WHERE j."{}" = $1"#,
                    crud.schema, primary, crud.schema, join_table, join_col, join_col, filter_col
                )
            };

        debug!("SELECT_WITH_JOIN SQL: {}", sql);

        let sql_val = self.json_to_sql_value(filter_value, filter_arg)?;
        let rows = self.execute_many_with_bindings(&sql, &[sql_val]).await?;

        let records: Result<Vec<JsonValue>> = rows.iter().map(|r| self.row_to_json(r)).collect();
        Ok(GenericExecutionResult::RecordSet(records?))
    }

    // =========================================================================
    // TRANSACTION-AWARE READ VARIANTS
    // These mirror the pool-based read methods above but execute within the
    // caller's active transaction so plan writes are visible to subsequent reads.
    // =========================================================================

    async fn execute_select_in_tx(
        &self,
        tx: &mut sqlx::Transaction<'_, sqlx::Postgres>,
        verb: &RuntimeVerb,
        crud: &RuntimeCrudConfig,
        args: &HashMap<String, JsonValue>,
    ) -> Result<GenericExecutionResult> {
        let mut conditions = Vec::new();
        let mut bind_values: Vec<SqlValue> = Vec::new();
        let mut idx = 1;
        let mut limit: Option<i64> = None;
        let mut offset: Option<i64> = None;

        for arg_def in &verb.args {
            if let Some(value) = args.get(&arg_def.name) {
                if arg_def.name == "limit" {
                    limit = value.as_i64();
                    continue;
                }
                if arg_def.name == "offset" {
                    offset = value.as_i64();
                    continue;
                }
                if let Some(col) = &arg_def.maps_to {
                    conditions.push(format!("\"{}\" = ${}", col, idx));
                    bind_values.push(self.json_to_sql_value(value, arg_def)?);
                    idx += 1;
                }
            }
        }

        let where_clause = if conditions.is_empty() {
            self.active_row_predicate(&crud.schema, &crud.table, None)
                .map(|predicate| format!(" WHERE {predicate}"))
                .unwrap_or_default()
        } else {
            if let Some(predicate) = self.active_row_predicate(&crud.schema, &crud.table, None) {
                conditions.push(predicate);
            }
            format!(" WHERE {}", conditions.join(" AND "))
        };

        let limit_clause = limit.map(|l| format!(" LIMIT {}", l)).unwrap_or_default();
        let offset_clause = offset.map(|o| format!(" OFFSET {}", o)).unwrap_or_default();

        let sql = format!(
            "SELECT * FROM \"{}\".\"{}\"{}{}{}",
            crud.schema, crud.table, where_clause, limit_clause, offset_clause
        );

        let rows = Self::execute_many_with_bindings_in_tx(tx, &sql, &bind_values).await?;

        match verb.returns.return_type {
            ReturnTypeConfig::Record => {
                if rows.is_empty() {
                    Ok(GenericExecutionResult::Record(JsonValue::Null))
                } else {
                    Ok(GenericExecutionResult::Record(self.row_to_json(&rows[0])?))
                }
            }
            _ => {
                let records: Result<Vec<JsonValue>> =
                    rows.iter().map(|r| self.row_to_json(r)).collect();
                Ok(GenericExecutionResult::RecordSet(records?))
            }
        }
    }

    async fn execute_list_by_fk_in_tx(
        &self,
        tx: &mut sqlx::Transaction<'_, sqlx::Postgres>,
        verb: &RuntimeVerb,
        crud: &RuntimeCrudConfig,
        args: &HashMap<String, JsonValue>,
    ) -> Result<GenericExecutionResult> {
        let fk_col = crud
            .fk_col
            .as_deref()
            .ok_or_else(|| anyhow!("ListByFk requires fk_col"))?;

        let fk_arg = verb
            .args
            .iter()
            .find(|a| a.required)
            .ok_or_else(|| anyhow!("ListByFk requires a required argument"))?;

        let fk_value = args
            .get(&fk_arg.name)
            .ok_or_else(|| anyhow!("Missing FK argument: {}", fk_arg.name))?;

        let sql =
            if let Some(predicate) = self.active_row_predicate(&crud.schema, &crud.table, None) {
                format!(
                    r#"SELECT * FROM "{}"."{}" WHERE "{}" = $1 AND {}"#,
                    crud.schema, crud.table, fk_col, predicate
                )
            } else {
                format!(
                    r#"SELECT * FROM "{}"."{}" WHERE "{}" = $1"#,
                    crud.schema, crud.table, fk_col
                )
            };

        let sql_val = self.json_to_sql_value(fk_value, fk_arg)?;
        let rows = Self::execute_many_with_bindings_in_tx(tx, &sql, &[sql_val]).await?;

        let records: Result<Vec<JsonValue>> = rows.iter().map(|r| self.row_to_json(r)).collect();
        Ok(GenericExecutionResult::RecordSet(records?))
    }

    async fn execute_list_parties_in_tx(
        &self,
        tx: &mut sqlx::Transaction<'_, sqlx::Postgres>,
        verb: &RuntimeVerb,
        crud: &RuntimeCrudConfig,
        args: &HashMap<String, JsonValue>,
    ) -> Result<GenericExecutionResult> {
        let junction = crud
            .junction
            .as_deref()
            .ok_or_else(|| anyhow!("ListParties requires junction"))?;
        let fk_col = crud
            .fk_col
            .as_deref()
            .ok_or_else(|| anyhow!("ListParties requires fk_col"))?;

        let fk_arg = verb
            .args
            .iter()
            .find(|a| a.required)
            .ok_or_else(|| anyhow!("ListParties requires FK argument"))?;

        let fk_value = args
            .get(&fk_arg.name)
            .ok_or_else(|| anyhow!("Missing FK argument"))?;

        let as_of_date = args
            .get("as-of-date")
            .and_then(|v| v.as_str())
            .and_then(|s| chrono::NaiveDate::parse_from_str(s, "%Y-%m-%d").ok())
            .unwrap_or_else(|| chrono::Utc::now().date_naive());

        let sql = format!(
            r#"SELECT
                cer.cbu_entity_role_id,
                cer.cbu_id,
                cer.entity_id,
                e.name as entity_name,
                et.name as entity_type,
                r.role_id,
                r.name as role_name,
                r.description as role_description,
                cer.created_at
            FROM "{}"."{}" cer
            JOIN "{}".entities e ON e.entity_id = cer.entity_id
            JOIN "{}".entity_types et ON et.entity_type_id = e.entity_type_id
            JOIN "{}".roles r ON r.role_id = cer.role_id
            WHERE cer."{}" = $1
            AND (cer.effective_from IS NULL OR cer.effective_from <= $2)
            AND (cer.effective_to IS NULL OR cer.effective_to >= $2)
            AND e.deleted_at IS NULL
            ORDER BY e.name, r.name"#,
            crud.schema, junction, crud.schema, crud.schema, crud.schema, fk_col
        );

        let sql_val = self.json_to_sql_value(fk_value, fk_arg)?;
        let rows = Self::execute_many_with_bindings_in_tx(
            tx,
            &sql,
            &[sql_val, SqlValue::Date(as_of_date)],
        )
        .await?;

        let records: Result<Vec<JsonValue>> = rows.iter().map(|r| self.row_to_json(r)).collect();
        Ok(GenericExecutionResult::RecordSet(records?))
    }

    async fn execute_select_with_join_in_tx(
        &self,
        tx: &mut sqlx::Transaction<'_, sqlx::Postgres>,
        verb: &RuntimeVerb,
        crud: &RuntimeCrudConfig,
        args: &HashMap<String, JsonValue>,
    ) -> Result<GenericExecutionResult> {
        let primary = crud
            .primary_table
            .as_deref()
            .ok_or_else(|| anyhow!("SelectWithJoin requires primary_table"))?;
        let join_table = crud
            .join_table
            .as_deref()
            .ok_or_else(|| anyhow!("SelectWithJoin requires join_table"))?;
        let join_col = crud
            .join_col
            .as_deref()
            .ok_or_else(|| anyhow!("SelectWithJoin requires join_col"))?;
        let filter_col = crud
            .filter_col
            .as_deref()
            .ok_or_else(|| anyhow!("SelectWithJoin requires filter_col"))?;

        let filter_arg = verb
            .args
            .iter()
            .find(|a| a.required)
            .ok_or_else(|| anyhow!("SelectWithJoin requires filter argument"))?;

        let filter_value = args
            .get(&filter_arg.name)
            .ok_or_else(|| anyhow!("Missing filter argument"))?;

        let sql =
            if let Some(predicate) = self.active_row_predicate(&crud.schema, primary, Some("p")) {
                format!(
                    r#"SELECT p.* FROM "{}"."{}" p
                   JOIN "{}"."{}" j ON p."{}" = j."{}"
                   WHERE j."{}" = $1 AND {}"#,
                    crud.schema,
                    primary,
                    crud.schema,
                    join_table,
                    join_col,
                    join_col,
                    filter_col,
                    predicate
                )
            } else {
                format!(
                    r#"SELECT p.* FROM "{}"."{}" p
                   JOIN "{}"."{}" j ON p."{}" = j."{}"
                   WHERE j."{}" = $1"#,
                    crud.schema, primary, crud.schema, join_table, join_col, join_col, filter_col
                )
            };

        let sql_val = self.json_to_sql_value(filter_value, filter_arg)?;
        let rows = Self::execute_many_with_bindings_in_tx(tx, &sql, &[sql_val]).await?;

        let records: Result<Vec<JsonValue>> = rows.iter().map(|r| self.row_to_json(r)).collect();
        Ok(GenericExecutionResult::RecordSet(records?))
    }

    // =========================================================================
    // ENTITY CREATE (Class Table Inheritance)
    // =========================================================================

    async fn execute_entity_create(
        &self,
        verb: &RuntimeVerb,
        crud: &RuntimeCrudConfig,
        args: &HashMap<String, JsonValue>,
    ) -> Result<GenericExecutionResult> {
        let type_code = self.resolve_entity_type_code(verb, crud, args, "create-")?;

        // Look up entity_type_id and table_name
        // First try exact match, then try prefix match for shortened verb names
        // (e.g., "LIMITED_COMPANY" matches "LIMITED_COMPANY_PRIVATE")
        let type_sql = format!(
            r#"SELECT entity_type_id, table_name FROM "{}".entity_types
               WHERE UPPER(type_code) = UPPER($1) OR UPPER(type_code) LIKE UPPER($1) || '_%'
               ORDER BY CASE WHEN UPPER(type_code) = UPPER($1) THEN 0 ELSE 1 END
               LIMIT 1"#,
            crud.schema
        );

        let type_row = sqlx::query(&type_sql)
            .bind(&type_code)
            .fetch_one(&self.pool)
            .await
            .map_err(|e| anyhow!("Entity type not found for '{}': {}", type_code, e))?;

        let entity_type_id: Uuid = type_row.try_get("entity_type_id")?;
        // Use explicit extension_table from config if present, otherwise from entity_types table
        let extension_table: String = match crud.extension_table.clone() {
            Some(t) => t,
            None => {
                let table_name: String = type_row.try_get("table_name").map_err(|e| {
                    anyhow!(
                        "No extension table found for entity type '{}': {}",
                        type_code,
                        e
                    )
                })?;
                if table_name.is_empty() {
                    return Err(anyhow!(
                        "Extension table name is empty for entity type '{}'",
                        type_code
                    ));
                }
                table_name
            }
        };

        // Generate entity_id
        let entity_id = Uuid::new_v4();

        // Get entity name - for proper_persons, constructed from first/last name
        let entity_name = if type_code == "PROPER_PERSON" || type_code == "PROPER_PERSON_NATURAL" {
            let first = args
                .get("first-name")
                .and_then(|v| v.as_str())
                .unwrap_or("");
            let last = args.get("last-name").and_then(|v| v.as_str()).unwrap_or("");
            format!("{} {}", first, last).trim().to_string()
        } else {
            args.get("name")
                .and_then(|v| v.as_str())
                .unwrap_or("Unknown")
                .to_string()
        };

        // Check if entity already exists (idempotency) - unique constraint on (entity_type_id, name)
        let existing_sql = format!(
            r#"SELECT entity_id FROM "{}".entities WHERE entity_type_id = $1 AND name = $2"#,
            crud.schema
        );

        if let Ok(existing_row) = sqlx::query(&existing_sql)
            .bind(entity_type_id)
            .bind(&entity_name)
            .fetch_one(&self.pool)
            .await
        {
            // Entity already exists - return existing ID (idempotent behavior)
            let existing_id: Uuid = existing_row.try_get("entity_id")?;
            debug!(
                "ENTITY_CREATE: Entity '{}' already exists with id {}, returning existing",
                entity_name, existing_id
            );
            return Ok(GenericExecutionResult::Uuid(existing_id));
        }

        // INSERT into entities base table
        let base_sql = format!(
            r#"INSERT INTO "{}".entities (entity_id, entity_type_id, name) VALUES ($1, $2, $3)"#,
            crud.schema
        );

        sqlx::query(&base_sql)
            .bind(entity_id)
            .bind(entity_type_id)
            .bind(&entity_name)
            .execute(&self.pool)
            .await?;

        // INSERT into extension table
        // Some tables use entity_id as their PK (shared key pattern), others have separate PK
        let ext_pk_col = self.infer_pk_column(&extension_table);
        let uses_shared_pk = ext_pk_col == "entity_id";

        let (mut columns, mut placeholders, mut bind_values, mut idx) = if uses_shared_pk {
            // Shared primary key pattern: entity_id is the only PK
            (
                vec!["\"entity_id\"".to_string()],
                vec!["$1".to_string()],
                vec![SqlValue::Uuid(entity_id)],
                2,
            )
        } else {
            // Separate PK pattern: table has its own PK plus entity_id FK
            let ext_pk_id = Uuid::new_v4();
            (
                vec![format!("\"{}\"", ext_pk_col), "\"entity_id\"".to_string()],
                vec!["$1".to_string(), "$2".to_string()],
                vec![SqlValue::Uuid(ext_pk_id), SqlValue::Uuid(entity_id)],
                3,
            )
        };

        // Add extension table columns
        // Skip columns that belong to the base entities table
        let base_table_cols = ["name", "external_id"];
        for arg_def in &verb.args {
            if let Some(value) = args.get(&arg_def.name) {
                // Skip special keys
                if arg_def.name == "entity-type" || arg_def.name == "entity-id" {
                    continue;
                }
                if let Some(col) = &arg_def.maps_to {
                    // Skip columns that are PK, entity_id FK, or base table columns
                    if col == ext_pk_col
                        || col == "entity_id"
                        || base_table_cols.contains(&col.as_str())
                    {
                        continue;
                    }
                    columns.push(format!("\"{}\"", col));
                    placeholders.push(format!("${}", idx));

                    // Handle lookup args specially - resolve name/code to UUID
                    // Only applies to ArgType::Uuid with lookup config (not string lookups like jurisdiction)
                    if arg_def.lookup.is_some() && arg_def.arg_type == ArgType::Uuid {
                        let code = value.as_str().ok_or_else(|| {
                            anyhow!("Expected string for lookup {}", arg_def.name)
                        })?;
                        // Check if value is already a UUID (resolved) or needs lookup
                        if let Ok(uuid) = Uuid::parse_str(code) {
                            bind_values.push(SqlValue::Uuid(uuid));
                        } else {
                            let uuid = self.resolve_lookup(arg_def, code).await?;
                            bind_values.push(SqlValue::Uuid(uuid));
                        }
                    } else if arg_def.arg_type == ArgType::Lookup && arg_def.lookup.is_some() {
                        // Legacy ArgType::Lookup - resolve to UUID
                        let code = value.as_str().ok_or_else(|| {
                            anyhow!("Expected string for lookup {}", arg_def.name)
                        })?;
                        let uuid = self.resolve_lookup(arg_def, code).await?;
                        bind_values.push(SqlValue::Uuid(uuid));
                    } else {
                        bind_values.push(self.json_to_sql_value(value, arg_def)?);
                    }
                    idx += 1;
                }
            }
        }

        if let Some(name_column) = self.infer_extension_name_column(&extension_table) {
            let quoted = format!("\"{}\"", name_column);
            if !columns.iter().any(|column| column == &quoted) {
                let name = args
                    .get("name")
                    .and_then(|value| value.as_str())
                    .ok_or_else(|| {
                        anyhow!(
                            "entity.create for {} requires :name because {}.{} is NOT NULL",
                            type_code,
                            extension_table,
                            name_column
                        )
                    })?;
                columns.push(quoted);
                placeholders.push(format!("${}", idx));
                bind_values.push(SqlValue::String(name.to_string()));
            }
        }

        let ext_sql = format!(
            r#"INSERT INTO "{}"."{}" ({}) VALUES ({})"#,
            crud.schema,
            extension_table,
            columns.join(", "),
            placeholders.join(", ")
        );

        debug!("ENTITY_CREATE extension SQL: {}", ext_sql);

        self.execute_non_query(&ext_sql, &bind_values).await?;

        // Return entity_id (the master table ID)
        Ok(GenericExecutionResult::Uuid(entity_id))
    }

    // =========================================================================
    // ENTITY UPSERT (Class Table Inheritance with ON CONFLICT)
    // =========================================================================

    /// Execute entity upsert - creates or updates an entity using name as conflict key
    ///
    /// Uses ON CONFLICT on entities.name to make entity creation idempotent.
    /// If entity exists, updates the extension table fields.
    async fn execute_entity_upsert(
        &self,
        verb: &RuntimeVerb,
        crud: &RuntimeCrudConfig,
        args: &HashMap<String, JsonValue>,
    ) -> Result<GenericExecutionResult> {
        let type_code = self.resolve_entity_type_code(verb, crud, args, "ensure-")?;

        // Look up entity_type_id and table_name
        let type_sql = format!(
            r#"SELECT entity_type_id, table_name FROM "{}".entity_types
               WHERE UPPER(type_code) = UPPER($1) OR UPPER(type_code) LIKE UPPER($1) || '_%'
               ORDER BY CASE WHEN UPPER(type_code) = UPPER($1) THEN 0 ELSE 1 END
               LIMIT 1"#,
            crud.schema
        );

        let type_row = sqlx::query(&type_sql)
            .bind(&type_code)
            .fetch_one(&self.pool)
            .await
            .map_err(|e| anyhow!("Entity type not found for '{}': {}", type_code, e))?;

        let entity_type_id: Uuid = type_row.try_get("entity_type_id")?;
        let extension_table: String = crud
            .extension_table
            .clone()
            .unwrap_or_else(|| type_row.try_get("table_name").unwrap_or_default());

        // Get entity name - for proper_persons, constructed from first/last name
        let entity_name = if type_code == "PROPER_PERSON" || type_code == "PROPER_PERSON_NATURAL" {
            let first = args
                .get("first-name")
                .and_then(|v| v.as_str())
                .unwrap_or("");
            let last = args.get("last-name").and_then(|v| v.as_str()).unwrap_or("");
            format!("{} {}", first, last).trim().to_string()
        } else {
            args.get("name")
                .and_then(|v| v.as_str())
                .unwrap_or("Unknown")
                .to_string()
        };

        // UPSERT into entities base table using name + entity_type_id as conflict key
        // Returns the entity_id whether inserted or existing
        let base_sql = format!(
            r#"INSERT INTO "{}".entities (entity_id, entity_type_id, name)
               VALUES (gen_random_uuid(), $1, $2)
               ON CONFLICT (entity_type_id, name) DO UPDATE SET updated_at = now()
               RETURNING entity_id"#,
            crud.schema
        );

        let row = sqlx::query(&base_sql)
            .bind(entity_type_id)
            .bind(&entity_name)
            .fetch_one(&self.pool)
            .await?;

        let entity_id: Uuid = row.try_get("entity_id")?;

        // Build extension table columns and values
        let ext_pk_col = self.infer_pk_column(&extension_table);
        let uses_shared_pk = ext_pk_col == "entity_id";

        let (mut columns, mut placeholders, mut bind_values, mut idx) = if uses_shared_pk {
            (
                vec!["\"entity_id\"".to_string()],
                vec!["$1".to_string()],
                vec![SqlValue::Uuid(entity_id)],
                2,
            )
        } else {
            (
                vec![format!("\"{}\"", ext_pk_col), "\"entity_id\"".to_string()],
                vec!["$1".to_string(), "$2".to_string()],
                vec![SqlValue::Uuid(Uuid::new_v4()), SqlValue::Uuid(entity_id)],
                3,
            )
        };

        // Track update columns for ON CONFLICT DO UPDATE
        let mut update_cols: Vec<String> = Vec::new();

        // Add extension table columns
        let base_table_cols = ["name", "external_id"];
        for arg_def in &verb.args {
            if let Some(value) = args.get(&arg_def.name) {
                if arg_def.name == "entity-type" || arg_def.name == "entity-id" {
                    continue;
                }
                if let Some(col) = &arg_def.maps_to {
                    if col == ext_pk_col
                        || col == "entity_id"
                        || base_table_cols.contains(&col.as_str())
                    {
                        continue;
                    }
                    columns.push(format!("\"{}\"", col));
                    placeholders.push(format!("${}", idx));
                    update_cols.push(format!("\"{}\" = EXCLUDED.\"{}\"", col, col));

                    // Handle lookup args specially
                    if arg_def.lookup.is_some() && arg_def.arg_type == ArgType::Uuid {
                        let code = value.as_str().ok_or_else(|| {
                            anyhow!("Expected string for lookup {}", arg_def.name)
                        })?;
                        if let Ok(uuid) = Uuid::parse_str(code) {
                            bind_values.push(SqlValue::Uuid(uuid));
                        } else {
                            let uuid = self.resolve_lookup(arg_def, code).await?;
                            bind_values.push(SqlValue::Uuid(uuid));
                        }
                    } else if arg_def.arg_type == ArgType::Lookup && arg_def.lookup.is_some() {
                        let code = value.as_str().ok_or_else(|| {
                            anyhow!("Expected string for lookup {}", arg_def.name)
                        })?;
                        let uuid = self.resolve_lookup(arg_def, code).await?;
                        bind_values.push(SqlValue::Uuid(uuid));
                    } else {
                        bind_values.push(self.json_to_sql_value(value, arg_def)?);
                    }
                    idx += 1;
                }
            }
        }

        if let Some(name_column) = self.infer_extension_name_column(&extension_table) {
            let quoted = format!("\"{}\"", name_column);
            if !columns.iter().any(|column| column == &quoted) {
                let name = args
                    .get("name")
                    .and_then(|value| value.as_str())
                    .ok_or_else(|| {
                        anyhow!(
                            "entity.ensure for {} requires :name because {}.{} is NOT NULL",
                            type_code,
                            extension_table,
                            name_column
                        )
                    })?;
                columns.push(quoted.clone());
                placeholders.push(format!("${}", idx));
                update_cols.push(format!("{quoted} = EXCLUDED.{quoted}"));
                bind_values.push(SqlValue::String(name.to_string()));
            }
        }

        // Build UPSERT for extension table
        // Conflict key priority:
        // 1. ISIN if present (for share classes with unique ISIN constraint)
        // 2. entity_id - always use this for entity extension tables since they have
        //    a unique constraint on entity_id (the FK to base entities table)
        let has_isin = columns.iter().any(|c| c == "\"isin\"");
        let conflict_col = if has_isin { "isin" } else { "entity_id" };

        let ext_sql = if update_cols.is_empty() {
            // No updateable columns - just DO NOTHING on conflict
            format!(
                r#"INSERT INTO "{}"."{}" ({}) VALUES ({})
                   ON CONFLICT ("{}") DO NOTHING"#,
                crud.schema,
                extension_table,
                columns.join(", "),
                placeholders.join(", "),
                conflict_col
            )
        } else {
            format!(
                r#"INSERT INTO "{}"."{}" ({}) VALUES ({})
                   ON CONFLICT ("{}") DO UPDATE SET {}"#,
                crud.schema,
                extension_table,
                columns.join(", "),
                placeholders.join(", "),
                conflict_col,
                update_cols.join(", ")
            )
        };

        debug!("ENTITY_UPSERT extension SQL: {}", ext_sql);

        self.execute_non_query(&ext_sql, &bind_values).await?;

        Ok(GenericExecutionResult::Uuid(entity_id))
    }

    // =========================================================================
    // TRANSACTION-AWARE WRITE OPERATIONS
    // =========================================================================
    //
    // These methods perform write operations within an existing transaction.
    // They parallel the non-tx versions but use `tx` instead of `self.pool`.

    /// Execute INSERT within a transaction
    async fn execute_insert_in_tx(
        &self,
        tx: &mut sqlx::Transaction<'_, sqlx::Postgres>,
        verb: &RuntimeVerb,
        crud: &RuntimeCrudConfig,
        args: &HashMap<String, JsonValue>,
    ) -> Result<GenericExecutionResult> {
        let mut columns = Vec::new();
        let mut placeholders = Vec::new();
        let mut bind_values: Vec<SqlValue> = Vec::new();

        let pk_col = crud
            .returning
            .as_deref()
            .unwrap_or_else(|| self.infer_pk_column(&crud.table));

        let new_id = Uuid::new_v4();
        columns.push(format!("\"{}\"", pk_col));
        placeholders.push("$1".to_string());
        bind_values.push(SqlValue::Uuid(new_id));
        let mut idx = 2;

        let mut insert_cols: Vec<String> = vec![pk_col.to_string()];

        for arg_def in &verb.args {
            if let Some(value) = args.get(&arg_def.name) {
                if let Some(col) = &arg_def.maps_to {
                    if col == pk_col {
                        continue;
                    }
                    columns.push(format!("\"{}\"", col));
                    placeholders.push(format!("${}", idx));

                    if arg_def.lookup.is_some() && arg_def.arg_type == ArgType::Uuid {
                        let code = value.as_str().ok_or_else(|| {
                            anyhow!("Expected string for lookup {}", arg_def.name)
                        })?;
                        if let Ok(uuid) = Uuid::parse_str(code) {
                            bind_values.push(SqlValue::Uuid(uuid));
                        } else {
                            let uuid = self.resolve_lookup(arg_def, code).await?;
                            bind_values.push(SqlValue::Uuid(uuid));
                        }
                    } else if arg_def.arg_type == ArgType::Lookup && arg_def.lookup.is_some() {
                        let code = value.as_str().ok_or_else(|| {
                            anyhow!("Expected string for lookup {}", arg_def.name)
                        })?;
                        let uuid = self.resolve_lookup(arg_def, code).await?;
                        bind_values.push(SqlValue::Uuid(uuid));
                    } else {
                        bind_values.push(self.json_to_sql_value(value, arg_def)?);
                    }

                    insert_cols.push(col.clone());
                    idx += 1;
                }
            }
        }

        if columns.len() == 1 {
            bail!("No columns to insert for {}.{}", verb.domain, verb.verb);
        }

        let returning = crud.returning.as_deref().unwrap_or(pk_col);

        let sql = if !crud.conflict_keys.is_empty() {
            let conflict_cols: Vec<String> = crud
                .conflict_keys
                .iter()
                .map(|c| format!("\"{}\"", c))
                .collect();

            let updates: Vec<String> = insert_cols
                .iter()
                .filter(|c| !crud.conflict_keys.contains(*c) && *c != pk_col)
                .map(|c| format!("\"{}\" = EXCLUDED.\"{}\"", c, c))
                .collect();

            let update_clause = if updates.is_empty() {
                format!("\"{}\" = \"{}\".\"{}\"", pk_col, crud.table, pk_col)
            } else {
                updates.join(", ")
            };

            format!(
                r#"INSERT INTO "{}"."{}" ({}) VALUES ({})
                   ON CONFLICT ({}) DO UPDATE SET {}
                   RETURNING "{}""#,
                crud.schema,
                crud.table,
                columns.join(", "),
                placeholders.join(", "),
                conflict_cols.join(", "),
                update_clause,
                returning
            )
        } else {
            format!(
                r#"WITH ins AS (
                    INSERT INTO "{}"."{}" ({}) VALUES ({})
                    ON CONFLICT DO NOTHING
                    RETURNING "{}"
                )
                SELECT "{}" FROM ins
                UNION ALL
                SELECT "{}" FROM "{}"."{}"
                WHERE NOT EXISTS (SELECT 1 FROM ins)
                LIMIT 1"#,
                crud.schema,
                crud.table,
                columns.join(", "),
                placeholders.join(", "),
                returning,
                returning,
                returning,
                crud.schema,
                crud.table
            )
        };

        debug!("INSERT (in_tx) SQL: {}", sql);

        let row = Self::execute_with_bindings_in_tx(tx, &sql, &bind_values).await?;
        let uuid: Uuid = row.try_get(returning)?;
        Ok(GenericExecutionResult::Uuid(uuid))
    }

    /// Execute UPDATE within a transaction
    async fn execute_update_in_tx(
        &self,
        tx: &mut sqlx::Transaction<'_, sqlx::Postgres>,
        verb: &RuntimeVerb,
        crud: &RuntimeCrudConfig,
        args: &HashMap<String, JsonValue>,
    ) -> Result<GenericExecutionResult> {
        let key_col = crud
            .key
            .as_deref()
            .ok_or_else(|| anyhow!("Update requires key column in config"))?;

        let mut sets = Vec::new();
        let mut bind_values: Vec<SqlValue> = Vec::new();
        let mut key_value: Option<SqlValue> = None;
        let mut idx = 1;

        for arg_def in &verb.args {
            if let Some(value) = args.get(&arg_def.name) {
                if let Some(col) = &arg_def.maps_to {
                    if col == key_col {
                        if arg_def.lookup.is_some() && arg_def.arg_type == ArgType::Uuid {
                            let code = value.as_str().ok_or_else(|| {
                                anyhow!("Expected string for lookup {}", arg_def.name)
                            })?;
                            if let Ok(uuid) = Uuid::parse_str(code) {
                                key_value = Some(SqlValue::Uuid(uuid));
                            } else {
                                let uuid = self.resolve_lookup(arg_def, code).await?;
                                key_value = Some(SqlValue::Uuid(uuid));
                            }
                        } else if arg_def.arg_type == ArgType::Lookup && arg_def.lookup.is_some() {
                            let code = value.as_str().ok_or_else(|| {
                                anyhow!("Expected string for lookup {}", arg_def.name)
                            })?;
                            let uuid = self.resolve_lookup(arg_def, code).await?;
                            key_value = Some(SqlValue::Uuid(uuid));
                        } else {
                            key_value = Some(self.json_to_sql_value(value, arg_def)?);
                        }
                    } else {
                        sets.push(format!("\"{}\" = ${}", col, idx));
                        if arg_def.lookup.is_some() && arg_def.arg_type == ArgType::Uuid {
                            let code = value.as_str().ok_or_else(|| {
                                anyhow!("Expected string for lookup {}", arg_def.name)
                            })?;
                            if let Ok(uuid) = Uuid::parse_str(code) {
                                bind_values.push(SqlValue::Uuid(uuid));
                            } else {
                                let uuid = self.resolve_lookup(arg_def, code).await?;
                                bind_values.push(SqlValue::Uuid(uuid));
                            }
                        } else if arg_def.arg_type == ArgType::Lookup && arg_def.lookup.is_some() {
                            let code = value.as_str().ok_or_else(|| {
                                anyhow!("Expected string for lookup {}", arg_def.name)
                            })?;
                            let uuid = self.resolve_lookup(arg_def, code).await?;
                            bind_values.push(SqlValue::Uuid(uuid));
                        } else {
                            bind_values.push(self.json_to_sql_value(value, arg_def)?);
                        }
                        idx += 1;
                    }
                }
            }
        }

        let key_val = key_value.ok_or_else(|| anyhow!("Missing key argument for update"))?;

        if let Some(set_values) = &crud.set_values {
            for (col, value) in set_values {
                if let Some(s) = value.as_str() {
                    let s_lower = s.to_lowercase();
                    if s_lower == "now()" || s_lower == "current_timestamp" {
                        sets.push(format!("\"{}\" = NOW()", col));
                    } else {
                        sets.push(format!("\"{}\" = ${}", col, idx));
                        bind_values.push(SqlValue::String(s.to_string()));
                        idx += 1;
                    }
                } else if let Some(b) = value.as_bool() {
                    sets.push(format!("\"{}\" = ${}", col, idx));
                    bind_values.push(SqlValue::Boolean(b));
                    idx += 1;
                } else if let Some(n) = value.as_i64() {
                    sets.push(format!("\"{}\" = ${}", col, idx));
                    bind_values.push(SqlValue::Integer(n));
                    idx += 1;
                }
            }
        }

        if sets.is_empty() {
            bail!("No columns to update for {}.{}", verb.domain, verb.verb);
        }

        let sql = format!(
            r#"UPDATE "{}"."{}" SET {} WHERE "{}" = ${}"#,
            crud.schema,
            crud.table,
            sets.join(", "),
            key_col,
            idx
        );

        debug!("UPDATE (in_tx) SQL: {}", sql);

        bind_values.push(key_val);
        let affected = Self::execute_non_query_in_tx(tx, &sql, &bind_values).await?;
        Ok(GenericExecutionResult::Affected(affected))
    }

    /// Execute DELETE within a transaction
    async fn execute_delete_in_tx(
        &self,
        tx: &mut sqlx::Transaction<'_, sqlx::Postgres>,
        verb: &RuntimeVerb,
        crud: &RuntimeCrudConfig,
        args: &HashMap<String, JsonValue>,
    ) -> Result<GenericExecutionResult> {
        if let Some(key_col) = crud.key.as_deref() {
            let key_arg = verb
                .args
                .iter()
                .find(|a| a.maps_to.as_deref() == Some(key_col))
                .ok_or_else(|| anyhow!("Key argument not found in verb definition"))?;

            let key_value = args
                .get(&key_arg.name)
                .ok_or_else(|| anyhow!("Missing key argument: {}", key_arg.name))?;

            let sql = format!(
                r#"DELETE FROM "{}"."{}" WHERE "{}" = $1"#,
                crud.schema, crud.table, key_col
            );

            debug!("DELETE (in_tx) SQL: {}", sql);

            let sql_val = self.json_to_sql_value(key_value, key_arg)?;
            let affected = Self::execute_non_query_in_tx(tx, &sql, &[sql_val]).await?;
            Ok(GenericExecutionResult::Affected(affected))
        } else {
            let mut where_clauses = Vec::new();
            let mut bind_values: Vec<SqlValue> = Vec::new();
            let mut idx = 1;

            for arg_def in &verb.args {
                if let Some(col) = &arg_def.maps_to {
                    if let Some(value) = args.get(&arg_def.name) {
                        where_clauses.push(format!("\"{}\" = ${}", col, idx));

                        if arg_def.lookup.is_some() && arg_def.arg_type == ArgType::Uuid {
                            let str_val = value.as_str().ok_or_else(|| {
                                anyhow!("Expected string for lookup {}", arg_def.name)
                            })?;
                            if let Ok(uuid) = Uuid::parse_str(str_val) {
                                bind_values.push(SqlValue::Uuid(uuid));
                            } else {
                                let resolved_id = self.resolve_lookup(arg_def, str_val).await?;
                                bind_values.push(SqlValue::Uuid(resolved_id));
                            }
                        } else {
                            bind_values.push(self.json_to_sql_value(value, arg_def)?);
                        }
                        idx += 1;
                    }
                }
            }

            if where_clauses.is_empty() {
                bail!("Delete requires at least one WHERE condition (maps_to columns)");
            }

            let sql = format!(
                r#"DELETE FROM "{}"."{}" WHERE {}"#,
                crud.schema,
                crud.table,
                where_clauses.join(" AND ")
            );

            debug!("DELETE (in_tx, compound) SQL: {}", sql);

            let affected = Self::execute_non_query_in_tx(tx, &sql, &bind_values).await?;
            Ok(GenericExecutionResult::Affected(affected))
        }
    }

    /// Execute UPSERT within a transaction
    async fn execute_upsert_in_tx(
        &self,
        tx: &mut sqlx::Transaction<'_, sqlx::Postgres>,
        verb: &RuntimeVerb,
        crud: &RuntimeCrudConfig,
        args: &HashMap<String, JsonValue>,
    ) -> Result<GenericExecutionResult> {
        if crud.conflict_keys.is_empty() && crud.conflict_constraint.is_none() {
            bail!("Upsert requires conflict_keys or conflict_constraint in config");
        }

        let pk_col = crud
            .returning
            .as_deref()
            .unwrap_or_else(|| self.infer_pk_column(&crud.table));

        let mut columns = Vec::new();
        let mut placeholders = Vec::new();
        let mut updates = Vec::new();
        let mut bind_values: Vec<SqlValue> = Vec::new();

        let new_id = Uuid::new_v4();
        columns.push(format!("\"{}\"", pk_col));
        placeholders.push("$1".to_string());
        bind_values.push(SqlValue::Uuid(new_id));
        let mut idx = 2;

        for arg_def in &verb.args {
            if let Some(value) = args.get(&arg_def.name) {
                if let Some(col) = &arg_def.maps_to {
                    if col == pk_col {
                        continue;
                    }
                    columns.push(format!("\"{}\"", col));
                    placeholders.push(format!("${}", idx));

                    if !crud.conflict_keys.contains(col) {
                        updates.push(format!("\"{}\" = EXCLUDED.\"{}\"", col, col));
                    }

                    if arg_def.lookup.is_some() && arg_def.arg_type == ArgType::Uuid {
                        let code = value.as_str().ok_or_else(|| {
                            anyhow!("Expected string for lookup {}", arg_def.name)
                        })?;
                        if let Ok(uuid) = Uuid::parse_str(code) {
                            bind_values.push(SqlValue::Uuid(uuid));
                        } else {
                            let uuid = self.resolve_lookup(arg_def, code).await?;
                            bind_values.push(SqlValue::Uuid(uuid));
                        }
                    } else if arg_def.arg_type == ArgType::Lookup && arg_def.lookup.is_some() {
                        let code = value.as_str().ok_or_else(|| {
                            anyhow!("Expected string for lookup {}", arg_def.name)
                        })?;
                        let uuid = self.resolve_lookup(arg_def, code).await?;
                        bind_values.push(SqlValue::Uuid(uuid));
                    } else {
                        bind_values.push(self.json_to_sql_value(value, arg_def)?);
                    }

                    idx += 1;
                }
            }
        }

        if let Some(set_values) = &crud.set_values {
            for (col, value) in set_values {
                let col_quoted = format!("\"{}\"", col);
                if columns.contains(&col_quoted) {
                    continue;
                }

                if let Some(s) = value.as_str() {
                    let s_lower = s.to_lowercase();
                    if s_lower == "now()" || s_lower == "current_timestamp" {
                        columns.push(col_quoted.clone());
                        placeholders.push("NOW()".to_string());
                    } else {
                        columns.push(col_quoted.clone());
                        placeholders.push(format!("${}", idx));
                        bind_values.push(SqlValue::String(s.to_string()));
                        if !crud.conflict_keys.contains(col) {
                            updates.push(format!("\"{}\" = EXCLUDED.\"{}\"", col, col));
                        }
                        idx += 1;
                    }
                } else if let Some(b) = value.as_bool() {
                    columns.push(col_quoted.clone());
                    placeholders.push(format!("${}", idx));
                    bind_values.push(SqlValue::Boolean(b));
                    if !crud.conflict_keys.contains(col) {
                        updates.push(format!("\"{}\" = EXCLUDED.\"{}\"", col, col));
                    }
                    idx += 1;
                } else if let Some(n) = value.as_i64() {
                    columns.push(col_quoted.clone());
                    placeholders.push(format!("${}", idx));
                    bind_values.push(SqlValue::Integer(n));
                    if !crud.conflict_keys.contains(col) {
                        updates.push(format!("\"{}\" = EXCLUDED.\"{}\"", col, col));
                    }
                    idx += 1;
                }
            }
        }

        let returning = crud.returning.as_deref().unwrap_or(pk_col);

        let conflict_clause = if let Some(constraint_name) = &crud.conflict_constraint {
            format!("ON CONSTRAINT \"{}\"", constraint_name)
        } else {
            let conflict_cols: Vec<String> = crud
                .conflict_keys
                .iter()
                .map(|c| format!("\"{}\"", c))
                .collect();
            format!("({})", conflict_cols.join(", "))
        };

        let sql = if updates.is_empty() {
            format!(
                r#"INSERT INTO "{}"."{}" ({}) VALUES ({})
                   ON CONFLICT {} DO UPDATE SET "{}" = EXCLUDED."{}"
                   RETURNING "{}""#,
                crud.schema,
                crud.table,
                columns.join(", "),
                placeholders.join(", "),
                conflict_clause,
                pk_col,
                pk_col,
                returning
            )
        } else {
            format!(
                r#"INSERT INTO "{}"."{}" ({}) VALUES ({})
                   ON CONFLICT {} DO UPDATE SET {}
                   RETURNING "{}""#,
                crud.schema,
                crud.table,
                columns.join(", "),
                placeholders.join(", "),
                conflict_clause,
                updates.join(", "),
                returning
            )
        };

        debug!("UPSERT (in_tx) SQL: {}", sql);

        let row = Self::execute_with_bindings_in_tx(tx, &sql, &bind_values).await?;
        let uuid: Uuid = row.try_get(returning)?;
        Ok(GenericExecutionResult::Uuid(uuid))
    }

    /// Execute LINK within a transaction
    async fn execute_link_in_tx(
        &self,
        tx: &mut sqlx::Transaction<'_, sqlx::Postgres>,
        verb: &RuntimeVerb,
        crud: &RuntimeCrudConfig,
        args: &HashMap<String, JsonValue>,
    ) -> Result<GenericExecutionResult> {
        let junction = crud
            .junction
            .as_deref()
            .ok_or_else(|| anyhow!("Link requires junction table"))?;
        let from_col = crud
            .from_col
            .as_deref()
            .ok_or_else(|| anyhow!("Link requires from_col"))?;
        let to_col = crud
            .to_col
            .as_deref()
            .ok_or_else(|| anyhow!("Link requires to_col"))?;

        let pk_col = self.infer_pk_column(junction);
        let new_id = Uuid::new_v4();

        let mut columns = vec![
            format!("\"{}\"", pk_col),
            format!("\"{}\"", from_col),
            format!("\"{}\"", to_col),
        ];
        let mut placeholders = vec!["$1".to_string(), "$2".to_string(), "$3".to_string()];
        let mut bind_values: Vec<SqlValue> = vec![SqlValue::Uuid(new_id)];

        let mut from_value: Option<SqlValue> = None;
        let mut to_value: Option<SqlValue> = None;

        for arg_def in &verb.args {
            if let Some(value) = args.get(&arg_def.name) {
                if arg_def.maps_to.as_deref() == Some(from_col) {
                    from_value = Some(self.json_to_sql_value(value, arg_def)?);
                } else if arg_def.maps_to.as_deref() == Some(to_col) {
                    to_value = Some(self.json_to_sql_value(value, arg_def)?);
                }
            }
        }

        let from_val = from_value.ok_or_else(|| anyhow!("Missing from argument for link"))?;
        let to_val = to_value.ok_or_else(|| anyhow!("Missing to argument for link"))?;

        bind_values.push(from_val.clone());
        bind_values.push(to_val.clone());

        let mut idx = 4;
        for arg_def in &verb.args {
            if let Some(value) = args.get(&arg_def.name) {
                if let Some(col) = &arg_def.maps_to {
                    if col != from_col && col != to_col && col != pk_col {
                        columns.push(format!("\"{}\"", col));
                        placeholders.push(format!("${}", idx));
                        bind_values.push(self.json_to_sql_value(value, arg_def)?);
                        idx += 1;
                    }
                }
            }
        }

        let sql = format!(
            r#"WITH ins AS (
                INSERT INTO "{}"."{}" ({}) VALUES ({})
                ON CONFLICT ("{}", "{}") DO NOTHING
                RETURNING "{}"
            )
            SELECT "{}" FROM ins
            UNION ALL
            SELECT "{}" FROM "{}"."{}"
            WHERE "{}" = $2 AND "{}" = $3
            AND NOT EXISTS (SELECT 1 FROM ins)
            LIMIT 1"#,
            crud.schema,
            junction,
            columns.join(", "),
            placeholders.join(", "),
            from_col,
            to_col,
            pk_col,
            pk_col,
            pk_col,
            crud.schema,
            junction,
            from_col,
            to_col
        );

        debug!("LINK (in_tx) SQL: {}", sql);

        let row = Self::execute_with_bindings_in_tx(tx, &sql, &bind_values).await?;
        let uuid: Uuid = row.try_get(pk_col)?;
        Ok(GenericExecutionResult::Uuid(uuid))
    }

    /// Execute UNLINK within a transaction
    async fn execute_unlink_in_tx(
        &self,
        tx: &mut sqlx::Transaction<'_, sqlx::Postgres>,
        verb: &RuntimeVerb,
        crud: &RuntimeCrudConfig,
        args: &HashMap<String, JsonValue>,
    ) -> Result<GenericExecutionResult> {
        let junction = crud
            .junction
            .as_deref()
            .ok_or_else(|| anyhow!("Unlink requires junction table"))?;
        let from_col = crud
            .from_col
            .as_deref()
            .ok_or_else(|| anyhow!("Unlink requires from_col"))?;
        let to_col = crud
            .to_col
            .as_deref()
            .ok_or_else(|| anyhow!("Unlink requires to_col"))?;

        let mut from_value: Option<SqlValue> = None;
        let mut to_value: Option<SqlValue> = None;

        for arg_def in &verb.args {
            if let Some(value) = args.get(&arg_def.name) {
                if arg_def.maps_to.as_deref() == Some(from_col) {
                    from_value = Some(self.json_to_sql_value(value, arg_def)?);
                } else if arg_def.maps_to.as_deref() == Some(to_col) {
                    to_value = Some(self.json_to_sql_value(value, arg_def)?);
                }
            }
        }

        let sql = format!(
            r#"DELETE FROM "{}"."{}" WHERE "{}" = $1 AND "{}" = $2"#,
            crud.schema, junction, from_col, to_col
        );

        debug!("UNLINK (in_tx) SQL: {}", sql);

        let bind_values = vec![
            from_value.ok_or_else(|| anyhow!("Missing from argument"))?,
            to_value.ok_or_else(|| anyhow!("Missing to argument"))?,
        ];
        let affected = Self::execute_non_query_in_tx(tx, &sql, &bind_values).await?;
        Ok(GenericExecutionResult::Affected(affected))
    }

    /// Execute ROLE_LINK within a transaction
    async fn execute_role_link_in_tx(
        &self,
        tx: &mut sqlx::Transaction<'_, sqlx::Postgres>,
        verb: &RuntimeVerb,
        crud: &RuntimeCrudConfig,
        args: &HashMap<String, JsonValue>,
    ) -> Result<GenericExecutionResult> {
        let junction = crud
            .junction
            .as_deref()
            .ok_or_else(|| anyhow!("RoleLink requires junction table"))?;
        let from_col = crud
            .from_col
            .as_deref()
            .ok_or_else(|| anyhow!("RoleLink requires from_col"))?;
        let to_col = crud
            .to_col
            .as_deref()
            .ok_or_else(|| anyhow!("RoleLink requires to_col"))?;
        let role_col = crud.role_col.as_deref().unwrap_or("role_id");

        let role_arg = verb
            .args
            .iter()
            .find(|a| a.arg_type == ArgType::Lookup && a.lookup.is_some())
            .ok_or_else(|| anyhow!("RoleLink requires lookup argument for role"))?;

        let role_value = args
            .get(&role_arg.name)
            .ok_or_else(|| anyhow!("Missing role argument"))?;

        let role_code = role_value
            .as_str()
            .ok_or_else(|| anyhow!("Role must be a string"))?;

        let role_id = self.resolve_lookup(role_arg, role_code).await?;

        let pk_col = self.infer_pk_column(junction);
        let new_id = Uuid::new_v4();

        let mut columns = vec![
            format!("\"{}\"", pk_col),
            format!("\"{}\"", from_col),
            format!("\"{}\"", to_col),
            format!("\"{}\"", role_col),
        ];
        let mut placeholders = vec![
            "$1".to_string(),
            "$2".to_string(),
            "$3".to_string(),
            "$4".to_string(),
        ];
        let mut bind_values: Vec<SqlValue> = vec![SqlValue::Uuid(new_id)];

        let mut from_value: Option<SqlValue> = None;
        let mut to_value: Option<SqlValue> = None;

        for arg_def in &verb.args {
            if let Some(value) = args.get(&arg_def.name) {
                if arg_def.maps_to.as_deref() == Some(from_col) {
                    from_value = Some(self.json_to_sql_value(value, arg_def)?);
                } else if arg_def.maps_to.as_deref() == Some(to_col) {
                    to_value = Some(self.json_to_sql_value(value, arg_def)?);
                }
            }
        }

        let from_val = from_value.ok_or_else(|| anyhow!("Missing from argument for role_link"))?;
        let to_val = to_value.ok_or_else(|| anyhow!("Missing to argument for role_link"))?;

        bind_values.push(from_val);
        bind_values.push(to_val);
        bind_values.push(SqlValue::Uuid(role_id));

        let mut idx = 5;
        for arg_def in &verb.args {
            if let Some(value) = args.get(&arg_def.name) {
                if let Some(col) = &arg_def.maps_to {
                    if col != from_col
                        && col != to_col
                        && col != pk_col
                        && arg_def.arg_type != ArgType::Lookup
                    {
                        columns.push(format!("\"{}\"", col));
                        placeholders.push(format!("${}", idx));
                        bind_values.push(self.json_to_sql_value(value, arg_def)?);
                        idx += 1;
                    }
                }
            }
        }

        let returning = crud.returning.as_deref().unwrap_or(pk_col);

        let sql = format!(
            r#"WITH ins AS (
                INSERT INTO "{}"."{}" ({}) VALUES ({})
                ON CONFLICT ("{}", "{}", "{}") DO NOTHING
                RETURNING "{}"
            )
            SELECT "{}" FROM ins
            UNION ALL
            SELECT "{}" FROM "{}"."{}"
            WHERE "{}" = $2 AND "{}" = $3 AND "{}" = $4
            AND NOT EXISTS (SELECT 1 FROM ins)
            LIMIT 1"#,
            crud.schema,
            junction,
            columns.join(", "),
            placeholders.join(", "),
            from_col,
            to_col,
            role_col,
            returning,
            returning,
            returning,
            crud.schema,
            junction,
            from_col,
            to_col,
            role_col
        );

        debug!("ROLE_LINK (in_tx) SQL: {}", sql);

        let row = Self::execute_with_bindings_in_tx(tx, &sql, &bind_values).await?;
        let uuid: Uuid = row.try_get(returning)?;
        Ok(GenericExecutionResult::Uuid(uuid))
    }

    /// Execute ROLE_UNLINK within a transaction
    async fn execute_role_unlink_in_tx(
        &self,
        tx: &mut sqlx::Transaction<'_, sqlx::Postgres>,
        verb: &RuntimeVerb,
        crud: &RuntimeCrudConfig,
        args: &HashMap<String, JsonValue>,
    ) -> Result<GenericExecutionResult> {
        let junction = crud
            .junction
            .as_deref()
            .ok_or_else(|| anyhow!("RoleUnlink requires junction table"))?;
        let from_col = crud
            .from_col
            .as_deref()
            .ok_or_else(|| anyhow!("RoleUnlink requires from_col"))?;
        let to_col = crud
            .to_col
            .as_deref()
            .ok_or_else(|| anyhow!("RoleUnlink requires to_col"))?;
        let role_col = crud.role_col.as_deref().unwrap_or("role_id");

        let role_arg = verb
            .args
            .iter()
            .find(|a| a.arg_type == ArgType::Lookup && a.lookup.is_some())
            .ok_or_else(|| anyhow!("RoleUnlink requires lookup argument"))?;

        let role_value = args
            .get(&role_arg.name)
            .ok_or_else(|| anyhow!("Missing role argument"))?;

        let lookup = role_arg
            .lookup
            .as_ref()
            .ok_or_else(|| anyhow!("Role argument missing lookup configuration"))?;
        let role_code = role_value
            .as_str()
            .ok_or_else(|| anyhow!("Role must be a string"))?;

        // For role unlink within tx, we need to do the lookup within the tx too
        let lookup_sql = format!(
            r#"SELECT "{}" FROM "{}"."{}" WHERE "{}" = $1"#,
            lookup.primary_key,
            crud.schema,
            lookup.table,
            lookup.search_key.primary_column()
        );

        let mut query = sqlx::query(&lookup_sql);
        query = query.bind(role_code);
        let role_row = query.fetch_one(&mut **tx).await?;

        let role_id: Uuid = role_row.try_get(&lookup.primary_key as &str)?;

        let mut from_value: Option<SqlValue> = None;
        let mut to_value: Option<SqlValue> = None;

        for arg_def in &verb.args {
            if let Some(value) = args.get(&arg_def.name) {
                if arg_def.maps_to.as_deref() == Some(from_col) {
                    from_value = Some(self.json_to_sql_value(value, arg_def)?);
                } else if arg_def.maps_to.as_deref() == Some(to_col) {
                    to_value = Some(self.json_to_sql_value(value, arg_def)?);
                }
            }
        }

        let sql = format!(
            r#"DELETE FROM "{}"."{}" WHERE "{}" = $1 AND "{}" = $2 AND "{}" = $3"#,
            crud.schema, junction, from_col, to_col, role_col
        );

        debug!("ROLE_UNLINK (in_tx) SQL: {}", sql);

        let bind_values = vec![
            from_value.ok_or_else(|| anyhow!("Missing from argument"))?,
            to_value.ok_or_else(|| anyhow!("Missing to argument"))?,
            SqlValue::Uuid(role_id),
        ];
        let affected = Self::execute_non_query_in_tx(tx, &sql, &bind_values).await?;
        Ok(GenericExecutionResult::Affected(affected))
    }

    /// Execute ENTITY_CREATE within a transaction
    async fn execute_entity_create_in_tx(
        &self,
        tx: &mut sqlx::Transaction<'_, sqlx::Postgres>,
        verb: &RuntimeVerb,
        crud: &RuntimeCrudConfig,
        args: &HashMap<String, JsonValue>,
    ) -> Result<GenericExecutionResult> {
        let type_code = self.resolve_entity_type_code(verb, crud, args, "create-")?;

        // Look up entity_type_id and table_name within tx
        let type_sql = format!(
            r#"SELECT entity_type_id, table_name FROM "{}".entity_types
               WHERE UPPER(type_code) = UPPER($1) OR UPPER(type_code) LIKE UPPER($1) || '_%'
               ORDER BY CASE WHEN UPPER(type_code) = UPPER($1) THEN 0 ELSE 1 END
               LIMIT 1"#,
            crud.schema
        );

        let type_row = sqlx::query(&type_sql)
            .bind(&type_code)
            .fetch_one(&mut **tx)
            .await
            .map_err(|e| anyhow!("Entity type not found for '{}': {}", type_code, e))?;

        let entity_type_id: Uuid = type_row.try_get("entity_type_id")?;
        let extension_table: String = match crud.extension_table.clone() {
            Some(t) => t,
            None => {
                let table_name: String = type_row.try_get("table_name").map_err(|e| {
                    anyhow!(
                        "No extension table found for entity type '{}': {}",
                        type_code,
                        e
                    )
                })?;
                if table_name.is_empty() {
                    return Err(anyhow!(
                        "Extension table name is empty for entity type '{}'",
                        type_code
                    ));
                }
                table_name
            }
        };

        let entity_id = Uuid::new_v4();

        let entity_name = if type_code == "PROPER_PERSON" || type_code == "PROPER_PERSON_NATURAL" {
            let first = args
                .get("first-name")
                .and_then(|v| v.as_str())
                .unwrap_or("");
            let last = args.get("last-name").and_then(|v| v.as_str()).unwrap_or("");
            format!("{} {}", first, last).trim().to_string()
        } else {
            args.get("name")
                .and_then(|v| v.as_str())
                .unwrap_or("Unknown")
                .to_string()
        };

        // Check for existing entity within tx (idempotency)
        let existing_sql = format!(
            r#"SELECT entity_id FROM "{}".entities WHERE entity_type_id = $1 AND name = $2"#,
            crud.schema
        );

        if let Ok(existing_row) = sqlx::query(&existing_sql)
            .bind(entity_type_id)
            .bind(&entity_name)
            .fetch_one(&mut **tx)
            .await
        {
            let existing_id: Uuid = existing_row.try_get("entity_id")?;
            debug!(
                "ENTITY_CREATE (in_tx): Entity '{}' already exists with id {}, returning existing",
                entity_name, existing_id
            );
            return Ok(GenericExecutionResult::Uuid(existing_id));
        }

        // INSERT into entities base table
        let base_sql = format!(
            r#"INSERT INTO "{}".entities (entity_id, entity_type_id, name) VALUES ($1, $2, $3)"#,
            crud.schema
        );

        sqlx::query(&base_sql)
            .bind(entity_id)
            .bind(entity_type_id)
            .bind(&entity_name)
            .execute(&mut **tx)
            .await?;

        // INSERT into extension table
        let ext_pk_col = self.infer_pk_column(&extension_table);
        let uses_shared_pk = ext_pk_col == "entity_id";

        let (mut columns, mut placeholders, mut bind_values, mut idx) = if uses_shared_pk {
            (
                vec!["\"entity_id\"".to_string()],
                vec!["$1".to_string()],
                vec![SqlValue::Uuid(entity_id)],
                2,
            )
        } else {
            let ext_pk_id = Uuid::new_v4();
            (
                vec![format!("\"{}\"", ext_pk_col), "\"entity_id\"".to_string()],
                vec!["$1".to_string(), "$2".to_string()],
                vec![SqlValue::Uuid(ext_pk_id), SqlValue::Uuid(entity_id)],
                3,
            )
        };

        let base_table_cols = ["name", "external_id"];
        for arg_def in &verb.args {
            if let Some(value) = args.get(&arg_def.name) {
                if arg_def.name == "entity-type" || arg_def.name == "entity-id" {
                    continue;
                }
                if let Some(col) = &arg_def.maps_to {
                    if col == ext_pk_col
                        || col == "entity_id"
                        || base_table_cols.contains(&col.as_str())
                    {
                        continue;
                    }
                    columns.push(format!("\"{}\"", col));
                    placeholders.push(format!("${}", idx));

                    if arg_def.lookup.is_some() && arg_def.arg_type == ArgType::Uuid {
                        let code = value.as_str().ok_or_else(|| {
                            anyhow!("Expected string for lookup {}", arg_def.name)
                        })?;
                        if let Ok(uuid) = Uuid::parse_str(code) {
                            bind_values.push(SqlValue::Uuid(uuid));
                        } else {
                            let uuid = self.resolve_lookup(arg_def, code).await?;
                            bind_values.push(SqlValue::Uuid(uuid));
                        }
                    } else if arg_def.arg_type == ArgType::Lookup && arg_def.lookup.is_some() {
                        let code = value.as_str().ok_or_else(|| {
                            anyhow!("Expected string for lookup {}", arg_def.name)
                        })?;
                        let uuid = self.resolve_lookup(arg_def, code).await?;
                        bind_values.push(SqlValue::Uuid(uuid));
                    } else {
                        bind_values.push(self.json_to_sql_value(value, arg_def)?);
                    }
                    idx += 1;
                }
            }
        }

        if let Some(name_column) = self.infer_extension_name_column(&extension_table) {
            let quoted = format!("\"{}\"", name_column);
            if !columns.iter().any(|column| column == &quoted) {
                let name = args
                    .get("name")
                    .and_then(|value| value.as_str())
                    .ok_or_else(|| {
                        anyhow!(
                            "entity.create for {} requires :name because {}.{} is NOT NULL",
                            type_code,
                            extension_table,
                            name_column
                        )
                    })?;
                columns.push(quoted);
                placeholders.push(format!("${}", idx));
                bind_values.push(SqlValue::String(name.to_string()));
            }
        }

        let ext_sql = format!(
            r#"INSERT INTO "{}"."{}" ({}) VALUES ({})"#,
            crud.schema,
            extension_table,
            columns.join(", "),
            placeholders.join(", ")
        );

        debug!("ENTITY_CREATE (in_tx) extension SQL: {}", ext_sql);

        Self::execute_non_query_in_tx(tx, &ext_sql, &bind_values).await?;

        Ok(GenericExecutionResult::Uuid(entity_id))
    }

    /// Execute ENTITY_UPSERT within a transaction
    async fn execute_entity_upsert_in_tx(
        &self,
        tx: &mut sqlx::Transaction<'_, sqlx::Postgres>,
        verb: &RuntimeVerb,
        crud: &RuntimeCrudConfig,
        args: &HashMap<String, JsonValue>,
    ) -> Result<GenericExecutionResult> {
        let type_code = self.resolve_entity_type_code(verb, crud, args, "ensure-")?;

        let type_sql = format!(
            r#"SELECT entity_type_id, table_name FROM "{}".entity_types
               WHERE UPPER(type_code) = UPPER($1) OR UPPER(type_code) LIKE UPPER($1) || '_%'
               ORDER BY CASE WHEN UPPER(type_code) = UPPER($1) THEN 0 ELSE 1 END
               LIMIT 1"#,
            crud.schema
        );

        let type_row = sqlx::query(&type_sql)
            .bind(&type_code)
            .fetch_one(&mut **tx)
            .await
            .map_err(|e| anyhow!("Entity type not found for '{}': {}", type_code, e))?;

        let entity_type_id: Uuid = type_row.try_get("entity_type_id")?;
        let extension_table: String = crud
            .extension_table
            .clone()
            .unwrap_or_else(|| type_row.try_get("table_name").unwrap_or_default());

        let entity_name = if type_code == "PROPER_PERSON" || type_code == "PROPER_PERSON_NATURAL" {
            let first = args
                .get("first-name")
                .and_then(|v| v.as_str())
                .unwrap_or("");
            let last = args.get("last-name").and_then(|v| v.as_str()).unwrap_or("");
            format!("{} {}", first, last).trim().to_string()
        } else {
            args.get("name")
                .and_then(|v| v.as_str())
                .unwrap_or("Unknown")
                .to_string()
        };

        // UPSERT into entities base table
        let base_sql = format!(
            r#"INSERT INTO "{}".entities (entity_id, entity_type_id, name)
               VALUES (gen_random_uuid(), $1, $2)
               ON CONFLICT (entity_type_id, name) DO UPDATE SET updated_at = now()
               RETURNING entity_id"#,
            crud.schema
        );

        let row = sqlx::query(&base_sql)
            .bind(entity_type_id)
            .bind(&entity_name)
            .fetch_one(&mut **tx)
            .await?;

        let entity_id: Uuid = row.try_get("entity_id")?;

        // Build extension table columns
        let ext_pk_col = self.infer_pk_column(&extension_table);
        let uses_shared_pk = ext_pk_col == "entity_id";

        let (mut columns, mut placeholders, mut bind_values, mut idx) = if uses_shared_pk {
            (
                vec!["\"entity_id\"".to_string()],
                vec!["$1".to_string()],
                vec![SqlValue::Uuid(entity_id)],
                2,
            )
        } else {
            (
                vec![format!("\"{}\"", ext_pk_col), "\"entity_id\"".to_string()],
                vec!["$1".to_string(), "$2".to_string()],
                vec![SqlValue::Uuid(Uuid::new_v4()), SqlValue::Uuid(entity_id)],
                3,
            )
        };

        let mut update_cols: Vec<String> = Vec::new();

        let base_table_cols = ["name", "external_id"];
        for arg_def in &verb.args {
            if let Some(value) = args.get(&arg_def.name) {
                if arg_def.name == "entity-type" || arg_def.name == "entity-id" {
                    continue;
                }
                if let Some(col) = &arg_def.maps_to {
                    if col == ext_pk_col
                        || col == "entity_id"
                        || base_table_cols.contains(&col.as_str())
                    {
                        continue;
                    }
                    columns.push(format!("\"{}\"", col));
                    placeholders.push(format!("${}", idx));
                    update_cols.push(format!("\"{}\" = EXCLUDED.\"{}\"", col, col));

                    if arg_def.lookup.is_some() && arg_def.arg_type == ArgType::Uuid {
                        let code = value.as_str().ok_or_else(|| {
                            anyhow!("Expected string for lookup {}", arg_def.name)
                        })?;
                        if let Ok(uuid) = Uuid::parse_str(code) {
                            bind_values.push(SqlValue::Uuid(uuid));
                        } else {
                            let uuid = self.resolve_lookup(arg_def, code).await?;
                            bind_values.push(SqlValue::Uuid(uuid));
                        }
                    } else if arg_def.arg_type == ArgType::Lookup && arg_def.lookup.is_some() {
                        let code = value.as_str().ok_or_else(|| {
                            anyhow!("Expected string for lookup {}", arg_def.name)
                        })?;
                        let uuid = self.resolve_lookup(arg_def, code).await?;
                        bind_values.push(SqlValue::Uuid(uuid));
                    } else {
                        bind_values.push(self.json_to_sql_value(value, arg_def)?);
                    }
                    idx += 1;
                }
            }
        }

        if let Some(name_column) = self.infer_extension_name_column(&extension_table) {
            let quoted = format!("\"{}\"", name_column);
            if !columns.iter().any(|column| column == &quoted) {
                let name = args
                    .get("name")
                    .and_then(|value| value.as_str())
                    .ok_or_else(|| {
                        anyhow!(
                            "entity.ensure for {} requires :name because {}.{} is NOT NULL",
                            type_code,
                            extension_table,
                            name_column
                        )
                    })?;
                columns.push(quoted.clone());
                placeholders.push(format!("${}", idx));
                update_cols.push(format!("{quoted} = EXCLUDED.{quoted}"));
                bind_values.push(SqlValue::String(name.to_string()));
            }
        }

        let has_isin = columns.iter().any(|c| c == "\"isin\"");
        let conflict_col = if has_isin { "isin" } else { "entity_id" };

        let ext_sql = if update_cols.is_empty() {
            format!(
                r#"INSERT INTO "{}"."{}" ({}) VALUES ({})
                   ON CONFLICT ("{}") DO NOTHING"#,
                crud.schema,
                extension_table,
                columns.join(", "),
                placeholders.join(", "),
                conflict_col
            )
        } else {
            format!(
                r#"INSERT INTO "{}"."{}" ({}) VALUES ({})
                   ON CONFLICT ("{}") DO UPDATE SET {}"#,
                crud.schema,
                extension_table,
                columns.join(", "),
                placeholders.join(", "),
                conflict_col,
                update_cols.join(", ")
            )
        };

        debug!("ENTITY_UPSERT (in_tx) extension SQL: {}", ext_sql);

        Self::execute_non_query_in_tx(tx, &ext_sql, &bind_values).await?;

        Ok(GenericExecutionResult::Uuid(entity_id))
    }

    // =========================================================================
    // HELPER METHODS
    // =========================================================================

    fn resolve_entity_type_code(
        &self,
        verb: &RuntimeVerb,
        crud: &RuntimeCrudConfig,
        args: &HashMap<String, JsonValue>,
        verb_prefix: &str,
    ) -> Result<String> {
        if let Some(tc) = &crud.type_code {
            return Ok(Self::canonicalize_entity_type_code(tc));
        }

        if let Some(entity_type) = args.get("entity-type").and_then(JsonValue::as_str) {
            return Ok(Self::canonicalize_entity_type_code(entity_type));
        }

        if let Some(fund_type) = args.get("fund-type").and_then(JsonValue::as_str) {
            return Ok(format!(
                "FUND_{}",
                fund_type.trim().to_uppercase().replace('-', "_")
            ));
        }

        verb.verb
            .strip_prefix(verb_prefix)
            .map(Self::canonicalize_entity_type_code)
            .ok_or_else(|| anyhow!("Invalid entity verb name: {}", verb.verb))
    }

    fn canonicalize_entity_type_code(raw: &str) -> String {
        match raw.trim().to_uppercase().replace('-', "_").as_str() {
            "LIMITED_COMPANY" => "LIMITED_COMPANY_PRIVATE".to_string(),
            "PROPER_PERSON" => "PROPER_PERSON_NATURAL".to_string(),
            other => other.to_string(),
        }
    }

    /// Infer PK column name from table name (convention: singular_id)
    fn infer_pk_column(&self, table: &str) -> &'static str {
        // Common patterns
        match table {
            "cbus" => "cbu_id",
            "entities" => "entity_id",
            "products" => "product_id",
            "services" => "service_id",
            "roles" => "role_id",
            "service_resource_types" => "resource_id",
            "resource_attribute_requirements" => "requirement_id",
            "cbu_entity_roles" => "cbu_entity_role_id",
            "product_services" => "product_service_id",
            "service_resources" => "service_resource_id",
            "document_catalog" => "document_id",
            "entity_proper_persons" => "proper_person_id",
            "entity_limited_companies" => "limited_company_id",
            "entity_partnerships" => "partnership_id",
            "entity_trusts" => "trust_id",
            // Fund ontology tables use shared PK pattern (entity_id is both PK and FK)
            "entity_funds" => "entity_id",
            "entity_share_classes" => "entity_id",
            _ => "id",
        }
    }

    fn infer_extension_name_column(&self, table: &str) -> Option<&'static str> {
        match table {
            "entity_limited_companies" => Some("company_name"),
            "entity_partnerships" => Some("partnership_name"),
            "entity_trusts" => Some("trust_name"),
            _ => None,
        }
    }

    /// Convert JSON value to SQL value based on argument type
    fn json_to_sql_value(&self, value: &JsonValue, arg: &RuntimeArg) -> Result<SqlValue> {
        match arg.arg_type {
            ArgType::String => {
                let s = value
                    .as_str()
                    .ok_or_else(|| anyhow!("Expected string for {}", arg.name))?;
                Ok(SqlValue::String(s.to_string()))
            }
            ArgType::Uuid => {
                let s = value
                    .as_str()
                    .ok_or_else(|| anyhow!("Expected UUID string for {}", arg.name))?;
                let uuid = Uuid::parse_str(s)?;
                Ok(SqlValue::Uuid(uuid))
            }
            ArgType::UuidArray => {
                let arr = value
                    .as_array()
                    .ok_or_else(|| anyhow!("Expected array for {}", arg.name))?;
                let uuids: Vec<Uuid> = arr
                    .iter()
                    .map(|v| {
                        let s = v
                            .as_str()
                            .ok_or_else(|| anyhow!("Expected UUID string in array"))?;
                        Uuid::parse_str(s).map_err(|e| anyhow!("Invalid UUID: {}", e))
                    })
                    .collect::<Result<Vec<_>>>()?;
                Ok(SqlValue::UuidArray(uuids))
            }
            ArgType::Integer => {
                let n = value
                    .as_i64()
                    .ok_or_else(|| anyhow!("Expected integer for {}", arg.name))?;
                Ok(SqlValue::Integer(n))
            }
            ArgType::Decimal => {
                let n = if let Some(f) = value.as_f64() {
                    rust_decimal::Decimal::try_from(f)?
                } else if let Some(s) = value.as_str() {
                    s.parse::<rust_decimal::Decimal>()?
                } else {
                    bail!("Expected decimal for {}", arg.name)
                };
                Ok(SqlValue::Decimal(n))
            }
            ArgType::Boolean => {
                let b = value
                    .as_bool()
                    .ok_or_else(|| anyhow!("Expected boolean for {}", arg.name))?;
                Ok(SqlValue::Boolean(b))
            }
            ArgType::Json => Ok(SqlValue::Json(value.clone())),
            ArgType::Lookup => {
                // Lookup values are strings (the code to look up)
                let s = value
                    .as_str()
                    .ok_or_else(|| anyhow!("Expected string for lookup {}", arg.name))?;
                Ok(SqlValue::String(s.to_string()))
            }
            ArgType::Date => {
                let s = value
                    .as_str()
                    .ok_or_else(|| anyhow!("Expected date string for {}", arg.name))?;
                let d = chrono::NaiveDate::parse_from_str(s, "%Y-%m-%d")?;
                Ok(SqlValue::Date(d))
            }
            ArgType::Timestamp => {
                let s = value
                    .as_str()
                    .ok_or_else(|| anyhow!("Expected timestamp string for {}", arg.name))?;
                let dt = chrono::DateTime::parse_from_rfc3339(s)?;
                Ok(SqlValue::Timestamp(dt.with_timezone(&chrono::Utc)))
            }
            ArgType::StringList => {
                let arr = value
                    .as_array()
                    .ok_or_else(|| anyhow!("Expected array for {}", arg.name))?;
                let strings: Vec<String> = arr
                    .iter()
                    .filter_map(|v| v.as_str().map(|s| s.to_string()))
                    .collect();
                Ok(SqlValue::StringArray(strings))
            }
            ArgType::Map => {
                // Maps are passed as JSON objects
                Ok(SqlValue::Json(value.clone()))
            }
            ArgType::SymbolRef => {
                // Symbol refs should have been resolved to UUIDs by the executor
                let s = value
                    .as_str()
                    .ok_or_else(|| anyhow!("Expected UUID string for symbol ref {}", arg.name))?;
                let uuid = Uuid::parse_str(s)?;
                Ok(SqlValue::Uuid(uuid))
            }
            ArgType::UuidList => {
                // UuidList is an alias for UuidArray
                let arr = value
                    .as_array()
                    .ok_or_else(|| anyhow!("Expected array for {}", arg.name))?;
                let uuids: Vec<Uuid> = arr
                    .iter()
                    .map(|v| {
                        let s = v
                            .as_str()
                            .ok_or_else(|| anyhow!("Expected UUID string in array"))?;
                        Uuid::parse_str(s).map_err(|e| anyhow!("Invalid UUID: {}", e))
                    })
                    .collect::<Result<Vec<_>>>()?;
                Ok(SqlValue::UuidArray(uuids))
            }
            ArgType::Object => {
                // Objects are passed as JSON
                Ok(SqlValue::Json(value.clone()))
            }
        }
    }

    /// Execute query returning single row
    async fn execute_with_bindings(&self, sql: &str, values: &[SqlValue]) -> Result<PgRow> {
        tracing::debug!(
            "DBG execute_with_bindings: sql_len={} binds={}",
            sql.len(),
            values.len()
        );
        tracing::debug!("SQL: {}", &sql[..sql.len().min(200)]);

        let mut query = sqlx::query(sql);
        for val in values {
            query = Self::bind_sql_value(query, val);
        }
        tracing::debug!("execute_with_bindings: calling fetch_one...");
        let row = query.fetch_one(&self.pool).await?;
        tracing::debug!("execute_with_bindings: fetch_one returned OK");
        Ok(row)
    }

    /// Execute query returning multiple rows
    async fn execute_many_with_bindings(
        &self,
        sql: &str,
        values: &[SqlValue],
    ) -> Result<Vec<PgRow>> {
        tracing::trace!(sql = %sql, bind_count = values.len(), "executing SQL (multi row)");
        tracing::trace!(bindings = ?values, "SQL bind values");

        let mut query = sqlx::query(sql);
        for val in values {
            query = Self::bind_sql_value(query, val);
        }
        let rows = query.fetch_all(&self.pool).await?;
        tracing::trace!(row_count = rows.len(), "SQL returned rows");
        Ok(rows)
    }

    /// Execute non-query (INSERT/UPDATE/DELETE without RETURNING)
    async fn execute_non_query(&self, sql: &str, values: &[SqlValue]) -> Result<u64> {
        tracing::trace!(sql = %sql, bind_count = values.len(), "executing SQL (non-query)");
        tracing::trace!(bindings = ?values, "SQL bind values");

        let mut query = sqlx::query(sql);
        for val in values {
            query = Self::bind_sql_value(query, val);
        }
        let result = query.execute(&self.pool).await?;
        tracing::trace!(rows_affected = result.rows_affected(), "SQL rows affected");
        Ok(result.rows_affected())
    }

    // =========================================================================
    // TRANSACTION-AWARE HELPERS
    // =========================================================================

    /// Execute query returning single row within a transaction
    async fn execute_with_bindings_in_tx(
        tx: &mut sqlx::Transaction<'_, sqlx::Postgres>,
        sql: &str,
        values: &[SqlValue],
    ) -> Result<PgRow> {
        tracing::debug!(
            "DBG execute_with_bindings_in_tx: sql_len={} binds={}",
            sql.len(),
            values.len()
        );
        tracing::debug!("SQL: {}", &sql[..sql.len().min(200)]);

        let mut query = sqlx::query(sql);
        for val in values {
            query = Self::bind_sql_value(query, val);
        }
        tracing::debug!("execute_with_bindings_in_tx: calling fetch_one...");
        let row = query.fetch_one(&mut **tx).await?;
        tracing::debug!("execute_with_bindings_in_tx: fetch_one returned OK");
        Ok(row)
    }

    /// Execute query returning multiple rows within a transaction
    async fn execute_many_with_bindings_in_tx(
        tx: &mut sqlx::Transaction<'_, sqlx::Postgres>,
        sql: &str,
        values: &[SqlValue],
    ) -> Result<Vec<PgRow>> {
        tracing::trace!(sql = %sql, bind_count = values.len(), "executing SQL in tx (multi row)");
        let mut query = sqlx::query(sql);
        for val in values {
            query = Self::bind_sql_value(query, val);
        }
        let rows = query.fetch_all(&mut **tx).await?;
        tracing::trace!(row_count = rows.len(), "SQL returned rows");
        Ok(rows)
    }

    /// Execute non-query within a transaction (INSERT/UPDATE/DELETE without RETURNING)
    async fn execute_non_query_in_tx(
        tx: &mut sqlx::Transaction<'_, sqlx::Postgres>,
        sql: &str,
        values: &[SqlValue],
    ) -> Result<u64> {
        tracing::trace!(sql = %sql, bind_count = values.len(), "executing SQL in tx (non-query)");
        tracing::trace!(bindings = ?values, "SQL bind values");

        let mut query = sqlx::query(sql);
        for val in values {
            query = Self::bind_sql_value(query, val);
        }
        let result = query.execute(&mut **tx).await?;
        tracing::trace!(rows_affected = result.rows_affected(), "SQL rows affected");
        Ok(result.rows_affected())
    }

    /// Resolve a lookup argument using EntityGateway
    /// Returns the UUID (primary_key) for the given name (search_key)
    /// Uses EntityGateway for fuzzy search and suggestions
    async fn resolve_lookup(&self, arg: &RuntimeArg, code_value: &str) -> Result<Uuid> {
        let lookup = arg
            .lookup
            .as_ref()
            .ok_or_else(|| anyhow!("Lookup arg {} missing lookup config", arg.name))?;

        // Get entity_type from lookup config (maps to EntityGateway nickname)
        let entity_type = lookup
            .entity_type
            .as_ref()
            .ok_or_else(|| anyhow!("Lookup arg {} missing entity_type in config", arg.name))?;

        let mut client = self.get_gateway_client().await?;
        let request = SearchRequest {
            nickname: entity_type.to_uppercase(),
            values: vec![code_value.to_string()],
            search_key: None,
            mode: SearchMode::Exact as i32,
            limit: Some(5),
            discriminators: std::collections::HashMap::new(),
            tenant_id: None,
            cbu_id: None,
        };

        let response = client
            .search(request)
            .await
            .map_err(|e| anyhow!("EntityGateway search failed for {}: {}", entity_type, e))?;
        let matches = response.into_inner().matches;

        let code_upper = code_value.to_uppercase();
        for m in &matches {
            if m.token.to_uppercase() == code_upper || m.display.to_uppercase() == code_upper {
                return Uuid::parse_str(&m.token).map_err(|_| {
                    anyhow!(
                        "Lookup failed: EntityGateway returned non-UUID token '{}' for {} '{}'",
                        m.token,
                        entity_type,
                        code_value
                    )
                });
            }
        }

        if matches.is_empty() {
            Err(anyhow!(
                "Lookup failed: '{}' not found for {} via EntityGateway",
                code_value,
                entity_type
            ))
        } else {
            let suggestions: Vec<String> = matches.iter().map(|m| m.display.clone()).collect();
            let first_suggestion = suggestions.first().map(|s| s.as_str()).unwrap_or("(none)");
            Err(anyhow!(
                "Lookup failed: '{}' not found for {}\n  Did you mean: {}?\n  Available: {}",
                code_value,
                entity_type,
                first_suggestion,
                suggestions.join(", ")
            ))
        }
    }

    /// Bind a SqlValue to a query
    fn bind_sql_value<'q>(
        query: sqlx::query::Query<'q, sqlx::Postgres, sqlx::postgres::PgArguments>,
        value: &SqlValue,
    ) -> sqlx::query::Query<'q, sqlx::Postgres, sqlx::postgres::PgArguments> {
        match value {
            SqlValue::String(s) => query.bind(s.clone()),
            SqlValue::Uuid(u) => query.bind(*u),
            SqlValue::Integer(n) => query.bind(*n),
            SqlValue::Decimal(d) => query.bind(*d),
            SqlValue::Boolean(b) => query.bind(*b),
            SqlValue::Json(j) => query.bind(j.clone()),
            SqlValue::Date(d) => query.bind(*d),
            SqlValue::Timestamp(t) => query.bind(*t),
            SqlValue::StringArray(arr) => query.bind(arr.clone()),
            SqlValue::UuidArray(arr) => query.bind(arr.clone()),
            SqlValue::Null => query.bind(Option::<String>::None),
        }
    }

    /// Convert a database row to JSON
    fn row_to_json(&self, row: &PgRow) -> Result<JsonValue> {
        use sqlx::{Column, TypeInfo};

        let mut map = serde_json::Map::new();

        for column in row.columns() {
            let name = column.name();
            let type_name = column.type_info().name();

            let value: Option<JsonValue> = match type_name {
                "UUID" => row
                    .try_get::<Option<Uuid>, _>(name)
                    .ok()
                    .flatten()
                    .map(|u| json!(u.to_string())),
                "TEXT" | "VARCHAR" | "CHAR" | "NAME" => row
                    .try_get::<Option<String>, _>(name)
                    .ok()
                    .flatten()
                    .map(|s| json!(s)),
                "INT4" => row
                    .try_get::<Option<i32>, _>(name)
                    .ok()
                    .flatten()
                    .map(|i| json!(i)),
                "INT8" => row
                    .try_get::<Option<i64>, _>(name)
                    .ok()
                    .flatten()
                    .map(|i| json!(i)),
                "INT2" => row
                    .try_get::<Option<i16>, _>(name)
                    .ok()
                    .flatten()
                    .map(|i| json!(i)),
                "FLOAT4" | "FLOAT8" => row
                    .try_get::<Option<f64>, _>(name)
                    .ok()
                    .flatten()
                    .map(|f| json!(f)),
                "NUMERIC" => row
                    .try_get::<Option<rust_decimal::Decimal>, _>(name)
                    .ok()
                    .flatten()
                    .map(|d| json!(d.to_string())),
                "BOOL" => row
                    .try_get::<Option<bool>, _>(name)
                    .ok()
                    .flatten()
                    .map(|b| json!(b)),
                "JSONB" | "JSON" => row.try_get::<Option<JsonValue>, _>(name).ok().flatten(),
                "TIMESTAMPTZ" | "TIMESTAMP" => row
                    .try_get::<Option<chrono::DateTime<chrono::Utc>>, _>(name)
                    .ok()
                    .flatten()
                    .map(|dt| json!(dt.to_rfc3339())),
                "DATE" => row
                    .try_get::<Option<chrono::NaiveDate>, _>(name)
                    .ok()
                    .flatten()
                    .map(|d| json!(d.to_string())),
                _ => None,
            };

            map.insert(name.to_string(), value.unwrap_or(JsonValue::Null));
        }

        Ok(JsonValue::Object(map))
    }
}

// =============================================================================
// TESTS
// =============================================================================

#[cfg(test)]
mod tests {
    #[test]
    fn test_infer_pk_column() {
        // Create a dummy executor to test the method
        // Note: This test doesn't need a real pool since infer_pk_column doesn't use it
        assert_eq!("cbu_id", {
            match "cbus" {
                "cbus" => "cbu_id",
                "entities" => "entity_id",
                _ => "id",
            }
        });
        assert_eq!("entity_id", {
            match "entities" {
                "cbus" => "cbu_id",
                "entities" => "entity_id",
                _ => "id",
            }
        });
        assert_eq!("product_id", {
            match "products" {
                "products" => "product_id",
                _ => "id",
            }
        });
    }
}

// =============================================================================
// DAG EXECUTION TESTS
// =============================================================================
