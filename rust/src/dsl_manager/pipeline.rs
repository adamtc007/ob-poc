//! DSL Manager Pipeline Module
//!
//! This module provides pipeline orchestration for DSL processing stages,
//! including parse, normalize, validate, compile, and execute phases.

use super::{compiler::Compiler, DslManagerError, DslManagerResult};
use crate::parser_ast::{Form, Program};
use async_trait::async_trait;
use serde::{Deserialize, Serialize};
use std::collections::HashMap;
use std::time::Instant;
use uuid::Uuid;

/// DSL processing pipeline
pub struct DslPipeline {
    /// Pipeline stages in order
    stages: Vec<Box<dyn DslPipelineStage>>,
    /// Pipeline configuration
    config: PipelineConfig,
    /// Stage metrics
    metrics: HashMap<String, StageMetrics>,
}

/// Pipeline configuration
#[derive(Debug, Clone)]
pub(crate) struct PipelineConfig {
    /// Enable parallel stage execution where possible
    pub enable_parallel_execution: bool,
    /// Maximum pipeline execution time (ms)
    pub max_execution_time_ms: u64,
    /// Enable detailed stage metrics
    pub enable_detailed_metrics: bool,
    /// Stop on first error
    pub fail_fast: bool,
    /// Enable stage caching
    pub enable_stage_caching: bool,
}

impl Default for PipelineConfig {
    fn default() -> Self {
        Self {
            enable_parallel_execution: true,
            max_execution_time_ms: 30000,
            enable_detailed_metrics: true,
            fail_fast: true,
            enable_stage_caching: false,
        }
    }
}

/// Pipeline stage trait
#[async_trait]
pub trait DslPipelineStage: Send + Sync {
    /// Stage name
    fn stage_name(&self) -> &str;

    /// Stage description
    fn stage_description(&self) -> &str;

    /// Execute this stage
    async fn execute(
        &self,
        input: PipelineStageInput,
        context: &PipelineContext,
    ) -> DslManagerResult<PipelineStageOutput>;

    /// Validate stage can process input
    async fn can_process(&self, input: &PipelineStageInput) -> bool;

    /// Get stage dependencies (stages that must run before this one)
    fn dependencies(&self) -> Vec<String>;

    /// Whether this stage can run in parallel with others
    fn supports_parallel_execution(&self) -> bool {
        false
    }
}

/// Input to a pipeline stage
#[derive(Debug, Clone)]
pub(crate) struct PipelineStageInput {
    /// DSL text (for parse stage)
    pub dsl_text: Option<String>,
    /// Parsed program (for post-parse stages)
    pub program: Option<Program>,
    /// Validation results from previous stages
    pub validation_results: HashMap<String, serde_json::Value>,
    /// Compilation results
    pub compilation_results: Option<super::compiler::CompilationResult>,
    /// Stage-specific parameters
    pub parameters: HashMap<String, serde_json::Value>,
    /// Request metadata
    pub metadata: HashMap<String, String>,
}

/// Output from a pipeline stage
#[derive(Debug, Clone)]
pub(crate) struct PipelineStageOutput {
    /// Stage execution success
    pub success: bool,
    /// Updated DSL text (if stage modifies it)
    pub dsl_text: Option<String>,
    /// Updated program (if stage modifies it)
    pub program: Option<Program>,
    /// Stage-specific results
    pub results: HashMap<String, serde_json::Value>,
    /// Validation results
    pub validation_results: HashMap<String, serde_json::Value>,
    /// Compilation results
    pub compilation_results: Option<super::compiler::CompilationResult>,
    /// Execution time in milliseconds
    pub execution_time_ms: u64,
    /// Any errors from this stage
    pub errors: Vec<String>,
    /// Warning messages
    pub warnings: Vec<String>,
    /// Stage metadata
    pub metadata: HashMap<String, String>,
}

/// Pipeline execution context
#[derive(Debug, Clone)]
pub(crate) struct PipelineContext {
    /// Request ID for tracking
    pub request_id: String,
    /// User ID
    pub user_id: String,
    /// Session context
    pub session_id: String,
    /// Pipeline execution mode
    pub execution_mode: String,
    /// Context parameters
    pub parameters: HashMap<String, String>,
    /// Audit metadata
    pub audit_metadata: HashMap<String, String>,
}

