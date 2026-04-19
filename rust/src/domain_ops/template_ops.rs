//! Template Operations - DSL verbs for template invocation and batch execution
//!
//! Provides:
//! - `template.invoke` - Single template invocation within DSL
//! - `template.batch` - Batch template execution over query results

use anyhow::{anyhow, Result};
use async_trait::async_trait;
use dsl_runtime_macros::register_custom_op;
use std::collections::HashMap;
use uuid::Uuid;

#[cfg(feature = "database")]
use sqlx::PgPool;

use super::CustomOperation;
use crate::dsl_v2::ast::VerbCall;
#[cfg(feature = "database")]
use crate::dsl_v2::execution::DslExecutor;
use crate::dsl_v2::executor::{ExecutionContext, ExecutionResult};
#[cfg(feature = "database")]
use crate::dsl_v2::runtime_registry::runtime_registry;
#[cfg(feature = "database")]
use crate::templates::{ExpansionContext, TemplateExpander};

/// Result of template invocation
#[derive(Debug, Clone)]
pub struct TemplateInvokeResult {
    /// Template ID that was invoked
    pub template_id: String,
    /// Number of statements executed
    pub statements_executed: usize,
    /// Output bindings produced by the template
    pub outputs: HashMap<String, Uuid>,
    /// Primary entity ID if template defines one
    pub primary_entity_id: Option<Uuid>,
}

/// `template.invoke` - Invoke a single template with parameters
///
/// Expands the template to DSL, then executes via the standard pipeline.
/// All template outputs are captured in the execution context.
///
/// Example DSL:
/// ```clojure
/// (template.invoke
///   :id "onboard-fund-cbu"
///   :params {:fund_entity @selected_fund
///            :manco_entity @manco
///            :im_entity @im
///            :jurisdiction "LU"}
///   :as @result)
/// ```
#[register_custom_op]
pub struct TemplateInvokeOp;

#[cfg(feature = "database")]
async fn template_invoke_impl(
    template_id: String,
    explicit_params: HashMap<String, String>,
    ctx: &mut ExecutionContext,
    pool: &PgPool,
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

#[async_trait]
impl CustomOperation for TemplateInvokeOp {
    fn domain(&self) -> &'static str {
        "template"
    }

    fn verb(&self) -> &'static str {
        "invoke"
    }

    fn rationale(&self) -> &'static str {
        "Expands template to DSL and executes via standard pipeline with context propagation"
    }

    #[cfg(feature = "database")]
    async fn execute(
        &self,
        verb_call: &VerbCall,
        ctx: &mut ExecutionContext,
        pool: &PgPool,
    ) -> Result<ExecutionResult> {
        let template_id = verb_call
            .arguments
            .iter()
            .find(|a| a.key == "id")
            .and_then(|a| a.value.as_string())
            .ok_or_else(|| anyhow!("Missing :id argument for template.invoke"))?;
        let explicit_params = extract_params_map(verb_call, ctx)?;
        let result =
            template_invoke_impl(template_id.to_string(), explicit_params, ctx, pool).await?;
        Ok(ExecutionResult::TemplateInvoked(result))
    }

    #[cfg(not(feature = "database"))]
    async fn execute(
        &self,
        _verb_call: &VerbCall,
        _ctx: &mut ExecutionContext,
    ) -> Result<ExecutionResult> {
        Err(anyhow!("template.invoke requires database feature"))
    }

    #[cfg(feature = "database")]
    async fn execute_json(
        &self,
        args: &serde_json::Value,
        ctx: &mut dsl_runtime::VerbExecutionContext,
        pool: &PgPool,
    ) -> Result<dsl_runtime::VerbExecutionOutcome> {
        use super::helpers::json_extract_string;

        let template_id = json_extract_string(args, "id")?;
        let explicit_params = extract_json_string_map(args, ctx, "params")?;
        let mut exec_ctx = crate::sem_os_runtime::verb_executor_adapter::to_dsl_context_pub(ctx);
        let result =
            template_invoke_impl(template_id, explicit_params, &mut exec_ctx, pool).await?;

        for (name, uuid) in &exec_ctx.symbols {
            ctx.bind(name, *uuid);
        }

        Ok(dsl_runtime::VerbExecutionOutcome::Record(
            serde_json::json!({"_type": "template_invoked", "_debug": format!("{result:?}")}),
        ))
    }

    fn is_migrated(&self) -> bool {
        true
    }
}

/// Extract :params map from verb call, resolving symbol references
#[cfg(feature = "database")]
fn extract_params_map(
    verb_call: &VerbCall,
    ctx: &ExecutionContext,
) -> Result<HashMap<String, String>> {
    let mut params = HashMap::new();

    // Find the :params argument
    let params_arg = verb_call.arguments.iter().find(|a| a.key == "params");

    if let Some(arg) = params_arg {
        // Expect a Map node
        if let Some(entries) = arg.value.as_map() {
            for (key, value) in entries {
                let resolved = resolve_value_to_string(value, ctx)?;
                params.insert(key.clone(), resolved);
            }
        }
    }

    // Also extract any top-level arguments that aren't reserved
    let reserved = ["id", "params", "as"];
    for arg in &verb_call.arguments {
        if !reserved.contains(&arg.key.as_str()) {
            let resolved = resolve_value_to_string(&arg.value, ctx)?;
            params.insert(arg.key.clone(), resolved);
        }
    }

    Ok(params)
}

