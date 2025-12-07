//! DSL Executor - YAML-driven execution engine for DSL v2
//!
//! This module implements the DslExecutor that processes parsed DSL programs
//! and executes them against the database using YAML-driven verb definitions.
//!
//! The executor routes verbs through:
//! - GenericCrudExecutor for CRUD operations (defined in verbs.yaml)
//! - CustomOperationRegistry for plugins (complex logic, external APIs)

use anyhow::{anyhow, bail, Result};
use serde_json::Value as JsonValue;
use std::collections::HashMap;
use uuid::Uuid;

#[cfg(feature = "database")]
use super::ast::{Value, VerbCall};
#[cfg(feature = "database")]
use super::custom_ops::CustomOperationRegistry;
#[cfg(feature = "database")]
use super::generic_executor::{GenericCrudExecutor, GenericExecutionResult};
#[cfg(feature = "database")]
use super::runtime_registry::{runtime_registry, RuntimeBehavior};

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
#[derive(Debug)]
pub struct ExecutionContext {
    /// Symbol table for @reference resolution
    pub symbols: HashMap<String, Uuid>,
    /// Audit user for tracking
    pub audit_user: Option<String>,
    /// Transaction ID for grouping operations
    pub transaction_id: Option<Uuid>,
    /// Execution ID for idempotency tracking (auto-generated if not set)
    pub execution_id: Uuid,
    /// Whether idempotency checking is enabled
    pub idempotency_enabled: bool,
}

impl Default for ExecutionContext {
    fn default() -> Self {
        Self {
            symbols: HashMap::new(),
            audit_user: None,
            transaction_id: None,
            execution_id: Uuid::new_v4(),
            idempotency_enabled: true,
        }
    }
}

impl ExecutionContext {
    pub fn new() -> Self {
        Self::default()
    }

    /// Create context with a specific execution ID (for resumable executions)
    pub fn with_execution_id(execution_id: Uuid) -> Self {
        Self {
            execution_id,
            ..Self::default()
        }
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

    /// Disable idempotency checking (for testing or forced re-execution)
    pub fn without_idempotency(mut self) -> Self {
        self.idempotency_enabled = false;
        self
    }
}

/// The main DSL executor
pub struct DslExecutor {
    #[cfg(feature = "database")]
    pool: PgPool,
    #[cfg(feature = "database")]
    custom_ops: CustomOperationRegistry,
    #[cfg(feature = "database")]
    generic_executor: GenericCrudExecutor,
    #[cfg(feature = "database")]
    idempotency: super::idempotency::IdempotencyManager,
}

impl DslExecutor {
    /// Create a new executor with a database pool
    #[cfg(feature = "database")]
    pub fn new(pool: PgPool) -> Self {
        Self {
            generic_executor: GenericCrudExecutor::new(pool.clone()),
            idempotency: super::idempotency::IdempotencyManager::new(pool.clone()),
            pool,
            custom_ops: CustomOperationRegistry::new(),
        }
    }

    /// Create an executor without database (for testing/parsing only)
    #[cfg(not(feature = "database"))]
    pub fn new_without_db() -> Self {
        Self {}
    }

