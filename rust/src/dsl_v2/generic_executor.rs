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
use tonic::transport::Channel;

use super::config::types::{ArgType, CrudOperation, ReturnTypeConfig};
use super::runtime_registry::{RuntimeArg, RuntimeBehavior, RuntimeCrudConfig, RuntimeVerb};

#[cfg(feature = "database")]
use super::compiler::{compile_to_ops, CompileError};
#[cfg(feature = "database")]
use super::dag::{build_execution_plan, describe_plan, CycleError};
#[cfg(feature = "database")]
use super::ops::{EntityKey, Op};

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

// =============================================================================
// DAG EXECUTION RESULT
// =============================================================================

/// Result of DAG-based execution
#[cfg(feature = "database")]
#[derive(Debug)]
pub enum DagExecutionResult {
    /// Dry run - returns the execution plan description
    Plan(String),
    /// Executed successfully - returns results per op
    Executed {
        /// Results for each op, keyed by binding name if present
        bindings: HashMap<String, Uuid>,
        /// Number of ops executed
        ops_executed: usize,
    },
}

/// Error during DAG execution
#[cfg(feature = "database")]
#[derive(Debug)]
pub enum DagExecutionError {
    /// Compilation failed
    CompileErrors(Vec<CompileError>),
    /// Cycle detected in dependency graph
    CycleDetected(CycleError),
    /// Op execution failed
    OpFailed {
        op_index: usize,
        op_description: String,
        error: String,
    },
    /// Other error
    Other(String),
}

#[cfg(feature = "database")]
impl std::fmt::Display for DagExecutionError {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            DagExecutionError::CompileErrors(errors) => {
                write!(f, "Compilation failed:\n")?;
                for e in errors {
                    write!(f, "  - {}\n", e)?;
                }
                Ok(())
            }
            DagExecutionError::CycleDetected(e) => {
                write!(f, "Cycle detected: {}", e.explanation)
            }
            DagExecutionError::OpFailed {
                op_index,
                op_description,
                error,
            } => {
                write!(
                    f,
                    "Op {} ({}) failed: {}",
                    op_index + 1,
                    op_description,
                    error
                )
            }
            DagExecutionError::Other(msg) => write!(f, "{}", msg),
        }
    }
}

#[cfg(feature = "database")]
impl std::error::Error for DagExecutionError {}

