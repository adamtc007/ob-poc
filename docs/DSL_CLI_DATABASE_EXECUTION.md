# DSL CLI Database Execution Implementation

**Goal**: Wire up the `dsl_cli execute` command to actually persist DSL operations to PostgreSQL.

**Prerequisites**: 
- CLI working for parse/validate/plan ✓
- Unified verb registry ✓
- Database schema with CSG columns ✓

---

## Current State

```
dsl_cli parse    → AST (works)
dsl_cli validate → AST + CSG lint (works)
dsl_cli plan     → Execution steps (works)
dsl_cli execute  → ??? (not wired up)
```

## Target State

```
dsl_cli execute --db-url postgres://... → Actually runs against DB
dsl_cli execute --dry-run              → Shows what would happen
```

---

## Implementation Steps

### Step 1: Verify DslExecutor Exists and Works

First, check the current state of `rust/src/dsl_v2/executor.rs`:

```bash
# Check if executor has the execute method
grep -n "pub async fn execute" rust/src/dsl_v2/executor.rs
```

The executor should have:
- `DslExecutor::new(pool: PgPool)`
- `execute(&self, plan: &ExecutionPlan, ctx: &ExecutionContext) -> Result<ExecutionResult>`

### Step 2: Update CLI Execute Command

Edit file: `rust/src/bin/dsl_cli.rs`

The `cmd_execute` function needs to be fully implemented:

```rust
#[cfg(feature = "database")]
async fn cmd_execute(
    file: Option<PathBuf>,
    db_url: String,
    dry_run: bool,
    client_type: Option<String>,
    jurisdiction: Option<String>,
    format: OutputFormat,
) -> Result<(), String> {
    use ob_poc::dsl_v2::{
        parse_program, compile, CsgLinter, DslExecutor, ExecutionContext,
        validation::{ValidationContext, RustStyleFormatter},
        verb_registry::VerbBehavior,
    };

    let source = read_input(file)?;

    // 1. Connect to database
    if format == OutputFormat::Pretty {
        println!("{}", "Connecting to database...".dimmed());
    }
    
    let pool = sqlx::PgPool::connect(&db_url)
        .await
        .map_err(|e| format!("Database connection failed: {}", e))?;

    // 2. Parse
    if format == OutputFormat::Pretty {
        println!("{}", "Parsing DSL...".dimmed());
    }
    
    let ast = parse_program(&source)
        .map_err(|e| format!("Parse error: {:?}", e))?;

    // 3. Build validation context
    let mut context = ValidationContext::default();
    if let Some(ct) = client_type {
        context.client_type = Some(parse_client_type(&ct)?);
    }
    if let Some(j) = jurisdiction {
        context.jurisdiction = Some(j);
    }

    // 4. CSG Lint with database rules
    if format == OutputFormat::Pretty {
        println!("{}", "Running CSG validation...".dimmed());
    }
    
    let mut linter = CsgLinter::new(pool.clone());
    linter.initialize().await
        .map_err(|e| format!("Linter initialization failed: {}", e))?;

    let lint_result = linter.lint(ast.clone(), &context, &source).await;

    if lint_result.has_errors() {
        let formatted = RustStyleFormatter::format(&source, &lint_result.diagnostics);
        if format == OutputFormat::Json {
            let output = serde_json::json!({
                "success": false,
                "stage": "validation",
                "diagnostics": lint_result.diagnostics,
            });
            println!("{}", serde_json::to_string_pretty(&output).unwrap());
        } else {
            eprintln!("{}", formatted);
        }
        return Err("Validation failed".to_string());
    }

    if format == OutputFormat::Pretty && lint_result.has_warnings() {
        let formatted = RustStyleFormatter::format(&source, &lint_result.diagnostics);
        eprintln!("{}", formatted);
    }

    // 5. Compile to execution plan
    if format == OutputFormat::Pretty {
        println!("{}", "Compiling execution plan...".dimmed());
    }
    
    let plan = compile(&ast)
        .map_err(|e| format!("Compile error: {:?}", e))?;

    if format == OutputFormat::Pretty {
        println!("{} Compiled {} step(s)", "✓".green(), plan.steps.len());
    }

    // 6. Dry run - stop here
    if dry_run {
        if format == OutputFormat::Json {
            let output = serde_json::json!({
                "success": true,
                "dry_run": true,
                "steps": plan.steps.len(),
                "plan": plan.steps.iter().enumerate().map(|(i, s)| {
                    serde_json::json!({
                        "step": i,
                        "verb": format!("{}.{}", s.verb_call.domain, s.verb_call.verb),
                        "binding": s.bind_as,
                    })
                }).collect::<Vec<_>>(),
            });
            println!("{}", serde_json::to_string_pretty(&output).unwrap());
        } else {
            println!();
            println!("{} Dry run complete - {} step(s) would execute:", 
                "✓".green().bold(), plan.steps.len());
            println!();
            for (i, step) in plan.steps.iter().enumerate() {
                let binding = step.bind_as.as_ref()
                    .map(|b| format!(" → @{}", b))
                    .unwrap_or_default();
                println!("  [{}] {}.{}{}", 
                    i, 
                    step.verb_call.domain.cyan(),
                    step.verb_call.verb.cyan().bold(),
                    binding.yellow());
            }
        }
        return Ok(());
    }

    // 7. Execute for real
    if format == OutputFormat::Pretty {
        println!();
        println!("{}", "Executing...".yellow().bold());
        println!();
    }

    let executor = DslExecutor::new(pool);
    let mut exec_ctx = ExecutionContext::default();
    
    // Execute step by step with progress
    let mut results: Vec<serde_json::Value> = Vec::new();
    let mut bindings: std::collections::HashMap<String, serde_json::Value> = std::collections::HashMap::new();
    
    for (i, step) in plan.steps.iter().enumerate() {
        let verb_name = format!("{}.{}", step.verb_call.domain, step.verb_call.verb);
        
        if format == OutputFormat::Pretty {
            print!("  [{}] {} ", i, verb_name.cyan());
        }

        match executor.execute_step(step, &mut exec_ctx).await {
            Ok(result) => {
                // Store binding if present
                if let Some(ref binding) = step.bind_as {
                    bindings.insert(binding.clone(), result.clone().into());
                }
                
                results.push(serde_json::json!({
                    "step": i,
                    "verb": verb_name,
                    "success": true,
                    "result": result,
                }));

                if format == OutputFormat::Pretty {
                    println!("{}", "✓".green());
                }
            }
            Err(e) => {
                results.push(serde_json::json!({
                    "step": i,
                    "verb": verb_name,
                    "success": false,
                    "error": e.to_string(),
                }));

                if format == OutputFormat::Json {
                    let output = serde_json::json!({
                        "success": false,
                        "stage": "execution",
                        "failed_step": i,
                        "error": e.to_string(),
                        "results": results,
                    });
                    println!("{}", serde_json::to_string_pretty(&output).unwrap());
                } else {
                    println!("{} {}", "✗".red(), e.to_string().red());
                }
                
                return Err(format!("Execution failed at step {}: {}", i, e));
            }
        }
    }

    // 8. Success output
    if format == OutputFormat::Json {
        let output = serde_json::json!({
            "success": true,
            "steps_executed": results.len(),
            "bindings": bindings,
            "results": results,
        });
        println!("{}", serde_json::to_string_pretty(&output).unwrap());
    } else {
        println!();
        println!("{} Executed {} step(s) successfully", 
            "✓".green().bold(), results.len());
        
        if !bindings.is_empty() {
            println!();
            println!("Bindings created:");
            for (name, value) in &bindings {
                println!("  @{} = {:?}", name.yellow(), value);
            }
        }
    }

    Ok(())
}
```

### Step 3: Ensure ExecutionContext Carries Bindings

Edit file: `rust/src/dsl_v2/executor.rs`

The `ExecutionContext` needs to track symbol bindings between steps:

```rust
/// Context passed through execution
#[derive(Debug, Default)]
pub struct ExecutionContext {
    /// Symbol bindings: @name → resolved UUID
    pub bindings: HashMap<String, uuid::Uuid>,
    
    /// Current CBU ID (if established)
    pub current_cbu_id: Option<uuid::Uuid>,
    
    /// Results from previous steps (for injection)
    pub step_results: Vec<StepResult>,
    
    /// Transaction handle (if in transaction)
    #[cfg(feature = "database")]
    pub transaction: Option<sqlx::Transaction<'static, sqlx::Postgres>>,
}

impl ExecutionContext {
    /// Resolve a symbol reference to a UUID
    pub fn resolve_binding(&self, name: &str) -> Option<uuid::Uuid> {
        self.bindings.get(name).copied()
    }
    
    /// Add a new binding
    pub fn add_binding(&mut self, name: String, id: uuid::Uuid) {
        self.bindings.insert(name, id);
    }
    
    /// Get result from a previous step
    pub fn get_step_result(&self, index: usize) -> Option<&StepResult> {
        self.step_results.get(index)
    }
}
```

### Step 4: Implement execute_step with Binding Resolution

The executor needs to resolve `@symbol` references before executing:

```rust
impl DslExecutor {
    /// Execute a single step
    pub async fn execute_step(
        &self,
        step: &ExecutionStep,
        ctx: &mut ExecutionContext,
    ) -> Result<StepResult> {
        // 1. Resolve any symbol references in arguments
        let resolved_args = self.resolve_step_args(step, ctx)?;
        
        // 2. Dispatch based on behavior
        let result = match step.behavior {
            VerbBehavior::Crud => {
                self.execute_crud(step, &resolved_args, ctx).await?
            }
            VerbBehavior::CustomOp => {
                let op_id = step.custom_op_id.as_ref()
                    .ok_or_else(|| anyhow!("Custom op missing handler ID"))?;
                self.execute_custom_op(op_id, step, &resolved_args, ctx).await?
            }
            VerbBehavior::Composite => {
                return Err(anyhow!("Composite ops not yet implemented"));
            }
        };
        
        // 3. Store binding if specified
        if let Some(ref binding_name) = step.bind_as {
            if let Some(id) = result.created_id {
                ctx.add_binding(binding_name.clone(), id);
            }
        }
        
        // 4. Track CBU if this was a cbu.create
        if step.verb_call.domain == "cbu" && step.verb_call.verb == "create" {
            if let Some(id) = result.created_id {
                ctx.current_cbu_id = Some(id);
            }
        }
        
        // 5. Store result for injection
        ctx.step_results.push(result.clone());
        
        Ok(result)
    }
    
    /// Resolve @symbol references in step arguments
    fn resolve_step_args(
        &self,
        step: &ExecutionStep,
        ctx: &ExecutionContext,
    ) -> Result<HashMap<String, ResolvedValue>> {
        let mut resolved = HashMap::new();
        
        for arg in &step.verb_call.arguments {
            let key = arg.key.canonical();
            let value = match &arg.value {
                Value::Reference(symbol) => {
                    // Look up in context bindings
                    let id = ctx.resolve_binding(symbol)
                        .ok_or_else(|| anyhow!(
                            "Unresolved symbol @{} - not found in execution context", 
                            symbol
                        ))?;
                    ResolvedValue::Uuid(id)
                }
                Value::String(s) => ResolvedValue::String(s.clone()),
                Value::Number(n) => ResolvedValue::Number(*n),
                Value::Boolean(b) => ResolvedValue::Boolean(*b),
                Value::List(items) => {
                    // Recursively resolve list items
                    let resolved_items: Result<Vec<_>> = items.iter()
                        .map(|v| self.resolve_value(v, ctx))
                        .collect();
                    ResolvedValue::List(resolved_items?)
                }
                Value::Map(entries) => {
                    let resolved_map: Result<HashMap<_, _>> = entries.iter()
                        .map(|(k, v)| {
                            let rv = self.resolve_value(v, ctx)?;
                            Ok((k.clone(), rv))
                        })
                        .collect();
                    ResolvedValue::Map(resolved_map?)
                }
                Value::Null => ResolvedValue::Null,
            };
            resolved.insert(key, value);
        }
        
        Ok(resolved)
    }
    
    fn resolve_value(&self, value: &Value, ctx: &ExecutionContext) -> Result<ResolvedValue> {
        match value {
            Value::Reference(symbol) => {
                let id = ctx.resolve_binding(symbol)
                    .ok_or_else(|| anyhow!("Unresolved symbol @{}", symbol))?;
                Ok(ResolvedValue::Uuid(id))
            }
            Value::String(s) => Ok(ResolvedValue::String(s.clone())),
            Value::Number(n) => Ok(ResolvedValue::Number(*n)),
            Value::Boolean(b) => Ok(ResolvedValue::Boolean(*b)),
            Value::Null => Ok(ResolvedValue::Null),
            _ => Err(anyhow!("Nested structures not supported in value resolution")),
        }
    }
}

/// A resolved value ready for database binding
#[derive(Debug, Clone)]
pub enum ResolvedValue {
    String(String),
    Number(f64),
    Boolean(bool),
    Uuid(uuid::Uuid),
    List(Vec<ResolvedValue>),
    Map(HashMap<String, ResolvedValue>),
    Null,
}

/// Result of executing a step
#[derive(Debug, Clone)]
pub struct StepResult {
    /// ID of created entity (if applicable)
    pub created_id: Option<uuid::Uuid>,
    /// Number of rows affected
    pub rows_affected: u64,
    /// Any returned data
    pub data: Option<serde_json::Value>,
}
```

