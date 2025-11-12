//! DSL Manager Compiler Module
//!
//! This module provides compilation capabilities for the DSL Manager,
//! converting parsed and validated DSL into executable operations.

use super::{DslManagerError, DslManagerResult};
use crate::{Form, Program, VerbForm};
use async_trait::async_trait;
use serde::{Deserialize, Serialize};
use std::collections::HashMap;
use uuid::Uuid;

/// DSL compilation result
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct CompilationResult {
    /// Compilation success status
    pub success: bool,
    /// Executable operations (JSON format for backend compatibility)
    pub executable_operations: Option<Vec<serde_json::Value>>,
    /// Compilation metadata
    pub metadata: HashMap<String, String>,
    /// Compilation time in milliseconds
    pub compilation_time_ms: u64,
    /// Any compilation errors
    pub errors: Vec<String>,
    /// Warning messages
    pub warnings: Vec<String>,
}

/// Execution context for compilation
#[derive(Debug, Clone, Default, PartialEq)]
pub struct ExecutionContext {
    /// Target execution environment
    pub target_environment: String,
    /// Execution mode (production, staging, test)
    pub execution_mode: String,
    /// User context
    pub user_id: String,
    /// Session context
    pub session_id: String,
    /// Execution parameters
    pub parameters: HashMap<String, String>,
    /// Timeout for operations (seconds)
    pub timeout_seconds: Option<u64>,
}

/// DSL compiler trait
#[async_trait]
pub trait Compiler: Send + Sync {
    /// Compile a DSL program into executable operations
    async fn compile(
        &self,
        program: Program,
        context: ExecutionContext,
    ) -> DslManagerResult<CompilationResult>;

    /// Validate compilation target
    async fn validate_target(&self, target: &str) -> DslManagerResult<bool>;

    /// Get supported compilation targets
    fn supported_targets(&self) -> Vec<String>;
}

/// Main DSL compiler implementation
pub struct DslCompiler {
    /// Compilation strategy registry
    strategies: HashMap<String, Box<dyn CompilationStrategy>>,
    /// Compiler configuration
    config: CompilerConfig,
}

/// Compilation strategy trait for different target types
#[async_trait]
pub trait CompilationStrategy: Send + Sync {
    /// Strategy name
    fn name(&self) -> &str;

    /// Compile for this specific strategy
    async fn compile_program(
        &self,
        program: &Program,
        context: &ExecutionContext,
    ) -> DslManagerResult<Vec<serde_json::Value>>;

    /// Validate program for this strategy
    async fn validate_program(
        &self,
        program: &Program,
        context: &ExecutionContext,
    ) -> DslManagerResult<bool>;
}

/// Compiler configuration
#[derive(Debug, Clone)]
pub struct CompilerConfig {
    /// Default target environment
    pub default_target: String,
    /// Enable optimization
    pub enable_optimization: bool,
    /// Maximum compilation time (ms)
    pub max_compilation_time_ms: u64,
    /// Enable detailed error reporting
    pub detailed_errors: bool,
}

impl Default for CompilerConfig {
    fn default() -> Self {
        Self {
            default_target: "database".to_string(),
            enable_optimization: true,
            max_compilation_time_ms: 10000,
            detailed_errors: true,
        }
    }
}

impl DslCompiler {
    /// Create a new DSL compiler
    pub fn new() -> Self {
        let mut compiler = Self {
            strategies: HashMap::new(),
            config: CompilerConfig::default(),
        };

        // Register default compilation strategies
        compiler.register_strategy(Box::new(DatabaseCompilationStrategy::new()));
        compiler.register_strategy(Box::new(MockCompilationStrategy::new()));
        compiler.register_strategy(Box::new(CrudCompilationStrategy::new()));

        compiler
    }

    /// Create with custom configuration
    pub fn with_config(config: CompilerConfig) -> Self {
        let mut compiler = Self::new();
        compiler.config = config;
        compiler
    }

    /// Register a compilation strategy
    pub fn register_strategy(&mut self, strategy: Box<dyn CompilationStrategy>) {
        self.strategies
            .insert(strategy.name().to_string(), strategy);
    }
}