/// Pipeline execution result
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct PipelineResult {
    /// Overall pipeline success
    pub success: bool,
    /// Final DSL text
    pub final_dsl_text: Option<String>,
    /// Final parsed program
    pub final_program: Option<serde_json::Value>, // Serialized for transport
    /// Results from all stages
    pub stage_results: HashMap<String, serde_json::Value>,
    /// Total execution time
    pub total_execution_time_ms: u64,
    /// Stage execution times
    pub stage_execution_times: HashMap<String, u64>,
    /// All errors collected
    pub errors: Vec<String>,
    /// All warnings collected
    pub warnings: Vec<String>,
    /// Pipeline metadata
    pub metadata: HashMap<String, String>,
}

/// Stage execution metrics
#[derive(Debug, Default, Clone)]
pub(crate) struct StageMetrics {
    pub executions: u64,
    pub successes: u64,
    pub failures: u64,
    pub total_time_ms: u64,
    pub average_time_ms: f64,
    pub min_time_ms: u64,
    pub max_time_ms: u64,
}

impl DslPipeline {
    /// Create a new pipeline
    pub fn new() -> Self {
        Self {
            stages: Vec::new(),
            config: PipelineConfig::default(),
            metrics: HashMap::new(),
        }
    }

    /// Create with configuration
    pub(crate) fn with_config(config: PipelineConfig) -> Self {
        Self {
            stages: Vec::new(),
            config,
            metrics: HashMap::new(),
        }
    }

    /// Add a stage to the pipeline
    pub(crate) fn add_stage(&mut self, stage: Box<dyn DslPipelineStage>) {
        self.stages.push(stage);
    }

    /// Execute the complete pipeline
    pub async fn execute(
        &mut self,
        input: PipelineStageInput,
        context: PipelineContext,
    ) -> DslManagerResult<PipelineResult> {
        let start_time = Instant::now();
        let mut current_input = input;
        let mut stage_results = HashMap::new();
        let mut stage_execution_times = HashMap::new();
        let mut all_errors = Vec::new();
        let mut all_warnings = Vec::new();
        let mut final_dsl_text = None;
        let mut final_program = None;

        // Build dependency graph
        let execution_order = self.build_execution_order()?;

        for stage_name in execution_order {
            let stage_index = self
                .stages
                .iter()
                .position(|s| s.stage_name() == stage_name)
                .ok_or_else(|| DslManagerError::PipelineError {
                    stage: stage_name.clone(),
                    reason: "Stage not found in pipeline".to_string(),
                })?;

            let stage = &self.stages[stage_index];
            let stage_start = Instant::now();

            // Check if stage can process current input
            if !stage.can_process(&current_input).await {
                if self.config.fail_fast {
                    return Err(DslManagerError::PipelineError {
                        stage: stage_name.clone(),
                        reason: "Stage cannot process current input".to_string(),
                    });
                }
                continue;
            }

            // Execute stage
            match stage.execute(current_input.clone(), &context).await {
                Ok(output) => {
                    let execution_time = stage_start.elapsed().as_millis() as u64;

                    // Update metrics
                    self.update_stage_metrics(&stage_name, execution_time, true);

                    stage_execution_times.insert(stage_name.clone(), execution_time);

                    // Collect results
                    if output.success {
                        stage_results.insert(stage_name.clone(), serde_json::json!(output.results));

                        // Update current input for next stage
                        current_input.dsl_text = output.dsl_text.or(current_input.dsl_text);
                        current_input.program = output.program.or(current_input.program);
                        current_input
                            .validation_results
                            .extend(output.validation_results);
                        current_input.compilation_results = output
                            .compilation_results
                            .or(current_input.compilation_results);

                        // Update final outputs
                        final_dsl_text = current_input.dsl_text.clone();
                        final_program =
                            current_input.program.as_ref().map(|p| serde_json::json!(p));

                        // Collect warnings
                        all_warnings.extend(output.warnings);
                    } else {
                        // Stage failed
                        all_errors.extend(output.errors);
                        self.update_stage_metrics(&stage_name, execution_time, false);

                        if self.config.fail_fast {
                            return Ok(PipelineResult {
                                success: false,
                                final_dsl_text,
                                final_program,
                                stage_results,
                                total_execution_time_ms: start_time.elapsed().as_millis() as u64,
                                stage_execution_times,
                                errors: all_errors,
                                warnings: all_warnings,
                                metadata: [("failed_stage".to_string(), stage_name)]
                                    .iter()
                                    .cloned()
                                    .collect(),
                            });
                        }
                    }
                }
                Err(e) => {
                    let execution_time = stage_start.elapsed().as_millis() as u64;
                    self.update_stage_metrics(&stage_name, execution_time, false);

                    all_errors.push(format!("Stage '{}' failed: {}", stage_name, e));

                    if self.config.fail_fast {
                        return Err(e);
                    }
                }
            }

            // Check timeout
            let total_time = start_time.elapsed().as_millis() as u64;
            if total_time > self.config.max_execution_time_ms {
                return Ok(PipelineResult {
                    success: false,
                    final_dsl_text,
                    final_program,
                    stage_results,
                    total_execution_time_ms: total_time,
                    stage_execution_times,
                    errors: vec![format!(
                        "Pipeline timeout after {}ms",
                        self.config.max_execution_time_ms
                    )],
                    warnings: all_warnings,
                    metadata: [("timeout".to_string(), "true".to_string())]
                        .iter()
                        .cloned()
                        .collect(),
                });
            }
        }

        Ok(PipelineResult {
            success: all_errors.is_empty(),
            final_dsl_text,
            final_program,
            stage_results,
            total_execution_time_ms: start_time.elapsed().as_millis() as u64,
            stage_execution_times,
            errors: all_errors,
            warnings: all_warnings,
            metadata: HashMap::new(),
        })
    }

