//! DSL Generator
//!
//! Uses Claude API to generate DSL from structured requirements.

use anyhow::{anyhow, Result};
use serde::Deserialize;

use crate::agentic::patterns::OnboardingPattern;
use crate::agentic::planner::OnboardingPlan;

/// DSL generator using Claude API
pub struct DslGenerator {
    api_key: String,
    client: reqwest::Client,
    model: String,
}

/// Response from Claude API
#[derive(Debug, Deserialize)]
struct ClaudeResponse {
    content: Vec<ContentBlock>,
}

#[derive(Debug, Deserialize)]
struct ContentBlock {
    #[serde(rename = "type")]
    #[allow(dead_code)]
    content_type: String,
    text: Option<String>,
}

impl DslGenerator {
    /// Create a new DSL generator
    pub fn new(api_key: String) -> Self {
        Self {
            api_key,
            client: reqwest::Client::new(),
            model: "claude-sonnet-4-20250514".to_string(),
        }
    }

    /// Create with a specific model
    pub fn with_model(api_key: String, model: &str) -> Self {
        Self {
            api_key,
            client: reqwest::Client::new(),
            model: model.to_string(),
        }
    }

    /// Generate DSL from an onboarding plan
    pub async fn generate(&self, plan: &OnboardingPlan) -> Result<String> {
        let system_prompt = self.build_system_prompt(plan.pattern);
        let user_prompt = self.build_user_prompt(plan);

        let response = self
            .client
            .post("https://api.anthropic.com/v1/messages")
            .header("x-api-key", &self.api_key)
            .header("anthropic-version", "2023-06-01")
            .header("content-type", "application/json")
            .json(&serde_json::json!({
                "model": &self.model,
                "max_tokens": 4000,
                "system": system_prompt,
                "messages": [
                    {"role": "user", "content": user_prompt}
                ]
            }))
            .send()
            .await?;

        if !response.status().is_success() {
            let status = response.status();
            let body = response.text().await.unwrap_or_default();
            return Err(anyhow!("Claude API error {}: {}", status, body));
        }

        let claude_response: ClaudeResponse = response.json().await?;
        let dsl = claude_response
            .content
            .first()
            .and_then(|c| c.text.as_ref())
            .ok_or_else(|| anyhow!("Empty response from Claude"))?;

        Ok(Self::strip_code_blocks(dsl))
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

        let response = self
            .client
            .post("https://api.anthropic.com/v1/messages")
            .header("x-api-key", &self.api_key)
            .header("anthropic-version", "2023-06-01")
            .header("content-type", "application/json")
            .json(&serde_json::json!({
                "model": &self.model,
                "max_tokens": 4000,
                "messages": [
                    {"role": "user", "content": prompt}
                ]
            }))
            .send()
            .await?;

        if !response.status().is_success() {
            let status = response.status();
            let body = response.text().await.unwrap_or_default();
            return Err(anyhow!("Claude API error {}: {}", status, body));
        }

        let claude_response: ClaudeResponse = response.json().await?;
        let dsl = claude_response
            .content
            .first()
            .and_then(|c| c.text.as_ref())
            .ok_or_else(|| anyhow!("Empty response from Claude"))?;

        Ok(Self::strip_code_blocks(dsl))
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
            prompt.push_str("\n");
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
        prompt.push_str("\n");

        // SSIs
        prompt.push_str("## SSIs Required\n");
        for s in &plan.ssis {
            prompt.push_str(&format!(
                "- {} ({}, {}) → @{}\n",
                s.name, s.ssi_type, s.currency, s.variable
            ));
        }
        prompt.push_str("\n");

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
        prompt.push_str("\n");

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
            prompt.push_str("\n");
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

/// Intent extractor using Claude API
pub struct IntentExtractor {
    api_key: String,
    client: reqwest::Client,
    model: String,
}

impl IntentExtractor {
    /// Create a new intent extractor
    pub fn new(api_key: String) -> Self {
        Self {
            api_key,
            client: reqwest::Client::new(),
            model: "claude-sonnet-4-20250514".to_string(),
        }
    }

    /// Extract structured intent from natural language
    pub async fn extract(
        &self,
        user_request: &str,
    ) -> Result<crate::agentic::intent::OnboardingIntent> {
        let system_prompt = include_str!("prompts/intent_extraction_system.md");

        let response = self
            .client
            .post("https://api.anthropic.com/v1/messages")
            .header("x-api-key", &self.api_key)
            .header("anthropic-version", "2023-06-01")
            .header("content-type", "application/json")
            .json(&serde_json::json!({
                "model": &self.model,
                "max_tokens": 2000,
                "system": system_prompt,
                "messages": [
                    {"role": "user", "content": format!(
                        "Extract the onboarding intent from this request:\n\n{}",
                        user_request
                    )}
                ]
            }))
            .send()
            .await?;

        if !response.status().is_success() {
            let status = response.status();
            let body = response.text().await.unwrap_or_default();
            return Err(anyhow!("Claude API error {}: {}", status, body));
        }

        let claude_response: ClaudeResponse = response.json().await?;
        let json_str = claude_response
            .content
            .first()
            .and_then(|c| c.text.as_ref())
            .ok_or_else(|| anyhow!("Empty response from Claude"))?;

        // Parse JSON response
        let clean_json = Self::extract_json(json_str)?;
        let intent: crate::agentic::intent::OnboardingIntent = serde_json::from_str(&clean_json)
            .map_err(|e| {
                anyhow!(
                    "Failed to parse intent JSON: {}\n\nJSON was:\n{}",
                    e,
                    clean_json
                )
            })?;

        Ok(intent)
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
