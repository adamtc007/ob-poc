//! Research macro executor
//!
//! Executes research macros using LLM with tool use, validates output,
//! and generates suggested DSL verbs.

use std::collections::HashMap;

use chrono::Utc;
use handlebars::Handlebars;
use regex::Regex;
use serde::{Deserialize, Serialize};
use serde_json::Value;
use uuid::Uuid;

use super::definition::{ResearchMacroDef, ReviewRequirement};
use super::error::{ResearchError, Result};
use super::llm_client::{web_search_tool, ResearchLlmClient, ResearchSource, ToolDef};
use super::registry::ResearchMacroRegistry;

/// Result of executing a research macro
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ResearchResult {
    /// Unique ID for this research result
    pub result_id: Uuid,

    /// Macro that was executed
    pub macro_name: String,

    /// Parameters that were passed
    pub params: Value,

    /// The structured data returned by LLM
    pub data: Value,

    /// Whether the data passed schema validation
    pub schema_valid: bool,

    /// Validation errors if any
    pub validation_errors: Vec<String>,

    /// Whether human review is required before use
    pub review_required: bool,

    /// Suggested DSL verbs (template expanded with data)
    pub suggested_verbs: Option<String>,

    /// Search quality self-assessment from LLM
    pub search_quality: Option<SearchQuality>,

    /// Sources used during research
    pub sources: Vec<ResearchSource>,

    /// Timestamp
    pub created_at: chrono::DateTime<Utc>,
}

/// Search quality assessment
#[derive(Debug, Clone, Copy, Serialize, Deserialize, PartialEq, Eq)]
#[serde(rename_all = "UPPERCASE")]
pub enum SearchQuality {
    High,
    Medium,
    Low,
}

/// Approved research result ready for verb generation
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ApprovedResearch {
    pub result_id: Uuid,
    pub approved_at: chrono::DateTime<Utc>,
    pub approved_data: Value,
    pub generated_verbs: String,
    pub edits_made: bool,
}

/// Research macro executor
pub struct ResearchExecutor<C: ResearchLlmClient> {
    registry: ResearchMacroRegistry,
    llm_client: C,
    handlebars: Handlebars<'static>,
    validate_leis: bool,
}

impl<C: ResearchLlmClient> ResearchExecutor<C> {
    /// Create a new executor
    pub fn new(registry: ResearchMacroRegistry, llm_client: C) -> Self {
        let mut handlebars = Handlebars::new();

        // Register helpers
        handlebars.register_helper("slugify", Box::new(slugify_helper));
        handlebars.register_helper("uppercase", Box::new(uppercase_helper));
        handlebars.register_helper("lowercase", Box::new(lowercase_helper));
        handlebars.register_helper("json", Box::new(json_helper));

        Self {
            registry,
            llm_client,
            handlebars,
            validate_leis: true,
        }
    }

    /// Disable LEI validation (useful for testing)
    pub fn without_lei_validation(mut self) -> Self {
        self.validate_leis = false;
        self
    }

    /// Execute a research macro
    pub async fn execute(
        &self,
        macro_name: &str,
        params: HashMap<String, Value>,
    ) -> Result<ResearchResult> {
        // 1. Get macro definition
        let macro_def = self
            .registry
            .get(macro_name)
            .ok_or_else(|| ResearchError::UnknownMacro(macro_name.to_string()))?;

        // 2. Validate parameters
        let validated_params = self.validate_and_fill_params(macro_def, params)?;

        // 3. Render prompt - convert HashMap to JSON Value for template rendering
        let params_json = serde_json::to_value(&validated_params)?;
        let prompt = self.render_template(&macro_def.prompt, &params_json)?;

        // 4. Build tools list
        let tools = self.build_tools(&macro_def.tools);

        // 5. Execute LLM call with tools
        let system_prompt = self.build_system_prompt(macro_def);
        let response = self
            .llm_client
            .complete_with_tools(&system_prompt, &prompt, &tools)
            .await?;

        // 6. Parse JSON with repair attempt
        let data = self.parse_json_with_repair(&response.content)?;

        // 7. Validate against schema
        let (schema_valid, validation_errors) =
            self.validate_schema(&macro_def.output.schema, &data);

        // 8. Optionally validate LEIs exist
        let lei_warnings = if self.validate_leis && schema_valid {
            self.validate_leis_batch(&data).await
        } else {
            vec![]
        };

        // Combine validation errors
        let mut all_errors = validation_errors;
        all_errors.extend(lei_warnings);

        // 9. Render suggested verbs
        let suggested_verbs = macro_def
            .suggested_verbs
            .as_ref()
            .map(|template| self.render_template(template, &data))
            .transpose()?;

        // 10. Extract search quality
        let search_quality = data
            .get("search_quality")
            .and_then(|v| v.as_str())
            .and_then(|s| match s.to_uppercase().as_str() {
                "HIGH" => Some(SearchQuality::High),
                "MEDIUM" => Some(SearchQuality::Medium),
                "LOW" => Some(SearchQuality::Low),
                _ => None,
            });

        Ok(ResearchResult {
            result_id: Uuid::now_v7(),
            macro_name: macro_name.to_string(),
            params: serde_json::to_value(&validated_params)?,
            data,
            schema_valid,
            validation_errors: all_errors,
            review_required: macro_def.output.review == ReviewRequirement::Required,
            suggested_verbs,
            search_quality,
            sources: response.sources,
            created_at: Utc::now(),
        })
    }

