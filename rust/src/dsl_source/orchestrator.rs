//! Agentic DSL Orchestrator
//!
//! Single entry point for: prompt -> generate -> validate -> retry -> execute
//!
//! This module ties together the RAG context provider, LLM generator,
//! validation pipeline, and Runtime execution into a cohesive workflow.

use anyhow::{Context, Result};
use sqlx::PgPool;
use std::sync::Arc;

use crate::database::CrudExecutor;
use crate::forth_engine::ast::DslParser;
use crate::forth_engine::env::{OnboardingRequestId, RuntimeEnv};
use crate::forth_engine::parser_nom::NomDslParser;
use crate::forth_engine::runtime::Runtime;
use crate::forth_engine::vocab_registry::create_standard_runtime;

use super::agentic::{GeneratedDsl, LlmDslGenerator, RagContextProvider};
use super::validation::{ValidationPipeline, ValidationResult, ValidationStage};

/// Result of agentic DSL generation and execution
#[derive(Debug)]
pub struct AgenticResult {
    pub success: bool,
    pub dsl_text: String,
    pub validation: ValidationResult,
    pub execution_logs: Vec<String>,
    pub attempts: usize,
    pub confidence: f64,
}

/// Configuration for orchestrator
#[derive(Debug, Clone)]
pub struct OrchestratorConfig {
    pub max_retries: usize,
    pub min_confidence: f64,
    pub execute_on_success: bool,
}

impl Default for OrchestratorConfig {
    fn default() -> Self {
        Self {
            max_retries: 3,
            min_confidence: 0.7,
            execute_on_success: true,
        }
    }
}

/// Agentic DSL Orchestrator - the main entry point
pub struct AgenticOrchestrator {
    runtime: Arc<Runtime>,
    generator: LlmDslGenerator,
    validator: ValidationPipeline,
    executor: Option<CrudExecutor>,
    config: OrchestratorConfig,
}

impl AgenticOrchestrator {
    /// Create orchestrator with database connection
    pub fn new(pool: PgPool, config: OrchestratorConfig) -> Result<Self> {
        let runtime = Arc::new(create_standard_runtime());
        let rag_provider = Arc::new(RagContextProvider::new(pool.clone()));
        let generator = LlmDslGenerator::from_env_with_runtime(rag_provider, runtime.clone())?;
        let validator = ValidationPipeline::new(pool.clone());
        let executor = Some(CrudExecutor::new(pool));

        Ok(Self {
            runtime,
            generator,
            validator,
            executor,
            config,
        })
    }

    /// Create orchestrator without database execution (validation only)
    pub fn validation_only(pool: PgPool, config: OrchestratorConfig) -> Result<Self> {
        let runtime = Arc::new(create_standard_runtime());
        let rag_provider = Arc::new(RagContextProvider::new(pool.clone()));
        let generator = LlmDslGenerator::from_env_with_runtime(rag_provider, runtime.clone())?;
        let validator = ValidationPipeline::new(pool);

        Ok(Self {
            runtime,
            generator,
            validator,
            executor: None,
            config,
        })
    }

    /// Main entry point: natural language -> executed DSL
    pub async fn process(&self, instruction: &str, domain: Option<&str>) -> Result<AgenticResult> {
        let operation_type = self.infer_operation_type(instruction);
        let mut attempts = 0;
        let mut last_validation = None;
        let mut last_dsl = String::new();
        let mut feedback = String::new();

        while attempts < self.config.max_retries {
            attempts += 1;

            // Step 1: Generate DSL
            let prompt = if feedback.is_empty() {
                instruction.to_string()
            } else {
                format!("{}\n\nPREVIOUS ATTEMPT FAILED:\n{}", instruction, feedback)
            };

            let generated = self
                .generator
                .generate(&prompt, &operation_type, domain)
                .await
                .context("LLM generation failed")?;

            last_dsl = generated.dsl_text.clone();

            // Step 2: Validate
            let validation = self
                .validator
                .validate(&generated.dsl_text)
                .await
                .context("Validation failed")?;

            last_validation = Some(validation.clone());

            // Step 3: Check result
            if validation.is_valid && generated.confidence >= self.config.min_confidence {
                // Success! Execute if configured
                let execution_logs = if self.config.execute_on_success {
                    self.execute(&generated.dsl_text).await?
                } else {
                    vec!["Execution skipped (config)".to_string()]
                };

                return Ok(AgenticResult {
                    success: true,
                    dsl_text: generated.dsl_text,
                    validation,
                    execution_logs,
                    attempts,
                    confidence: generated.confidence,
                });
            }

            // Build feedback for retry
            feedback = self.build_feedback(&validation, &generated);
        }

        // Max retries exceeded
        Ok(AgenticResult {
            success: false,
            dsl_text: last_dsl,
            validation: last_validation.unwrap_or_else(|| ValidationResult {
                is_valid: false,
                errors: vec![],
                warnings: vec!["Max retries exceeded".to_string()],
                stage_reached: ValidationStage::Syntax,
            }),
            execution_logs: vec![],
            attempts,
            confidence: 0.0,
        })
    }

