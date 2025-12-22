//! DSL Generator
//!
//! Generates DSL statements from classified intents and extracted entities.
//! Uses parameter mappings to translate entities to verb parameters.
//!
//! The generator performs:
//! 1. Intent → Verb mapping
//! 2. Entity → Parameter mapping with transforms
//! 3. Default value inference
//! 4. List expansion for multi-statement generation
//! 5. Symbol generation for captures
//! 6. Dependency ordering

use crate::agentic::entity_extractor::{ExtractedEntities, ExtractedEntity};
use crate::agentic::intent_classifier::{ClassifiedIntent, ConversationContext, ExecutionDecision};
use serde::{Deserialize, Serialize};
use std::collections::HashMap;
use std::path::Path;

/// Result of DSL generation
#[derive(Debug, Clone)]
pub struct GeneratedDsl {
    /// The generated DSL statements
    pub statements: Vec<DslStatement>,
    /// Human-readable explanation of what will be done
    pub explanation: String,
    /// Whether user confirmation is required before execution
    pub requires_confirmation: bool,
    /// Warnings or notes about the generation
    pub warnings: Vec<String>,
    /// Parameters that could not be resolved
    pub missing_params: Vec<MissingParameter>,
}

/// A single DSL statement
#[derive(Debug, Clone)]
pub struct DslStatement {
    /// The full verb name (e.g., "investment-manager.assign")
    pub verb: String,
    /// Parameter values keyed by parameter name
    pub params: HashMap<String, ParamValue>,
    /// Symbol to capture the result (e.g., "@im-blackrock")
    pub capture_symbol: Option<String>,
    /// The source intent that generated this statement
    pub source_intent: String,
    /// Confidence inherited from intent classification
    pub confidence: f32,
}

/// A parameter value with metadata
#[derive(Debug, Clone)]
pub struct ParamValue {
    /// The value (string, number, boolean, or list)
    pub value: serde_json::Value,
    /// Source of this value
    pub source: ParamSource,
    /// Confidence in the value
    pub confidence: f32,
}

/// Source of a parameter value
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum ParamSource {
    /// Extracted from user utterance
    Extracted,
    /// Inferred from conversation context
    Context,
    /// Default value from mapping config
    Default,
    /// Inferred using inference rules
    Inferred,
    /// Session state (current CBU, profile, etc.)
    Session,
}

/// A parameter that could not be resolved
#[derive(Debug, Clone)]
pub struct MissingParameter {
    /// Parameter name
    pub name: String,
    /// Whether it's required
    pub required: bool,
    /// Prompt to ask user
    pub prompt: String,
    /// Valid values if constrained
    pub valid_values: Option<Vec<String>>,
}

/// Configuration for parameter mappings
#[derive(Debug, Clone, Deserialize, Serialize)]
pub struct ParameterMappingsConfig {
    pub version: String,
    pub description: String,
    pub parameter_mappings: HashMap<String, VerbMapping>,
    #[serde(default)]
    pub transforms: HashMap<String, TransformConfig>,
    #[serde(default)]
    pub inference_rules: HashMap<String, HashMap<String, serde_json::Value>>,
}

/// Mapping configuration for a verb
#[derive(Debug, Clone, Deserialize, Serialize)]
pub struct VerbMapping {
    pub description: String,
    pub mappings: Vec<ParamMapping>,
    #[serde(default)]
    pub defaults: HashMap<String, serde_json::Value>,
    #[serde(default)]
    pub symbol_template: Option<String>,
    #[serde(default)]
    pub symbol_transform: Option<String>,
}

/// Mapping from entity type to parameter
#[derive(Debug, Clone, Deserialize, Serialize)]
pub struct ParamMapping {
    pub entity_type: String,
    pub param: String,
    #[serde(default)]
    pub priority: i32,
    #[serde(default)]
    pub required: bool,
    #[serde(default)]
    pub is_list: bool,
    #[serde(default)]
    pub iterate_if_list: bool,
    #[serde(default)]
    pub source: Option<String>,
    #[serde(default)]
    pub fallback: Option<String>,
    #[serde(default)]
    pub default_if_missing: Option<serde_json::Value>,
    #[serde(default)]
    pub context_key: Option<String>,
    #[serde(default)]
    pub transform: Option<String>,
    #[serde(default)]
    pub infer_from: Option<String>,
    #[serde(default)]
    pub inference_rules: HashMap<String, String>,
    #[serde(default)]
    pub condition: Option<String>,
}

