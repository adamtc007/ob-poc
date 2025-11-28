//! DSL Executor - Data-driven execution engine for DSL v2
//!
//! This module implements the DslExecutor that processes parsed DSL programs
//! and executes them against the database using data-driven verb definitions.

use anyhow::{anyhow, bail, Result};
use serde_json::Value as JsonValue;
use std::collections::HashMap;
use uuid::Uuid;

use super::ast::{Program, Statement, Value, VerbCall};
use super::custom_ops::CustomOperationRegistry;
use super::mappings::{get_pk_column, resolve_column};
use super::verbs::{find_verb, Behavior, VerbDef};

#[cfg(feature = "database")]
use sqlx::PgPool;

/// Return type specification for verb execution
#[derive(Debug, Clone)]
pub enum ReturnType {
    /// Returns a single UUID (e.g., created entity ID)
    Uuid { name: &'static str, capture: bool },
    /// Returns a single record as JSON
    Record,
    /// Returns multiple records as JSON array
    RecordSet,
    /// Returns count of affected rows
    Affected,
    /// Returns nothing (void operation)
    Void,
}

/// Result of executing a verb
#[derive(Debug, Clone)]
pub enum ExecutionResult {
    /// A UUID was returned (e.g., from INSERT RETURNING)
    Uuid(Uuid),
    /// A single record was returned
    Record(JsonValue),
    /// Multiple records were returned
    RecordSet(Vec<JsonValue>),
    /// Count of affected rows
    Affected(u64),
    /// No result (void operation)
    Void,
}

/// Execution context holding state during DSL execution
#[derive(Debug, Default)]
pub struct ExecutionContext {
    /// Symbol table for @reference resolution
    pub symbols: HashMap<String, Uuid>,
    /// Audit user for tracking
    pub audit_user: Option<String>,
    /// Transaction ID for grouping operations
    pub transaction_id: Option<Uuid>,
}

impl ExecutionContext {
    pub fn new() -> Self {
        Self::default()
    }

    /// Bind a symbol to a UUID value
    pub fn bind(&mut self, name: &str, value: Uuid) {
        self.symbols.insert(name.to_string(), value);
    }

    /// Resolve a symbol reference
    pub fn resolve(&self, name: &str) -> Option<Uuid> {
        self.symbols.get(name).copied()
    }

    /// Set the audit user
    pub fn with_audit_user(mut self, user: &str) -> Self {
        self.audit_user = Some(user.to_string());
        self
    }
}

/// The main DSL executor
pub struct DslExecutor {
    #[cfg(feature = "database")]
    pool: PgPool,
    custom_ops: CustomOperationRegistry,
}

impl DslExecutor {
    /// Create a new executor with a database pool
    #[cfg(feature = "database")]
    pub fn new(pool: PgPool) -> Self {
        Self {
            pool,
            custom_ops: CustomOperationRegistry::new(),
        }
    }

    /// Create an executor without database (for testing/parsing only)
    #[cfg(not(feature = "database"))]
    pub fn new_without_db() -> Self {
        Self {
            custom_ops: CustomOperationRegistry::new(),
        }
    }

    /// Execute a complete DSL program
    #[cfg(feature = "database")]
    pub async fn execute_program(
        &self,
        program: &Program,
        ctx: &mut ExecutionContext,
    ) -> Result<Vec<ExecutionResult>> {
        let mut results = Vec::new();

        for statement in &program.statements {
            match statement {
                Statement::VerbCall(vc) => {
                    let result = self.execute_verb(vc, ctx).await?;
                    results.push(result);
                }
                Statement::Comment(_) => {
                    // Comments are no-ops
                }
            }
        }

        Ok(results)
    }

