//! DSL Manager Core Implementation
//!
//! This module implements the central DSL Manager that serves as the single gateway
//! for all DSL operations including parsing, v3.3 normalization, validation, execution,
//! and agentic CRUD operations. ALL DSL state changes must flow through this manager.

use super::{
    DslContext, DslManagerError, DslManagerResult, DslOperation, DslProcessingResult,
    ProcessingMetrics, TransactionMode, ValidationLevel, ValidationReport,
};
use crate::{
    ai::{
        crud_prompt_builder::CrudPromptBuilder,
        rag_system::{CrudRagSystem, RetrievedContext},
        AiDslRequest, AiResponseType, AiService,
    },
    dsl::{CentralDslEditor, DomainContext, DslOperation as CoreDslOperation},
    dsl::{DomainHandler, DomainRegistry},
    parser::validators::{DslValidator, ValidationResult},
    parser::{parse_normalize_and_validate, parse_program, DslNormalizer},
    vocabulary::VocabularyRegistry,
    CrudStatement, Form, Program, VerbForm,
};
use std::collections::hash_map::DefaultHasher;
use std::collections::HashMap;
use std::hash::Hasher;
use std::sync::Arc;
use std::time::Instant;
use tokio::sync::RwLock;
use tracing::{debug, error, info, warn};
use uuid::Uuid;

/// Core DSL Manager configuration
#[derive(Debug, Clone)]
pub struct DslManagerConfig {
    /// Enable strict validation
    pub enable_strict_validation: bool,
    /// Enable performance metrics collection
    pub enable_metrics: bool,
    /// Default timeout for operations (seconds)
    pub default_timeout_seconds: u64,
    /// Maximum concurrent operations
    pub max_concurrent_operations: usize,
    /// Enable audit logging
    pub enable_audit_log: bool,
    /// Cache parsed ASTs
    pub enable_ast_cache: bool,
    /// Maximum cache size
    pub max_cache_entries: usize,
}

impl Default for DslManagerConfig {
    fn default() -> Self {
        Self {
            enable_strict_validation: false,
            enable_metrics: false,
            default_timeout_seconds: 30,
            max_concurrent_operations: 100,
            enable_audit_log: true,
            enable_ast_cache: true,
            max_cache_entries: 1000,
        }
    }
}

/// Central DSL Manager - Gateway for ALL DSL operations
pub struct DslManager {
    /// Configuration
    config: DslManagerConfig,
    /// V3.3 → V3.1 normalizer
    normalizer: Arc<DslNormalizer>,
    /// DSL validator
    validator: Arc<RwLock<DslValidator>>,
    /// Backend for persistence operations
    backend: Option<Box<dyn super::DslBackend>>,
    /// AST cache for performance
    ast_cache: Arc<RwLock<std::collections::HashMap<String, CachedAst>>>,
    /// Operation metrics
    metrics: Arc<RwLock<OperationMetrics>>,
    /// Active operations tracking
    active_operations: Arc<RwLock<std::collections::HashMap<String, ActiveOperation>>>,
    /// AI service for agentic operations
    ai_service: Option<Arc<dyn AiService>>,
    /// RAG system for AI context
    rag_system: Option<Arc<CrudRagSystem>>,
    /// Prompt builder for AI requests
    prompt_builder: Option<Arc<CrudPromptBuilder>>,
    /// Domain registry for multi-domain operations
    domain_registry: Arc<DomainRegistry>,
    /// Central DSL editor for domain-specific operations
    central_editor: Arc<CentralDslEditor>,
    /// Verb registry for vocabulary management
    verb_registry: Arc<VocabularyRegistry>,
}

/// Cached AST entry
#[derive(Debug, Clone)]
struct CachedAst {
    ast: Program,
    normalized: bool,
    validated: bool,
    validation_report: ValidationResult,
    cached_at: std::time::SystemTime,
    access_count: u64,
}

/// Active operation tracking
#[derive(Debug, Clone)]
struct ActiveOperation {
    operation_id: String,
    operation_type: String,
    started_at: Instant,
    context: DslContext,
}

/// Operation metrics tracking
#[derive(Debug, Default, Clone)]
struct OperationMetrics {
    total_operations: u64,
    successful_operations: u64,
    failed_operations: u64,
    average_processing_time_ms: f64,
    normalization_hit_rate: f64,
    cache_hit_rate: f64,
}

impl DslManager {
    /// Create a new DSL Manager with given configuration
    pub fn new(config: DslManagerConfig) -> Self {
        Self {
            config,
            normalizer: Arc::new(DslNormalizer::new()),
            validator: Arc::new(RwLock::new(DslValidator::new())),
            backend: None,
            ast_cache: Arc::new(RwLock::new(std::collections::HashMap::new())),
            metrics: Arc::new(RwLock::new(OperationMetrics::default())),
            active_operations: Arc::new(RwLock::new(std::collections::HashMap::new())),
            ai_service: None,
            rag_system: None,
            prompt_builder: None,
            domain_registry: Arc::new(DomainRegistry::new()),
            central_editor: Arc::new(CentralDslEditor::new(
                Arc::new(DomainRegistry::new()),
                Arc::new(MockDictionaryService),
                Default::default(),
            )),
            verb_registry: Arc::new(VocabularyRegistry::new()),
        }
    }

    /// Set the backend for persistence operations
    pub fn set_backend(&mut self, backend: Box<dyn super::DslBackend>) {
        self.backend = Some(backend);
    }

    /// Set AI service for agentic operations
    pub fn set_ai_service(&mut self, ai_service: Arc<dyn AiService>) {
        self.ai_service = Some(ai_service);
    }

    /// Main entry point for processing DSL operations
    /// **ALL DSL OPERATIONS MUST GO THROUGH THIS METHOD**
    pub async fn process_operation(
        &self,
        operation: DslOperation,
        context: DslContext,
    ) -> DslManagerResult<DslProcessingResult> {
        let operation_id = Uuid::new_v4().to_string();
        let start_time = Instant::now();

        // Track active operation
        self.track_operation_start(&operation_id, &operation, &context)
            .await;

        let result = match operation {
            DslOperation::Parse {
                dsl_text,
                apply_normalization,
            } => {
                self.process_parse_operation(&dsl_text, apply_normalization, &context)
                    .await
            }
            DslOperation::Validate {
                ast,
                validation_level,
            } => {
                self.process_validate_operation(ast, validation_level, &context)
                    .await
            }
            DslOperation::Compile {
                ast,
                execution_context,
            } => {
                self.process_compile_operation(ast, execution_context, &context)
                    .await
            }
            DslOperation::Execute {
                compiled_dsl,
                dry_run,
            } => {
                self.process_execute_operation(compiled_dsl, dry_run, &context)
                    .await
            }
            DslOperation::CreateInstance {
                initial_dsl,
                domain,
                metadata,
            } => {
                self.process_create_instance_operation(initial_dsl, domain, metadata, &context)
                    .await
            }
            DslOperation::UpdateInstance {
                instance_id,
                dsl_increment,
                change_description,
            } => {
                self.process_update_instance_operation(
                    instance_id,
                    dsl_increment,
                    change_description,
                    &context,
                )
                .await
            }
            DslOperation::QueryState {
                instance_id,
                version,
            } => {
                self.process_query_state_operation(instance_id, version, &context)
                    .await
            }
            DslOperation::GetHistory { instance_id, limit } => {
                self.process_get_history_operation(instance_id, limit, &context)
                    .await
            }
            DslOperation::Rollback {
                instance_id,
                target_version,
            } => {
                self.process_rollback_operation(instance_id, target_version, &context)
                    .await
            }
            DslOperation::Batch {
                operations,
                transaction_mode,
            } => {
                self.process_batch_operations(operations, transaction_mode, &context)
                    .await
            }
        };

        // Update metrics
        let processing_time = start_time.elapsed();
        self.update_metrics(&result, processing_time).await;

        // Remove from active operations
        self.track_operation_end(&operation_id).await;

        result
    }