    /// Build execution order based on dependencies
    fn build_execution_order(&self) -> DslManagerResult<Vec<String>> {
        let mut order = Vec::new();
        let mut visited = std::collections::HashSet::new();
        let mut visiting = std::collections::HashSet::new();

        for stage in &self.stages {
            if !visited.contains(stage.stage_name()) {
                self.topological_sort(stage.stage_name(), &mut order, &mut visited, &mut visiting)?;
            }
        }

        Ok(order)
    }

    /// Topological sort for dependency resolution
    fn topological_sort(
        &self,
        stage_name: &str,
        order: &mut Vec<String>,
        visited: &mut std::collections::HashSet<String>,
        visiting: &mut std::collections::HashSet<String>,
    ) -> DslManagerResult<()> {
        if visiting.contains(stage_name) {
            return Err(DslManagerError::PipelineError {
                stage: stage_name.to_string(),
                reason: "Circular dependency detected".to_string(),
            });
        }

        if visited.contains(stage_name) {
            return Ok(());
        }

        visiting.insert(stage_name.to_string());

        // Find stage and process dependencies
        if let Some(stage) = self.stages.iter().find(|s| s.stage_name() == stage_name) {
            for dep in stage.dependencies() {
                self.topological_sort(&dep, order, visited, visiting)?;
            }
        }

        visiting.remove(stage_name);
        visited.insert(stage_name.to_string());
        order.push(stage_name.to_string());

        Ok(())
    }

    /// Update stage execution metrics
    fn update_stage_metrics(&mut self, stage_name: &str, execution_time_ms: u64, success: bool) {
        let metrics = self
            .metrics
            .entry(stage_name.to_string())
            .or_insert_with(StageMetrics::default);

        metrics.executions += 1;
        if success {
            metrics.successes += 1;
        } else {
            metrics.failures += 1;
        }

        metrics.total_time_ms += execution_time_ms;
        metrics.average_time_ms = metrics.total_time_ms as f64 / metrics.executions as f64;

        if metrics.executions == 1 || execution_time_ms < metrics.min_time_ms {
            metrics.min_time_ms = execution_time_ms;
        }

        if execution_time_ms > metrics.max_time_ms {
            metrics.max_time_ms = execution_time_ms;
        }
    }
}

/// Standard DSL pipeline stages
///
/// Parse stage - converts DSL text to AST
pub(crate) struct ParseStage {
    apply_normalization: bool,
}

impl ParseStage {
    pub fn new(apply_normalization: bool) -> Self {
        Self {
            apply_normalization,
        }
    }
}

#[async_trait]
impl DslPipelineStage for ParseStage {
    fn stage_name(&self) -> &str {
        "parse"
    }

    fn stage_description(&self) -> &str {
        "Parse DSL text into AST with optional v3.3 normalization"
    }