    /// Execute a single verb call
    #[cfg(feature = "database")]
    pub async fn execute_verb(
        &self,
        vc: &VerbCall,
        ctx: &mut ExecutionContext,
    ) -> Result<ExecutionResult> {
        // First check for custom operations
        if let Some(op) = self.custom_ops.get(&vc.domain, &vc.verb) {
            return op.execute(vc, ctx, &self.pool).await;
        }

        // Look up standard verb definition
        let verb_def = find_verb(&vc.domain, &vc.verb)
            .ok_or_else(|| anyhow!("Unknown verb: {}.{}", vc.domain, vc.verb))?;

        // Validate required arguments
        self.validate_args(vc, verb_def)?;

        // Resolve references in arguments
        let resolved_args = self.resolve_args(&vc.arguments, ctx)?;

        // Execute based on behavior
        let result = match &verb_def.behavior {
            Behavior::Insert { table } => {
                self.execute_insert(table, &resolved_args, verb_def, ctx)
                    .await?
            }
            Behavior::Select { table } => {
                self.execute_select(table, &resolved_args, verb_def).await?
            }
            Behavior::Update { table } => {
                self.execute_update(table, &resolved_args, verb_def).await?
            }
            Behavior::Delete { table } => self.execute_delete(table, &resolved_args).await?,
            Behavior::Upsert {
                table,
                conflict_keys,
            } => {
                self.execute_upsert(table, conflict_keys, &resolved_args, verb_def, ctx)
                    .await?
            }
            Behavior::Link {
                junction,
                from_col,
                to_col,
                role_col,
            } => {
                self.execute_link(
                    junction,
                    from_col,
                    to_col,
                    role_col.as_deref(),
                    &resolved_args,
                )
                .await?
            }
            Behavior::Unlink {
                junction,
                from_col,
                to_col,
            } => {
                self.execute_unlink(junction, from_col, to_col, &resolved_args)
                    .await?
            }
            Behavior::ListByFk { table, fk_col } => {
                self.execute_list_by_fk(table, fk_col, &resolved_args)
                    .await?
            }
            Behavior::SelectWithJoin {
                primary_table,
                join_table,
                join_col,
            } => {
                self.execute_select_with_join(primary_table, join_table, join_col, &resolved_args)
                    .await?
            }
        };

        // Handle symbol capture if specified
        if let ReturnType::Uuid {
            name,
            capture: true,
        } = &verb_def.returns
        {
            if let ExecutionResult::Uuid(uuid) = &result {
                ctx.bind(name, *uuid);
            }
        }

        Ok(result)
    }

    /// Validate that required arguments are present
    fn validate_args(&self, vc: &VerbCall, verb_def: &VerbDef) -> Result<()> {
        for required in verb_def.required_args {
            let found = vc
                .arguments
                .iter()
                .any(|arg| arg.key.canonical() == *required || arg.key.matches(required));
            if !found {
                bail!(
                    "Missing required argument '{}' for {}.{}",
                    required,
                    vc.domain,
                    vc.verb
                );
            }
        }
        Ok(())
    }

    /// Resolve @references in arguments
    fn resolve_args(
        &self,
        args: &[super::ast::Argument],
        ctx: &ExecutionContext,
    ) -> Result<HashMap<String, ResolvedValue>> {
        let mut resolved = HashMap::new();

        for arg in args {
            let key = arg.key.canonical();
            let value = self.resolve_value(&arg.value, ctx)?;
            resolved.insert(key, value);
        }

        Ok(resolved)
    }

    /// Resolve a single value, looking up references
    fn resolve_value(&self, value: &Value, ctx: &ExecutionContext) -> Result<ResolvedValue> {
        match value {
            Value::String(s) => Ok(ResolvedValue::String(s.clone())),
            Value::Integer(i) => Ok(ResolvedValue::Integer(*i)),
            Value::Decimal(d) => Ok(ResolvedValue::Decimal(*d)),
            Value::Boolean(b) => Ok(ResolvedValue::Boolean(*b)),
            Value::Null => Ok(ResolvedValue::Null),
            Value::Reference(name) => {
                let uuid = ctx
                    .resolve(name)
                    .ok_or_else(|| anyhow!("Unresolved reference: @{}", name))?;
                Ok(ResolvedValue::Uuid(uuid))
            }
            Value::AttributeRef(uuid) => Ok(ResolvedValue::Uuid(*uuid)),
            Value::DocumentRef(uuid) => Ok(ResolvedValue::Uuid(*uuid)),
            Value::List(items) => {
                let resolved: Result<Vec<_>> =
                    items.iter().map(|v| self.resolve_value(v, ctx)).collect();
                Ok(ResolvedValue::List(resolved?))
            }
            Value::Map(map) => {
                let resolved: Result<HashMap<_, _>> = map
                    .iter()
                    .map(|(k, v)| {
                        let rv = self.resolve_value(v, ctx)?;
                        Ok((k.clone(), rv))
                    })
                    .collect();
                Ok(ResolvedValue::Map(resolved?))
            }
        }
    }

