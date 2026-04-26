//! RealDslExecutor — Bridge to the existing DSL pipeline
//!
//! Replaces `StubExecutor` with a real implementation that parses, compiles,
//! and executes DSL strings via the dsl_v2 pipeline.
//!
//! # Feature Gates
//!
//! This module requires both `vnext-repl` and `database` features.
//! The `StubExecutor` (in orchestrator_v2.rs) remains available for tests.

use serde_json::json;
use sqlx::PgPool;
use uuid::Uuid;

use crate::dsl_v2::execution::{ExecutionContext, ExecutionResult};
use crate::dsl_v2::planning::compile;
use crate::dsl_v2::syntax::parse_program;
use crate::sequencer::DslExecutor;

// ---------------------------------------------------------------------------
// RealDslExecutor
// ---------------------------------------------------------------------------

/// Bridge from the v2 REPL `DslExecutor` trait to the real dsl-core pipeline.
///
/// Lifecycle: parse → compile → execute → collect results as JSON.
pub struct RealDslExecutor {
    pool: PgPool,
    allow_durable_direct: bool,
    service_registry: std::sync::Arc<dsl_runtime::ServiceRegistry>,
    /// Canonical [`sem_os_postgres::ops::SemOsVerbOpRegistry`] threaded onto
    /// every inner `DslExecutor` this bridge constructs. Required for plugin
    /// dispatch post-Phase-5c-migrate slice #80 — without it, plugin verbs
    /// fail with an actionable "no SemOsVerbOp registered" error.
    sem_os_ops: Option<std::sync::Arc<sem_os_postgres::ops::SemOsVerbOpRegistry>>,
}

impl RealDslExecutor {
    pub fn new(pool: PgPool) -> Self {
        Self {
            pool,
            allow_durable_direct: false,
            service_registry: std::sync::Arc::new(dsl_runtime::ServiceRegistry::empty()),
            sem_os_ops: None,
        }
    }

    pub fn allow_durable_direct(mut self) -> Self {
        self.allow_durable_direct = true;
        self
    }

    /// Install the platform service registry. Threaded into every inner
    /// `DslExecutor` this bridge constructs.
    pub fn with_services(mut self, services: std::sync::Arc<dsl_runtime::ServiceRegistry>) -> Self {
        self.service_registry = services;
        self
    }

    /// Install the canonical SemOS plugin op registry. Threaded into every
    /// inner `DslExecutor` this bridge constructs so plugin verbs dispatch
    /// correctly.
    pub fn with_sem_os_ops(
        mut self,
        ops: std::sync::Arc<sem_os_postgres::ops::SemOsVerbOpRegistry>,
    ) -> Self {
        self.sem_os_ops = Some(ops);
        self
    }
}

impl RealDslExecutor {
    /// Shared parse → compile → build context path used by both the
    /// self-scoped (`execute`) and in-scope (`execute_in_scope`) entry
    /// points.
    fn build_executor_and_ctx(&self) -> (crate::dsl_v2::executor::DslExecutor, ExecutionContext) {
        let mut ctx = if self.allow_durable_direct {
            ExecutionContext::new().allow_durable_direct()
        } else {
            ExecutionContext::new()
        };
        ctx.execution_id = Uuid::new_v4();

        let mut executor = crate::dsl_v2::executor::DslExecutor::new(self.pool.clone())
            .with_services(self.service_registry.clone());
        if let Some(ref ops) = self.sem_os_ops {
            executor = executor.with_sem_os_ops(ops.clone());
        }

        (executor, ctx)
    }

    fn build_response(results: &[ExecutionResult], ctx: &ExecutionContext) -> serde_json::Value {
        let step_results: Vec<serde_json::Value> =
            results.iter().map(execution_result_to_json).collect();

        let bindings: serde_json::Map<String, serde_json::Value> = ctx
            .symbols
            .iter()
            .map(|(k, v)| (k.clone(), json!(v.to_string())))
            .collect();

        json!({
            "success": true,
            "steps_executed": step_results.len(),
            "results": step_results,
            "bindings": bindings,
        })
    }
}

