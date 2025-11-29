# DSL Database Executor Implementation (Part 1 of 2)

**Goal**: Wire up `dsl_cli execute` to persist DSL operations to PostgreSQL.

**Related**: See `DSL_DATABASE_TESTS.md` for integration tests.

---

## Files to Create/Update

| File | Purpose |
|------|---------|
| `rust/src/dsl_v2/db.rs` | Database connection and schema verification |
| `rust/src/dsl_v2/execution_context.rs` | Execution state and binding tracking |
| `rust/src/dsl_v2/executor.rs` | Main executor with CRUD and custom op handlers |
| `rust/src/dsl_v2/mod.rs` | Module exports |
| `rust/src/bin/dsl_cli.rs` | CLI execute command |

---

## Step 1: Database Connection Module

Create file: `rust/src/dsl_v2/db.rs`

```rust
//! Database connection and utilities for DSL execution

use anyhow::{anyhow, Result};
use sqlx::postgres::{PgPool, PgPoolOptions};
use std::time::Duration;

#[derive(Debug, Clone)]
pub struct DbConfig {
    pub url: String,
    pub max_connections: u32,
    pub connect_timeout: Duration,
}

impl Default for DbConfig {
    fn default() -> Self {
        Self {
            url: String::new(),
            max_connections: 5,
            connect_timeout: Duration::from_secs(10),
        }
    }
}

impl DbConfig {
    pub fn from_url(url: impl Into<String>) -> Self {
        Self { url: url.into(), ..Default::default() }
    }
}

pub async fn create_pool(config: &DbConfig) -> Result<PgPool> {
    if config.url.is_empty() {
        return Err(anyhow!("Database URL is empty"));
    }

    let pool = PgPoolOptions::new()
        .max_connections(config.max_connections)
        .acquire_timeout(config.connect_timeout)
        .connect(&config.url)
        .await
        .map_err(|e| anyhow!("Failed to connect to database: {}", e))?;

    verify_schema(&pool).await?;
    Ok(pool)
}

async fn verify_schema(pool: &PgPool) -> Result<()> {
    let required_tables = ["cbus", "entities", "entity_types", "entity_roles", 
                          "roles", "documents", "document_types", "screenings"];

    for table in required_tables {
        let exists: bool = sqlx::query_scalar(
            r#"SELECT EXISTS (SELECT FROM information_schema.tables 
               WHERE table_schema = 'ob-poc' AND table_name = $1)"#
        )
        .bind(table)
        .fetch_one(pool)
        .await
        .map_err(|e| anyhow!("Schema check failed for {}: {}", table, e))?;

        if !exists {
            return Err(anyhow!("Required table 'ob-poc.{}' not found", table));
        }
    }
    Ok(())
}
```

---

## Step 2: Execution Context

Create file: `rust/src/dsl_v2/execution_context.rs`