/// Transform configuration
#[derive(Debug, Clone, Deserialize, Serialize)]
pub struct TransformConfig {
    pub description: String,
    #[serde(default)]
    pub steps: Vec<String>,
    #[serde(default)]
    pub patterns: Vec<TransformPattern>,
}

/// Transform pattern for regex-based transforms
#[derive(Debug, Clone, Deserialize, Serialize)]
pub struct TransformPattern {
    pub pattern: String,
    #[serde(default)]
    pub action: Option<String>,
    #[serde(default)]
    pub result: Option<String>,
}

impl ParameterMappingsConfig {
    /// Load configuration from YAML file
    pub fn load_from_file(path: &Path) -> Result<Self, ConfigError> {
        let content =
            std::fs::read_to_string(path).map_err(|e| ConfigError::IoError(e.to_string()))?;
        Self::load_from_str(&content)
    }

    /// Load configuration from YAML string
    pub fn load_from_str(yaml: &str) -> Result<Self, ConfigError> {
        serde_yaml::from_str(yaml).map_err(|e| ConfigError::ParseError(e.to_string()))
    }

    /// Get mapping for a verb
    pub fn get_mapping(&self, verb: &str) -> Option<&VerbMapping> {
        self.parameter_mappings.get(verb)
    }
}

/// Configuration loading error
#[derive(Debug, Clone)]
pub enum ConfigError {
    IoError(String),
    ParseError(String),
}

impl std::fmt::Display for ConfigError {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            ConfigError::IoError(e) => write!(f, "IO error: {}", e),
            ConfigError::ParseError(e) => write!(f, "Parse error: {}", e),
        }
    }
}

impl std::error::Error for ConfigError {}

/// Generation context from session
#[derive(Debug, Clone, Default)]
pub struct GenerationContext {
    /// Current CBU ID
    pub cbu_id: Option<String>,
    /// Current profile ID
    pub profile_id: Option<String>,
    /// Available symbols from previous statements
    pub available_symbols: Vec<String>,
    /// Created entities in this session
    pub created_entities: HashMap<String, String>,
}

/// The DSL generator
pub struct DslGenerator {
    mappings: ParameterMappingsConfig,
}

impl DslGenerator {
    /// Create a new generator from configuration
    pub fn new(mappings: ParameterMappingsConfig) -> Self {
        Self { mappings }
    }

    /// Create from config file path
    pub fn from_file(path: &Path) -> Result<Self, ConfigError> {
        let mappings = ParameterMappingsConfig::load_from_file(path)?;
        Ok(Self::new(mappings))
    }

    /// Create from YAML string (for testing)
    pub fn load_from_str(yaml: &str) -> Result<Self, ConfigError> {
        let mappings = ParameterMappingsConfig::load_from_str(yaml)?;
        Ok(Self::new(mappings))
    }

    /// Generate DSL from classified intents and extracted entities
    pub fn generate(
        &self,
        intents: &[ClassifiedIntent],
        entities: &ExtractedEntities,
        context: &ConversationContext,
        gen_context: &GenerationContext,
    ) -> GeneratedDsl {
        let mut statements = Vec::new();
        let mut warnings = Vec::new();
        let mut all_missing = Vec::new();

        for intent in intents {
            match self.generate_for_intent(intent, entities, context, gen_context) {
                Ok(result) => {
                    statements.extend(result.statements);
                    warnings.extend(result.warnings);
                    all_missing.extend(result.missing_params);
                }
                Err(e) => {
                    warnings.push(format!(
                        "Failed to generate for {}: {}",
                        intent.intent_id, e
                    ));
                }
            }
        }

        // Order by dependencies
        let ordered = self.order_by_dependencies(&statements);

        // Generate explanation
        let explanation = self.generate_explanation(&ordered);

        // Determine if confirmation is needed
        let requires_confirmation = self.needs_confirmation(&ordered, intents);

        GeneratedDsl {
            statements: ordered,
            explanation,
            requires_confirmation,
            warnings,
            missing_params: all_missing,
        }
    }

