//! Feedback Loop
//!
//! Retry loop for DSL generation with error correction.

use anyhow::{anyhow, Result};
use serde::{Deserialize, Serialize};

use crate::agentic::generator::DslGenerator;
use crate::agentic::planner::OnboardingPlan;
use crate::agentic::validator::{AgentValidator, ValidationResult};

/// Validated DSL with attempt count
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ValidatedDsl {
    pub source: String,
    pub attempts: usize,
    pub validation: ValidationResult,
}

/// Feedback loop for DSL generation with retry
pub struct FeedbackLoop {
    generator: DslGenerator,
    validator: AgentValidator,
    max_retries: usize,
}

impl FeedbackLoop {
    /// Create a new feedback loop with explicit API key
    pub fn new(api_key: String, max_retries: usize) -> Result<Self> {
        Ok(Self {
            generator: DslGenerator::new(api_key),
            validator: AgentValidator::new()?,
            max_retries,
        })
    }

    /// Create from environment variables
    pub fn from_env(max_retries: usize) -> Result<Self> {
        Ok(Self {
            generator: DslGenerator::from_env()?,
            validator: AgentValidator::new()?,
            max_retries,
        })
    }

    /// Create with an existing DslGenerator
    pub fn with_generator(generator: DslGenerator, max_retries: usize) -> Result<Self> {
        Ok(Self {
            generator,
            validator: AgentValidator::new()?,
            max_retries,
        })
    }

    /// Generate valid DSL with retry loop
    pub async fn generate_valid_dsl(&self, plan: &OnboardingPlan) -> Result<ValidatedDsl> {
        let mut attempts = 0;
        let mut current_dsl = self.generator.generate(plan).await?;

        loop {
            attempts += 1;
            let validation = self.validator.validate(&current_dsl);

            if validation.is_valid {
                return Ok(ValidatedDsl {
                    source: current_dsl,
                    attempts,
                    validation,
                });
            }

            if attempts >= self.max_retries {
                return Err(anyhow!(
                    "Failed to generate valid DSL after {} attempts.\nLast errors: {:?}\nLast DSL:\n{}",
                    attempts,
                    validation.errors,
                    current_dsl
                ));
            }

            // Collect error messages for feedback
            let errors: Vec<String> = validation
                .errors
                .iter()
                .map(|e| {
                    let ctx = e
                        .suggestion
                        .as_ref()
                        .map(|s| format!(" (hint: {})", s))
                        .unwrap_or_default();
                    match e.line {
                        Some(line) => format!("Line {}: {}{}", line, e.message, ctx),
                        None => format!("{}{}", e.message, ctx),
                    }
                })
                .collect();

            // Ask LLM to fix
            current_dsl = self
                .generator
                .generate_with_fix(plan, &current_dsl, &errors)
                .await?;
        }
    }

    /// Validate DSL without generation
    pub fn validate(&self, dsl: &str) -> ValidationResult {
        self.validator.validate(dsl)
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_validation_only() {
        // This test doesn't need API key since we're just validating
        let validator = AgentValidator::new().unwrap();

        let valid_dsl =
            r#"(cbu.ensure :name "Test" :jurisdiction "US" :client-type "FUND" :as @cbu)"#;
        let result = validator.validate(valid_dsl);
        assert!(result.is_valid);
    }
}
