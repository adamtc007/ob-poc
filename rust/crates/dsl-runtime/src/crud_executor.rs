//! CrudExecutionPort implementation — metadata-driven SQL execution.
//!
//! Executes CRUD verbs using `VerbContractBody` metadata (table, schema,
//! operation, column mappings). No hand-coded Rust per verb.
//!
//! This is the data-plane interpreter that replaces per-verb Rust impls
//! for the CRUD portion of `domain_ops`. Unsupported operations return
//! `SemOsError::InvalidInput` until migrated.
//!
//! Phase 3 note (three-plane architecture v0.3 §13): this module was
//! relocated here from `sem_os_postgres::crud_executor` because CRUD
//! interpretation is a data-plane concern. `sem_os_postgres` retains
//! only metadata-loading code.

use async_trait::async_trait;
use sqlx::postgres::PgRow;
use sqlx::{Column, PgPool, Row, TypeInfo};
use tracing::debug;
use uuid::Uuid;

use crate::{CrudExecutionPort, VerbExecutionContext, VerbExecutionOutcome};
use sem_os_core::error::SemOsError;
use sem_os_ontology::verb_contract::{VerbArgDef, VerbContractBody, VerbCrudMapping};

/// SemOS-native CRUD executor backed by PostgreSQL.
pub struct PgCrudExecutor {
    pool: PgPool,
}

impl PgCrudExecutor {
    pub fn new(pool: PgPool) -> Self {
        Self { pool }
    }
}

/// T9.2 (§3 Branch 2): the one executor abstraction shared by the pool-based
/// `execute_crud` and the scope-based `execute_crud_in_scope` — every
/// per-operation method below takes `exec: &mut CrudExec<'_>` instead of
/// reading `self.pool` directly, so the same dispatch logic runs atomically
/// inside a caller-supplied `PgTransactionScope` when one is provided,
/// without duplicating the ~14 operation methods.
///
/// `Pool(&PgPool)` is `Copy`-cheap to re-borrow every helper call.
/// `Scope(&mut dyn TransactionScope)` is reborrowed (`.executor()`) per
/// call, the same pattern established elsewhere in this crate —
/// sequential statements against the same scope compose without fighting
/// the borrow checker.
///
/// T10.3 (EOP-PLAN-CONTROLPLANE-001 Addendum C): widened from
/// `Conn(&mut sqlx::PgConnection)` to `Scope(&mut dyn TransactionScope)`
/// so write-capture (`record_write`) is reachable — SQL dispatch is
/// otherwise unchanged (`scope.executor()` yields the same
/// `&mut PgConnection` the old `Conn` variant held directly).
pub(crate) enum CrudExec<'e> {
    Pool(&'e PgPool),
    Scope(&'e mut dyn crate::TransactionScope),
}

impl CrudExec<'_> {
    /// T10.3: self-report a write for G14's write-set attestation.
    /// No-op for `Pool` — the pool-based `execute_crud` fast path never
    /// runs inside a caller's admitting scope (pre-T9.2 legacy path), so
    /// there is nothing honest to attest against; only `Scope` dispatches
    /// (T9.2's atomic-admission branch) participate in capture.
    fn record_write(
        &mut self,
        table: &str,
        entity_id: Uuid,
        columns: &[String],
        created_new_entity: bool,
    ) {
        if let CrudExec::Scope(s) = self {
            s.record_write(table, entity_id, columns, created_new_entity);
        }
    }
}

#[async_trait]
impl CrudExecutionPort for PgCrudExecutor {
    async fn execute_crud(
        &self,
        contract: &VerbContractBody,
        args: serde_json::Value,
        _ctx: &VerbExecutionContext,
    ) -> crate::Result<VerbExecutionOutcome> {
        let mut exec = CrudExec::Pool(&self.pool);
        self.dispatch(contract, args, &mut exec).await
    }

    async fn execute_crud_in_scope(
        &self,
        contract: &VerbContractBody,
        args: serde_json::Value,
        _ctx: &VerbExecutionContext,
        scope: &mut dyn crate::TransactionScope,
    ) -> crate::Result<VerbExecutionOutcome> {
        let mut exec = CrudExec::Scope(scope);
        self.dispatch(contract, args, &mut exec).await
    }
}

impl PgCrudExecutor {
    async fn dispatch(
        &self,
        contract: &VerbContractBody,
        args: serde_json::Value,
        exec: &mut CrudExec<'_>,
    ) -> crate::Result<VerbExecutionOutcome> {
        let crud = contract.crud_mapping.as_ref().ok_or_else(|| {
            SemOsError::InvalidInput(format!("Verb {} has no crud_mapping", contract.fqn))
        })?;

        let table = crud.table.as_deref().ok_or_else(|| {
            SemOsError::InvalidInput(format!("Verb {} crud_mapping has no table", contract.fqn))
        })?;
        let schema = crud.schema.as_deref().unwrap_or("ob-poc");

        match crud.operation.as_str() {
            "select" => {
                self.execute_select(exec, schema, table, &contract.args, &args, &contract.returns)
                    .await
            }
            "insert" => {
                self.execute_insert(exec, schema, table, crud, &contract.args, &args)
                    .await
            }
            "update" => {
                self.execute_update(exec, schema, table, crud, &contract.args, &args)
                    .await
            }
            "delete" => {
                self.execute_delete(exec, schema, table, crud, &contract.args, &args)
                    .await
            }
            "upsert" => {
                self.execute_upsert(exec, schema, table, crud, &contract.args, &args)
                    .await
            }
            "link" => {
                self.execute_link(exec, schema, table, crud, &contract.args, &args)
                    .await
            }
            "unlink" => {
                self.execute_unlink(exec, schema, table, crud, &contract.args, &args)
                    .await
            }
            "role_link" => {
                self.execute_role_link(exec, schema, crud, &contract.args, &args)
                    .await
            }
            "role_unlink" => {
                self.execute_role_unlink(exec, schema, crud, &contract.args, &args)
                    .await
            }
            "list_by_fk" => {
                self.execute_list_by_fk(exec, schema, table, crud, &contract.args, &args)
                    .await
            }
            "list_parties" => {
                self.execute_list_parties(exec, schema, crud, &contract.args, &args)
                    .await
            }
            "select_with_join" => {
                self.execute_select_with_join(exec, schema, crud, &contract.args, &args)
                    .await
            }
            "entitycreate" | "entity_create" => {
                self.execute_entity_create(exec, schema, table, crud, &contract.args, &args)
                    .await
            }
            "entityupsert" | "entity_upsert" => {
                self.execute_entity_upsert(exec, schema, table, crud, &contract.args, &args)
                    .await
            }
            op => Err(SemOsError::InvalidInput(format!(
                "CRUD operation '{}' not yet migrated to CrudExecutionPort (verb: {})",
                op, contract.fqn
            ))),
        }
    }

    // ── SELECT ──────────────────────────────────────────────────

    async fn execute_select(
        &self,
        exec: &mut CrudExec<'_>,
        schema: &str,
        table: &str,
        arg_defs: &[VerbArgDef],
        args: &serde_json::Value,
        returns: &Option<sem_os_ontology::verb_contract::VerbReturnSpec>,
    ) -> crate::Result<VerbExecutionOutcome> {
        let args_map = args.as_object().cloned().unwrap_or_default();
        let mut conditions = Vec::new();
        let mut bind_values: Vec<SqlValue> = Vec::new();
        let mut idx = 1;
        let mut limit: Option<i64> = None;
        let mut offset: Option<i64> = None;

        for arg_def in arg_defs {
            if let Some(value) = args_map.get(&arg_def.name) {
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
                    bind_values.push(json_to_sql_value(value, &arg_def.arg_type, &arg_def.name)?);
                    idx += 1;
                }
            }
        }

        let where_clause = if conditions.is_empty() {
            soft_delete_predicate(schema, table)
                .map(|p| format!(" WHERE {p}"))
                .unwrap_or_default()
        } else {
            if let Some(predicate) = soft_delete_predicate(schema, table) {
                conditions.push(predicate);
            }
            format!(" WHERE {}", conditions.join(" AND "))
        };

        let limit_clause = limit.map(|l| format!(" LIMIT {l}")).unwrap_or_default();
        let offset_clause = offset.map(|o| format!(" OFFSET {o}")).unwrap_or_default();

        let sql = format!(
            "SELECT * FROM \"{schema}\".\"{table}\"{where_clause}{limit_clause}{offset_clause}"
        );

        debug!(sql = %sql, binds = bind_values.len(), "CrudExecutionPort SELECT");

        let rows = execute_query(exec, &sql, &bind_values).await?;

        // Single record vs record set based on return type
        let is_single = returns
            .as_ref()
            .map(|r| r.return_type == "record")
            .unwrap_or(false);