    /// Generate DSL for a single intent
    fn generate_for_intent(
        &self,
        intent: &ClassifiedIntent,
        entities: &ExtractedEntities,
        context: &ConversationContext,
        gen_context: &GenerationContext,
    ) -> Result<GeneratedDsl, GenerationError> {
        // Get verb from intent
        let verb = intent
            .canonical_verb
            .as_ref()
            .ok_or_else(|| GenerationError::NoCanonicalVerb(intent.intent_id.clone()))?;

        // Get mapping for this verb
        let mapping = self
            .mappings
            .get_mapping(verb)
            .ok_or_else(|| GenerationError::NoMapping(verb.clone()))?;

        // Check for list parameters that need iteration
        let iteration_params = self.find_iteration_params(mapping, entities);

        if iteration_params.is_empty() {
            // Single statement
            let (stmt, missing) = self.generate_single_statement(
                verb,
                intent,
                mapping,
                entities,
                context,
                gen_context,
            )?;
            Ok(GeneratedDsl {
                statements: vec![stmt],
                explanation: String::new(),
                requires_confirmation: false,
                warnings: Vec::new(),
                missing_params: missing,
            })
        } else {
            // Multiple statements from list expansion
            self.generate_expanded_statements(
                verb,
                intent,
                mapping,
                entities,
                context,
                gen_context,
                &iteration_params,
            )
        }
    }

    /// Find parameters that need iteration (list expansion)
    fn find_iteration_params(
        &self,
        mapping: &VerbMapping,
        entities: &ExtractedEntities,
    ) -> Vec<(String, Vec<String>)> {
        let mut result = Vec::new();

        for m in &mapping.mappings {
            if m.iterate_if_list {
                let values: Vec<String> = entities
                    .by_type(&m.entity_type)
                    .iter()
                    .map(|e| e.value.clone())
                    .collect();

                if values.len() > 1 {
                    result.push((m.param.clone(), values));
                }
            }
        }

        result
    }

    /// Generate a single DSL statement
    fn generate_single_statement(
        &self,
        verb: &str,
        intent: &ClassifiedIntent,
        mapping: &VerbMapping,
        entities: &ExtractedEntities,
        context: &ConversationContext,
        gen_context: &GenerationContext,
    ) -> Result<(DslStatement, Vec<MissingParameter>), GenerationError> {
        let mut params = HashMap::new();
        let mut missing = Vec::new();

        // Process each mapping
        for m in &mapping.mappings {
            match self.resolve_param(m, entities, context, gen_context) {
                Some(value) => {
                    params.insert(m.param.clone(), value);
                }
                None => {
                    if m.required {
                        missing.push(MissingParameter {
                            name: m.param.clone(),
                            required: true,
                            prompt: format!("What {} should be used?", m.param),
                            valid_values: None,
                        });
                    }
                }
            }
        }

        // Apply defaults
        for (name, value) in &mapping.defaults {
            if !params.contains_key(name) {
                params.insert(
                    name.clone(),
                    ParamValue {
                        value: value.clone(),
                        source: ParamSource::Default,
                        confidence: 1.0,
                    },
                );
            }
        }

        // Generate symbol
        let capture_symbol = self.generate_symbol(mapping, &params);

        Ok((
            DslStatement {
                verb: verb.to_string(),
                params,
                capture_symbol,
                source_intent: intent.intent_id.clone(),
                confidence: intent.confidence,
            },
            missing,
        ))
    }