### Step 5: Implement CRUD Execution

The CRUD executor needs to map verbs to SQL:

```rust
impl DslExecutor {
    async fn execute_crud(
        &self,
        step: &ExecutionStep,
        args: &HashMap<String, ResolvedValue>,
        ctx: &ExecutionContext,
    ) -> Result<StepResult> {
        let domain = &step.verb_call.domain;
        let verb = &step.verb_call.verb;
        
        match (domain.as_str(), verb.as_str()) {
            ("cbu", "create") => self.create_cbu(args).await,
            ("cbu", "assign-role") => self.assign_role(args).await,
            ("entity", verb) if verb.starts_with("create-") => {
                let entity_type = verb.strip_prefix("create-")
                    .map(|s| s.to_uppercase().replace('-', "_"))
                    .unwrap_or_else(|| "ENTITY".to_string());
                self.create_entity(&entity_type, args).await
            }
            ("entity", "read") => self.read_entity(args).await,
            ("entity", "update") => self.update_entity(args).await,
            ("entity", "delete") => self.delete_entity(args).await,
            // Add more CRUD mappings as needed
            _ => Err(anyhow!("CRUD handler not implemented for {}.{}", domain, verb)),
        }
    }
    
    async fn create_cbu(&self, args: &HashMap<String, ResolvedValue>) -> Result<StepResult> {
        let name = args.get("name")
            .and_then(|v| v.as_string())
            .ok_or_else(|| anyhow!("cbu.create requires :name"))?;
        
        let jurisdiction = args.get("jurisdiction")
            .and_then(|v| v.as_string());
        
        let client_type = args.get("client-type")
            .and_then(|v| v.as_string());
        
        let id = uuid::Uuid::new_v4();
        
        sqlx::query!(
            r#"
            INSERT INTO "ob-poc".cbus (cbu_id, name, jurisdiction, client_type, created_at)
            VALUES ($1, $2, $3, $4, NOW())
            "#,
            id,
            name,
            jurisdiction,
            client_type,
        )
        .execute(&self.pool)
        .await
        .map_err(|e| anyhow!("Failed to create CBU: {}", e))?;
        
        Ok(StepResult {
            created_id: Some(id),
            rows_affected: 1,
            data: Some(serde_json::json!({ "cbu_id": id.to_string() })),
        })
    }
    
    async fn create_entity(
        &self, 
        entity_type: &str, 
        args: &HashMap<String, ResolvedValue>
    ) -> Result<StepResult> {
        let cbu_id = args.get("cbu-id")
            .and_then(|v| v.as_uuid())
            .ok_or_else(|| anyhow!("entity.create requires :cbu-id"))?;
        
        let name = args.get("name")
            .and_then(|v| v.as_string())
            .ok_or_else(|| anyhow!("entity.create requires :name"))?;
        
        let id = uuid::Uuid::new_v4();
        
        // Look up entity_type_id from entity_types table
        let entity_type_id: Option<uuid::Uuid> = sqlx::query_scalar!(
            r#"SELECT entity_type_id FROM "ob-poc".entity_types WHERE type_code = $1"#,
            entity_type
        )
        .fetch_optional(&self.pool)
        .await
        .map_err(|e| anyhow!("Failed to lookup entity type: {}", e))?;
        
        let entity_type_id = entity_type_id
            .ok_or_else(|| anyhow!("Unknown entity type: {}", entity_type))?;
        
        sqlx::query!(
            r#"
            INSERT INTO "ob-poc".entities (entity_id, cbu_id, entity_type_id, name, created_at)
            VALUES ($1, $2, $3, $4, NOW())
            "#,
            id,
            cbu_id,
            entity_type_id,
            name,
        )
        .execute(&self.pool)
        .await
        .map_err(|e| anyhow!("Failed to create entity: {}", e))?;
        
        Ok(StepResult {
            created_id: Some(id),
            rows_affected: 1,
            data: Some(serde_json::json!({ 
                "entity_id": id.to_string(),
                "entity_type": entity_type,
            })),
        })
    }
    
    async fn assign_role(&self, args: &HashMap<String, ResolvedValue>) -> Result<StepResult> {
        let cbu_id = args.get("cbu-id")
            .and_then(|v| v.as_uuid())
            .ok_or_else(|| anyhow!("cbu.assign-role requires :cbu-id"))?;
        
        let entity_id = args.get("entity-id")
            .and_then(|v| v.as_uuid())
            .ok_or_else(|| anyhow!("cbu.assign-role requires :entity-id"))?;
        
        let target_entity_id = args.get("target-entity-id")
            .and_then(|v| v.as_uuid())
            .ok_or_else(|| anyhow!("cbu.assign-role requires :target-entity-id"))?;
        
        let role = args.get("role")
            .and_then(|v| v.as_string())
            .ok_or_else(|| anyhow!("cbu.assign-role requires :role"))?;
        
        let ownership_percentage = args.get("ownership-percentage")
            .and_then(|v| v.as_number());
        
        let id = uuid::Uuid::new_v4();
        
        // Look up role_id
        let role_id: Option<uuid::Uuid> = sqlx::query_scalar!(
            r#"SELECT role_id FROM "ob-poc".roles WHERE role_code = $1"#,
            role
        )
        .fetch_optional(&self.pool)
        .await
        .map_err(|e| anyhow!("Failed to lookup role: {}", e))?;
        
        let role_id = role_id
            .ok_or_else(|| anyhow!("Unknown role: {}", role))?;
        
        sqlx::query!(
            r#"
            INSERT INTO "ob-poc".entity_roles 
                (entity_role_id, cbu_id, entity_id, target_entity_id, role_id, ownership_percentage, created_at)
            VALUES ($1, $2, $3, $4, $5, $6, NOW())
            "#,
            id,
            cbu_id,
            entity_id,
            target_entity_id,
            role_id,
            ownership_percentage,
        )
        .execute(&self.pool)
        .await
        .map_err(|e| anyhow!("Failed to assign role: {}", e))?;
        
        Ok(StepResult {
            created_id: Some(id),
            rows_affected: 1,
            data: Some(serde_json::json!({ 
                "entity_role_id": id.to_string(),
                "role": role,
            })),
        })
    }
    
    // Placeholder implementations
    async fn read_entity(&self, _args: &HashMap<String, ResolvedValue>) -> Result<StepResult> {
        Err(anyhow!("entity.read not yet implemented"))
    }
    
    async fn update_entity(&self, _args: &HashMap<String, ResolvedValue>) -> Result<StepResult> {
        Err(anyhow!("entity.update not yet implemented"))
    }
    
    async fn delete_entity(&self, _args: &HashMap<String, ResolvedValue>) -> Result<StepResult> {
        Err(anyhow!("entity.delete not yet implemented"))
    }
}

// Helper methods for ResolvedValue
impl ResolvedValue {
    pub fn as_string(&self) -> Option<&str> {
        match self {
            ResolvedValue::String(s) => Some(s),
            _ => None,
        }
    }
    
    pub fn as_uuid(&self) -> Option<uuid::Uuid> {
        match self {
            ResolvedValue::Uuid(id) => Some(*id),
            _ => None,
        }
    }
    
    pub fn as_number(&self) -> Option<f64> {
        match self {
            ResolvedValue::Number(n) => Some(*n),
            _ => None,
        }
    }
}
```