#[async_trait::async_trait]
impl DslExecutor for RealDslExecutor {
    async fn execute(&self, dsl: &str) -> Result<serde_json::Value, String> {
        // 1. Parse DSL string → Program AST.
        let program = parse_program(dsl).map_err(|e| format!("Parse error: {}", e))?;

        // 2. Compile → ExecutionPlan (topological sort, injections).
        let plan = compile(&program).map_err(|e| format!("Compile error: {:?}", e))?;

        // 3. Build executor + context (shared with execute_in_scope).
        let (executor, mut ctx) = self.build_executor_and_ctx();

        // 4. Execute via execute_plan (per-verb txns — each verb commits).
        //    This preserves legacy non-atomic semantics for callers that
        //    don't have an outer scope to pass in.
        let results = executor
            .execute_plan(&plan, &mut ctx)
            .await
            .map_err(|e| format!("Execution error: {}", e))?;

        Ok(Self::build_response(&results, &ctx))
    }

    /// Phase B.2b-γ (2026-04-22): scope-aware entry point for the
    /// Sequencer's step executor bridge. Parses + compiles the DSL,
    /// then runs the plan through
    /// [`DslExecutor::execute_plan_atomic_in_scope`] so every verb
    /// dispatches through the CALLER'S scope — no per-verb txns.
    ///
    /// Caller (step executor bridge → runbook executor → Sequencer)
    /// owns commit/rollback. When the Sequencer opens one scope per
    /// runbook (B.2b-ζ), multiple steps share one transaction and
    /// commit atomically or roll back together.
    async fn execute_in_scope(
        &self,
        dsl: &str,
        scope: &mut dyn dsl_runtime::tx::TransactionScope,
    ) -> Result<serde_json::Value, String> {
        let program = parse_program(dsl).map_err(|e| format!("Parse error: {}", e))?;
        let plan = compile(&program).map_err(|e| format!("Compile error: {:?}", e))?;

        let (executor, mut ctx) = self.build_executor_and_ctx();

        let results = executor
            .execute_plan_atomic_in_scope(&plan, &mut ctx, scope)
            .await
            .map_err(|e| format!("Execution error: {}", e))?;

        Ok(Self::build_response(&results, &ctx))
    }
}

/// Convert an ExecutionResult variant to a JSON value.
fn execution_result_to_json(result: &ExecutionResult) -> serde_json::Value {
    match result {
        ExecutionResult::Uuid(id) => json!({
            "type": "uuid",
            "value": id.to_string(),
        }),
        ExecutionResult::Record(val) => json!({
            "type": "record",
            "value": val,
        }),
        ExecutionResult::RecordSet(vals) => json!({
            "type": "record_set",
            "value": vals,
        }),
        ExecutionResult::Affected(count) => json!({
            "type": "affected",
            "value": count,
        }),
        ExecutionResult::Void => json!({
            "type": "void",
        }),
        // Complex result types — serialize to generic JSON.
        _ => json!({
            "type": "complex",
            "value": format!("{:?}", result),
        }),
    }
}

// ---------------------------------------------------------------------------
// Tests
// ---------------------------------------------------------------------------

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_execution_result_to_json_uuid() {
        let id = Uuid::new_v4();
        let json = execution_result_to_json(&ExecutionResult::Uuid(id));
        assert_eq!(json["type"], "uuid");
        assert_eq!(json["value"], id.to_string());
    }

    #[test]
    fn test_execution_result_to_json_void() {
        let json = execution_result_to_json(&ExecutionResult::Void);
        assert_eq!(json["type"], "void");
    }

    #[test]
    fn test_execution_result_to_json_affected() {
        let json = execution_result_to_json(&ExecutionResult::Affected(42));
        assert_eq!(json["type"], "affected");
        assert_eq!(json["value"], 42);
    }

    #[test]
    fn test_execution_result_to_json_record() {
        let val = json!({"name": "test", "id": 1});
        let json = execution_result_to_json(&ExecutionResult::Record(val.clone()));
        assert_eq!(json["type"], "record");
        assert_eq!(json["value"], val);
    }

    #[test]
    fn test_execution_result_to_json_record_set() {
        let vals = vec![json!({"a": 1}), json!({"a": 2})];
        let json = execution_result_to_json(&ExecutionResult::RecordSet(vals.clone()));
        assert_eq!(json["type"], "record_set");
        assert_eq!(json["value"].as_array().unwrap().len(), 2);
    }
}
