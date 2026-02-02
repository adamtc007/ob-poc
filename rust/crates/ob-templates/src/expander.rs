//! Template Expander
//!
//! Expands template definitions to DSL source text by substituting parameters.

use std::collections::HashMap;
use uuid::Uuid;

use super::definition::{ParamDefinition, TemplateDefinition};

/// Session context for template expansion
///
/// This is a standalone context type that can be populated from various sources.
/// Integration with specific session types (e.g., ReplSession, SessionContext) is
/// provided via extension traits in the main crate.
#[derive(Debug, Clone, Default)]
pub struct ExpansionContext {
    /// Current CBU (if any)
    pub current_cbu: Option<Uuid>,
    /// Current KYC case (if any)
    pub current_case: Option<Uuid>,
    /// Named bindings from previous executions or resolved entities
    /// Format: name → UUID string
    pub bindings: HashMap<String, String>,
    /// Entity type information for bindings (for type validation)
    /// Format: name → entity_type (e.g., "cbu", "entity.proper_person")
    pub binding_types: HashMap<String, String>,
}

impl ExpansionContext {
    /// Create a new empty context
    pub fn new() -> Self {
        Self::default()
    }

    /// Create context with CBU
    pub fn with_cbu(cbu_id: Uuid) -> Self {
        Self {
            current_cbu: Some(cbu_id),
            ..Default::default()
        }
    }

    /// Create context with CBU and case
    pub fn with_cbu_and_case(cbu_id: Uuid, case_id: Uuid) -> Self {
        Self {
            current_cbu: Some(cbu_id),
            current_case: Some(case_id),
            ..Default::default()
        }
    }

    /// Add a binding
    pub fn bind(&mut self, name: impl Into<String>, value: impl Into<String>) {
        self.bindings.insert(name.into(), value.into());
    }

    /// Add a typed binding
    pub fn bind_typed(
        &mut self,
        name: impl Into<String>,
        value: impl Into<String>,
        entity_type: impl Into<String>,
    ) {
        let name = name.into();
        self.bindings.insert(name.clone(), value.into());
        self.binding_types.insert(name, entity_type.into());
    }

    /// Get entity type for a binding
    pub fn get_binding_type(&self, name: &str) -> Option<&str> {
        self.binding_types.get(name).map(|s| s.as_str())
    }

    /// Check if a binding's type matches expected type
    ///
    /// Supports:
    /// - Exact match: "cbu" matches "cbu"
    /// - Base type match: "entity" matches "entity.proper_person"
    /// - Subtype match: "entity.proper_person" matches "entity.proper_person"
    pub fn binding_matches_type(&self, name: &str, expected: &str) -> bool {
        match self.get_binding_type(name) {
            None => false,
            Some(actual) => {
                if actual == expected {
                    return true;
                }
                // Check if expected is base type (e.g., "entity" matches "entity.proper_person")
                if actual.starts_with(&format!("{}.", expected)) {
                    return true;
                }
                // Check if expected has subtype that matches
                if expected.starts_with(&format!("{}.", actual)) {
                    return true;
                }
                false
            }
        }
    }

    /// Set current CBU and add it as a binding
    pub fn set_current_cbu(&mut self, cbu_id: Uuid) {
        self.current_cbu = Some(cbu_id);
        self.bindings.insert("cbu".to_string(), cbu_id.to_string());
        self.binding_types
            .insert("cbu".to_string(), "cbu".to_string());
    }

    /// Set current case and add it as a binding
    pub fn set_current_case(&mut self, case_id: Uuid) {
        self.current_case = Some(case_id);
        self.bindings
            .insert("case".to_string(), case_id.to_string());
        self.binding_types
            .insert("case".to_string(), "kyc_case".to_string());
    }
}

/// Result of template expansion
#[derive(Debug, Clone)]
pub struct ExpansionResult {
    /// The expanded DSL source text
    pub dsl: String,
    /// Parameters that were filled
    pub filled_params: Vec<String>,
    /// Parameters still needing values
    pub missing_params: Vec<MissingParam>,
    /// What the template will output
    pub outputs: Vec<String>,
    /// Template ID that was expanded
    pub template_id: String,
}

/// A parameter that still needs a value
#[derive(Debug, Clone)]
pub struct MissingParam {
    /// Parameter name
    pub name: String,
    /// Parameter type
    pub param_type: String,
    /// Human-readable prompt
    pub prompt: String,
    /// Example value
    pub example: Option<String>,
    /// Is this required?
    pub required: bool,
    /// Validation hint
    pub validation: Option<String>,
}

/// Expands templates to DSL source text
pub struct TemplateExpander;

