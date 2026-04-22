//! Template verbs (2 plugin verbs) — `template.invoke` (single
//! invocation) and `template.batch` (iteration over query results).
//!
//! Phase 5c-migrate Phase B Pattern B slice #74: ported from
//! `CustomOperation` + `inventory::collect!` to `SemOsVerbOp`. Stays
//! in `ob-poc::domain_ops::template_ops` because the ops bridge to
//! `crate::templates::TemplateExpander`, `crate::dsl_v2::{parser,
//! execution_plan, execution::DslExecutor, batch_executor::BatchExecutor,
//! runtime_registry}` — all upstream of `sem_os_postgres`.
//!
//! Result DTOs `TemplateInvokeResult` / `TemplateBatchResult` stay
//! here — they're only used internally by this module and (for debug
//! projections) by `ExecutionResult::TemplateInvoked` /
//! `ExecutionResult::TemplateBatch` in `dsl_v2::executor`.

use anyhow::{anyhow, Result};
use async_trait::async_trait;
use sem_os_postgres::ops::SemOsVerbOp;
use std::collections::HashMap;
use uuid::Uuid;

use dsl_runtime::domain_ops::helpers::{
    json_extract_int_opt, json_extract_string, json_extract_string_opt,
};
use dsl_runtime::tx::TransactionScope;
use dsl_runtime::{VerbExecutionContext, VerbExecutionOutcome};

use crate::dsl_v2::execution::DslExecutor;
use crate::dsl_v2::executor::ExecutionContext;
use crate::dsl_v2::runtime_registry::runtime_registry;
use crate::templates::{ExpansionContext, TemplateExpander};

// =============================================================================
// Result types
// =============================================================================

#[derive(Debug, Clone)]
pub struct TemplateInvokeResult {
    pub template_id: String,
    pub statements_executed: usize,
    pub outputs: HashMap<String, Uuid>,
    pub primary_entity_id: Option<Uuid>,
}

use crate::dsl_v2::batch_executor::{BatchExecutor, BatchResultAccumulator, OnErrorMode};

#[derive(Debug, Clone)]
pub struct TemplateBatchResult {
    pub template_id: String,
    pub total_items: usize,
    pub success_count: usize,
    pub failure_count: usize,
    pub primary_entity_ids: Vec<Uuid>,
    pub primary_entity_type: String,
    pub aborted: bool,
    pub errors: HashMap<usize, String>,
}

impl TemplateBatchResult {
    pub fn from_accumulator(
        template_id: String,
        acc: &BatchResultAccumulator,
        total_items: usize,
        aborted: bool,
    ) -> Self {
        Self {
            template_id,
            total_items,
            success_count: acc.success_count,
            failure_count: acc.failure_count,
            primary_entity_ids: acc.primary_entity_ids.clone(),
            primary_entity_type: acc.primary_entity_type.clone(),
            aborted,
            errors: acc.errors.clone(),
        }
    }

    pub fn cbu_ids(&self) -> &[Uuid] {
        &self.primary_entity_ids
    }
}

// =============================================================================
// template.invoke
// =============================================================================

pub struct TemplateInvoke;

#[async_trait]
impl SemOsVerbOp for TemplateInvoke {
    fn fqn(&self) -> &str {
        "template.invoke"
    }
    async fn execute(
        &self,
        args: &serde_json::Value,
        ctx: &mut VerbExecutionContext,
        scope: &mut dyn TransactionScope,
    ) -> Result<VerbExecutionOutcome> {
        let template_id = json_extract_string(args, "id")?;
        let explicit_params = extract_json_string_map(args, ctx, "params")?;
        let mut exec_ctx = crate::sem_os_runtime::verb_executor_adapter::to_dsl_context_pub(ctx);
        let pool = scope.pool().clone();
        let result =
            template_invoke_impl(template_id, explicit_params, &mut exec_ctx, &pool).await?;

        for (name, uuid) in &exec_ctx.symbols {
            ctx.bind(name, *uuid);
        }

        Ok(VerbExecutionOutcome::Record(
            serde_json::json!({"_type": "template_invoked", "_debug": format!("{result:?}")}),
        ))
    }
}

