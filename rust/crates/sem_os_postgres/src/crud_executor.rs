//! CrudExecutionPort implementation — metadata-driven SQL execution.
//!
//! Executes CRUD verbs using `VerbContractBody` metadata (table, schema,
//! operation, column mappings). No hand-coded Rust per verb.
//!
//! This is the SemOS-native replacement for ob-poc's `GenericCrudExecutor`.
//! Operations are added incrementally — unsupported operations return
//! `SemOsError::InvalidInput` until migrated.

use async_trait::async_trait;
use sqlx::postgres::PgRow;
use sqlx::{Column, PgPool, Row, TypeInfo};
use tracing::debug;
use uuid::Uuid;

use sem_os_core::error::SemOsError;
use sem_os_core::execution::{
    CrudExecutionPort, VerbExecutionContext, VerbExecutionOutcome,
};
use sem_os_core::verb_contract::{VerbArgDef, VerbContractBody, VerbCrudMapping};

/// SemOS-native CRUD executor backed by PostgreSQL.
pub struct PgCrudExecutor {
    pool: PgPool,
}

impl PgCrudExecutor {
    pub fn new(pool: PgPool) -> Self {
        Self { pool }
    }
}

#[async_trait]
impl CrudExecutionPort for PgCrudExecutor {
    async fn execute_crud(
        &self,
        contract: &VerbContractBody,
        args: serde_json::Value,
        _ctx: &VerbExecutionContext,
    ) -> sem_os_core::execution::Result<VerbExecutionOutcome> {
        let crud = contract.crud_mapping.as_ref().ok_or_else(|| {
            SemOsError::InvalidInput(format!(
                "Verb {} has no crud_mapping",
                contract.fqn
            ))
        })?;

        let table = crud.table.as_deref().ok_or_else(|| {
            SemOsError::InvalidInput(format!("Verb {} crud_mapping has no table", contract.fqn))
        })?;
        let schema = crud.schema.as_deref().unwrap_or("ob-poc");

        match crud.operation.as_str() {
            "select" => self.execute_select(schema, table, &contract.args, &args, &contract.returns).await,
            "list_by_fk" => self.execute_list_by_fk(schema, table, crud, &contract.args, &args).await,
            op => Err(SemOsError::InvalidInput(format!(
                "CRUD operation '{}' not yet migrated to CrudExecutionPort (verb: {})",
                op, contract.fqn
            ))),
        }
    }
}

impl PgCrudExecutor {
    // ── SELECT ──────────────────────────────────────────────────