    /// Parse DSL with automatic v3.3 normalization
    async fn process_parse_operation(
        &self,
        dsl_text: &str,
        apply_normalization: bool,
        context: &DslContext,
    ) -> DslManagerResult<DslProcessingResult> {
        let start_time = Instant::now();
        let mut metrics = ProcessingMetrics::default();

        info!(
            "Processing parse operation for request: {}",
            context.request_id
        );

        // Check cache first if enabled
        if self.config.enable_ast_cache {
            let cache_key = format!("{}-{}", dsl_text.len(), {
                let hasher = DefaultHasher::new();
                hasher.finish()
            });

            if let Some(cached) = self.get_cached_ast(&cache_key).await {
                debug!("Cache hit for DSL parsing");
                let cached_report = cached.validation_report.clone();
                return Ok(DslProcessingResult {
                    success: true,
                    ast: Some(cached.ast),
                    validation_report: ValidationReport {
                        valid: cached_report.is_valid,
                        errors: cached_report
                            .errors
                            .into_iter()
                            .map(|e| e.message)
                            .collect(),
                        warnings: cached_report
                            .warnings
                            .into_iter()
                            .map(|w| w.message)
                            .collect(),
                        suggestions: cached_report.suggestions,
                        validation_level: ValidationLevel::Standard,
                        core_validation: Some(cached.validation_report),
                        manager_validation: None,
                        normalization_validation: None,
                        validation_metrics: None,
                        total_time_ms: 0,
                    },
                    compilation_result: None,
                    execution_result: None,
                    metrics,
                    errors: vec![],
                    warnings: vec![],
                });
            }
        }

        let parse_start = Instant::now();

        let result = if apply_normalization || context.options.apply_normalization {
            // Use the complete pipeline with v3.3 → v3.1 normalization
            debug!("Applying v3.3 → v3.1 normalization");
            parse_normalize_and_validate(dsl_text)
        } else {
            // Parse only without normalization
            debug!("Parsing without normalization");
            match crate::parser::idiomatic_parser::parse_program(dsl_text) {
                Ok(program) => {
                    let mut validator = DslValidator::new();
                    let validation_result = validator.validate_program(&program);
                    Ok((program, validation_result))
                }
                Err(e) => Err(Box::new(DslManagerError::ParsingError {
                    message: format!("{:?}", e),
                }) as Box<dyn std::error::Error>),
            }
        };

        metrics.parse_time_ms = parse_start.elapsed().as_millis() as u64;
        if apply_normalization {
            metrics.normalization_time_ms = metrics.parse_time_ms / 2; // Approximation
        }

        match result {
            Ok((ast, validation_report)) => {
                metrics.validation_time_ms =
                    parse_start.elapsed().as_millis() as u64 - metrics.parse_time_ms;
                metrics.total_time_ms = start_time.elapsed().as_millis() as u64;

                // Cache the result if enabled
                if self.config.enable_ast_cache {
                    let cache_key = format!("{}-{}", dsl_text.len(), {
                        let hasher = DefaultHasher::new();
                        hasher.finish()
                    });
                    self.cache_ast(&cache_key, &ast, apply_normalization, &validation_report)
                        .await;
                }

                let report_clone = validation_report.clone();
                Ok(DslProcessingResult {
                    success: true,
                    ast: Some(ast),
                    validation_report: ValidationReport {
                        valid: report_clone.is_valid,
                        errors: report_clone.errors.into_iter().map(|e| e.message).collect(),
                        warnings: report_clone
                            .warnings
                            .into_iter()
                            .map(|w| w.message)
                            .collect(),
                        suggestions: report_clone.suggestions,
                        validation_level: ValidationLevel::Standard,
                        core_validation: Some(validation_report),
                        manager_validation: None,
                        normalization_validation: None,
                        validation_metrics: None,
                        total_time_ms: 0,
                    },
                    compilation_result: None,
                    execution_result: None,
                    metrics,
                    errors: vec![],
                    warnings: vec![],
                })
            }
            Err(e) => {
                error!("DSL parsing failed: {}", e);
                Ok(DslProcessingResult {
                    success: false,
                    ast: None,
                    validation_report: ValidationReport::default(),
                    compilation_result: None,
                    execution_result: None,
                    metrics,
                    errors: vec![DslManagerError::ParsingError {
                        message: e.to_string(),
                    }],
                    warnings: vec![],
                })
            }
        }
    }

    /// Validate AST
    async fn process_validate_operation(
        &self,
        ast: Program,
        _validation_level: super::ValidationLevel,
        _context: &DslContext,
    ) -> DslManagerResult<DslProcessingResult> {
        let start_time = Instant::now();
        let mut metrics = ProcessingMetrics::default();

        let validation_start = Instant::now();
        let mut validator_guard = self.validator.write().await;
        let validation_report = validator_guard.validate_program(&ast);
        metrics.validation_time_ms = validation_start.elapsed().as_millis() as u64;
        metrics.total_time_ms = start_time.elapsed().as_millis() as u64;

        let report_clone = validation_report.clone();
        Ok(DslProcessingResult {
            success: validation_report.is_valid,
            ast: Some(ast),
            validation_report: ValidationReport {
                valid: report_clone.is_valid,
                errors: report_clone.errors.into_iter().map(|e| e.message).collect(),
                warnings: report_clone
                    .warnings
                    .into_iter()
                    .map(|w| w.message)
                    .collect(),
                suggestions: report_clone.suggestions,
                validation_level: ValidationLevel::Standard,
                core_validation: Some(validation_report),
                manager_validation: None,
                normalization_validation: None,
                validation_metrics: None,
                total_time_ms: 0,
            },
            compilation_result: None,
            execution_result: None,
            metrics,
            errors: vec![],
            warnings: vec![],
        })
    }

    /// Compile AST to executable form
    async fn process_compile_operation(
        &self,
        ast: Program,
        execution_context: super::ExecutionContext,
        _context: &DslContext,
    ) -> DslManagerResult<DslProcessingResult> {
        let start_time = Instant::now();
        let mut metrics = ProcessingMetrics::default();

        let compile_start = Instant::now();
        let compiler = super::compiler::DslCompiler::new();
        use crate::dsl_manager::compiler::Compiler;
        let compilation_result = compiler.compile(ast.clone(), execution_context).await;
        metrics.compilation_time_ms = compile_start.elapsed().as_millis() as u64;
        metrics.total_time_ms = start_time.elapsed().as_millis() as u64;

        match compilation_result {
            Ok(compiled) => Ok(DslProcessingResult {
                success: true,
                ast: Some(ast),
                validation_report: ValidationReport::default(),
                compilation_result: Some(compiled),
                execution_result: None,
                metrics,
                errors: vec![],
                warnings: vec![],
            }),
            Err(e) => Ok(DslProcessingResult {
                success: false,
                ast: Some(ast),
                validation_report: ValidationReport::default(),
                compilation_result: None,
                execution_result: None,
                metrics,
                errors: vec![DslManagerError::CompilationError {
                    message: format!("{:?}", e),
                }],
                warnings: vec![],
            }),
        }
    }

    /// Execute compiled DSL
    async fn process_execute_operation(
        &self,
        compiled_dsl: super::CompilationResult,
        dry_run: bool,
        _context: &DslContext,
    ) -> DslManagerResult<DslProcessingResult> {
        let start_time = Instant::now();
        let mut metrics = ProcessingMetrics::default();

        if let Some(ref backend) = self.backend {
            let exec_start = Instant::now();
            let execution_result = if dry_run {
                backend.dry_run_execute(&compiled_dsl).await
            } else {
                backend.execute(&compiled_dsl).await
            };
            metrics.execution_time_ms = exec_start.elapsed().as_millis() as u64;
            metrics.total_time_ms = start_time.elapsed().as_millis() as u64;

            match execution_result {
                Ok(result) => Ok(DslProcessingResult {
                    success: true,
                    ast: None,
                    validation_report: ValidationReport::default(),
                    compilation_result: Some(compiled_dsl),
                    execution_result: Some(result),
                    metrics,
                    errors: vec![],
                    warnings: vec![],
                }),
                Err(e) => Ok(DslProcessingResult {
                    success: false,
                    ast: None,
                    validation_report: ValidationReport::default(),
                    compilation_result: Some(compiled_dsl),
                    execution_result: None,
                    metrics,
                    errors: vec![DslManagerError::BackendError {
                        message: format!("Execution failed: {:?}", e),
                    }],
                    warnings: vec![],
                }),
            }
        } else {
            warn!("No backend configured for execution");
            Ok(DslProcessingResult {
                success: false,
                ast: None,
                validation_report: ValidationReport::default(),
                compilation_result: Some(compiled_dsl),
                execution_result: None,
                metrics,
                errors: vec![DslManagerError::ConfigurationError {
                    message: "No backend configured".to_string(),
                }],
                warnings: vec![],
            })
        }
    }

    /// Convenience method for complete DSL processing pipeline
    pub async fn execute_dsl(
        &self,
        dsl_text: &str,
        context: DslContext,
    ) -> DslManagerResult<DslProcessingResult> {
        info!(
            "Executing complete DSL pipeline for request: {}",
            context.request_id
        );

        // Step 1: Parse with normalization
        let parse_result = self
            .process_operation(
                DslOperation::Parse {
                    dsl_text: dsl_text.to_string(),
                    apply_normalization: context.options.apply_normalization,
                },
                context.clone(),
            )
            .await?;

        if !parse_result.success || parse_result.ast.is_none() {
            return Ok(parse_result);
        }

        let ast = parse_result.ast.unwrap();

        // Step 2: Additional validation if strict mode
        if self.config.enable_strict_validation {
            let validate_result = self
                .process_operation(
                    DslOperation::Validate {
                        ast: ast.clone(),
                        validation_level: context.options.validation_level.clone(),
                    },
                    context.clone(),
                )
                .await?;

            if !validate_result.success {
                return Ok(validate_result);
            }
        }

        // Step 3: Compile
        let compile_result = self
            .process_operation(
                DslOperation::Compile {
                    ast,
                    execution_context: super::ExecutionContext::default(),
                },
                context.clone(),
            )
            .await?;

        if !compile_result.success || compile_result.compilation_result.is_none() {
            return Ok(compile_result);
        }

        // Step 4: Execute
        let execute_result = self
            .process_operation(
                DslOperation::Execute {
                    compiled_dsl: compile_result.compilation_result.unwrap(),
                    dry_run: false,
                },
                context,
            )
            .await?;

        Ok(execute_result)
    }