async fn template_invoke_impl(
    template_id: String,
    explicit_params: HashMap<String, String>,
    ctx: &mut ExecutionContext,
    pool: &sqlx::PgPool,
) -> Result<TemplateInvokeResult> {
    let registry = runtime_registry();
    let template = registry
        .get_template(&template_id)
        .ok_or_else(|| anyhow!("Template not found: {}", template_id))?;

    let exp_ctx = ExpansionContext {
        current_cbu: ctx.resolve("cbu"),
        current_case: ctx.resolve("case"),
        bindings: ctx.all_bindings_as_strings(),
        binding_types: ctx.effective_symbol_types(),
    };

    let expansion = TemplateExpander::expand(template, &explicit_params, &exp_ctx);

    if !expansion.missing_params.is_empty() {
        let missing_names: Vec<String> = expansion
            .missing_params
            .iter()
            .map(|p| p.name.clone())
            .collect();
        return Err(anyhow!(
            "Template '{}' missing required params: {}",
            template_id,
            missing_names.join(", ")
        ));
    }

    tracing::debug!(
        "template.invoke: expanded '{}' to {} bytes DSL",
        template_id,
        expansion.dsl.len()
    );

    let program = crate::dsl_v2::parser::parse_program(&expansion.dsl)
        .map_err(|e| anyhow!("Template '{}' parse error: {}", template_id, e))?;
    let plan = crate::dsl_v2::execution_plan::compile(&program)
        .map_err(|e| anyhow!("Template '{}' compile error: {:?}", template_id, e))?;

    tracing::debug!(
        "template.invoke: compiled {} statements from '{}'",
        plan.steps.len(),
        template_id
    );

    let executor = DslExecutor::new(pool.clone());
    let results = executor.execute_plan(&plan, ctx).await?;

    let mut outputs: HashMap<String, Uuid> = HashMap::new();
    for output_name in &expansion.outputs {
        if let Some(pk) = ctx.resolve(output_name) {
            outputs.insert(output_name.clone(), pk);
        }
    }

    let primary_entity_id = template
        .primary_entity_param()
        .and_then(|param_name| ctx.resolve(param_name))
        .or_else(|| outputs.values().next().copied());

    let result = TemplateInvokeResult {
        template_id: template_id.to_string(),
        statements_executed: results.len(),
        outputs,
        primary_entity_id,
    };

    tracing::info!(
        "template.invoke: '{}' completed, {} statements, primary={:?}",
        result.template_id,
        result.statements_executed,
        result.primary_entity_id
    );

    Ok(result)
}

// =============================================================================
// template.batch
// =============================================================================

pub struct TemplateBatch;

#[async_trait]
impl SemOsVerbOp for TemplateBatch {
    fn fqn(&self) -> &str {
        "template.batch"
    }
    async fn execute(
        &self,
        args: &serde_json::Value,
        ctx: &mut VerbExecutionContext,
        scope: &mut dyn TransactionScope,
    ) -> Result<VerbExecutionOutcome> {
        let template_id = json_extract_string(args, "id")?;
        let source_symbol = json_extract_string(args, "source")?
            .trim_start_matches('@')
            .to_string();
        let bind_param =
            json_extract_string_opt(args, "bind-as").unwrap_or_else(|| "entity".to_string());
        let shared_params = extract_json_string_map(args, ctx, "shared")?;
        let on_error_str =
            json_extract_string_opt(args, "on-error").unwrap_or_else(|| "continue".to_string());
        let on_error: OnErrorMode = on_error_str.parse().unwrap_or_default();
        let limit = json_extract_int_opt(args, "limit").map(|i| i as usize);

        let mut exec_ctx = crate::sem_os_runtime::verb_executor_adapter::to_dsl_context_pub(ctx);
        let pool = scope.pool().clone();
        let result = template_batch_impl(
            template_id,
            source_symbol,
            bind_param,
            shared_params,
            on_error,
            limit,
            &mut exec_ctx,
            &pool,
        )
        .await?;

        for (name, uuid) in &exec_ctx.symbols {
            ctx.bind(name, *uuid);
        }

        Ok(VerbExecutionOutcome::Record(
            serde_json::json!({"_type": "template_batch", "_debug": format!("{result:?}")}),
        ))
    }
}