### Step 6: Implement Custom Op Handlers

Edit file: `rust/src/dsl_v2/custom_ops/mod.rs`

Add execution implementations for custom ops:

```rust
use super::executor::{ResolvedValue, StepResult, ExecutionContext, ExecutionStep};
use anyhow::Result;
use sqlx::PgPool;
use std::collections::HashMap;

/// Trait for custom operation handlers
#[async_trait::async_trait]
pub trait CustomOpHandler: Send + Sync {
    async fn execute(
        &self,
        pool: &PgPool,
        args: &HashMap<String, ResolvedValue>,
        ctx: &ExecutionContext,
    ) -> Result<StepResult>;
}

// Document Catalog Operation
pub struct DocumentCatalogOp;

#[async_trait::async_trait]
impl CustomOpHandler for DocumentCatalogOp {
    async fn execute(
        &self,
        pool: &PgPool,
        args: &HashMap<String, ResolvedValue>,
        _ctx: &ExecutionContext,
    ) -> Result<StepResult> {
        let cbu_id = args.get("cbu-id")
            .and_then(|v| v.as_uuid())
            .ok_or_else(|| anyhow::anyhow!("document.catalog requires :cbu-id"))?;
        
        let entity_id = args.get("entity-id")
            .and_then(|v| v.as_uuid())
            .ok_or_else(|| anyhow::anyhow!("document.catalog requires :entity-id"))?;
        
        let document_type = args.get("document-type")
            .and_then(|v| v.as_string())
            .ok_or_else(|| anyhow::anyhow!("document.catalog requires :document-type"))?;
        
        let id = uuid::Uuid::new_v4();
        
        // Look up document type_id
        let type_id: Option<uuid::Uuid> = sqlx::query_scalar!(
            r#"SELECT type_id FROM "ob-poc".document_types WHERE type_code = $1"#,
            document_type
        )
        .fetch_optional(pool)
        .await?;
        
        let type_id = type_id
            .ok_or_else(|| anyhow::anyhow!("Unknown document type: {}", document_type))?;
        
        sqlx::query!(
            r#"
            INSERT INTO "ob-poc".documents 
                (document_id, cbu_id, entity_id, document_type_id, status, created_at)
            VALUES ($1, $2, $3, $4, 'pending', NOW())
            "#,
            id,
            cbu_id,
            entity_id,
            type_id,
        )
        .execute(pool)
        .await?;
        
        Ok(StepResult {
            created_id: Some(id),
            rows_affected: 1,
            data: Some(serde_json::json!({
                "document_id": id.to_string(),
                "document_type": document_type,
                "status": "pending",
            })),
        })
    }
}

// Screening PEP Operation
pub struct ScreeningPepOp;

#[async_trait::async_trait]
impl CustomOpHandler for ScreeningPepOp {
    async fn execute(
        &self,
        pool: &PgPool,
        args: &HashMap<String, ResolvedValue>,
        _ctx: &ExecutionContext,
    ) -> Result<StepResult> {
        let entity_id = args.get("entity-id")
            .and_then(|v| v.as_uuid())
            .ok_or_else(|| anyhow::anyhow!("screening.pep requires :entity-id"))?;
        
        let id = uuid::Uuid::new_v4();
        
        // Create screening record
        sqlx::query!(
            r#"
            INSERT INTO "ob-poc".screenings 
                (screening_id, entity_id, screening_type, status, created_at)
            VALUES ($1, $2, 'PEP', 'pending', NOW())
            "#,
            id,
            entity_id,
        )
        .execute(pool)
        .await?;
        
        Ok(StepResult {
            created_id: Some(id),
            rows_affected: 1,
            data: Some(serde_json::json!({
                "screening_id": id.to_string(),
                "screening_type": "PEP",
                "status": "pending",
            })),
        })
    }
}

// Screening Sanctions Operation
pub struct ScreeningSanctionsOp;

#[async_trait::async_trait]
impl CustomOpHandler for ScreeningSanctionsOp {
    async fn execute(
        &self,
        pool: &PgPool,
        args: &HashMap<String, ResolvedValue>,
        _ctx: &ExecutionContext,
    ) -> Result<StepResult> {
        let entity_id = args.get("entity-id")
            .and_then(|v| v.as_uuid())
            .ok_or_else(|| anyhow::anyhow!("screening.sanctions requires :entity-id"))?;
        
        let id = uuid::Uuid::new_v4();
        
        sqlx::query!(
            r#"
            INSERT INTO "ob-poc".screenings 
                (screening_id, entity_id, screening_type, status, created_at)
            VALUES ($1, $2, 'SANCTIONS', 'pending', NOW())
            "#,
            id,
            entity_id,
        )
        .execute(pool)
        .await?;
        
        Ok(StepResult {
            created_id: Some(id),
            rows_affected: 1,
            data: Some(serde_json::json!({
                "screening_id": id.to_string(),
                "screening_type": "SANCTIONS",
                "status": "pending",
            })),
        })
    }
}

// UBO Calculate Operation
pub struct UboCalculateOp;

#[async_trait::async_trait]
impl CustomOpHandler for UboCalculateOp {
    async fn execute(
        &self,
        pool: &PgPool,
        args: &HashMap<String, ResolvedValue>,
        _ctx: &ExecutionContext,
    ) -> Result<StepResult> {
        let cbu_id = args.get("cbu-id")
            .and_then(|v| v.as_uuid())
            .ok_or_else(|| anyhow::anyhow!("ubo.calculate requires :cbu-id"))?;
        
        let entity_id = args.get("entity-id")
            .and_then(|v| v.as_uuid())
            .ok_or_else(|| anyhow::anyhow!("ubo.calculate requires :entity-id"))?;
        
        let threshold = args.get("threshold")
            .and_then(|v| v.as_number())
            .unwrap_or(25.0);
        
        // Query the ownership chain
        let ubos = sqlx::query!(
            r#"
            SELECT 
                er.entity_id,
                e.name,
                er.ownership_percentage
            FROM "ob-poc".entity_roles er
            JOIN "ob-poc".entities e ON er.entity_id = e.entity_id
            JOIN "ob-poc".roles r ON er.role_id = r.role_id
            WHERE er.target_entity_id = $1
              AND r.role_code = 'BENEFICIAL_OWNER'
              AND er.ownership_percentage >= $2
            "#,
            entity_id,
            threshold,
        )
        .fetch_all(pool)
        .await?;
        
        let ubo_list: Vec<_> = ubos.iter().map(|row| {
            serde_json::json!({
                "entity_id": row.entity_id.to_string(),
                "name": row.name,
                "ownership_percentage": row.ownership_percentage,
            })
        }).collect();
        
        Ok(StepResult {
            created_id: None,
            rows_affected: 0,
            data: Some(serde_json::json!({
                "entity_id": entity_id.to_string(),
                "threshold": threshold,
                "ubos": ubo_list,
                "ubo_count": ubos.len(),
            })),
        })
    }
}
```