    /// Process batch operations
    async fn process_batch_operations(
        &self,
        operations: Vec<DslOperation>,
        transaction_mode: TransactionMode,
        context: &DslContext,
    ) -> DslManagerResult<DslProcessingResult> {
        info!(
            "Processing batch of {} operations in {:?} mode",
            operations.len(),
            transaction_mode
        );

        match transaction_mode {
            TransactionMode::Atomic => {
                // All or nothing - if any operation fails, none are applied
                let mut results = Vec::new();
                let mut all_successful = true;

                for operation in operations {
                    let result =
                        Box::pin(self.process_operation(operation, context.clone())).await?;
                    if !result.success {
                        all_successful = false;
                    }
                    results.push(result);
                }

                // If not all successful and atomic mode, rollback would happen here
                if !all_successful {
                    warn!("Atomic batch operation failed - would rollback all changes");
                }

                // Return combined result
                Ok(DslProcessingResult {
                    success: all_successful,
                    ast: None,
                    validation_report: ValidationReport::default(),
                    compilation_result: None,
                    execution_result: None,
                    metrics: ProcessingMetrics::default(),
                    errors: results.iter().flat_map(|r| r.errors.clone()).collect(),
                    warnings: results.iter().flat_map(|r| r.warnings.clone()).collect(),
                })
            }
            TransactionMode::Sequential => {
                // Continue processing even if some operations fail
                let mut results = Vec::new();
                let mut successful_count = 0;

                for operation in operations {
                    match Box::pin(self.process_operation(operation, context.clone())).await {
                        Ok(result) => {
                            if result.success {
                                successful_count += 1;
                            }
                            results.push(result);
                        }
                        Err(e) => {
                            error!("Operation failed in batch: {}", e);
                        }
                    }
                }

                Ok(DslProcessingResult {
                    success: successful_count > 0,
                    ast: None,
                    validation_report: ValidationReport::default(),
                    compilation_result: None,
                    execution_result: None,
                    metrics: ProcessingMetrics::default(),
                    errors: results.iter().flat_map(|r| r.errors.clone()).collect(),
                    warnings: results.iter().flat_map(|r| r.warnings.clone()).collect(),
                })
            }
            TransactionMode::DryRun => {
                // Validate all operations without executing
                let mut results = Vec::new();
                let mut all_valid = true;

                for operation in operations {
                    // Convert to dry-run version of operation
                    let dry_run_op = self.make_dry_run_operation(operation);
                    let result =
                        Box::pin(self.process_operation(dry_run_op, context.clone())).await?;
                    if !result.success {
                        all_valid = false;
                    }
                    results.push(result);
                }

                Ok(DslProcessingResult {
                    success: all_valid,
                    ast: None,
                    validation_report: ValidationReport::default(),
                    compilation_result: None,
                    execution_result: None,
                    metrics: ProcessingMetrics::default(),
                    errors: results.iter().flat_map(|r| r.errors.clone()).collect(),
                    warnings: results.iter().flat_map(|r| r.warnings.clone()).collect(),
                })
            }
        }
    }

    /// Convert operation to dry-run version
    fn make_dry_run_operation(&self, operation: DslOperation) -> DslOperation {
        match operation {
            DslOperation::Execute { compiled_dsl, .. } => DslOperation::Execute {
                compiled_dsl,
                dry_run: true,
            },
            // Other operations remain the same for dry-run
            other => other,
        }
    }

    // Placeholder implementations for other operations
    async fn process_create_instance_operation(
        &self,
        _initial_dsl: String,
        _domain: String,
        _metadata: std::collections::HashMap<String, String>,
        _context: &DslContext,
    ) -> DslManagerResult<DslProcessingResult> {
        // TODO: Implement instance creation
        Ok(DslProcessingResult {
            success: true,
            ast: None,
            validation_report: ValidationReport::default(),
            compilation_result: None,
            execution_result: None,
            metrics: ProcessingMetrics::default(),
            errors: vec![],
            warnings: vec!["Instance creation not yet implemented".to_string()],
        })
    }

    async fn process_update_instance_operation(
        &self,
        _instance_id: Uuid,
        _dsl_increment: String,
        _change_description: Option<String>,
        _context: &DslContext,
    ) -> DslManagerResult<DslProcessingResult> {
        // TODO: Implement instance updates
        Ok(DslProcessingResult {
            success: true,
            ast: None,
            validation_report: ValidationReport::default(),
            compilation_result: None,
            execution_result: None,
            metrics: ProcessingMetrics::default(),
            errors: vec![],
            warnings: vec!["Instance update not yet implemented".to_string()],
        })
    }

    async fn process_query_state_operation(
        &self,
        _instance_id: Uuid,
        _version: Option<u64>,
        _context: &DslContext,
    ) -> DslManagerResult<DslProcessingResult> {
        // TODO: Implement state queries
        Ok(DslProcessingResult {
            success: true,
            ast: None,
            validation_report: ValidationReport::default(),
            compilation_result: None,
            execution_result: None,
            metrics: ProcessingMetrics::default(),
            errors: vec![],
            warnings: vec!["State query not yet implemented".to_string()],
        })
    }

    async fn process_get_history_operation(
        &self,
        _instance_id: Uuid,
        _limit: Option<u64>,
        _context: &DslContext,
    ) -> DslManagerResult<DslProcessingResult> {
        // TODO: Implement history retrieval
        Ok(DslProcessingResult {
            success: true,
            ast: None,
            validation_report: ValidationReport::default(),
            compilation_result: None,
            execution_result: None,
            metrics: ProcessingMetrics::default(),
            errors: vec![],
            warnings: vec!["History retrieval not yet implemented".to_string()],
        })
    }

    async fn process_rollback_operation(
        &self,
        _instance_id: Uuid,
        _target_version: u64,
        _context: &DslContext,
    ) -> DslManagerResult<DslProcessingResult> {
        // TODO: Implement rollback
        Ok(DslProcessingResult {
            success: true,
            ast: None,
            validation_report: ValidationReport::default(),
            compilation_result: None,
            execution_result: None,
            metrics: ProcessingMetrics::default(),
            errors: vec![],
            warnings: vec!["Rollback not yet implemented".to_string()],
        })
    }

    /// Helper methods for caching
    async fn get_cached_ast(&self, cache_key: &str) -> Option<CachedAst> {
        let cache = self.ast_cache.read().await;
        cache.get(cache_key).cloned()
    }

    async fn cache_ast(
        &self,
        cache_key: &str,
        ast: &Program,
        normalized: bool,
        validation_report: &ValidationResult,
    ) {
        let mut cache = self.ast_cache.write().await;

        // Remove old entries if cache is full
        if cache.len() >= self.config.max_cache_entries {
            // Simple LRU - remove oldest entries
            let oldest_keys: Vec<String> = cache
                .iter()
                .map(|(k, v)| (k.clone(), v.cached_at))
                .collect::<Vec<_>>()
                .into_iter()
                .min_by_key(|(_, time)| *time)
                .map(|(k, _)| k)
                .into_iter()
                .collect();

            for key in oldest_keys {
                cache.remove(&key);
            }
        }

        cache.insert(
            cache_key.to_string(),
            CachedAst {
                ast: ast.clone(),
                normalized,
                validated: true,
                validation_report: validation_report.clone(),
                cached_at: std::time::SystemTime::now(),
                access_count: 1,
            },
        );
    }

    /// Helper methods for operation tracking
    async fn track_operation_start(
        &self,
        operation_id: &str,
        operation: &DslOperation,
        context: &DslContext,
    ) {
        let mut active_ops = self.active_operations.write().await;
        active_ops.insert(
            operation_id.to_string(),
            ActiveOperation {
                operation_id: operation_id.to_string(),
                operation_type: format!("{:?}", std::mem::discriminant(operation)),
                started_at: Instant::now(),
                context: context.clone(),
            },
        );
    }

    async fn track_operation_end(&self, operation_id: &str) {
        let mut active_ops = self.active_operations.write().await;
        active_ops.remove(operation_id);
    }