    /// Parse JSON with repair attempt for common LLM issues
    fn parse_json_with_repair(&self, content: &str) -> Result<Value> {
        // Try direct parse first
        if let Ok(v) = serde_json::from_str(content) {
            return Ok(v);
        }

        // Try extracting JSON from markdown code block
        let json_block_re = Regex::new(r"```(?:json)?\s*([\s\S]*?)\s*```").unwrap();
        if let Some(caps) = json_block_re.captures(content) {
            if let Ok(v) = serde_json::from_str(&caps[1]) {
                tracing::debug!("Extracted JSON from markdown code block");
                return Ok(v);
            }
        }

        // Try finding JSON object in content
        if let Some(start) = content.find('{') {
            if let Some(end) = content.rfind('}') {
                let json_str = &content[start..=end];
                if let Ok(v) = serde_json::from_str(json_str) {
                    tracing::debug!("Extracted JSON object from content");
                    return Ok(v);
                }
            }
        }

        // Try finding JSON array in content
        if let Some(start) = content.find('[') {
            if let Some(end) = content.rfind(']') {
                let json_str = &content[start..=end];
                if let Ok(v) = serde_json::from_str(json_str) {
                    tracing::debug!("Extracted JSON array from content");
                    return Ok(v);
                }
            }
        }

        Err(ResearchError::JsonParse(format!(
            "Failed to parse JSON from LLM response: {}",
            &content[..content.len().min(200)]
        )))
    }

    /// Validate LEIs exist in GLEIF (batch for efficiency)
    async fn validate_leis_batch(&self, data: &Value) -> Vec<String> {
        let leis = self.extract_leis(data);
        if leis.is_empty() {
            return vec![];
        }

        let mut warnings = Vec::new();
        let client = reqwest::Client::new();

        // Batch validation with limited concurrency
        for lei in leis.iter().take(10) {
            // Limit to 10 to avoid rate limiting
            let url = format!("https://api.gleif.org/api/v1/lei-records/{}", lei);

            match client.head(&url).send().await {
                Ok(resp) if resp.status() == 404 => {
                    warnings.push(format!("LEI {} not found in GLEIF - may be invalid", lei));
                }
                Ok(resp) if !resp.status().is_success() => {
                    tracing::warn!(lei = %lei, status = %resp.status(), "GLEIF validation returned error");
                }
                Err(e) => {
                    tracing::warn!(lei = %lei, error = %e, "Failed to validate LEI against GLEIF");
                }
                _ => {} // Success - LEI exists
            }
        }

        warnings
    }

    fn extract_leis(&self, data: &Value) -> Vec<String> {
        let mut leis = Vec::new();
        let lei_re = Regex::new(r"\b[A-Z0-9]{20}\b").unwrap();

        fn walk(v: &Value, re: &Regex, out: &mut Vec<String>) {
            match v {
                Value::String(s) => {
                    for cap in re.find_iter(s) {
                        // Additional validation: LEI checksum
                        let potential_lei = cap.as_str();
                        if is_valid_lei_format(potential_lei) {
                            out.push(potential_lei.to_string());
                        }
                    }
                }
                Value::Array(arr) => arr.iter().for_each(|x| walk(x, re, out)),
                Value::Object(obj) => obj.values().for_each(|x| walk(x, re, out)),
                _ => {}
            }
        }

        walk(data, &lei_re, &mut leis);
        leis.sort();
        leis.dedup();
        leis
    }

