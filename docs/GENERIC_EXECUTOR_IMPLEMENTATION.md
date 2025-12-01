# Generic Executor Implementation & Dead Code Cleanup

**Document for Claude Code**  
**Goal**: Complete the YAML-driven DSL system by implementing `generic_executor.rs` and integrating it with the existing infrastructure, then remove dead/replaced code.

---

## Executive Summary

The YAML configuration infrastructure is complete:
- `config/verbs.yaml` - 1,478 lines of verb definitions
- `config/csg_rules.yaml` - CSG validation rules  
- `src/dsl_v2/config/types.rs` - Serde structs for YAML
- `src/dsl_v2/config/loader.rs` - YAML loading
- `src/dsl_v2/runtime_registry.rs` - RuntimeVerbRegistry

**Remaining work**:
1. Create `generic_executor.rs` - Execute verbs from RuntimeVerb config
2. Integrate with existing `executor.rs` - Route YAML verbs through generic executor
3. Clean up dead code - Remove static arrays now replaced by YAML

---

## Part 1: Create `generic_executor.rs`

### File: `rust/src/dsl_v2/generic_executor.rs`

This executor reads verb configuration from `RuntimeVerb` and executes accordingly. No hardcoded table names or column mappings.

```rust
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
use tracing::debug;
use uuid::Uuid;

#[cfg(feature = "database")]
use sqlx::{PgPool, Row, postgres::PgRow};

use super::config::types::{ArgType, CrudOperation, LookupConfig, ReturnTypeConfig};
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
            GenericExecutionResult::Record(r) => super::executor::ExecutionResult::Record(r.clone()),
            GenericExecutionResult::RecordSet(rs) => super::executor::ExecutionResult::RecordSet(rs.clone()),
            GenericExecutionResult::Affected(n) => super::executor::ExecutionResult::Affected(*n),
            GenericExecutionResult::Void => super::executor::ExecutionResult::Void,
        }
    }
}

// =============================================================================
// GENERIC CRUD EXECUTOR
// =============================================================================

/// Generic CRUD executor - executes verbs based on YAML config
#[cfg(feature = "database")]
pub struct GenericCrudExecutor {
    pool: PgPool,
}

#[cfg(feature = "database")]
impl GenericCrudExecutor {
    pub fn new(pool: PgPool) -> Self {
        Self { pool }
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
        let crud = match &verb.behavior {
            RuntimeBehavior::Crud(crud) => crud,
            RuntimeBehavior::Plugin(handler) => {
                return Err(anyhow!(
                    "Verb {}.{} is a plugin (handler: {}), use plugin executor",
                    verb.domain, verb.verb, handler
                ));
            }
        };

        debug!("GenericCrudExecutor: executing {}.{} with operation {:?}",
            verb.domain, verb.verb, crud.operation);

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
            CrudOperation::SelectWithJoin => self.execute_select_with_join(verb, crud, args).await,
            CrudOperation::EntityCreate => self.execute_entity_create(verb, crud, args).await,
        }
    }

    // =========================================================================
    // INSERT
    // =========================================================================

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
        let pk_col = crud.returning.as_deref()
            .unwrap_or_else(|| Self::infer_pk_column(&crud.table));
        
        let new_id = Uuid::new_v4();
        columns.push(format!("\"{}\"", pk_col));
        placeholders.push("$1".to_string());
        bind_values.push(SqlValue::Uuid(new_id));
        let mut idx = 2;

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
                    bind_values.push(self.json_to_sql_value(value, arg_def)?);
                    idx += 1;
                }
            }
        }

        if columns.len() == 1 {
            // Only PK, no other columns
            bail!("No columns to insert for {}.{}", verb.domain, verb.verb);
        }

        let returning = crud.returning.as_deref().unwrap_or("*");
        let sql = format!(
            r#"INSERT INTO "{}"."{}" ({}) VALUES ({}) RETURNING "{}""#,
            crud.schema,
            crud.table,
            columns.join(", "),
            placeholders.join(", "),
            returning
        );

        debug!("INSERT SQL: {}", sql);

        let row = self.execute_with_bindings(&sql, &bind_values).await?;
        
        if returning != "*" {
            let uuid: Uuid = row.try_get(returning)?;
            Ok(GenericExecutionResult::Uuid(uuid))
        } else {
            Ok(GenericExecutionResult::Record(self.row_to_json(&row)?))
        }
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
            r#"SELECT * FROM "{}"."{}"{}{}{}""#,
            crud.schema,
            crud.table,
            where_clause,
            limit_clause,
            offset_clause
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
                let records: Result<Vec<JsonValue>> = rows.iter()
                    .map(|r| self.row_to_json(r))
                    .collect();
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
        let key_col = crud.key.as_deref()
            .ok_or_else(|| anyhow!("Update requires key column in config"))?;

        let mut sets = Vec::new();
        let mut bind_values: Vec<SqlValue> = Vec::new();
        let mut key_value: Option<SqlValue> = None;
        let mut idx = 1;

        for arg_def in &verb.args {
            if let Some(value) = args.get(&arg_def.name) {
                if let Some(col) = &arg_def.maps_to {
                    if col == key_col {
                        key_value = Some(self.json_to_sql_value(value, arg_def)?);
                    } else {
                        sets.push(format!("\"{}\" = ${}", col, idx));
                        bind_values.push(self.json_to_sql_value(value, arg_def)?);
                        idx += 1;
                    }
                }
            }
        }

        let key_val = key_value.ok_or_else(|| anyhow!("Missing key argument for update"))?;

        if sets.is_empty() {
            bail!("No columns to update for {}.{}", verb.domain, verb.verb);
        }

        // Add updated_at if not explicitly set
        sets.push("updated_at = NOW()".to_string());

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
        let key_col = crud.key.as_deref()
            .ok_or_else(|| anyhow!("Delete requires key column in config"))?;

        // Find the key argument
        let key_arg = verb.args.iter()
            .find(|a| a.maps_to.as_deref() == Some(key_col))
            .ok_or_else(|| anyhow!("Key argument not found in verb definition"))?;

        let key_value = args.get(&key_arg.name)
            .ok_or_else(|| anyhow!("Missing key argument: {}", key_arg.name))?;

        let sql = format!(
            r#"DELETE FROM "{}"."{}" WHERE "{}" = $1"#,
            crud.schema,
            crud.table,
            key_col
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

        let pk_col = crud.returning.as_deref()
            .unwrap_or_else(|| Self::infer_pk_column(&crud.table));

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

                    bind_values.push(self.json_to_sql_value(value, arg_def)?);
                    idx += 1;
                }
            }
        }

        let conflict_cols: Vec<String> = crud.conflict_keys.iter()
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

    async fn execute_link(
        &self,
        verb: &RuntimeVerb,
        crud: &RuntimeCrudConfig,
        args: &HashMap<String, JsonValue>,
    ) -> Result<GenericExecutionResult> {
        let junction = crud.junction.as_deref()
            .ok_or_else(|| anyhow!("Link requires junction table"))?;
        let from_col = crud.from_col.as_deref()
            .ok_or_else(|| anyhow!("Link requires from_col"))?;
        let to_col = crud.to_col.as_deref()
            .ok_or_else(|| anyhow!("Link requires to_col"))?;

        let pk_col = Self::infer_pk_column(junction);
        let new_id = Uuid::new_v4();

        let mut columns = vec![
            format!("\"{}\"", pk_col),
            format!("\"{}\"", from_col),
            format!("\"{}\"", to_col),
        ];
        let mut placeholders = vec!["$1".to_string(), "$2".to_string(), "$3".to_string()];
        let mut bind_values: Vec<SqlValue> = vec![SqlValue::Uuid(new_id)];

        // Get from/to values from args
        for arg_def in &verb.args {
            if let Some(value) = args.get(&arg_def.name) {
                if arg_def.maps_to.as_deref() == Some(from_col) {
                    bind_values.push(self.json_to_sql_value(value, arg_def)?);
                } else if arg_def.maps_to.as_deref() == Some(to_col) {
                    bind_values.push(self.json_to_sql_value(value, arg_def)?);
                }
            }
        }

        if bind_values.len() < 3 {
            bail!("Missing from or to argument for link");
        }

        // Add extra junction columns
        let mut idx = 4;
        for arg_def in &verb.args {
            if let Some(value) = args.get(&arg_def.name) {
                if let Some(col) = &arg_def.maps_to {
                    if col != from_col && col != to_col && col != &pk_col {
                        columns.push(format!("\"{}\"", col));
                        placeholders.push(format!("${}", idx));
                        bind_values.push(self.json_to_sql_value(value, arg_def)?);
                        idx += 1;
                    }
                }
            }
        }

        let sql = format!(
            r#"INSERT INTO "{}"."{}" ({}) VALUES ({}) ON CONFLICT DO NOTHING RETURNING "{}""#,
            crud.schema,
            junction,
            columns.join(", "),
            placeholders.join(", "),
            pk_col
        );

        debug!("LINK SQL: {}", sql);

        let row = self.execute_with_bindings(&sql, &bind_values).await?;
        let uuid: Uuid = row.try_get(&pk_col as &str)?;
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
        let junction = crud.junction.as_deref()
            .ok_or_else(|| anyhow!("Unlink requires junction table"))?;
        let from_col = crud.from_col.as_deref()
            .ok_or_else(|| anyhow!("Unlink requires from_col"))?;
        let to_col = crud.to_col.as_deref()
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
            crud.schema,
            junction,
            from_col,
            to_col
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
    // ROLE LINK (Junction with role lookup)
    // =========================================================================

    async fn execute_role_link(
        &self,
        verb: &RuntimeVerb,
        crud: &RuntimeCrudConfig,
        args: &HashMap<String, JsonValue>,
    ) -> Result<GenericExecutionResult> {
        let junction = crud.junction.as_deref()
            .ok_or_else(|| anyhow!("RoleLink requires junction table"))?;
        let from_col = crud.from_col.as_deref()
            .ok_or_else(|| anyhow!("RoleLink requires from_col"))?;
        let to_col = crud.to_col.as_deref()
            .ok_or_else(|| anyhow!("RoleLink requires to_col"))?;
        let role_table = crud.role_table.as_deref().unwrap_or("roles");
        let role_col = crud.role_col.as_deref().unwrap_or("role_id");

        // Find the lookup argument for role
        let role_arg = verb.args.iter()
            .find(|a| a.arg_type == ArgType::Lookup && a.lookup.is_some())
            .ok_or_else(|| anyhow!("RoleLink requires lookup argument for role"))?;

        let role_value = args.get(&role_arg.name)
            .ok_or_else(|| anyhow!("Missing role argument"))?;

        let lookup = role_arg.lookup.as_ref().unwrap();
        let role_code = role_value.as_str()
            .ok_or_else(|| anyhow!("Role must be a string"))?;

        // Look up role_id
        let lookup_sql = format!(
            r#"SELECT "{}" FROM "{}"."{}" WHERE "{}" = $1"#,
            lookup.id_column,
            crud.schema,
            lookup.table,
            lookup.code_column
        );

        let role_row = sqlx::query(&lookup_sql)
            .bind(role_code)
            .fetch_one(&self.pool)
            .await?;

        let role_id: Uuid = role_row.try_get(&lookup.id_column as &str)?;

        // Build insert
        let pk_col = Self::infer_pk_column(junction);
        let new_id = Uuid::new_v4();

        let mut columns = vec![
            format!("\"{}\"", pk_col),
            format!("\"{}\"", from_col),
            format!("\"{}\"", to_col),
            format!("\"{}\"", role_col),
        ];
        let mut placeholders = vec!["$1".to_string(), "$2".to_string(), "$3".to_string(), "$4".to_string()];
        let mut bind_values: Vec<SqlValue> = vec![SqlValue::Uuid(new_id)];

        // Get from/to values
        for arg_def in &verb.args {
            if let Some(value) = args.get(&arg_def.name) {
                if arg_def.maps_to.as_deref() == Some(from_col) {
                    bind_values.push(self.json_to_sql_value(value, arg_def)?);
                } else if arg_def.maps_to.as_deref() == Some(to_col) {
                    bind_values.push(self.json_to_sql_value(value, arg_def)?);
                }
            }
        }

        bind_values.push(SqlValue::Uuid(role_id));

        // Add extra columns (like ownership-percentage)
        let mut idx = 5;
        for arg_def in &verb.args {
            if let Some(value) = args.get(&arg_def.name) {
                if let Some(col) = &arg_def.maps_to {
                    if col != from_col && col != to_col && col != &pk_col && arg_def.arg_type != ArgType::Lookup {
                        columns.push(format!("\"{}\"", col));
                        placeholders.push(format!("${}", idx));
                        bind_values.push(self.json_to_sql_value(value, arg_def)?);
                        idx += 1;
                    }
                }
            }
        }

        let returning = crud.returning.as_deref().unwrap_or(&pk_col);
        let sql = format!(
            r#"INSERT INTO "{}"."{}" ({}) VALUES ({}) RETURNING "{}""#,
            crud.schema,
            junction,
            columns.join(", "),
            placeholders.join(", "),
            returning
        );

        debug!("ROLE_LINK SQL: {}", sql);

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
        let junction = crud.junction.as_deref()
            .ok_or_else(|| anyhow!("RoleUnlink requires junction table"))?;
        let from_col = crud.from_col.as_deref()
            .ok_or_else(|| anyhow!("RoleUnlink requires from_col"))?;
        let to_col = crud.to_col.as_deref()
            .ok_or_else(|| anyhow!("RoleUnlink requires to_col"))?;
        let role_col = crud.role_col.as_deref().unwrap_or("role_id");

        // Find and lookup role
        let role_arg = verb.args.iter()
            .find(|a| a.arg_type == ArgType::Lookup && a.lookup.is_some())
            .ok_or_else(|| anyhow!("RoleUnlink requires lookup argument"))?;

        let role_value = args.get(&role_arg.name)
            .ok_or_else(|| anyhow!("Missing role argument"))?;

        let lookup = role_arg.lookup.as_ref().unwrap();
        let role_code = role_value.as_str()
            .ok_or_else(|| anyhow!("Role must be a string"))?;

        let lookup_sql = format!(
            r#"SELECT "{}" FROM "{}"."{}" WHERE "{}" = $1"#,
            lookup.id_column,
            crud.schema,
            lookup.table,
            lookup.code_column
        );

        let role_row = sqlx::query(&lookup_sql)
            .bind(role_code)
            .fetch_one(&self.pool)
            .await?;

        let role_id: Uuid = role_row.try_get(&lookup.id_column as &str)?;

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
            crud.schema,
            junction,
            from_col,
            to_col,
            role_col
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
        let fk_col = crud.fk_col.as_deref()
            .ok_or_else(|| anyhow!("ListByFk requires fk_col"))?;

        // Find the FK argument (first required arg typically)
        let fk_arg = verb.args.iter()
            .find(|a| a.required)
            .ok_or_else(|| anyhow!("ListByFk requires a required argument"))?;

        let fk_value = args.get(&fk_arg.name)
            .ok_or_else(|| anyhow!("Missing FK argument: {}", fk_arg.name))?;

        let sql = format!(
            r#"SELECT * FROM "{}"."{}" WHERE "{}" = $1"#,
            crud.schema,
            crud.table,
            fk_col
        );

        debug!("LIST_BY_FK SQL: {}", sql);

        let sql_val = self.json_to_sql_value(fk_value, fk_arg)?;
        let rows = self.execute_many_with_bindings(&sql, &[sql_val]).await?;

        let records: Result<Vec<JsonValue>> = rows.iter()
            .map(|r| self.row_to_json(r))
            .collect();
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
        let junction = crud.junction.as_deref()
            .ok_or_else(|| anyhow!("ListParties requires junction"))?;
        let fk_col = crud.fk_col.as_deref()
            .ok_or_else(|| anyhow!("ListParties requires fk_col"))?;

        let fk_arg = verb.args.iter()
            .find(|a| a.required)
            .ok_or_else(|| anyhow!("ListParties requires FK argument"))?;

        let fk_value = args.get(&fk_arg.name)
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
            crud.schema, junction,
            crud.schema,
            crud.schema,
            crud.schema,
            fk_col
        );

        debug!("LIST_PARTIES SQL: {}", sql);

        let sql_val = self.json_to_sql_value(fk_value, fk_arg)?;
        let rows = self.execute_many_with_bindings(&sql, &[sql_val]).await?;

        let records: Result<Vec<JsonValue>> = rows.iter()
            .map(|r| self.row_to_json(r))
            .collect();
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
        let primary = crud.primary_table.as_deref()
            .ok_or_else(|| anyhow!("SelectWithJoin requires primary_table"))?;
        let join_table = crud.join_table.as_deref()
            .ok_or_else(|| anyhow!("SelectWithJoin requires join_table"))?;
        let join_col = crud.join_col.as_deref()
            .ok_or_else(|| anyhow!("SelectWithJoin requires join_col"))?;
        let filter_col = crud.filter_col.as_deref()
            .ok_or_else(|| anyhow!("SelectWithJoin requires filter_col"))?;

        let filter_arg = verb.args.iter()
            .find(|a| a.required)
            .ok_or_else(|| anyhow!("SelectWithJoin requires filter argument"))?;

        let filter_value = args.get(&filter_arg.name)
            .ok_or_else(|| anyhow!("Missing filter argument"))?;

        let sql = format!(
            r#"SELECT p.* FROM "{}"."{}" p
               JOIN "{}"."{}" j ON p."{}" = j."{}"
               WHERE j."{}" = $1"#,
            crud.schema, primary,
            crud.schema, join_table,
            join_col, join_col,
            filter_col
        );

        debug!("SELECT_WITH_JOIN SQL: {}", sql);

        let sql_val = self.json_to_sql_value(filter_value, filter_arg)?;
        let rows = self.execute_many_with_bindings(&sql, &[sql_val]).await?;

        let records: Result<Vec<JsonValue>> = rows.iter()
            .map(|r| self.row_to_json(r))
            .collect();
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
        // Extract type code from verb name (e.g., "create-limited-company" -> "LIMITED_COMPANY")
        let type_code = verb.verb
            .strip_prefix("create-")
            .map(|s| s.to_uppercase().replace('-', "_"))
            .ok_or_else(|| anyhow!("Invalid entity create verb name: {}", verb.verb))?;

        // Look up entity_type_id and table_name
        let type_sql = format!(
            r#"SELECT entity_type_id, table_name FROM "{}".entity_types WHERE type_code = $1"#,
            crud.schema
        );

        let type_row = sqlx::query(&type_sql)
            .bind(&type_code)
            .fetch_one(&self.pool)
            .await?;

        let entity_type_id: Uuid = type_row.try_get("entity_type_id")?;
        let extension_table: String = type_row.try_get("table_name")?;

        // Generate entity_id
        let entity_id = Uuid::new_v4();

        // Get entity name - for proper_persons, constructed from first/last name
        let entity_name = if type_code == "PROPER_PERSON" {
            let first = args.get("first-name")
                .and_then(|v| v.as_str())
                .unwrap_or("");
            let last = args.get("last-name")
                .and_then(|v| v.as_str())
                .unwrap_or("");
            format!("{} {}", first, last).trim().to_string()
        } else {
            args.get("name")
                .and_then(|v| v.as_str())
                .unwrap_or("Unknown")
                .to_string()
        };

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
        let ext_pk_col = Self::infer_pk_column(&extension_table);
        let ext_pk_id = Uuid::new_v4();

        let mut columns = vec![format!("\"{}\"", ext_pk_col), "\"entity_id\"".to_string()];
        let mut placeholders = vec!["$1".to_string(), "$2".to_string()];
        let mut bind_values: Vec<SqlValue> = vec![SqlValue::Uuid(ext_pk_id), SqlValue::Uuid(entity_id)];
        let mut idx = 3;

        // Add extension table columns
        for arg_def in &verb.args {
            if let Some(value) = args.get(&arg_def.name) {
                // Skip special keys
                if arg_def.name == "entity-type" || arg_def.name == "entity-id" {
                    continue;
                }
                if let Some(col) = &arg_def.maps_to {
                    if col == &ext_pk_col || col == "entity_id" {
                        continue;
                    }
                    columns.push(format!("\"{}\"", col));
                    placeholders.push(format!("${}", idx));
                    bind_values.push(self.json_to_sql_value(value, arg_def)?);
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
    // HELPER METHODS
    // =========================================================================

    /// Infer PK column name from table name (convention: singular_id)
    fn infer_pk_column(table: &str) -> &str {
        // Common patterns
        match table {
            "cbus" => "cbu_id",
            "entities" => "entity_id",
            "products" => "product_id",
            "services" => "service_id",
            "roles" => "role_id",
            "prod_resources" => "resource_id",
            "resource_attribute_requirements" => "requirement_id",
            "cbu_entity_roles" => "cbu_entity_role_id",
            "product_services" => "product_service_id",
            "service_resources" => "service_resource_id",
            "document_catalog" => "document_id",
            _ => {
                // Default: strip trailing 's' and add '_id'
                if table.ends_with("ies") {
                    // e.g., "entities" -> handled above, but fallback
                    "id"
                } else if table.ends_with('s') {
                    // e.g., "cbus" -> handled, "products" -> handled
                    "id"
                } else {
                    "id"
                }
            }
        }
    }

    /// Convert JSON value to SQL value based on argument type
    fn json_to_sql_value(&self, value: &JsonValue, arg: &RuntimeArg) -> Result<SqlValue> {
        match arg.arg_type {
            ArgType::String => {
                let s = value.as_str()
                    .ok_or_else(|| anyhow!("Expected string for {}", arg.name))?;
                Ok(SqlValue::String(s.to_string()))
            }
            ArgType::Uuid => {
                let s = value.as_str()
                    .ok_or_else(|| anyhow!("Expected UUID string for {}", arg.name))?;
                let uuid = Uuid::parse_str(s)?;
                Ok(SqlValue::Uuid(uuid))
            }
            ArgType::Integer => {
                let n = value.as_i64()
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
                let b = value.as_bool()
                    .ok_or_else(|| anyhow!("Expected boolean for {}", arg.name))?;
                Ok(SqlValue::Boolean(b))
            }
            ArgType::Json => {
                Ok(SqlValue::Json(value.clone()))
            }
            ArgType::Lookup => {
                // Lookup values are strings (the code to look up)
                let s = value.as_str()
                    .ok_or_else(|| anyhow!("Expected string for lookup {}", arg.name))?;
                Ok(SqlValue::String(s.to_string()))
            }
            ArgType::Date => {
                let s = value.as_str()
                    .ok_or_else(|| anyhow!("Expected date string for {}", arg.name))?;
                let d = chrono::NaiveDate::parse_from_str(s, "%Y-%m-%d")?;
                Ok(SqlValue::Date(d))
            }
            ArgType::Timestamp => {
                let s = value.as_str()
                    .ok_or_else(|| anyhow!("Expected timestamp string for {}", arg.name))?;
                let dt = chrono::DateTime::parse_from_rfc3339(s)?;
                Ok(SqlValue::Timestamp(dt.with_timezone(&chrono::Utc)))
            }
            ArgType::StringList => {
                let arr = value.as_array()
                    .ok_or_else(|| anyhow!("Expected array for {}", arg.name))?;
                let strings: Vec<String> = arr.iter()
                    .filter_map(|v| v.as_str().map(|s| s.to_string()))
                    .collect();
                Ok(SqlValue::StringArray(strings))
            }
        }
    }

    /// Execute query returning single row
    async fn execute_with_bindings(&self, sql: &str, values: &[SqlValue]) -> Result<PgRow> {
        let mut query = sqlx::query(sql);
        for val in values {
            query = self.bind_sql_value(query, val);
        }
        let row = query.fetch_one(&self.pool).await?;
        Ok(row)
    }

    /// Execute query returning multiple rows
    async fn execute_many_with_bindings(&self, sql: &str, values: &[SqlValue]) -> Result<Vec<PgRow>> {
        let mut query = sqlx::query(sql);
        for val in values {
            query = self.bind_sql_value(query, val);
        }
        let rows = query.fetch_all(&self.pool).await?;
        Ok(rows)
    }

    /// Execute non-query (INSERT/UPDATE/DELETE without RETURNING)
    async fn execute_non_query(&self, sql: &str, values: &[SqlValue]) -> Result<u64> {
        let mut query = sqlx::query(sql);
        for val in values {
            query = self.bind_sql_value(query, val);
        }
        let result = query.execute(&self.pool).await?;
        Ok(result.rows_affected())
    }

    /// Bind a SqlValue to a query
    fn bind_sql_value<'q>(
        &self,
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
                "UUID" => row.try_get::<Option<Uuid>, _>(name)
                    .ok().flatten()
                    .map(|u| json!(u.to_string())),
                "TEXT" | "VARCHAR" | "CHAR" | "NAME" => row.try_get::<Option<String>, _>(name)
                    .ok().flatten()
                    .map(|s| json!(s)),
                "INT4" => row.try_get::<Option<i32>, _>(name)
                    .ok().flatten()
                    .map(|i| json!(i)),
                "INT8" => row.try_get::<Option<i64>, _>(name)
                    .ok().flatten()
                    .map(|i| json!(i)),
                "INT2" => row.try_get::<Option<i16>, _>(name)
                    .ok().flatten()
                    .map(|i| json!(i)),
                "FLOAT4" | "FLOAT8" => row.try_get::<Option<f64>, _>(name)
                    .ok().flatten()
                    .map(|f| json!(f)),
                "NUMERIC" => row.try_get::<Option<rust_decimal::Decimal>, _>(name)
                    .ok().flatten()
                    .map(|d| json!(d.to_string())),
                "BOOL" => row.try_get::<Option<bool>, _>(name)
                    .ok().flatten()
                    .map(|b| json!(b)),
                "JSONB" | "JSON" => row.try_get::<Option<JsonValue>, _>(name)
                    .ok().flatten(),
                "TIMESTAMPTZ" | "TIMESTAMP" => row.try_get::<Option<chrono::DateTime<chrono::Utc>>, _>(name)
                    .ok().flatten()
                    .map(|dt| json!(dt.to_rfc3339())),
                "DATE" => row.try_get::<Option<chrono::NaiveDate>, _>(name)
                    .ok().flatten()
                    .map(|d| json!(d.to_string())),
                _ => None,
            };

            map.insert(name.to_string(), value.unwrap_or(JsonValue::Null));
        }

        Ok(JsonValue::Object(map))
    }
}

// =============================================================================
// SQL VALUE TYPE
// =============================================================================

/// Internal SQL value representation for dynamic binding
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
    Null,
}

// =============================================================================
// TESTS
// =============================================================================

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_infer_pk_column() {
        assert_eq!(GenericCrudExecutor::infer_pk_column("cbus"), "cbu_id");
        assert_eq!(GenericCrudExecutor::infer_pk_column("entities"), "entity_id");
        assert_eq!(GenericCrudExecutor::infer_pk_column("products"), "product_id");
        assert_eq!(GenericCrudExecutor::infer_pk_column("cbu_entity_roles"), "cbu_entity_role_id");
    }
}
```