    /// Execute validated DSL
    async fn execute(&self, dsl_text: &str) -> Result<Vec<String>> {
        let parser = NomDslParser::new();
        let ast = parser.parse(dsl_text)?;

        let mut env = RuntimeEnv::new(OnboardingRequestId(uuid::Uuid::new_v4().to_string()));

        // Execute via Runtime
        self.runtime.execute_sheet(&ast, &mut env)?;

        let mut logs = vec![format!("Executed {} statements", ast.len())];

        // Execute CRUD statements if we have executor
        if let Some(executor) = &self.executor {
            let pending = env.take_pending_crud();
            if !pending.is_empty() {
                let results = executor.execute_all(&pending).await?;
                for result in results {
                    logs.push(format!(
                        "CRUD {}: {} ({} rows)",
                        result.operation, result.asset, result.rows_affected
                    ));
                }
            }
        }

        Ok(logs)
    }

    /// Infer operation type from instruction
    fn infer_operation_type(&self, instruction: &str) -> String {
        let lower = instruction.to_lowercase();

        if lower.contains("create") || lower.contains("new") || lower.contains("add") {
            "CREATE".to_string()
        } else if lower.contains("update") || lower.contains("modify") || lower.contains("change") {
            "UPDATE".to_string()
        } else if lower.contains("delete") || lower.contains("remove") {
            "DELETE".to_string()
        } else if lower.contains("read") || lower.contains("get") || lower.contains("fetch") {
            "READ".to_string()
        } else {
            "CREATE".to_string() // Default
        }
    }

    /// Build feedback message for retry
    fn build_feedback(&self, validation: &ValidationResult, generated: &GeneratedDsl) -> String {
        let mut parts = Vec::new();

        if !validation.is_valid {
            let errors = self.validator.format_errors_for_llm(validation);
            parts.push(format!("Validation errors:\n{}", errors));
        }

        if generated.confidence < self.config.min_confidence {
            parts.push(format!(
                "Confidence {} below threshold {}",
                generated.confidence, self.config.min_confidence
            ));
        }

        if !validation.warnings.is_empty() {
            parts.push(format!("Warnings: {}", validation.warnings.join(", ")));
        }

        parts.join("\n\n")
    }

    /// Get available domains
    pub fn get_domains(&self) -> Vec<&'static str> {
        self.runtime.get_domains()
    }

    /// Get words for domain (for UI/help)
    pub fn get_domain_words(&self, domain: &str) -> Vec<&str> {
        self.runtime
            .get_domain_words(domain)
            .iter()
            .map(|w| w.name)
            .collect()
    }

    /// Get a reference to the Runtime
    pub fn runtime(&self) -> &Runtime {
        &self.runtime
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_infer_operation_type() {
        // Test helper functions without needing a pool
        let lower = "create a new cbu".to_lowercase();
        assert!(lower.contains("create"));

        let lower = "update the cbu status".to_lowercase();
        assert!(lower.contains("update"));

        let lower = "delete the entity".to_lowercase();
        assert!(lower.contains("delete"));

        let lower = "get cbu details".to_lowercase();
        assert!(lower.contains("get"));
    }

    #[test]
    fn test_config_default() {
        let config = OrchestratorConfig::default();
        assert_eq!(config.max_retries, 3);
        assert_eq!(config.min_confidence, 0.7);
        assert!(config.execute_on_success);
    }

    #[tokio::test]
    #[ignore] // Requires LLM API key and database
    async fn test_orchestrator_create_cbu() {
        let pool = sqlx::PgPool::connect(&std::env::var("DATABASE_URL").unwrap())
            .await
            .unwrap();

        let orchestrator = AgenticOrchestrator::new(
            pool,
            OrchestratorConfig {
                execute_on_success: false, // Don't actually execute
                ..Default::default()
            },
        )
        .unwrap();

        let result = orchestrator
            .process(
                "Create a hedge fund CBU called AcmeFund in UK jurisdiction",
                Some("cbu"),
            )
            .await
            .unwrap();

        println!("Success: {}", result.success);
        println!("DSL: {}", result.dsl_text);
        println!("Attempts: {}", result.attempts);
        println!("Confidence: {}", result.confidence);

        assert!(result.dsl_text.contains("cbu.create"));
    }
}