        if is_single {
            if rows.is_empty() {
                Ok(VerbExecutionOutcome::Record(serde_json::Value::Null))
            } else {
                Ok(VerbExecutionOutcome::Record(row_to_json(&rows[0])?))
            }
        } else {
            let records: Result<Vec<serde_json::Value>, _> = rows.iter().map(row_to_json).collect();
            Ok(VerbExecutionOutcome::RecordSet(records?))
        }
    }

    // ── INSERT ───────────────────────────────────────────────────

    async fn execute_insert(
        &self,
        exec: &mut CrudExec<'_>,
        schema: &str,
        table: &str,
        crud: &VerbCrudMapping,
        arg_defs: &[VerbArgDef],
        args: &serde_json::Value,
    ) -> crate::Result<VerbExecutionOutcome> {
        let args_map = args.as_object().cloned().unwrap_or_default();

        let pk_col = crud
            .returning
            .as_deref()
            .unwrap_or_else(|| infer_pk_column(table));

        let new_id = Uuid::new_v4();
        let mut columns = vec![format!("\"{}\"", pk_col)];
        let mut raw_columns = vec![pk_col.to_string()];
        let mut placeholders = vec!["$1".to_string()];
        let mut bind_values: Vec<SqlValue> = vec![SqlValue::Uuid(new_id)];
        let mut idx = 2;

        for arg_def in arg_defs {
            if let Some(value) = args_map.get(&arg_def.name) {
                if let Some(col) = &arg_def.maps_to {
                    if col == pk_col {
                        continue;
                    }
                    columns.push(format!("\"{}\"", col));
                    raw_columns.push(col.clone());
                    placeholders.push(format!("${idx}"));
                    bind_values.push(json_to_sql_value(value, &arg_def.arg_type, &arg_def.name)?);
                    idx += 1;
                }
            }
        }

        let returning = crud.returning.as_deref().unwrap_or(pk_col);

        let sql = if !crud.conflict_keys.is_empty() {
            let conflict_cols: Vec<String> = crud
                .conflict_keys
                .iter()
                .map(|c| format!("\"{}\"", c))
                .collect();
            format!(
                r#"INSERT INTO "{schema}"."{table}" ({cols}) VALUES ({vals}) ON CONFLICT ({conflict}) DO UPDATE SET "{pk}" = "{table}"."{pk}" RETURNING "{ret}""#,
                cols = columns.join(", "),
                vals = placeholders.join(", "),
                conflict = conflict_cols.join(", "),
                pk = pk_col,
                ret = returning,
            )
        } else {
            format!(
                r#"WITH ins AS (
                    INSERT INTO "{schema}"."{table}" ({cols}) VALUES ({vals})
                    ON CONFLICT DO NOTHING RETURNING "{ret}"
                ) SELECT "{ret}" FROM ins
                  UNION ALL SELECT "{ret}" FROM "{schema}"."{table}"
                  WHERE NOT EXISTS (SELECT 1 FROM ins) LIMIT 1"#,
                cols = columns.join(", "),
                vals = placeholders.join(", "),
                ret = returning,
            )
        };

        debug!(sql = %sql, binds = bind_values.len(), "CrudExecutionPort INSERT");

        let row = execute_query_one(exec, &sql, &bind_values).await?;
        let uuid: Uuid = row.try_get(returning).map_err(|e| {
            SemOsError::Internal(anyhow::anyhow!("Failed to extract {returning}: {e}"))
        })?;
        // T10.3: self-report for G14. Recorded unconditionally on success,
        // including the idempotent-no-op branch (ON CONFLICT DO NOTHING
        // fell through to the existing row) — a slight over-report, never
        // an under-report, and harmless to attestation since it's the
        // same row/table this operation already declares it targets.
        exec.record_write(&format!("{schema}.{table}"), uuid, &raw_columns, true);
        Ok(VerbExecutionOutcome::Uuid(uuid))
    }

    // ── UPDATE ──────────────────────────────────────────────────

    async fn execute_update(
        &self,
        exec: &mut CrudExec<'_>,
        schema: &str,
        table: &str,
        crud: &VerbCrudMapping,
        arg_defs: &[VerbArgDef],
        args: &serde_json::Value,
    ) -> crate::Result<VerbExecutionOutcome> {
        let args_map = args.as_object().cloned().unwrap_or_default();
        let key_col = crud.key_column.as_deref().ok_or_else(|| {
            SemOsError::InvalidInput("Update requires key_column in crud_mapping".into())
        })?;

        let mut sets = Vec::new();
        let mut raw_columns = Vec::new();
        let mut bind_values: Vec<SqlValue> = Vec::new();
        let mut key_value: Option<SqlValue> = None;
        let mut idx = 1;

        for arg_def in arg_defs {
            if let Some(value) = args_map.get(&arg_def.name) {
                if let Some(col) = &arg_def.maps_to {
                    if col == key_col {
                        key_value =
                            Some(json_to_sql_value(value, &arg_def.arg_type, &arg_def.name)?);
                    } else {
                        sets.push(format!("\"{}\" = ${}", col, idx));
                        raw_columns.push(col.clone());
                        bind_values.push(json_to_sql_value(
                            value,
                            &arg_def.arg_type,
                            &arg_def.name,
                        )?);
                        idx += 1;
                    }
                }
            }
        }

        let key_val = key_value
            .ok_or_else(|| SemOsError::InvalidInput("Missing key argument for update".into()))?;

        if sets.is_empty() {
            return Err(SemOsError::InvalidInput("No columns to update".into()));
        }

        let mut sql = format!(
            r#"UPDATE "{schema}"."{table}" SET {} WHERE "{key_col}" = ${idx}"#,
            sets.join(", "),
        );
        if let Some(predicate) = soft_delete_predicate(schema, table) {
            sql = format!("{sql} AND {predicate}");
        }

        debug!(sql = %sql, binds = bind_values.len() + 1, "CrudExecutionPort UPDATE");

        // T10.3: capture requires a real entity id to attest against — only
        // a UUID key qualifies (matches CapturedWrite's own type). A
        // non-UUID key (rare; most tables in this schema use UUID PKs per
        // CLAUDE.md's own PostgreSQL<->Rust type table) means this write
        // goes unrecorded — an honest coverage gap, not a fabricated one.
        let key_entity_id = if let SqlValue::Uuid(id) = &key_val {
            Some(*id)
        } else {
            None
        };

        bind_values.push(key_val);
        let affected = execute_non_query(exec, &sql, &bind_values).await?;
        if affected > 0 {
            if let Some(entity_id) = key_entity_id {
                exec.record_write(&format!("{schema}.{table}"), entity_id, &raw_columns, false);
            }
        }
        Ok(VerbExecutionOutcome::Affected(affected))
    }

    // ── DELETE ───────────────────────────────────────────────────

    async fn execute_delete(
        &self,
        exec: &mut CrudExec<'_>,
        schema: &str,
        table: &str,
        crud: &VerbCrudMapping,
        arg_defs: &[VerbArgDef],
        args: &serde_json::Value,
    ) -> crate::Result<VerbExecutionOutcome> {
        let args_map = args.as_object().cloned().unwrap_or_default();
        let is_soft = soft_delete_predicate(schema, table).is_some();

        let mut conditions = Vec::new();
        let mut bind_values: Vec<SqlValue> = Vec::new();
        let mut idx = 1;
        // T10.3: only the single-key_column path has an unambiguous entity
        // id to attest against — the multi-condition else-branch below is
        // an honest coverage gap (PARTIAL), not a fabricated capture.
        let mut capture_entity_id: Option<Uuid> = None;

        // Use key_column if specified, otherwise all maps_to columns
        if let Some(key_col) = crud.key_column.as_deref() {
            let key_arg = arg_defs
                .iter()
                .find(|a| a.maps_to.as_deref() == Some(key_col))
                .ok_or_else(|| SemOsError::InvalidInput("Key arg not found".into()))?;
            let value = args_map.get(&key_arg.name).ok_or_else(|| {
                SemOsError::InvalidInput(format!("Missing key arg: {}", key_arg.name))
            })?;
            let key_val = json_to_sql_value(value, &key_arg.arg_type, &key_arg.name)?;
            if let SqlValue::Uuid(id) = &key_val {
                capture_entity_id = Some(*id);
            }
            conditions.push(format!("\"{key_col}\" = $1"));
            bind_values.push(key_val);
        } else {
            for arg_def in arg_defs {
                if let Some(col) = &arg_def.maps_to {
                    if let Some(value) = args_map.get(&arg_def.name) {
                        conditions.push(format!("\"{}\" = ${}", col, idx));
                        bind_values.push(json_to_sql_value(
                            value,
                            &arg_def.arg_type,
                            &arg_def.name,
                        )?);
                        idx += 1;
                    }
                }
            }
        }

        if conditions.is_empty() {
            return Err(SemOsError::InvalidInput(
                "Delete requires at least one WHERE condition".into(),
            ));
        }

        if is_soft {
            conditions.push("deleted_at IS NULL".to_string());
        }

        let sql = if is_soft {
            format!(
                r#"UPDATE "{schema}"."{table}" SET deleted_at = NOW() WHERE {}"#,
                conditions.join(" AND ")
            )
        } else {
            format!(
                r#"DELETE FROM "{schema}"."{table}" WHERE {}"#,
                conditions.join(" AND ")
            )
        };

        debug!(sql = %sql, binds = bind_values.len(), "CrudExecutionPort DELETE");

        let affected = execute_non_query(exec, &sql, &bind_values).await?;
        if affected > 0 {
            if let Some(entity_id) = capture_entity_id {
                // Soft delete writes `deleted_at`; hard delete removes the
                // row entirely — an empty column list makes no per-column
                // claim (vacuously within any declared bound), only the
                // table+entity assertion applies.
                let columns: Vec<String> =
                    if is_soft { vec!["deleted_at".to_string()] } else { vec![] };
                exec.record_write(&format!("{schema}.{table}"), entity_id, &columns, false);
            }
        }
        Ok(VerbExecutionOutcome::Affected(affected))
    }

    // ── UPSERT ──────────────────────────────────────────────────

    async fn execute_upsert(
        &self,
        exec: &mut CrudExec<'_>,
        schema: &str,
        table: &str,
        crud: &VerbCrudMapping,
        arg_defs: &[VerbArgDef],
        args: &serde_json::Value,
    ) -> crate::Result<VerbExecutionOutcome> {
        let args_map = args.as_object().cloned().unwrap_or_default();

        let pk_col = crud
            .returning
            .as_deref()
            .unwrap_or_else(|| infer_pk_column(table));
        let new_id = Uuid::new_v4();
        let mut columns = vec![format!("\"{}\"", pk_col)];
        let mut placeholders = vec!["$1".to_string()];
        let mut bind_values: Vec<SqlValue> = vec![SqlValue::Uuid(new_id)];
        let mut insert_cols: Vec<String> = vec![pk_col.to_string()];
        let mut idx = 2;

        for arg_def in arg_defs {
            if let Some(value) = args_map.get(&arg_def.name) {
                if let Some(col) = &arg_def.maps_to {
                    if col == pk_col {
                        continue;
                    }
                    columns.push(format!("\"{}\"", col));
                    placeholders.push(format!("${idx}"));
                    bind_values.push(json_to_sql_value(value, &arg_def.arg_type, &arg_def.name)?);
                    insert_cols.push(col.clone());
                    idx += 1;
                }
            }
        }

        let conflict_cols = if !crud.conflict_keys.is_empty() {
            crud.conflict_keys
                .iter()
                .map(|c| format!("\"{}\"", c))
                .collect::<Vec<_>>()
                .join(", ")
        } else {
            return Err(SemOsError::InvalidInput(
                "Upsert requires conflict_keys".into(),
            ));
        };

        let updates: Vec<String> = insert_cols
            .iter()
            .filter(|c| !crud.conflict_keys.contains(c) && *c != pk_col)
            .map(|c| format!("\"{}\" = EXCLUDED.\"{}\"", c, c))
            .collect();

        let update_clause = if updates.is_empty() {
            format!("\"{}\" = \"{}\".\"{}\"", pk_col, table, pk_col)
        } else {
            updates.join(", ")
        };

        let returning = crud.returning.as_deref().unwrap_or(pk_col);
        let sql = format!(
            r#"INSERT INTO "{schema}"."{table}" ({cols}) VALUES ({vals}) ON CONFLICT ({conflict}) DO UPDATE SET {update} RETURNING "{ret}""#,
            cols = columns.join(", "),
            vals = placeholders.join(", "),
            conflict = conflict_cols,
            update = update_clause,
            ret = returning,
        );

        debug!(sql = %sql, binds = bind_values.len(), "CrudExecutionPort UPSERT");

        let row = execute_query_one(exec, &sql, &bind_values).await?;
        let uuid: Uuid = row.try_get(returning).map_err(|e| {
            SemOsError::Internal(anyhow::anyhow!("Failed to extract {returning}: {e}"))
        })?;
        // T10.3: self-report for G14 — same posture as execute_insert.
        exec.record_write(&format!("{schema}.{table}"), uuid, &insert_cols, true);
        Ok(VerbExecutionOutcome::Uuid(uuid))
    }

    // ── LINK ────────────────────────────────────────────────────

    async fn execute_link(
        &self,
        exec: &mut CrudExec<'_>,
        schema: &str,
        _table: &str,
        crud: &VerbCrudMapping,
        arg_defs: &[VerbArgDef],
        args: &serde_json::Value,
    ) -> crate::Result<VerbExecutionOutcome> {
        let args_map = args.as_object().cloned().unwrap_or_default();
        let junction = crud
            .junction
            .as_deref()
            .ok_or_else(|| SemOsError::InvalidInput("Link requires junction table".into()))?;
        let from_col = crud
            .from_col
            .as_deref()
            .ok_or_else(|| SemOsError::InvalidInput("Link requires from_col".into()))?;
        let to_col = crud
            .to_col
            .as_deref()
            .ok_or_else(|| SemOsError::InvalidInput("Link requires to_col".into()))?;

        let mut from_val = None;
        let mut to_val = None;
        for arg_def in arg_defs {
            if let Some(value) = args_map.get(&arg_def.name) {
                if arg_def.maps_to.as_deref() == Some(from_col) {
                    from_val = Some(json_to_sql_value(value, &arg_def.arg_type, &arg_def.name)?);
                } else if arg_def.maps_to.as_deref() == Some(to_col) {
                    to_val = Some(json_to_sql_value(value, &arg_def.arg_type, &arg_def.name)?);
                }
            }
        }

        let from =
            from_val.ok_or_else(|| SemOsError::InvalidInput(format!("Missing {from_col}")))?;
        let to = to_val.ok_or_else(|| SemOsError::InvalidInput(format!("Missing {to_col}")))?;

        let sql = format!(
            r#"INSERT INTO "{schema}"."{junction}" ("{from_col}", "{to_col}") VALUES ($1, $2) ON CONFLICT DO NOTHING"#,
        );

        debug!(sql = %sql, "CrudExecutionPort LINK");

        // T10.3: capture entity ids before `from`/`to` are moved into the
        // bind slice below.
        let from_id = if let SqlValue::Uuid(id) = &from { Some(*id) } else { None };
        let to_id = if let SqlValue::Uuid(id) = &to { Some(*id) } else { None };

        let affected = execute_non_query(exec, &sql, &[from, to]).await?;
        if affected > 0 {
            // T10.3: self-report for G14 — only when a row was actually
            // inserted (ON CONFLICT DO NOTHING no-ops on a re-link, which
            // makes no write at all to attest against). Junction rows have
            // no own PK column here, so both known-Uuid sides are recorded
            // against the junction table, one call per side.
            let junction_table = format!("{schema}.{junction}");
            let columns = vec![from_col.to_string(), to_col.to_string()];
            if let Some(id) = from_id {
                exec.record_write(&junction_table, id, &columns, false);
            }
            if let Some(id) = to_id {
                exec.record_write(&junction_table, id, &columns, false);
            }
        }
        Ok(VerbExecutionOutcome::Affected(affected))
    }

    // ── UNLINK ──────────────────────────────────────────────────

    async fn execute_unlink(
        &self,
        exec: &mut CrudExec<'_>,
        schema: &str,
        _table: &str,
        crud: &VerbCrudMapping,
        arg_defs: &[VerbArgDef],
        args: &serde_json::Value,
    ) -> crate::Result<VerbExecutionOutcome> {
        let args_map = args.as_object().cloned().unwrap_or_default();
        let junction = crud
            .junction
            .as_deref()
            .ok_or_else(|| SemOsError::InvalidInput("Unlink requires junction table".into()))?;
        let from_col = crud
            .from_col
            .as_deref()
            .ok_or_else(|| SemOsError::InvalidInput("Unlink requires from_col".into()))?;
        let to_col = crud
            .to_col
            .as_deref()
            .ok_or_else(|| SemOsError::InvalidInput("Unlink requires to_col".into()))?;

        let mut from_val = None;
        let mut to_val = None;
        for arg_def in arg_defs {
            if let Some(value) = args_map.get(&arg_def.name) {
                if arg_def.maps_to.as_deref() == Some(from_col) {
                    from_val = Some(json_to_sql_value(value, &arg_def.arg_type, &arg_def.name)?);
                } else if arg_def.maps_to.as_deref() == Some(to_col) {
                    to_val = Some(json_to_sql_value(value, &arg_def.arg_type, &arg_def.name)?);
                }
            }
        }

        let from =
            from_val.ok_or_else(|| SemOsError::InvalidInput(format!("Missing {from_col}")))?;
        let to = to_val.ok_or_else(|| SemOsError::InvalidInput(format!("Missing {to_col}")))?;

        let sql = format!(
            r#"DELETE FROM "{schema}"."{junction}" WHERE "{from_col}" = $1 AND "{to_col}" = $2"#,
        );

        debug!(sql = %sql, "CrudExecutionPort UNLINK");

        // T10.3: capture entity ids before `from`/`to` are moved into the
        // bind slice below.
        let from_id = if let SqlValue::Uuid(id) = &from { Some(*id) } else { None };
        let to_id = if let SqlValue::Uuid(id) = &to { Some(*id) } else { None };

        let affected = execute_non_query(exec, &sql, &[from, to]).await?;
        if affected > 0 {
            // T10.3: self-report for G14 — symmetric to execute_link. Hard
            // delete of the whole junction row: an empty column list makes
            // no per-column claim (matches execute_delete's own hard-delete
            // precedent), only the table+entity assertion applies.
            let junction_table = format!("{schema}.{junction}");
            let columns: Vec<String> = vec![];
            if let Some(id) = from_id {
                exec.record_write(&junction_table, id, &columns, false);
            }
            if let Some(id) = to_id {
                exec.record_write(&junction_table, id, &columns, false);
            }
        }
        Ok(VerbExecutionOutcome::Affected(affected))
    }

    // ── ROLE_LINK ────────────────────────────────────────────────

    async fn execute_role_link(
        &self,
        exec: &mut CrudExec<'_>,
        schema: &str,
        crud: &VerbCrudMapping,
        arg_defs: &[VerbArgDef],
        args: &serde_json::Value,
    ) -> crate::Result<VerbExecutionOutcome> {
        let args_map = args.as_object().cloned().unwrap_or_default();
        let junction = crud
            .junction
            .as_deref()
            .ok_or_else(|| SemOsError::InvalidInput("role_link requires junction".into()))?;
        let from_col = crud
            .from_col
            .as_deref()
            .ok_or_else(|| SemOsError::InvalidInput("role_link requires from_col".into()))?;
        let to_col = crud
            .to_col
            .as_deref()
            .ok_or_else(|| SemOsError::InvalidInput("role_link requires to_col".into()))?;
        let role_col = crud.role_col.as_deref().unwrap_or("role_id");
        let pk_col = infer_pk_column(junction);
        let new_id = Uuid::new_v4();

        let mut from_val = None;
        let mut to_val = None;
        let mut role_val = None;
        let mut extra_cols = Vec::new();
        let mut extra_raw_cols = Vec::new();
        let mut extra_vals = Vec::new();

        for arg_def in arg_defs {
            if let Some(value) = args_map.get(&arg_def.name) {
                if arg_def.maps_to.as_deref() == Some(from_col) {
                    from_val = Some(json_to_sql_value(value, &arg_def.arg_type, &arg_def.name)?);
                } else if arg_def.maps_to.as_deref() == Some(to_col) {
                    to_val = Some(json_to_sql_value(value, &arg_def.arg_type, &arg_def.name)?);
                } else if arg_def.maps_to.as_deref() == Some(role_col) {
                    role_val = Some(json_to_sql_value(value, &arg_def.arg_type, &arg_def.name)?);
                } else if let Some(col) = &arg_def.maps_to {
                    if col != pk_col {
                        extra_cols.push(format!("\"{}\"", col));
                        extra_raw_cols.push(col.clone());
                        extra_vals.push(json_to_sql_value(
                            value,
                            &arg_def.arg_type,
                            &arg_def.name,
                        )?);
                    }
                }
            }
        }

        let from =
            from_val.ok_or_else(|| SemOsError::InvalidInput(format!("Missing {from_col}")))?;
        let to = to_val.ok_or_else(|| SemOsError::InvalidInput(format!("Missing {to_col}")))?;
        let role =
            role_val.ok_or_else(|| SemOsError::InvalidInput(format!("Missing {role_col}")))?;

        let mut columns = vec![
            format!("\"{pk_col}\""),
            format!("\"{from_col}\""),
            format!("\"{to_col}\""),
            format!("\"{role_col}\""),
        ];
        columns.extend(extra_cols);

        // T10.3: unquoted parallel vec for `record_write` — same technique
        // as `execute_insert`'s `raw_columns`.
        let mut raw_columns = vec![
            pk_col.to_string(),
            from_col.to_string(),
            to_col.to_string(),
            role_col.to_string(),
        ];
        raw_columns.extend(extra_raw_cols);

        let mut bind_values = vec![SqlValue::Uuid(new_id), from, to, role];
        bind_values.extend(extra_vals);

        let placeholders: Vec<String> = (1..=bind_values.len()).map(|i| format!("${i}")).collect();
        let returning = crud.returning.as_deref().unwrap_or(pk_col);

        let sql = format!(
            r#"WITH ins AS (
                INSERT INTO "{schema}"."{junction}" ({cols}) VALUES ({vals})
                ON CONFLICT ("{from_col}", "{to_col}", "{role_col}") DO NOTHING
                RETURNING "{returning}"
            ) SELECT "{returning}" FROM ins
              UNION ALL SELECT "{returning}" FROM "{schema}"."{junction}"
              WHERE "{from_col}" = $2 AND "{to_col}" = $3 AND "{role_col}" = $4
              AND NOT EXISTS (SELECT 1 FROM ins) LIMIT 1"#,
            cols = columns.join(", "),
            vals = placeholders.join(", "),
        );

        debug!(sql = %sql, binds = bind_values.len(), "CrudExecutionPort ROLE_LINK");

        let row = execute_query_one(exec, &sql, &bind_values).await?;
        let uuid: Uuid = row.try_get(returning).map_err(|e| {
            SemOsError::Internal(anyhow::anyhow!("Failed to extract {returning}: {e}"))
        })?;
        // T10.3: self-report for G14 — same posture as execute_insert (same
        // idempotent-INSERT-with-fallback-SELECT SQL shape, recorded
        // unconditionally on success). `uuid` is this junction row's own
        // generated PK, not `from`/`to` — role_link is the one of the six
        // ops with its own generated row identity, matching execute_insert.
        exec.record_write(&format!("{schema}.{junction}"), uuid, &raw_columns, true);
        Ok(VerbExecutionOutcome::Uuid(uuid))
    }

    // ── ROLE_UNLINK ─────────────────────────────────────────────

    async fn execute_role_unlink(
        &self,
        exec: &mut CrudExec<'_>,
        schema: &str,
        crud: &VerbCrudMapping,
        arg_defs: &[VerbArgDef],
        args: &serde_json::Value,
    ) -> crate::Result<VerbExecutionOutcome> {
        let args_map = args.as_object().cloned().unwrap_or_default();
        let junction = crud
            .junction
            .as_deref()
            .ok_or_else(|| SemOsError::InvalidInput("role_unlink requires junction".into()))?;
        let from_col = crud
            .from_col
            .as_deref()
            .ok_or_else(|| SemOsError::InvalidInput("role_unlink requires from_col".into()))?;
        let to_col = crud
            .to_col
            .as_deref()
            .ok_or_else(|| SemOsError::InvalidInput("role_unlink requires to_col".into()))?;
        let role_col = crud.role_col.as_deref().unwrap_or("role_id");

        let mut from_val = None;
        let mut to_val = None;
        let mut role_val = None;

        for arg_def in arg_defs {
            if let Some(value) = args_map.get(&arg_def.name) {
                if arg_def.maps_to.as_deref() == Some(from_col) {
                    from_val = Some(json_to_sql_value(value, &arg_def.arg_type, &arg_def.name)?);
                } else if arg_def.maps_to.as_deref() == Some(to_col) {
                    to_val = Some(json_to_sql_value(value, &arg_def.arg_type, &arg_def.name)?);
                } else if arg_def.maps_to.as_deref() == Some(role_col) {
                    role_val = Some(json_to_sql_value(value, &arg_def.arg_type, &arg_def.name)?);
                }
            }
        }

        let from =
            from_val.ok_or_else(|| SemOsError::InvalidInput(format!("Missing {from_col}")))?;
        let to = to_val.ok_or_else(|| SemOsError::InvalidInput(format!("Missing {to_col}")))?;
        let role =
            role_val.ok_or_else(|| SemOsError::InvalidInput(format!("Missing {role_col}")))?;

        let sql = format!(
            r#"DELETE FROM "{schema}"."{junction}" WHERE "{from_col}" = $1 AND "{to_col}" = $2 AND "{role_col}" = $3"#,
        );

        debug!(sql = %sql, "CrudExecutionPort ROLE_UNLINK");

        // T10.3: capture entity ids before `from`/`to`/`role` are moved
        // into the bind slice below.
        let from_id = if let SqlValue::Uuid(id) = &from { Some(*id) } else { None };
        let to_id = if let SqlValue::Uuid(id) = &to { Some(*id) } else { None };
        let role_id = if let SqlValue::Uuid(id) = &role { Some(*id) } else { None };

        let affected = execute_non_query(exec, &sql, &[from, to, role]).await?;
        if affected > 0 {
            // T10.3: self-report for G14 — hard delete of the composite-
            // keyed junction row, same empty-column-list posture as
            // execute_unlink; one call per known-Uuid side (from/to/role).
            let junction_table = format!("{schema}.{junction}");
            let columns: Vec<String> = vec![];
            if let Some(id) = from_id {
                exec.record_write(&junction_table, id, &columns, false);
            }
            if let Some(id) = to_id {
                exec.record_write(&junction_table, id, &columns, false);
            }
            if let Some(id) = role_id {
                exec.record_write(&junction_table, id, &columns, false);
            }
        }
        Ok(VerbExecutionOutcome::Affected(affected))
    }

    // ── LIST_PARTIES ────────────────────────────────────────────

    async fn execute_list_parties(
        &self,
        exec: &mut CrudExec<'_>,
        schema: &str,
        crud: &VerbCrudMapping,
        arg_defs: &[VerbArgDef],
        args: &serde_json::Value,
    ) -> crate::Result<VerbExecutionOutcome> {
        let args_map = args.as_object().cloned().unwrap_or_default();
        let junction = crud
            .junction
            .as_deref()
            .ok_or_else(|| SemOsError::InvalidInput("list_parties requires junction".into()))?;
        let fk_col = crud
            .fk_col
            .as_deref()
            .ok_or_else(|| SemOsError::InvalidInput("list_parties requires fk_col".into()))?;

        // Find FK arg value
        let fk_val = arg_defs
            .iter()
            .find(|a| a.required)
            .and_then(|a| args_map.get(&a.name))
            .and_then(|v| v.as_str())
            .and_then(|s| Uuid::parse_str(s).ok())
            .ok_or_else(|| SemOsError::InvalidInput("list_parties: missing FK value".into()))?;

        let as_of_date = args_map
            .get("as-of-date")
            .and_then(|v| v.as_str())
            .and_then(|s| chrono::NaiveDate::parse_from_str(s, "%Y-%m-%d").ok())
            .unwrap_or_else(|| chrono::Utc::now().date_naive());

        let sql = format!(
            r#"SELECT
                cer.*, e.name as entity_name,
                et.name as entity_type, r.name as role_name
            FROM "{schema}"."{junction}" cer
            JOIN "{schema}".entities e ON e.entity_id = cer.entity_id
            JOIN "{schema}".entity_types et ON et.entity_type_id = e.entity_type_id
            JOIN "{schema}".roles r ON r.role_id = cer.role_id
            WHERE cer."{fk_col}" = $1
            AND (cer.effective_from IS NULL OR cer.effective_from <= $2)
            AND (cer.effective_to IS NULL OR cer.effective_to >= $2)
            AND e.deleted_at IS NULL
            ORDER BY e.name, r.name"#,
        );

        debug!(sql = %sql, "CrudExecutionPort LIST_PARTIES");

        let bind_values = vec![SqlValue::Uuid(fk_val), SqlValue::Date(as_of_date)];
        let rows = execute_query(exec, &sql, &bind_values).await?;
        let records: Result<Vec<serde_json::Value>, _> = rows.iter().map(row_to_json).collect();
        Ok(VerbExecutionOutcome::RecordSet(records?))
    }

    // ── SELECT_WITH_JOIN ────────────────────────────────────────

    async fn execute_select_with_join(
        &self,
        exec: &mut CrudExec<'_>,
        schema: &str,
        crud: &VerbCrudMapping,
        arg_defs: &[VerbArgDef],
        args: &serde_json::Value,
    ) -> crate::Result<VerbExecutionOutcome> {
        let args_map = args.as_object().cloned().unwrap_or_default();
        let primary = crud.primary_table.as_deref().ok_or_else(|| {
            SemOsError::InvalidInput("select_with_join requires primary_table".into())
        })?;
        let join_table = crud.join_table.as_deref().ok_or_else(|| {
            SemOsError::InvalidInput("select_with_join requires join_table".into())
        })?;
        let join_col = crud
            .join_col
            .as_deref()
            .ok_or_else(|| SemOsError::InvalidInput("select_with_join requires join_col".into()))?;
        let filter_col = crud.filter_col.as_deref().ok_or_else(|| {
            SemOsError::InvalidInput("select_with_join requires filter_col".into())
        })?;

        let filter_val = arg_defs
            .iter()
            .find(|a| a.required)
            .and_then(|a| args_map.get(&a.name))
            .ok_or_else(|| {
                SemOsError::InvalidInput("select_with_join: missing filter arg".into())
            })?;

        let filter_arg = arg_defs.iter().find(|a| a.required).unwrap();
        let sql_val = json_to_sql_value(filter_val, &filter_arg.arg_type, &filter_arg.name)?;

        let mut sql = format!(
            r#"SELECT p.* FROM "{schema}"."{primary}" p
               JOIN "{schema}"."{join_table}" j ON p."{join_col}" = j."{join_col}"
               WHERE j."{filter_col}" = $1"#,
        );
        if let Some(predicate) = soft_delete_predicate(schema, primary) {
            sql = format!("{sql} AND p.{predicate}");
        }

        debug!(sql = %sql, "CrudExecutionPort SELECT_WITH_JOIN");

        let rows = execute_query(exec, &sql, &[sql_val]).await?;
        let records: Result<Vec<serde_json::Value>, _> = rows.iter().map(row_to_json).collect();
        Ok(VerbExecutionOutcome::RecordSet(records?))
    }

    // ── LIST_BY_FK ──────────────────────────────────────────────

    async fn execute_list_by_fk(
        &self,
        exec: &mut CrudExec<'_>,
        schema: &str,
        table: &str,
        crud: &VerbCrudMapping,
        arg_defs: &[VerbArgDef],
        args: &serde_json::Value,
    ) -> crate::Result<VerbExecutionOutcome> {
        let args_map = args.as_object().cloned().unwrap_or_default();

        let fk_col = crud
            .fk_col
            .as_deref()
            .ok_or_else(|| SemOsError::InvalidInput("list_by_fk requires fk_col".into()))?;

        // Find the FK arg value
        let mut fk_value = None;
        let mut extra_conditions = Vec::new();
        let mut bind_values: Vec<SqlValue> = Vec::new();
        let mut idx = 1;

        for arg_def in arg_defs {
            if let Some(value) = args_map.get(&arg_def.name) {
                if let Some(col) = &arg_def.maps_to {
                    if col == fk_col {
                        fk_value =
                            Some(json_to_sql_value(value, &arg_def.arg_type, &arg_def.name)?);
                    } else {
                        extra_conditions.push(format!("\"{}\" = ${}", col, idx + 1));
                        bind_values.push(json_to_sql_value(
                            value,
                            &arg_def.arg_type,
                            &arg_def.name,
                        )?);
                    }
                    idx += 1;
                }
            }
        }

        let fk_val = fk_value.ok_or_else(|| {
            SemOsError::InvalidInput(format!("list_by_fk: no value for fk_col '{fk_col}'"))
        })?;

        let mut all_values = vec![fk_val];
        all_values.extend(bind_values);

        let mut where_parts = vec![format!("\"{fk_col}\" = $1")];
        where_parts.extend(extra_conditions);
        if let Some(predicate) = soft_delete_predicate(schema, table) {
            where_parts.push(predicate);
        }

        let sql = format!(
            "SELECT * FROM \"{schema}\".\"{table}\" WHERE {}",
            where_parts.join(" AND ")
        );

        debug!(sql = %sql, binds = all_values.len(), "CrudExecutionPort LIST_BY_FK");

        let rows = execute_query(exec, &sql, &all_values).await?;
        let records: Result<Vec<serde_json::Value>, _> = rows.iter().map(row_to_json).collect();
        Ok(VerbExecutionOutcome::RecordSet(records?))
    }

    // ── ENTITY_CREATE (Class Table Inheritance) ─────────────────

    async fn execute_entity_create(
        &self,
        exec: &mut CrudExec<'_>,
        schema: &str,
        _table: &str,
        crud: &VerbCrudMapping,
        arg_defs: &[VerbArgDef],
        args: &serde_json::Value,
    ) -> crate::Result<VerbExecutionOutcome> {
        let args_map = args.as_object().cloned().unwrap_or_default();

        // Resolve entity type code from args or verb name
        let type_code = resolve_entity_type_code(crud, &args_map)?;

        // Look up entity_type_id and extension table
        let type_sql = format!(
            r#"SELECT entity_type_id, table_name FROM "{schema}".entity_types
               WHERE UPPER(type_code) = UPPER($1) OR UPPER(type_code) LIKE UPPER($1) || '_%'
               ORDER BY CASE WHEN UPPER(type_code) = UPPER($1) THEN 0 ELSE 1 END
               LIMIT 1"#,
        );
        let type_row = execute_query_one(
            exec,
            &type_sql,
            &[SqlValue::String(type_code.clone())],
        )
        .await
        .map_err(|_| SemOsError::InvalidInput(format!("Entity type not found: {type_code}")))?;
        let entity_type_id: Uuid = type_row
            .try_get("entity_type_id")
            .map_err(|e| SemOsError::Internal(anyhow::anyhow!("{e}")))?;
        let extension_table: String = type_row
            .try_get("table_name")
            .map_err(|e| SemOsError::Internal(anyhow::anyhow!("{e}")))?;

        // Build entity name
        let entity_name = if type_code == "PROPER_PERSON" || type_code == "PROPER_PERSON_NATURAL" {
            let first = args_map
                .get("first-name")
                .and_then(|v| v.as_str())
                .unwrap_or("");
            let last = args_map
                .get("last-name")
                .and_then(|v| v.as_str())
                .unwrap_or("");
            format!("{first} {last}").trim().to_string()
        } else {
            args_map
                .get("name")
                .and_then(|v| v.as_str())
                .unwrap_or("Unknown")
                .to_string()
        };

        // Idempotency: check if entity already exists
        let existing_sql = format!(
            r#"SELECT entity_id FROM "{schema}".entities WHERE entity_type_id = $1 AND name = $2"#,
        );
        if let Ok(rows) = execute_query(
            exec,
            &existing_sql,
            &[
                SqlValue::Uuid(entity_type_id),
                SqlValue::String(entity_name.clone()),
            ],
        )
        .await
        {
            if let Some(row) = rows.first() {
                let existing_id: Uuid = row
                    .try_get("entity_id")
                    .map_err(|e| SemOsError::Internal(anyhow::anyhow!("{e}")))?;
                return Ok(VerbExecutionOutcome::Uuid(existing_id));
            }
        }

        // INSERT into base entities table
        let entity_id = Uuid::new_v4();
        let base_sql = format!(
            r#"INSERT INTO "{schema}".entities (entity_id, entity_type_id, name) VALUES ($1, $2, $3)"#,
        );
        execute_non_query(
            exec,
            &base_sql,
            &[
                SqlValue::Uuid(entity_id),
                SqlValue::Uuid(entity_type_id),
                SqlValue::String(entity_name.clone()),
            ],
        )
        .await?;
        // T10.3: self-report for G14 — base-table half of the CTI write.
        // The idempotent early-return above never reaches here, so this
        // line only runs when a base `entities` row was genuinely inserted.
        let base_write_cols = [
            "entity_id".to_string(),
            "entity_type_id".to_string(),
            "name".to_string(),
        ];
        exec.record_write(&format!("{schema}.entities"), entity_id, &base_write_cols, true);

        // INSERT into extension table
        let ext_pk_col = infer_pk_column(&extension_table);
        let uses_shared_pk = ext_pk_col == "entity_id";

        let (mut columns, mut placeholders, mut bind_values, mut idx) = if uses_shared_pk {
            (
                vec![format!("\"{ext_pk_col}\"")],
                vec!["$1".to_string()],
                vec![SqlValue::Uuid(entity_id)],
                2,
            )
        } else {
            (
                vec![format!("\"{ext_pk_col}\""), "\"entity_id\"".to_string()],
                vec!["$1".to_string(), "$2".to_string()],
                vec![SqlValue::Uuid(Uuid::new_v4()), SqlValue::Uuid(entity_id)],
                3,
            )
        };
        // T10.3: unquoted parallel vec for `record_write`, built in
        // lockstep with `columns` — same technique as `execute_insert`'s
        // `raw_columns`.
        let mut raw_ext_columns: Vec<String> = if uses_shared_pk {
            vec![ext_pk_col.to_string()]
        } else {
            vec![ext_pk_col.to_string(), "entity_id".to_string()]
        };

        let base_cols = ["name", "external_id"];
        for arg_def in arg_defs {
            if let Some(value) = args_map.get(&arg_def.name) {
                if arg_def.name == "entity-type" || arg_def.name == "entity-id" {
                    continue;
                }
                if let Some(col) = &arg_def.maps_to {
                    if col == ext_pk_col || col == "entity_id" || base_cols.contains(&col.as_str())
                    {
                        continue;
                    }
                    columns.push(format!("\"{col}\""));
                    raw_ext_columns.push(col.clone());
                    placeholders.push(format!("${idx}"));
                    bind_values.push(json_to_sql_value(value, &arg_def.arg_type, &arg_def.name)?);
                    idx += 1;
                }
            }
        }

        // Infer extension name column
        if let Some(name_col) = infer_extension_name_column(&extension_table) {
            let quoted = format!("\"{name_col}\"");
            if !columns.contains(&quoted) {
                if let Some(name) = args_map.get("name").and_then(|v| v.as_str()) {
                    columns.push(quoted);
                    raw_ext_columns.push(name_col.to_string());
                    placeholders.push(format!("${idx}"));
                    bind_values.push(SqlValue::String(name.to_string()));
                }
            }
        }

        let ext_sql = format!(
            r#"INSERT INTO "{schema}"."{extension_table}" ({}) VALUES ({})"#,
            columns.join(", "),
            placeholders.join(", ")
        );
        debug!(sql = %ext_sql, "CrudExecutionPort ENTITY_CREATE extension");
        execute_non_query(exec, &ext_sql, &bind_values).await?;
        // T10.3: self-report for G14 — extension-table half of the CTI
        // write. Keyed by `entity_id` (the semantic FK tying this row to
        // the entity), not `ext_pk_col`'s own surrogate value even when the
        // extension table has a separate generated PK — `entity_id` is
        // what a caller's bound-entity-id list actually contains.
        exec.record_write(
            &format!("{schema}.{extension_table}"),
            entity_id,
            &raw_ext_columns,
            true,
        );

        Ok(VerbExecutionOutcome::Uuid(entity_id))
    }

    // ── ENTITY_UPSERT (Class Table Inheritance with ON CONFLICT) ─

    async fn execute_entity_upsert(
        &self,
        exec: &mut CrudExec<'_>,
        schema: &str,
        _table: &str,
        crud: &VerbCrudMapping,
        arg_defs: &[VerbArgDef],
        args: &serde_json::Value,
    ) -> crate::Result<VerbExecutionOutcome> {
        let args_map = args.as_object().cloned().unwrap_or_default();
        let type_code = resolve_entity_type_code(crud, &args_map)?;

        // Look up entity_type_id and extension table
        let type_sql = format!(
            r#"SELECT entity_type_id, table_name FROM "{schema}".entity_types
               WHERE UPPER(type_code) = UPPER($1) OR UPPER(type_code) LIKE UPPER($1) || '_%'
               ORDER BY CASE WHEN UPPER(type_code) = UPPER($1) THEN 0 ELSE 1 END
               LIMIT 1"#,
        );
        let type_row = execute_query_one(
            exec,
            &type_sql,
            &[SqlValue::String(type_code.clone())],
        )
        .await
        .map_err(|_| SemOsError::InvalidInput(format!("Entity type not found: {type_code}")))?;
        let entity_type_id: Uuid = type_row
            .try_get("entity_type_id")
            .map_err(|e| SemOsError::Internal(anyhow::anyhow!("{e}")))?;
        let extension_table: String = type_row
            .try_get("table_name")
            .map_err(|e| SemOsError::Internal(anyhow::anyhow!("{e}")))?;

        let entity_name = if type_code == "PROPER_PERSON" || type_code == "PROPER_PERSON_NATURAL" {
            let first = args_map
                .get("first-name")
                .and_then(|v| v.as_str())
                .unwrap_or("");
            let last = args_map
                .get("last-name")
                .and_then(|v| v.as_str())
                .unwrap_or("");
            format!("{first} {last}").trim().to_string()
        } else {
            args_map
                .get("name")
                .and_then(|v| v.as_str())
                .unwrap_or("Unknown")
                .to_string()
        };

        // UPSERT base entity
        let base_sql = format!(
            r#"INSERT INTO "{schema}".entities (entity_id, entity_type_id, name)
               VALUES (gen_random_uuid(), $1, $2)
               ON CONFLICT (entity_type_id, name) DO UPDATE SET updated_at = now()
               RETURNING entity_id"#,
        );
        let row = execute_query_one(
            exec,
            &base_sql,
            &[
                SqlValue::Uuid(entity_type_id),
                SqlValue::String(entity_name.clone()),
            ],
        )
        .await?;
        let entity_id: Uuid = row
            .try_get("entity_id")
            .map_err(|e| SemOsError::Internal(anyhow::anyhow!("{e}")))?;
        // T10.3: self-report for G14 — base-table half of the CTI write,
        // same posture as execute_entity_create (unconditional on success;
        // this statement always runs — no idempotent-skip branch here).
        let base_write_cols = [
            "entity_id".to_string(),
            "entity_type_id".to_string(),
            "name".to_string(),
        ];
        exec.record_write(&format!("{schema}.entities"), entity_id, &base_write_cols, true);

        // Build extension columns
        let ext_pk_col = infer_pk_column(&extension_table);
        let uses_shared_pk = ext_pk_col == "entity_id";

        let (mut columns, mut placeholders, mut bind_values, mut idx) = if uses_shared_pk {
            (
                vec![format!("\"{ext_pk_col}\"")],
                vec!["$1".to_string()],
                vec![SqlValue::Uuid(entity_id)],
                2,
            )
        } else {
            (
                vec![format!("\"{ext_pk_col}\""), "\"entity_id\"".to_string()],
                vec!["$1".to_string(), "$2".to_string()],
                vec![SqlValue::Uuid(Uuid::new_v4()), SqlValue::Uuid(entity_id)],
                3,
            )
        };
        // T10.3: unquoted parallel vec for `record_write`, built in
        // lockstep with `columns` — same technique as execute_entity_create.
        let mut raw_ext_columns: Vec<String> = if uses_shared_pk {
            vec![ext_pk_col.to_string()]
        } else {
            vec![ext_pk_col.to_string(), "entity_id".to_string()]
        };

        let mut update_cols: Vec<String> = Vec::new();
        let base_cols = ["name", "external_id"];
        for arg_def in arg_defs {
            if let Some(value) = args_map.get(&arg_def.name) {
                if arg_def.name == "entity-type" || arg_def.name == "entity-id" {
                    continue;
                }
                if let Some(col) = &arg_def.maps_to {
                    if col == ext_pk_col || col == "entity_id" || base_cols.contains(&col.as_str())
                    {
                        continue;
                    }
                    columns.push(format!("\"{col}\""));
                    raw_ext_columns.push(col.clone());
                    placeholders.push(format!("${idx}"));
                    update_cols.push(format!("\"{col}\" = EXCLUDED.\"{col}\""));
                    bind_values.push(json_to_sql_value(value, &arg_def.arg_type, &arg_def.name)?);
                    idx += 1;
                }
            }
        }

        if let Some(name_col) = infer_extension_name_column(&extension_table) {
            let quoted = format!("\"{name_col}\"");
            if !columns.contains(&quoted) {
                if let Some(name) = args_map.get("name").and_then(|v| v.as_str()) {
                    columns.push(quoted.clone());
                    raw_ext_columns.push(name_col.to_string());
                    placeholders.push(format!("${idx}"));
                    update_cols.push(format!("{quoted} = EXCLUDED.{quoted}"));
                    bind_values.push(SqlValue::String(name.to_string()));
                }
            }
        }

        let conflict_col = if columns.iter().any(|c| c == "\"isin\"") {
            "isin"
        } else {
            "entity_id"
        };

        let ext_sql = if update_cols.is_empty() {
            format!(
                r#"INSERT INTO "{schema}"."{extension_table}" ({}) VALUES ({}) ON CONFLICT ("{conflict_col}") DO NOTHING"#,
                columns.join(", "),
                placeholders.join(", ")
            )
        } else {
            format!(
                r#"INSERT INTO "{schema}"."{extension_table}" ({}) VALUES ({}) ON CONFLICT ("{conflict_col}") DO UPDATE SET {}"#,
                columns.join(", "),
                placeholders.join(", "),
                update_cols.join(", ")
            )
        };

        debug!(sql = %ext_sql, "CrudExecutionPort ENTITY_UPSERT extension");
        execute_non_query(exec, &ext_sql, &bind_values).await?;
        // T10.3: self-report for G14 — extension-table half of the CTI
        // write, same posture as execute_entity_create: keyed by
        // `entity_id`, not `ext_pk_col`'s own surrogate value.
        exec.record_write(
            &format!("{schema}.{extension_table}"),
            entity_id,
            &raw_ext_columns,
            true,
        );

        Ok(VerbExecutionOutcome::Uuid(entity_id))
    }
}

