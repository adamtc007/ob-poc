//! LLM DSL Generator
//!
//! Uses multi-provider LLM support with RAG context to generate valid DSL.
//! Vocabulary context comes from the in-memory Runtime (vocab_registry.rs).

use anyhow::{Context, Result};
use serde::{Deserialize, Serialize};
use std::sync::Arc;

use super::providers::MultiProviderLlm;
use super::rag_context::{RagContext, RagContextProvider};
use crate::forth_engine::runtime::Runtime;

#[derive(Clone)]
pub struct LlmDslGenerator {
    llm_client: Arc<MultiProviderLlm>,
    rag_provider: Arc<RagContextProvider>,
    runtime: Arc<Runtime>,
    max_retries: usize,
}

#[derive(Debug, Serialize, Deserialize)]
pub struct GeneratedDsl {
    pub dsl_text: String,
    pub confidence: f64,
    pub reasoning: String,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct GeneratorConfig {
    pub model: String,
    pub max_tokens: usize,
    pub temperature: f32,
    pub max_retries: usize,
}

impl Default for GeneratorConfig {
    fn default() -> Self {
        Self {
            model: "claude-sonnet-4-20250514".to_string(),
            max_tokens: 4000,
            temperature: 0.1,
            max_retries: 3,
        }
    }
}

impl LlmDslGenerator {
    /// Create from environment variables with Runtime (preferred)
    pub fn from_env_with_runtime(
        rag_provider: Arc<RagContextProvider>,
        runtime: Arc<Runtime>,
    ) -> Result<Self> {
        let llm_client = MultiProviderLlm::from_env()?;
        Ok(Self {
            llm_client: Arc::new(llm_client),
            rag_provider,
            runtime,
            max_retries: 3,
        })
    }

    /// Create from environment variables (uses MultiProviderLlm)
    /// Creates its own Runtime internally
    pub fn from_env(rag_provider: Arc<RagContextProvider>) -> Result<Self> {
        use crate::forth_engine::vocab_registry::create_standard_runtime;
        let llm_client = MultiProviderLlm::from_env()?;
        Ok(Self {
            llm_client: Arc::new(llm_client),
            rag_provider,
            runtime: Arc::new(create_standard_runtime()),
            max_retries: 3,
        })
    }

    /// Create with explicit LLM client and Runtime
    pub fn with_client(
        llm_client: Arc<MultiProviderLlm>,
        rag_provider: Arc<RagContextProvider>,
        runtime: Arc<Runtime>,
    ) -> Self {
        Self {
            llm_client,
            rag_provider,
            runtime,
            max_retries: 3,
        }
    }

    /// Create with Anthropic API key (legacy compatibility)
    pub fn new(anthropic_api_key: String, rag_provider: Arc<RagContextProvider>) -> Self {
        use super::providers::{LlmProvider, ProviderConfig};
        use crate::forth_engine::vocab_registry::create_standard_runtime;

        let config = ProviderConfig {
            provider: LlmProvider::Anthropic,
            api_key: anthropic_api_key,
            model: "claude-sonnet-4-20250514".to_string(),
            base_url: "https://api.anthropic.com/v1".to_string(),
            max_tokens: 1024,
            temperature: 0.1,
            timeout_seconds: 30,
        };

        let llm_client =
            MultiProviderLlm::new(vec![config], 5).expect("Failed to create LLM client");

        Self {
            llm_client: Arc::new(llm_client),
            rag_provider,
            runtime: Arc::new(create_standard_runtime()),
            max_retries: 3,
        }
    }