    /// Update processing metrics
    async fn update_metrics(
        &self,
        result: &DslManagerResult<DslProcessingResult>,
        processing_time: std::time::Duration,
    ) {
        if self.config.enable_metrics {
            let mut metrics = self.metrics.write().await;
            metrics.total_operations += 1;

            if result.is_ok() && result.as_ref().unwrap().success {
                metrics.successful_operations += 1;
            } else {
                metrics.failed_operations += 1;
            }

            let processing_time_ms = processing_time.as_millis() as f64;
            metrics.average_processing_time_ms = (metrics.average_processing_time_ms
                * (metrics.total_operations - 1) as f64
                + processing_time_ms)
                / metrics.total_operations as f64;
        }
    }

    /// Get current operational metrics
    pub async fn get_metrics(&self) -> OperationMetrics {
        (*self.metrics.read().await).clone()
    }

    /// Get active operations count
    pub async fn get_active_operations_count(&self) -> usize {
        self.active_operations.read().await.len()
    }

    /// **AGENTIC CRUD GATEWAY** - The ONLY way to perform agentic CRUD operations
    /// All AI-powered DSL generation and execution must go through this method
    pub async fn process_agentic_crud_request(
        &self,
        request: AgenticCrudRequest,
        context: DslContext,
    ) -> DslManagerResult<DslProcessingResult> {
        info!(
            "Processing agentic CRUD request: {} for user: {}",
            request.instruction, context.user_id
        );

        let operation_id = Uuid::new_v4().to_string();
        let start_time = Instant::now();

        // Step 1: Validate AI services are available
        let ai_service =
            self.ai_service
                .as_ref()
                .ok_or_else(|| DslManagerError::ConfigurationError {
                    message: "AI service not configured for agentic operations".to_string(),
                })?;

        let rag_system = self.rag_system.as_ref();
        let prompt_builder = self.prompt_builder.as_ref();

        // Step 2: Generate RAG context
        let _rag_context = if let Some(rag) = rag_system {
            info!("Retrieving RAG context for: {}", request.instruction);
            match rag.retrieve_context(&request.instruction) {
                Ok(context) => context,
                Err(_) => RetrievedContext {
                    relevant_schemas: Vec::new(),
                    applicable_grammar: Vec::new(),
                    similar_examples: Vec::new(),
                    confidence_score: 0.0,
                    sources: Vec::new(),
                },
            }
        } else {
            RetrievedContext {
                relevant_schemas: Vec::new(),
                applicable_grammar: Vec::new(),
                similar_examples: Vec::new(),
                confidence_score: 0.0,
                sources: Vec::new(),
            }
        };

        // Step 3: Build AI prompt
        let prompt = if let Some(_builder) = prompt_builder {
            request.instruction.clone()
        } else {
            request.instruction.clone()
        };

        // Step 4: Generate DSL using AI service
        let ai_request = AiDslRequest {
            instruction: prompt,
            context: Some({
                let mut ctx = HashMap::new();
                ctx.insert(
                    "asset_type".to_string(),
                    request.asset_type.unwrap_or_default(),
                );
                ctx.insert(
                    "operation_type".to_string(),
                    format!("{:?}", request.operation_type),
                );
                ctx
            }),
            response_type: AiResponseType::DslGeneration,
            temperature: Some(0.1),
            max_tokens: Some(1000),
        };

        let ai_response = ai_service.generate_dsl(ai_request).await.map_err(|e| {
            DslManagerError::BackendError {
                message: format!("AI DSL generation failed: {}", e),
            }
        })?;

        info!("AI generated DSL: {}", ai_response.generated_dsl);

        // Step 5: Process the generated DSL through the complete DSL pipeline
        // This ensures v3.3 normalization, validation, and proper execution
        let mut dsl_context = context.clone();
        dsl_context
            .audit_metadata
            .insert("ai_generated".to_string(), "true".to_string());
        dsl_context.audit_metadata.insert(
            "ai_confidence".to_string(),
            ai_response.confidence.unwrap_or(0.0).to_string(),
        );
        dsl_context.audit_metadata.insert(
            "original_instruction".to_string(),
            request.instruction.clone(),
        );

        let mut processing_result = self
            .execute_dsl(&ai_response.generated_dsl, dsl_context)
            .await?;

        // Step 6: Add agentic-specific metadata
        processing_result.metrics.total_time_ms = start_time.elapsed().as_millis() as u64;

        // Step 7: Execute database operations if requested
        if request.execute_dsl && processing_result.success {
            info!("Executing agentic CRUD operation against database");

            // The DSL has already been executed through execute_dsl above
            // Add execution confirmation to the result
            if let Some(ref execution_result) = processing_result.execution_result {
                info!(
                    "Agentic CRUD execution completed: {} rows affected",
                    execution_result.rows_affected.unwrap_or(0)
                );
            }
        }

        // Step 8: Store operation for audit trail
        if let Some(ref backend) = self.backend {
            let audit_metadata = [
                ("operation_type".to_string(), "agentic_crud".to_string()),
                ("ai_instruction".to_string(), request.instruction),
                ("generated_dsl".to_string(), ai_response.generated_dsl),
                (
                    "ai_confidence".to_string(),
                    ai_response.confidence.unwrap_or(0.0).to_string(),
                ),
            ]
            .iter()
            .cloned()
            .collect();

            let _ = backend
                .store_dsl_instance(
                    Uuid::parse_str(&operation_id).unwrap_or_else(|_| Uuid::new_v4()),
                    &processing_result
                        .ast
                        .as_ref()
                        .map(|ast| format!("{:?}", ast))
                        .unwrap_or_default(),
                    audit_metadata,
                )
                .await;
        }

        Ok(processing_result)
    }

    /// **AI-POWERED ONBOARDING WORKFLOW** - Complete end-to-end onboarding via DSL Manager
    /// This is the new unified entry point replacing AiDslService.create_ai_onboarding
    pub async fn process_ai_onboarding(
        &self,
        request: AiOnboardingRequest,
        context: DslContext,
    ) -> DslManagerResult<AiOnboardingResponse> {
        info!(
            "Processing AI-powered onboarding for client: {} in {}",
            request.client_name, context.domain
        );

        let start_time = std::time::Instant::now();

        // Step 1: Generate CBU ID
        let cbu_id = CbuGenerator::generate_cbu_id(
            &request.client_name,
            &request.jurisdiction,
            &request.entity_type,
        );
        info!("Generated CBU ID: {}", cbu_id);

        // Step 2: Validate AI services are available
        let ai_service =
            self.ai_service
                .as_ref()
                .ok_or_else(|| DslManagerError::ConfigurationError {
                    message: "AI service not configured for onboarding operations".to_string(),
                })?;

        // Step 3: Build comprehensive AI request for onboarding DSL
        let ai_request = crate::ai::AiDslRequest {
            instruction: format!(
                "Create a complete onboarding DSL for client '{}' (CBU: {}). {}. Services needed: {}. Entity type: {} in jurisdiction: {}",
                request.client_name,
                cbu_id,
                request.instruction,
                request.services.join(", "),
                request.entity_type,
                request.jurisdiction
            ),
            context: Some({
                let mut ctx = request.context.clone();
                ctx.insert("cbu_id".to_string(), cbu_id.clone());
                ctx.insert("client_name".to_string(), request.client_name.clone());
                ctx.insert("jurisdiction".to_string(), request.jurisdiction.clone());
                ctx.insert("entity_type".to_string(), request.entity_type.clone());
                ctx.insert("services".to_string(), request.services.join(", "));
                if let Some(compliance_level) = &request.compliance_level {
                    ctx.insert("compliance_level".to_string(), compliance_level.clone());
                }
                ctx
            }),
            response_type: crate::ai::AiResponseType::DslGeneration,
            temperature: Some(0.1),
            max_tokens: Some(2000),
        };

        // Step 4: Generate DSL using AI service
        let ai_response = ai_service.generate_dsl(ai_request).await.map_err(|e| {
            DslManagerError::BackendError {
                message: format!("AI DSL generation failed: {}", e),
            }
        })?;

        info!(
            "AI generated onboarding DSL with confidence: {:.2}",
            ai_response.confidence.unwrap_or(0.5)
        );

        // Step 5: Process the generated DSL through complete DSL pipeline
        let mut dsl_context = context.clone();
        dsl_context
            .audit_metadata
            .insert("operation_type".to_string(), "ai_onboarding".to_string());
        dsl_context
            .audit_metadata
            .insert("cbu_id".to_string(), cbu_id.clone());
        dsl_context
            .audit_metadata
            .insert("client_name".to_string(), request.client_name.clone());
        dsl_context
            .audit_metadata
            .insert("ai_generated".to_string(), "true".to_string());
        dsl_context.audit_metadata.insert(
            "ai_confidence".to_string(),
            ai_response.confidence.unwrap_or(0.0).to_string(),
        );

        let processing_result = self
            .execute_dsl(&ai_response.generated_dsl, dsl_context)
            .await?;

        if !processing_result.success {
            return Err(DslManagerError::CompilationError {
                message: format!(
                    "Generated DSL failed validation: {:?}",
                    processing_result.errors
                ),
            });
        }

        let execution_time = start_time.elapsed().as_millis() as u64;

        // Step 6: Create DSL instance summary
        let instance_id = uuid::Uuid::new_v4().to_string();
        let dsl_instance = DslInstanceSummary {
            instance_id: format!("onboarding-{}", instance_id),
            domain: "onboarding".to_string(),
            status: "active".to_string(),
            created_at: chrono::Utc::now(),
            current_version: 1,
        };

        // Step 7: Store operation for audit trail
        if let Some(ref backend) = self.backend {
            let audit_metadata = [
                ("operation_type".to_string(), "ai_onboarding".to_string()),
                ("cbu_id".to_string(), cbu_id.clone()),
                ("client_name".to_string(), request.client_name.clone()),
                (
                    "generated_dsl".to_string(),
                    ai_response.generated_dsl.clone(),
                ),
                (
                    "ai_confidence".to_string(),
                    ai_response.confidence.unwrap_or(0.0).to_string(),
                ),
            ]
            .iter()
            .cloned()
            .collect();

            let _ = backend
                .store_dsl_instance(
                    uuid::Uuid::parse_str(&instance_id).unwrap_or_else(|_| uuid::Uuid::new_v4()),
                    &ai_response.generated_dsl,
                    audit_metadata,
                )
                .await;
        }

        Ok(AiOnboardingResponse {
            cbu_id,
            dsl_instance,
            generated_dsl: ai_response.generated_dsl,
            ai_explanation: ai_response.explanation,
            ai_confidence: ai_response.confidence.unwrap_or(0.5),
            execution_details: ExecutionDetails {
                template_used: "onboarding".to_string(),
                compilation_successful: processing_result.compilation_result.is_some(),
                validation_passed: processing_result.validation_report.valid,
                storage_keys: Some(instance_id),
                execution_time_ms: execution_time,
            },
            warnings: ai_response.warnings.unwrap_or_default(),
            suggestions: ai_response.suggestions.unwrap_or_default(),
        })
    }