    // =========================================================================
    // Generic CRUD Operations
    // =========================================================================

    #[cfg(feature = "database")]
    async fn execute_insert(
        &self,
        table: &str,
        args: &HashMap<String, ResolvedValue>,
        _verb_def: &VerbDef,
        _ctx: &ExecutionContext,
    ) -> Result<ExecutionResult> {
        let pk_col = get_pk_column(table).ok_or_else(|| anyhow!("Unknown table: {}", table))?;

        // Build column list and values
        let mut columns = Vec::new();
        let mut placeholders = Vec::new();
        let mut bind_values: Vec<BindValue> = Vec::new();
        let mut idx = 1;

        // Generate UUID for primary key
        let new_id = Uuid::new_v4();
        columns.push(pk_col.to_string());
        placeholders.push(format!("${}", idx));
        bind_values.push(BindValue::Uuid(new_id));
        idx += 1;

        // Add provided arguments
        for (key, value) in args {
            if let Some((db_col, _db_type)) = resolve_column(table, key) {
                // Skip if it's the PK (we already added it)
                if db_col == pk_col {
                    continue;
                }
                columns.push(db_col.to_string());
                placeholders.push(format!("${}", idx));
                bind_values.push(value.to_bind_value());
                idx += 1;
            }
        }

        let sql = format!(
            "INSERT INTO {} ({}) VALUES ({}) RETURNING {}",
            table,
            columns.join(", "),
            placeholders.join(", "),
            pk_col
        );

        // Build and execute query with dynamic bindings
        let mut query = sqlx::query_scalar::<_, Uuid>(&sql);
        for bv in &bind_values {
            query = bind_value_to_query(query, bv);
        }

        let returned_id = query.fetch_one(&self.pool).await?;
        Ok(ExecutionResult::Uuid(returned_id))
    }

    #[cfg(feature = "database")]
    async fn execute_select(
        &self,
        table: &str,
        args: &HashMap<String, ResolvedValue>,
        verb_def: &VerbDef,
    ) -> Result<ExecutionResult> {
        let _pk_col = get_pk_column(table).ok_or_else(|| anyhow!("Unknown table: {}", table))?;

        // Build WHERE clause
        let mut conditions = Vec::new();
        let mut bind_values: Vec<BindValue> = Vec::new();
        let mut idx = 1;

        for (key, value) in args {
            if let Some((db_col, _db_type)) = resolve_column(table, key) {
                // Skip pagination args
                if key == "limit" || key == "offset" {
                    continue;
                }
                conditions.push(format!("{} = ${}", db_col, idx));
                bind_values.push(value.to_bind_value());
                idx += 1;
            }
        }

        let where_clause = if conditions.is_empty() {
            String::new()
        } else {
            format!(" WHERE {}", conditions.join(" AND "))
        };

        // Handle pagination
        let mut pagination = String::new();
        if let Some(ResolvedValue::Integer(limit)) = args.get("limit") {
            pagination.push_str(&format!(" LIMIT {}", limit));
        }
        if let Some(ResolvedValue::Integer(offset)) = args.get("offset") {
            pagination.push_str(&format!(" OFFSET {}", offset));
        }

        let sql = format!("SELECT * FROM {}{}{}", table, where_clause, pagination);

        // Determine if we expect single or multiple results
        match &verb_def.returns {
            ReturnType::Record => {
                let row = sqlx::query(&sql).fetch_optional(&self.pool).await?;

                match row {
                    Some(r) => {
                        let json = row_to_json(&r)?;
                        Ok(ExecutionResult::Record(json))
                    }
                    None => Ok(ExecutionResult::Record(JsonValue::Null)),
                }
            }
            ReturnType::RecordSet => {
                let rows = sqlx::query(&sql).fetch_all(&self.pool).await?;

                let records: Result<Vec<_>> = rows.iter().map(row_to_json).collect();
                Ok(ExecutionResult::RecordSet(records?))
            }
            _ => bail!("Invalid return type for SELECT"),
        }
    }

