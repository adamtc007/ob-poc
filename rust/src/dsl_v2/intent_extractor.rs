//! Intent Extractor - Extract structured DSL intent from natural language
//!
//! This module uses LLM API (Anthropic or OpenAI) to convert natural language
//! requests into structured DslIntentBatch objects that can be deterministically
//! assembled into valid DSL code.
//!
//! The key insight: AI extracts STRUCTURE, not DSL text. All entity resolution
//! and DSL assembly happens deterministically in Rust.

use anyhow::{anyhow, Result};
use serde::Deserialize;
use std::sync::Arc;

use crate::agentic::{create_llm_client, create_llm_client_with_key, LlmClient};
use crate::dsl_v2::intent::{ArgIntent, DslIntent, DslIntentBatch};

/// Intent extractor using LLM API
pub struct IntentExtractor {
    client: Arc<dyn LlmClient>,
}

/// Raw intent batch as returned by LLM (before conversion)
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
    /// Create a new intent extractor with explicit API key
    ///
    /// Returns an error if the LLM client cannot be created.
    pub fn new(api_key: String) -> Result<Self> {
        let client = create_llm_client_with_key(api_key)?;
        Ok(Self { client })
    }

    /// Create with a specific model (legacy compatibility)
    pub fn with_model(api_key: String, _model: &str) -> Result<Self> {
        // Model is now controlled via environment variables
        Self::new(api_key)
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
    pub async fn extract(&self, user_request: &str) -> Result<DslIntentBatch> {
        let system_prompt = include_str!("prompts/general_intent_extraction.md");

        // Use chat_json for structured output
        let response = self.client.chat_json(system_prompt, user_request).await?;

        // Parse JSON response
        let clean_json = Self::extract_json(&response)?;
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

        // Use chat_json for structured output
        let response = self.client.chat_json(&full_system, user_request).await?;

        let clean_json = Self::extract_json(&response)?;
        let raw_batch: RawIntentBatch = serde_json::from_str(&clean_json).map_err(|e| {
            anyhow!(
                "Failed to parse intent JSON: {}\n\nJSON was:\n{}",
                e,
                clean_json
            )
        })?;

        Ok(self.convert_batch(raw_batch))
    }

    /// Convert raw LLM response to our internal types
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