// ── Shared helpers ──────────────────────────────────────────────

/// SQL parameter value — mirrors the main crate's SqlValue.
#[derive(Debug, Clone)]
enum SqlValue {
    String(String),
    Uuid(Uuid),
    Integer(i64),
    Boolean(bool),
    Json(serde_json::Value),
    Date(chrono::NaiveDate),
}

fn json_to_sql_value(
    value: &serde_json::Value,
    arg_type: &str,
    arg_name: &str,
) -> crate::Result<SqlValue> {
    match arg_type {
        "string" | "str" | "lookup" => {
            let s = value.as_str().ok_or_else(|| {
                SemOsError::InvalidInput(format!("Expected string for {arg_name}"))
            })?;
            Ok(SqlValue::String(s.to_string()))
        }
        "uuid" => {
            let s = value.as_str().ok_or_else(|| {
                SemOsError::InvalidInput(format!("Expected UUID string for {arg_name}"))
            })?;
            let uuid = Uuid::parse_str(s).map_err(|e| {
                SemOsError::InvalidInput(format!("Invalid UUID for {arg_name}: {e}"))
            })?;
            Ok(SqlValue::Uuid(uuid))
        }
        "integer" | "int" => {
            let n = value.as_i64().ok_or_else(|| {
                SemOsError::InvalidInput(format!("Expected integer for {arg_name}"))
            })?;
            Ok(SqlValue::Integer(n))
        }
        "boolean" | "bool" => {
            let b = value.as_bool().ok_or_else(|| {
                SemOsError::InvalidInput(format!("Expected boolean for {arg_name}"))
            })?;
            Ok(SqlValue::Boolean(b))
        }
        "json" => Ok(SqlValue::Json(value.clone())),
        _ => {
            // Fallback: treat as string
            let s = value.as_str().unwrap_or(&value.to_string()).to_string();
            Ok(SqlValue::String(s))
        }
    }
}