#[async_trait]
impl Compiler for DslCompiler {
    async fn compile(
        &self,
        program: Program,
        context: ExecutionContext,
    ) -> DslManagerResult<CompilationResult> {
        let start_time = std::time::Instant::now();
        let mut metadata = HashMap::new();

        // Determine compilation target
        let target = if context.target_environment.is_empty() {
            &self.config.default_target
        } else {
            &context.target_environment
        };

        metadata.insert("target".to_string(), target.clone());
        metadata.insert(
            "compilation_mode".to_string(),
            context.execution_mode.clone(),
        );

        // Get compilation strategy
        let strategy =
            self.strategies
                .get(target)
                .ok_or_else(|| DslManagerError::CompilationError {
                    message: format!("No compilation strategy for target: {}", target),
                })?;

        // Validate program for this strategy
        let is_valid = strategy.validate_program(&program, &context).await?;
        if !is_valid {
            return Ok(CompilationResult {
                success: false,
                executable_operations: None,
                metadata,
                compilation_time_ms: start_time.elapsed().as_millis() as u64,
                errors: vec![format!("Program validation failed for target: {}", target)],
                warnings: vec![],
            });
        }

        // Perform compilation
        match strategy.compile_program(&program, &context).await {
            Ok(operations) => {
                let compilation_time_ms = start_time.elapsed().as_millis() as u64;

                // Check timeout
                if compilation_time_ms > self.config.max_compilation_time_ms {
                    return Ok(CompilationResult {
                        success: false,
                        executable_operations: None,
                        metadata,
                        compilation_time_ms,
                        errors: vec![format!(
                            "Compilation timeout after {}ms",
                            self.config.max_compilation_time_ms
                        )],
                        warnings: vec![],
                    });
                }

                metadata.insert("operations_count".to_string(), operations.len().to_string());

                Ok(CompilationResult {
                    success: true,
                    executable_operations: Some(operations),
                    metadata,
                    compilation_time_ms,
                    errors: vec![],
                    warnings: vec![],
                })
            }
            Err(e) => Ok(CompilationResult {
                success: false,
                executable_operations: None,
                metadata,
                compilation_time_ms: start_time.elapsed().as_millis() as u64,
                errors: vec![format!("Compilation failed: {}", e)],
                warnings: vec![],
            }),
        }
    }

    async fn validate_target(&self, target: &str) -> DslManagerResult<bool> {
        Ok(self.strategies.contains_key(target))
    }

    fn supported_targets(&self) -> Vec<String> {
        self.strategies.keys().cloned().collect()
    }
}

/// Database compilation strategy
pub struct DatabaseCompilationStrategy {
    config: DatabaseStrategyConfig,
}

#[derive(Debug, Clone)]
pub struct DatabaseStrategyConfig {
    pub schema_name: String,
    pub enable_transactions: bool,
    pub batch_size: usize,
}

impl Default for DatabaseStrategyConfig {
    fn default() -> Self {
        Self {
            schema_name: "ob-poc".to_string(),
            enable_transactions: true,
            batch_size: 100,
        }
    }
}

impl DatabaseCompilationStrategy {
    pub fn new() -> Self {
        Self {
            config: DatabaseStrategyConfig::default(),
        }
    }
}

#[async_trait]
impl CompilationStrategy for DatabaseCompilationStrategy {
    fn name(&self) -> &str {
        "database"
    }

    async fn compile_program(
        &self,
        program: &Program,
        context: &ExecutionContext,
    ) -> DslManagerResult<Vec<serde_json::Value>> {
        let mut operations = Vec::new();

        for form in program {
            if let Form::Verb(verb_form) = form {
                let operation = self.compile_verb_form(verb_form, context).await?;
                operations.push(operation);
            }
        }

        Ok(operations)
    }

    async fn validate_program(
        &self,
        program: &Program,
        _context: &ExecutionContext,
    ) -> DslManagerResult<bool> {
        // Validate all forms can be compiled to database operations
        for form in program {
            if let Form::Verb(verb_form) = form {
                if !self.is_database_compatible_verb(&verb_form.verb) {
                    return Ok(false);
                }
            }
        }
        Ok(true)
    }
}