/// Resolve an AST node to a string value, handling symbol references
#[cfg(feature = "database")]
fn resolve_value_to_string(
    node: &crate::dsl_v2::ast::AstNode,
    ctx: &ExecutionContext,
) -> Result<String> {
    use crate::dsl_v2::ast::AstNode;

    match node {
        AstNode::Literal(lit, _) => Ok(literal_to_string(lit)),
        AstNode::SymbolRef { name, .. } => {
            let uuid = ctx
                .resolve(name)
                .ok_or_else(|| anyhow!("Unresolved symbol reference: @{}", name))?;
            Ok(uuid.to_string())
        }
        AstNode::EntityRef {
            resolved_key,
            value,
            ..
        } => {
            // Use resolved key if available, otherwise the display value
            Ok(resolved_key.clone().unwrap_or_else(|| value.clone()))
        }
        AstNode::List { items, .. } => {
            let resolved: Result<Vec<String>> = items
                .iter()
                .map(|item| resolve_value_to_string(item, ctx))
                .collect();
            Ok(format!("[{}]", resolved?.join(", ")))
        }
        AstNode::Map { entries, .. } => {
            let resolved: Result<Vec<String>> = entries
                .iter()
                .map(|(k, v)| {
                    let rv = resolve_value_to_string(v, ctx)?;
                    Ok(format!("{}: {}", k, rv))
                })
                .collect();
            Ok(format!("{{{}}}", resolved?.join(", ")))
        }
        AstNode::Nested(_) => Err(anyhow!("Nested verb calls not supported in params")),
    }
}

/// Convert a Literal to a string representation
#[cfg(feature = "database")]
fn literal_to_string(lit: &crate::dsl_v2::ast::Literal) -> String {
    use crate::dsl_v2::ast::Literal;
    match lit {
        Literal::String(s) => s.clone(),
        Literal::Integer(i) => i.to_string(),
        Literal::Decimal(d) => d.to_string(),
        Literal::Boolean(b) => b.to_string(),
        Literal::Null => "nil".to_string(),
        Literal::Uuid(u) => u.to_string(),
    }
}

// ============================================================================
// TemplateBatchOp - Batch template execution over query results
// ============================================================================

#[cfg(feature = "database")]
use crate::dsl_v2::batch_executor::{BatchExecutor, BatchResultAccumulator, OnErrorMode};

/// Result of batch template execution
#[derive(Debug, Clone)]
pub struct TemplateBatchResult {
    /// Template ID that was executed
    pub template_id: String,
    /// Number of items processed
    pub total_items: usize,
    /// Number of successful iterations
    pub success_count: usize,
    /// Number of failed iterations
    pub failure_count: usize,
    /// Primary entity IDs from successful iterations (e.g., CBU IDs)
    pub primary_entity_ids: Vec<Uuid>,
    /// Primary entity type (e.g., "cbu")
    pub primary_entity_type: String,
    /// Whether batch was aborted early
    pub aborted: bool,
    /// Errors by iteration index
    pub errors: HashMap<usize, String>,
}

impl TemplateBatchResult {
    /// Create from BatchResultAccumulator
    #[cfg(feature = "database")]
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

    /// Get all primary entity IDs (convenience method)
    pub fn cbu_ids(&self) -> &[Uuid] {
        &self.primary_entity_ids
    }
}

/// `template.batch` - Batch template execution over a query result set
///
/// Iterates over entities from `entity.query` and executes a template for each,
/// with proper context isolation per iteration and parent binding inheritance.
///
/// Example DSL:
/// ```clojure
/// ;; First query entities
/// (entity.query :type "fund" :name-like "Allianz%" :jurisdiction "LU" :as @funds)
///
/// ;; Batch execute template
/// (template.batch
///   :id "onboard-fund-cbu"
///   :source @funds                  ;; Binding from entity.query
///   :bind-as "fund_entity"          ;; Each item binds to this param
///   :shared {:manco_entity @manco
///            :im_entity @im
///            :jurisdiction "LU"}
///   :on-error :continue             ;; :stop | :continue | :rollback
///   :limit 10                       ;; Optional limit
///   :as @batch_result)
/// ```
#[register_custom_op]
pub struct TemplateBatchOp;