    #[cfg(feature = "database")]
    async fn execute_update(
        &self,
        table: &str,
        args: &HashMap<String, ResolvedValue>,
        _verb_def: &VerbDef,
    ) -> Result<ExecutionResult> {
        let pk_col = get_pk_column(table).ok_or_else(|| anyhow!("Unknown table: {}", table))?;

        // Find the PK value
        let pk_key = pk_col.replace('_', "-");
        let pk_value = args
            .get(&pk_key)
            .ok_or_else(|| anyhow!("Missing primary key argument: {}", pk_key))?;

        // Build SET clause
        let mut sets = Vec::new();
        let mut bind_values: Vec<BindValue> = Vec::new();
        let mut idx = 1;

        for (key, value) in args {
            if let Some((db_col, _db_type)) = resolve_column(table, key) {
                // Skip the PK - it goes in WHERE clause
                if db_col == pk_col {
                    continue;
                }
                sets.push(format!("{} = ${}", db_col, idx));
                bind_values.push(value.to_bind_value());
                idx += 1;
            }
        }

        if sets.is_empty() {
            bail!("No fields to update");
        }

        // Add updated_at if table has it
        if resolve_column(table, "updated-at").is_some() {
            sets.push("updated_at = NOW()".to_string());
        }

        // Add PK to values for WHERE clause
        bind_values.push(pk_value.to_bind_value());

        let sql = format!(
            "UPDATE {} SET {} WHERE {} = ${}",
            table,
            sets.join(", "),
            pk_col,
            idx
        );

        let mut query = sqlx::query(&sql);
        for bv in &bind_values {
            query = bind_value_to_query_regular(query, bv);
        }

        let result = query.execute(&self.pool).await?;
        Ok(ExecutionResult::Affected(result.rows_affected()))
    }

    #[cfg(feature = "database")]
    async fn execute_delete(
        &self,
        table: &str,
        args: &HashMap<String, ResolvedValue>,
    ) -> Result<ExecutionResult> {
        let pk_col = get_pk_column(table).ok_or_else(|| anyhow!("Unknown table: {}", table))?;

        let pk_key = pk_col.replace('_', "-");
        let pk_value = args
            .get(&pk_key)
            .ok_or_else(|| anyhow!("Missing primary key argument: {}", pk_key))?;

        let sql = format!("DELETE FROM {} WHERE {} = $1", table, pk_col);

        let result = sqlx::query(&sql)
            .bind(pk_value.as_uuid()?)
            .execute(&self.pool)
            .await?;

        Ok(ExecutionResult::Affected(result.rows_affected()))
    }

    #[cfg(feature = "database")]
    async fn execute_upsert(
        &self,
        table: &str,
        conflict_keys: &[&str],
        args: &HashMap<String, ResolvedValue>,
        _verb_def: &VerbDef,
        _ctx: &ExecutionContext,
    ) -> Result<ExecutionResult> {
        let pk_col = get_pk_column(table).ok_or_else(|| anyhow!("Unknown table: {}", table))?;

        // Build column list and values
        let mut columns = Vec::new();
        let mut placeholders = Vec::new();
        let mut update_sets = Vec::new();
        let mut bind_values: Vec<BindValue> = Vec::new();
        let mut idx = 1;

        // Generate UUID for primary key
        let new_id = Uuid::new_v4();
        columns.push(pk_col.to_string());
        placeholders.push(format!("${}", idx));
        bind_values.push(BindValue::Uuid(new_id));
        idx += 1;

        // Resolve conflict key columns
        let conflict_cols: Vec<_> = conflict_keys
            .iter()
            .filter_map(|k| resolve_column(table, k).map(|(c, _)| c))
            .collect();

        // Add provided arguments
        for (key, value) in args {
            if let Some((db_col, _db_type)) = resolve_column(table, key) {
                if db_col == pk_col {
                    continue;
                }
                columns.push(db_col.to_string());
                placeholders.push(format!("${}", idx));

                // Add to UPDATE SET if not a conflict key
                if !conflict_cols.contains(&db_col) {
                    update_sets.push(format!("{} = EXCLUDED.{}", db_col, db_col));
                }

                bind_values.push(value.to_bind_value());
                idx += 1;
            }
        }

        let sql = format!(
            "INSERT INTO {} ({}) VALUES ({}) \
             ON CONFLICT ({}) DO UPDATE SET {} \
             RETURNING {}",
            table,
            columns.join(", "),
            placeholders.join(", "),
            conflict_cols.join(", "),
            if update_sets.is_empty() {
                format!("{} = EXCLUDED.{}", pk_col, pk_col)
            } else {
                update_sets.join(", ")
            },
            pk_col
        );

        let mut query = sqlx::query_scalar::<_, Uuid>(&sql);
        for bv in &bind_values {
            query = bind_value_to_query(query, bv);
        }

        let returned_id = query.fetch_one(&self.pool).await?;
        Ok(ExecutionResult::Uuid(returned_id))
    }