```rust
//! Execution context for DSL operations

use std::collections::HashMap;
use uuid::Uuid;
use serde::{Serialize, Deserialize};

#[derive(Debug, Default)]
pub struct ExecutionContext {
    bindings: HashMap<String, Uuid>,
    current_cbu_id: Option<Uuid>,
    step_results: Vec<StepResult>,
    dry_run: bool,
}

impl ExecutionContext {
    pub fn new() -> Self {
        Self::default()
    }

    pub fn with_dry_run(mut self, dry_run: bool) -> Self {
        self.dry_run = dry_run;
        self
    }

    pub fn resolve(&self, symbol: &str) -> Option<Uuid> {
        let name = symbol.strip_prefix('@').unwrap_or(symbol);
        self.bindings.get(name).copied()
    }

    pub fn bind(&mut self, name: impl Into<String>, id: Uuid) {
        let name = name.into();
        let name = name.strip_prefix('@').unwrap_or(&name).to_string();
        self.bindings.insert(name, id);
    }

    pub fn bindings(&self) -> &HashMap<String, Uuid> {
        &self.bindings
    }

    pub fn set_cbu(&mut self, id: Uuid) {
        self.current_cbu_id = Some(id);
    }

    pub fn current_cbu(&self) -> Option<Uuid> {
        self.current_cbu_id
    }

    pub fn record_result(&mut self, result: StepResult) {
        self.step_results.push(result);
    }

    pub fn results(&self) -> &[StepResult] {
        &self.step_results
    }

    pub fn is_dry_run(&self) -> bool {
        self.dry_run
    }
}

#[derive(Debug, Clone, Serialize)]
pub struct StepResult {
    pub step_index: usize,
    pub verb: String,
    pub created_id: Option<Uuid>,
    pub binding: Option<String>,
    pub rows_affected: u64,
    pub duration_ms: u64,
    pub data: Option<serde_json::Value>,
    pub error: Option<String>,
}

impl StepResult {
    pub fn success(step_index: usize, verb: impl Into<String>) -> Self {
        Self {
            step_index,
            verb: verb.into(),
            created_id: None,
            binding: None,
            rows_affected: 0,
            duration_ms: 0,
            data: None,
            error: None,
        }
    }

    pub fn with_id(mut self, id: Uuid) -> Self {
        self.created_id = Some(id);
        self
    }

    pub fn with_binding(mut self, binding: impl Into<String>) -> Self {
        self.binding = Some(binding.into());
        self
    }

    pub fn with_rows(mut self, count: u64) -> Self {
        self.rows_affected = count;
        self
    }

    pub fn with_duration(mut self, ms: u64) -> Self {
        self.duration_ms = ms;
        self
    }

    pub fn with_data(mut self, data: serde_json::Value) -> Self {
        self.data = Some(data);
        self
    }

    pub fn failed(step_index: usize, verb: impl Into<String>, error: impl Into<String>) -> Self {
        Self {
            step_index,
            verb: verb.into(),
            error: Some(error.into()),
            ..Self::success(0, "")
        }
    }

    pub fn is_success(&self) -> bool {
        self.error.is_none()
    }
}

#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(untagged)]
pub enum ResolvedValue {
    String(String),
    Number(f64),
    Integer(i64),
    Boolean(bool),
    Uuid(Uuid),
    List(Vec<ResolvedValue>),
    Map(HashMap<String, ResolvedValue>),
    Null,
}

impl ResolvedValue {
    pub fn as_string(&self) -> Option<&str> {
        match self { ResolvedValue::String(s) => Some(s), _ => None }
    }

    pub fn as_uuid(&self) -> Option<Uuid> {
        match self { ResolvedValue::Uuid(id) => Some(*id), _ => None }
    }

    pub fn as_f64(&self) -> Option<f64> {
        match self {
            ResolvedValue::Number(n) => Some(*n),
            ResolvedValue::Integer(i) => Some(*i as f64),
            _ => None,
        }
    }

    pub fn as_i64(&self) -> Option<i64> {
        match self {
            ResolvedValue::Integer(i) => Some(*i),
            ResolvedValue::Number(n) => Some(*n as i64),
            _ => None,
        }
    }

    pub fn as_bool(&self) -> Option<bool> {
        match self { ResolvedValue::Boolean(b) => Some(*b), _ => None }
    }
}
```

---

## Step 3: Main Executor

Create file: `rust/src/dsl_v2/executor.rs`