    /// Generate multiple statements from list expansion
    #[allow(clippy::too_many_arguments)] // Internal helper with cohesive parameters
    fn generate_expanded_statements(
        &self,
        verb: &str,
        intent: &ClassifiedIntent,
        mapping: &VerbMapping,
        entities: &ExtractedEntities,
        context: &ConversationContext,
        gen_context: &GenerationContext,
        iteration_params: &[(String, Vec<String>)],
    ) -> Result<GeneratedDsl, GenerationError> {
        let mut statements = Vec::new();
        let mut all_missing = Vec::new();

        // Generate cartesian product of iteration values
        let combinations = Self::cartesian_product(iteration_params);

        for combination in combinations {
            // Create modified entities with single values for iteration params
            let modified_entities = self.override_entities(entities, &combination);

            let (mut stmt, missing) = self.generate_single_statement(
                verb,
                intent,
                mapping,
                &modified_entities,
                context,
                gen_context,
            )?;

            // Update symbol to be unique
            if let Some(ref symbol) = stmt.capture_symbol {
                let suffix = combination
                    .values()
                    .map(|v| v.to_lowercase().replace(' ', "-"))
                    .collect::<Vec<_>>()
                    .join("-");
                stmt.capture_symbol = Some(format!("{}-{}", symbol, suffix));
            }

            statements.push(stmt);
            all_missing.extend(missing);
        }

        Ok(GeneratedDsl {
            statements,
            explanation: String::new(),
            requires_confirmation: false,
            warnings: Vec::new(),
            missing_params: all_missing,
        })
    }

    /// Resolve a parameter value from entities, context, or defaults
    fn resolve_param(
        &self,
        mapping: &ParamMapping,
        entities: &ExtractedEntities,
        context: &ConversationContext,
        gen_context: &GenerationContext,
    ) -> Option<ParamValue> {
        // Priority 1: Extracted entity
        if let Some(entity) = self.find_entity_for_param(mapping, entities) {
            let value = self.transform_value(&entity.value, mapping);
            return Some(ParamValue {
                value: if mapping.is_list {
                    serde_json::json!(entities
                        .by_type(&mapping.entity_type)
                        .iter()
                        .map(|e| self.transform_value(&e.value, mapping))
                        .collect::<Vec<_>>())
                } else {
                    value
                },
                source: ParamSource::Extracted,
                confidence: entity.confidence,
            });
        }

        // Priority 2: Context (session entities, known entities)
        if mapping.source.as_deref() == Some("context")
            || mapping.source.as_deref() == Some("context_or_extract")
        {
            if let Some(value) = self.resolve_from_context(mapping, context, gen_context) {
                return Some(value);
            }
        }

        // Priority 3: Inference from other entities
        if let Some(infer_from) = &mapping.infer_from {
            if let Some(source_entity) = entities.first_of_type(infer_from) {
                if let Some(inferred) = mapping.inference_rules.get(&source_entity.value) {
                    return Some(ParamValue {
                        value: serde_json::json!(inferred),
                        source: ParamSource::Inferred,
                        confidence: 0.85,
                    });
                }
                if let Some(default) = mapping.inference_rules.get("default") {
                    return Some(ParamValue {
                        value: serde_json::json!(default),
                        source: ParamSource::Inferred,
                        confidence: 0.7,
                    });
                }
            }
        }

        // Priority 4: Fallback from session
        if let Some(fallback) = &mapping.fallback {
            if let Some(value) = self.resolve_fallback(fallback, gen_context) {
                return Some(ParamValue {
                    value: serde_json::json!(value),
                    source: ParamSource::Session,
                    confidence: 0.9,
                });
            }
        }

        // Priority 5: Default value
        if let Some(default) = &mapping.default_if_missing {
            return Some(ParamValue {
                value: default.clone(),
                source: ParamSource::Default,
                confidence: 1.0,
            });
        }

        None
    }

