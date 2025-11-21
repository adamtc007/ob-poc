//! DSL Orchestration Interface
//!
//! This module defines the interface for DSL Manager to orchestrate DSL Mod operations
//! following the call chain pattern:
//! DSL_Manager (Entry Point) → DSL Mod (Processing) → Database/SQLx (Persistence) → Response
//!
//! The orchestration interface provides a clean abstraction layer that allows DSL Manager
//! to coordinate complex DSL operations through DSL Mod while maintaining separation of concerns.

use async_trait::async_trait;
use serde::{Deserialize, Serialize};
use std::collections::HashMap;
use std::time::{SystemTime, UNIX_EPOCH};

use crate::error::DSLResult;

/// Interface for DSL Manager to orchestrate DSL operations
#[async_trait]
pub trait DslOrchestrationInterface {
    /// Get current performance metrics
    async fn get_orchestration_metrics(&self) -> DSLResult<OrchestrationMetrics>;

    /// Reset performance metrics
    async fn reset_orchestration_metrics(&self) -> DSLResult<()>;
    /// Process DSL operation from DSL Manager
    async fn process_orchestrated_operation(
        &self,
        operation: OrchestrationOperation,
    ) -> DSLResult<OrchestrationResult>;

    /// Validate DSL from DSL Manager
    async fn validate_orchestrated_dsl(
        &self,
        dsl_content: &str,
        context: OrchestrationContext,
    ) -> DSLResult<ValidationReport>;

    /// Execute DSL from DSL Manager
    async fn execute_orchestrated_dsl(
        &self,
        dsl_content: &str,
        context: OrchestrationContext,
    ) -> DSLResult<ExecutionResult>;

    /// Parse DSL from DSL Manager
    async fn parse_orchestrated_dsl(
        &self,
        dsl_content: &str,
        context: OrchestrationContext,
    ) -> DSLResult<ParseResult>;

    /// Transform DSL from DSL Manager
    async fn transform_orchestrated_dsl(
        &self,
        dsl_content: &str,
        transform_type: TransformationType,
        context: OrchestrationContext,
    ) -> DSLResult<TransformationResult>;

    /// Health check for orchestrated components
    async fn orchestration_health_check(&self) -> DSLResult<HealthStatus>;
}

/// Operation from DSL Manager to DSL Mod
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct OrchestrationOperation {
    /// Unique operation identifier
    pub operation_id: String,
    /// Type of operation to perform
    pub operation_type: OrchestrationOperationType,
    /// DSL content to process
    pub dsl_content: String,
    /// Processing context
    pub context: OrchestrationContext,
    /// Additional metadata
    pub metadata: HashMap<String, String>,
    /// Operation priority (1-10, 10 being highest)
    pub priority: u8,
    /// Maximum processing time allowed (milliseconds)
    pub timeout_ms: Option<u64>,
}

/// Types of orchestration operations
#[derive(Debug, Clone, Serialize, Deserialize, PartialEq)]
pub enum OrchestrationOperationType {
    /// Parse DSL into AST
    Parse,
    /// Validate DSL syntax and semantics
    Validate,
    /// Execute DSL operations
    Execute,
    /// Transform DSL to different format
    Transform,
    /// Compile DSL to executable form
    Compile,
    /// Parse, validate, and execute in one operation
    ProcessComplete,
    /// Batch operation on multiple DSL fragments
    BatchProcess,
}

/// Context passed from DSL Manager to DSL Mod
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct OrchestrationContext {
    /// Unique request identifier for tracing
    pub request_id: String,
    /// User or system identifier
    pub user_id: String,
    /// Domain context (kyc, ubo, isda, etc.)
    pub domain: String,
    /// Case ID if applicable
    pub case_id: Option<String>,
    /// Processing options
    pub processing_options: ProcessingOptions,
    /// Audit trail entries
    pub audit_trail: Vec<String>,
    /// Timestamp when context was created
    pub created_at: u64,
    /// Session information
    pub session: SessionInfo,
}