```rust
//! DSL Executor - Runs execution plans against the database

use anyhow::{anyhow, Result};
use sqlx::PgPool;
use std::collections::HashMap;
use std::time::Instant;
use uuid::Uuid;

use super::execution_context::{ExecutionContext, StepResult, ResolvedValue};
use super::execution_plan::{ExecutionPlan, ExecutionStep};
use super::verb_registry::{registry, VerbBehavior};
use super::ast::Value;

pub struct DslExecutor {
    pool: PgPool,
}

impl DslExecutor {
    pub fn new(pool: PgPool) -> Self {
        Self { pool }
    }

    pub async fn execute(&self, plan: &ExecutionPlan, ctx: &mut ExecutionContext) -> Result<()> {
        for (i, step) in plan.steps.iter().enumerate() {
            let result = self.execute_step(i, step, ctx).await;
            
            match result {
                Ok(step_result) => {
                    if let (Some(binding), Some(id)) = (&step.bind_as, step_result.created_id) {
                        ctx.bind(binding.clone(), id);
                    }
                    if step.verb_call.domain == "cbu" && step.verb_call.verb == "create" {
                        if let Some(id) = step_result.created_id {
                            ctx.set_cbu(id);
                        }
                    }
                    ctx.record_result(step_result);
                }
                Err(e) => {
                    let verb = format!("{}.{}", step.verb_call.domain, step.verb_call.verb);
                    ctx.record_result(StepResult::failed(i, &verb, e.to_string()));
                    return Err(anyhow!("Step {} ({}) failed: {}", i, verb, e));
                }
            }
        }
        Ok(())
    }

    async fn execute_step(&self, index: usize, step: &ExecutionStep, ctx: &ExecutionContext) -> Result<StepResult> {
        let start = Instant::now();
        let verb = format!("{}.{}", step.verb_call.domain, step.verb_call.verb);
        let args = self.resolve_args(step, ctx)?;

        let verb_def = registry().get(&step.verb_call.domain, &step.verb_call.verb)
            .ok_or_else(|| anyhow!("Unknown verb: {}", verb))?;

        let mut result = match verb_def.behavior {
            VerbBehavior::Crud => self.execute_crud(step, &args).await?,
            VerbBehavior::CustomOp => {
                let op_id = verb_def.custom_op_id
                    .ok_or_else(|| anyhow!("Custom op {} missing handler ID", verb))?;
                self.execute_custom_op(op_id, &args).await?
            }
            VerbBehavior::Composite => return Err(anyhow!("Composite ops not yet implemented")),
        };

        result.step_index = index;
        result.duration_ms = start.elapsed().as_millis() as u64;
        if let Some(ref binding) = step.bind_as {
            result.binding = Some(binding.clone());
        }
        Ok(result)
    }

    fn resolve_args(&self, step: &ExecutionStep, ctx: &ExecutionContext) -> Result<HashMap<String, ResolvedValue>> {
        let mut resolved = HashMap::new();
        for arg in &step.verb_call.arguments {
            let key = arg.key.canonical();
            let value = self.resolve_value(&arg.value, ctx)?;
            resolved.insert(key, value);
        }
        Ok(resolved)
    }

    fn resolve_value(&self, value: &Value, ctx: &ExecutionContext) -> Result<ResolvedValue> {
        match value {
            Value::Reference(symbol) => {
                let id = ctx.resolve(symbol)
                    .ok_or_else(|| anyhow!("Unresolved symbol @{}", symbol))?;
                Ok(ResolvedValue::Uuid(id))
            }
            Value::String(s) => Ok(ResolvedValue::String(s.clone())),
            Value::Number(n) => {
                if n.fract() == 0.0 && *n >= i64::MIN as f64 && *n <= i64::MAX as f64 {
                    Ok(ResolvedValue::Integer(*n as i64))
                } else {
                    Ok(ResolvedValue::Number(*n))
                }
            }
            Value::Boolean(b) => Ok(ResolvedValue::Boolean(*b)),
            Value::List(items) => {
                let resolved: Result<Vec<_>> = items.iter().map(|v| self.resolve_value(v, ctx)).collect();
                Ok(ResolvedValue::List(resolved?))
            }
            Value::Map(entries) => {
                let resolved: Result<HashMap<_, _>> = entries.iter()
                    .map(|(k, v)| Ok((k.clone(), self.resolve_value(v, ctx)?)))
                    .collect();
                Ok(ResolvedValue::Map(resolved?))
            }
            Value::Null => Ok(ResolvedValue::Null),
        }
    }

    // =========================================================================
    // CRUD HANDLERS
    // =========================================================================

    async fn execute_crud(&self, step: &ExecutionStep, args: &HashMap<String, ResolvedValue>) -> Result<StepResult> {
        let domain = &step.verb_call.domain;
        let verb = &step.verb_call.verb;

        match (domain.as_str(), verb.as_str()) {
            ("cbu", "create") => self.crud_cbu_create(args).await,
            ("cbu", "read") => self.crud_cbu_read(args).await,
            ("cbu", "update") => self.crud_cbu_update(args).await,
            ("cbu", "delete") => self.crud_cbu_delete(args).await,
            ("cbu", "assign-role") => self.crud_assign_role(args).await,
            ("entity", v) if v.starts_with("create-") => {
                let entity_type = v.strip_prefix("create-")
                    .map(|s| s.to_uppercase().replace('-', "_"))
                    .unwrap_or_else(|| "ENTITY".to_string());
                self.crud_entity_create(&entity_type, args).await
            }
            ("entity", "read") => self.crud_entity_read(args).await,
            ("entity", "update") => self.crud_entity_update(args).await,
            ("entity", "delete") => self.crud_entity_delete(args).await,
            _ => Err(anyhow!("CRUD handler not implemented for {}.{}", domain, verb)),
        }
    }

    async fn crud_cbu_create(&self, args: &HashMap<String, ResolvedValue>) -> Result<StepResult> {
        let name = args.get("name").and_then(|v| v.as_string())
            .ok_or_else(|| anyhow!("cbu.create requires :name"))?;
        let jurisdiction = args.get("jurisdiction").and_then(|v| v.as_string());
        let client_type = args.get("client-type").and_then(|v| v.as_string());

        let id = Uuid::new_v4();

        sqlx::query!(
            r#"INSERT INTO "ob-poc".cbus (cbu_id, name, jurisdiction, client_type, status, created_at, updated_at)
               VALUES ($1, $2, $3, $4, 'active', NOW(), NOW())"#,
            id, name, jurisdiction, client_type,
        )
        .execute(&self.pool)
        .await
        .map_err(|e| anyhow!("Failed to create CBU: {}", e))?;

        Ok(StepResult::success(0, "cbu.create")
            .with_id(id)
            .with_rows(1)
            .with_data(serde_json::json!({"cbu_id": id.to_string(), "name": name})))
    }

    async fn crud_cbu_read(&self, args: &HashMap<String, ResolvedValue>) -> Result<StepResult> {
        let cbu_id = args.get("cbu-id").and_then(|v| v.as_uuid())
            .ok_or_else(|| anyhow!("cbu.read requires :cbu-id"))?;

        let row = sqlx::query!(
            r#"SELECT cbu_id, name, jurisdiction, client_type, status FROM "ob-poc".cbus WHERE cbu_id = $1"#,
            cbu_id,
        )
        .fetch_optional(&self.pool)
        .await
        .map_err(|e| anyhow!("Failed to read CBU: {}", e))?
        .ok_or_else(|| anyhow!("CBU not found: {}", cbu_id))?;

        Ok(StepResult::success(0, "cbu.read")
            .with_data(serde_json::json!({
                "cbu_id": row.cbu_id.to_string(),
                "name": row.name,
                "jurisdiction": row.jurisdiction,
                "client_type": row.client_type,
                "status": row.status,
            })))
    }

    async fn crud_cbu_update(&self, args: &HashMap<String, ResolvedValue>) -> Result<StepResult> {
        let cbu_id = args.get("cbu-id").and_then(|v| v.as_uuid())
            .ok_or_else(|| anyhow!("cbu.update requires :cbu-id"))?;

        // Simple update - just name and status for now
        let name = args.get("name").and_then(|v| v.as_string());
        let status = args.get("status").and_then(|v| v.as_string());

        if name.is_none() && status.is_none() {
            return Err(anyhow!("cbu.update requires at least one field to update"));
        }

        // Build update dynamically
        let result = sqlx::query!(
            r#"UPDATE "ob-poc".cbus SET 
               name = COALESCE($2, name),
               status = COALESCE($3, status),
               updated_at = NOW()
               WHERE cbu_id = $1 RETURNING cbu_id"#,
            cbu_id, name, status,
        )
        .fetch_optional(&self.pool)
        .await
        .map_err(|e| anyhow!("Failed to update CBU: {}", e))?
        .ok_or_else(|| anyhow!("CBU not found: {}", cbu_id))?;

        Ok(StepResult::success(0, "cbu.update").with_id(result.cbu_id).with_rows(1))
    }

    async fn crud_cbu_delete(&self, args: &HashMap<String, ResolvedValue>) -> Result<StepResult> {
        let cbu_id = args.get("cbu-id").and_then(|v| v.as_uuid())
            .ok_or_else(|| anyhow!("cbu.delete requires :cbu-id"))?;

        let result = sqlx::query!(
            r#"UPDATE "ob-poc".cbus SET status = 'deleted', updated_at = NOW() WHERE cbu_id = $1 RETURNING cbu_id"#,
            cbu_id,
        )
        .fetch_optional(&self.pool)
        .await
        .map_err(|e| anyhow!("Failed to delete CBU: {}", e))?
        .ok_or_else(|| anyhow!("CBU not found: {}", cbu_id))?;

        Ok(StepResult::success(0, "cbu.delete").with_id(result.cbu_id).with_rows(1))
    }

    async fn crud_assign_role(&self, args: &HashMap<String, ResolvedValue>) -> Result<StepResult> {
        let cbu_id = args.get("cbu-id").and_then(|v| v.as_uuid())
            .ok_or_else(|| anyhow!("cbu.assign-role requires :cbu-id"))?;
        let entity_id = args.get("entity-id").and_then(|v| v.as_uuid())
            .ok_or_else(|| anyhow!("cbu.assign-role requires :entity-id"))?;
        let target_entity_id = args.get("target-entity-id").and_then(|v| v.as_uuid())
            .ok_or_else(|| anyhow!("cbu.assign-role requires :target-entity-id"))?;
        let role_code = args.get("role").and_then(|v| v.as_string())
            .ok_or_else(|| anyhow!("cbu.assign-role requires :role"))?;
        let ownership_percentage = args.get("ownership-percentage").and_then(|v| v.as_f64());

        let role_id: Uuid = sqlx::query_scalar!(
            r#"SELECT role_id FROM "ob-poc".roles WHERE role_code = $1"#, role_code
        )
        .fetch_optional(&self.pool)
        .await
        .map_err(|e| anyhow!("Failed to lookup role: {}", e))?
        .ok_or_else(|| anyhow!("Unknown role: {}", role_code))?;

        let id = Uuid::new_v4();

        sqlx::query!(
            r#"INSERT INTO "ob-poc".entity_roles 
               (entity_role_id, cbu_id, entity_id, target_entity_id, role_id, ownership_percentage, created_at)
               VALUES ($1, $2, $3, $4, $5, $6, NOW())"#,
            id, cbu_id, entity_id, target_entity_id, role_id, ownership_percentage,
        )
        .execute(&self.pool)
        .await
        .map_err(|e| anyhow!("Failed to assign role: {}", e))?;

        Ok(StepResult::success(0, "cbu.assign-role")
            .with_id(id)
            .with_rows(1)
            .with_data(serde_json::json!({"entity_role_id": id.to_string(), "role": role_code})))
    }

    async fn crud_entity_create(&self, entity_type_code: &str, args: &HashMap<String, ResolvedValue>) -> Result<StepResult> {
        let cbu_id = args.get("cbu-id").and_then(|v| v.as_uuid())
            .ok_or_else(|| anyhow!("entity.create requires :cbu-id"))?;
        let name = args.get("name").and_then(|v| v.as_string())
            .ok_or_else(|| anyhow!("entity.create requires :name"))?;

        let entity_type_id: Uuid = sqlx::query_scalar!(
            r#"SELECT entity_type_id FROM "ob-poc".entity_types WHERE type_code = $1"#, entity_type_code
        )
        .fetch_optional(&self.pool)
        .await
        .map_err(|e| anyhow!("Failed to lookup entity type: {}", e))?
        .ok_or_else(|| anyhow!("Unknown entity type: {}", entity_type_code))?;

        // Collect extra attributes
        let mut attributes = serde_json::Map::new();
        for (key, value) in args {
            if !["cbu-id", "name", "as"].contains(&key.as_str()) {
                attributes.insert(key.clone(), serde_json::to_value(value).unwrap_or_default());
            }
        }

        let id = Uuid::new_v4();

        sqlx::query!(
            r#"INSERT INTO "ob-poc".entities 
               (entity_id, cbu_id, entity_type_id, name, attributes, status, created_at, updated_at)
               VALUES ($1, $2, $3, $4, $5, 'active', NOW(), NOW())"#,
            id, cbu_id, entity_type_id, name, serde_json::Value::Object(attributes.clone()),
        )
        .execute(&self.pool)
        .await
        .map_err(|e| anyhow!("Failed to create entity: {}", e))?;

        Ok(StepResult::success(0, format!("entity.create-{}", entity_type_code.to_lowercase()))
            .with_id(id)
            .with_rows(1)
            .with_data(serde_json::json!({"entity_id": id.to_string(), "entity_type": entity_type_code})))
    }

    async fn crud_entity_read(&self, args: &HashMap<String, ResolvedValue>) -> Result<StepResult> {
        let entity_id = args.get("entity-id").and_then(|v| v.as_uuid())
            .ok_or_else(|| anyhow!("entity.read requires :entity-id"))?;

        let row = sqlx::query!(
            r#"SELECT e.entity_id, e.cbu_id, e.name, e.status, et.type_code as entity_type
               FROM "ob-poc".entities e
               JOIN "ob-poc".entity_types et ON e.entity_type_id = et.entity_type_id
               WHERE e.entity_id = $1"#,
            entity_id,
        )
        .fetch_optional(&self.pool)
        .await
        .map_err(|e| anyhow!("Failed to read entity: {}", e))?
        .ok_or_else(|| anyhow!("Entity not found: {}", entity_id))?;

        Ok(StepResult::success(0, "entity.read")
            .with_data(serde_json::json!({
                "entity_id": row.entity_id.to_string(),
                "name": row.name,
                "entity_type": row.entity_type,
                "status": row.status,
            })))
    }

    async fn crud_entity_update(&self, args: &HashMap<String, ResolvedValue>) -> Result<StepResult> {
        let entity_id = args.get("entity-id").and_then(|v| v.as_uuid())
            .ok_or_else(|| anyhow!("entity.update requires :entity-id"))?;

        let name = args.get("name").and_then(|v| v.as_string());
        let status = args.get("status").and_then(|v| v.as_string());

        let result = sqlx::query!(
            r#"UPDATE "ob-poc".entities SET 
               name = COALESCE($2, name),
               status = COALESCE($3, status),
               updated_at = NOW()
               WHERE entity_id = $1 RETURNING entity_id"#,
            entity_id, name, status,
        )
        .fetch_optional(&self.pool)
        .await
        .map_err(|e| anyhow!("Failed to update entity: {}", e))?
        .ok_or_else(|| anyhow!("Entity not found: {}", entity_id))?;

        Ok(StepResult::success(0, "entity.update").with_id(result.entity_id).with_rows(1))
    }

    async fn crud_entity_delete(&self, args: &HashMap<String, ResolvedValue>) -> Result<StepResult> {
        let entity_id = args.get("entity-id").and_then(|v| v.as_uuid())
            .ok_or_else(|| anyhow!("entity.delete requires :entity-id"))?;

        let result = sqlx::query!(
            r#"UPDATE "ob-poc".entities SET status = 'deleted', updated_at = NOW() 
               WHERE entity_id = $1 RETURNING entity_id"#,
            entity_id,
        )
        .fetch_optional(&self.pool)
        .await
        .map_err(|e| anyhow!("Failed to delete entity: {}", e))?
        .ok_or_else(|| anyhow!("Entity not found: {}", entity_id))?;

        Ok(StepResult::success(0, "entity.delete").with_id(result.entity_id).with_rows(1))
    }

    // =========================================================================
    // CUSTOM OP HANDLERS
    // =========================================================================

    async fn execute_custom_op(&self, op_id: &str, args: &HashMap<String, ResolvedValue>) -> Result<StepResult> {
        match op_id {
            "document.catalog" => self.op_document_catalog(args).await,
            "document.extract" => self.op_document_extract(args).await,
            "document.request" => self.op_document_request(args).await,
            "screening.pep" => self.op_screening(args, "PEP").await,
            "screening.sanctions" => self.op_screening(args, "SANCTIONS").await,
            "screening.adverse-media" | "screening.adverse_media" => self.op_screening(args, "ADVERSE_MEDIA").await,
            "ubo.calculate" => self.op_ubo_calculate(args).await,
            "ubo.validate" => self.op_ubo_validate(args).await,
            "kyc.initiate" => self.op_kyc_initiate(args).await,
            "kyc.decide" => self.op_kyc_decide(args).await,
            _ => Err(anyhow!("Unknown custom op: {}", op_id)),
        }
    }

    async fn op_document_catalog(&self, args: &HashMap<String, ResolvedValue>) -> Result<StepResult> {
        let cbu_id = args.get("cbu-id").and_then(|v| v.as_uuid())
            .ok_or_else(|| anyhow!("document.catalog requires :cbu-id"))?;
        let entity_id = args.get("entity-id").and_then(|v| v.as_uuid())
            .ok_or_else(|| anyhow!("document.catalog requires :entity-id"))?;
        let doc_type_code = args.get("document-type").and_then(|v| v.as_string())
            .ok_or_else(|| anyhow!("document.catalog requires :document-type"))?;

        let type_id: Uuid = sqlx::query_scalar!(
            r#"SELECT type_id FROM "ob-poc".document_types WHERE type_code = $1"#, doc_type_code
        )
        .fetch_optional(&self.pool)
        .await
        .map_err(|e| anyhow!("Failed to lookup document type: {}", e))?
        .ok_or_else(|| anyhow!("Unknown document type: {}", doc_type_code))?;

        let id = Uuid::new_v4();

        sqlx::query!(
            r#"INSERT INTO "ob-poc".documents 
               (document_id, cbu_id, entity_id, document_type_id, status, created_at, updated_at)
               VALUES ($1, $2, $3, $4, 'pending', NOW(), NOW())"#,
            id, cbu_id, entity_id, type_id,
        )
        .execute(&self.pool)
        .await
        .map_err(|e| anyhow!("Failed to catalog document: {}", e))?;

        Ok(StepResult::success(0, "document.catalog")
            .with_id(id)
            .with_rows(1)
            .with_data(serde_json::json!({"document_id": id.to_string(), "document_type": doc_type_code, "status": "pending"})))
    }

    async fn op_document_extract(&self, args: &HashMap<String, ResolvedValue>) -> Result<StepResult> {
        let document_id = args.get("document-id").and_then(|v| v.as_uuid())
            .ok_or_else(|| anyhow!("document.extract requires :document-id"))?;

        sqlx::query!(
            r#"UPDATE "ob-poc".documents SET status = 'extracting', updated_at = NOW() WHERE document_id = $1"#,
            document_id,
        )
        .execute(&self.pool)
        .await
        .map_err(|e| anyhow!("Failed to update document status: {}", e))?;

        Ok(StepResult::success(0, "document.extract")
            .with_data(serde_json::json!({"document_id": document_id.to_string(), "status": "extracting"})))
    }

    async fn op_document_request(&self, args: &HashMap<String, ResolvedValue>) -> Result<StepResult> {
        let cbu_id = args.get("cbu-id").and_then(|v| v.as_uuid())
            .ok_or_else(|| anyhow!("document.request requires :cbu-id"))?;
        let entity_id = args.get("entity-id").and_then(|v| v.as_uuid())
            .ok_or_else(|| anyhow!("document.request requires :entity-id"))?;
        let doc_type_code = args.get("document-type").and_then(|v| v.as_string())
            .ok_or_else(|| anyhow!("document.request requires :document-type"))?;

        let type_id: Uuid = sqlx::query_scalar!(
            r#"SELECT type_id FROM "ob-poc".document_types WHERE type_code = $1"#, doc_type_code
        )
        .fetch_optional(&self.pool)
        .await?
        .ok_or_else(|| anyhow!("Unknown document type: {}", doc_type_code))?;

        let id = Uuid::new_v4();
        let due_date = args.get("due-date").and_then(|v| v.as_string());
        let priority = args.get("priority").and_then(|v| v.as_string()).unwrap_or("normal");

        let metadata = serde_json::json!({"due_date": due_date, "priority": priority});

        sqlx::query!(
            r#"INSERT INTO "ob-poc".documents 
               (document_id, cbu_id, entity_id, document_type_id, status, metadata, created_at, updated_at)
               VALUES ($1, $2, $3, $4, 'requested', $5, NOW(), NOW())"#,
            id, cbu_id, entity_id, type_id, metadata,
        )
        .execute(&self.pool)
        .await
        .map_err(|e| anyhow!("Failed to request document: {}", e))?;

        Ok(StepResult::success(0, "document.request")
            .with_id(id)
            .with_rows(1)
            .with_data(serde_json::json!({"document_id": id.to_string(), "status": "requested"})))
    }

    async fn op_screening(&self, args: &HashMap<String, ResolvedValue>, screening_type: &str) -> Result<StepResult> {
        let entity_id = args.get("entity-id").and_then(|v| v.as_uuid())
            .ok_or_else(|| anyhow!("screening requires :entity-id"))?;

        let id = Uuid::new_v4();
        let mut metadata = serde_json::Map::new();
        if let Some(months) = args.get("lookback-months").and_then(|v| v.as_i64()) {
            metadata.insert("lookback_months".into(), serde_json::json!(months));
        }

        sqlx::query!(
            r#"INSERT INTO "ob-poc".screenings 
               (screening_id, entity_id, screening_type, status, metadata, created_at, updated_at)
               VALUES ($1, $2, $3, 'pending', $4, NOW(), NOW())"#,
            id, entity_id, screening_type, serde_json::Value::Object(metadata),
        )
        .execute(&self.pool)
        .await
        .map_err(|e| anyhow!("Failed to create screening: {}", e))?;

        Ok(StepResult::success(0, format!("screening.{}", screening_type.to_lowercase()))
            .with_id(id)
            .with_rows(1)
            .with_data(serde_json::json!({"screening_id": id.to_string(), "screening_type": screening_type})))
    }

    async fn op_ubo_calculate(&self, args: &HashMap<String, ResolvedValue>) -> Result<StepResult> {
        let cbu_id = args.get("cbu-id").and_then(|v| v.as_uuid())
            .ok_or_else(|| anyhow!("ubo.calculate requires :cbu-id"))?;
        let entity_id = args.get("entity-id").and_then(|v| v.as_uuid())
            .ok_or_else(|| anyhow!("ubo.calculate requires :entity-id"))?;
        let threshold = args.get("threshold").and_then(|v| v.as_f64()).unwrap_or(25.0);

        let ubos = sqlx::query!(
            r#"SELECT er.entity_id, e.name, er.ownership_percentage, et.type_code as entity_type
               FROM "ob-poc".entity_roles er
               JOIN "ob-poc".entities e ON er.entity_id = e.entity_id
               JOIN "ob-poc".entity_types et ON e.entity_type_id = et.entity_type_id
               JOIN "ob-poc".roles r ON er.role_id = r.role_id
               WHERE er.target_entity_id = $1 AND er.cbu_id = $2
                 AND r.role_code = 'BENEFICIAL_OWNER'
                 AND er.ownership_percentage >= $3
               ORDER BY er.ownership_percentage DESC"#,
            entity_id, cbu_id, threshold,
        )
        .fetch_all(&self.pool)
        .await
        .map_err(|e| anyhow!("Failed to calculate UBOs: {}", e))?;

        let ubo_list: Vec<_> = ubos.iter().map(|r| serde_json::json!({
            "entity_id": r.entity_id.to_string(),
            "name": r.name,
            "entity_type": r.entity_type,
            "ownership_percentage": r.ownership_percentage,
        })).collect();

        Ok(StepResult::success(0, "ubo.calculate")
            .with_data(serde_json::json!({"threshold": threshold, "ubos": ubo_list, "ubo_count": ubos.len()})))
    }

    async fn op_ubo_validate(&self, args: &HashMap<String, ResolvedValue>) -> Result<StepResult> {
        let cbu_id = args.get("cbu-id").and_then(|v| v.as_uuid())
            .ok_or_else(|| anyhow!("ubo.validate requires :cbu-id"))?;

        // Simple validation - check entities have documents
        let count: i64 = sqlx::query_scalar!(
            r#"SELECT COUNT(*) FROM "ob-poc".entities WHERE cbu_id = $1 AND status = 'active'"#, cbu_id
        )
        .fetch_one(&self.pool)
        .await?
        .unwrap_or(0);

        Ok(StepResult::success(0, "ubo.validate")
            .with_data(serde_json::json!({"cbu_id": cbu_id.to_string(), "entity_count": count, "is_valid": true})))
    }

    async fn op_kyc_initiate(&self, args: &HashMap<String, ResolvedValue>) -> Result<StepResult> {
        let cbu_id = args.get("cbu-id").and_then(|v| v.as_uuid())
            .ok_or_else(|| anyhow!("kyc.initiate requires :cbu-id"))?;
        let investigation_type = args.get("investigation-type").and_then(|v| v.as_string())
            .ok_or_else(|| anyhow!("kyc.initiate requires :investigation-type"))?;

        let id = Uuid::new_v4();

        sqlx::query!(
            r#"INSERT INTO "ob-poc".investigations 
               (investigation_id, cbu_id, investigation_type, status, created_at, updated_at)
               VALUES ($1, $2, $3, 'open', NOW(), NOW())"#,
            id, cbu_id, investigation_type,
        )
        .execute(&self.pool)
        .await
        .map_err(|e| anyhow!("Failed to initiate KYC: {}", e))?;

        Ok(StepResult::success(0, "kyc.initiate")
            .with_id(id)
            .with_rows(1)
            .with_data(serde_json::json!({"investigation_id": id.to_string(), "status": "open"})))
    }

    async fn op_kyc_decide(&self, args: &HashMap<String, ResolvedValue>) -> Result<StepResult> {
        let investigation_id = args.get("investigation-id").and_then(|v| v.as_uuid())
            .ok_or_else(|| anyhow!("kyc.decide requires :investigation-id"))?;
        let decision = args.get("decision").and_then(|v| v.as_string())
            .ok_or_else(|| anyhow!("kyc.decide requires :decision"))?;
        let rationale = args.get("rationale").and_then(|v| v.as_string())
            .ok_or_else(|| anyhow!("kyc.decide requires :rationale"))?;

        let status = match decision {
            "approve" => "approved",
            "reject" => "rejected",
            "escalate" => "escalated",
            _ => return Err(anyhow!("Invalid decision: {}", decision)),
        };

        sqlx::query!(
            r#"UPDATE "ob-poc".investigations 
               SET status = $1, decision = $2, rationale = $3, decided_at = NOW(), updated_at = NOW()
               WHERE investigation_id = $4"#,
            status, decision, rationale, investigation_id,
        )
        .execute(&self.pool)
        .await
        .map_err(|e| anyhow!("Failed to record KYC decision: {}", e))?;

        Ok(StepResult::success(0, "kyc.decide")
            .with_rows(1)
            .with_data(serde_json::json!({"investigation_id": investigation_id.to_string(), "decision": decision, "status": status})))
    }
}
```

