//! Shared delegation helpers for Semantic Registry CustomOps.
//!
//! Every sem_reg CustomOp follows the same pattern:
//! 1. Build `SemRegToolContext` from `ExecutionContext` + `PgPool`
//! 2. Extract tool arguments from `VerbCall`
//! 3. Delegate to `dispatch_tool()` (or stewardship dispatch)
//! 4. Convert `SemRegToolResult` → `ExecutionResult`

use anyhow::{anyhow, Result};
use serde_json::json;

use crate::dsl_v2::ast::VerbCall;
use crate::dsl_v2::executor::{ExecutionContext, ExecutionResult};
use crate::sem_reg::abac::ActorContext;
use crate::sem_reg::agent::mcp_tools::{SemRegToolContext, SemRegToolResult};

#[cfg(feature = "database")]
use sqlx::PgPool;

/// Build an `ActorContext` from the execution context.
pub fn build_actor_from_ctx(ctx: &ExecutionContext) -> ActorContext {
    let actor_id = ctx.audit_user.as_deref().unwrap_or("dsl_executor");
    ActorContext {
        actor_id: actor_id.to_string(),
        roles: vec!["operator".to_string()],
        department: None,
        clearance: Some(crate::sem_reg::types::Classification::Internal),
        jurisdictions: vec![],
    }
}

/// Extract DSL verb arguments as a JSON object suitable for MCP tool dispatch.
///
/// Maps VerbCall arguments to a flat JSON object where each key is the
/// argument name and each value is the string/uuid representation.
pub fn extract_args_as_json(verb_call: &VerbCall, ctx: &ExecutionContext) -> serde_json::Value {
    let mut map = serde_json::Map::new();
    for arg in &verb_call.arguments {
        let key = arg.key.clone();
        // Try symbol reference first
        if let Some(sym) = arg.value.as_symbol() {
            if let Some(uuid) = ctx.resolve(sym) {
                map.insert(key, json!(uuid.to_string()));
                continue;
            }
        }
        // Try UUID literal
        if let Some(uuid) = arg.value.as_uuid() {
            map.insert(key, json!(uuid.to_string()));
            continue;
        }
        // Try string literal
        if let Some(s) = arg.value.as_string() {
            map.insert(key, json!(s));
            continue;
        }
        // Try integer
        if let Some(n) = arg.value.as_integer() {
            map.insert(key, json!(n));
            continue;
        }
        // Try boolean
        if let Some(b) = arg.value.as_boolean() {
            map.insert(key, json!(b));
            continue;
        }
        // Fallback: use debug representation
        map.insert(key, json!(format!("{:?}", arg.value)));
    }
    serde_json::Value::Object(map)
}

/// Extract a specific string argument from a VerbCall.
pub fn get_string_arg(verb_call: &VerbCall, name: &str) -> Option<String> {
    verb_call
        .arguments
        .iter()
        .find(|a| a.key == name)
        .and_then(|a| a.value.as_string().map(|s| s.to_string()))
}

/// Extract a specific optional integer argument from a VerbCall.
#[allow(dead_code)]
pub fn get_int_arg(verb_call: &VerbCall, name: &str) -> Option<i64> {
    verb_call
        .arguments
        .iter()
        .find(|a| a.key == name)
        .and_then(|a| a.value.as_integer())
}

/// Extract a specific optional boolean argument from a VerbCall.
pub fn get_bool_arg(verb_call: &VerbCall, name: &str) -> Option<bool> {
    verb_call
        .arguments
        .iter()
        .find(|a| a.key == name)
        .and_then(|a| a.value.as_boolean())
}

/// Convert a `SemRegToolResult` into an `ExecutionResult`.
///
/// - Success with data → `ExecutionResult::Record(data)`
/// - Failure → `Err` with the error message
pub fn convert_tool_result(result: SemRegToolResult) -> Result<ExecutionResult> {
    if result.success {
        Ok(ExecutionResult::Record(result.data))
    } else {
        Err(anyhow!(
            "{}",
            result
                .error
                .unwrap_or_else(|| "Unknown tool error".to_string())
        ))
    }
}

/// Delegate to `dispatch_tool()` and convert the result.
///
/// This is the main entry point for most sem_reg CustomOps:
/// ```ignore
/// delegate_to_tool(pool, ctx, verb_call, "sem_reg_describe_attribute").await
/// ```
#[cfg(feature = "database")]
pub async fn delegate_to_tool(
    pool: &PgPool,
    ctx: &ExecutionContext,
    verb_call: &VerbCall,
    tool_name: &str,
) -> Result<ExecutionResult> {
    let actor = build_actor_from_ctx(ctx);
    let tool_ctx = SemRegToolContext {
        pool,
        actor: &actor,
    };
    let args = extract_args_as_json(verb_call, ctx);
    let result = crate::sem_reg::agent::mcp_tools::dispatch_tool(&tool_ctx, tool_name, &args).await;
    convert_tool_result(result)
}

/// Delegate to stewardship phase 0 tool dispatch.
#[cfg(feature = "database")]
pub async fn delegate_to_stew_tool(
    pool: &PgPool,
    ctx: &ExecutionContext,
    verb_call: &VerbCall,
    tool_name: &str,
) -> Result<ExecutionResult> {
    let actor = build_actor_from_ctx(ctx);
    let tool_ctx = SemRegToolContext {
        pool,
        actor: &actor,
    };
    let args = extract_args_as_json(verb_call, ctx);

    // Try phase 0 first, then phase 1
    if let Some(result) =
        crate::sem_reg::stewardship::dispatch_phase0_tool(&tool_ctx, tool_name, &args).await
    {
        return convert_tool_result(result);
    }
    if let Some(result) =
        crate::sem_reg::stewardship::dispatch_phase1_tool(&tool_ctx, tool_name, &args).await
    {
        return convert_tool_result(result);
    }

    Err(anyhow!("Unknown stewardship tool: {}", tool_name))
}