impl DatabaseCompilationStrategy {
    async fn compile_verb_form(
        &self,
        verb_form: &VerbForm,
        context: &ExecutionContext,
    ) -> DslManagerResult<serde_json::Value> {
        match verb_form.verb.as_str() {
            "case.create" => self.compile_case_create(verb_form, context).await,
            "case.update" => self.compile_case_update(verb_form, context).await,
            "case.approve" => self.compile_case_approve(verb_form, context).await,
            "entity.register" => self.compile_entity_register(verb_form, context).await,
            "entity.link" => self.compile_entity_link(verb_form, context).await,
            "document.catalog" => self.compile_document_catalog(verb_form, context).await,
            "document.use" => self.compile_document_use(verb_form, context).await,
            "workflow.transition" => self.compile_workflow_transition(verb_form, context).await,
            "data.create" => self.compile_data_create(verb_form, context).await,
            "data.read" => self.compile_data_read(verb_form, context).await,
            "data.update" => self.compile_data_update(verb_form, context).await,
            "data.delete" => self.compile_data_delete(verb_form, context).await,
            _ => Ok(serde_json::json!({
                "operation": "unsupported",
                "verb": verb_form.verb,
                "error": "Verb not supported for database compilation"
            })),
        }
    }

    async fn compile_case_create(
        &self,
        verb_form: &VerbForm,
        _context: &ExecutionContext,
    ) -> DslManagerResult<serde_json::Value> {
        Ok(serde_json::json!({
            "type": "sql_insert",
            "table": format!("\"{}\".cases", self.config.schema_name),
            "data": verb_form.pairs,
            "returning": ["case_id", "created_at"]
        }))
    }

    async fn compile_case_update(
        &self,
        verb_form: &VerbForm,
        _context: &ExecutionContext,
    ) -> DslManagerResult<serde_json::Value> {
        Ok(serde_json::json!({
            "type": "sql_update",
            "table": format!("\"{}\".cases", self.config.schema_name),
            "data": verb_form.pairs,
            "where": {"case_id": verb_form.pairs.get(&crate::Key::new("case-id"))}
        }))
    }

    async fn compile_case_approve(
        &self,
        verb_form: &VerbForm,
        _context: &ExecutionContext,
    ) -> DslManagerResult<serde_json::Value> {
        Ok(serde_json::json!({
            "type": "sql_update",
            "table": format!("\"{}\".cases", self.config.schema_name),
            "data": {
                "status": "APPROVED",
                "approved_at": "NOW()",
                "approved_by": verb_form.pairs.get(&crate::Key::new("approved-by"))
            },
            "where": {"case_id": verb_form.pairs.get(&crate::Key::new("case-id"))}
        }))
    }

    async fn compile_entity_register(
        &self,
        verb_form: &VerbForm,
        _context: &ExecutionContext,
    ) -> DslManagerResult<serde_json::Value> {
        Ok(serde_json::json!({
            "type": "sql_insert",
            "table": format!("\"{}\".entities", self.config.schema_name),
            "data": verb_form.pairs,
            "returning": ["entity_id", "created_at"]
        }))
    }

    async fn compile_entity_link(
        &self,
        verb_form: &VerbForm,
        _context: &ExecutionContext,
    ) -> DslManagerResult<serde_json::Value> {
        Ok(serde_json::json!({
            "type": "sql_insert",
            "table": format!("\"{}\".entity_relationships", self.config.schema_name),
            "data": verb_form.pairs,
            "returning": ["relationship_id", "created_at"]
        }))
    }

    async fn compile_document_catalog(
        &self,
        verb_form: &VerbForm,
        _context: &ExecutionContext,
    ) -> DslManagerResult<serde_json::Value> {
        Ok(serde_json::json!({
            "type": "sql_insert",
            "table": format!("\"{}\".document_catalog", self.config.schema_name),
            "data": verb_form.pairs,
            "returning": ["document_id", "cataloged_at"]
        }))
    }

    async fn compile_document_use(
        &self,
        verb_form: &VerbForm,
        _context: &ExecutionContext,
    ) -> DslManagerResult<serde_json::Value> {
        Ok(serde_json::json!({
            "type": "sql_insert",
            "table": format!("\"{}\".document_usage", self.config.schema_name),
            "data": verb_form.pairs,
            "returning": ["usage_id", "used_at"]
        }))
    }

    async fn compile_workflow_transition(
        &self,
        verb_form: &VerbForm,
        _context: &ExecutionContext,
    ) -> DslManagerResult<serde_json::Value> {
        Ok(serde_json::json!({
            "type": "sql_insert",
            "table": format!("\"{}\".workflow_transitions", self.config.schema_name),
            "data": verb_form.pairs,
            "returning": ["transition_id", "transitioned_at"]
        }))
    }