---

## Execution Checklist

### Phase 1: CLI Execute Command
- [ ] Update `cmd_execute` in `dsl_cli.rs` with full implementation
- [ ] Ensure proper error handling and progress output
- [ ] Add dry-run support
- [ ] Run `cargo check --features cli,database`

### Phase 2: ExecutionContext Updates
- [ ] Add binding tracking to `ExecutionContext`
- [ ] Implement `resolve_binding` and `add_binding`
- [ ] Run `cargo check --features database`

### Phase 3: Executor Step Execution
- [ ] Implement `execute_step` with binding resolution
- [ ] Implement `resolve_step_args`
- [ ] Add `ResolvedValue` type with helper methods
- [ ] Run `cargo check --features database`

### Phase 4: CRUD Handlers
- [ ] Implement `create_cbu`
- [ ] Implement `create_entity` (maps entity type from verb)
- [ ] Implement `assign_role`
- [ ] Run `cargo check --features database`

### Phase 5: Custom Op Handlers
- [ ] Implement `DocumentCatalogOp`
- [ ] Implement `ScreeningPepOp`
- [ ] Implement `ScreeningSanctionsOp`
- [ ] Implement `UboCalculateOp`
- [ ] Run `cargo check --features database`

### Phase 6: Integration Test
- [ ] Start with dry run:
  ```bash
  echo '<corporate DSL>' | cargo run --features cli,database --bin dsl_cli -- execute --dry-run --db-url postgres://...
  ```