    /// Execute a single verb call
    ///
    /// Routes through YAML-driven generic executor for CRUD verbs,
    /// and custom operations registry for plugins.
    #[cfg(feature = "database")]
    pub async fn execute_verb(
        &self,
        vc: &VerbCall,
        ctx: &mut ExecutionContext,
    ) -> Result<ExecutionResult> {
        // Look up verb in runtime registry (loaded from YAML)
        let runtime_verb = runtime_registry()
            .get(&vc.domain, &vc.verb)
            .ok_or_else(|| anyhow!("Unknown verb: {}.{}", vc.domain, vc.verb))?;

        // Check if this is a plugin (custom operation)
        eprintln!(
            "DEBUG executor: {}.{} behavior={:?}",
            vc.domain, vc.verb, runtime_verb.behavior
        );
        if let RuntimeBehavior::Plugin(_handler) = &runtime_verb.behavior {
            eprintln!("DEBUG executor: routing to plugin");
            // Dispatch to custom operations handler
            if let Some(op) = self.custom_ops.get(&vc.domain, &vc.verb) {
                eprintln!("DEBUG executor: found handler, calling execute");
                return op.execute(vc, ctx, &self.pool).await;
            }
            return Err(anyhow!(
                "Plugin {}.{} has no handler implementation",
                vc.domain,
                vc.verb
            ));
        }
        eprintln!("DEBUG executor: routing to generic executor");

        // Convert VerbCall arguments to JSON for generic executor
        let json_args = Self::verbcall_args_to_json(&vc.arguments, ctx)?;

        // Execute via generic executor
        let result = self
            .generic_executor
            .execute(runtime_verb, &json_args)
            .await?;

        // Handle symbol capture
        if runtime_verb.returns.capture {
            if let GenericExecutionResult::Uuid(uuid) = &result {
                if let Some(name) = &runtime_verb.returns.name {
                    ctx.bind(name, *uuid);
                }
            }
        }

        Ok(result.to_legacy())
    }

    /// Convert VerbCall arguments to JSON HashMap for generic executor
    #[cfg(feature = "database")]
    fn verbcall_args_to_json(
        args: &[super::ast::Argument],
        ctx: &ExecutionContext,
    ) -> Result<HashMap<String, JsonValue>> {
        let mut result = HashMap::new();
        for arg in args {
            let key = arg.key.canonical();
            let value = Self::value_to_json(&arg.value, ctx)?;
            result.insert(key, value);
        }
        Ok(result)
    }