---

## Part 2: Integration with Existing Executor

### Modify `rust/src/dsl_v2/executor.rs`

Add a method to route YAML-defined verbs through GenericCrudExecutor:

```rust
// Add at top of file:
use super::generic_executor::{GenericCrudExecutor, GenericExecutionResult};
use super::runtime_registry::{RuntimeVerbRegistry, RuntimeBehavior};

// Add to DslExecutor struct:
#[cfg(feature = "database")]
pub struct DslExecutor {
    pool: PgPool,
    custom_ops: CustomOperationRegistry,
    generic_executor: GenericCrudExecutor,  // ADD THIS
}

// Modify DslExecutor::new():
#[cfg(feature = "database")]
pub fn new(pool: PgPool) -> Self {
    Self {
        generic_executor: GenericCrudExecutor::new(pool.clone()),  // ADD THIS
        pool,
        custom_ops: CustomOperationRegistry::new(),
    }
}

// Add new method to DslExecutor impl:
/// Execute a verb using the YAML-driven generic executor
///
/// This is the new path for verbs defined in verbs.yaml
#[cfg(feature = "database")]
pub async fn execute_verb_generic(
    &self,
    verb: &RuntimeVerb,
    args: &HashMap<String, JsonValue>,
    ctx: &mut ExecutionContext,
) -> Result<ExecutionResult> {
    // Check if this is a plugin (custom op)
    if let RuntimeBehavior::Plugin(handler) = &verb.behavior {
        // Route to custom ops
        if let Some(op) = self.custom_ops.get(&verb.domain, &verb.verb) {
            // Convert JsonValue args to VerbCall for custom ops compatibility
            // (This is a bridge - eventually custom ops should accept JsonValue directly)
            todo!("Bridge to custom ops with handler: {}", handler);
        }
        return Err(anyhow!("Plugin {} has no handler", handler));
    }

    // Execute via generic executor
    let result = self.generic_executor.execute(verb, args).await?;

    // Handle symbol capture
    if verb.returns.capture {
        if let GenericExecutionResult::Uuid(uuid) = &result {
            if let Some(name) = &verb.returns.name {
                ctx.bind(name, *uuid);
            }
        }
    }

    Ok(result.to_legacy())
}
```