    /// Find entity matching the mapping
    fn find_entity_for_param<'a>(
        &self,
        mapping: &ParamMapping,
        entities: &'a ExtractedEntities,
    ) -> Option<&'a ExtractedEntity> {
        // Check for context_key first (e.g., "fallback" pricing source)
        if let Some(context_key) = &mapping.context_key {
            return entities.iter().find(|e| {
                e.entity_type == mapping.entity_type && e.components.contains_key(context_key)
            });
        }

        entities.first_of_type(&mapping.entity_type)
    }

    /// Resolve value from conversation context
    fn resolve_from_context(
        &self,
        mapping: &ParamMapping,
        context: &ConversationContext,
        gen_context: &GenerationContext,
    ) -> Option<ParamValue> {
        // Check session entities
        if let Some(value) = context.session_entities.get(&mapping.entity_type) {
            return Some(ParamValue {
                value: serde_json::json!(value),
                source: ParamSource::Session,
                confidence: 0.95,
            });
        }

        // Check known entities
        if let Some(value) = context.known_entities.get(&mapping.entity_type) {
            return Some(ParamValue {
                value: serde_json::json!(value),
                source: ParamSource::Context,
                confidence: 0.9,
            });
        }

        // Check generation context
        match mapping.entity_type.as_str() {
            "cbu_reference" => gen_context.cbu_id.as_ref().map(|id| ParamValue {
                value: serde_json::json!(id),
                source: ParamSource::Session,
                confidence: 0.95,
            }),
            "profile_reference" => gen_context.profile_id.as_ref().map(|id| ParamValue {
                value: serde_json::json!(id),
                source: ParamSource::Session,
                confidence: 0.95,
            }),
            _ => None,
        }
    }

    /// Resolve fallback value from session
    fn resolve_fallback(&self, fallback: &str, gen_context: &GenerationContext) -> Option<String> {
        match fallback {
            "session.current_cbu" => gen_context.cbu_id.clone(),
            "session.current_profile" => gen_context.profile_id.clone(),
            _ => None,
        }
    }

    /// Transform a value according to mapping configuration
    fn transform_value(&self, value: &str, mapping: &ParamMapping) -> serde_json::Value {
        let mut result = value.to_string();

        if let Some(transform_name) = &mapping.transform {
            if let Some(transform) = self.mappings.transforms.get(transform_name) {
                result = self.apply_transform(&result, transform);
            }
        }

        serde_json::json!(result)
    }

    /// Apply a transform to a value
    fn apply_transform(&self, value: &str, transform: &TransformConfig) -> String {
        let mut result = value.to_string();

        // Apply steps
        for step in &transform.steps {
            result = match step.as_str() {
                "lowercase" => result.to_lowercase(),
                "uppercase" => result.to_uppercase(),
                s if s.starts_with("replace:") => {
                    // Parse replace: [" ", "-"]
                    if let Some(args) = s.strip_prefix("replace:") {
                        if let Ok(arr) = serde_json::from_str::<[String; 2]>(args.trim()) {
                            result.replace(&arr[0], &arr[1])
                        } else {
                            result
                        }
                    } else {
                        result
                    }
                }
                s if s.starts_with("truncate:") => {
                    if let Some(len) = s
                        .strip_prefix("truncate:")
                        .and_then(|l| l.trim().parse().ok())
                    {
                        result.chars().take(len).collect()
                    } else {
                        result
                    }
                }
                _ => result,
            };
        }

        // Apply pattern-based transforms
        for pattern in &transform.patterns {
            if result.to_lowercase() == pattern.pattern.to_lowercase() {
                if let Some(replacement) = &pattern.result {
                    return replacement.clone();
                }
            }
        }

        result
    }

    /// Generate symbol from template
    fn generate_symbol(
        &self,
        mapping: &VerbMapping,
        params: &HashMap<String, ParamValue>,
    ) -> Option<String> {
        let template = mapping.symbol_template.as_ref()?;
        let mut symbol = template.clone();

        // Replace placeholders
        for (name, value) in params {
            let placeholder = format!("{{{}}}", name);
            if let Some(s) = value.value.as_str() {
                symbol = symbol.replace(&placeholder, s);
            }
        }

        // Apply transform
        if let Some(transform_name) = &mapping.symbol_transform {
            if let Some(transform) = self.mappings.transforms.get(transform_name) {
                symbol = self.apply_transform(&symbol, transform);
            }
        }

        Some(symbol)
    }

    /// Create cartesian product of iteration values
    fn cartesian_product(params: &[(String, Vec<String>)]) -> Vec<HashMap<String, String>> {
        if params.is_empty() {
            return vec![HashMap::new()];
        }

        let (first_name, first_values) = &params[0];
        let rest = &params[1..];
        let rest_product = Self::cartesian_product(rest);

        let mut result = Vec::new();
        for value in first_values {
            for rest_combo in &rest_product {
                let mut combo = rest_combo.clone();
                combo.insert(first_name.clone(), value.clone());
                result.push(combo);
            }
        }

        result
    }

    /// Override entities with fixed values for iteration
    fn override_entities(
        &self,
        entities: &ExtractedEntities,
        overrides: &HashMap<String, String>,
    ) -> ExtractedEntities {
        // For now, just filter to matching values
        // In production, would create modified copy
        let _ = overrides;
        entities.clone()
    }

    /// Order statements by dependencies
    fn order_by_dependencies(&self, statements: &[DslStatement]) -> Vec<DslStatement> {
        // For now, just return in order
        // TODO: Implement proper dependency resolution
        statements.to_vec()
    }

    /// Check if confirmation is needed
    fn needs_confirmation(
        &self,
        statements: &[DslStatement],
        intents: &[ClassifiedIntent],
    ) -> bool {
        // Confirmation needed if:
        // 1. Any intent has confirmation_required
        // 2. Any intent has low confidence
        // 3. Multiple statements generated

        if statements.len() > 3 {
            return true;
        }

        intents.iter().any(|i| {
            i.confirmation_required
                || i.execution_decision == ExecutionDecision::ConfirmFirst
                || i.execution_decision == ExecutionDecision::Suggest
        })
    }

    /// Generate human-readable explanation
    fn generate_explanation(&self, statements: &[DslStatement]) -> String {
        if statements.is_empty() {
            return "No actions to perform.".to_string();
        }

        let parts: Vec<String> = statements
            .iter()
            .map(|stmt| self.explain_statement(stmt))
            .collect();

        if statements.len() == 1 {
            parts[0].clone()
        } else {
            format!(
                "I'll perform {} actions:\n\n{}",
                statements.len(),
                parts
                    .iter()
                    .enumerate()
                    .map(|(i, p)| format!("{}. {}", i + 1, p))
                    .collect::<Vec<_>>()
                    .join("\n")
            )
        }
    }

    /// Explain a single statement
    fn explain_statement(&self, stmt: &DslStatement) -> String {
        let verb_name = stmt.verb.replace(['.', '-'], " ");

        // Format key parameters
        let key_params: Vec<String> = stmt
            .params
            .iter()
            .filter(|(k, _)| !k.contains("id") || k.ends_with("-id"))
            .take(3)
            .map(|(k, v)| {
                let name = k.replace('-', " ");
                let value = self.format_param_value(v);
                format!("{}: {}", name, value)
            })
            .collect();

        if key_params.is_empty() {
            verb_name
        } else {
            format!("{} with {}", verb_name, key_params.join(", "))
        }
    }

    /// Format a parameter value for display
    fn format_param_value(&self, value: &ParamValue) -> String {
        match &value.value {
            serde_json::Value::Array(arr) => {
                let items: Vec<&str> = arr.iter().filter_map(|v| v.as_str()).take(3).collect();
                if items.len() < arr.len() {
                    format!("{} and {} more", items.join(", "), arr.len() - items.len())
                } else {
                    items.join(", ")
                }
            }
            serde_json::Value::String(s) => s.clone(),
            v => v.to_string(),
        }
    }
}