fn bind_sql_value<'q>(
    query: sqlx::query::Query<'q, sqlx::Postgres, sqlx::postgres::PgArguments>,
    value: &SqlValue,
) -> sqlx::query::Query<'q, sqlx::Postgres, sqlx::postgres::PgArguments> {
    match value {
        SqlValue::String(s) => query.bind(s.clone()),
        SqlValue::Uuid(u) => query.bind(*u),
        SqlValue::Integer(n) => query.bind(*n),
        SqlValue::Boolean(b) => query.bind(*b),
        SqlValue::Json(j) => query.bind(j.clone()),
        SqlValue::Date(d) => query.bind(*d),
    }
}

async fn execute_query_one(
    exec: &mut CrudExec<'_>,
    sql: &str,
    values: &[SqlValue],
) -> crate::Result<PgRow> {
    let mut query = sqlx::query(sql);
    for val in values {
        query = bind_sql_value(query, val);
    }
    let result = match exec {
        CrudExec::Pool(p) => query.fetch_one(*p).await,
        CrudExec::Scope(s) => query.fetch_one(s.executor()).await,
    };
    result.map_err(|e| SemOsError::Internal(anyhow::anyhow!("SQL error: {e}")))
}

async fn execute_non_query(
    exec: &mut CrudExec<'_>,
    sql: &str,
    values: &[SqlValue],
) -> crate::Result<u64> {
    let mut query = sqlx::query(sql);
    for val in values {
        query = bind_sql_value(query, val);
    }
    let result = match exec {
        CrudExec::Pool(p) => query.execute(*p).await,
        CrudExec::Scope(s) => query.execute(s.executor()).await,
    };
    let result = result.map_err(|e| SemOsError::Internal(anyhow::anyhow!("SQL error: {e}")))?;
    Ok(result.rows_affected())
}