---

## Part 3: Update `mod.rs` Exports

### File: `rust/src/dsl_v2/mod.rs`

Add the generic_executor module:

```rust
// Add to module declarations:
#[cfg(feature = "database")]
pub mod generic_executor;

// Add to re-exports:
#[cfg(feature = "database")]
pub use generic_executor::{GenericCrudExecutor, GenericExecutionResult};
```

---

## Part 4: Dead Code to Remove (After Validation)

Once the generic executor is working and tested, the following code becomes dead:

### 4.1 `verbs.rs` - ENTIRE FILE (1,299 lines)

The entire file can be removed:
- `Behavior` enum - Replaced by `CrudOperation` in config/types.rs
- `VerbDef` struct - Replaced by `RuntimeVerb` in runtime_registry.rs
- `STANDARD_VERBS` static array - Replaced by `config/verbs.yaml`
- `find_verb()` function - Replaced by `RuntimeVerbRegistry::get()`
- `domains()`, `verb_count()`, `verbs_for_domain()` - Replaced by registry methods

**Action**: Delete `rust/src/dsl_v2/verbs.rs` entirely

### 4.2 `mappings.rs` - ENTIRE FILE (1,424 lines)

All static column mappings can be removed:
- `ColumnMapping` struct - Replaced by `maps_to` in YAML arg definitions
- `TableMappings` struct - Replaced by `table` in YAML crud config
- `ENTITIES_MAPPINGS`, `LIMITED_COMPANIES_MAPPINGS`, etc. - All in YAML
- `resolve_column()` function - Not needed, mappings are per-verb in YAML
- `get_pk_column()` function - Replaced by `returning` in YAML or inferred
- `get_table_mappings()` function - Not needed