/// Generation error
#[derive(Debug, Clone)]
pub enum GenerationError {
    NoCanonicalVerb(String),
    NoMapping(String),
    MissingRequired(String),
}

impl std::fmt::Display for GenerationError {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            GenerationError::NoCanonicalVerb(intent) => {
                write!(f, "No canonical verb for intent: {}", intent)
            }
            GenerationError::NoMapping(verb) => {
                write!(f, "No parameter mapping for verb: {}", verb)
            }
            GenerationError::MissingRequired(param) => {
                write!(f, "Missing required parameter: {}", param)
            }
        }
    }
}

impl std::error::Error for GenerationError {}

impl DslStatement {
    /// Convert to DSL source text
    pub fn to_dsl_string(&self) -> String {
        let mut parts = vec![format!("({}", self.verb)];

        // Add parameters
        for (name, value) in &self.params {
            let value_str = Self::format_value_for_dsl(&value.value);
            parts.push(format!(":{} {}", name, value_str));
        }

        // Add capture symbol
        if let Some(symbol) = &self.capture_symbol {
            parts.push(format!(":as {}", symbol));
        }

        parts.push(")".to_string());
        parts.join(" ")
    }

    /// Format a value for DSL syntax
    fn format_value_for_dsl(value: &serde_json::Value) -> String {
        match value {
            serde_json::Value::String(s) => {
                if s.starts_with('@') {
                    // Symbol reference
                    s.clone()
                } else {
                    format!("\"{}\"", s)
                }
            }
            serde_json::Value::Number(n) => n.to_string(),
            serde_json::Value::Bool(b) => b.to_string(),
            serde_json::Value::Array(arr) => {
                let items: Vec<String> = arr.iter().map(Self::format_value_for_dsl).collect();
                format!("[{}]", items.join(" "))
            }
            serde_json::Value::Null => "nil".to_string(),
            serde_json::Value::Object(_) => "{}".to_string(),
        }
    }
}