async fn execute_query(
    exec: &mut CrudExec<'_>,
    sql: &str,
    values: &[SqlValue],
) -> crate::Result<Vec<PgRow>> {
    let mut query = sqlx::query(sql);
    for val in values {
        query = bind_sql_value(query, val);
    }
    let result = match exec {
        CrudExec::Pool(p) => query.fetch_all(*p).await,
        CrudExec::Scope(s) => query.fetch_all(s.executor()).await,
    };
    result.map_err(|e| SemOsError::Internal(anyhow::anyhow!("SQL error: {e}")))
}

/// Infer the primary key column name from the table name.
/// Convention: singular form + "_id" (e.g., "cbus" → "cbu_id").
fn infer_pk_column(table: &str) -> &str {
    // Strip trailing 's' for simple plurals, then append _id
    // This is a best-effort heuristic; explicit `returning` in YAML overrides it.
    match table {
        "cbus" => "cbu_id",
        "entities" => "entity_id",
        "cases" => "case_id",
        "deals" => "deal_id",
        "documents" => "document_id",
        "requirements" => "requirement_id",
        "roles" => "role_id",
        "mandates" => "mandate_id",
        "billing_profiles" => "billing_profile_id",
        _ => {
            // Fallback: table name singular + _id (handled by returning field in YAML)
            // For safety, return the table name itself — YAML should always specify `returning`
            "id"
        }
    }
}