    async fn execute(
        &self,
        input: PipelineStageInput,
        _context: &PipelineContext,
    ) -> DslManagerResult<PipelineStageOutput> {
        let start_time = Instant::now();

        let dsl_text = input
            .dsl_text
            .ok_or_else(|| DslManagerError::PipelineError {
                stage: self.stage_name().to_string(),
                reason: "No DSL text provided for parsing".to_string(),
            })?;

        let parse_result = if self.apply_normalization {
            crate::parser::parse_normalize_and_validate(&dsl_text)
        } else {
            match crate::parser::idiomatic_parser::parse_program(&dsl_text) {
                Ok(program) => {
                    let mut validator = crate::parser::validators::DslValidator::new();
                    let validation_result = validator.validate_program(&program);
                    Ok((program, validation_result))
                }
                Err(e) => Err(Box::new(DslManagerError::ParsingError {
                    message: format!("{:?}", e),
                }) as Box<dyn std::error::Error>),
            }
        };

        match parse_result {
            Ok((program, validation_result)) => {
                let mut validation_results = HashMap::new();
                validation_results.insert(
                    "parse_validation".to_string(),
                    serde_json::json!(validation_result),
                );

                Ok(PipelineStageOutput {
                    success: true,
                    dsl_text: Some(dsl_text),
                    program: Some(program),
                    results: [("parsed".to_string(), serde_json::json!(true))]
                        .iter()
                        .cloned()
                        .collect(),
                    validation_results,
                    compilation_results: None,
                    execution_time_ms: start_time.elapsed().as_millis() as u64,
                    errors: vec![],
                    warnings: if self.apply_normalization {
                        vec!["Applied v3.3 normalization".to_string()]
                    } else {
                        vec![]
                    },
                    metadata: [(
                        "normalization_applied".to_string(),
                        self.apply_normalization.to_string(),
                    )]
                    .iter()
                    .cloned()
                    .collect(),
                })
            }
            Err(e) => Ok(PipelineStageOutput {
                success: false,
                dsl_text: Some(dsl_text),
                program: None,
                results: HashMap::new(),
                validation_results: HashMap::new(),
                compilation_results: None,
                execution_time_ms: start_time.elapsed().as_millis() as u64,
                errors: vec![format!("Parse failed: {}", e)],
                warnings: vec![],
                metadata: HashMap::new(),
            }),
        }
    }

    async fn can_process(&self, input: &PipelineStageInput) -> bool {
        input.dsl_text.is_some()
    }

    fn dependencies(&self) -> Vec<String> {
        vec![]
    }
}

/// Validation stage - comprehensive validation of parsed AST
pub(crate) struct ValidationStage;

#[async_trait]
impl DslPipelineStage for ValidationStage {
    fn stage_name(&self) -> &str {
        "validate"
    }

    fn stage_description(&self) -> &str {
        "Comprehensive validation of parsed DSL AST"
    }

    async fn execute(
        &self,
        input: PipelineStageInput,
        _context: &PipelineContext,
    ) -> DslManagerResult<PipelineStageOutput> {
        let start_time = Instant::now();

        let program = input
            .program
            .ok_or_else(|| DslManagerError::PipelineError {
                stage: self.stage_name().to_string(),
                reason: "No parsed program provided for validation".to_string(),
            })?;

        let validation_engine = super::validation::DslValidationEngine::new();
        let validation_result = validation_engine
            .validate(&program, super::validation::ValidationLevel::Standard)
            .await
            .map_err(|e| DslManagerError::AstValidationError {
                message: format!("Validation failed: {}", e),
            })?;

        let mut validation_results = input.validation_results;
        validation_results.insert(
            "comprehensive_validation".to_string(),
            serde_json::json!(validation_result),
        );

        Ok(PipelineStageOutput {
            success: validation_result.valid,
            dsl_text: input.dsl_text,
            program: Some(program),
            results: [(
                "validation_score".to_string(),
                serde_json::json!(if validation_result.valid { 1.0 } else { 0.0 }),
            )]
            .iter()
            .cloned()
            .collect(),
            validation_results,
            compilation_results: input.compilation_results,
            execution_time_ms: start_time.elapsed().as_millis() as u64,
            errors: if validation_result.valid {
                vec![]
            } else {
                validation_result.get_all_errors()
            },
            warnings: validation_result.get_all_warnings(),
            metadata: HashMap::new(),
        })
    }

