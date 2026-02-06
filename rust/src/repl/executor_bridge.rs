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

use super::orchestrator_v2::DslExecutor;
use crate::dsl_v2::{compile, parse_program, ExecutionContext, ExecutionResult};

// ---------------------------------------------------------------------------
// RealDslExecutor
// ---------------------------------------------------------------------------

/// Bridge from the v2 REPL `DslExecutor` trait to the real dsl-core pipeline.
///
/// Lifecycle: parse → compile → execute → collect results as JSON.
pub struct RealDslExecutor {
    pool: PgPool,
}

impl RealDslExecutor {
    pub fn new(pool: PgPool) -> Self {
        Self { pool }
    }
}

#[async_trait::async_trait]
impl DslExecutor for RealDslExecutor {
    async fn execute(&self, dsl: &str) -> Result<serde_json::Value, String> {
        // 1. Parse DSL string → Program AST.
        let program = parse_program(dsl).map_err(|e| format!("Parse error: {}", e))?;

        // 2. Compile → ExecutionPlan (topological sort, injections).
        let plan = compile(&program).map_err(|e| format!("Compile error: {:?}", e))?;

        // 3. Build execution context.
        let mut ctx = ExecutionContext::new();
        ctx.execution_id = Uuid::new_v4();

        // 4. Execute via the real dsl_v2 executor.
        let executor = crate::dsl_v2::executor::DslExecutor::new(self.pool.clone());
        let results = executor
            .execute_plan(&plan, &mut ctx)
            .await
            .map_err(|e| format!("Execution error: {}", e))?;

        // 5. Convert results to JSON.
        let step_results: Vec<serde_json::Value> =
            results.iter().map(execution_result_to_json).collect();

        let bindings: serde_json::Map<String, serde_json::Value> = ctx
            .symbols
            .iter()
            .map(|(k, v)| (k.clone(), json!(v.to_string())))
            .collect();

        Ok(json!({
            "success": true,
            "steps_executed": step_results.len(),
            "results": step_results,
            "bindings": bindings,
        }))
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