    async fn compile_data_create(
        &self,
        verb_form: &VerbForm,
        _context: &ExecutionContext,
    ) -> DslManagerResult<serde_json::Value> {
        let asset = verb_form
            .pairs
            .get(&crate::Key::new("asset"))
            .and_then(|v| {
                if let crate::Value::String(s) = v {
                    Some(s.clone())
                } else {
                    None
                }
            })
            .unwrap_or_default();

        Ok(serde_json::json!({
            "type": "crud_create",
            "asset": asset,
            "data": verb_form.pairs.get(&crate::Key::new("values")),
            "table": format!("\"{}\".{}", self.config.schema_name, asset)
        }))
    }

    async fn compile_data_read(
        &self,
        verb_form: &VerbForm,
        _context: &ExecutionContext,
    ) -> DslManagerResult<serde_json::Value> {
        let asset = verb_form
            .pairs
            .get(&crate::Key::new("asset"))
            .and_then(|v| {
                if let crate::Value::String(s) = v {
                    Some(s.clone())
                } else {
                    None
                }
            })
            .unwrap_or_default();

        Ok(serde_json::json!({
            "type": "crud_read",
            "asset": asset,
            "where": verb_form.pairs.get(&crate::Key::new("where")),
            "select": verb_form.pairs.get(&crate::Key::new("select")),
            "limit": verb_form.pairs.get(&crate::Key::new("limit")),
            "table": format!("\"{}\".{}", self.config.schema_name, asset)
        }))
    }

    async fn compile_data_update(
        &self,
        verb_form: &VerbForm,
        _context: &ExecutionContext,
    ) -> DslManagerResult<serde_json::Value> {
        let asset = verb_form
            .pairs
            .get(&crate::Key::new("asset"))
            .and_then(|v| {
                if let crate::Value::String(s) = v {
                    Some(s.clone())
                } else {
                    None
                }
            })
            .unwrap_or_default();

        Ok(serde_json::json!({
            "type": "crud_update",
            "asset": asset,
            "where": verb_form.pairs.get(&crate::Key::new("where")),
            "values": verb_form.pairs.get(&crate::Key::new("values")),
            "table": format!("\"{}\".{}", self.config.schema_name, asset)
        }))
    }

    async fn compile_data_delete(
        &self,
        verb_form: &VerbForm,
        _context: &ExecutionContext,
    ) -> DslManagerResult<serde_json::Value> {
        let asset = verb_form
            .pairs
            .get(&crate::Key::new("asset"))
            .and_then(|v| {
                if let crate::Value::String(s) = v {
                    Some(s.clone())
                } else {
                    None
                }
            })
            .unwrap_or_default();

        Ok(serde_json::json!({
            "type": "crud_delete",
            "asset": asset,
            "where": verb_form.pairs.get(&crate::Key::new("where")),
            "table": format!("\"{}\".{}", self.config.schema_name, asset)
        }))
    }

    fn is_database_compatible_verb(&self, verb: &str) -> bool {
        matches!(
            verb,
            "case.create"
                | "case.update"
                | "case.approve"
                | "case.close"
                | "entity.register"
                | "entity.link"
                | "entity.classify"
                | "document.catalog"
                | "document.use"
                | "document.verify"
                | "workflow.transition"
                | "data.create"
                | "data.read"
                | "data.update"
                | "data.delete"
                | "data.query"
                | "data.batch"
        )
    }
}

/// CRUD compilation strategy for agentic operations
pub struct CrudCompilationStrategy;

impl CrudCompilationStrategy {
    pub fn new() -> Self {
        Self
    }
}

#[async_trait]
impl CompilationStrategy for CrudCompilationStrategy {
    fn name(&self) -> &str {
        "crud"
    }

    async fn compile_program(
        &self,
        program: &Program,
        context: &ExecutionContext,
    ) -> DslManagerResult<Vec<serde_json::Value>> {
        let mut operations = Vec::new();

        for form in program {
            if let Form::Verb(verb_form) = form {
                if verb_form.verb.starts_with("data.") {
                    let operation = serde_json::json!({
                        "type": "agentic_crud",
                        "verb": verb_form.verb,
                        "parameters": verb_form.pairs,
                        "execution_context": {
                            "user_id": context.user_id,
                            "session_id": context.session_id,
                            "mode": context.execution_mode
                        }
                    });
                    operations.push(operation);
                }
            }
        }

        Ok(operations)
    }