    async fn execute_select(
        &self,
        schema: &str,
        table: &str,
        arg_defs: &[VerbArgDef],
        args: &serde_json::Value,
        returns: &Option<sem_os_core::verb_contract::VerbReturnSpec>,
    ) -> sem_os_core::execution::Result<VerbExecutionOutcome> {
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

        let rows = execute_query(&self.pool, &sql, &bind_values).await?;

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
            let records: Result<Vec<serde_json::Value>, _> =
                rows.iter().map(row_to_json).collect();
            Ok(VerbExecutionOutcome::RecordSet(records?))
        }
    }

    // ── LIST_BY_FK ──────────────────────────────────────────────

    async fn execute_list_by_fk(
        &self,
        schema: &str,
        table: &str,
        crud: &VerbCrudMapping,
        arg_defs: &[VerbArgDef],
        args: &serde_json::Value,
    ) -> sem_os_core::execution::Result<VerbExecutionOutcome> {
        let args_map = args.as_object().cloned().unwrap_or_default();

        let fk_col = crud.fk_col.as_deref().ok_or_else(|| {
            SemOsError::InvalidInput("list_by_fk requires fk_col".into())
        })?;

        // Find the FK arg value
        let mut fk_value = None;
        let mut extra_conditions = Vec::new();
        let mut bind_values: Vec<SqlValue> = Vec::new();
        let mut idx = 1;

        for arg_def in arg_defs {
            if let Some(value) = args_map.get(&arg_def.name) {
                if let Some(col) = &arg_def.maps_to {
                    if col == fk_col {
                        fk_value = Some(json_to_sql_value(value, &arg_def.arg_type, &arg_def.name)?);
                    } else {
                        extra_conditions.push(format!("\"{}\" = ${}", col, idx + 1));
                        bind_values.push(json_to_sql_value(value, &arg_def.arg_type, &arg_def.name)?);
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

        let rows = execute_query(&self.pool, &sql, &all_values).await?;
        let records: Result<Vec<serde_json::Value>, _> = rows.iter().map(row_to_json).collect();
        Ok(VerbExecutionOutcome::RecordSet(records?))
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
}

fn json_to_sql_value(
    value: &serde_json::Value,
    arg_type: &str,
    arg_name: &str,
) -> sem_os_core::execution::Result<SqlValue> {
    match arg_type {
        "string" | "str" | "lookup" => {
            let s = value
                .as_str()
                .ok_or_else(|| SemOsError::InvalidInput(format!("Expected string for {arg_name}")))?;
            Ok(SqlValue::String(s.to_string()))
        }
        "uuid" => {
            let s = value
                .as_str()
                .ok_or_else(|| SemOsError::InvalidInput(format!("Expected UUID string for {arg_name}")))?;
            let uuid = Uuid::parse_str(s)
                .map_err(|e| SemOsError::InvalidInput(format!("Invalid UUID for {arg_name}: {e}")))?;
            Ok(SqlValue::Uuid(uuid))
        }
        "integer" | "int" => {
            let n = value
                .as_i64()
                .ok_or_else(|| SemOsError::InvalidInput(format!("Expected integer for {arg_name}")))?;
            Ok(SqlValue::Integer(n))
        }
        "boolean" | "bool" => {
            let b = value
                .as_bool()
                .ok_or_else(|| SemOsError::InvalidInput(format!("Expected boolean for {arg_name}")))?;
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
    }
}

async fn execute_query(
    pool: &PgPool,
    sql: &str,
    values: &[SqlValue],
) -> sem_os_core::execution::Result<Vec<PgRow>> {
    let mut query = sqlx::query(sql);
    for val in values {
        query = bind_sql_value(query, val);
    }
    query
        .fetch_all(pool)
        .await
        .map_err(|e| SemOsError::Internal(anyhow::anyhow!("SQL error: {e}")))
}

fn soft_delete_predicate(schema: &str, table: &str) -> Option<String> {
    if schema == "ob-poc" && matches!(table, "cbus" | "entities") {
        Some("\"deleted_at\" IS NULL".to_string())
    } else {
        None
    }
}

fn row_to_json(row: &PgRow) -> sem_os_core::execution::Result<serde_json::Value> {
    let mut map = serde_json::Map::new();

    for column in row.columns() {
        let name = column.name();
        let type_name = column.type_info().name();

        let value: Option<serde_json::Value> = match type_name {
            "UUID" => row.try_get::<Option<Uuid>, _>(name).ok().flatten()
                .map(|u| serde_json::json!(u.to_string())),
            "TEXT" | "VARCHAR" | "CHAR" | "NAME" => row.try_get::<Option<String>, _>(name).ok().flatten()
                .map(|s| serde_json::json!(s)),
            "INT4" => row.try_get::<Option<i32>, _>(name).ok().flatten()
                .map(|i| serde_json::json!(i)),
            "INT8" => row.try_get::<Option<i64>, _>(name).ok().flatten()
                .map(|i| serde_json::json!(i)),
            "INT2" => row.try_get::<Option<i16>, _>(name).ok().flatten()
                .map(|i| serde_json::json!(i)),
            "FLOAT4" | "FLOAT8" => row.try_get::<Option<f64>, _>(name).ok().flatten()
                .map(|f| serde_json::json!(f)),
            "NUMERIC" => row.try_get::<Option<rust_decimal::Decimal>, _>(name).ok().flatten()
                .map(|d| serde_json::json!(d.to_string())),
            "BOOL" => row.try_get::<Option<bool>, _>(name).ok().flatten()
                .map(|b| serde_json::json!(b)),
            "JSONB" | "JSON" => row.try_get::<Option<serde_json::Value>, _>(name).ok().flatten(),
            "TIMESTAMPTZ" | "TIMESTAMP" => row.try_get::<Option<chrono::DateTime<chrono::Utc>>, _>(name).ok().flatten()
                .map(|dt| serde_json::json!(dt.to_rfc3339())),
            "DATE" => row.try_get::<Option<chrono::NaiveDate>, _>(name).ok().flatten()
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
    use sem_os_core::verb_contract::{VerbArgDef, VerbCrudMapping, VerbReturnSpec};

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
        let val = json_to_sql_value(
            &serde_json::json!("hello"),
            "string",
            "name",
        ).unwrap();
        assert!(matches!(val, SqlValue::String(s) if s == "hello"));
    }

    #[test]
    fn json_to_sql_value_uuid() {
        let id = Uuid::new_v4();
        let val = json_to_sql_value(
            &serde_json::json!(id.to_string()),
            "uuid",
            "id",
        ).unwrap();
        assert!(matches!(val, SqlValue::Uuid(u) if u == id));
    }

    #[test]
    fn json_to_sql_value_integer() {
        let val = json_to_sql_value(
            &serde_json::json!(42),
            "integer",
            "count",
        ).unwrap();
        assert!(matches!(val, SqlValue::Integer(42)));
    }

    #[test]
    fn json_to_sql_value_boolean() {
        let val = json_to_sql_value(
            &serde_json::json!(true),
            "boolean",
            "active",
        ).unwrap();
        assert!(matches!(val, SqlValue::Boolean(true)));
    }

    #[test]
    fn json_to_sql_value_fallback() {
        let val = json_to_sql_value(
            &serde_json::json!("fallback"),
            "unknown_type",
            "x",
        ).unwrap();
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
