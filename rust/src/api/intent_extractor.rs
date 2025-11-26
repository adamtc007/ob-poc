//! Intent Extractor - uses LLM to convert natural language to structured intents
//!
//! The LLM is prompted to output JSON intents rather than DSL code directly.
//! This allows for deterministic DSL assembly on the Rust side.

use super::intent::IntentSequence;
use crate::dsl_source::agentic::{MultiProviderLlm, RagContextProvider};
use crate::forth_engine::runtime::Runtime;
use anyhow::{Context, Result};
use std::sync::Arc;

/// Extracts structured intents from natural language using LLM
pub struct IntentExtractor {
    rag_provider: Arc<RagContextProvider>,
    runtime: Arc<Runtime>,
}

impl IntentExtractor {
    /// Create a new intent extractor
    pub fn new(rag_provider: Arc<RagContextProvider>, runtime: Arc<Runtime>) -> Self {
        Self {
            rag_provider,
            runtime,
        }
    }

    /// Extract intents from natural language
    pub async fn extract(&self, input: &str, domain: Option<&str>) -> Result<IntentSequence> {
        // Build the system prompt with verb catalog
        let system_prompt = self.build_extraction_prompt(domain);

        // Call LLM
        let llm = MultiProviderLlm::from_env().context("Failed to initialize LLM")?;

        let user_prompt = format!(
            "Extract intents from this request:\n\n{}\n\nRespond with JSON only.",
            input
        );

        let response = llm
            .generate(&system_prompt, &user_prompt)
            .await
            .context("LLM call failed")?;

        // Parse JSON response
        self.parse_response(&response.content)
    }

    /// Build the system prompt that instructs LLM to output structured intents
    fn build_extraction_prompt(&self, domain: Option<&str>) -> String {
        // Get available verbs from runtime
        let verbs: Vec<String> = if let Some(d) = domain {
            self.runtime
                .get_domain_words(d)
                .iter()
                .map(|w| format!("  - {} : {}", w.name, w.signature))
                .collect()
        } else {
            self.runtime
                .get_all_word_names()
                .iter()
                .filter_map(|name| self.runtime.get_word(name))
                .take(60) // Limit for prompt size
                .map(|w| format!("  - {} : {}", w.name, w.signature))
                .collect()
        };

        format!(
            r#"You are an intent extractor for a financial onboarding DSL system.

Your task is to extract STRUCTURED INTENTS from natural language requests.
You do NOT generate DSL code. You output JSON describing what operations to perform.

AVAILABLE VERBS:
{}

COMMON PARAMETER PATTERNS:
- cbu.ensure: cbu-name (string), client-type (COMPANY|INDIVIDUAL|TRUST|PARTNERSHIP), jurisdiction (2-letter ISO country code like "GB", "US", "LU"), nature-purpose (string)
- entity.create-proper-person: given-name, family-name, nationality (2-letter ISO), date-of-birth (YYYY-MM-DD)
- entity.create-limited-company: company-name, registration-number, jurisdiction (2-letter ISO), incorporation-date (YYYY-MM-DD)
- cbu.attach-entity: role (PRINCIPAL|DIRECTOR|SHAREHOLDER|BENEFICIAL_OWNER|SIGNATORY|AUTHORIZED_PERSON)
- document.request: document-type (PASSPORT|ID_CARD|CERT_OF_INCORP|PROOF_OF_ADDRESS|FINANCIAL_STATEMENT)
- screening.pep: Check if entity is a Politically Exposed Person
- screening.sanctions: Check entity against sanctions lists

REFERENCE SYNTAX:
- Use "@last_cbu" to reference the most recently created CBU
- Use "@last_entity" to reference the most recently created entity
- Use refs for relationships between entities created in the same request

OUTPUT FORMAT (JSON only, no markdown code blocks):
{{
  "intents": [
    {{
      "verb": "cbu.ensure",
      "params": {{"cbu-name": "Example Corp", "client-type": "COMPANY", "jurisdiction": "GB"}},
      "refs": {{}}
    }},
    {{
      "verb": "entity.create-proper-person",
      "params": {{"given-name": "John", "family-name": "Smith", "nationality": "GB"}},
      "refs": {{}}
    }},
    {{
      "verb": "cbu.attach-entity",
      "params": {{"role": "DIRECTOR"}},
      "refs": {{"cbu-id": "@last_cbu", "entity-id": "@last_entity"}}
    }}
  ],
  "reasoning": "User wants to create a company CBU with John Smith as director",
  "confidence": 0.95
}}

RULES:
1. Only use verbs from the AVAILABLE VERBS list
2. Extract ALL implied operations (e.g., "CBU with director" = create CBU + create person + attach)
3. Use refs for relationships between entities
4. If unsure about a value, use reasonable defaults or omit optional params
5. Output VALID JSON only - no markdown, no explanation outside JSON
6. Order intents logically (create CBU before attaching entities to it)
7. Use 2-letter ISO country codes for jurisdictions (GB, US, LU, DE, FR, etc.)
"#,
            verbs.join("\n")
        )
    }

