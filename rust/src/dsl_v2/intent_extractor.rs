//! Intent Extractor - Extract structured DSL intent from natural language
//!
//! This module uses Claude API to convert natural language requests into
//! structured DslIntentBatch objects that can be deterministically assembled
//! into valid DSL code.
//!
//! The key insight: AI extracts STRUCTURE, not DSL text. All entity resolution
//! and DSL assembly happens deterministically in Rust.

use anyhow::{anyhow, Result};
use serde::Deserialize;

use crate::dsl_v2::intent::{ArgIntent, DslIntent, DslIntentBatch};

/// Intent extractor using Claude API
pub struct IntentExtractor {
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

/// Raw intent batch as returned by Claude (before conversion)
#[derive(Debug, Deserialize)]
struct RawIntentBatch {
    actions: Vec<RawIntent>,
    context: Option<String>,
    original_request: String,
}

#[derive(Debug, Deserialize)]
struct RawIntent {
    verb: Option<String>,
    action: String,
    domain: String,
    args: std::collections::HashMap<String, RawArgIntent>,
    bind_as: Option<String>,
    source_text: Option<String>,
}

#[derive(Debug, Deserialize)]
#[serde(tag = "type")]
enum RawArgIntent {
    Literal {
        value: serde_json::Value,
    },
    SymbolRef {
        symbol: String,
    },
    EntityLookup {
        search_text: String,
        entity_type: Option<String>,
    },
    RefDataLookup {
        search_text: String,
        ref_type: String,
    },
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

    /// Create with a specific model
    pub fn with_model(api_key: String, model: &str) -> Self {
        Self {
            api_key,
            client: reqwest::Client::new(),
            model: model.to_string(),
        }
    }

    /// Create from environment variables
    pub fn from_env() -> Result<Self> {
        let api_key = std::env::var("ANTHROPIC_API_KEY")
            .map_err(|_| anyhow!("ANTHROPIC_API_KEY environment variable not set"))?;
        Ok(Self::new(api_key))
    }

    /// Extract structured intent from natural language
    pub async fn extract(&self, user_request: &str) -> Result<DslIntentBatch> {
        let system_prompt = include_str!("prompts/general_intent_extraction.md");

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
                    {"role": "user", "content": user_request}
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
        let raw_batch: RawIntentBatch = serde_json::from_str(&clean_json).map_err(|e| {
            anyhow!(
                "Failed to parse intent JSON: {}\n\nJSON was:\n{}",
                e,
                clean_json
            )
        })?;

        // Convert to our internal types
        Ok(self.convert_batch(raw_batch))
    }

    /// Extract with additional context (e.g., verb schemas)
    pub async fn extract_with_context(
        &self,
        user_request: &str,
        additional_context: &str,
    ) -> Result<DslIntentBatch> {
        let system_prompt = include_str!("prompts/general_intent_extraction.md");
        let full_system = format!(
            "{}\n\n## Additional Context\n\n{}",
            system_prompt, additional_context
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
                "system": full_system,
                "messages": [
                    {"role": "user", "content": user_request}
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

        let clean_json = Self::extract_json(json_str)?;
        let raw_batch: RawIntentBatch = serde_json::from_str(&clean_json).map_err(|e| {
            anyhow!(
                "Failed to parse intent JSON: {}\n\nJSON was:\n{}",
                e,
                clean_json
            )
        })?;

        Ok(self.convert_batch(raw_batch))
    }

    /// Convert raw Claude response to our internal types
    fn convert_batch(&self, raw: RawIntentBatch) -> DslIntentBatch {
        let actions = raw
            .actions
            .into_iter()
            .map(|a| self.convert_intent(a))
            .collect();

        DslIntentBatch {
            actions,
            context: raw.context,
            original_request: raw.original_request,
        }
    }

    fn convert_intent(&self, raw: RawIntent) -> DslIntent {
        let args = raw
            .args
            .into_iter()
            .map(|(k, v)| (k, self.convert_arg(v)))
            .collect();

        DslIntent {
            verb: raw.verb,
            action: raw.action,
            domain: raw.domain,
            args,
            bind_as: raw.bind_as,
            source_text: raw.source_text,
        }
    }

    fn convert_arg(&self, raw: RawArgIntent) -> ArgIntent {
        match raw {
            RawArgIntent::Literal { value } => ArgIntent::Literal { value },
            RawArgIntent::SymbolRef { symbol } => ArgIntent::SymbolRef { symbol },
            RawArgIntent::EntityLookup {
                search_text,
                entity_type,
            } => ArgIntent::EntityLookup {
                search_text,
                entity_type,
            },
            RawArgIntent::RefDataLookup {
                search_text,
                ref_type,
            } => ArgIntent::RefDataLookup {
                search_text,
                ref_type,
            },
        }
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

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_extract_json_plain() {
        let input = r#"{"actions": []}"#;
        let result = IntentExtractor::extract_json(input).unwrap();
        assert_eq!(result, r#"{"actions": []}"#);
    }

    #[test]
    fn test_extract_json_with_markdown() {
        let input = r#"```json
{"actions": []}
```"#;
        let result = IntentExtractor::extract_json(input).unwrap();
        assert_eq!(result, r#"{"actions": []}"#);
    }

    #[test]
    fn test_parse_raw_intent() {
        let json = r#"{
            "actions": [
                {
                    "verb": "cbu.ensure",
                    "action": "create",
                    "domain": "cbu",
                    "args": {
                        "name": {"type": "Literal", "value": "Test Fund"},
                        "jurisdiction": {"type": "RefDataLookup", "search_text": "LU", "ref_type": "jurisdiction"}
                    },
                    "bind_as": "fund",
                    "source_text": "Create Test Fund"
                }
            ],
            "context": "Test context",
            "original_request": "Create a test fund in Luxembourg"
        }"#;

        let raw: RawIntentBatch = serde_json::from_str(json).unwrap();
        assert_eq!(raw.actions.len(), 1);
        assert_eq!(raw.actions[0].verb, Some("cbu.ensure".to_string()));
    }
}
