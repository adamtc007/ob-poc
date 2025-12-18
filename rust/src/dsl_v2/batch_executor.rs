//! Batch Executor for Template Iteration
//!
//! Provides batch template execution with context propagation where each iteration
//! runs the full DSL pipeline (expand -> parse -> compile -> execute) with proper
//! DAG ordering respected within each iteration.
//!
//! Key features:
//! - Child contexts with parent binding inheritance
//! - Per-iteration symbol isolation
//! - BatchResultAccumulator for post-batch operations
//! - Error handling modes: continue, stop, rollback

use std::collections::HashMap;
use uuid::Uuid;

#[cfg(feature = "database")]
use anyhow::{anyhow, Result};
#[cfg(feature = "database")]
use sqlx::PgPool;

#[cfg(feature = "database")]
use super::execution_plan::compile;
#[cfg(feature = "database")]
use super::executor::DslExecutor;
use super::executor::ExecutionContext;
#[cfg(feature = "database")]
use super::parser::parse_program;
use crate::templates::{ExpansionContext, TemplateDefinition, TemplateExpander};

/// Error handling mode for batch execution
#[derive(Debug, Clone, Copy, PartialEq, Eq, Default)]
pub enum OnErrorMode {
    /// Continue processing remaining items (default)
    #[default]
    Continue,
    /// Stop processing on first error
    Stop,
    /// Rollback all changes on first error (requires transaction)
    Rollback,
}

impl OnErrorMode {
    /// Parse from string
    pub fn from_str(s: &str) -> Self {
        match s.to_lowercase().as_str() {
            "stop" => OnErrorMode::Stop,
            "rollback" => OnErrorMode::Rollback,
            _ => OnErrorMode::Continue,
        }
    }
}

/// Accumulated results from batch execution
///
/// Collects primary keys and bindings from each successful iteration,
/// enabling post-batch operations like `batch.add-products`.
#[derive(Debug, Clone, Default)]
pub struct BatchResultAccumulator {
    /// Per-iteration bindings: each entry is the symbol table from that iteration
    pub iteration_bindings: Vec<HashMap<String, Uuid>>,

    /// Per-iteration symbol types
    pub iteration_types: Vec<HashMap<String, String>>,

    /// Flattened primary entity IDs (e.g., all @cbu values)
    pub primary_entity_ids: Vec<Uuid>,

    /// Primary entity type from template (e.g., "cbu", "kyc_case")
    pub primary_entity_type: String,

    /// Primary entity binding name (e.g., "cbu", "case")
    pub primary_binding_name: String,

    /// Number of successful iterations
    pub success_count: usize,

    /// Number of failed iterations
    pub failure_count: usize,

    /// Errors by iteration index: (index, error_message)
    pub errors: HashMap<usize, String>,

    /// Skipped iterations (if any limit was applied)
    pub skipped_count: usize,
}

impl BatchResultAccumulator {
    /// Create a new accumulator for a template
    pub fn new(template: &TemplateDefinition) -> Self {
        // Extract primary entity info from template
        let (entity_type, binding_name) = template
            .primary_entity
            .as_ref()
            .map(|p| {
                let entity_type = match p.entity_type {
                    crate::templates::PrimaryEntityType::Cbu => "cbu".to_string(),
                    crate::templates::PrimaryEntityType::KycCase => "kyc_case".to_string(),
                    crate::templates::PrimaryEntityType::OnboardingRequest => {
                        "onboarding_request".to_string()
                    }
                };
                (entity_type, p.param.clone())
            })
            .unwrap_or_else(|| ("unknown".to_string(), "result".to_string()));

        Self {
            primary_entity_type: entity_type,
            primary_binding_name: binding_name,
            ..Default::default()
        }
    }

    /// Create an empty accumulator with specified primary entity info
    pub fn with_primary(entity_type: impl Into<String>, binding_name: impl Into<String>) -> Self {
        Self {
            primary_entity_type: entity_type.into(),
            primary_binding_name: binding_name.into(),
            ..Default::default()
        }
    }

    /// Get all primary entity IDs (alias for common use case)
    pub fn cbu_ids(&self) -> &[Uuid] {
        &self.primary_entity_ids
    }

    /// Record a successful iteration
    pub fn record_success(
        &mut self,
        index: usize,
        symbols: HashMap<String, Uuid>,
        symbol_types: HashMap<String, String>,
    ) {
        // Extract primary entity ID
        if let Some(pk) = symbols.get(&self.primary_binding_name) {
            self.primary_entity_ids.push(*pk);
        }

        self.iteration_bindings.push(symbols);
        self.iteration_types.push(symbol_types);
        self.success_count += 1;

        tracing::debug!(
            index,
            success_count = self.success_count,
            "Batch iteration succeeded"
        );
    }

    /// Record a failed iteration
    pub fn record_failure(&mut self, index: usize, error: String) {
        self.errors.insert(index, error.clone());
        self.failure_count += 1;

        tracing::warn!(index, %error, "Batch iteration failed");
    }