/// Processing options for orchestration
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ProcessingOptions {
    /// Enable strict validation
    pub strict_validation: bool,
    /// Fail fast on first error
    pub fail_fast: bool,
    /// Enable detailed logging
    pub enable_logging: bool,
    /// Enable performance metrics collection
    pub collect_metrics: bool,
    /// Enable database persistence
    pub persist_to_database: bool,
    /// Enable visualization generation
    pub generate_visualization: bool,
    /// Custom processing flags
    pub custom_flags: HashMap<String, String>,
}

/// Session information for orchestration context
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct SessionInfo {
    /// Session ID
    pub session_id: String,
    /// Session start time
    pub started_at: u64,
    /// Session permissions
    pub permissions: Vec<String>,
    /// Session metadata
    pub metadata: HashMap<String, String>,
}

/// Result back to DSL Manager from orchestration
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct OrchestrationResult {
    /// Whether the operation succeeded
    pub success: bool,
    /// Original operation ID
    pub operation_id: String,
    /// Result data (JSON serialized)
    pub result_data: Option<String>,
    /// Any errors encountered
    pub errors: Vec<String>,
    /// Non-fatal warnings
    pub warnings: Vec<String>,
    /// Processing time in milliseconds
    pub processing_time_ms: u64,
    /// Timestamp when result was created
    pub completed_at: u64,
    /// Detailed step results
    pub step_results: Vec<StepResult>,
    /// Performance metrics
    pub metrics: OrchestrationMetrics,
}

/// Individual step result in orchestration
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct StepResult {
    /// Step name
    pub step_name: String,
    /// Step success status
    pub success: bool,
    /// Step processing time
    pub processing_time_ms: u64,
    /// Step-specific data
    pub step_data: HashMap<String, String>,
    /// Step errors
    pub errors: Vec<String>,
    /// Step warnings
    pub warnings: Vec<String>,
}

/// Performance metrics for orchestration
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct OrchestrationMetrics {
    /// Total operations processed
    pub total_operations: u64,
    /// Successful operations
    pub successful_operations: u64,
    /// Failed operations
    pub failed_operations: u64,
    /// Average processing time
    pub average_processing_time_ms: f64,
    /// DSL Manager to DSL Mod latency
    pub orchestration_latency_ms: f64,
    /// Memory usage in bytes
    pub memory_usage_bytes: usize,
    /// CPU usage percentage
    pub cpu_usage_percent: f32,
    /// Peak memory usage during operation
    pub peak_memory_bytes: usize,
    /// Database operation count
    pub database_operations_count: u32,
    /// Cache hit rate (0.0 to 1.0)
    pub cache_hit_rate: f64,
    /// Error rate (0.0 to 1.0)
    pub error_rate: f64,
    /// Throughput (operations per second)
    pub operations_per_second: f64,
    /// Concurrent operations count
    pub concurrent_operations: u32,
    /// Queue depth for pending operations
    pub queue_depth: u32,
}

/// Validation report from orchestrated validation
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ValidationReport {
    /// Overall validation success
    pub valid: bool,
    /// Validation errors
    pub errors: Vec<ValidationError>,
    /// Validation warnings
    pub warnings: Vec<ValidationWarning>,
    /// Rules that were checked
    pub rules_checked: Vec<String>,
    /// Compliance score (0.0 to 1.0)
    pub compliance_score: f64,
    /// Validation time
    pub validation_time_ms: u64,
    /// Domain-specific validation results
    pub domain_results: HashMap<String, DomainValidationResult>,
}

/// Validation error details
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ValidationError {
    /// Error code
    pub code: String,
    /// Human-readable message
    pub message: String,
    /// Location in DSL where error occurred
    pub location: Option<SourceLocation>,
    /// Suggested fix
    pub suggestion: Option<String>,
}

/// Validation warning details
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ValidationWarning {
    /// Warning code
    pub code: String,
    /// Human-readable message
    pub message: String,
    /// Location in DSL where warning occurred
    pub location: Option<SourceLocation>,
    /// Recommended action
    pub recommendation: Option<String>,
}

/// Source location for errors/warnings
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct SourceLocation {
    /// Line number (1-based)
    pub line: u32,
    /// Column number (1-based)
    pub column: u32,
    /// Character offset
    pub offset: usize,
    /// Length of the problematic section
    pub length: usize,
}