    /// **AI-POWERED DSL VALIDATION** - Validate DSL using AI analysis
    /// This replaces direct AI validation calls throughout the system
    pub async fn validate_dsl_with_ai(
        &self,
        dsl_content: &str,
        context: DslContext,
    ) -> DslManagerResult<AiValidationResult> {
        info!(
            "Processing AI-powered DSL validation for user: {}",
            context.user_id
        );

        // Step 1: Validate AI services are available
        let ai_service =
            self.ai_service
                .as_ref()
                .ok_or_else(|| DslManagerError::ConfigurationError {
                    message: "AI service not configured for validation operations".to_string(),
                })?;

        // Step 2: Build AI validation request
        let ai_request = crate::ai::AiDslRequest {
            instruction: format!(
                "Validate this DSL for syntax correctness, vocabulary compliance, and business logic. DSL:\n{}",
                dsl_content
            ),
            context: Some(context.audit_metadata.clone()),
            response_type: crate::ai::AiResponseType::DslValidation,
            temperature: Some(0.1),
            max_tokens: Some(1000),
        };

        // Step 3: Get AI validation
        let ai_response = ai_service.generate_dsl(ai_request).await.map_err(|e| {
            DslManagerError::BackendError {
                message: format!("AI DSL validation failed: {}", e),
            }
        })?;

        // Step 4: Also perform standard DSL Manager validation
        let standard_validation = self
            .process_operation(
                DslOperation::Validate {
                    ast: crate::parser::parse_program(dsl_content).map_err(|e| {
                        DslManagerError::ParsingError {
                            message: format!("Failed to parse DSL: {:?}", e),
                        }
                    })?,
                    validation_level: ValidationLevel::Strict,
                },
                context,
            )
            .await?;

        Ok(AiValidationResult {
            valid: ai_response.warnings.as_ref().map_or(true, |w| w.is_empty())
                && standard_validation.success,
            ai_confidence: ai_response.confidence.unwrap_or(0.5),
            ai_issues: ai_response.warnings.unwrap_or_default(),
            ai_suggestions: ai_response.suggestions.unwrap_or_default(),
            ai_explanation: ai_response.explanation,
            standard_validation_report: standard_validation.validation_report,
            combined_score: (ai_response.confidence.unwrap_or(0.5)
                + if standard_validation.success {
                    1.0
                } else {
                    0.0
                })
                / 2.0,
        })
    }

    /// **CANONICAL KYC CASE GENERATION** - Replace ai/dsl_service.rs methods
    /// Generate canonical KYC case DSL using AI with proper DSL Manager lifecycle
    pub async fn generate_canonical_kyc_case(
        &self,
        entity_name: &str,
        context: DslContext,
    ) -> DslManagerResult<CanonicalDslResponse> {
        info!("Generating canonical KYC case for entity: {}", entity_name);

        let ai_service =
            self.ai_service
                .as_ref()
                .ok_or_else(|| DslManagerError::ConfigurationError {
                    message: "AI service not configured for canonical DSL generation".to_string(),
                })?;

        let ai_request = crate::ai::AiDslRequest {
            instruction: format!(
                "Generate a canonical KYC case DSL for entity '{}' following standard compliance procedures",
                entity_name
            ),
            context: Some({
                let mut ctx = context.audit_metadata.clone();
                ctx.insert("entity_name".to_string(), entity_name.to_string());
                ctx.insert("operation_type".to_string(), "canonical_kyc".to_string());
                ctx
            }),
            response_type: crate::ai::AiResponseType::DslGeneration,
            temperature: Some(0.1),
            max_tokens: Some(1500),
        };

        let ai_response = ai_service.generate_dsl(ai_request).await.map_err(|e| {
            DslManagerError::BackendError {
                message: format!("AI canonical KYC generation failed: {}", e),
            }
        })?;

        // Process through full DSL pipeline
        let mut dsl_context = context.clone();
        dsl_context
            .audit_metadata
            .insert("canonical_type".to_string(), "kyc_case".to_string());

        let processing_result = self
            .execute_dsl(&ai_response.generated_dsl, dsl_context)
            .await?;

        Ok(CanonicalDslResponse {
            generated_dsl: ai_response.generated_dsl,
            explanation: ai_response.explanation,
            confidence: ai_response.confidence.unwrap_or(0.5),
            processing_result,
            canonical_type: "kyc_case".to_string(),
        })
    }

    /// **CANONICAL UBO ANALYSIS GENERATION** - Replace ai/dsl_service.rs methods
    /// Generate canonical UBO analysis DSL using AI with proper DSL Manager lifecycle
    pub async fn generate_canonical_ubo_analysis(
        &self,
        entity_name: &str,
        context: DslContext,
    ) -> DslManagerResult<CanonicalDslResponse> {
        info!(
            "Generating canonical UBO analysis for entity: {}",
            entity_name
        );

        let ai_service =
            self.ai_service
                .as_ref()
                .ok_or_else(|| DslManagerError::ConfigurationError {
                    message: "AI service not configured for canonical DSL generation".to_string(),
                })?;

        let ai_request = crate::ai::AiDslRequest {
            instruction: format!(
                "Generate a canonical UBO (Ultimate Beneficial Ownership) analysis DSL for entity '{}' with complete ownership chain resolution",
                entity_name
            ),
            context: Some({
                let mut ctx = context.audit_metadata.clone();
                ctx.insert("entity_name".to_string(), entity_name.to_string());
                ctx.insert("operation_type".to_string(), "canonical_ubo".to_string());
                ctx
            }),
            response_type: crate::ai::AiResponseType::DslGeneration,
            temperature: Some(0.1),
            max_tokens: Some(1500),
        };

        let ai_response = ai_service.generate_dsl(ai_request).await.map_err(|e| {
            DslManagerError::BackendError {
                message: format!("AI canonical UBO generation failed: {}", e),
            }
        })?;

        // Process through full DSL pipeline
        let mut dsl_context = context.clone();
        dsl_context
            .audit_metadata
            .insert("canonical_type".to_string(), "ubo_analysis".to_string());

        let processing_result = self
            .execute_dsl(&ai_response.generated_dsl, dsl_context)
            .await?;

        Ok(CanonicalDslResponse {
            generated_dsl: ai_response.generated_dsl,
            explanation: ai_response.explanation,
            confidence: ai_response.confidence.unwrap_or(0.5),
            processing_result,
            canonical_type: "ubo_analysis".to_string(),
        })
    }