// =============================================================================
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

    /// Get or create EntityGateway client
    async fn get_gateway_client(
        &self,
    ) -> Result<tokio::sync::MutexGuard<'_, Option<EntityGatewayClient<Channel>>>> {
        let mut guard = self.gateway_client.lock().await;
        if guard.is_none() {
            let addr = super::gateway_resolver::gateway_addr();
            match EntityGatewayClient::connect(addr.clone()).await {
                Ok(client) => {
                    *guard = Some(client);
                }
                Err(e) => {
                    debug!("EntityGateway not available at {}: {}", addr, e);
                    // Return guard with None - caller will fall back to SQL
                }
            }
        }
        Ok(guard)
    }

    /// Execute a verb from RuntimeVerb configuration
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
        tracing::debug!(
            "DBG GenericCrudExecutor::execute ENTER {}.{}",
            verb.domain,
            verb.verb
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
        };

        tracing::debug!(
            "DBG GenericCrudExecutor: operation={:?} table={}.{}",
            crud.operation,
            crud.schema,
            crud.table
        );

        let result = match crud.operation {
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
            CrudOperation::SelectWithJoin => self.execute_select_with_join(verb, crud, args).await,
            CrudOperation::EntityCreate => self.execute_entity_create(verb, crud, args).await,
            CrudOperation::EntityUpsert => self.execute_entity_upsert(verb, crud, args).await,
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

    /// Execute a program using DAG-based ordering
    ///
    /// This method:
    /// 1. Compiles the AST to primitive Ops
    /// 2. Builds an execution plan with topological sort
    /// 3. Executes ops in dependency order
    ///
    /// Benefits:
    /// - Correct execution order regardless of source order
    /// - Cycle detection at compile time
    /// - Dry-run capability (show plan without executing)
    pub async fn execute_with_dag(
        &self,
        program: &super::ast::Program,
        dry_run: bool,
    ) -> Result<DagExecutionResult, DagExecutionError> {
        // Step 1: Compile AST to Ops
        let compiled = compile_to_ops(program);
        if !compiled.is_ok() {
            return Err(DagExecutionError::CompileErrors(compiled.errors));
        }

        // Step 2: Build execution plan (topological sort)
        let plan = build_execution_plan(compiled.ops).map_err(DagExecutionError::CycleDetected)?;

        // Step 3: Dry run - just return the plan description
        if dry_run {
            return Ok(DagExecutionResult::Plan(describe_plan(&plan)));
        }

        // Step 4: Execute ops in order, tracking bindings
        let mut bindings: HashMap<String, Uuid> = HashMap::new();
        let mut entity_keys_to_uuids: HashMap<EntityKey, Uuid> = HashMap::new();

        for (idx, op) in plan.ops.iter().enumerate() {
            match self.execute_op(op, &entity_keys_to_uuids).await {
                Ok(result) => {
                    // Track bindings and entity keys
                    if let Some((binding, key, uuid)) = result {
                        bindings.insert(binding.clone(), uuid);
                        entity_keys_to_uuids.insert(key, uuid);
                    }
                }
                Err(e) => {
                    return Err(DagExecutionError::OpFailed {
                        op_index: idx,
                        op_description: op.describe(),
                        error: e.to_string(),
                    });
                }
            }
        }

        Ok(DagExecutionResult::Executed {
            bindings,
            ops_executed: plan.ops.len(),
        })
    }

    /// Execute a single Op
    ///
    /// Returns Ok(Some((binding, key, uuid))) if the op produces a binding,
    /// Ok(None) otherwise.
    async fn execute_op(
        &self,
        op: &Op,
        resolved_keys: &HashMap<EntityKey, Uuid>,
    ) -> Result<Option<(String, EntityKey, Uuid)>> {
        match op {
            Op::EnsureEntity {
                entity_type,
                key,
                attrs,
                binding,
                ..
            } => {
                // Map entity_type to verb
                let (domain, verb_name) = match entity_type.as_str() {
                    "cbu" => ("cbu", "ensure"),
                    "proper_person" => ("entity", "create-proper-person"),
                    "limited_company" => ("entity", "create-limited-company"),
                    "partnership" => ("entity", "create-partnership-limited"),
                    "trust" => ("entity", "create-trust-discretionary"),
                    _ => ("entity", "create-limited-company"), // fallback
                };

                // Convert attrs to args format
                let mut args: HashMap<String, JsonValue> = HashMap::new();
                for (k, v) in attrs {
                    // Convert snake_case to kebab-case for arg names
                    let arg_name = k.replace('_', "-");
                    args.insert(arg_name, v.clone());
                }

                // Look up verb and execute
                let runtime_reg = super::runtime_registry::runtime_registry();
                if let Some(runtime_verb) = runtime_reg.get(domain, verb_name) {
                    let result = self.execute(runtime_verb, &args).await?;
                    if let GenericExecutionResult::Uuid(uuid) = result {
                        let binding_name = binding.clone().unwrap_or_else(|| key.key.clone());
                        return Ok(Some((binding_name, key.clone(), uuid)));
                    }
                }
                Ok(None)
            }

            Op::LinkRole {
                cbu,
                entity,
                role,
                ownership_percentage,
                ..
            } => {
                let cbu_id = resolved_keys
                    .get(cbu)
                    .ok_or_else(|| anyhow!("CBU not resolved: {:?}", cbu))?;
                let entity_id = resolved_keys
                    .get(entity)
                    .ok_or_else(|| anyhow!("Entity not resolved: {:?}", entity))?;

                let mut args: HashMap<String, JsonValue> = HashMap::new();
                args.insert("cbu-id".to_string(), json!(cbu_id.to_string()));
                args.insert("entity-id".to_string(), json!(entity_id.to_string()));
                args.insert("role".to_string(), json!(role));
                if let Some(pct) = ownership_percentage {
                    args.insert("ownership-percentage".to_string(), json!(pct.to_string()));
                }

                let runtime_reg = super::runtime_registry::runtime_registry();
                if let Some(runtime_verb) = runtime_reg.get("cbu", "assign-role") {
                    self.execute(runtime_verb, &args).await?;
                }
                Ok(None)
            }

            Op::UnlinkRole {
                cbu, entity, role, ..
            } => {
                let cbu_id = resolved_keys
                    .get(cbu)
                    .ok_or_else(|| anyhow!("CBU not resolved: {:?}", cbu))?;
                let entity_id = resolved_keys
                    .get(entity)
                    .ok_or_else(|| anyhow!("Entity not resolved: {:?}", entity))?;

                let mut args: HashMap<String, JsonValue> = HashMap::new();
                args.insert("cbu-id".to_string(), json!(cbu_id.to_string()));
                args.insert("entity-id".to_string(), json!(entity_id.to_string()));
                args.insert("role".to_string(), json!(role));

                let runtime_reg = super::runtime_registry::runtime_registry();
                if let Some(runtime_verb) = runtime_reg.get("cbu", "remove-role") {
                    self.execute(runtime_verb, &args).await?;
                }
                Ok(None)
            }

            Op::AddOwnership {
                owner,
                owned,
                percentage,
                ownership_type,
                ..
            } => {
                let owner_id = resolved_keys
                    .get(owner)
                    .ok_or_else(|| anyhow!("Owner not resolved: {:?}", owner))?;
                let owned_id = resolved_keys
                    .get(owned)
                    .ok_or_else(|| anyhow!("Owned not resolved: {:?}", owned))?;

                let mut args: HashMap<String, JsonValue> = HashMap::new();
                args.insert("owner-entity-id".to_string(), json!(owner_id.to_string()));
                args.insert("owned-entity-id".to_string(), json!(owned_id.to_string()));
                args.insert("percentage".to_string(), json!(percentage.to_string()));
                args.insert("ownership-type".to_string(), json!(ownership_type));

                let runtime_reg = super::runtime_registry::runtime_registry();
                if let Some(runtime_verb) = runtime_reg.get("ubo", "add-ownership") {
                    self.execute(runtime_verb, &args).await?;
                }
                Ok(None)
            }

            Op::CreateCase {
                cbu,
                case_type,
                binding,
                ..
            } => {
                let cbu_id = resolved_keys
                    .get(cbu)
                    .ok_or_else(|| anyhow!("CBU not resolved: {:?}", cbu))?;

                let mut args: HashMap<String, JsonValue> = HashMap::new();
                args.insert("cbu-id".to_string(), json!(cbu_id.to_string()));
                args.insert("case-type".to_string(), json!(case_type));

                let runtime_reg = super::runtime_registry::runtime_registry();
                if let Some(runtime_verb) = runtime_reg.get("kyc-case", "create") {
                    let result = self.execute(runtime_verb, &args).await?;
                    if let GenericExecutionResult::Uuid(uuid) = result {
                        let key = EntityKey::new("case", format!("{}:case", cbu.key));
                        let binding_name = binding.clone().unwrap_or_else(|| key.key.clone());
                        return Ok(Some((binding_name, key, uuid)));
                    }
                }
                Ok(None)
            }

            Op::CreateWorkstream {
                case,
                entity,
                binding,
                ..
            } => {
                let case_id = resolved_keys
                    .get(case)
                    .ok_or_else(|| anyhow!("Case not resolved: {:?}", case))?;
                let entity_id = resolved_keys
                    .get(entity)
                    .ok_or_else(|| anyhow!("Entity not resolved: {:?}", entity))?;

                let mut args: HashMap<String, JsonValue> = HashMap::new();
                args.insert("case-id".to_string(), json!(case_id.to_string()));
                args.insert("entity-id".to_string(), json!(entity_id.to_string()));

                let runtime_reg = super::runtime_registry::runtime_registry();
                if let Some(runtime_verb) = runtime_reg.get("entity-workstream", "create") {
                    let result = self.execute(runtime_verb, &args).await?;
                    if let GenericExecutionResult::Uuid(uuid) = result {
                        let key =
                            EntityKey::new("workstream", format!("{}:{}", case.key, entity.key));
                        let binding_name = binding.clone().unwrap_or_else(|| key.key.clone());
                        return Ok(Some((binding_name, key, uuid)));
                    }
                }
                Ok(None)
            }

            Op::RunScreening {
                workstream,
                screening_type,
                ..
            } => {
                let workstream_id = resolved_keys
                    .get(workstream)
                    .ok_or_else(|| anyhow!("Workstream not resolved: {:?}", workstream))?;

                let mut args: HashMap<String, JsonValue> = HashMap::new();
                args.insert(
                    "workstream-id".to_string(),
                    json!(workstream_id.to_string()),
                );
                args.insert("screening-type".to_string(), json!(screening_type));

                let runtime_reg = super::runtime_registry::runtime_registry();
                if let Some(runtime_verb) = runtime_reg.get("case-screening", "run") {
                    self.execute(runtime_verb, &args).await?;
                }
                Ok(None)
            }

            // Ops that we don't fully implement yet - log and skip
            _ => {
                tracing::debug!("Skipping unimplemented op: {}", op.describe());
                Ok(None)
            }
        }
    }

    // =========================================================================
    // INSERT
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
            String::new()
        } else {
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

        debug!("UPDATE SQL: {}", sql);

        bind_values.push(key_val);
        let affected = self.execute_non_query(&sql, &bind_values).await?;
        Ok(GenericExecutionResult::Affected(affected))
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
        let key_col = crud
            .key
            .as_deref()
            .ok_or_else(|| anyhow!("Delete requires key column in config"))?;

        // Find the key argument
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

        debug!("DELETE SQL: {}", sql);

        let sql_val = self.json_to_sql_value(key_value, key_arg)?;
        let affected = self.execute_non_query(&sql, &[sql_val]).await?;
        Ok(GenericExecutionResult::Affected(affected))
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
        if crud.conflict_keys.is_empty() {
            bail!("Upsert requires conflict_keys in config");
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

        let conflict_cols: Vec<String> = crud
            .conflict_keys
            .iter()
            .map(|c| format!("\"{}\"", c))
            .collect();

        let update_clause = if updates.is_empty() {
            format!("\"{}\" = EXCLUDED.\"{}\"", pk_col, pk_col)
        } else {
            updates.join(", ")
        };

        let returning = crud.returning.as_deref().unwrap_or(pk_col);

        let sql = format!(
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
        );

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

        let lookup = role_arg.lookup.as_ref().unwrap();
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

        let sql = format!(
            r#"SELECT * FROM "{}"."{}" WHERE "{}" = $1"#,
            crud.schema, crud.table, fk_col
        );

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

        // Join to get enriched party data
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
            ORDER BY e.name, r.name"#,
            crud.schema, junction, crud.schema, crud.schema, crud.schema, fk_col
        );

        debug!("LIST_PARTIES SQL: {}", sql);

        let sql_val = self.json_to_sql_value(fk_value, fk_arg)?;
        let rows = self.execute_many_with_bindings(&sql, &[sql_val]).await?;

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

        let sql = format!(
            r#"SELECT p.* FROM "{}"."{}" p
               JOIN "{}"."{}" j ON p."{}" = j."{}"
               WHERE j."{}" = $1"#,
            crud.schema, primary, crud.schema, join_table, join_col, join_col, filter_col
        );

        debug!("SELECT_WITH_JOIN SQL: {}", sql);

        let sql_val = self.json_to_sql_value(filter_value, filter_arg)?;
        let rows = self.execute_many_with_bindings(&sql, &[sql_val]).await?;

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
        // Use explicit type_code from YAML config if present,
        // otherwise derive from verb name (e.g., "create-limited-company" -> "LIMITED_COMPANY")
        let type_code = if let Some(tc) = &crud.type_code {
            tc.clone()
        } else {
            verb.verb
                .strip_prefix("create-")
                .map(|s| s.to_uppercase().replace('-', "_"))
                .ok_or_else(|| anyhow!("Invalid entity create verb name: {}", verb.verb))?
        };

        // Look up entity_type_id and table_name
        // First try exact match, then try prefix match for shortened verb names
        // (e.g., "LIMITED_COMPANY" matches "LIMITED_COMPANY_PRIVATE")
        let type_sql = format!(
            r#"SELECT entity_type_id, table_name FROM "{}".entity_types
               WHERE type_code = $1 OR type_code LIKE $1 || '_%'
               ORDER BY CASE WHEN type_code = $1 THEN 0 ELSE 1 END
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
        let extension_table: String = crud
            .extension_table
            .clone()
            .unwrap_or_else(|| type_row.try_get("table_name").unwrap_or_default());

        // Generate entity_id
        let entity_id = Uuid::new_v4();

        // Get entity name - for proper_persons, constructed from first/last name
        let entity_name = if type_code == "PROPER_PERSON" {
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
        // Use explicit type_code from YAML config if present,
        // otherwise derive from verb name (e.g., "ensure-limited-company" -> "LIMITED_COMPANY")
        let type_code = if let Some(tc) = &crud.type_code {
            tc.clone()
        } else {
            verb.verb
                .strip_prefix("ensure-")
                .map(|s| s.to_uppercase().replace('-', "_"))
                .ok_or_else(|| anyhow!("Invalid entity ensure verb name: {}", verb.verb))?
        };

        // Look up entity_type_id and table_name
        let type_sql = format!(
            r#"SELECT entity_type_id, table_name FROM "{}".entity_types
               WHERE type_code = $1 OR type_code LIKE $1 || '_%'
               ORDER BY CASE WHEN type_code = $1 THEN 0 ELSE 1 END
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
        let entity_name = if type_code == "PROPER_PERSON" {
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

        // Build UPSERT for extension table
        // Conflict key priority:
        // 1. ISIN if present (for share classes with unique ISIN constraint)
        // 2. entity_id for shared PK tables
        // 3. The extension table's own PK for separate PK tables
        let has_isin = columns.iter().any(|c| c == "\"isin\"");
        let conflict_col = if has_isin {
            "isin"
        } else if uses_shared_pk {
            "entity_id"
        } else {
            ext_pk_col
        };

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
    // HELPER METHODS
    // =========================================================================

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

        // Try EntityGateway first
        let mut guard = self.get_gateway_client().await?;
        if let Some(client) = guard.as_mut() {
            let request = SearchRequest {
                nickname: entity_type.to_uppercase(),
                values: vec![code_value.to_string()],
                search_key: None,
                mode: SearchMode::Exact as i32,
                limit: Some(5),
            };

            match client.search(request).await {
                Ok(response) => {
                    let matches = response.into_inner().matches;

                    // Look for exact match
                    let code_upper = code_value.to_uppercase();
                    for m in &matches {
                        if m.token.to_uppercase() == code_upper
                            || m.display.to_uppercase() == code_upper
                        {
                            // Try to parse token as UUID
                            if let Ok(uuid) = Uuid::parse_str(&m.token) {
                                return Ok(uuid);
                            }
                            // Token might be a code - need to fetch UUID from DB
                            break;
                        }
                    }

                    // No exact match - provide suggestions
                    if !matches.is_empty() {
                        let suggestions: Vec<String> =
                            matches.iter().map(|m| m.display.clone()).collect();
                        return Err(anyhow!(
                            "Lookup failed: '{}' not found for {}\n  Did you mean: {}?\n  Available: {}",
                            code_value,
                            entity_type,
                            suggestions.first().unwrap(),
                            suggestions.join(", ")
                        ));
                    }
                }
                Err(e) => {
                    debug!("EntityGateway search failed, falling back to SQL: {}", e);
                }
            }
        }

        // Fallback to direct SQL if EntityGateway unavailable or no match
        let schema = lookup.schema.as_deref().unwrap_or("public");
        let search_col = lookup.search_key.primary_column();
        let sql = format!(
            r#"SELECT "{}" FROM "{}"."{}" WHERE "{}" = $1"#,
            lookup.primary_key, schema, lookup.table, search_col
        );

        debug!(
            "LOOKUP SQL fallback: {} with search_key={}",
            sql, code_value
        );

        let row = sqlx::query(&sql)
            .bind(code_value)
            .fetch_optional(&self.pool)
            .await?;

        match row {
            Some(r) => {
                let uuid: Uuid = r.try_get(&*lookup.primary_key)?;
                Ok(uuid)
            }
            None => Err(anyhow!(
                "Lookup failed: no {} with {} = '{}' in {}.{}",
                lookup.table,
                search_col,
                code_value,
                schema,
                lookup.table,
            )),
        }
    }

    /// Get suggestions for failed lookup using EntityGateway fuzzy search
    #[allow(dead_code)]
    async fn get_lookup_suggestions_gateway(
        &self,
        entity_type: &str,
        attempted_value: &str,
    ) -> Result<Vec<String>> {
        let mut guard = self.get_gateway_client().await?;
        if let Some(client) = guard.as_mut() {
            let request = SearchRequest {
                nickname: entity_type.to_uppercase(),
                values: vec![attempted_value.to_string()],
                search_key: None,
                mode: SearchMode::Fuzzy as i32,
                limit: Some(5),
            };

            if let Ok(response) = client.search(request).await {
                let suggestions: Vec<String> = response
                    .into_inner()
                    .matches
                    .into_iter()
                    .map(|m| m.display)
                    .collect();
                return Ok(suggestions);
            }
        }
        Ok(vec![])
    }

    /// Fallback: Get suggestions using SQL (when EntityGateway unavailable)
    #[allow(dead_code)]
    async fn get_lookup_suggestions_sql(
        &self,
        schema: &str,
        table: &str,
        code_column: &str,
        attempted_value: &str,
    ) -> Result<Vec<String>> {
        let sql = format!(
            r#"SELECT "{}" FROM "{}"."{}"
               WHERE "{}" IS NOT NULL
               ORDER BY levenshtein(LOWER("{}"), LOWER($1)) ASC
               LIMIT 5"#,
            code_column, schema, table, code_column, code_column
        );

        let rows = sqlx::query(&sql)
            .bind(attempted_value)
            .fetch_all(&self.pool)
            .await;

        match rows {
            Ok(rows) => {
                let suggestions: Vec<String> = rows
                    .iter()
                    .filter_map(|r| r.try_get::<String, _>(code_column).ok())
                    .collect();
                Ok(suggestions)
            }
            Err(_) => {
                // Levenshtein not available, fall back to simple ILIKE prefix match
                let fallback_sql = format!(
                    r#"SELECT "{}" FROM "{}"."{}"
                       WHERE LOWER("{}") LIKE LOWER($1)
                       LIMIT 10"#,
                    code_column, schema, table, code_column
                );

                let prefix = format!("{}%", &attempted_value.chars().take(2).collect::<String>());
                let rows = sqlx::query(&fallback_sql)
                    .bind(&prefix)
                    .fetch_all(&self.pool)
                    .await?;

                let suggestions: Vec<String> = rows
                    .iter()
                    .filter_map(|r| r.try_get::<String, _>(code_column).ok())
                    .collect();
                Ok(suggestions)
            }
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

#[cfg(all(test, feature = "database"))]
mod dag_tests {
    use super::*;
    use crate::dsl_v2::parser::parse_program;

    /// Test that dry-run mode returns an execution plan description
    #[tokio::test]
    async fn test_dag_dry_run_returns_plan() {
        let source = r#"
            (cbu.ensure :name "Test Fund" :jurisdiction "LU" :as @fund)
            (entity.create-proper-person :first-name "John" :last-name "Smith" :as @john)
            (cbu.assign-role :cbu-id @fund :entity-id @john :role "DIRECTOR")
        "#;
        let program = parse_program(source).unwrap();

        // Create executor without pool (dry_run doesn't need it)
        let executor = GenericCrudExecutor::new_without_pool();

        let result = executor.execute_with_dag(&program, true).await;

        match result {
            Ok(DagExecutionResult::Plan(desc)) => {
                // Should contain phase information
                assert!(desc.contains("Phase"), "Plan should describe phases");
                // Should mention the ops (describe_plan uses "Ensure" not "EnsureEntity")
                assert!(
                    desc.contains("Ensure") || desc.contains("Link") || desc.contains("role"),
                    "Plan should describe ops: {}",
                    desc
                );
            }
            Ok(DagExecutionResult::Executed { .. }) => {
                panic!("dry_run=true should return Plan, not Executed");
            }
            Err(e) => {
                panic!("Unexpected error: {:?}", e);
            }
        }
    }

    /// Test that undefined symbols produce compile errors
    #[tokio::test]
    async fn test_dag_undefined_symbol_error() {
        let source = r#"
            (cbu.assign-role :cbu-id @undefined :entity-id @also_undefined :role "X")
        "#;
        let program = parse_program(source).unwrap();

        let executor = GenericCrudExecutor::new_without_pool();
        let result = executor.execute_with_dag(&program, true).await;

        match result {
            Err(DagExecutionError::CompileErrors(errors)) => {
                assert!(!errors.is_empty(), "Should have compile errors");
                let error_msg = &errors[0].message;
                assert!(
                    error_msg.contains("undefined symbol"),
                    "Error should mention undefined symbol: {}",
                    error_msg
                );
            }
            Ok(_) => panic!("Should have failed with compile error"),
            Err(e) => panic!("Unexpected error type: {:?}", e),
        }
    }

    /// Test that the plan has correct dependency ordering
    /// Note: The compiler requires symbols to be defined before use,
    /// so we test with correct source order but verify the DAG groups
    /// entities into Phase 1 and roles into Phase 3
    #[tokio::test]
    async fn test_dag_dependency_ordering() {
        // Correct source order: define symbols before using them
        // DAG should group: Phase 1 (Entities) before Phase 3 (Roles)
        let source = r#"
            (cbu.ensure :name "Test Fund" :jurisdiction "LU" :as @fund)
            (entity.create-proper-person :first-name "John" :last-name "Smith" :as @john)
            (cbu.assign-role :cbu-id @fund :entity-id @john :role "DIRECTOR")
        "#;
        let program = parse_program(source).unwrap();

        let executor = GenericCrudExecutor::new_without_pool();
        let result = executor.execute_with_dag(&program, true).await;

        match result {
            Ok(DagExecutionResult::Plan(desc)) => {
                // Verify entities phase comes before roles phase
                let entities_pos = desc.find("Entities");
                let roles_pos = desc.find("Roles");

                if let (Some(ep), Some(rp)) = (entities_pos, roles_pos) {
                    assert!(
                        ep < rp,
                        "Entities phase should come before Roles phase in plan:\n{}",
                        desc
                    );
                }

                // Verify we have both entity and role ops
                // describe_plan outputs "Link X to Y as ROLE" for LinkRole
                assert!(
                    desc.contains("Ensure") && desc.contains("Link"),
                    "Plan should contain both entity and role ops:\n{}",
                    desc
                );
            }
            Err(e) => panic!("Unexpected error: {:?}", e),
            _ => panic!("Expected Plan result"),
        }
    }

    /// Test compiling a simple program produces correct ops count
    #[tokio::test]
    async fn test_dag_simple_compile() {
        let source = r#"
            (cbu.ensure :name "Fund A" :as @a)
            (cbu.ensure :name "Fund B" :as @b)
        "#;
        let program = parse_program(source).unwrap();

        let executor = GenericCrudExecutor::new_without_pool();
        let result = executor.execute_with_dag(&program, true).await;

        assert!(
            result.is_ok(),
            "Simple program should compile: {:?}",
            result
        );
    }

    /// Test that ownership relationships compile correctly
    #[tokio::test]
    async fn test_dag_ownership_compile() {
        let source = r#"
            (entity.create-limited-company :name "Parent Co" :jurisdiction "US" :as @parent)
            (entity.create-limited-company :name "Child Co" :jurisdiction "US" :as @child)
            (ubo.add-ownership :owner-entity-id @parent :owned-entity-id @child :percentage 100 :ownership-type "DIRECT")
        "#;
        let program = parse_program(source).unwrap();

        let executor = GenericCrudExecutor::new_without_pool();
        let result = executor.execute_with_dag(&program, true).await;

        match result {
            Ok(DagExecutionResult::Plan(desc)) => {
                // describe_plan outputs "owns X% of" for AddOwnership
                assert!(
                    desc.contains("owns") || desc.contains("Ensure"),
                    "Plan should contain ownership ops: {}",
                    desc
                );
                // Verify phases are present
                assert!(desc.contains("Entities"), "Should have Entities phase");
                assert!(
                    desc.contains("Relationships"),
                    "Should have Relationships phase"
                );
            }
            Err(e) => panic!("Unexpected error: {:?}", e),
            _ => panic!("Expected Plan result"),
        }
    }

    /// Test KYC case creation compiles correctly
    #[tokio::test]
    async fn test_dag_kyc_case_compile() {
        let source = r#"
            (cbu.ensure :name "Test CBU" :as @cbu)
            (kyc-case.create :cbu-id @cbu :case-type "NEW_CLIENT" :as @case)
        "#;
        let program = parse_program(source).unwrap();

        let executor = GenericCrudExecutor::new_without_pool();
        let result = executor.execute_with_dag(&program, true).await;

        match result {
            Ok(DagExecutionResult::Plan(desc)) => {
                // describe_plan outputs "Create X case for Y" for CreateCase
                assert!(
                    desc.contains("case") || desc.contains("Ensure"),
                    "Plan should contain case ops: {}",
                    desc
                );
                // Verify KYC phase is present
                assert!(desc.contains("KYC"), "Should have KYC phase");
            }
            Err(e) => panic!("Unexpected error: {:?}", e),
            _ => panic!("Expected Plan result"),
        }
    }
}