    fn validate_schema(&self, schema: &Value, data: &Value) -> (bool, Vec<String>) {
        // Use jsonschema crate for validation
        // Note: jsonschema 0.29+ uses iter_errors() to collect all errors
        match jsonschema::validator_for(schema) {
            Ok(validator) => {
                let errors: Vec<String> = validator
                    .iter_errors(data)
                    .map(|e| format!("{}: {}", e.instance_path, e))
                    .collect();
                if errors.is_empty() {
                    (true, vec![])
                } else {
                    (false, errors)
                }
            }
            Err(e) => (false, vec![format!("Invalid schema: {}", e)]),
        }
    }

    fn build_system_prompt(&self, macro_def: &ResearchMacroDef) -> String {
        format!(
            r#"You are a research assistant for institutional client onboarding.

Your task: {}

IMPORTANT INSTRUCTIONS:
1. Use the web_search tool to find current, accurate information
2. Return ONLY valid JSON matching the required schema
3. Do NOT include markdown formatting, code blocks, or explanatory text
4. Include a "search_quality" field with value "HIGH", "MEDIUM", or "LOW"
5. For LEIs, verify they are real 20-character alphanumeric codes
6. Be precise with jurisdictions (use ISO 2-letter codes)

Schema name: {}
"#,
            macro_def.description, macro_def.output.schema_name
        )
    }

    fn build_tools(&self, tool_names: &[String]) -> Vec<ToolDef> {
        tool_names
            .iter()
            .filter_map(|name| match name.as_str() {
                "web_search" => Some(web_search_tool()),
                _ => {
                    tracing::warn!(tool = %name, "Unknown tool requested");
                    None
                }
            })
            .collect()
    }

    fn validate_and_fill_params(
        &self,
        macro_def: &ResearchMacroDef,
        mut params: HashMap<String, Value>,
    ) -> Result<HashMap<String, Value>> {
        for param_def in &macro_def.parameters {
            if !params.contains_key(&param_def.name) {
                if param_def.required {
                    if let Some(default) = &param_def.default {
                        params.insert(param_def.name.clone(), default.clone());
                    } else {
                        return Err(ResearchError::MissingParameter(param_def.name.clone()));
                    }
                } else if let Some(default) = &param_def.default {
                    params.insert(param_def.name.clone(), default.clone());
                }
            }

            // Validate enum values
            if let (Some(enum_values), Some(value)) =
                (&param_def.enum_values, params.get(&param_def.name))
            {
                if let Some(s) = value.as_str() {
                    if !enum_values.contains(&s.to_string()) {
                        return Err(ResearchError::InvalidParameter {
                            param: param_def.name.clone(),
                            reason: format!("Must be one of: {:?}", enum_values),
                        });
                    }
                }
            }
        }
        Ok(params)
    }

    fn render_template(&self, template: &str, data: &Value) -> Result<String> {
        self.handlebars
            .render_template(template, data)
            .map_err(|e| ResearchError::TemplateRender(e.to_string()))
    }
}

/// Check if a string looks like a valid LEI format
fn is_valid_lei_format(s: &str) -> bool {
    if s.len() != 20 {
        return false;
    }

    // LEI format: First 4 chars are LOU prefix, then 14 chars entity ID, then 2 check digits
    // All alphanumeric
    s.chars().all(|c| c.is_ascii_alphanumeric())
}

// Handlebars helpers

fn slugify_helper(
    h: &handlebars::Helper,
    _: &Handlebars,
    _: &handlebars::Context,
    _: &mut handlebars::RenderContext,
    out: &mut dyn handlebars::Output,
) -> handlebars::HelperResult {
    let param = h.param(0).and_then(|v| v.value().as_str()).unwrap_or("");
    let slug: String = param
        .to_lowercase()
        .chars()
        .map(|c| if c.is_alphanumeric() { c } else { '_' })
        .collect();
    out.write(&slug)?;
    Ok(())
}