    /// Create with config (legacy compatibility)
    pub fn with_config(
        anthropic_api_key: String,
        rag_provider: Arc<RagContextProvider>,
        config: GeneratorConfig,
    ) -> Self {
        use super::providers::{LlmProvider, ProviderConfig};
        use crate::forth_engine::vocab_registry::create_standard_runtime;

        let provider_config = ProviderConfig {
            provider: LlmProvider::Anthropic,
            api_key: anthropic_api_key,
            model: config.model,
            base_url: "https://api.anthropic.com/v1".to_string(),
            max_tokens: config.max_tokens as u32,
            temperature: config.temperature,
            timeout_seconds: 30,
        };

        let llm_client =
            MultiProviderLlm::new(vec![provider_config], 5).expect("Failed to create LLM client");

        Self {
            llm_client: Arc::new(llm_client),
            rag_provider,
            runtime: Arc::new(create_standard_runtime()),
            max_retries: config.max_retries,
        }
    }

    /// Get a reference to the Runtime
    pub fn runtime(&self) -> &Runtime {
        &self.runtime
    }

    /// Generate DSL from natural language instruction
    pub async fn generate(
        &self,
        instruction: &str,
        operation_type: &str,
        domain: Option<&str>,
    ) -> Result<GeneratedDsl> {
        // Step 1: Get RAG context using Runtime vocabulary (preferred method)
        let context = self
            .rag_provider
            .get_context_with_runtime(&self.runtime, operation_type, instruction, domain)
            .context("Failed to retrieve RAG context")?;

        // Step 2: Build prompts
        let system_prompt = self.build_system_prompt(&context);
        let mut user_prompt = format!("Generate DSL for: {}", instruction);

        // Step 3: Retry loop
        for attempt in 0..self.max_retries {
            let response = self
                .llm_client
                .generate(&system_prompt, &user_prompt)
                .await
                .context("Failed to call LLM")?;

            // Step 4: Parse response
            match self.parse_response(&response.content) {
                Ok(generated) => {
                    // Basic validation - check it starts with (
                    if generated.dsl_text.trim().starts_with('(') {
                        return Ok(generated);
                    } else if attempt < self.max_retries - 1 {
                        user_prompt = format!(
                            "{}\n\nPREVIOUS ATTEMPT FAILED:\nGenerated: {}\nError: DSL must start with '('\nPlease fix and regenerate.",
                            user_prompt, generated.dsl_text
                        );
                        continue;
                    }
                }
                Err(e) if attempt < self.max_retries - 1 => {
                    user_prompt = format!(
                        "{}\n\nPREVIOUS ATTEMPT FAILED:\nError: {}\nPlease regenerate valid DSL.",
                        user_prompt, e
                    );
                    continue;
                }
                Err(e) => return Err(e),
            }
        }

        anyhow::bail!(
            "Failed to generate valid DSL after {} attempts",
            self.max_retries
        )
    }

    fn build_system_prompt(&self, context: &RagContext) -> String {
        let vocab_list = context
            .vocabulary
            .iter()
            .map(|v| format!("  - {} {}", v.verb_name, v.signature))
            .collect::<Vec<_>>()
            .join("\n");

        let examples_list = context
            .examples
            .iter()
            .take(3)
            .map(|e| format!("  {}", e.dsl_text))
            .collect::<Vec<_>>()
            .join("\n");

        let attributes_list = context
            .attributes
            .iter()
            .take(10)
            .map(|a| {
                format!(
                    "  @attr(\"{}\") - {} ({})",
                    a.semantic_id, a.name, a.data_type
                )
            })
            .collect::<Vec<_>>()
            .join("\n");

        let grammar_list = context.grammar_hints.join("\n");

        let constraints_list = context.constraints.join("\n  - ");

        format!(
            r#"You are an expert DSL generator for the ob-poc financial onboarding system.
Your task is to generate VALID s-expression DSL code that will be parsed by a strict Forth-based parser.

CRITICAL RULES:
1. Output MUST be valid s-expression with balanced parentheses
2. Use ONLY verbs from the vocabulary list below
3. Follow the EBNF grammar exactly
4. Attribute references use @attr("SEMANTIC.ID") format
5. All keywords use :keyword format
6. Strings must be double-quoted

EBNF GRAMMAR:
{}

VALID VOCABULARY (USE ONLY THESE):
{}

AVAILABLE ATTRIBUTES:
{}

EXAMPLE DSL:
{}

CONSTRAINTS:
  - {}

OUTPUT FORMAT:
Respond with JSON:
{{
  "dsl_text": "(your-dsl-here)",
  "confidence": 0.95,
  "reasoning": "Brief explanation"
}}

IMPORTANT: The DSL must be EXECUTABLE. Invalid syntax will cause system failure."#,
            grammar_list,
            if vocab_list.is_empty() {
                "  (no specific vocabulary loaded)"
            } else {
                &vocab_list
            },
            if attributes_list.is_empty() {
                "  (no specific attributes loaded)"
            } else {
                &attributes_list
            },
            if examples_list.is_empty() {
                "  (no examples available)"
            } else {
                &examples_list
            },
            if constraints_list.is_empty() {
                "None"
            } else {
                &constraints_list
            }
        )
    }