async fn template_batch_impl(
    template_id: String,
    source_symbol: String,
    bind_param: String,
    shared_params: HashMap<String, String>,
    on_error: OnErrorMode,
    limit: Option<usize>,
    ctx: &mut ExecutionContext,
    pool: &sqlx::PgPool,
) -> Result<TemplateBatchResult> {
    let items = get_entity_query_items(&source_symbol, ctx)?;

    if items.is_empty() {
        tracing::info!(
            "template.batch: source @{} is empty, skipping",
            source_symbol
        );
        return Ok(TemplateBatchResult {
            template_id,
            total_items: 0,
            success_count: 0,
            failure_count: 0,
            primary_entity_ids: vec![],
            primary_entity_type: "unknown".to_string(),
            aborted: false,
            errors: HashMap::new(),
        });
    }

    tracing::info!(
        "template.batch: starting '{}' with {} items from @{}",
        template_id,
        items.len(),
        source_symbol
    );

    let registry = runtime_registry();
    let template = registry
        .get_template(&template_id)
        .ok_or_else(|| anyhow!("Template not found: {}", template_id))?;

    let parent_ctx = ExecutionContext {
        symbols: ctx.symbols.clone(),
        symbol_types: ctx.symbol_types.clone(),
        parent_symbols: ctx.parent_symbols.clone(),
        parent_symbol_types: ctx.parent_symbol_types.clone(),
        batch_index: ctx.batch_index,
        audit_user: ctx.audit_user.clone(),
        transaction_id: ctx.transaction_id,
        execution_id: ctx.execution_id,
        idempotency_enabled: ctx.idempotency_enabled,
        json_bindings: ctx.json_bindings.clone(),
        current_selection: ctx.current_selection.clone(),
        pending_view_state: None,
        pending_viewport_state: None,
        pending_scope_change: None,
        source_attribution: ctx.source_attribution.clone(),
        session_id: ctx.session_id,
        pending_session: None,
        session_cbu_ids: ctx.session_cbu_ids.clone(),
        client_group_id: ctx.client_group_id,
        client_group_name: ctx.client_group_name.clone(),
        persona: ctx.persona.clone(),
        pending_session_name: None,
        pending_dag_flags: Vec::new(),
        pending_structure_id: None,
        pending_structure_name: None,
        pending_case_id: None,
        pending_mandate_id: None,
        pending_deal_id: None,
        pending_deal_name: None,
        cbu_scope_dirty: false,
        allow_durable_direct: ctx.allow_durable_direct,
    };

    let batch_executor =
        BatchExecutor::new(pool.clone(), template.clone(), shared_params, parent_ctx);

    let batch_result = batch_executor
        .execute_batch(items.clone(), &bind_param, on_error, limit)
        .await?;

    let result = TemplateBatchResult::from_accumulator(
        template_id.to_string(),
        &batch_result.accumulator,
        batch_result.total_items,
        batch_result.aborted,
    );

    tracing::info!(
        "template.batch: '{}' completed, {}/{} succeeded, {} primary entities",
        result.template_id,
        result.success_count,
        result.total_items,
        result.primary_entity_ids.len()
    );

    Ok(result)
}

// =============================================================================
// Helpers
// =============================================================================

fn extract_json_string_map(
    args: &serde_json::Value,
    ctx: &VerbExecutionContext,
    key: &str,
) -> Result<HashMap<String, String>> {
    let mut params = HashMap::new();
    if let Some(entries) = args.get(key).and_then(|v| v.as_object()) {
        for (map_key, value) in entries {
            let resolved = match value {
                serde_json::Value::String(s) => {
                    if let Some(sym) = s.strip_prefix('@') {
                        ctx.resolve(sym)
                            .map(|u| u.to_string())
                            .unwrap_or_else(|| s.clone())
                    } else {
                        s.clone()
                    }
                }
                other => other.to_string(),
            };
            params.insert(map_key.clone(), resolved);
        }
    }
    Ok(params)
}

/// Get entity query items from context.
///
/// The source symbol should have been bound by a prior `entity.query` call,
/// or be the special `@session_cbus` symbol providing the session's CBU scope.
fn get_entity_query_items(
    source_symbol: &str,
    ctx: &ExecutionContext,
) -> Result<Vec<(Uuid, String)>> {
    if source_symbol == "session_cbus" {
        let cbu_ids = ctx.session_cbu_ids();
        if cbu_ids.is_empty() {
            tracing::info!("@session_cbus: No CBUs in session scope");
            return Ok(vec![]);
        }
        tracing::info!(
            "@session_cbus: Providing {} CBUs from session scope for iteration",
            cbu_ids.len()
        );
        return Ok(cbu_ids
            .iter()
            .map(|id| (*id, format!("cbu:{}", id)))
            .collect());
    }

    Err(anyhow!(
        "Source symbol @{} not found. Use @session_cbus for session CBUs, \
         or ensure the symbol was bound by a prior entity.query call.",
        source_symbol
    ))
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_template_invoke_result() {
        let result = TemplateInvokeResult {
            template_id: "test-template".to_string(),
            statements_executed: 5,
            outputs: HashMap::new(),
            primary_entity_id: None,
        };
        assert_eq!(result.template_id, "test-template");
        assert_eq!(result.statements_executed, 5);
    }

    #[test]
    fn test_template_batch_result() {
        let result = TemplateBatchResult {
            template_id: "test-batch".to_string(),
            total_items: 10,
            success_count: 8,
            failure_count: 2,
            primary_entity_ids: vec![Uuid::new_v4(), Uuid::new_v4()],
            primary_entity_type: "cbu".to_string(),
            aborted: false,
            errors: HashMap::new(),
        };
        assert_eq!(result.template_id, "test-batch");
        assert_eq!(result.success_count, 8);
        assert_eq!(result.cbu_ids().len(), 2);
    }
}