/// Domain-specific validation result
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct DomainValidationResult {
    /// Domain name
    pub domain: String,
    /// Domain validation success
    pub valid: bool,
    /// Domain-specific errors
    pub errors: Vec<String>,
    /// Domain-specific warnings
    pub warnings: Vec<String>,
    /// Domain compliance score
    pub compliance_score: f64,
}

/// Execution result from orchestrated execution
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ExecutionResult {
    /// Execution success status
    pub success: bool,
    /// Execution output/result
    pub output: Option<String>,
    /// Execution errors
    pub errors: Vec<String>,
    /// Execution warnings
    pub warnings: Vec<String>,
    /// Execution time
    pub execution_time_ms: u64,
    /// Database operations performed
    pub database_operations: Vec<DatabaseOperation>,
    /// Side effects produced
    pub side_effects: Vec<SideEffect>,
}

/// Database operation performed during execution
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct DatabaseOperation {
    /// Operation type (INSERT, UPDATE, DELETE, etc.)
    pub operation_type: String,
    /// Table or collection affected
    pub target: String,
    /// Number of rows/documents affected
    pub affected_count: u64,
    /// Operation success
    pub success: bool,
    /// Operation error if any
    pub error: Option<String>,
}

/// Side effect from DSL execution
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct SideEffect {
    /// Effect type
    pub effect_type: String,
    /// Effect description
    pub description: String,
    /// Effect data
    pub data: HashMap<String, String>,
}

/// Parse result from orchestrated parsing
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ParseResult {
    /// Parse success status
    pub success: bool,
    /// Parsed AST (JSON serialized)
    pub ast: Option<String>,
    /// Parse errors
    pub errors: Vec<String>,
    /// Parse warnings
    pub warnings: Vec<String>,
    /// Parse time
    pub parse_time_ms: u64,
    /// Syntax tree metrics
    pub ast_metrics: AstMetrics,
}

/// AST metrics from parsing
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct AstMetrics {
    /// Number of nodes in AST
    pub node_count: usize,
    /// Maximum depth of AST
    pub max_depth: u32,
    /// Number of statements
    pub statement_count: usize,
    /// Number of expressions
    pub expression_count: usize,
    /// Memory usage of AST in bytes
    pub memory_usage_bytes: usize,
}

/// Transformation result from orchestrated transformation
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct TransformationResult {
    /// Transformation success status
    pub success: bool,
    /// Transformed output
    pub transformed_content: Option<String>,
    /// Original format
    pub original_format: String,
    /// Target format
    pub target_format: String,
    /// Transformation errors
    pub errors: Vec<String>,
    /// Transformation warnings
    pub warnings: Vec<String>,
    /// Transformation time
    pub transformation_time_ms: u64,
}

/// Types of transformations available
#[derive(Debug, Clone, Serialize, Deserialize, PartialEq)]
pub enum TransformationType {
    /// Convert to JSON format
    ToJson,
    /// Convert to YAML format
    ToYaml,
    /// Convert to XML format
    ToXml,
    /// Normalize DSL format
    Normalize,
    /// Minify DSL content
    Minify,
    /// Pretty print DSL content
    PrettyPrint,
    /// Convert to SQL operations
    ToSql,
    /// Convert to documentation format
    ToDocumentation,
}

/// Health status for orchestration components
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct HealthStatus {
    /// Overall health status
    pub healthy: bool,
    /// Component health details
    pub components: HashMap<String, ComponentHealth>,
    /// System metrics
    pub system_metrics: SystemMetrics,
    /// Health check time
    pub checked_at: u64,
}

/// Individual component health
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ComponentHealth {
    /// Component name
    pub name: String,
    /// Component health status
    pub healthy: bool,
    /// Health message
    pub message: String,
    /// Component metrics
    pub metrics: HashMap<String, f64>,
    /// Last health check time
    pub last_check: u64,
}

/// System-level metrics
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct SystemMetrics {
    /// Memory usage in bytes
    pub memory_usage_bytes: usize,
    /// CPU usage percentage
    pub cpu_usage_percent: f32,
    /// Disk usage percentage
    pub disk_usage_percent: f32,
    /// Network latency in milliseconds
    pub network_latency_ms: f64,
    /// Active connections
    pub active_connections: u32,
}