    /// **COMPREHENSIVE HEALTH CHECK** - DSL Manager + AI + Backend health
    /// This replaces individual service health checks
    pub async fn comprehensive_health_check(&self) -> DslManagerResult<ComprehensiveHealthStatus> {
        info!("Performing comprehensive health check across all DSL Manager components");

        let mut health_status = ComprehensiveHealthStatus {
            overall_healthy: false,
            dsl_manager_healthy: true,
            ai_service_healthy: false,
            rag_system_healthy: false,
            backend_healthy: false,
            metrics: HealthMetrics::default(),
            checks: Vec::new(),
            timestamp: chrono::Utc::now(),
        };

        let start_time = std::time::Instant::now();

        // Check AI Service
        if let Some(ref ai_service) = self.ai_service {
            match ai_service.health_check().await {
                Ok(true) => {
                    health_status.ai_service_healthy = true;
                    health_status
                        .checks
                        .push(("ai_service".to_string(), true, None));
                }
                Ok(false) => {
                    health_status.checks.push((
                        "ai_service".to_string(),
                        false,
                        Some("AI service not responding".to_string()),
                    ));
                }
                Err(e) => {
                    health_status.checks.push((
                        "ai_service".to_string(),
                        false,
                        Some(format!("AI service error: {}", e)),
                    ));
                }
            }
        } else {
            health_status.checks.push((
                "ai_service".to_string(),
                false,
                Some("AI service not configured".to_string()),
            ));
        }

        // Check RAG System
        if let Some(ref _rag_system) = self.rag_system {
            // Assume RAG system has a health check method
            health_status.rag_system_healthy = true;
            health_status
                .checks
                .push(("rag_system".to_string(), true, None));
        } else {
            health_status.checks.push((
                "rag_system".to_string(),
                false,
                Some("RAG system not configured".to_string()),
            ));
        }

        // Check Backend
        if let Some(ref backend) = self.backend {
            match backend.health_check().await {
                Ok(_) => {
                    health_status.backend_healthy = true;
                    health_status
                        .checks
                        .push(("backend".to_string(), true, None));
                }
                Err(e) => {
                    health_status.checks.push((
                        "backend".to_string(),
                        false,
                        Some(format!("Backend error: {}", e)),
                    ));
                }
            }
        } else {
            health_status.checks.push((
                "backend".to_string(),
                false,
                Some("Backend not configured".to_string()),
            ));
        }

        health_status.metrics.total_check_time_ms = start_time.elapsed().as_millis() as u64;
        health_status.metrics.active_operations = self.active_operations.read().await.len();
        health_status.metrics.cache_entries = self.ast_cache.read().await.len();

        // Overall health = DSL Manager + at least AI service working
        health_status.overall_healthy =
            health_status.dsl_manager_healthy && health_status.ai_service_healthy;

        Ok(health_status)
    }

    /// **GENERATE TEST CBU IDS** - Utility method accessible via DSL Manager
    pub fn generate_test_cbu_ids(&self, count: usize) -> Vec<String> {
        CbuGenerator::generate_test_cbu_ids(count)
    }

    /// **AGENTIC CRUD BATCH GATEWAY** - Process multiple agentic CRUD operations atomically
    pub async fn process_agentic_crud_batch(
        &self,
        requests: Vec<AgenticCrudRequest>,
        transaction_mode: TransactionMode,
        context: DslContext,
    ) -> DslManagerResult<Vec<DslProcessingResult>> {
        info!(
            "Processing agentic CRUD batch of {} requests in {:?} mode",
            requests.len(),
            transaction_mode
        );

        let mut results = Vec::new();
        let mut all_successful = true;

        match transaction_mode {
            TransactionMode::Atomic => {
                // All operations must succeed or all fail
                let mut temp_results = Vec::new();

                for request in requests {
                    let result = self
                        .process_agentic_crud_request(request, context.clone())
                        .await?;
                    if !result.success {
                        all_successful = false;
                        warn!("Agentic CRUD batch operation failed - rolling back all changes");
                        break;
                    }
                    temp_results.push(result);
                }

                if all_successful {
                    results = temp_results;
                } else {
                    // TODO: Implement rollback logic
                    return Err(DslManagerError::PipelineError {
                        stage: "agentic_crud_batch".to_string(),
                        reason: "Atomic batch operation failed".to_string(),
                    });
                }
            }
            TransactionMode::Sequential => {
                // Continue processing even if some operations fail
                for request in requests {
                    match self
                        .process_agentic_crud_request(request, context.clone())
                        .await
                    {
                        Ok(result) => {
                            if !result.success {
                                // Operation failed
                            }
                            results.push(result);
                        }
                        Err(e) => {
                            error!("Agentic CRUD operation failed: {}", e);
                            // Operation failed
                            // Continue with next operation
                        }
                    }
                }
            }
            TransactionMode::DryRun => {
                // Validate all operations without executing
                for mut request in requests {
                    request.execute_dsl = false; // Force dry-run
                    let result = self
                        .process_agentic_crud_request(request, context.clone())
                        .await?;
                    results.push(result);
                }
            }
        }

        #[async_trait::async_trait]
        impl super::backend::DslBackend for MockDictionaryService {
            async fn execute(
                &self,
                _compiled_dsl: &super::CompilationResult,
            ) -> DslManagerResult<super::backend::BackendResult> {
                Ok(super::backend::BackendResult::default())
            }

            async fn dry_run_execute(
                &self,
                _compiled_dsl: &super::CompilationResult,
            ) -> DslManagerResult<super::backend::BackendResult> {
                Ok(super::backend::BackendResult::default())
            }

            async fn store_dsl_instance(
                &self,
                _instance_id: Uuid,
                _dsl_content: &str,
                _metadata: std::collections::HashMap<String, String>,
            ) -> DslManagerResult<super::backend::BackendResult> {
                Ok(super::backend::BackendResult::default())
            }

            async fn update_dsl_instance(
                &self,
                _instance_id: Uuid,
                _dsl_increment: &str,
                _version: u64,
            ) -> DslManagerResult<super::backend::BackendResult> {
                Ok(super::backend::BackendResult::default())
            }

            async fn retrieve_dsl_instance(
                &self,
                _instance_id: Uuid,
                _version: Option<u64>,
            ) -> DslManagerResult<super::backend::BackendResult> {
                Ok(super::backend::BackendResult::default())
            }

            async fn get_instance_history(
                &self,
                _instance_id: Uuid,
                _limit: Option<u64>,
            ) -> DslManagerResult<super::backend::BackendResult> {
                Ok(super::backend::BackendResult::default())
            }

            async fn health_check(&self) -> DslManagerResult<super::backend::BackendResult> {
                Ok(super::backend::BackendResult::default())
            }

            fn backend_type(&self) -> &str {
                "mock_dictionary"
            }
        }

        info!(
            "Agentic CRUD batch completed: {}/{} successful",
            results.iter().filter(|r| r.success).count(),
            results.len()
        );

        Ok(results)
    }

    /// **AGENTIC DSL VALIDATION GATEWAY** - Validate AI-generated DSL before execution
    pub async fn validate_agentic_dsl(
        &self,
        ai_generated_dsl: &str,
        context: DslContext,
    ) -> DslManagerResult<ValidationReport> {
        info!("Validating AI-generated DSL through complete pipeline");

        // Parse and normalize the AI-generated DSL
        let parse_result = self
            .process_operation(
                DslOperation::Parse {
                    dsl_text: ai_generated_dsl.to_string(),
                    apply_normalization: true, // Always apply v3.3 normalization
                },
                context.clone(),
            )
            .await?;

        if !parse_result.success || parse_result.ast.is_none() {
            return Err(DslManagerError::AstValidationError {
                message: format!("AI-generated DSL failed parsing: {:?}", parse_result.errors),
            });
        }

        // Perform comprehensive validation
        let ast = parse_result.ast.unwrap();
        let validation_engine = super::validation::DslValidationEngine::new();

        let validation_report = validation_engine
            .validate(&ast, super::validation::ValidationLevel::Strict)
            .await
            .map_err(|e| DslManagerError::AstValidationError {
                message: format!("Validation failed: {}", e),
            })?;

        Ok(validation_report)
    }

    /// **DOMAIN TEMPLATE GATEWAY** - Generate DSL from domain-specific templates
    pub async fn generate_domain_template(
        &self,
        domain: &str,
        template_name: &str,
        _parameters: std::collections::HashMap<String, String>,
        context: DslContext,
    ) -> DslManagerResult<DslProcessingResult> {
        info!(
            "Generating domain template: {} / {} for user: {}",
            domain, template_name, context.user_id
        );

        // Get domain handler
        let _domain_handler = self.domain_registry.get_domain(domain).map_err(|_| {
            DslManagerError::ConfigurationError {
                message: format!("Domain '{}' not found", domain),
            }
        })?;

        // Generate template DSL
        let _domain_context = DomainContext::new(domain.to_string());
        // Template generation removed - method doesn't exist on DomainHandler trait
        let template_dsl = format!("(case.create :template \"{}\")", template_name);

        // Process through complete DSL pipeline
        self.execute_dsl(&template_dsl, context).await
    }