    /// Convert AST Value to JSON, resolving references
    #[cfg(feature = "database")]
    fn value_to_json(value: &Value, ctx: &ExecutionContext) -> Result<JsonValue> {
        match value {
            Value::String(s) => Ok(JsonValue::String(s.clone())),
            Value::Integer(i) => Ok(serde_json::json!(*i)),
            Value::Decimal(d) => Ok(serde_json::json!(d.to_string())),
            Value::Boolean(b) => Ok(JsonValue::Bool(*b)),
            Value::Null => Ok(JsonValue::Null),
            Value::Reference(name) => {
                let uuid = ctx
                    .resolve(name)
                    .ok_or_else(|| anyhow!("Unresolved reference: @{}", name))?;
                Ok(JsonValue::String(uuid.to_string()))
            }
            Value::AttributeRef(uuid) => Ok(JsonValue::String(uuid.to_string())),
            Value::DocumentRef(uuid) => Ok(JsonValue::String(uuid.to_string())),
            Value::List(items) => {
                let json_items: Result<Vec<JsonValue>> =
                    items.iter().map(|v| Self::value_to_json(v, ctx)).collect();
                Ok(JsonValue::Array(json_items?))
            }
            Value::Map(map) => {
                let mut json_map = serde_json::Map::new();
                for (k, v) in map {
                    json_map.insert(k.clone(), Self::value_to_json(v, ctx)?);
                }
                Ok(JsonValue::Object(json_map))
            }
            Value::NestedCall(_) => {
                bail!("NestedCall found during value conversion. Use compile() + execute_plan() for nested DSL.")
            }
            Value::LookupRef {
                ref_type: _,
                search_key,
                primary_key,
            } => {
                // Use resolved primary_key if available, otherwise fall back to search_key
                // The executor's resolve_lookup will handle the final resolution if needed
                if let Some(pk) = primary_key {
                    Ok(JsonValue::String(pk.clone()))
                } else {
                    // Not yet resolved - pass search_key for lookup during execution
                    Ok(JsonValue::String(search_key.clone()))
                }
            }
        }
    }
}

// ============================================================================
// Plan Execution
// ============================================================================

#[cfg(feature = "database")]
impl DslExecutor {
    /// Execute a compiled execution plan
    ///
    /// This is the preferred method for executing DSL with nested/composite operations.
    /// The plan has already been dependency-sorted by the compiler.
    ///
    /// Idempotency: Each statement is checked against the idempotency table.
    /// If already executed (same execution_id + statement_index + verb + args),
    /// the cached result is returned. Otherwise, the statement is executed
    /// and the result is recorded for future runs.
    ///
    /// # Example
    /// ```ignore
    /// let program = parse_program(dsl_source)?;
    /// let plan = compile(&program)?;
    /// let results = executor.execute_plan(&plan, &mut ctx).await?;
    /// ```
    pub async fn execute_plan(
        &self,
        plan: &super::execution_plan::ExecutionPlan,
        ctx: &mut ExecutionContext,
    ) -> Result<Vec<ExecutionResult>> {
        let mut results: Vec<ExecutionResult> = Vec::with_capacity(plan.steps.len());

        for (step_index, step) in plan.steps.iter().enumerate() {
            // Clone the verb call so we can inject values
            let mut vc = step.verb_call.clone();

            // Trace each verb execution
            tracing::debug!(
                step = step_index,
                verb = %format!("{}.{}", &vc.domain, &vc.verb),
                bind_as = ?step.bind_as,
                "executing DSL step"
            );

            // Inject values from previous steps
            for inj in &step.injections {
                if let Some(ExecutionResult::Uuid(id)) = results.get(inj.from_step) {
                    // Add the injected argument
                    vc.arguments.push(super::ast::Argument {
                        key: super::ast::Key::Simple(inj.into_arg.clone()),
                        key_span: super::ast::Span::default(),
                        value: super::ast::Value::String(id.to_string()),
                        value_span: super::ast::Span::default(),
                    });
                }
            }

            // Build args for idempotency check
            let verb_name = format!("{}.{}", vc.domain, vc.verb);
            let json_args = Self::verbcall_args_to_json(&vc.arguments, ctx)?;

            // Check idempotency cache if enabled
            if ctx.idempotency_enabled {
                if let Some(cached) = self
                    .idempotency
                    .check(ctx.execution_id, step_index, &verb_name, &json_args)
                    .await?
                {
                    let result = cached.to_execution_result();

                    // Restore symbol binding from cached result
                    if let Some(ref binding_name) = step.bind_as {
                        if let ExecutionResult::Uuid(id) = &result {
                            ctx.bind(binding_name, *id);
                        }
                    }

                    results.push(result);
                    continue;
                }
            }

            // Execute the verb call
            let result = self.execute_verb(&vc, ctx).await?;

            // Trace the result
            tracing::debug!(
                step = step_index,
                verb = %format!("{}.{}", &vc.domain, &vc.verb),
                result = ?result,
                "DSL step completed"
            );

            // Record in idempotency table if enabled
            if ctx.idempotency_enabled {
                self.idempotency
                    .record(
                        ctx.execution_id,
                        step_index,
                        &verb_name,
                        &json_args,
                        &result,
                    )
                    .await?;
            }

            // Handle explicit :as binding (in addition to verb's default capture)
            if let Some(ref binding_name) = step.bind_as {
                if let ExecutionResult::Uuid(id) = &result {
                    ctx.bind(binding_name, *id);
                }
            }

            results.push(result);
        }

        Ok(results)
    }

    /// Convenience method: parse, compile, and execute DSL source
    ///
    /// This is the all-in-one method for executing DSL strings.
    pub async fn execute_dsl(
        &self,
        source: &str,
        ctx: &mut ExecutionContext,
    ) -> Result<Vec<ExecutionResult>> {
        let program =
            super::parser::parse_program(source).map_err(|e| anyhow!("Parse error: {}", e))?;

        let plan = super::execution_plan::compile(&program)
            .map_err(|e| anyhow!("Compile error: {}", e))?;

        self.execute_plan(&plan, ctx).await
    }
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
}