impl OrchestrationContext {
    /// Create a new orchestration context
    pub fn new(user_id: String, domain: String) -> Self {
        let now = SystemTime::now()
            .duration_since(UNIX_EPOCH)
            .unwrap_or_default()
            .as_secs();

        Self {
            request_id: uuid::Uuid::new_v4().to_string(),
            user_id,
            domain,
            case_id: None,
            processing_options: ProcessingOptions::default(),
            audit_trail: Vec::new(),
            created_at: now,
            session: SessionInfo::default(),
        }
    }

    /// Add entry to audit trail
    pub fn add_audit_entry(&mut self, entry: String) {
        let now = SystemTime::now()
            .duration_since(UNIX_EPOCH)
            .unwrap_or_default()
            .as_secs();

        self.audit_trail.push(format!("[{}] {}", now, entry));
    }

    /// Set case ID for context
    pub fn with_case_id(mut self, case_id: String) -> Self {
        self.case_id = Some(case_id);
        self
    }
}

impl ProcessingOptions {
    /// Create default processing options
    pub fn new() -> Self {
        Self::default()
    }

    /// Enable all processing features
    pub fn all_enabled() -> Self {
        Self {
            strict_validation: true,
            fail_fast: false,
            enable_logging: true,
            collect_metrics: true,
            persist_to_database: true,
            generate_visualization: true,
            custom_flags: HashMap::new(),
        }
    }

    /// Minimal processing options for testing
    pub fn minimal() -> Self {
        Self {
            strict_validation: false,
            fail_fast: true,
            enable_logging: false,
            collect_metrics: false,
            persist_to_database: false,
            generate_visualization: false,
            custom_flags: HashMap::new(),
        }
    }
}

impl Default for ProcessingOptions {
    fn default() -> Self {
        Self {
            strict_validation: true,
            fail_fast: false,
            enable_logging: true,
            collect_metrics: true,
            persist_to_database: false, // Default to false for safety
            generate_visualization: true,
            custom_flags: HashMap::new(),
        }
    }
}

impl Default for SessionInfo {
    fn default() -> Self {
        let now = SystemTime::now()
            .duration_since(UNIX_EPOCH)
            .unwrap_or_default()
            .as_secs();

        Self {
            session_id: uuid::Uuid::new_v4().to_string(),
            started_at: now,
            permissions: vec!["read".to_string(), "write".to_string()],
            metadata: HashMap::new(),
        }
    }
}

impl OrchestrationOperation {
    /// Create a new orchestration operation
    pub fn new(
        operation_type: OrchestrationOperationType,
        dsl_content: String,
        context: OrchestrationContext,
    ) -> Self {
        Self {
            operation_id: uuid::Uuid::new_v4().to_string(),
            operation_type,
            dsl_content,
            context,
            metadata: HashMap::new(),
            priority: 5,              // Default priority
            timeout_ms: Some(30_000), // Default 30 second timeout
        }
    }

    /// Set priority for operation
    pub fn with_priority(mut self, priority: u8) -> Self {
        self.priority = priority.clamp(1, 10);
        self
    }

    /// Set timeout for operation
    pub fn with_timeout(mut self, timeout_ms: u64) -> Self {
        self.timeout_ms = Some(timeout_ms);
        self
    }

    /// Add metadata to operation
    pub fn add_metadata(mut self, key: String, value: String) -> Self {
        self.metadata.insert(key, value);
        self
    }
}

impl OrchestrationMetrics {
    /// Create new metrics instance
    pub fn new() -> Self {
        Self {
            total_operations: 0,
            successful_operations: 0,
            failed_operations: 0,
            average_processing_time_ms: 0.0,
            orchestration_latency_ms: 0.0,
            memory_usage_bytes: 0,
            cpu_usage_percent: 0.0,
            peak_memory_bytes: 0,
            database_operations_count: 0,
            cache_hit_rate: 0.0,
            error_rate: 0.0,
            operations_per_second: 0.0,
            concurrent_operations: 0,
            queue_depth: 0,
        }
    }