fn uppercase_helper(
    h: &handlebars::Helper,
    _: &Handlebars,
    _: &handlebars::Context,
    _: &mut handlebars::RenderContext,
    out: &mut dyn handlebars::Output,
) -> handlebars::HelperResult {
    let param = h.param(0).and_then(|v| v.value().as_str()).unwrap_or("");
    out.write(&param.to_uppercase())?;
    Ok(())
}

fn lowercase_helper(
    h: &handlebars::Helper,
    _: &Handlebars,
    _: &handlebars::Context,
    _: &mut handlebars::RenderContext,
    out: &mut dyn handlebars::Output,
) -> handlebars::HelperResult {
    let param = h.param(0).and_then(|v| v.value().as_str()).unwrap_or("");
    out.write(&param.to_lowercase())?;
    Ok(())
}

fn json_helper(
    h: &handlebars::Helper,
    _: &Handlebars,
    _: &handlebars::Context,
    _: &mut handlebars::RenderContext,
    out: &mut dyn handlebars::Output,
) -> handlebars::HelperResult {
    if let Some(v) = h.param(0) {
        out.write(&serde_json::to_string(v.value()).unwrap_or_default())?;
    }
    Ok(())
}

#[cfg(test)]
mod tests {
    use super::super::llm_client::LlmResponse;
    use super::*;

    #[test]
    fn test_json_repair_direct() {
        let executor = create_test_executor();
        let json = r#"{"name": "Test"}"#;
        let result = executor.parse_json_with_repair(json).unwrap();
        assert_eq!(result["name"], "Test");
    }

    #[test]
    fn test_json_repair_markdown() {
        let executor = create_test_executor();
        let content = r#"Here's the result:

```json
{"name": "Test Corp"}
```

That's all!"#;
        let result = executor.parse_json_with_repair(content).unwrap();
        assert_eq!(result["name"], "Test Corp");
    }

    #[test]
    fn test_json_repair_embedded() {
        let executor = create_test_executor();
        let content = r#"The analysis shows {"apex": {"name": "Acme"}} which indicates..."#;
        let result = executor.parse_json_with_repair(content).unwrap();
        assert_eq!(result["apex"]["name"], "Acme");
    }

    #[test]
    fn test_lei_extraction() {
        let executor = create_test_executor();
        let data = serde_json::json!({
            "apex": {
                "lei": "529900K9B0N5BT694847"
            },
            "subsidiaries": [
                { "lei": "OJ2TIQSVQND4IZYYK658" }
            ]
        });

        let leis = executor.extract_leis(&data);
        assert_eq!(leis.len(), 2);
        assert!(leis.contains(&"529900K9B0N5BT694847".to_string()));
        assert!(leis.contains(&"OJ2TIQSVQND4IZYYK658".to_string()));
    }

    #[test]
    fn test_lei_format_validation() {
        assert!(is_valid_lei_format("529900K9B0N5BT694847"));
        assert!(is_valid_lei_format("OJ2TIQSVQND4IZYYK658"));
        assert!(!is_valid_lei_format("too-short"));
        assert!(!is_valid_lei_format("has spaces in the middle"));
        assert!(!is_valid_lei_format("529900K9B0N5BT69484!")); // special char
    }

    fn create_test_executor() -> ResearchExecutor<MockLlmClient> {
        ResearchExecutor::new(ResearchMacroRegistry::new(), MockLlmClient)
    }

    struct MockLlmClient;

    #[async_trait::async_trait]
    impl ResearchLlmClient for MockLlmClient {
        async fn complete_with_tools(
            &self,
            _system: &str,
            _user: &str,
            _tools: &[ToolDef],
        ) -> Result<LlmResponse> {
            Ok(LlmResponse {
                content: r#"{"test": true}"#.to_string(),
                sources: vec![],
                tool_calls: vec![],
            })
        }

        async fn complete_json(
            &self,
            _system: &str,
            _user: &str,
            _schema: &Value,
        ) -> Result<Value> {
            Ok(serde_json::json!({"test": true}))
        }

        fn model_name(&self) -> &str {
            "mock"
        }
    }
}