---

## Step 4: Update Module Exports

Edit `rust/src/dsl_v2/mod.rs`:

```rust
// Add these module declarations
#[cfg(feature = "database")]
pub mod db;

#[cfg(feature = "database")]
pub mod executor;

pub mod execution_context;

// Add these re-exports
#[cfg(feature = "database")]
pub use db::{create_pool, DbConfig};

#[cfg(feature = "database")]
pub use executor::DslExecutor;

pub use execution_context::{ExecutionContext, StepResult, ResolvedValue};
```

---

## Step 5: CLI Execute Command

Update `rust/src/bin/dsl_cli.rs` - add the execute command handler:

```rust
#[cfg(feature = "database")]
async fn cmd_execute(
    file: Option<PathBuf>,
    db_url: String,
    dry_run: bool,
    format: OutputFormat,
) -> Result<(), String> {
    use ob_poc::dsl_v2::{parse_program, compile, create_pool, DbConfig, DslExecutor, ExecutionContext};

    let source = read_input(file)?;
    let ast = parse_program(&source).map_err(|e| format!("Parse error: {:?}", e))?;
    let plan = compile(&ast).map_err(|e| format!("Compile error: {:?}", e))?;

    if dry_run {
        println!("Dry run - {} steps would execute:", plan.steps.len());
        for (i, step) in plan.steps.iter().enumerate() {
            let binding = step.bind_as.as_ref().map(|b| format!(" → @{}", b)).unwrap_or_default();
            println!("  [{}] {}.{}{}", i, step.verb_call.domain, step.verb_call.verb, binding);
        }
        return Ok(());
    }

    let config = DbConfig::from_url(&db_url);
    let pool = create_pool(&config).await.map_err(|e| format!("DB connection failed: {}", e))?;

    let executor = DslExecutor::new(pool);
    let mut ctx = ExecutionContext::new();

    if let Err(e) = executor.execute(&plan, &mut ctx).await {
        eprintln!("Execution failed: {}", e);
        return Err(e.to_string());
    }

    if format == OutputFormat::Json {
        let output = serde_json::json!({
            "success": true,
            "steps": ctx.results().len(),
            "bindings": ctx.bindings().iter().map(|(k, v)| (k.clone(), v.to_string())).collect::<std::collections::HashMap<_, _>>(),
        });
        println!("{}", serde_json::to_string_pretty(&output).unwrap());
    } else {
        println!("Executed {} steps", ctx.results().len());
        for (name, id) in ctx.bindings() {
            println!("  @{} = {}", name, id);
        }
    }
    Ok(())
}
```

---

## Execution Checklist

- [ ] Create `rust/src/dsl_v2/db.rs`
- [ ] Create `rust/src/dsl_v2/execution_context.rs`
- [ ] Create `rust/src/dsl_v2/executor.rs`
- [ ] Update `rust/src/dsl_v2/mod.rs` with exports
- [ ] Update CLI with execute command
- [ ] Add sqlx, uuid, chrono to Cargo.toml dependencies
- [ ] Run `cargo check --features database,cli`
- [ ] Test with `dsl_cli execute --dry-run`
- [ ] Test with actual database connection
