//! Agent Orchestrator
//!
//! Main entry point for agentic DSL generation.
//! Coordinates intent extraction, planning, generation, and validation.

use anyhow::{anyhow, Result};
use serde::{Deserialize, Serialize};

use crate::agentic::feedback::{FeedbackLoop, ValidatedDsl};
use crate::agentic::generator::IntentExtractor;
use crate::agentic::intent::OnboardingIntent;
use crate::agentic::planner::{OnboardingPlan, RequirementPlanner};

/// Agent orchestrator for DSL generation
pub struct AgentOrchestrator {
    intent_extractor: IntentExtractor,
    feedback_loop: FeedbackLoop,
    #[cfg(feature = "database")]
    executor: Option<crate::dsl_v2::DslExecutor>,
}

/// Result of DSL generation
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct GenerationResult {
    pub intent: OnboardingIntent,
    pub plan: OnboardingPlan,
    pub dsl: ValidatedDsl,
    pub execution: Option<ExecutionResult>,
}

/// Result of DSL execution
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ExecutionResult {
    pub success: bool,
    pub bindings: Vec<BindingEntry>,
    pub error: Option<String>,
}

/// Variable binding from execution
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct BindingEntry {
    pub variable: String,
    pub uuid: String,
}

impl AgentOrchestrator {
    /// Create a new orchestrator without database support
    pub fn new(api_key: String) -> Result<Self> {
        Ok(Self {
            intent_extractor: IntentExtractor::new(api_key.clone()),
            feedback_loop: FeedbackLoop::new(api_key, 3)?,
            #[cfg(feature = "database")]
            executor: None,
        })
    }

    /// Create a new orchestrator with database support
    #[cfg(feature = "database")]
    pub fn with_executor(api_key: String, pool: sqlx::PgPool) -> Result<Self> {
        Ok(Self {
            intent_extractor: IntentExtractor::new(api_key.clone()),
            feedback_loop: FeedbackLoop::new(api_key, 3)?,
            executor: Some(crate::dsl_v2::DslExecutor::new(pool)),
        })
    }

    /// Generate DSL from natural language
    pub async fn generate(&self, request: &str, execute: bool) -> Result<GenerationResult> {
        // Phase 1: Extract intent
        let intent = self.intent_extractor.extract(request).await?;

        // Phase 2: Plan (deterministic)
        let plan = RequirementPlanner::plan(&intent);

        // Phase 3-4: Generate and validate DSL (with retry)
        let dsl = self.feedback_loop.generate_valid_dsl(&plan).await?;

        // Phase 5: Execute if requested
        let execution = if execute {
            #[cfg(feature = "database")]
            {
                if let Some(ref executor) = self.executor {
                    Some(self.execute_dsl(executor, &dsl.source).await?)
                } else {
                    return Err(anyhow!("Execution requested but no database connection"));
                }
            }
            #[cfg(not(feature = "database"))]
            {
                return Err(anyhow!(
                    "Execution requested but database feature not enabled"
                ));
            }
        } else {
            None
        };

        Ok(GenerationResult {
            intent,
            plan,
            dsl,
            execution,
        })
    }

    /// Extract intent only (for debugging/inspection)
    pub async fn extract_intent(&self, request: &str) -> Result<OnboardingIntent> {
        self.intent_extractor.extract(request).await
    }

    /// Plan from intent (for debugging/inspection)
    pub fn plan_from_intent(&self, intent: &OnboardingIntent) -> OnboardingPlan {
        RequirementPlanner::plan(intent)
    }

    /// Validate DSL (without generation)
    pub fn validate(&self, dsl: &str) -> crate::agentic::validator::ValidationResult {
        self.feedback_loop.validate(dsl)
    }

    #[cfg(feature = "database")]
    async fn execute_dsl(
        &self,
        executor: &crate::dsl_v2::DslExecutor,
        source: &str,
    ) -> Result<ExecutionResult> {
        use crate::dsl_v2::{compile, parse_program, ExecutionContext};

        // Parse and compile
        let program = parse_program(source).map_err(|e| anyhow!("Parse error: {}", e))?;
        let plan = compile(&program).map_err(|e| anyhow!("Compile error: {}", e))?;

        // Execute
        let mut ctx = ExecutionContext::new();
        match executor.execute_plan(&plan, &mut ctx).await {
            Ok(_results) => {
                let bindings: Vec<BindingEntry> = ctx
                    .symbols
                    .iter()
                    .map(|(k, v)| BindingEntry {
                        variable: k.clone(),
                        uuid: v.to_string(),
                    })
                    .collect();

                Ok(ExecutionResult {
                    success: true,
                    bindings,
                    error: None,
                })
            }
            Err(e) => Ok(ExecutionResult {
                success: false,
                bindings: vec![],
                error: Some(e.to_string()),
            }),
        }
    }
}

/// Builder for AgentOrchestrator
pub struct OrchestratorBuilder {
    api_key: String,
    max_retries: usize,
    #[cfg(feature = "database")]
    pool: Option<sqlx::PgPool>,
}

impl OrchestratorBuilder {
    /// Create a new builder
    pub fn new(api_key: String) -> Self {
        Self {
            api_key,
            max_retries: 3,
            #[cfg(feature = "database")]
            pool: None,
        }
    }

    /// Set max retries for DSL generation
    pub fn max_retries(mut self, retries: usize) -> Self {
        self.max_retries = retries;
        self
    }

    /// Set database pool for execution
    #[cfg(feature = "database")]
    pub fn with_pool(mut self, pool: sqlx::PgPool) -> Self {
        self.pool = Some(pool);
        self
    }

    /// Build the orchestrator
    pub fn build(self) -> Result<AgentOrchestrator> {
        Ok(AgentOrchestrator {
            intent_extractor: IntentExtractor::new(self.api_key.clone()),
            feedback_loop: FeedbackLoop::new(self.api_key, self.max_retries)?,
            #[cfg(feature = "database")]
            executor: self.pool.map(crate::dsl_v2::DslExecutor::new),
        })
    }
}
