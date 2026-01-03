//! Template Operations - DSL verbs for template invocation and batch execution
//!
//! Provides:
//! - `template.invoke` - Single template invocation within DSL
//! - `template.batch` - Batch template execution over query results

use anyhow::{anyhow, Result};
use async_trait::async_trait;
use std::collections::HashMap;
use uuid::Uuid;

#[cfg(feature = "database")]
use sqlx::PgPool;

use super::CustomOperation;
use crate::dsl_v2::ast::VerbCall;
use crate::dsl_v2::executor::{ExecutionContext, ExecutionResult};
#[cfg(feature = "database")]
use crate::dsl_v2::runtime_registry::runtime_registry;
#[cfg(feature = "database")]
use crate::dsl_v2::DslExecutor;
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
pub struct TemplateInvokeOp;

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
        // Extract :id argument (template ID)
        let template_id = verb_call
            .arguments
            .iter()
            .find(|a| a.key == "id")
            .and_then(|a| a.value.as_string())
            .ok_or_else(|| anyhow!("Missing :id argument for template.invoke"))?;

        // Extract :params map (explicit parameters)
        let explicit_params = extract_params_map(verb_call, ctx)?;

        // Get template from registry
        let registry = runtime_registry();
        let template = registry
            .get_template(template_id)
            .ok_or_else(|| anyhow!("Template not found: {}", template_id))?;

        // Build expansion context from current execution context
        let exp_ctx = ExpansionContext {
            current_cbu: ctx.resolve("cbu"),
            current_case: ctx.resolve("case"),
            bindings: ctx.all_bindings_as_strings(),
            binding_types: ctx.effective_symbol_types(),
        };

        // Expand template
        let expansion = TemplateExpander::expand(template, &explicit_params, &exp_ctx);

        // Check for missing required params
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

        // Parse expanded DSL
        let program = crate::dsl_v2::parser::parse_program(&expansion.dsl)
            .map_err(|e| anyhow!("Template '{}' parse error: {}", template_id, e))?;

        // Compile to execution plan
        let plan = crate::dsl_v2::execution_plan::compile(&program)
            .map_err(|e| anyhow!("Template '{}' compile error: {:?}", template_id, e))?;

        tracing::debug!(
            "template.invoke: compiled {} statements from '{}'",
            plan.steps.len(),
            template_id
        );

        // Execute plan (nested execution, same context)
        let executor = DslExecutor::new(pool.clone());
        let results = executor.execute_plan(&plan, ctx).await?;

        // Collect output bindings from context
        let mut outputs: HashMap<String, Uuid> = HashMap::new();
        for output_name in &expansion.outputs {
            if let Some(pk) = ctx.resolve(output_name) {
                outputs.insert(output_name.clone(), pk);
            }
        }

        // Determine primary entity (from template primary_entity param or first output)
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
        AstNode::Literal(lit) => Ok(literal_to_string(lit)),
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
pub struct TemplateBatchOp;

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
        // 1. Extract :id (template ID)
        let template_id = verb_call
            .arguments
            .iter()
            .find(|a| a.key == "id")
            .and_then(|a| a.value.as_string())
            .ok_or_else(|| anyhow!("Missing :id argument for template.batch"))?;

        // 2. Extract :source (symbol reference to entity.query result)
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

        // 3. Extract :bind-as (parameter name to bind each entity ID to)
        let bind_param = verb_call
            .arguments
            .iter()
            .find(|a| a.key == "bind-as")
            .and_then(|a| a.value.as_string())
            .unwrap_or("entity")
            .to_string();

        // 4. Extract :shared params map
        let shared_params = extract_shared_params(verb_call, ctx)?;

        // 5. Extract :on-error mode
        let on_error_str = verb_call
            .arguments
            .iter()
            .find(|a| a.key == "on-error")
            .and_then(|a| a.value.as_string())
            .unwrap_or("continue");
        let on_error: OnErrorMode = on_error_str.parse().unwrap_or_default();

        // 6. Extract :limit (optional)
        let limit: Option<usize> = verb_call
            .arguments
            .iter()
            .find(|a| a.key == "limit")
            .and_then(|a| a.value.as_integer())
            .map(|i| i as usize);

        // 7. Get the source entity query result from context
        // The source should be an EntityQueryResult stored as a special binding
        let items = get_entity_query_items(source_symbol, ctx)?;

        if items.is_empty() {
            tracing::info!(
                "template.batch: source @{} is empty, skipping",
                source_symbol
            );
            let result = TemplateBatchResult {
                template_id: template_id.to_string(),
                total_items: 0,
                success_count: 0,
                failure_count: 0,
                primary_entity_ids: vec![],
                primary_entity_type: "unknown".to_string(),
                aborted: false,
                errors: HashMap::new(),
            };
            return Ok(ExecutionResult::TemplateBatch(result));
        }

        tracing::info!(
            "template.batch: starting '{}' with {} items from @{}",
            template_id,
            items.len(),
            source_symbol
        );

        // 8. Get template from registry
        let registry = runtime_registry();
        let template = registry
            .get_template(template_id)
            .ok_or_else(|| anyhow!("Template not found: {}", template_id))?;

        // 9. Create parent context snapshot for batch executor
        // BatchExecutor needs an owned ExecutionContext, so create one from current state
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
        };

        // 10. Create and run BatchExecutor
        let batch_executor =
            BatchExecutor::new(pool.clone(), template.clone(), shared_params, parent_ctx);

        let batch_result = batch_executor
            .execute_batch(items.clone(), &bind_param, on_error, limit)
            .await?;

        // 11. Propagate primary entity IDs to context for post-batch operations
        // Store the batch result so subsequent verbs can access the created PKs
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

/// Get entity query items from context
///
/// The source symbol should have been bound by a prior `entity.query` call.
/// We need to retrieve the EntityQueryResult from execution state.
#[cfg(feature = "database")]
fn get_entity_query_items(
    _source_symbol: &str,
    _ctx: &ExecutionContext,
) -> Result<Vec<(Uuid, String)>> {
    // TODO: The EntityQueryResult needs to be stored somewhere accessible.
    // For now, we'll need to extend ExecutionContext to store query results.
    // This is a placeholder - the actual implementation requires adding
    // query_results: HashMap<String, EntityQueryResult> to ExecutionContext.

    // As a workaround, we can re-query based on the source symbol if it
    // contains the query parameters, or require the batch to be passed
    // the items directly via a different mechanism.

    Err(anyhow!(
        "EntityQueryResult retrieval not yet implemented. \
         Consider using batch_executor directly with items."
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