    /// Check if batch was fully successful
    pub fn is_complete_success(&self) -> bool {
        self.failure_count == 0 && self.skipped_count == 0
    }

    /// Get total items processed (success + failure)
    pub fn total_processed(&self) -> usize {
        self.success_count + self.failure_count
    }

    /// Get binding from a specific iteration
    pub fn get_iteration_binding(&self, index: usize, name: &str) -> Option<Uuid> {
        self.iteration_bindings
            .get(index)
            .and_then(|bindings| bindings.get(name).copied())
    }
}

/// Result of batch execution
#[derive(Debug)]
pub struct BatchExecutionResult {
    /// Accumulated results from all iterations
    pub accumulator: BatchResultAccumulator,

    /// Total items that were to be processed
    pub total_items: usize,

    /// Whether batch was aborted early
    pub aborted: bool,

    /// Index at which batch was aborted (if aborted)
    pub aborted_at: Option<usize>,
}

impl BatchExecutionResult {
    /// Create a successful batch result
    pub fn success(accumulator: BatchResultAccumulator, total_items: usize) -> Self {
        Self {
            accumulator,
            total_items,
            aborted: false,
            aborted_at: None,
        }
    }

    /// Create an aborted batch result
    pub fn aborted(
        accumulator: BatchResultAccumulator,
        total_items: usize,
        at_index: usize,
    ) -> Self {
        Self {
            accumulator,
            total_items,
            aborted: true,
            aborted_at: Some(at_index),
        }
    }
}

/// Orchestrates batch template execution
///
/// For each item in the source set:
/// 1. Creates child ExecutionContext with parent bindings
/// 2. Expands template with item-specific params
/// 3. Parses expanded DSL
/// 4. Compiles to execution plan (DAG)
/// 5. Executes plan respecting topo order
/// 6. Propagates bindings to BatchResultAccumulator
#[cfg(feature = "database")]
pub struct BatchExecutor {
    pool: PgPool,
    template: TemplateDefinition,
    shared_params: HashMap<String, String>,
    parent_context: ExecutionContext,
}

#[cfg(feature = "database")]
impl BatchExecutor {
    /// Create a new batch executor
    pub fn new(
        pool: PgPool,
        template: TemplateDefinition,
        shared_params: HashMap<String, String>,
        parent_context: ExecutionContext,
    ) -> Self {
        Self {
            pool,
            template,
            shared_params,
            parent_context,
        }
    }

    /// Execute batch with full pipeline per iteration
    ///
    /// # Arguments
    /// * `items` - List of (entity_id, name) tuples to iterate over
    /// * `bind_param` - Template param name to bind each item's ID to
    /// * `on_error` - Error handling mode
    /// * `limit` - Optional limit on items to process
    pub async fn execute_batch(
        &self,
        items: Vec<(Uuid, String)>,
        bind_param: &str,
        on_error: OnErrorMode,
        limit: Option<usize>,
    ) -> Result<BatchExecutionResult> {
        let mut accumulator = BatchResultAccumulator::new(&self.template);
        let total_items = items.len();

        // Apply limit if specified
        let items_to_process: Vec<_> = match limit {
            Some(n) => {
                accumulator.skipped_count = items.len().saturating_sub(n);
                items.into_iter().take(n).collect()
            }
            None => items,
        };

        tracing::info!(
            template_id = %self.template.template,
            total_items,
            limit = ?limit,
            on_error = ?on_error,
            "Starting batch execution"
        );

        // Optional transaction for rollback mode
        let mut tx = if on_error == OnErrorMode::Rollback {
            Some(self.pool.begin().await?)
        } else {
            None
        };

        for (index, (entity_id, name)) in items_to_process.iter().enumerate() {
            let result = self
                .execute_iteration(index, *entity_id, name, bind_param)
                .await;

            match result {
                Ok((symbols, symbol_types)) => {
                    accumulator.record_success(index, symbols, symbol_types);
                }
                Err(e) => {
                    let error_msg = e.to_string();
                    accumulator.record_failure(index, error_msg.clone());

                    match on_error {
                        OnErrorMode::Stop => {
                            tracing::warn!(index, "Batch stopped due to error");
                            return Ok(BatchExecutionResult::aborted(
                                accumulator,
                                total_items,
                                index,
                            ));
                        }
                        OnErrorMode::Rollback => {
                            tracing::warn!(index, "Batch rolling back due to error");
                            if let Some(tx) = tx.take() {
                                tx.rollback().await?;
                            }
                            return Ok(BatchExecutionResult::aborted(
                                accumulator,
                                total_items,
                                index,
                            ));
                        }
                        OnErrorMode::Continue => {
                            // Continue to next iteration
                        }
                    }
                }
            }
        }

        // Commit transaction if rollback mode succeeded
        if let Some(tx) = tx {
            tx.commit().await?;
            tracing::debug!("Batch transaction committed");
        }

        tracing::info!(
            success_count = accumulator.success_count,
            failure_count = accumulator.failure_count,
            "Batch execution completed"
        );

        Ok(BatchExecutionResult::success(accumulator, total_items))
    }