**Action**: Delete `rust/src/dsl_v2/mappings.rs` entirely

### 4.3 `verb_registry.rs` - PARTIAL REMOVAL (~600 lines)

The following can be removed after migration:
- `custom_ops_definitions()` function - Plugins now in YAML `plugins:` section
- `infer_arg_type()` function - Arg types explicit in YAML
- The `build()` method's CRUD verb loading from `STANDARD_VERBS`

**Keep**:
- `UnifiedVerbRegistry` struct (or rename to just `VerbRegistry`)
- `VerbBehavior` enum (Crud, CustomOp, Composite)
- `ArgDef` struct (still useful)
- `UnifiedVerbDef` struct (still useful)
- `registry()` global accessor

**Action**: Refactor to load from `RuntimeVerbRegistry` instead of static arrays

### 4.4 `executor.rs` - PARTIAL REMOVAL (~800 lines)

The following execute_* methods become dead:
- `execute_insert()` - Replaced by `generic_executor::execute_insert()`
- `execute_select()` - Replaced by `generic_executor::execute_select()`
- `execute_update()` - Replaced by `generic_executor::execute_update()`
- `execute_delete()` - Replaced by `generic_executor::execute_delete()`
- `execute_upsert()` - Replaced by `generic_executor::execute_upsert()`
- `execute_link()` - Replaced by `generic_executor::execute_link()`
- `execute_unlink()` - Replaced by `generic_executor::execute_unlink()`
- `execute_list_by_fk()` - Replaced by `generic_executor::execute_list_by_fk()`
- `execute_select_with_join()` - Replaced by `generic_executor::execute_select_with_join()`
- `execute_entity_create()` - Replaced by `generic_executor::execute_entity_create()`
- `execute_role_link()` - Replaced by `generic_executor::execute_role_link()`
- `execute_role_unlink()` - Replaced by `generic_executor::execute_role_unlink()`
- `execute_list_parties()` - Replaced by `generic_executor::execute_list_parties()`