- [ ] Test actual execution:
  ```bash
  echo '<corporate DSL>' | cargo run --features cli,database --bin dsl_cli -- execute --db-url postgres://...
  ```
- [ ] Verify data in database

---

## Test Commands

```bash
# Build with database feature
cargo build --features cli,database --bin dsl_cli

# Dry run (no DB changes)
cat <<'EOF' | cargo run --features cli,database --bin dsl_cli -- execute --dry-run --db-url postgresql://localhost/ob-poc
(cbu.create :name "Test Corp" :client-type "corporate" :jurisdiction "GB" :as @cbu)
(entity.create-limited-company :cbu-id @cbu :name "Test Corp" :as @company)
EOF

# Actual execution
cat <<'EOF' | cargo run --features cli,database --bin dsl_cli -- execute --db-url postgresql://localhost/ob-poc
(cbu.create :name "Test Corp" :client-type "corporate" :jurisdiction "GB" :as @cbu)
(entity.create-limited-company :cbu-id @cbu :name "Test Corp" :as @company)
EOF

# Verify in database
psql ob-poc -c "SELECT * FROM \"ob-poc\".cbus ORDER BY created_at DESC LIMIT 1;"
psql ob-poc -c "SELECT * FROM \"ob-poc\".entities ORDER BY created_at DESC LIMIT 1;"
```

---

## JSON Output Format

For machine consumption (Claude Code), use `--format json`:

```bash
echo '...' | dsl_cli execute --db-url ... --format json
```

Success output:
```json
{
  "success": true,
  "steps_executed": 5,
  "bindings": {
    "cbu": "550e8400-e29b-41d4-a716-446655440000",
    "company": "550e8400-e29b-41d4-a716-446655440001"
  },
  "results": [
    {"step": 0, "verb": "cbu.create", "success": true, "result": {...}},
    {"step": 1, "verb": "entity.create-limited-company", "success": true, "result": {...}}
  ]
}
```

Failure output:
```json
{
  "success": false,
  "stage": "execution",
  "failed_step": 2,
  "error": "Unknown entity type: INVALID_TYPE",
  "results": [
    {"step": 0, "verb": "cbu.create", "success": true, "result": {...}},
    {"step": 1, "verb": "entity.create-limited-company", "success": true, "result": {...}},
    {"step": 2, "verb": "entity.create-invalid", "success": false, "error": "..."}
  ]
}
```
