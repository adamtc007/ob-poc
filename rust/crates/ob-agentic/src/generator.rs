//! DSL Generator
//!
//! Uses LLM API (Anthropic or OpenAI) to generate DSL from structured requirements.

use anyhow::{anyhow, Result};
use std::sync::Arc;

use crate::client_factory::{create_llm_client, create_llm_client_with_key};
use crate::llm_client::LlmClient;
use crate::patterns::OnboardingPattern;
use crate::planner::OnboardingPlan;

/// DSL generator using LLM API
pub struct DslGenerator {
    client: Arc<dyn LlmClient>,
}

impl DslGenerator {
    /// Create a new DSL generator with explicit API key
    pub fn new(api_key: String) -> Self {
        let client = create_llm_client_with_key(api_key).expect("Failed to create LLM client");
        Self { client }
    }

    /// Create from environment variables
    pub fn from_env() -> Result<Self> {
        let client = create_llm_client()?;
        Ok(Self { client })
    }

    /// Create with a specific LLM client
    pub fn with_client(client: Arc<dyn LlmClient>) -> Self {
        Self { client }
    }

    /// Generate DSL from an onboarding plan
    pub async fn generate(&self, plan: &OnboardingPlan) -> Result<String> {
        let system_prompt = self.build_system_prompt(plan.pattern);
        let user_prompt = self.build_user_prompt(plan);

        let response = self.client.chat(&system_prompt, &user_prompt).await?;
        Ok(Self::strip_code_blocks(&response))
    }

    /// Generate DSL with error correction
    pub async fn generate_with_fix(
        &self,
        plan: &OnboardingPlan,
        previous_dsl: &str,
        errors: &[String],
    ) -> Result<String> {
        let error_text = errors.join("\n");

        let prompt = format!(
            r#"The following DSL has validation errors. Fix them and return ONLY the corrected DSL.

## Errors
{}

## Current DSL
```
{}
```

## Requirements (for reference)
{}

Return ONLY the corrected DSL code, no explanations."#,
            error_text,
            previous_dsl,
            self.build_user_prompt(plan)
        );

        let system_prompt = self.build_system_prompt(plan.pattern);
        let response = self.client.chat(&system_prompt, &prompt).await?;
        Ok(Self::strip_code_blocks(&response))
    }

    fn build_system_prompt(&self, pattern: OnboardingPattern) -> String {
        let verb_schemas = include_str!("schemas/custody_verbs.md");
        let reference_data = include_str!("schemas/reference_data.md");
        let example = pattern.example_dsl();

        format!(
            r#"# DSL Generation System

You are a DSL code generator for a custody onboarding system. Generate valid DSL code based on the structured requirements provided.

## DSL Syntax

S-expression format:
```
(domain.verb :arg1 value1 :arg2 value2 :as @variable)
```

- Keywords are prefixed with `:`
- Strings use double quotes: `"value"`
- UUIDs reference variables: `@variable` (use hyphens in variable names, e.g., @ssi-us)
- Lists use brackets: `["a" "b" "c"]`
- Comments start with `;`

## Available Verbs

{verb_schemas}

## Reference Data

{reference_data}

## Example ({pattern} pattern)

{example}

## Rules

1. Generate ONLY valid DSL code
2. Use `:as @variable` to capture results for later reference
3. Order statements so dependencies are defined before use
4. Include section comments for readability
5. Add validation at the end: `(cbu-custody.validate-booking-coverage :cbu-id @cbu)`
6. Use placeholder values for account numbers (e.g., "SAFE-001", "CASH-001")
7. Use today's date for effective-date: "2024-12-01"
8. Output ONLY the DSL code, no explanations
"#,
            verb_schemas = verb_schemas,
            reference_data = reference_data,
            example = example,
            pattern = pattern
        )
    }