    /// Update metrics with operation result
    pub fn update_with_operation(&mut self, success: bool, processing_time_ms: u64, db_ops: u32) {
        self.total_operations += 1;

        if success {
            self.successful_operations += 1;
        } else {
            self.failed_operations += 1;
        }

        // Update average processing time using incremental formula
        let new_average = (self.average_processing_time_ms * (self.total_operations - 1) as f64
            + processing_time_ms as f64)
            / self.total_operations as f64;
        self.average_processing_time_ms = new_average;

        self.database_operations_count += db_ops;
        self.error_rate = self.failed_operations as f64 / self.total_operations as f64;

        // Update throughput (simple approximation)
        if self.average_processing_time_ms > 0.0 {
            self.operations_per_second = 1000.0 / self.average_processing_time_ms;
        }
    }

    /// Update system resource metrics
    pub fn update_system_metrics(&mut self, memory_bytes: usize, cpu_percent: f32) {
        self.memory_usage_bytes = memory_bytes;
        self.cpu_usage_percent = cpu_percent;

        if memory_bytes > self.peak_memory_bytes {
            self.peak_memory_bytes = memory_bytes;
        }
    }

    /// Update cache metrics
    pub fn update_cache_metrics(&mut self, hit_rate: f64) {
        self.cache_hit_rate = hit_rate.clamp(0.0, 1.0);
    }

    /// Update concurrency metrics
    pub fn update_concurrency(&mut self, active_ops: u32, queue_size: u32) {
        self.concurrent_operations = active_ops;
        self.queue_depth = queue_size;
    }

    /// Update latency metrics
    pub fn update_latency(&mut self, latency_ms: f64) {
        self.orchestration_latency_ms = latency_ms;
    }

    /// Get performance summary as a formatted string
    pub fn performance_summary(&self) -> String {
        format!(
            "Orchestration Metrics: {} total ops, {:.1}% success rate, {:.2}ms avg time, {:.1} ops/sec",
            self.total_operations,
            (self.successful_operations as f64 / self.total_operations.max(1) as f64) * 100.0,
            self.average_processing_time_ms,
            self.operations_per_second
        )
    }

    /// Reset all metrics to initial state
    pub fn reset(&mut self) {
        *self = Self::new();
    }
}

impl Default for OrchestrationMetrics {
    fn default() -> Self {
        Self::new()
    }
}

impl OrchestrationResult {
    /// Create a successful orchestration result
    pub fn success(operation_id: String, processing_time_ms: u64) -> Self {
        let now = SystemTime::now()
            .duration_since(UNIX_EPOCH)
            .unwrap_or_default()
            .as_secs();

        Self {
            success: true,
            operation_id,
            result_data: None,
            errors: Vec::new(),
            warnings: Vec::new(),
            processing_time_ms,
            completed_at: now,
            step_results: Vec::new(),
            metrics: OrchestrationMetrics::default(),
        }
    }

    /// Create a failed orchestration result
    pub fn failure(operation_id: String, errors: Vec<String>, processing_time_ms: u64) -> Self {
        let now = SystemTime::now()
            .duration_since(UNIX_EPOCH)
            .unwrap_or_default()
            .as_secs();

        Self {
            success: false,
            operation_id,
            result_data: None,
            errors,
            warnings: Vec::new(),
            processing_time_ms,
            completed_at: now,
            step_results: Vec::new(),
            metrics: OrchestrationMetrics::default(),
        }
    }

    /// Add result data to the result
    pub fn with_data(mut self, data: String) -> Self {
        self.result_data = Some(data);
        self
    }

    /// Add step result
    pub fn add_step_result(mut self, step: StepResult) -> Self {
        self.step_results.push(step);
        self
    }
}

/// Utility functions for orchestration
impl OrchestrationOperation {
    /// Check if operation has timed out
    pub fn is_timed_out(&self, start_time: SystemTime) -> bool {
        if let Some(timeout_ms) = self.timeout_ms {
            let elapsed = start_time.elapsed().unwrap_or_default().as_millis() as u64;
            elapsed > timeout_ms
        } else {
            false
        }
    }

    /// Get operation priority as string
    pub fn priority_str(&self) -> &'static str {
        match self.priority {
            9..=10 => "Critical",
            7..=8 => "High",
            4..=6 => "Normal",
            2..=3 => "Low",
            1 => "Minimal",
            _ => "Unknown",
        }
    }
}