    async fn can_process(&self, input: &PipelineStageInput) -> bool {
        input.program.is_some()
    }

    fn dependencies(&self) -> Vec<String> {
        vec!["parse".to_string()]
    }
}

/// Compilation stage - compile AST to executable operations
pub(crate) struct CompilationStage {
    target: String,
}

impl CompilationStage {
    pub fn new(target: String) -> Self {
        Self { target }
    }
}

#[async_trait]
impl DslPipelineStage for CompilationStage {
    fn stage_name(&self) -> &str {
        "compile"
    }

    fn stage_description(&self) -> &str {
        "Compile validated AST to executable operations"
    }

    async fn execute(
        &self,
        input: PipelineStageInput,
        context: &PipelineContext,
    ) -> DslManagerResult<PipelineStageOutput> {
        let start_time = Instant::now();

        let program = input
            .program
            .ok_or_else(|| DslManagerError::PipelineError {
                stage: self.stage_name().to_string(),
                reason: "No parsed program provided for compilation".to_string(),
            })?;

        let compiler = super::compiler::DslCompiler::new();
        let execution_context = super::compiler::ExecutionContext {
            target_environment: self.target.clone(),
            execution_mode: context.execution_mode.clone(),
            user_id: context.user_id.clone(),
            session_id: context.session_id.clone(),
            parameters: context.parameters.clone(),
            timeout_seconds: Some(30),
        };

        let compilation_result = compiler.compile(program.clone(), execution_context).await?;

        Ok(PipelineStageOutput {
            success: compilation_result.success,
            dsl_text: input.dsl_text,
            program: Some(program),
            results: [(
                "operations_count".to_string(),
                serde_json::json!(compilation_result
                    .executable_operations
                    .as_ref()
                    .map(|ops| ops.len())
                    .unwrap_or(0)),
            )]
            .iter()
            .cloned()
            .collect(),
            validation_results: input.validation_results,
            compilation_results: Some(compilation_result.clone()),
            execution_time_ms: start_time.elapsed().as_millis() as u64,
            errors: compilation_result.errors,
            warnings: compilation_result.warnings,
            metadata: compilation_result.metadata,
        })
    }

    async fn can_process(&self, input: &PipelineStageInput) -> bool {
        input.program.is_some()
    }

    fn dependencies(&self) -> Vec<String> {
        vec!["parse".to_string(), "validate".to_string()]
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[tokio::test]
    async fn test_pipeline_creation() {
        let pipeline = DslPipeline::new();
        assert_eq!(pipeline.stages.len(), 0);
    }

    #[tokio::test]
    async fn test_parse_stage() {
        let stage = ParseStage::new(true);
        let input = PipelineStageInput {
            dsl_text: Some("(case.create :case-id \"test\")".to_string()),
            program: None,
            validation_results: HashMap::new(),
            compilation_results: None,
            parameters: HashMap::new(),
            metadata: HashMap::new(),
        };

        let context = PipelineContext {
            request_id: "test".to_string(),
            user_id: "test-user".to_string(),
            session_id: "test-session".to_string(),
            execution_mode: "test".to_string(),
            parameters: HashMap::new(),
            audit_metadata: HashMap::new(),
        };

        let result = stage.execute(input, &context).await.unwrap();
        assert!(result.success);
        assert!(result.program.is_some());
    }

    #[tokio::test]
    async fn test_pipeline_with_stages() {
        let mut pipeline = DslPipeline::new();
        pipeline.add_stage(Box::new(ParseStage::new(true)));
        pipeline.add_stage(Box::new(ValidationStage));

        let input = PipelineStageInput {
            dsl_text: Some("(case.create :case-id \"test\")".to_string()),
            program: None,
            validation_results: HashMap::new(),
            compilation_results: None,
            parameters: HashMap::new(),
            metadata: HashMap::new(),
        };

        let context = PipelineContext {
            request_id: "test".to_string(),
            user_id: "test-user".to_string(),
            session_id: "test-session".to_string(),
            execution_mode: "test".to_string(),
            parameters: HashMap::new(),
            audit_metadata: HashMap::new(),
        };

        let result = pipeline.execute(input, context).await.unwrap();
        assert!(result.success);
        assert!(result.stage_results.contains_key("parse"));
        assert!(result.stage_results.contains_key("validate"));
    }
}