    /// Parse the LLM response into an IntentSequence
    fn parse_response(&self, content: &str) -> Result<IntentSequence> {
        // Strip markdown code blocks if present
        let json_str = content
            .trim()
            .trim_start_matches("```json")
            .trim_start_matches("```")
            .trim_end_matches("```")
            .trim();

        // Try to parse as IntentSequence
        match serde_json::from_str::<IntentSequence>(json_str) {
            Ok(seq) => Ok(seq),
            Err(e) => {
                // Try to extract JSON from the response if it's wrapped in text
                if let Some(start) = json_str.find('{') {
                    if let Some(end) = json_str.rfind('}') {
                        let extracted = &json_str[start..=end];
                        return serde_json::from_str(extracted)
                            .with_context(|| format!("Failed to parse extracted JSON: {}", e));
                    }
                }
                Err(anyhow::anyhow!(
                    "Failed to parse LLM response as IntentSequence: {}. Content: {}",
                    e,
                    &json_str[..json_str.len().min(500)]
                ))
            }
        }
    }
}

// ============================================================================
// Tests
// ============================================================================

#[cfg(test)]
mod tests {
    use super::*;

    /// Helper to parse JSON without needing full IntentExtractor
    fn parse_json(content: &str) -> Result<IntentSequence> {
        let json_str = content
            .trim()
            .trim_start_matches("```json")
            .trim_start_matches("```")
            .trim_end_matches("```")
            .trim();

        match serde_json::from_str::<IntentSequence>(json_str) {
            Ok(seq) => Ok(seq),
            Err(e) => {
                if let Some(start) = json_str.find('{') {
                    if let Some(end) = json_str.rfind('}') {
                        let extracted = &json_str[start..=end];
                        return serde_json::from_str(extracted)
                            .with_context(|| format!("Failed to parse extracted JSON: {}", e));
                    }
                }
                Err(anyhow::anyhow!("Failed to parse: {}", e))
            }
        }
    }

    #[test]
    fn test_parse_clean_json() {
        let json = r#"{
            "intents": [
                {
                    "verb": "cbu.ensure",
                    "params": {"cbu-name": "Test Corp"},
                    "refs": {}
                }
            ],
            "reasoning": "Creating a CBU",
            "confidence": 0.95
        }"#;

        let result = parse_json(json);
        assert!(result.is_ok());
        let seq = result.unwrap();
        assert_eq!(seq.intents.len(), 1);
        assert_eq!(seq.intents[0].verb, "cbu.ensure");
    }

    #[test]
    fn test_parse_markdown_wrapped_json() {
        let json = r#"```json
{
    "intents": [
        {
            "verb": "cbu.ensure",
            "params": {"cbu-name": "Test Corp"},
            "refs": {}
        }
    ],
    "reasoning": "Creating a CBU",
    "confidence": 0.95
}
```"#;

        let result = parse_json(json);
        assert!(result.is_ok());
    }

    #[test]
    fn test_parse_text_wrapped_json() {
        let json = r#"Here is the extracted intent:

{
    "intents": [
        {
            "verb": "cbu.ensure",
            "params": {"cbu-name": "Test Corp"},
            "refs": {}
        }
    ],
    "reasoning": "Creating a CBU",
    "confidence": 0.95
}

Let me know if you need anything else."#;

        let result = parse_json(json);
        assert!(result.is_ok());
    }

    #[test]
    fn test_parse_multi_intent() {
        let json = r#"{
            "intents": [
                {
                    "verb": "cbu.ensure",
                    "params": {"cbu-name": "Test Corp", "client-type": "COMPANY"},
                    "refs": {}
                },
                {
                    "verb": "entity.create-proper-person",
                    "params": {"given-name": "John", "family-name": "Smith"},
                    "refs": {}
                },
                {
                    "verb": "cbu.attach-entity",
                    "params": {"role": "DIRECTOR"},
                    "refs": {"cbu-id": "@last_cbu", "entity-id": "@last_entity"}
                }
            ],
            "reasoning": "Creating CBU with director",
            "confidence": 0.9
        }"#;

        let result = parse_json(json);
        assert!(result.is_ok());
        let seq = result.unwrap();
        assert_eq!(seq.intents.len(), 3);
        assert_eq!(
            seq.intents[2].refs.get("cbu-id"),
            Some(&"@last_cbu".to_string())
        );
    }
}