    #[cfg(feature = "database")]
    async fn execute_link(
        &self,
        junction: &str,
        from_col: &str,
        to_col: &str,
        role_col: Option<&str>,
        args: &HashMap<String, ResolvedValue>,
    ) -> Result<ExecutionResult> {
        let pk_col = get_pk_column(junction)
            .ok_or_else(|| anyhow!("Unknown junction table: {}", junction))?;

        let new_id = Uuid::new_v4();
        let from_key = from_col.replace('_', "-");
        let to_key = to_col.replace('_', "-");

        let from_val = args
            .get(&from_key)
            .ok_or_else(|| anyhow!("Missing {} argument", from_key))?
            .as_uuid()?;
        let to_val = args
            .get(&to_key)
            .ok_or_else(|| anyhow!("Missing {} argument", to_key))?
            .as_uuid()?;

        let mut columns = vec![pk_col.to_string(), from_col.to_string(), to_col.to_string()];
        let mut placeholders = vec!["$1".to_string(), "$2".to_string(), "$3".to_string()];
        let mut idx = 4;

        // Add role column if specified
        let role_val = if let Some(rc) = role_col {
            let role_key = rc.replace('_', "-");
            if let Some(v) = args.get(&role_key).or_else(|| args.get("role")) {
                columns.push(rc.to_string());
                placeholders.push(format!("${}", idx));
                idx += 1;
                Some(v.as_string()?.to_string())
            } else {
                None
            }
        } else {
            None
        };

        // Add other optional columns
        for (key, _value) in args {
            if let Some((db_col, _db_type)) = resolve_column(junction, key) {
                if db_col == pk_col || db_col == from_col || db_col == to_col {
                    continue;
                }
                if role_col.is_some() && (db_col == role_col.unwrap() || key == "role") {
                    continue;
                }
                columns.push(db_col.to_string());
                placeholders.push(format!("${}", idx));
                idx += 1;
            }
        }

        let sql = format!(
            "INSERT INTO {} ({}) VALUES ({}) RETURNING {}",
            junction,
            columns.join(", "),
            placeholders.join(", "),
            pk_col
        );

        let mut query = sqlx::query_scalar::<_, Uuid>(&sql)
            .bind(new_id)
            .bind(from_val)
            .bind(to_val);

        if let Some(rv) = &role_val {
            query = query.bind(rv);
        }

        let returned_id = query.fetch_one(&self.pool).await?;
        Ok(ExecutionResult::Uuid(returned_id))
    }

    #[cfg(feature = "database")]
    async fn execute_unlink(
        &self,
        junction: &str,
        from_col: &str,
        to_col: &str,
        args: &HashMap<String, ResolvedValue>,
    ) -> Result<ExecutionResult> {
        let from_key = from_col.replace('_', "-");
        let to_key = to_col.replace('_', "-");

        let from_val = args
            .get(&from_key)
            .ok_or_else(|| anyhow!("Missing {} argument", from_key))?
            .as_uuid()?;
        let to_val = args
            .get(&to_key)
            .ok_or_else(|| anyhow!("Missing {} argument", to_key))?
            .as_uuid()?;

        let sql = format!(
            "DELETE FROM {} WHERE {} = $1 AND {} = $2",
            junction, from_col, to_col
        );

        let result = sqlx::query(&sql)
            .bind(from_val)
            .bind(to_val)
            .execute(&self.pool)
            .await?;

        Ok(ExecutionResult::Affected(result.rows_affected()))
    }