    fn parse_response(&self, response: &str) -> Result<GeneratedDsl> {
        Self::parse_response_text(response)
    }

    /// Static helper to parse response without requiring an LlmDslGenerator instance
    pub fn parse_response_text(response: &str) -> Result<GeneratedDsl> {
        // Try to parse as JSON first
        if let Ok(generated) = serde_json::from_str::<GeneratedDsl>(response) {
            return Ok(generated);
        }

        // Try to find JSON in the response
        if let Some(start) = response.find('{') {
            if let Some(end) = response.rfind('}') {
                let json_str = &response[start..=end];
                if let Ok(generated) = serde_json::from_str::<GeneratedDsl>(json_str) {
                    return Ok(generated);
                }
            }
        }

        // Fallback: extract DSL from text
        if let Some(start) = response.find('(') {
            if let Some(end) = response.rfind(')') {
                let dsl_text = response[start..=end].to_string();
                return Ok(GeneratedDsl {
                    dsl_text,
                    confidence: 0.7,
                    reasoning: "Extracted from non-JSON response".to_string(),
                });
            }
        }

        anyhow::bail!("Could not parse DSL from response: {}", response)
    }
}

#[cfg(all(test, feature = "database"))]
mod tests {
    use super::*;

    #[tokio::test]
    #[ignore] // Requires API key and database
    async fn test_llm_generation() {
        let database_url = std::env::var("DATABASE_URL")
            .unwrap_or_else(|_| "postgresql://localhost/data_designer".to_string());

        let pool = sqlx::PgPool::connect(&database_url).await.unwrap();
        let rag_provider = Arc::new(RagContextProvider::new(pool));

        let generator = LlmDslGenerator::from_env(rag_provider).unwrap();

        let result = generator
            .generate(
                "Create a CBU for TechCorp Ltd with banking services",
                "CREATE",
                Some("cbu"),
            )
            .await
            .unwrap();

        println!("Generated DSL: {}", result.dsl_text);
        assert!(result.dsl_text.starts_with('('));
        assert!(result.confidence > 0.5);
    }

    #[test]
    fn test_parse_json_response() {
        let response = r#"{"dsl_text": "(cbu.create :name \"Test\")", "confidence": 0.9, "reasoning": "test"}"#;
        let result = LlmDslGenerator::parse_response_text(response).unwrap();

        assert_eq!(result.dsl_text, "(cbu.create :name \"Test\")");
        assert_eq!(result.confidence, 0.9);
    }

    #[test]
    fn test_parse_embedded_json() {
        let response = r#"Here is the DSL: {"dsl_text": "(cbu.create :name \"Test\")", "confidence": 0.9, "reasoning": "test"} Done."#;
        let result = LlmDslGenerator::parse_response_text(response).unwrap();

        assert_eq!(result.dsl_text, "(cbu.create :name \"Test\")");
    }

    #[test]
    fn test_parse_fallback_extraction() {
        let response = "Here is your DSL: (cbu.create :name \"Test\") I hope this works.";
        let result = LlmDslGenerator::parse_response_text(response).unwrap();

        assert_eq!(result.dsl_text, "(cbu.create :name \"Test\")");
        assert_eq!(result.confidence, 0.7);
    }
}