    async fn validate_program(
        &self,
        program: &Program,
        _context: &ExecutionContext,
    ) -> DslManagerResult<bool> {
        // Validate at least one CRUD operation exists
        for form in program {
            if let Form::Verb(verb_form) = form {
                if verb_form.verb.starts_with("data.") {
                    return Ok(true);
                }
            }
        }
        Ok(false)
    }
}

/// Mock compilation strategy for testing
pub struct MockCompilationStrategy;

impl MockCompilationStrategy {
    pub fn new() -> Self {
        Self
    }
}

#[async_trait]
impl CompilationStrategy for MockCompilationStrategy {
    fn name(&self) -> &str {
        "mock"
    }

    async fn compile_program(
        &self,
        program: &Program,
        _context: &ExecutionContext,
    ) -> DslManagerResult<Vec<serde_json::Value>> {
        let operations: Vec<_> = program
            .iter()
            .enumerate()
            .map(|(index, form)| {
                serde_json::json!({
                    "mock_operation": index,
                    "form_type": match form {
                        Form::Verb(v) => format!("verb:{}", v.verb),
                        Form::Comment(c) => format!("comment:{}", c),
                    },
                    "timestamp": chrono::Utc::now().to_rfc3339()
                })
            })
            .collect();

        Ok(operations)
    }

    async fn validate_program(
        &self,
        _program: &Program,
        _context: &ExecutionContext,
    ) -> DslManagerResult<bool> {
        Ok(true) // Mock always validates successfully
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::parser::idiomatic_parser::parse_program;

    #[tokio::test]
    async fn test_compiler_creation() {
        let compiler = DslCompiler::new();
        assert_eq!(compiler.supported_targets().len(), 3);
        assert!(compiler
            .supported_targets()
            .contains(&"database".to_string()));
        assert!(compiler.supported_targets().contains(&"mock".to_string()));
        assert!(compiler.supported_targets().contains(&"crud".to_string()));
    }

    #[tokio::test]
    async fn test_database_compilation() {
        let compiler = DslCompiler::new();
        let dsl = "(case.create :case-id \"test-001\" :case-type \"KYC_CASE\")";
        let program = parse_program(dsl).unwrap();

        let context = ExecutionContext {
            target_environment: "database".to_string(),
            execution_mode: "test".to_string(),
            user_id: "test-user".to_string(),
            ..Default::default()
        };

        let result = compiler.compile(program, context).await.unwrap();
        assert!(result.success);
        assert!(result.executable_operations.is_some());
        assert_eq!(result.executable_operations.unwrap().len(), 1);
    }

    #[tokio::test]
    async fn test_crud_compilation() {
        let compiler = DslCompiler::new();
        let dsl = r#"(data.create :asset "cbu" :values {:name "Test CBU"})"#;
        let program = parse_program(dsl).unwrap();

        let context = ExecutionContext {
            target_environment: "crud".to_string(),
            execution_mode: "test".to_string(),
            user_id: "test-user".to_string(),
            ..Default::default()
        };

        let result = compiler.compile(program, context).await.unwrap();
        assert!(result.success);
        assert!(result.executable_operations.is_some());
    }

    #[tokio::test]
    async fn test_mock_compilation() {
        let compiler = DslCompiler::new();
        let dsl = "(test.verb :param \"value\")";
        let program = parse_program(dsl).unwrap();

        let context = ExecutionContext {
            target_environment: "mock".to_string(),
            execution_mode: "test".to_string(),
            user_id: "test-user".to_string(),
            ..Default::default()
        };

        let result = compiler.compile(program, context).await.unwrap();
        assert!(result.success);
        assert!(result.executable_operations.is_some());
    }

    #[tokio::test]
    async fn test_unsupported_target() {
        let compiler = DslCompiler::new();
        let dsl = "(test.verb)";
        let program = parse_program(dsl).unwrap();

        let context = ExecutionContext {
            target_environment: "unsupported".to_string(),
            ..Default::default()
        };

        let result = compiler.compile(program, context).await;
        assert!(result.is_err());
    }
}