#[cfg(feature = "database")]
async fn template_batch_impl(
    template_id: String,
    source_symbol: String,
    bind_param: String,
    shared_params: HashMap<String, String>,
    on_error: OnErrorMode,
    limit: Option<usize>,
    ctx: &mut ExecutionContext,
    pool: &PgPool,
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

#[async_trait]
impl CustomOperation for TemplateBatchOp {
    fn domain(&self) -> &'static str {
        "template"
    }

    fn verb(&self) -> &'static str {
        "batch"
    }

    fn rationale(&self) -> &'static str {
        "Iterates over query results executing template per item with context isolation"
    }

    #[cfg(feature = "database")]
    async fn execute(
        &self,
        verb_call: &VerbCall,
        ctx: &mut ExecutionContext,
        pool: &PgPool,
    ) -> Result<ExecutionResult> {
        let template_id = verb_call
            .arguments
            .iter()
            .find(|a| a.key == "id")
            .and_then(|a| a.value.as_string())
            .ok_or_else(|| anyhow!("Missing :id argument for template.batch"))?;
        let source_symbol = verb_call
            .arguments
            .iter()
            .find(|a| a.key == "source")
            .and_then(|a| {
                if let crate::dsl_v2::ast::AstNode::SymbolRef { name, .. } = &a.value {
                    Some(name.as_str())
                } else {
                    None
                }
            })
            .ok_or_else(|| anyhow!("Missing :source symbol for template.batch"))?;
        let bind_param = verb_call
            .arguments
            .iter()
            .find(|a| a.key == "bind-as")
            .and_then(|a| a.value.as_string())
            .unwrap_or("entity")
            .to_string();
        let shared_params = extract_shared_params(verb_call, ctx)?;
        let on_error_str = verb_call
            .arguments
            .iter()
            .find(|a| a.key == "on-error")
            .and_then(|a| a.value.as_string())
            .unwrap_or("continue");
        let on_error: OnErrorMode = on_error_str.parse().unwrap_or_default();
        let limit: Option<usize> = verb_call
            .arguments
            .iter()
            .find(|a| a.key == "limit")
            .and_then(|a| a.value.as_integer())
            .map(|i| i as usize);
        let result = template_batch_impl(
            template_id.to_string(),
            source_symbol.to_string(),
            bind_param,
            shared_params,
            on_error,
            limit,
            ctx,
            pool,
        )
        .await?;
        Ok(ExecutionResult::TemplateBatch(result))
    }

    #[cfg(not(feature = "database"))]
    async fn execute(
        &self,
        _verb_call: &VerbCall,
        _ctx: &mut ExecutionContext,
    ) -> Result<ExecutionResult> {
        Err(anyhow!("template.batch requires database feature"))
    }

    #[cfg(feature = "database")]
    async fn execute_json(
        &self,
        args: &serde_json::Value,
        ctx: &mut dsl_runtime::VerbExecutionContext,
        pool: &PgPool,
    ) -> Result<dsl_runtime::VerbExecutionOutcome> {
        use super::helpers::{json_extract_int_opt, json_extract_string, json_extract_string_opt};

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
        let result = template_batch_impl(
            template_id,
            source_symbol,
            bind_param,
            shared_params,
            on_error,
            limit,
            &mut exec_ctx,
            pool,
        )
        .await?;

        for (name, uuid) in &exec_ctx.symbols {
            ctx.bind(name, *uuid);
        }

        Ok(dsl_runtime::VerbExecutionOutcome::Record(
            serde_json::json!({"_type": "template_batch", "_debug": format!("{result:?}")}),
        ))
    }

    fn is_migrated(&self) -> bool {
        true
    }
}

/// Extract :shared params map from verb call
#[cfg(feature = "database")]
fn extract_shared_params(
    verb_call: &VerbCall,
    ctx: &ExecutionContext,
) -> Result<HashMap<String, String>> {
    let mut params = HashMap::new();

    // Find the :shared argument
    let shared_arg = verb_call.arguments.iter().find(|a| a.key == "shared");

    if let Some(arg) = shared_arg {
        if let Some(entries) = arg.value.as_map() {
            for (key, value) in entries {
                let resolved = resolve_value_to_string(value, ctx)?;
                params.insert(key.clone(), resolved);
            }
        }
    }

    Ok(params)
}

#[cfg(feature = "database")]
fn extract_json_string_map(
    args: &serde_json::Value,
    ctx: &dsl_runtime::VerbExecutionContext,
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

/// Get entity query items from context
///
/// The source symbol should have been bound by a prior `entity.query` call,
/// or be the special `@session_cbus` symbol which provides session's active CBUs.
///
/// Special symbols:
/// - `@session_cbus` - Returns all CBU IDs currently in session scope
#[cfg(feature = "database")]
fn get_entity_query_items(
    source_symbol: &str,
    ctx: &ExecutionContext,
) -> Result<Vec<(Uuid, String)>> {
    // Check for special @session_cbus symbol - iterates over session's active CBU set
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
        // Return CBU IDs with a placeholder name (the actual name can be resolved later)
        return Ok(cbu_ids
            .iter()
            .map(|id| (*id, format!("cbu:{}", id)))
            .collect());
    }

    // Check for other special symbols or context bindings
    // TODO: Add support for EntityQueryResult storage in ExecutionContext
    // query_results: HashMap<String, EntityQueryResult>

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
