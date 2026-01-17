//! Structured Intent Pipeline
//!
//! Extracts structured intent from natural language and assembles
//! deterministic DSL code. The LLM NEVER writes DSL syntax — it only
//! extracts argument values.
//!
//! ## Pipeline
//!
//! 1. verb_search finds candidate verbs (learned → phrase → semantic)
//! 2. LLM extracts structured arguments (JSON only, never DSL syntax)
//! 3. Arguments assembled into DSL deterministically
//! 4. DSL validated before return

use anyhow::{anyhow, Result};
use serde::{Deserialize, Serialize};
use serde_json::Value;
use std::sync::Arc;

use ob_agentic::{create_llm_client, LlmClient};

use crate::dsl_v2::runtime_registry::RuntimeVerb;
use crate::dsl_v2::{compile, parse_program, registry};
use crate::mcp::verb_search::{HybridVerbSearcher, VerbSearchResult};

/// Extracted structured intent
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct StructuredIntent {
    /// The verb to execute
    pub verb: String,
    /// Extracted argument values
    pub arguments: Vec<IntentArgument>,
    /// Confidence in extraction
    pub confidence: f32,
    /// Any extraction notes/warnings
    pub notes: Vec<String>,
}

/// A single argument extracted from user intent
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct IntentArgument {
    pub name: String,
    pub value: ArgumentValue,
    pub resolved: bool,
}

/// Possible argument value types
#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(untagged)]
pub enum ArgumentValue {
    String(String),
    Number(f64),
    Boolean(bool),
    Reference(String),  // @symbol reference
    Uuid(String),       // Resolved UUID
    Unresolved(String), // Needs resolution via dsl_lookup
}

/// Pipeline result
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct PipelineResult {
    pub intent: StructuredIntent,
    pub verb_candidates: Vec<VerbSearchResult>,
    pub dsl: String,
    pub valid: bool,
    pub validation_error: Option<String>,
    pub unresolved_refs: Vec<UnresolvedRef>,
}

/// An unresolved entity reference that needs lookup
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct UnresolvedRef {
    pub param_name: String,
    pub search_value: String,
    pub entity_type: Option<String>,
}

/// Structured intent extraction pipeline
pub struct IntentPipeline {
    verb_searcher: HybridVerbSearcher,
    llm_client: Option<Arc<dyn LlmClient>>,
}

impl IntentPipeline {
    /// Create pipeline with verb searcher (lazy LLM init)
    pub fn new(verb_searcher: HybridVerbSearcher) -> Self {
        Self {
            verb_searcher,
            llm_client: None,
        }
    }

    /// Create pipeline with pre-initialized LLM client
    pub fn with_llm(verb_searcher: HybridVerbSearcher, llm_client: Arc<dyn LlmClient>) -> Self {
        Self {
            verb_searcher,
            llm_client: Some(llm_client),
        }
    }

    /// Get or create LLM client
    fn get_llm(&self) -> Result<Arc<dyn LlmClient>> {
        if let Some(client) = &self.llm_client {
            Ok(Arc::clone(client))
        } else {
            create_llm_client()
        }
    }

    /// Full pipeline: instruction → structured intent → DSL
    pub async fn process(
        &self,
        instruction: &str,
        domain_hint: Option<&str>,
    ) -> Result<PipelineResult> {
        // Step 1: Find verb candidates
        let candidates = self
            .verb_searcher
            .search(instruction, domain_hint, 5)
            .await?;

        if candidates.is_empty() {
            return Err(anyhow!("No matching verbs found for: {}", instruction));
        }

        let top_verb = &candidates[0].verb;

        // Step 2: Get verb signature from registry
        let reg = registry();
        let parts: Vec<&str> = top_verb.splitn(2, '.').collect();
        if parts.len() != 2 {
            return Err(anyhow!("Invalid verb format: {}", top_verb));
        }

        let verb_def = reg
            .get_runtime_verb(parts[0], parts[1])
            .ok_or_else(|| anyhow!("Verb not in registry: {}", top_verb))?;

        // Step 3: Extract arguments via LLM (structured output only)
        let intent = self
            .extract_arguments(instruction, top_verb, verb_def, candidates[0].score)
            .await?;

        // Step 4: Assemble DSL deterministically
        let (dsl, unresolved) = self.assemble_dsl(&intent)?;

        // Step 5: Validate
        let (valid, validation_error) = self.validate_dsl(&dsl);

        Ok(PipelineResult {
            intent,
            verb_candidates: candidates,
            dsl,
            valid,
            validation_error,
            unresolved_refs: unresolved,
        })
    }