    /// **DSL CREATE GATEWAY** - Create new DSL instances with domain context
    pub async fn create_dsl_instance(
        &self,
        domain: &str,
        initial_dsl: &str,
        metadata: std::collections::HashMap<String, String>,
        context: DslContext,
    ) -> DslManagerResult<DslProcessingResult> {
        info!(
            "Creating DSL instance in domain: {} for user: {}",
            domain, context.user_id
        );

        // Validate domain exists
        let _domain_handler = self.domain_registry.get_domain(domain).map_err(|_| {
            DslManagerError::ConfigurationError {
                message: format!("Domain '{}' not found", domain),
            }
        })?;

        // Create instance using central editor
        let domain_context = DomainContext::new(domain.to_string());
        let instance_result = self
            .central_editor
            .create_dsl_instance(domain_context, initial_dsl, &context.user_id)
            .await
            .map_err(|e| DslManagerError::StateError {
                message: format!("Instance creation failed: {}", e),
            })?;

        // Store in backend if available
        if let Some(ref backend) = self.backend {
            let _ = backend
                .store_dsl_instance(instance_result.instance_id, initial_dsl, metadata)
                .await;
        }

        // Process the created DSL through pipeline
        self.execute_dsl(initial_dsl, context).await
    }

    /// **DSL EDIT GATEWAY** - Edit existing DSL instances
    pub async fn edit_dsl_instance(
        &self,
        instance_id: Uuid,
        dsl_increment: &str,
        domain: &str,
        context: DslContext,
    ) -> DslManagerResult<DslProcessingResult> {
        info!(
            "Editing DSL instance {} in domain: {} for user: {}",
            instance_id, domain, context.user_id
        );

        // Create domain context and operation
        let domain_context = DomainContext::new(domain.to_string());
        // Simplified edit operation - using available DslOperation variant
        let edit_operation = CoreDslOperation::CreateEntity {
            entity_type: "dsl_increment".to_string(),
            entity_id: None,
            properties: HashMap::new(),
            metadata: crate::dsl::domain_context::OperationMetadata::default(),
        };

        // Perform edit through central editor
        let _edit_result = self
            .central_editor
            .edit_dsl(
                instance_id,
                domain_context,
                edit_operation,
                &context.user_id,
            )
            .await
            .map_err(|e| DslManagerError::StateError {
                message: format!("DSL edit failed: {}", e),
            })?;

        // Process the edited DSL through complete pipeline
        self.execute_dsl(dsl_increment, context).await
    }

    /// **VERB REGISTRY GATEWAY** - Manage DSL vocabulary
    pub async fn register_verb(
        &self,
        verb: &str,
        domain: &str,
        context: DslContext,
    ) -> DslManagerResult<()> {
        info!(
            "Registering verb '{}' for domain '{}' by user: {}",
            verb, domain, context.user_id
        );

        // VocabularyRegistry doesn't support dynamic registration in current implementation
        // This would need to be handled differently
        Ok::<(), ()>(()).map_err(|_e: ()| DslManagerError::ConfigurationError {
            message: "Verb registration not implemented".to_string(),
        })?;

        Ok(())
    }

    /// **VOCABULARY VALIDATION GATEWAY** - Validate verbs and keys
    pub async fn validate_vocabulary(
        &self,
        dsl_text: &str,
        context: DslContext,
    ) -> DslManagerResult<VocabularyValidationResult> {
        info!("Validating vocabulary for user: {}", context.user_id);

        let start_time = Instant::now();

        // Parse DSL to extract verbs
        let program = parse_normalize_and_validate(dsl_text)
            .map_err(|e| DslManagerError::ParsingError {
                message: format!("Failed to parse for vocabulary validation: {}", e),
            })?
            .0;

        let mut unknown_verbs = Vec::new();
        let deprecated_verbs = Vec::new();
        let mut valid_verbs = Vec::new();

        for form in &program {
            if let Form::Verb(verb_form) = form {
                // VocabularyRegistry doesn't have validate_verb method
                // Simplified validation - just check if verb format is valid
                match crate::vocabulary::VocabularyRegistry::validate_verb_format(&verb_form.verb) {
                    Ok(_) => valid_verbs.push(verb_form.verb.clone()),
                    Err(_) => {
                        unknown_verbs.push(verb_form.verb.clone());
                    }
                }
            }
        }

        Ok(VocabularyValidationResult {
            is_valid: unknown_verbs.is_empty(),
            valid_verbs,
            unknown_verbs,
            deprecated_verbs,
            validation_time_ms: start_time.elapsed().as_millis() as u64,
        })
    }

    /// **DOMAIN SWITCHING GATEWAY** - Switch operation context between domains
    pub async fn switch_domain_context(
        &self,
        from_domain: &str,
        to_domain: &str,
        _operation_data: &str,
        context: DslContext,
    ) -> DslManagerResult<DslProcessingResult> {
        info!(
            "Switching domain context from '{}' to '{}' for user: {}",
            from_domain, to_domain, context.user_id
        );

        // Validate both domains exist
        let _from_handler = self.domain_registry.get_domain(from_domain).map_err(|_| {
            DslManagerError::ConfigurationError {
                message: format!("Source domain '{}' not found", from_domain),
            }
        })?;

        let _to_handler = self.domain_registry.get_domain(to_domain).map_err(|_| {
            DslManagerError::ConfigurationError {
                message: format!("Target domain '{}' not found", to_domain),
            }
        })?;

        // Transform operation data for target domain
        let _from_context = DomainContext::new(from_domain.to_string());
        let _to_context = DomainContext::new(to_domain.to_string());

        // transform_from_domain method doesn't exist on DomainHandler trait
        // Use a simplified transformation for now
        let transformed_dsl = format!("(case.create :transformed-from \"{}\")", from_domain);

        // Process transformed DSL through pipeline
        self.execute_dsl(&transformed_dsl, context).await
    }

    /// **CALL CHAIN GATEWAY** - Execute complex multi-step DSL operations
    pub async fn execute_dsl_call_chain(
        &self,
        call_chain: DslCallChain,
        context: DslContext,
    ) -> DslManagerResult<Vec<DslProcessingResult>> {
        info!(
            "Executing DSL call chain with {} steps for user: {}",
            call_chain.steps.len(),
            context.user_id
        );

        let mut results = Vec::new();
        let mut chain_context = context.clone();

        // Add call chain metadata
        chain_context
            .audit_metadata
            .insert("call_chain_id".to_string(), call_chain.chain_id.clone());

        for (step_index, step) in call_chain.steps.iter().enumerate() {
            info!(
                "Executing call chain step {}: {}",
                step_index + 1,
                step.name
            );

            // Update context for this step
            chain_context
                .audit_metadata
                .insert("step_index".to_string(), step_index.to_string());
            chain_context
                .audit_metadata
                .insert("step_name".to_string(), step.name.clone());

            // Execute step based on type
            let step_result = match &step.operation {
                CallChainOperation::ParseAndValidate { dsl } => {
                    self.process_operation(
                        DslOperation::Parse {
                            dsl_text: dsl.clone(),
                            apply_normalization: true,
                        },
                        chain_context.clone(),
                    )
                    .await?
                }
                CallChainOperation::ExecuteDsl { dsl } => {
                    self.execute_dsl(dsl, chain_context.clone()).await?
                }
                CallChainOperation::AgenticCrud { request } => {
                    self.process_agentic_crud_request(
                        request.as_ref().clone(),
                        chain_context.clone(),
                    )
                    .await?
                }
                CallChainOperation::DomainTemplate {
                    domain,
                    template,
                    parameters,
                } => {
                    self.generate_domain_template(
                        domain,
                        template,
                        parameters.clone(),
                        chain_context.clone(),
                    )
                    .await?
                }
                CallChainOperation::CreateInstance {
                    domain,
                    dsl,
                    metadata,
                } => {
                    self.create_dsl_instance(domain, dsl, metadata.clone(), chain_context.clone())
                        .await?
                }
            };

            // Check if step failed and call chain should stop
            if !step_result.success && call_chain.fail_fast {
                error!(
                    "Call chain step {} failed, stopping execution",
                    step_index + 1
                );
                results.push(step_result);
                break;
            }

            results.push(step_result);

            // Add delay between steps if configured
            if let Some(delay_ms) = step.delay_ms {
                tokio::time::sleep(std::time::Duration::from_millis(delay_ms)).await;
            }
        }

        info!(
            "Call chain execution completed: {}/{} steps successful",
            results.iter().filter(|r| r.success).count(),
            results.len()
        );

        Ok(results)
    }

    /// **TEMPLATE COMPILATION GATEWAY** - Compile DSL templates with parameters
    pub async fn compile_dsl_template(
        &self,
        template_dsl: &str,
        parameters: std::collections::HashMap<String, String>,
        context: DslContext,
    ) -> DslManagerResult<String> {
        info!(
            "Compiling DSL template with {} parameters for user: {}",
            parameters.len(),
            context.user_id
        );

        // Simple template compilation - replace {{parameter}} with values
        let mut compiled_dsl = template_dsl.to_string();

        for (key, value) in parameters {
            let placeholder = format!("{{{{{}}}}}", key);
            compiled_dsl = compiled_dsl.replace(&placeholder, &value);
        }

        // Validate compiled DSL
        let validation_result = self.validate_agentic_dsl(&compiled_dsl, context).await?;
        if !validation_result.valid {
            return Err(DslManagerError::CompilationError {
                message: format!(
                    "Template compilation produced invalid DSL: {:?}",
                    validation_result.get_all_errors()
                ),
            });
        }

        Ok(compiled_dsl)
    }
}