/// Resolve entity type code from CRUD config or args.
///
/// Slice 8.1 (2026-04-22): `crud` parameter prefixed with `_` — reserved
/// for future `type_code` support from YAML mapping; not yet consumed.
fn resolve_entity_type_code(
    _crud: &VerbCrudMapping,
    args: &serde_json::Map<String, serde_json::Value>,
) -> crate::Result<String> {
    // 1. Explicit type_code in config (not currently in VerbCrudMapping — future)
    // 2. entity-type arg
    if let Some(et) = args.get("entity-type").and_then(|v| v.as_str()) {
        return Ok(canonicalize_entity_type_code(et));
    }
    // 3. fund-type arg
    if let Some(ft) = args.get("fund-type").and_then(|v| v.as_str()) {
        return Ok(format!(
            "FUND_{}",
            ft.trim().to_uppercase().replace('-', "_")
        ));
    }
    // 4. Infer from operation string (e.g., "entitycreate" doesn't help, but verb action might)
    Err(SemOsError::InvalidInput(
        "Cannot resolve entity type: provide entity-type arg".into(),
    ))
}

fn canonicalize_entity_type_code(raw: &str) -> String {
    match raw.trim().to_uppercase().replace('-', "_").as_str() {
        "LIMITED_COMPANY" => "LIMITED_COMPANY_PRIVATE".to_string(),
        "PROPER_PERSON" => "PROPER_PERSON_NATURAL".to_string(),
        other => other.to_string(),
    }
}

fn infer_extension_name_column(table: &str) -> Option<&'static str> {
    match table {
        "entity_limited_companies" => Some("company_name"),
        "entity_partnerships" => Some("partnership_name"),
        "entity_trusts" => Some("trust_name"),
        _ => None,
    }
}

fn soft_delete_predicate(schema: &str, table: &str) -> Option<String> {
    if schema == "ob-poc" && matches!(table, "cbus" | "entities") {
        Some("\"deleted_at\" IS NULL".to_string())
    } else {
        None
    }
}