    /// Execute a single iteration of the batch
    async fn execute_iteration(
        &self,
        index: usize,
        entity_id: Uuid,
        name: &str,
        bind_param: &str,
    ) -> Result<(HashMap<String, Uuid>, HashMap<String, String>)> {
        tracing::debug!(index, %entity_id, %name, bind_param, "Executing batch iteration");

        // 1. Build iteration-specific params
        let mut params = self.shared_params.clone();
        params.insert(bind_param.to_string(), entity_id.to_string());
        params.insert(format!("{}.name", bind_param), name.to_string());

        // 2. Build expansion context from parent
        let exp_ctx = ExpansionContext {
            current_cbu: self.parent_context.resolve("cbu"),
            current_case: self.parent_context.resolve("case"),
            bindings: self.parent_context.all_bindings_as_strings(),
            binding_types: self.parent_context.effective_symbol_types(),
        };

        // 3. EXPAND template
        let expansion = TemplateExpander::expand(&self.template, &params, &exp_ctx);

        if !expansion.missing_params.is_empty() {
            let missing: Vec<_> = expansion
                .missing_params
                .iter()
                .map(|p| p.name.as_str())
                .collect();
            return Err(anyhow!("Missing required params: {}", missing.join(", ")));
        }

        tracing::trace!(dsl = %expansion.dsl, "Template expanded");

        // 4. PARSE expanded DSL
        let ast = parse_program(&expansion.dsl).map_err(|e| anyhow!("Parse error: {}", e))?;

        // 5. COMPILE to execution plan (DAG)
        let plan = compile(&ast).map_err(|e| anyhow!("Compile error: {:?}", e))?;

        // 6. Create CHILD context with parent bindings
        let mut child_ctx = self.parent_context.child_for_iteration(index);

        // 7. EXECUTE plan
        let executor = DslExecutor::new(self.pool.clone());
        executor.execute_plan(&plan, &mut child_ctx).await?;

        // 8. Return iteration bindings
        Ok((child_ctx.symbols, child_ctx.symbol_types))
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_on_error_mode_from_str() {
        assert_eq!(OnErrorMode::from_str("continue"), OnErrorMode::Continue);
        assert_eq!(OnErrorMode::from_str("stop"), OnErrorMode::Stop);
        assert_eq!(OnErrorMode::from_str("rollback"), OnErrorMode::Rollback);
        assert_eq!(OnErrorMode::from_str("unknown"), OnErrorMode::Continue);
    }

    #[test]
    fn test_batch_result_accumulator_record_success() {
        let mut acc = BatchResultAccumulator::with_primary("cbu", "cbu");

        let mut symbols = HashMap::new();
        symbols.insert("cbu".to_string(), Uuid::new_v4());
        symbols.insert("case".to_string(), Uuid::new_v4());

        acc.record_success(0, symbols.clone(), HashMap::new());

        assert_eq!(acc.success_count, 1);
        assert_eq!(acc.failure_count, 0);
        assert_eq!(acc.primary_entity_ids.len(), 1);
        assert!(acc.is_complete_success());
    }

    #[test]
    fn test_batch_result_accumulator_record_failure() {
        let mut acc = BatchResultAccumulator::with_primary("cbu", "cbu");

        acc.record_failure(0, "Test error".to_string());

        assert_eq!(acc.success_count, 0);
        assert_eq!(acc.failure_count, 1);
        assert!(!acc.is_complete_success());
        assert!(acc.errors.contains_key(&0));
    }

    #[test]
    fn test_batch_result_accumulator_mixed() {
        let mut acc = BatchResultAccumulator::with_primary("cbu", "cbu");

        let mut symbols = HashMap::new();
        let cbu_id = Uuid::new_v4();
        symbols.insert("cbu".to_string(), cbu_id);

        acc.record_success(0, symbols, HashMap::new());
        acc.record_failure(1, "Error on item 1".to_string());
        acc.record_success(2, HashMap::new(), HashMap::new());

        assert_eq!(acc.success_count, 2);
        assert_eq!(acc.failure_count, 1);
        assert_eq!(acc.total_processed(), 3);
        assert!(!acc.is_complete_success());
        assert_eq!(acc.primary_entity_ids.len(), 1);
        assert_eq!(acc.primary_entity_ids[0], cbu_id);
    }

    #[test]
    fn test_batch_execution_result_success() {
        let acc = BatchResultAccumulator::with_primary("cbu", "cbu");
        let result = BatchExecutionResult::success(acc, 10);

        assert!(!result.aborted);
        assert!(result.aborted_at.is_none());
        assert_eq!(result.total_items, 10);
    }

    #[test]
    fn test_batch_execution_result_aborted() {
        let acc = BatchResultAccumulator::with_primary("cbu", "cbu");
        let result = BatchExecutionResult::aborted(acc, 10, 5);

        assert!(result.aborted);
        assert_eq!(result.aborted_at, Some(5));
        assert_eq!(result.total_items, 10);
    }
}