**Keep**:
- `ExecutionContext` struct
- `ExecutionResult` enum
- `ReturnType` enum
- `ResolvedValue` enum
- `resolve_args()`, `resolve_value()` methods
- `validate_args()` method (modify to use RuntimeVerb)
- `execute_plan()` method (modify to use generic executor)
- `execute_dsl()` method
- `BindValue` enum and bind helpers (or move to generic_executor)
- `row_to_json()` helper (or move to generic_executor)

**Action**: Remove execute_* methods, update remaining methods to use generic executor

---

## Part 5: Migration Checklist

### Phase 1: Create generic_executor.rs
- [ ] Create file with all 13 operation implementations
- [ ] Add module export in mod.rs
- [ ] Run `cargo build` - verify compilation

### Phase 2: Integration Test
- [ ] Add `execute_verb_generic()` to DslExecutor
- [ ] Write test that loads YAML config
- [ ] Write test that executes `cbu.create` via generic executor
- [ ] Write test that executes `entity.create-limited-company` via generic executor
- [ ] Compare results with existing executor

### Phase 3: Parallel Running
- [ ] Add feature flag `yaml_verbs` 
- [ ] Route verbs through generic executor when flag enabled
- [ ] Run existing test suite with flag off (should pass)
- [ ] Run existing test suite with flag on (should pass)