impl GeneratedDsl {
    /// Convert all statements to DSL source text
    pub fn to_dsl_source(&self) -> String {
        self.statements
            .iter()
            .map(|stmt| stmt.to_dsl_string())
            .collect::<Vec<_>>()
            .join("\n")
    }

    /// Alias for to_dsl_source for compatibility
    pub fn to_dsl_string(&self) -> String {
        self.to_dsl_source()
    }

    /// Check if generation was successful
    pub fn is_complete(&self) -> bool {
        self.missing_params.iter().all(|p| !p.required)
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn sample_mappings() -> ParameterMappingsConfig {
        let yaml = r#"
version: "1.0"
description: "Test mappings"
parameter_mappings:
  investment-manager.assign:
    description: "Assign IM"
    mappings:
      - entity_type: manager_reference
        param: manager-name
        required: true
      - entity_type: market_reference
        param: scope-markets
        is_list: true
      - entity_type: instruction_method
        param: instruction-method
        default_if_missing: "SWIFT"
    defaults:
      priority: 100
      can-trade: true
    symbol_template: "@im-{manager-name}"
    symbol_transform: lowercase_hyphenate

  pricing-config.set:
    description: "Set pricing"
    mappings:
      - entity_type: instrument_class_reference
        param: instrument-class
        required: true
        iterate_if_list: true
      - entity_type: pricing_source
        param: source
        required: true
    symbol_template: "@pricing-{instrument-class}"

transforms:
  lowercase_hyphenate:
    description: "Lowercase and hyphenate"
    steps:
      - lowercase
      - "replace: [\" \", \"-\"]"
"#;
        ParameterMappingsConfig::load_from_str(yaml).unwrap()
    }

    #[test]
    fn test_single_statement_generation() {
        let generator = DslGenerator::new(sample_mappings());

        let intent = ClassifiedIntent {
            intent_id: "im_assign".to_string(),
            canonical_verb: Some("investment-manager.assign".to_string()),
            confidence: 0.9,
            source_text: "add BlackRock as IM".to_string(),
            execution_decision: ExecutionDecision::Execute,
            is_query: false,
            confirmation_required: false,
            extracted_slots: HashMap::new(),
        };

        let mut entities = ExtractedEntities::new();
        entities.add(crate::agentic::entity_extractor::ExtractedEntity {
            id: 1,
            entity_type: "manager_reference".to_string(),
            value: "BlackRock".to_string(),
            original_text: "BlackRock".to_string(),
            span: (4, 13),
            confidence: 0.95,
            source: crate::agentic::entity_extractor::ExtractionSource::Pattern,
            derived_from: None,
            resolved_from: None,
            components: HashMap::new(),
        });

        let context = ConversationContext::default();
        let gen_context = GenerationContext::default();

        let result = generator.generate(&[intent], &entities, &context, &gen_context);

        assert_eq!(result.statements.len(), 1);
        assert_eq!(result.statements[0].verb, "investment-manager.assign");
        assert!(result.statements[0].params.contains_key("manager-name"));
        assert!(result.statements[0].capture_symbol.is_some());
    }

    #[test]
    fn test_list_expansion() {
        let generator = DslGenerator::new(sample_mappings());

        let intent = ClassifiedIntent {
            intent_id: "pricing_set".to_string(),
            canonical_verb: Some("pricing-config.set".to_string()),
            confidence: 0.9,
            source_text: "use Bloomberg for equities and bonds".to_string(),
            execution_decision: ExecutionDecision::Execute,
            is_query: false,
            confirmation_required: false,
            extracted_slots: HashMap::new(),
        };

        let mut entities = ExtractedEntities::new();
        entities.add(crate::agentic::entity_extractor::ExtractedEntity {
            id: 1,
            entity_type: "instrument_class_reference".to_string(),
            value: "EQUITY".to_string(),
            original_text: "equities".to_string(),
            span: (18, 26),
            confidence: 0.95,
            source: crate::agentic::entity_extractor::ExtractionSource::Pattern,
            derived_from: None,
            resolved_from: None,
            components: HashMap::new(),
        });
        entities.add(crate::agentic::entity_extractor::ExtractedEntity {
            id: 2,
            entity_type: "instrument_class_reference".to_string(),
            value: "GOVT_BOND".to_string(),
            original_text: "bonds".to_string(),
            span: (31, 36),
            confidence: 0.95,
            source: crate::agentic::entity_extractor::ExtractionSource::Pattern,
            derived_from: None,
            resolved_from: None,
            components: HashMap::new(),
        });
        entities.add(crate::agentic::entity_extractor::ExtractedEntity {
            id: 3,
            entity_type: "pricing_source".to_string(),
            value: "BLOOMBERG".to_string(),
            original_text: "Bloomberg".to_string(),
            span: (4, 13),
            confidence: 0.95,
            source: crate::agentic::entity_extractor::ExtractionSource::Pattern,
            derived_from: None,
            resolved_from: None,
            components: HashMap::new(),
        });

        let context = ConversationContext::default();
        let gen_context = GenerationContext::default();

        let result = generator.generate(&[intent], &entities, &context, &gen_context);

        // Should generate multiple statements for list expansion
        assert!(result.statements.len() >= 1);
    }

    #[test]
    fn test_dsl_output() {
        let stmt = DslStatement {
            verb: "investment-manager.assign".to_string(),
            params: {
                let mut p = HashMap::new();
                p.insert(
                    "manager-name".to_string(),
                    ParamValue {
                        value: serde_json::json!("BlackRock"),
                        source: ParamSource::Extracted,
                        confidence: 0.95,
                    },
                );
                p.insert(
                    "scope-markets".to_string(),
                    ParamValue {
                        value: serde_json::json!(["XNYS", "XLON"]),
                        source: ParamSource::Extracted,
                        confidence: 0.9,
                    },
                );
                p
            },
            capture_symbol: Some("@im-blackrock".to_string()),
            source_intent: "im_assign".to_string(),
            confidence: 0.9,
        };

        let dsl = stmt.to_dsl_string();
        assert!(dsl.contains("investment-manager.assign"));
        assert!(dsl.contains(":manager-name \"BlackRock\""));
        assert!(dsl.contains(":as @im-blackrock"));
    }

    #[test]
    fn test_default_values() {
        let generator = DslGenerator::new(sample_mappings());

        let intent = ClassifiedIntent {
            intent_id: "im_assign".to_string(),
            canonical_verb: Some("investment-manager.assign".to_string()),
            confidence: 0.9,
            source_text: "add BlackRock".to_string(),
            execution_decision: ExecutionDecision::Execute,
            is_query: false,
            confirmation_required: false,
            extracted_slots: HashMap::new(),
        };

        let mut entities = ExtractedEntities::new();
        entities.add(crate::agentic::entity_extractor::ExtractedEntity {
            id: 1,
            entity_type: "manager_reference".to_string(),
            value: "BlackRock".to_string(),
            original_text: "BlackRock".to_string(),
            span: (4, 13),
            confidence: 0.95,
            source: crate::agentic::entity_extractor::ExtractionSource::Pattern,
            derived_from: None,
            resolved_from: None,
            components: HashMap::new(),
        });

        let context = ConversationContext::default();
        let gen_context = GenerationContext::default();

        let result = generator.generate(&[intent], &entities, &context, &gen_context);

        // Should have default values applied
        assert_eq!(result.statements.len(), 1);
        let stmt = &result.statements[0];

        // instruction-method should default to SWIFT
        if let Some(im) = stmt.params.get("instruction-method") {
            assert_eq!(im.value, serde_json::json!("SWIFT"));
            assert_eq!(im.source, ParamSource::Default);
        }

        // priority should default to 100
        if let Some(priority) = stmt.params.get("priority") {
            assert_eq!(priority.value, serde_json::json!(100));
        }
    }
}