fn row_to_json(row: &PgRow) -> crate::Result<serde_json::Value> {
    let mut map = serde_json::Map::new();

    for column in row.columns() {
        let name = column.name();
        let type_name = column.type_info().name();

        let value: Option<serde_json::Value> = match type_name {
            "UUID" => row
                .try_get::<Option<Uuid>, _>(name)
                .ok()
                .flatten()
                .map(|u| serde_json::json!(u.to_string())),
            "TEXT" | "VARCHAR" | "CHAR" | "NAME" => row
                .try_get::<Option<String>, _>(name)
                .ok()
                .flatten()
                .map(|s| serde_json::json!(s)),
            "INT4" => row
                .try_get::<Option<i32>, _>(name)
                .ok()
                .flatten()
                .map(|i| serde_json::json!(i)),
            "INT8" => row
                .try_get::<Option<i64>, _>(name)
                .ok()
                .flatten()
                .map(|i| serde_json::json!(i)),
            "INT2" => row
                .try_get::<Option<i16>, _>(name)
                .ok()
                .flatten()
                .map(|i| serde_json::json!(i)),
            "FLOAT4" | "FLOAT8" => row
                .try_get::<Option<f64>, _>(name)
                .ok()
                .flatten()
                .map(|f| serde_json::json!(f)),
            "NUMERIC" => row
                .try_get::<Option<rust_decimal::Decimal>, _>(name)
                .ok()
                .flatten()
                .map(|d| serde_json::json!(d.to_string())),
            "BOOL" => row
                .try_get::<Option<bool>, _>(name)
                .ok()
                .flatten()
                .map(|b| serde_json::json!(b)),
            "JSONB" | "JSON" => row
                .try_get::<Option<serde_json::Value>, _>(name)
                .ok()
                .flatten(),
            "TIMESTAMPTZ" | "TIMESTAMP" => row
                .try_get::<Option<chrono::DateTime<chrono::Utc>>, _>(name)
                .ok()
                .flatten()
                .map(|dt| serde_json::json!(dt.to_rfc3339())),
            "DATE" => row
                .try_get::<Option<chrono::NaiveDate>, _>(name)
                .ok()
                .flatten()
                .map(|d| serde_json::json!(d.to_string())),
            _ => None,
        };

        map.insert(name.to_string(), value.unwrap_or(serde_json::Value::Null));
    }

    Ok(serde_json::Value::Object(map))
}

// ── Tests ───────────────────────────────────────────────────────

#[cfg(test)]
mod tests {
    use super::*;
    use sem_os_ontology::verb_contract::{VerbArgDef, VerbCrudMapping, VerbReturnSpec};

    fn make_select_contract(
        fqn: &str,
        table: &str,
        args: Vec<VerbArgDef>,
        return_type: &str,
    ) -> VerbContractBody {
        VerbContractBody {
            fqn: fqn.to_string(),
            domain: fqn.split('.').next().unwrap_or("test").to_string(),
            action: fqn.split('.').nth(1).unwrap_or("test").to_string(),
            description: "test".to_string(),
            behavior: "crud".to_string(),
            args,
            returns: Some(VerbReturnSpec {
                return_type: return_type.to_string(),
                schema: None,
            }),
            crud_mapping: Some(VerbCrudMapping {
                operation: "select".to_string(),
                table: Some(table.to_string()),
                schema: Some("ob-poc".to_string()),
                ..Default::default()
            }),
            ..default_contract_body()
        }
    }

    fn default_contract_body() -> VerbContractBody {
        VerbContractBody {
            fqn: String::new(),
            domain: String::new(),
            action: String::new(),
            description: String::new(),
            behavior: String::new(),
            args: vec![],
            returns: None,
            preconditions: vec![],
            postconditions: vec![],
            produces: None,
            consumes: vec![],
            invocation_phrases: vec![],
            subject_kinds: vec![],
            phase_tags: vec![],
            harm_class: None,
            action_class: None,
            precondition_states: vec![],
            requires_subject: true,
            produces_focus: false,
            metadata: None,
            crud_mapping: None,
            reads_from: vec![],
            writes_to: vec![],
            outputs: vec![],
            produces_shared_facts: vec![],
        }
    }

    #[test]
    fn json_to_sql_value_string() {
        let val = json_to_sql_value(&serde_json::json!("hello"), "string", "name").unwrap();
        assert!(matches!(val, SqlValue::String(s) if s == "hello"));
    }

    #[test]
    fn json_to_sql_value_uuid() {
        let id = Uuid::new_v4();
        let val = json_to_sql_value(&serde_json::json!(id.to_string()), "uuid", "id").unwrap();
        assert!(matches!(val, SqlValue::Uuid(u) if u == id));
    }

    #[test]
    fn json_to_sql_value_integer() {
        let val = json_to_sql_value(&serde_json::json!(42), "integer", "count").unwrap();
        assert!(matches!(val, SqlValue::Integer(42)));
    }

    #[test]
    fn json_to_sql_value_boolean() {
        let val = json_to_sql_value(&serde_json::json!(true), "boolean", "active").unwrap();
        assert!(matches!(val, SqlValue::Boolean(true)));
    }

    #[test]
    fn json_to_sql_value_fallback() {
        let val = json_to_sql_value(&serde_json::json!("fallback"), "unknown_type", "x").unwrap();
        assert!(matches!(val, SqlValue::String(s) if s == "fallback"));
    }

    #[test]
    fn soft_delete_for_cbus() {
        assert!(soft_delete_predicate("ob-poc", "cbus").is_some());
        assert!(soft_delete_predicate("ob-poc", "entities").is_some());
        assert!(soft_delete_predicate("ob-poc", "other_table").is_none());
        assert!(soft_delete_predicate("other-schema", "cbus").is_none());
    }

    #[test]
    fn make_select_contract_compiles() {
        let contract = make_select_contract(
            "cbu.show",
            "cbus",
            vec![VerbArgDef {
                name: "cbu-id".to_string(),
                arg_type: "uuid".to_string(),
                required: true,
                description: None,
                lookup: None,
                valid_values: None,
                default: None,
                maps_to: Some("cbu_id".to_string()),
            }],
            "record",
        );
        assert_eq!(contract.fqn, "cbu.show");
        assert!(contract.crud_mapping.is_some());
    }
}

// ── T9.2 §3 Branch 2: execute_crud_in_scope live-DB tests ────────

#[cfg(all(test, feature = "database"))]
mod db_integration_tests {
    use super::*;
    use sem_os_ontology::verb_contract::{VerbArgDef, VerbContractBody, VerbCrudMapping, VerbReturnSpec};

    async fn test_pool() -> PgPool {
        let url = std::env::var("DATABASE_URL").expect("DATABASE_URL required for db-integration tests");
        PgPool::connect(&url).await.expect("connect")
    }

    /// T10.3: minimal test-only `TransactionScope` wrapper. `dsl-runtime`
    /// has no concrete `TransactionScope` impl of its own (the real one,
    /// `PgTransactionScope`, lives in `ob-poc` — see `tx.rs`'s module doc
    /// for why the txn-opener stays out of this crate) — this exists
    /// purely so these tests can exercise `execute_crud_in_scope`'s
    /// `&mut dyn TransactionScope` signature without depending on `ob-poc`.
    struct TestScope {
        tx: sqlx::Transaction<'static, sqlx::Postgres>,
        pool: PgPool,
        id: ob_poc_types::TransactionScopeId,
        captured: Vec<(String, Uuid, Vec<String>, bool)>,
    }

    impl TestScope {
        async fn begin(pool: &PgPool) -> Self {
            Self {
                tx: pool.begin().await.expect("begin"),
                pool: pool.clone(),
                id: ob_poc_types::TransactionScopeId::new(),
                captured: Vec::new(),
            }
        }

        async fn rollback(self) {
            self.tx.rollback().await.expect("rollback");
        }

        async fn commit(self) {
            self.tx.commit().await.expect("commit");
        }
    }

    impl crate::TransactionScope for TestScope {
        fn scope_id(&self) -> ob_poc_types::TransactionScopeId {
            self.id
        }

        fn transaction(&mut self) -> &mut sqlx::Transaction<'static, sqlx::Postgres> {
            &mut self.tx
        }

        fn pool(&self) -> &PgPool {
            &self.pool
        }