impl TemplateExpander {
    /// Expand a template to DSL text
    ///
    /// Substitutes parameters in order of precedence:
    /// 1. Explicit params provided
    /// 2. Session context (current_cbu, current_case, bindings)
    /// 3. Default values from param definition
    /// 4. Leave as placeholder if still unknown (and track as missing)
    pub fn expand(
        template: &TemplateDefinition,
        explicit_params: &HashMap<String, String>,
        context: &ExpansionContext,
    ) -> ExpansionResult {
        let mut dsl = template.body.clone();
        let mut filled = Vec::new();
        let mut missing = Vec::new();

        for (name, param_def) in &template.params {
            let value = Self::resolve_param(name, param_def, explicit_params, context);

            match value {
                Some(v) => {
                    // Substitute $param with value
                    dsl = Self::substitute_param(&dsl, name, &v);
                    filled.push(name.clone());
                }
                None if param_def.required => {
                    // Required but missing - add to missing list
                    missing.push(MissingParam {
                        name: name.clone(),
                        param_type: param_def.param_type.clone(),
                        prompt: param_def
                            .prompt
                            .clone()
                            .unwrap_or_else(|| format!("Value for {}", name)),
                        example: param_def.example.clone(),
                        required: true,
                        validation: param_def.validation.clone(),
                    });
                }
                None => {
                    // Optional and missing - try default
                    if let Some(default) = &param_def.default {
                        let resolved_default = Self::resolve_default(default, explicit_params);
                        dsl = Self::substitute_param(&dsl, name, &resolved_default);
                        filled.push(name.clone());
                    }
                    // If no default, leave placeholder or empty
                }
            }
        }

        // Final pass: substitute any remaining dotted property access patterns
        // like $fund_entity.name that were passed as explicit params
        dsl = Self::substitute_all_params(&dsl, explicit_params);

        let outputs = template.outputs.keys().cloned().collect();

        ExpansionResult {
            dsl,
            filled_params: filled,
            missing_params: missing,
            outputs,
            template_id: template.template.clone(),
        }
    }

    /// Substitute a parameter in the DSL text
    ///
    /// Handles both simple params ($param) and dotted property access ($param.property)
    fn substitute_param(dsl: &str, name: &str, value: &str) -> String {
        let mut result = dsl.to_string();

        // Handle "$param" (quoted)
        result = result.replace(&format!("\"${}\"", name), &format!("\"{}\"", value));

        // Handle $param (unquoted - for enum values, numbers, etc.)
        // Use word boundary check to avoid partial matches
        // e.g., don't replace $fund when we have $fund_entity
        let pattern = format!("${}", name);
        let mut new_result = String::new();
        let mut last_end = 0;

        for (start, _) in result.match_indices(&pattern) {
            // Check if this is a complete token (not followed by alphanumeric or underscore)
            let after_pattern = start + pattern.len();
            let next_char = result[after_pattern..].chars().next();

            // If followed by a dot, this is a property access - skip
            // If followed by alphanumeric/underscore, this is a different variable - skip
            let is_property_access = next_char == Some('.');
            let is_longer_name = next_char
                .map(|c| c.is_alphanumeric() || c == '_')
                .unwrap_or(false);

            if !is_property_access && !is_longer_name {
                new_result.push_str(&result[last_end..start]);
                new_result.push_str(value);
                last_end = after_pattern;
            }
        }
        new_result.push_str(&result[last_end..]);

        if !new_result.is_empty() {
            result = new_result;
        }

        result
    }

    /// Substitute all parameters including dotted property access
    ///
    /// Call this after processing individual params to handle $param.property patterns
    fn substitute_all_params(dsl: &str, explicit_params: &HashMap<String, String>) -> String {
        let mut result = dsl.to_string();

        // Sort by key length descending to substitute longer keys first
        // This prevents $fund from matching in $fund_entity.name
        let mut sorted_keys: Vec<_> = explicit_params.keys().collect();
        sorted_keys.sort_by_key(|k| std::cmp::Reverse(k.len()));

        for key in sorted_keys {
            if let Some(value) = explicit_params.get(key) {
                // Handle "$key" (quoted)
                result = result.replace(&format!("\"${}\"", key), &format!("\"{}\"", value));

                // Handle $key (unquoted) - but only if not followed by more identifier chars
                let pattern = format!("${}", key);
                let mut new_result = String::new();
                let mut last_end = 0;

                for (start, _) in result.match_indices(&pattern) {
                    let after_pattern = start + pattern.len();
                    let next_char = result[after_pattern..].chars().next();

                    // Only substitute if this is the complete token
                    let is_longer = next_char
                        .map(|c| c.is_alphanumeric() || c == '_' || c == '.')
                        .unwrap_or(false);

                    if !is_longer {
                        new_result.push_str(&result[last_end..start]);
                        new_result.push_str(value);
                        last_end = after_pattern;
                    }
                }
                new_result.push_str(&result[last_end..]);

                if !new_result.is_empty() {
                    result = new_result;
                }
            }
        }

        result
    }