    /// Extract arguments from instruction using LLM (structured output only)
    async fn extract_arguments(
        &self,
        instruction: &str,
        verb: &str,
        verb_def: &RuntimeVerb,
        verb_confidence: f32,
    ) -> Result<StructuredIntent> {
        let llm = self.get_llm()?;

        // Build parameter schema for LLM
        let params_desc: Vec<String> = verb_def
            .args
            .iter()
            .map(|p| {
                let req = if p.required { "REQUIRED" } else { "optional" };
                let desc = p.description.as_deref().unwrap_or("");
                format!("- {}: {:?} ({}) - {}", p.name, p.arg_type, req, desc)
            })
            .collect();

        let system_prompt = format!(
            r#"You are an argument extractor for a DSL system.

Given a natural language instruction, extract argument values for the verb: {verb}

VERB PARAMETERS:
{params}

RULES:
1. Extract ONLY the values mentioned - do not invent data
2. For entity references (people, companies, CBUs), extract the name as given
3. For dates, use ISO format (YYYY-MM-DD)
4. For enums, match to closest valid value
5. If a required parameter cannot be extracted, set value to null
6. Do NOT write DSL syntax - only extract values

Respond with ONLY valid JSON:
{{
  "arguments": [
    {{"name": "param_name", "value": "extracted_value"}},
    ...
  ],
  "notes": ["any extraction notes"]
}}"#,
            verb = verb,
            params = params_desc.join("\n"),
        );

        let response = llm.chat(&system_prompt, instruction).await?;

        // Parse LLM response - handle potential markdown code blocks
        let json_str = extract_json_from_response(&response);

        let parsed: Value = serde_json::from_str(json_str)
            .map_err(|e| anyhow!("LLM returned invalid JSON: {} - response: {}", e, response))?;

        let mut arguments = Vec::new();
        if let Some(args) = parsed["arguments"].as_array() {
            for arg in args {
                let name = arg["name"].as_str().unwrap_or_default().to_string();
                if name.is_empty() {
                    continue;
                }
                let value = match &arg["value"] {
                    Value::String(s) => ArgumentValue::Unresolved(s.clone()),
                    Value::Number(n) => ArgumentValue::Number(n.as_f64().unwrap_or(0.0)),
                    Value::Bool(b) => ArgumentValue::Boolean(*b),
                    Value::Null => continue, // Skip null values
                    _ => ArgumentValue::Unresolved(arg["value"].to_string()),
                };
                arguments.push(IntentArgument {
                    name,
                    value,
                    resolved: false,
                });
            }
        }

        let notes: Vec<String> = parsed["notes"]
            .as_array()
            .map(|arr| {
                arr.iter()
                    .filter_map(|v| v.as_str().map(String::from))
                    .collect()
            })
            .unwrap_or_default();

        Ok(StructuredIntent {
            verb: verb.to_string(),
            arguments,
            confidence: verb_confidence,
            notes,
        })
    }

    /// Assemble DSL from structured intent (deterministic)
    fn assemble_dsl(&self, intent: &StructuredIntent) -> Result<(String, Vec<UnresolvedRef>)> {
        let mut dsl = format!("({}", intent.verb);
        let mut unresolved = Vec::new();

        for arg in &intent.arguments {
            let value_str = match &arg.value {
                ArgumentValue::String(s) => format!("\"{}\"", s.replace('"', "\\\"")),
                ArgumentValue::Number(n) => n.to_string(),
                ArgumentValue::Boolean(b) => b.to_string(),
                ArgumentValue::Reference(r) => format!("@{}", r),
                ArgumentValue::Uuid(u) => format!("\"{}\"", u),
                ArgumentValue::Unresolved(u) => {
                    // Mark for resolution
                    unresolved.push(UnresolvedRef {
                        param_name: arg.name.clone(),
                        search_value: u.clone(),
                        entity_type: None, // Could be inferred from verb arg type
                    });
                    format!("\"{}\"", u.replace('"', "\\\""))
                }
            };
            dsl.push_str(&format!(" :{} {}", arg.name, value_str));
        }

        dsl.push(')');
        Ok((dsl, unresolved))
    }

    /// Validate generated DSL
    fn validate_dsl(&self, dsl: &str) -> (bool, Option<String>) {
        match parse_program(dsl) {
            Ok(ast) => match compile(&ast) {
                Ok(_) => (true, None),
                Err(e) => (false, Some(format!("Compile error: {:?}", e))),
            },
            Err(e) => (false, Some(format!("Parse error: {:?}", e))),
        }
    }
}

/// Extract JSON from LLM response, handling markdown code blocks
fn extract_json_from_response(response: &str) -> &str {
    let trimmed = response.trim();

    // Handle ```json ... ``` blocks
    if trimmed.starts_with("```json") {
        if let Some(end) = trimmed.rfind("```") {
            let start = "```json".len();
            if end > start {
                return trimmed[start..end].trim();
            }
        }
    }

    // Handle ``` ... ``` blocks without language
    if let Some(stripped) = trimmed.strip_prefix("```") {
        if let Some(end) = stripped.find("```") {
            return stripped[..end].trim();
        }
    }

    trimmed
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_extract_json_from_response() {
        // Plain JSON
        let plain = r#"{"arguments": []}"#;
        assert_eq!(extract_json_from_response(plain), plain);

        // Markdown code block
        let markdown = "```json\n{\"arguments\": []}\n```";
        assert_eq!(extract_json_from_response(markdown), "{\"arguments\": []}");

        // With whitespace
        let whitespace = "  \n```json\n{\"arguments\": []}\n```\n  ";
        assert_eq!(
            extract_json_from_response(whitespace),
            "{\"arguments\": []}"
        );
    }

    #[test]
    fn test_assemble_dsl() {
        let intent = StructuredIntent {
            verb: "cbu.create".to_string(),
            arguments: vec![
                IntentArgument {
                    name: "name".to_string(),
                    value: ArgumentValue::Unresolved("Apex Fund".to_string()),
                    resolved: false,
                },
                IntentArgument {
                    name: "jurisdiction".to_string(),
                    value: ArgumentValue::String("LU".to_string()),
                    resolved: true,
                },
            ],
            confidence: 0.95,
            notes: vec![],
        };

        let searcher = HybridVerbSearcher::phrase_only("config/verbs");
        if searcher.is_err() {
            return; // Skip if config not available
        }
        let pipeline = IntentPipeline::new(searcher.unwrap());

        let (dsl, unresolved) = pipeline.assemble_dsl(&intent).unwrap();
        assert!(dsl.contains("cbu.create"));
        assert!(dsl.contains(":name \"Apex Fund\""));
        assert!(dsl.contains(":jurisdiction \"LU\""));
        assert_eq!(unresolved.len(), 1);
        assert_eq!(unresolved[0].param_name, "name");
    }
}