    fn build_user_prompt(&self, plan: &OnboardingPlan) -> String {
        let mut prompt = String::new();

        prompt.push_str("Generate DSL for this custody onboarding:\n\n");

        // CBU
        prompt.push_str(&format!(
            "## CBU\n- Name: {}\n- Jurisdiction: {}\n- Type: {}\n- Variable: @{}\n\n",
            plan.cbu.name, plan.cbu.jurisdiction, plan.cbu.client_type, plan.cbu.variable
        ));

        // Entities
        if !plan.entities.is_empty() {
            prompt.push_str("## Entities (lookup existing)\n");
            for e in &plan.entities {
                prompt.push_str(&format!("- {} → @{}\n", e.name, e.variable));
            }
            prompt.push('\n');
        }

        // Universe
        prompt.push_str("## Universe Entries\n");
        for u in &plan.universe {
            let market = u.market.as_deref().unwrap_or("OTC");
            let cp = u
                .counterparty_var
                .as_ref()
                .map(|v| format!(" (counterparty: @{})", v))
                .unwrap_or_default();
            prompt.push_str(&format!(
                "- {} in {} with currencies {:?}{}\n",
                u.instrument_class, market, u.currencies, cp
            ));
        }
        prompt.push('\n');

        // SSIs
        prompt.push_str("## SSIs Required\n");
        for s in &plan.ssis {
            prompt.push_str(&format!(
                "- {} ({}, {}) → @{}\n",
                s.name, s.ssi_type, s.currency, s.variable
            ));
        }
        prompt.push('\n');

        // Booking Rules
        prompt.push_str("## Booking Rules\n");
        for r in &plan.booking_rules {
            let criteria: Vec<String> = [
                r.instrument_class.as_ref().map(|v| format!("class={}", v)),
                r.market.as_ref().map(|v| format!("market={}", v)),
                r.currency.as_ref().map(|v| format!("currency={}", v)),
                r.settlement_type
                    .as_ref()
                    .map(|v| format!("settlement={}", v)),
                r.counterparty_var
                    .as_ref()
                    .map(|v| format!("counterparty=@{}", v)),
            ]
            .into_iter()
            .flatten()
            .collect();

            let criteria_str = if criteria.is_empty() {
                "ANY".to_string()
            } else {
                criteria.join(", ")
            };
            prompt.push_str(&format!(
                "- {} (priority {}, {}) → @{}\n",
                r.name, r.priority, criteria_str, r.ssi_variable
            ));
        }
        prompt.push('\n');

        // ISDA
        if !plan.isdas.is_empty() {
            prompt.push_str("## ISDA Agreements\n");
            for isda in &plan.isdas {
                prompt.push_str(&format!(
                    "- With {} ({} law) → @{}\n",
                    isda.counterparty_name, isda.governing_law, isda.variable
                ));
                prompt.push_str(&format!("  Coverages: {:?}\n", isda.coverages));
                if let Some(csa) = &isda.csa {
                    prompt.push_str(&format!("  CSA: {} → @{}\n", csa.csa_type, csa.variable));
                }
            }
            prompt.push('\n');
        }

        prompt.push_str("Generate the complete DSL now.\n");

        prompt
    }

    fn strip_code_blocks(text: &str) -> String {
        let text = text.trim();
        if text.starts_with("```") {
            let lines: Vec<&str> = text.lines().collect();
            if lines.len() > 2 {
                // Skip first line (```...) and last line (```)
                return lines[1..lines.len() - 1].join("\n");
            }
        }
        text.to_string()
    }
}

/// Intent extractor using LLM API
pub struct IntentExtractor {
    client: Arc<dyn LlmClient>,
}

impl IntentExtractor {
    /// Create a new intent extractor with explicit API key
    pub fn new(api_key: String) -> Result<Self> {
        let client = create_llm_client_with_key(api_key)?;
        Ok(Self { client })
    }

    /// Create from environment variables
    pub fn from_env() -> Result<Self> {
        let client = create_llm_client()?;
        Ok(Self { client })
    }

    /// Create with a specific LLM client
    pub fn with_client(client: Arc<dyn LlmClient>) -> Self {
        Self { client }
    }

    /// Extract structured intent from natural language
    /// Returns IntentResult which is either Clear(intent) or NeedsClarification(request)
    pub async fn extract(
        &self,
        user_request: &str,
    ) -> Result<crate::intent::IntentResult> {
        let system_prompt = include_str!("prompts/intent_extraction_system.md");
        let user_prompt = format!(
            "Extract the onboarding intent from this request:\n\n{}",
            user_request
        );

        // Use chat_json for structured output
        let response = self.client.chat_json(system_prompt, &user_prompt).await?;

        // Parse JSON response - IntentResult uses untagged enum so serde picks the right variant
        let clean_json = Self::extract_json(&response)?;
        let result: crate::intent::IntentResult = serde_json::from_str(&clean_json)
            .map_err(|e| {
                anyhow!(
                    "Failed to parse intent JSON: {}\n\nJSON was:\n{}",
                    e,
                    clean_json
                )
            })?;

        Ok(result)
    }

    /// Extract intent, resolving clarification with user's choice
    /// `choice` is the user's response (e.g., "1", "2", or a typed name)
    pub async fn extract_with_clarification(
        &self,
        original_request: &str,
        clarification: &crate::intent::ClarificationRequest,
        user_choice: &str,
    ) -> Result<crate::intent::IntentResult> {
        // Build a clarified prompt incorporating the user's choice
        let system_prompt = include_str!("prompts/intent_extraction_system.md");

        let user_prompt = format!(
            r#"The user was asked to clarify an ambiguous request.

Original request: {}

Ambiguity: {}

User's choice: {}

Now extract the intent based on their clarification. If they chose option 1, use interpretation 1. If they chose option 2, use interpretation 2. If they typed a name, use that as the client name.

Return the structured intent JSON (not a clarification request)."#,
            original_request, clarification.ambiguity.question, user_choice
        );

        let response = self.client.chat_json(system_prompt, &user_prompt).await?;
        let clean_json = Self::extract_json(&response)?;
        let result: crate::intent::IntentResult = serde_json::from_str(&clean_json)
            .map_err(|e| {
                anyhow!(
                    "Failed to parse clarified intent: {}\n\nJSON: {}",
                    e,
                    clean_json
                )
            })?;

        Ok(result)
    }

    fn extract_json(text: &str) -> Result<String> {
        let text = text.trim();

        // Strip ```json ... ``` if present
        let json = if text.contains("```json") {
            text.split("```json")
                .nth(1)
                .and_then(|s| s.split("```").next())
                .unwrap_or(text)
        } else if text.contains("```") {
            text.split("```")
                .nth(1)
                .and_then(|s| s.split("```").next())
                .unwrap_or(text)
        } else {
            text
        };

        Ok(json.trim().to_string())
    }
}