    /// Resolve a parameter value
    fn resolve_param(
        name: &str,
        param_def: &ParamDefinition,
        explicit: &HashMap<String, String>,
        context: &ExpansionContext,
    ) -> Option<String> {
        // 1. Check explicit params first
        if let Some(v) = explicit.get(name) {
            return Some(v.clone());
        }

        // 2. Check source directive
        match param_def.source.as_deref() {
            Some("session") => {
                // Try to get from session context
                match name {
                    "cbu_id" | "cbu" => context.current_cbu.map(|u| u.to_string()),
                    "case_id" | "case" => context.current_case.map(|u| u.to_string()),
                    _ => context.bindings.get(name).cloned(),
                }
            }
            Some("blocker") => {
                // Would be populated by blocker context - check bindings
                context.bindings.get(name).cloned()
            }
            Some(ref_name) if ref_name.starts_with('$') => {
                // Reference another parameter: source: "$nationality"
                let ref_param = &ref_name[1..];
                explicit.get(ref_param).cloned()
            }
            _ => {
                // Check bindings as fallback
                context.bindings.get(name).cloned()
            }
        }
    }

    /// Resolve a default value
    fn resolve_default(default: &str, explicit: &HashMap<String, String>) -> String {
        if default == "today" {
            chrono::Utc::now().format("%Y-%m-%d").to_string()
        } else if let Some(ref_name) = default.strip_prefix('$') {
            // Reference another param
            explicit
                .get(ref_name)
                .cloned()
                .unwrap_or_else(|| default.to_string())
        } else {
            default.to_string()
        }
    }

    /// Check if expansion has all required params
    pub fn is_complete(result: &ExpansionResult) -> bool {
        result.missing_params.is_empty()
    }

    /// Format missing params as a prompt for the user
    pub fn format_missing_params_prompt(missing: &[MissingParam]) -> String {
        if missing.is_empty() {
            return String::new();
        }

        let mut prompt = String::from("Please provide the following:\n");
        for param in missing {
            prompt.push_str(&format!("- {}", param.prompt));
            if let Some(ref example) = param.example {
                prompt.push_str(&format!(" (e.g., {})", example));
            }
            prompt.push('\n');
        }
        prompt
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn sample_template() -> TemplateDefinition {
        serde_yaml::from_str(
            r#"
template: test-expand
version: 1
metadata:
  name: Test Expansion
  summary: Test template expansion
params:
  cbu_id:
    type: cbu_ref
    required: true
    source: session
  name:
    type: string
    required: true
    prompt: "Enter name"
    example: "John Smith"
  country:
    type: string
    required: false
    default: "US"
body: |
  (entity.create :cbu "$cbu_id" :name "$name" :country "$country")
"#,
        )
        .unwrap()
    }

    #[test]
    fn test_expand_with_all_params() {
        let template = sample_template();
        let mut params = HashMap::new();
        params.insert("name".to_string(), "Alice".to_string());

        let context = ExpansionContext::with_cbu(Uuid::now_v7());

        let result = TemplateExpander::expand(&template, &params, &context);

        assert!(result.missing_params.is_empty());
        assert!(result.dsl.contains("Alice"));
        assert!(result.dsl.contains("US")); // Default value
    }

    #[test]
    fn test_expand_missing_required() {
        let template = sample_template();
        let params = HashMap::new();
        let context = ExpansionContext::new(); // No CBU

        let result = TemplateExpander::expand(&template, &params, &context);

        // Should have 2 missing: cbu_id (no session) and name (required, no value)
        assert_eq!(result.missing_params.len(), 2);
        assert!(result.missing_params.iter().any(|p| p.name == "cbu_id"));
        assert!(result.missing_params.iter().any(|p| p.name == "name"));
    }

    #[test]
    fn test_expand_with_session_context() {
        let template = sample_template();
        let mut params = HashMap::new();
        params.insert("name".to_string(), "Bob".to_string());

        let cbu_id = Uuid::now_v7();
        let context = ExpansionContext::with_cbu(cbu_id);

        let result = TemplateExpander::expand(&template, &params, &context);

        assert!(result.missing_params.is_empty());
        assert!(result.dsl.contains(&cbu_id.to_string()));
    }

    #[test]
    fn test_format_missing_params() {
        let missing = vec![
            MissingParam {
                name: "name".to_string(),
                param_type: "string".to_string(),
                prompt: "Enter name".to_string(),
                example: Some("John".to_string()),
                required: true,
                validation: None,
            },
            MissingParam {
                name: "date".to_string(),
                param_type: "date".to_string(),
                prompt: "Enter date".to_string(),
                example: None,
                required: true,
                validation: None,
            },
        ];

        let prompt = TemplateExpander::format_missing_params_prompt(&missing);
        assert!(prompt.contains("Enter name"));
        assert!(prompt.contains("John"));
        assert!(prompt.contains("Enter date"));
    }

    #[test]
    fn test_context_binding_type_matching() {
        let mut ctx = ExpansionContext::new();
        ctx.bind_typed("company", "uuid-123", "entity.limited_company");

        // Exact match
        assert!(ctx.binding_matches_type("company", "entity.limited_company"));

        // Base type match
        assert!(ctx.binding_matches_type("company", "entity"));

        // Non-match
        assert!(!ctx.binding_matches_type("company", "cbu"));
        assert!(!ctx.binding_matches_type("nonexistent", "entity"));
    }
}