### Phase 4: Cleanup
- [ ] Delete `verbs.rs`
- [ ] Delete `mappings.rs`
- [ ] Refactor `verb_registry.rs` to use RuntimeVerbRegistry
- [ ] Remove old execute_* methods from `executor.rs`
- [ ] Update all imports/references
- [ ] Run full test suite

---

## Summary

### Files to Create
| File | Lines | Purpose |
|------|-------|---------|
| `generic_executor.rs` | ~900 | Execute verbs from YAML config |

### Files to Modify
| File | Change |
|------|--------|
| `executor.rs` | Add `execute_verb_generic()`, add GenericCrudExecutor field |
| `mod.rs` | Add generic_executor export |

### Files to Delete (After Migration)
| File | Lines | Reason |
|------|-------|--------|
| `verbs.rs` | 1,299 | Replaced by `config/verbs.yaml` |
| `mappings.rs` | 1,424 | Replaced by `maps_to` in YAML |

### Files to Refactor
| File | Lines to Remove | Reason |
|------|-----------------|--------|
| `verb_registry.rs` | ~600 | Load from RuntimeVerbRegistry instead |
| `executor.rs` | ~800 | Remove old execute_* methods |

### Net Result
- **Remove**: ~4,100 lines of static Rust definitions
- **Add**: ~900 lines of generic executor
- **Net**: ~3,200 lines removed
- **Benefit**: All verb definitions in YAML, no recompilation to add verbs