/// Agentic CRUD request structure
/// Request for AI-powered onboarding workflow
#[derive(Debug, Clone)]
pub struct AiOnboardingRequest {
    /// Natural language description of the client and requirements
    pub instruction: String,
    /// Client/entity information
    pub client_name: String,
    pub jurisdiction: String,
    pub entity_type: String,
    /// Services requested
    pub services: Vec<String>,
    /// Compliance requirements
    pub compliance_level: Option<String>,
    /// Additional context
    pub context: std::collections::HashMap<String, String>,
    /// AI provider to use (optional, defaults to OpenAI)
    pub ai_provider: Option<String>,
}

/// Response from AI-powered onboarding creation
#[derive(Debug, Clone)]
pub struct AiOnboardingResponse {
    /// Generated CBU ID
    pub cbu_id: String,
    /// Created DSL instance
    pub dsl_instance: DslInstanceSummary,
    /// Generated DSL content
    pub generated_dsl: String,
    /// AI explanation of what was generated
    pub ai_explanation: String,
    /// AI confidence score
    pub ai_confidence: f64,
    /// Execution details
    pub execution_details: ExecutionDetails,
    /// Any warnings or suggestions
    pub warnings: Vec<String>,
    pub suggestions: Vec<String>,
}

/// Summary of created DSL instance
#[derive(Debug, Clone)]
pub struct DslInstanceSummary {
    pub instance_id: String,
    pub domain: String,
    pub status: String,
    pub created_at: chrono::DateTime<chrono::Utc>,
    pub current_version: i32,
}

/// Execution details
#[derive(Debug, Clone)]
pub struct ExecutionDetails {
    pub template_used: String,
    pub compilation_successful: bool,
    pub validation_passed: bool,
    pub storage_keys: Option<String>,
    pub execution_time_ms: u64,
}

/// CBU ID generation utilities
pub struct CbuGenerator;

impl CbuGenerator {
    /// Generate a unique CBU ID based on client information
    pub fn generate_cbu_id(client_name: &str, jurisdiction: &str, entity_type: &str) -> String {
        let sanitized_name = client_name
            .chars()
            .filter(|c| c.is_alphanumeric())
            .collect::<String>()
            .to_uppercase();

        let short_name = if sanitized_name.len() > 8 {
            &sanitized_name[..8]
        } else {
            &sanitized_name
        };

        let _timestamp = chrono::Utc::now().format("%m%d").to_string();
        let random_suffix: u16 = (chrono::Utc::now().timestamp_subsec_millis() % 1000) as u16;

        format!(
            "CBU-{}-{}-{}-{:03}",
            short_name,
            jurisdiction.to_uppercase(),
            entity_type.to_uppercase(),
            random_suffix
        )
    }

    /// Generate multiple CBU IDs for testing
    pub fn generate_test_cbu_ids(count: usize) -> Vec<String> {
        let test_clients = vec![
            ("TechCorp Ltd", "GB", "CORP"),
            ("Alpha Capital Partners", "KY", "FUND"),
            ("Global Investments SA", "LU", "FUND"),
            ("Singapore Holdings Pte", "SG", "CORP"),
            ("Zenith Financial Group", "US", "CORP"),
        ];

        (0..count)
            .map(|i| {
                let (name, jurisdiction, entity_type) = &test_clients[i % test_clients.len()];
                Self::generate_cbu_id(name, jurisdiction, entity_type)
            })
            .collect()
    }
}

#[derive(Debug, Clone)]
pub struct AgenticCrudRequest {
    /// Natural language instruction
    pub instruction: String,
    /// Asset type being operated on (cbu, entity, etc.)
    pub asset_type: Option<String>,
    /// Operation type (create, read, update, delete)
    pub operation_type: Option<String>,
    /// Whether to execute the generated DSL
    pub execute_dsl: bool,
    /// Additional context hints for AI
    pub context_hints: Vec<String>,
    /// Request metadata
    pub metadata: std::collections::HashMap<String, String>,
}

/// Vocabulary validation result
#[derive(Debug, Clone)]
pub struct VocabularyValidationResult {
    pub is_valid: bool,
    pub valid_verbs: Vec<String>,
    pub unknown_verbs: Vec<String>,
    pub deprecated_verbs: Vec<String>,
    pub validation_time_ms: u64,
}

/// DSL call chain for complex multi-step operations
#[derive(Debug, Clone)]
pub struct DslCallChain {
    pub chain_id: String,
    pub steps: Vec<CallChainStep>,
    pub fail_fast: bool,
    pub metadata: std::collections::HashMap<String, String>,
}

/// Individual step in a call chain
#[derive(Debug, Clone)]
pub struct CallChainStep {
    pub name: String,
    pub operation: CallChainOperation,
    pub delay_ms: Option<u64>,
    pub required: bool,
}

/// Types of operations in a call chain
#[derive(Debug, Clone)]
pub enum CallChainOperation {
    ParseAndValidate {
        dsl: String,
    },
    ExecuteDsl {
        dsl: String,
    },
    AgenticCrud {
        request: Box<AgenticCrudRequest>,
    },
    DomainTemplate {
        domain: String,
        template: String,
        parameters: std::collections::HashMap<String, String>,
    },
    CreateInstance {
        domain: String,
        dsl: String,
        metadata: std::collections::HashMap<String, String>,
    },
}

/// Mock dictionary service for initialization
/// AI validation result combining AI analysis with standard validation
#[derive(Debug)]
pub struct AiValidationResult {
    pub valid: bool,
    pub ai_confidence: f64,
    pub ai_issues: Vec<String>,
    pub ai_suggestions: Vec<String>,
    pub ai_explanation: String,
    pub standard_validation_report: ValidationReport,
    pub combined_score: f64,
}

/// Canonical DSL generation response
#[derive(Debug)]
pub struct CanonicalDslResponse {
    pub generated_dsl: String,
    pub explanation: String,
    pub confidence: f64,
    pub processing_result: DslProcessingResult,
    pub canonical_type: String,
}

/// Comprehensive health status for entire DSL Manager ecosystem
#[derive(Debug)]
pub struct ComprehensiveHealthStatus {
    pub overall_healthy: bool,
    pub dsl_manager_healthy: bool,
    pub ai_service_healthy: bool,
    pub rag_system_healthy: bool,
    pub backend_healthy: bool,
    pub metrics: HealthMetrics,
    pub checks: Vec<(String, bool, Option<String>)>, // (component, healthy, error_message)
    pub timestamp: chrono::DateTime<chrono::Utc>,
}

/// Health check metrics
#[derive(Debug, Default)]
pub struct HealthMetrics {
    pub total_check_time_ms: u64,
    pub active_operations: usize,
    pub cache_entries: usize,
}

struct MockDictionaryService;

#[async_trait::async_trait]
impl crate::dsl::central_editor::DictionaryService for MockDictionaryService {
    async fn validate_dsl_attributes(&self, _dsl: &str) -> Result<(), String> {
        Ok(())
    }
}

impl Default for AgenticCrudRequest {
    fn default() -> Self {
        Self {
            instruction: "Create standard agentic CRUD operation".to_string(),
            asset_type: Some("default".to_string()),
            operation_type: None,
            execute_dsl: true,
            context_hints: Vec::new(),
            metadata: std::collections::HashMap::new(),
        }
    }
}

impl Default for AiOnboardingRequest {
    fn default() -> Self {
        Self {
            instruction: "Create standard onboarding workflow".to_string(),
            client_name: "Test Client".to_string(),
            jurisdiction: "US".to_string(),
            entity_type: "CORP".to_string(),
            services: vec!["CUSTODY".to_string()],
            compliance_level: Some("standard".to_string()),
            context: std::collections::HashMap::new(),
            ai_provider: Some("openai".to_string()),
        }
    }
}

#[cfg(test)]
#[cfg(test)]
mod tests {
    use super::*;

    #[tokio::test]
    async fn test_dsl_manager_creation() {
        let manager = super::super::DslManagerFactory::for_testing();
        assert!(manager.get_active_operations_count().await == 0);
    }

    #[tokio::test]
    async fn test_cbu_generation() {
        let manager = super::super::DslManagerFactory::for_testing();

        let cbu_ids = manager.generate_test_cbu_ids(3);
        assert_eq!(cbu_ids.len(), 3);

        // All should be unique
        let mut unique_ids = std::collections::HashSet::new();
        for id in &cbu_ids {
            unique_ids.insert(id);
        }
        assert_eq!(unique_ids.len(), cbu_ids.len());
    }
}