    #[cfg(feature = "database")]
    async fn execute_list_by_fk(
        &self,
        table: &str,
        fk_col: &str,
        args: &HashMap<String, ResolvedValue>,
    ) -> Result<ExecutionResult> {
        let fk_key = fk_col.replace('_', "-");
        let fk_val = args
            .get(&fk_key)
            .ok_or_else(|| anyhow!("Missing {} argument", fk_key))?
            .as_uuid()?;

        let conditions = vec![format!("{} = $1", fk_col)];

        let sql = format!("SELECT * FROM {} WHERE {}", table, conditions.join(" AND "));

        let rows = sqlx::query(&sql).bind(fk_val).fetch_all(&self.pool).await?;

        let records: Result<Vec<_>> = rows.iter().map(row_to_json).collect();
        Ok(ExecutionResult::RecordSet(records?))
    }

    #[cfg(feature = "database")]
    async fn execute_select_with_join(
        &self,
        primary_table: &str,
        join_table: &str,
        join_col: &str,
        args: &HashMap<String, ResolvedValue>,
    ) -> Result<ExecutionResult> {
        let primary_pk = get_pk_column(primary_table)
            .ok_or_else(|| anyhow!("Unknown table: {}", primary_table))?;

        // Build the join query
        let sql = format!(
            "SELECT p.* FROM {} p \
             INNER JOIN {} j ON p.{} = j.{} \
             WHERE j.entity_id = $1",
            primary_table, join_table, primary_pk, join_col
        );

        let entity_id = args
            .get("entity-id")
            .ok_or_else(|| anyhow!("Missing entity-id argument"))?
            .as_uuid()?;

        let rows = sqlx::query(&sql)
            .bind(entity_id)
            .fetch_all(&self.pool)
            .await?;

        let records: Result<Vec<_>> = rows.iter().map(row_to_json).collect();
        Ok(ExecutionResult::RecordSet(records?))
    }
}

// ============================================================================
// Helper Types
// ============================================================================

/// A resolved value with references replaced by UUIDs
#[derive(Debug, Clone)]
pub enum ResolvedValue {
    String(String),
    Integer(i64),
    Decimal(rust_decimal::Decimal),
    Boolean(bool),
    Null,
    Uuid(Uuid),
    List(Vec<ResolvedValue>),
    Map(HashMap<String, ResolvedValue>),
}

impl ResolvedValue {
    pub fn as_uuid(&self) -> Result<Uuid> {
        match self {
            ResolvedValue::Uuid(u) => Ok(*u),
            ResolvedValue::String(s) => {
                Uuid::parse_str(s).map_err(|_| anyhow!("Invalid UUID: {}", s))
            }
            _ => bail!("Cannot convert to UUID"),
        }
    }

    pub fn as_string(&self) -> Result<&str> {
        match self {
            ResolvedValue::String(s) => Ok(s),
            _ => bail!("Cannot convert to string"),
        }
    }

    fn to_bind_value(&self) -> BindValue {
        match self {
            ResolvedValue::String(s) => BindValue::String(s.clone()),
            ResolvedValue::Integer(i) => BindValue::Integer(*i),
            ResolvedValue::Decimal(d) => BindValue::Decimal(*d),
            ResolvedValue::Boolean(b) => BindValue::Boolean(*b),
            ResolvedValue::Null => BindValue::Null,
            ResolvedValue::Uuid(u) => BindValue::Uuid(*u),
            ResolvedValue::List(_) => BindValue::Null, // TODO: handle arrays
            ResolvedValue::Map(_) => {
                // For maps, we'd need to serialize properly - for now use null
                BindValue::Null
            }
        }
    }
}

/// Enum for dynamic SQL binding
#[derive(Debug, Clone)]
enum BindValue {
    String(String),
    Integer(i64),
    Decimal(rust_decimal::Decimal),
    Boolean(bool),
    Uuid(Uuid),
    Json(JsonValue),
    Null,
}

#[cfg(feature = "database")]
fn bind_value_to_query<'q>(
    query: sqlx::query::QueryScalar<'q, sqlx::Postgres, Uuid, sqlx::postgres::PgArguments>,
    bv: &BindValue,
) -> sqlx::query::QueryScalar<'q, sqlx::Postgres, Uuid, sqlx::postgres::PgArguments> {
    match bv {
        BindValue::String(s) => query.bind(s.clone()),
        BindValue::Integer(i) => query.bind(*i),
        BindValue::Decimal(d) => query.bind(*d),
        BindValue::Boolean(b) => query.bind(*b),
        BindValue::Uuid(u) => query.bind(*u),
        BindValue::Json(j) => query.bind(j.clone()),
        BindValue::Null => query.bind(Option::<String>::None),
    }
}