        fn record_write(
            &mut self,
            table: &str,
            entity_id: Uuid,
            columns: &[String],
            created_new_entity: bool,
        ) {
            self.captured
                .push((table.to_string(), entity_id, columns.to_vec(), created_new_entity));
        }
    }

    fn default_contract_body() -> VerbContractBody {
        VerbContractBody {
            fqn: String::new(),
            domain: String::new(),
            action: String::new(),
            description: String::new(),
            behavior: String::new(),
            args: vec![],
            returns: None,
            preconditions: vec![],
            postconditions: vec![],
            produces: None,
            consumes: vec![],
            invocation_phrases: vec![],
            subject_kinds: vec![],
            phase_tags: vec![],
            harm_class: None,
            action_class: None,
            precondition_states: vec![],
            requires_subject: true,
            produces_focus: false,
            metadata: None,
            crud_mapping: None,
            reads_from: vec![],
            writes_to: vec![],
            outputs: vec![],
            produces_shared_facts: vec![],
        }
    }

    fn cbu_id_arg() -> VerbArgDef {
        VerbArgDef {
            name: "cbu-id".to_string(),
            arg_type: "uuid".to_string(),
            required: true,
            description: None,
            lookup: None,
            valid_values: None,
            default: None,
            maps_to: Some("cbu_id".to_string()),
        }
    }

    fn select_contract() -> VerbContractBody {
        VerbContractBody {
            fqn: "cbu.show".to_string(),
            domain: "cbu".to_string(),
            action: "show".to_string(),
            behavior: "crud".to_string(),
            args: vec![cbu_id_arg()],
            returns: Some(VerbReturnSpec {
                return_type: "record".to_string(),
                schema: None,
            }),
            crud_mapping: Some(VerbCrudMapping {
                operation: "select".to_string(),
                table: Some("cbus".to_string()),
                schema: Some("ob-poc".to_string()),
                ..Default::default()
            }),
            ..default_contract_body()
        }
    }

    fn update_description_contract() -> VerbContractBody {
        VerbContractBody {
            fqn: "cbu.set-description-test".to_string(),
            domain: "cbu".to_string(),
            action: "set-description-test".to_string(),
            behavior: "crud".to_string(),
            args: vec![
                cbu_id_arg(),
                VerbArgDef {
                    name: "description".to_string(),
                    arg_type: "string".to_string(),
                    required: true,
                    description: None,
                    lookup: None,
                    valid_values: None,
                    default: None,
                    maps_to: Some("description".to_string()),
                },
            ],
            returns: Some(VerbReturnSpec {
                return_type: "affected".to_string(),
                schema: None,
            }),
            crud_mapping: Some(VerbCrudMapping {
                operation: "update".to_string(),
                table: Some("cbus".to_string()),
                schema: Some("ob-poc".to_string()),
                key_column: Some("cbu_id".to_string()),
                ..Default::default()
            }),
            ..default_contract_body()
        }
    }

    fn fake_ctx() -> VerbExecutionContext {
        VerbExecutionContext::new(sem_os_core::principal::Principal::system())
    }

    /// Mirrors the real `entity.create` verb (`config/verbs/entity.yaml`,
    /// `operation: entity_create`) — same args/maps_to shape.
    fn entity_create_contract() -> VerbContractBody {
        VerbContractBody {
            fqn: "entity.create".to_string(),
            domain: "entity".to_string(),
            action: "create".to_string(),
            behavior: "crud".to_string(),
            args: vec![
                VerbArgDef {
                    name: "entity-type".to_string(),
                    arg_type: "string".to_string(),
                    required: true,
                    description: None,
                    lookup: None,
                    valid_values: None,
                    default: None,
                    maps_to: None,
                },
                VerbArgDef {
                    name: "first-name".to_string(),
                    arg_type: "string".to_string(),
                    required: false,
                    description: None,
                    lookup: None,
                    valid_values: None,
                    default: None,
                    maps_to: Some("first_name".to_string()),
                },
                VerbArgDef {
                    name: "last-name".to_string(),
                    arg_type: "string".to_string(),
                    required: false,
                    description: None,
                    lookup: None,
                    valid_values: None,
                    default: None,
                    maps_to: Some("last_name".to_string()),
                },
            ],
            returns: Some(VerbReturnSpec {
                return_type: "uuid".to_string(),
                schema: None,
            }),
            crud_mapping: Some(VerbCrudMapping {
                operation: "entity_create".to_string(),
                table: Some("entities".to_string()),
                schema: Some("ob-poc".to_string()),
                returning: Some("entity_id".to_string()),
                ..Default::default()
            }),
            ..default_contract_body()
        }
    }

    #[tokio::test]
    #[ignore = "requires DATABASE_URL"]
    async fn execute_crud_in_scope_matches_execute_crud_for_select() {
        let pool = test_pool().await;
        let (cbu_id,): (Uuid,) = sqlx::query_as(r#"SELECT cbu_id FROM "ob-poc".cbus LIMIT 1"#)
            .fetch_one(&pool)
            .await
            .expect("at least one cbu row exists in the dev database");

        let executor = PgCrudExecutor::new(pool.clone());
        let contract = select_contract();
        let args = serde_json::json!({ "cbu-id": cbu_id.to_string() });
        let ctx = fake_ctx();

        let pool_result = executor
            .execute_crud(&contract, args.clone(), &ctx)
            .await
            .expect("pool-based select");

        let mut scope = TestScope::begin(&pool).await;
        let scope_result = executor
            .execute_crud_in_scope(&contract, args, &ctx, &mut scope)
            .await
            .expect("scope-based select");
        scope.rollback().await;

        assert_eq!(
            format!("{:?}", pool_result),
            format!("{:?}", scope_result),
            "execute_crud_in_scope must return the same record execute_crud does"
        );
    }

    #[tokio::test]
    #[ignore = "requires DATABASE_URL"]
    async fn execute_crud_in_scope_update_rolled_back_leaves_no_durable_trace() {
        // The Branch 2 durability-on-failure proof (design doc §6): a write
        // issued through execute_crud_in_scope must be scoped to the
        // caller's transaction, not committed independently the way the old
        // per-statement-autocommit CRUD fast path always was.
        let pool = test_pool().await;
        // T10.3: deterministic OFFSET 0 — must be a *different* physical
        // row than the commit-path counterpart test below (OFFSET 1),
        // otherwise the two tests race for the same row's lock and one's
        // commit can leak into the other's "after rollback" read (both run
        // concurrently by default under `cargo test`). Fixed as a test-
        // isolation issue, not a product-code one, matching this project's
        // established PIR-D-004 precedent (shared mutable fixture races).
        let (cbu_id, original_description): (Uuid, Option<String>) = sqlx::query_as(
            r#"SELECT cbu_id, description FROM "ob-poc".cbus ORDER BY cbu_id LIMIT 1 OFFSET 0"#,
        )
        .fetch_one(&pool)
        .await
        .expect("at least one cbu row exists in the dev database");

        let executor = PgCrudExecutor::new(pool.clone());
        let contract = update_description_contract();
        let marker = format!("t9.2-scope-test-{}", Uuid::new_v4());
        let args = serde_json::json!({ "cbu-id": cbu_id.to_string(), "description": marker });
        let ctx = fake_ctx();

        let mut scope = TestScope::begin(&pool).await;
        let outcome = executor
            .execute_crud_in_scope(&contract, args, &ctx, &mut scope)
            .await
            .expect("scope-based update");
        assert!(matches!(outcome, VerbExecutionOutcome::Affected(1)));
        scope.rollback().await;

        let (after,): (Option<String>,) =
            sqlx::query_as(r#"SELECT description FROM "ob-poc".cbus WHERE cbu_id = $1"#)
                .bind(cbu_id)
                .fetch_one(&pool)
                .await
                .unwrap();
        assert_eq!(
            after, original_description,
            "a rolled-back scope must leave zero durable trace of the write"
        );
    }

    #[tokio::test]
    #[ignore = "requires DATABASE_URL"]
    async fn execute_crud_in_scope_update_committed_is_durable() {
        // The commit-path counterpart to the rollback proof above: a write
        // issued through execute_crud_in_scope, once the caller commits the
        // scope, IS durable — this is the whole point of joining CRUD to
        // the admitting transaction rather than leaving it autocommitted.
        let pool = test_pool().await;
        // T10.3: OFFSET 1 — the deliberately distinct row from the
        // rollback test's OFFSET 0 (see the isolation note there).
        let (cbu_id, original_description): (Uuid, Option<String>) = sqlx::query_as(
            r#"SELECT cbu_id, description FROM "ob-poc".cbus ORDER BY cbu_id LIMIT 1 OFFSET 1"#,
        )
        .fetch_one(&pool)
        .await
        .expect("at least two cbu rows exist in the dev database");

        let executor = PgCrudExecutor::new(pool.clone());
        let contract = update_description_contract();
        let marker = format!("t9.2-scope-test-{}", Uuid::new_v4());
        let args = serde_json::json!({ "cbu-id": cbu_id.to_string(), "description": marker.clone() });
        let ctx = fake_ctx();

        let mut scope = TestScope::begin(&pool).await;
        executor
            .execute_crud_in_scope(&contract, args, &ctx, &mut scope)
            .await
            .expect("scope-based update");
        scope.commit().await;

        let (after,): (Option<String>,) =
            sqlx::query_as(r#"SELECT description FROM "ob-poc".cbus WHERE cbu_id = $1"#)
                .bind(cbu_id)
                .fetch_one(&pool)
                .await
                .unwrap();
        assert_eq!(after, Some(marker), "a committed scope's write must be durable");

        // Cleanup — restore the original value so this test leaves no
        // lasting side effect on the dev database.
        sqlx::query(r#"UPDATE "ob-poc".cbus SET description = $1 WHERE cbu_id = $2"#)
            .bind(&original_description)
            .bind(cbu_id)
            .execute(&pool)
            .await
            .unwrap();
    }

    /// T10.3: proves `execute_entity_create`'s base-`entities`-table
    /// `record_write` call fires correctly.
    ///
    /// NOTE — pre-existing, unrelated bug discovered while writing this
    /// test: `infer_pk_column` in *this* file (unlike its counterpart in
    /// `dsl_v2::generic_executor.rs`, the actual live path for
    /// `entity.create`) has no case for any entity extension table
    /// (`entity_proper_persons`, `entity_limited_companies`,
    /// `entity_funds`, ...) — it falls back to a literal `"id"` column,
    /// which does not exist on any of them. `execute_entity_create`'s
    /// extension-table INSERT therefore fails for every real entity type
    /// today, independent of this task's `record_write` change (confirmed:
    /// same failure with the `record_write` calls commented out). Fixing
    /// `infer_pk_column` is out of scope for T10.3 (write-set-attestation
    /// wiring only) — so this test proves the base-table capture (which
    /// runs, and succeeds, before the broken extension step) and stops
    /// short of the second capture, which cannot be live-DB-proven until
    /// that separate bug is fixed. Real verb: `entity.create`
    /// (`config/verbs/entity.yaml`, `operation: entity_create`).
    #[tokio::test]
    #[ignore = "requires DATABASE_URL"]
    async fn execute_crud_in_scope_entity_create_records_base_table_write() {
        let pool = test_pool().await;
        let executor = PgCrudExecutor::new(pool.clone());
        let contract = entity_create_contract();
        let marker_first = format!("t10.3-{}", Uuid::new_v4());
        let args = serde_json::json!({
            "entity-type": "proper-person",
            "first-name": marker_first,
            "last-name": "RecordWriteTest",
        });
        let ctx = fake_ctx();

        let mut scope = TestScope::begin(&pool).await;
        let result = executor
            .execute_crud_in_scope(&contract, args, &ctx, &mut scope)
            .await;

        // Expected to fail at the (pre-existing, unrelated) broken
        // extension-table insert — see note above.
        assert!(
            result.is_err(),
            "expected the pre-existing extension-table infer_pk_column bug \
             to surface (if this now passes, that bug was fixed independently \
             and this test should be upgraded to assert both captures)"
        );

        assert_eq!(
            scope.captured.len(),
            1,
            "expected exactly 1 captured write (base entities row, before \
             the extension insert fails), got {:?}",
            scope.captured
        );
        let (base_table, _base_id, base_cols, base_created_new_entity) =
            scope.captured[0].clone();
        assert_eq!(base_table, "ob-poc.entities");
        assert!(
            base_created_new_entity,
            "entity_create's base-table write always creates the entities row it writes to"
        );
        assert_eq!(
            base_cols,
            vec![
                "entity_id".to_string(),
                "entity_type_id".to_string(),
                "name".to_string()
            ]
        );

        // Postgres aborts the whole transaction on the extension-insert
        // error (25P02: current transaction is aborted), so no further
        // statement — including a same-tx existence check — can run
        // against `scope` here. `scope.captured` is populated in-process
        // by `record_write` independent of the SQL result, which is
        // exactly what proves the base-table capture itself fired.
        scope.rollback().await;
    }
}