#[cfg(feature = "database")]
fn bind_value_to_query_regular<'q>(
    query: sqlx::query::Query<'q, sqlx::Postgres, sqlx::postgres::PgArguments>,
    bv: &BindValue,
) -> sqlx::query::Query<'q, sqlx::Postgres, sqlx::postgres::PgArguments> {
    match bv {
        BindValue::String(s) => query.bind(s.clone()),
        BindValue::Integer(i) => query.bind(*i),
        BindValue::Decimal(d) => query.bind(*d),
        BindValue::Boolean(b) => query.bind(*b),
        BindValue::Uuid(u) => query.bind(*u),
        BindValue::Json(j) => query.bind(j.clone()),
        BindValue::Null => query.bind(Option::<String>::None),
    }
}

/// Convert a database row to JSON
#[cfg(feature = "database")]
fn row_to_json(row: &sqlx::postgres::PgRow) -> Result<JsonValue> {
    use sqlx::{Column, Row, TypeInfo};

    let mut map = serde_json::Map::new();

    for column in row.columns() {
        let name = column.name();
        let type_name = column.type_info().name();
        let value: Option<JsonValue> = match type_name {
            "UUID" => row
                .try_get::<Option<Uuid>, _>(name)
                .ok()
                .flatten()
                .map(|u| JsonValue::String(u.to_string())),
            "TEXT" | "VARCHAR" => row
                .try_get::<Option<String>, _>(name)
                .ok()
                .flatten()
                .map(JsonValue::String),
            "INT4" | "INT8" => row
                .try_get::<Option<i64>, _>(name)
                .ok()
                .flatten()
                .map(|i| JsonValue::Number(i.into())),
            "BOOL" => row
                .try_get::<Option<bool>, _>(name)
                .ok()
                .flatten()
                .map(JsonValue::Bool),
            "JSONB" | "JSON" => row.try_get::<Option<JsonValue>, _>(name).ok().flatten(),
            "TIMESTAMPTZ" | "TIMESTAMP" => row
                .try_get::<Option<chrono::DateTime<chrono::Utc>>, _>(name)
                .ok()
                .flatten()
                .map(|dt| JsonValue::String(dt.to_rfc3339())),
            "DATE" => row
                .try_get::<Option<chrono::NaiveDate>, _>(name)
                .ok()
                .flatten()
                .map(|d| JsonValue::String(d.to_string())),
            "NUMERIC" => row
                .try_get::<Option<rust_decimal::Decimal>, _>(name)
                .ok()
                .flatten()
                .map(|d| JsonValue::String(d.to_string())),
            _ => None,
        };

        map.insert(name.to_string(), value.unwrap_or(JsonValue::Null));
    }

    Ok(JsonValue::Object(map))
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_execution_context_bind_resolve() {
        let mut ctx = ExecutionContext::new();
        let id = Uuid::new_v4();
        ctx.bind("test", id);
        assert_eq!(ctx.resolve("test"), Some(id));
        assert_eq!(ctx.resolve("nonexistent"), None);
    }

    #[test]
    fn test_resolved_value_as_uuid() {
        let uuid = Uuid::new_v4();
        let rv = ResolvedValue::Uuid(uuid);
        assert_eq!(rv.as_uuid().unwrap(), uuid);

        let rv_str = ResolvedValue::String(uuid.to_string());
        assert_eq!(rv_str.as_uuid().unwrap(), uuid);
    }
}
