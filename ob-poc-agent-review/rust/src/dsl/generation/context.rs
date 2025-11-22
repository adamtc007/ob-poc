//! Generation Context Module
//!
//! This module provides context management for DSL generation,
//! including template variable resolution and AI context enhancement.

use crate::dsl::orchestration_interface::OrchestrationContext;
use serde::{Deserialize, Serialize};
use std::collections::HashMap;
use uuid::Uuid;

/// Context builder for DSL generation
#[derive(Debug, Clone)]
pub struct GenerationContextBuilder {
    cbu_id: Option<String>,
    case_id: Option<String>,
    entity_data: HashMap<String, String>,
    domain: Option<String>,
    instruction: Option<String>,
    template_variables: HashMap<String, String>,
    orchestration_context: Option<OrchestrationContext>,
}

/// Context resolver for different generation methods
#[derive(Debug, Clone)]
pub struct ContextResolver {
    /// Variable substitution rules
    substitution_rules: HashMap<String, String>,

    /// Default values for common variables
    default_values: HashMap<String, String>,

    /// Context validation rules
    validation_rules: Vec<ContextValidationRule>,
}

/// Context validation rule
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ContextValidationRule {
    /// Rule name
    pub name: String,

    /// Required variables for this rule
    pub required_variables: Vec<String>,

    /// Optional variables
    pub optional_variables: Vec<String>,

    /// Context conditions that must be met
    pub conditions: Vec<ContextCondition>,

    /// Error message if validation fails
    pub error_message: String,
}

/// Context condition for validation
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ContextCondition {
    /// Variable name to check
    pub variable: String,

    /// Condition type
    pub condition_type: ContextConditionType,

    /// Expected value(s)
    pub expected_values: Vec<String>,
}

/// Types of context conditions
#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq)]
pub enum ContextConditionType {
    /// Variable must equal one of the expected values
    Equals,

    /// Variable must not equal any of the expected values
    NotEquals,

    /// Variable must contain one of the expected values
    Contains,

    /// Variable must not be empty
    NotEmpty,

    /// Variable must match a regex pattern
    Matches,

    /// Variable must be a valid UUID
    ValidUuid,

    /// Variable must be a valid email
    ValidEmail,

    /// Variable must be a valid date
    ValidDate,
}

/// Context enhancement result
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ContextEnhancementResult {
    /// Enhanced context data
    pub enhanced_context: super::traits::GenerationContext,

    /// Variables that were added or modified
    pub modifications: Vec<ContextModification>,

    /// Any warnings during enhancement
    pub warnings: Vec<String>,

    /// Enhancement metadata
    pub metadata: HashMap<String, String>,
}

/// Context modification record
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ContextModification {
    /// Variable name
    pub variable: String,

    /// Modification type
    pub modification_type: ModificationType,

    /// Old value (if any)
    pub old_value: Option<String>,

    /// New value
    pub new_value: String,

    /// Reason for modification
    pub reason: String,
}

/// Types of context modifications
#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq)]
pub enum ModificationType {
    /// Variable was added
    Added,

    /// Variable value was updated
    Updated,

    /// Variable was removed
    Removed,

    /// Variable was normalized (format change)
    Normalized,

    /// Variable was generated automatically
    Generated,
}

impl GenerationContextBuilder {
    /// Create a new context builder
    pub fn new() -> Self {
        Self {
            cbu_id: None,
            case_id: None,
            entity_data: HashMap::new(),
            domain: None,
            instruction: None,
            template_variables: HashMap::new(),
            orchestration_context: None,
        }
    }

    /// Set CBU ID
    pub fn cbu_id(mut self, cbu_id: impl Into<String>) -> Self {
        self.cbu_id = Some(cbu_id.into());
        self
    }

    /// Set case ID
    pub fn case_id(mut self, case_id: impl Into<String>) -> Self {
        self.case_id = Some(case_id.into());
        self
    }

    /// Add entity data
    pub fn entity_data(mut self, key: impl Into<String>, value: impl Into<String>) -> Self {
        self.entity_data.insert(key.into(), value.into());
        self
    }

    /// Add multiple entity data entries
    pub fn entity_data_map(mut self, data: HashMap<String, String>) -> Self {
        self.entity_data.extend(data);
        self
    }

    /// Set domain
    pub fn domain(mut self, domain: impl Into<String>) -> Self {
        self.domain = Some(domain.into());
        self
    }

    /// Set instruction
    pub fn instruction(mut self, instruction: impl Into<String>) -> Self {
        self.instruction = Some(instruction.into());
        self
    }

    /// Add template variable
    pub fn template_variable(mut self, key: impl Into<String>, value: impl Into<String>) -> Self {
        self.template_variables.insert(key.into(), value.into());
        self
    }

    /// Add multiple template variables
    pub fn template_variables(mut self, variables: HashMap<String, String>) -> Self {
        self.template_variables.extend(variables);
        self
    }

    /// Set orchestration context
    pub fn orchestration_context(mut self, context: OrchestrationContext) -> Self {
        self.orchestration_context = Some(context);
        self
    }

    /// Build the generation context
    pub fn build(self) -> super::traits::GenerationContext {
        super::traits::GenerationContext {
            cbu_id: self.cbu_id,
            case_id: self.case_id,
            entity_data: self.entity_data,
            domain: self.domain,
            instruction: self.instruction,
            template_variables: self.template_variables,
            orchestration_context: self.orchestration_context,
        }
    }
}

impl Default for GenerationContextBuilder {
    fn default() -> Self {
        Self::new()
    }
}

impl ContextResolver {
    /// Create a new context resolver
    pub fn new() -> Self {
        Self {
            substitution_rules: HashMap::new(),
            default_values: Self::create_default_values(),
            validation_rules: Vec::new(),
        }
    }

    /// Create with custom substitution rules
    pub fn with_substitution_rules(mut self, rules: HashMap<String, String>) -> Self {
        self.substitution_rules = rules;
        self
    }

    /// Add a substitution rule
    pub fn add_substitution_rule(mut self, from: String, to: String) -> Self {
        self.substitution_rules.insert(from, to);
        self
    }

    /// Add validation rule
    pub fn add_validation_rule(mut self, rule: ContextValidationRule) -> Self {
        self.validation_rules.push(rule);
        self
    }

    /// Create default values for common variables
    fn create_default_values() -> HashMap<String, String> {
        let mut defaults = HashMap::new();

        defaults.insert("timestamp".to_string(), chrono::Utc::now().to_rfc3339());
        defaults.insert("request_id".to_string(), Uuid::new_v4().to_string());
        defaults.insert("ob_request_id".to_string(), Uuid::new_v4().to_string());
        defaults.insert("session_id".to_string(), Uuid::new_v4().to_string());

        // Common business defaults
        defaults.insert("status".to_string(), "CREATED".to_string());
        defaults.insert("priority".to_string(), "NORMAL".to_string());
        defaults.insert("version".to_string(), "1.0".to_string());

        defaults
    }

    /// Enhance context for template generation
    pub fn enhance_for_template(
        &self,
        context: &super::traits::GenerationContext,
        template_id: &str,
    ) -> ContextEnhancementResult {
        let mut enhanced = context.clone();
        let mut modifications = Vec::new();
        let mut warnings = Vec::new();

        // Apply substitution rules
        for (from, to) in &self.substitution_rules {
            if let Some(value) = enhanced.template_variables.get(from) {
                let new_value = value.replace(from, to);
                if new_value != *value {
                    modifications.push(ContextModification {
                        variable: from.clone(),
                        modification_type: ModificationType::Updated,
                        old_value: Some(value.clone()),
                        new_value: new_value.clone(),
                        reason: "Applied substitution rule".to_string(),
                    });
                    enhanced.template_variables.insert(from.clone(), new_value);
                }
            }
        }

        // Add missing default values
        for (key, default_value) in &self.default_values {
            if !enhanced.template_variables.contains_key(key) {
                enhanced
                    .template_variables
                    .insert(key.clone(), default_value.clone());
                modifications.push(ContextModification {
                    variable: key.clone(),
                    modification_type: ModificationType::Generated,
                    old_value: None,
                    new_value: default_value.clone(),
                    reason: "Added default value".to_string(),
                });
            }
        }

        // Ensure template-specific variables
        if !enhanced.template_variables.contains_key("template_id") {
            enhanced
                .template_variables
                .insert("template_id".to_string(), template_id.to_string());
            modifications.push(ContextModification {
                variable: "template_id".to_string(),
                modification_type: ModificationType::Generated,
                old_value: None,
                new_value: template_id.to_string(),
                reason: "Added template identifier".to_string(),
            });
        }

        // Validate enhanced context
        let validation_warnings = self.validate_context(&enhanced);
        warnings.extend(validation_warnings);

        let mut metadata = HashMap::new();
        metadata.insert("template_id".to_string(), template_id.to_string());
        metadata.insert(
            "modifications_count".to_string(),
            modifications.len().to_string(),
        );
        metadata.insert("warnings_count".to_string(), warnings.len().to_string());

        ContextEnhancementResult {
            enhanced_context: enhanced,
            modifications,
            warnings,
            metadata,
        }
    }

    /// Enhance context for AI generation
    pub fn enhance_for_ai(
        &self,
        context: &super::traits::GenerationContext,
        operation_type: &super::traits::GenerationOperationType,
    ) -> ContextEnhancementResult {
        let mut enhanced = context.clone();
        let mut modifications = Vec::new();
        let mut warnings = Vec::new();

        // Ensure instruction exists for AI generation
        if enhanced.instruction.is_none() {
            let generated_instruction = self.generate_instruction_from_operation(operation_type);
            enhanced.instruction = Some(generated_instruction.clone());
            modifications.push(ContextModification {
                variable: "instruction".to_string(),
                modification_type: ModificationType::Generated,
                old_value: None,
                new_value: generated_instruction,
                reason: "Generated instruction from operation type".to_string(),
            });
        }

        // Add operation context to entity data
        enhanced
            .entity_data
            .insert("operation_type".to_string(), operation_type.to_string());
        modifications.push(ContextModification {
            variable: "operation_type".to_string(),
            modification_type: ModificationType::Generated,
            old_value: None,
            new_value: operation_type.to_string(),
            reason: "Added operation type context".to_string(),
        });

        // Normalize entity data
        let normalized_data = self.normalize_entity_data(&enhanced.entity_data);
        for (key, new_value) in normalized_data {
            if let Some(old_value) = enhanced.entity_data.get(&key) {
                if old_value != &new_value {
                    modifications.push(ContextModification {
                        variable: key.clone(),
                        modification_type: ModificationType::Normalized,
                        old_value: Some(old_value.clone()),
                        new_value: new_value.clone(),
                        reason: "Normalized entity data".to_string(),
                    });
                    enhanced.entity_data.insert(key, new_value);
                }
            }
        }

        // Validate enhanced context
        let validation_warnings = self.validate_context(&enhanced);
        warnings.extend(validation_warnings);

        let mut metadata = HashMap::new();
        metadata.insert("operation_type".to_string(), operation_type.to_string());
        metadata.insert("ai_enhanced".to_string(), "true".to_string());
        metadata.insert(
            "modifications_count".to_string(),
            modifications.len().to_string(),
        );

        ContextEnhancementResult {
            enhanced_context: enhanced,
            modifications,
            warnings,
            metadata,
        }
    }

    /// Generate instruction from operation type
    fn generate_instruction_from_operation(
        &self,
        operation_type: &super::traits::GenerationOperationType,
    ) -> String {
        match operation_type {
            super::traits::GenerationOperationType::CreateCbu => {
                "Create a new Client Business Unit (CBU) with the provided information".to_string()
            }
            super::traits::GenerationOperationType::RegisterEntity => {
                "Register a new entity with the system including all required attributes"
                    .to_string()
            }
            super::traits::GenerationOperationType::CalculateUbo => {
                "Calculate Ultimate Beneficial Ownership structure for the entity".to_string()
            }
            super::traits::GenerationOperationType::UpdateDsl => {
                "Update existing DSL with new information or modifications".to_string()
            }
            super::traits::GenerationOperationType::KycWorkflow => {
                "Create a Know Your Customer (KYC) workflow for entity verification".to_string()
            }
            super::traits::GenerationOperationType::ComplianceCheck => {
                "Perform compliance screening and validation checks".to_string()
            }
            super::traits::GenerationOperationType::DocumentCatalog => {
                "Catalog and manage documents in the system".to_string()
            }
            super::traits::GenerationOperationType::IsdaTrade => {
                "Execute ISDA derivative trade operations".to_string()
            }
            super::traits::GenerationOperationType::Custom { operation_name } => {
                format!("Perform custom operation: {}", operation_name)
            }
        }
    }

    /// Normalize entity data
    fn normalize_entity_data(&self, data: &HashMap<String, String>) -> HashMap<String, String> {
        let mut normalized = HashMap::new();

        for (key, value) in data {
            let normalized_key = key.to_lowercase().replace(' ', "_");
            let normalized_value = value.trim().to_string();

            // Special normalizations
            match normalized_key.as_str() {
                "email" => {
                    if self.is_valid_email(&normalized_value) {
                        normalized.insert(normalized_key, normalized_value.to_lowercase());
                    } else {
                        normalized.insert(normalized_key, normalized_value);
                    }
                }
                "phone" | "phone_number" => {
                    normalized.insert(normalized_key, self.normalize_phone(&normalized_value));
                }
                "country" | "jurisdiction" => {
                    normalized.insert(normalized_key, normalized_value.to_uppercase());
                }
                _ => {
                    normalized.insert(normalized_key, normalized_value);
                }
            }
        }

        normalized
    }

    /// Validate context against rules
    fn validate_context(&self, context: &super::traits::GenerationContext) -> Vec<String> {
        let mut warnings = Vec::new();

        for rule in &self.validation_rules {
            let mut rule_satisfied = true;

            // Check required variables
            for required_var in &rule.required_variables {
                if !self.context_has_variable(context, required_var) {
                    warnings.push(format!(
                        "Missing required variable '{}' for rule '{}'",
                        required_var, rule.name
                    ));
                    rule_satisfied = false;
                }
            }

            // Check conditions
            for condition in &rule.conditions {
                if !self.check_condition(context, condition) {
                    warnings.push(format!(
                        "Condition failed for variable '{}' in rule '{}'",
                        condition.variable, rule.name
                    ));
                    rule_satisfied = false;
                }
            }

            if !rule_satisfied {
                warnings.push(rule.error_message.clone());
            }
        }

        warnings
    }

    /// Check if context has a variable
    fn context_has_variable(
        &self,
        context: &super::traits::GenerationContext,
        variable: &str,
    ) -> bool {
        context.template_variables.contains_key(variable)
            || context.entity_data.contains_key(variable)
            || match variable {
                "cbu_id" => context.cbu_id.is_some(),
                "case_id" => context.case_id.is_some(),
                "domain" => context.domain.is_some(),
                "instruction" => context.instruction.is_some(),
                _ => false,
            }
    }

    /// Check a context condition
    fn check_condition(
        &self,
        context: &super::traits::GenerationContext,
        condition: &ContextCondition,
    ) -> bool {
        let value = self.get_variable_value(context, &condition.variable);

        match &value {
            Some(val) => match condition.condition_type {
                ContextConditionType::Equals => condition.expected_values.contains(val),
                ContextConditionType::NotEquals => !condition.expected_values.contains(val),
                ContextConditionType::Contains => condition
                    .expected_values
                    .iter()
                    .any(|expected| val.contains(expected)),
                ContextConditionType::NotEmpty => !val.is_empty(),
                ContextConditionType::Matches => {
                    // Simple pattern matching - in real implementation, use regex
                    condition
                        .expected_values
                        .iter()
                        .any(|pattern| val.contains(pattern))
                }
                ContextConditionType::ValidUuid => Uuid::parse_str(val).is_ok(),
                ContextConditionType::ValidEmail => self.is_valid_email(val),
                ContextConditionType::ValidDate => {
                    chrono::DateTime::parse_from_rfc3339(val).is_ok()
                        || chrono::NaiveDate::parse_from_str(val, "%Y-%m-%d").is_ok()
                }
            },
            None => condition.condition_type == ContextConditionType::NotEmpty,
        }
    }

    /// Get variable value from context
    fn get_variable_value(
        &self,
        context: &super::traits::GenerationContext,
        variable: &str,
    ) -> Option<String> {
        if let Some(value) = context.template_variables.get(variable) {
            return Some(value.clone());
        }

        if let Some(value) = context.entity_data.get(variable) {
            return Some(value.clone());
        }

        match variable {
            "cbu_id" => context.cbu_id.clone(),
            "case_id" => context.case_id.clone(),
            "domain" => context.domain.clone(),
            "instruction" => context.instruction.clone(),
            _ => None,
        }
    }

    /// Validate email format (simple check)
    fn is_valid_email(&self, email: &str) -> bool {
        email.contains('@')
            && email.contains('.')
            && !email.starts_with('@')
            && !email.ends_with('@')
    }

    /// Normalize phone number (simple implementation)
    fn normalize_phone(&self, phone: &str) -> String {
        phone
            .chars()
            .filter(|c| c.is_ascii_digit() || *c == '+')
            .collect()
    }
}

impl Default for ContextResolver {
    fn default() -> Self {
        Self::new()
    }
}

/// Factory for creating context resolvers
pub struct ContextResolverFactory;

impl ContextResolverFactory {
    /// Create a resolver for template generation
    pub fn for_template() -> ContextResolver {
        let resolver = ContextResolver::new();

        // Add template-specific validation rules
        resolver.add_validation_rule(ContextValidationRule {
            name: "template_basic".to_string(),
            required_variables: vec!["timestamp".to_string()],
            optional_variables: vec!["request_id".to_string(), "session_id".to_string()],
            conditions: vec![],
            error_message: "Basic template variables missing".to_string(),
        })
    }

    /// Create a resolver for AI generation
    pub fn for_ai() -> ContextResolver {
        let resolver = ContextResolver::new();

        // Add AI-specific validation rules
        resolver.add_validation_rule(ContextValidationRule {
            name: "ai_instruction".to_string(),
            required_variables: vec!["instruction".to_string()],
            optional_variables: vec!["operation_type".to_string()],
            conditions: vec![ContextCondition {
                variable: "instruction".to_string(),
                condition_type: ContextConditionType::NotEmpty,
                expected_values: vec![],
            }],
            error_message: "AI generation requires non-empty instruction".to_string(),
        })
    }

    /// Create a resolver with custom rules
    pub fn with_rules(rules: Vec<ContextValidationRule>) -> ContextResolver {
        let mut resolver = ContextResolver::new();
        for rule in rules {
            resolver = resolver.add_validation_rule(rule);
        }
        resolver
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::dsl::generation::GenerationOperationType;

    #[test]
    fn test_context_builder() {
        let context = GenerationContextBuilder::new()
            .cbu_id("test-cbu")
            .case_id("test-case")
            .domain("test")
            .instruction("Test instruction")
            .entity_data("name", "Test Entity")
            .template_variable("var1", "value1")
            .build();

        assert_eq!(context.cbu_id, Some("test-cbu".to_string()));
        assert_eq!(context.case_id, Some("test-case".to_string()));
        assert_eq!(context.domain, Some("test".to_string()));
        assert_eq!(context.instruction, Some("Test instruction".to_string()));
        assert_eq!(
            context.entity_data.get("name"),
            Some(&"Test Entity".to_string())
        );
        assert_eq!(
            context.template_variables.get("var1"),
            Some(&"value1".to_string())
        );
    }

    #[test]
    fn test_context_resolver_defaults() {
        let resolver = ContextResolver::new();
        assert!(resolver.default_values.contains_key("timestamp"));
        assert!(resolver.default_values.contains_key("request_id"));
        assert!(resolver.default_values.contains_key("status"));
    }

    #[test]
    fn test_template_enhancement() {
        let resolver = ContextResolver::new();
        let context = GenerationContextBuilder::new()
            .template_variable("test_var", "test_value")
            .build();

        let result = resolver.enhance_for_template(&context, "test_template");

        assert!(result
            .enhanced_context
            .template_variables
            .contains_key("template_id"));
        assert!(result
            .enhanced_context
            .template_variables
            .contains_key("timestamp"));
        assert!(!result.modifications.is_empty());
    }

    #[test]
    fn test_ai_enhancement() {
        let resolver = ContextResolver::new();
        let context = GenerationContextBuilder::new()
            .entity_data("name", "Test")
            .build();

        let result = resolver.enhance_for_ai(&context, &GenerationOperationType::CreateCbu);

        assert!(result.enhanced_context.instruction.is_some());
        assert!(result
            .enhanced_context
            .entity_data
            .contains_key("operation_type"));
        assert!(!result.modifications.is_empty());
    }

    #[test]
    fn test_email_validation() {
        let resolver = ContextResolver::new();

        assert!(resolver.is_valid_email("test@example.com"));
        assert!(!resolver.is_valid_email("invalid-email"));
        assert!(!resolver.is_valid_email("@example.com"));
        assert!(!resolver.is_valid_email("test@"));
    }

    #[test]
    fn test_phone_normalization() {
        let resolver = ContextResolver::new();

        let normalized = resolver.normalize_phone("+1 (555) 123-4567");
        assert_eq!(normalized, "+15551234567");

        let normalized2 = resolver.normalize_phone("555.123.4567");
        assert_eq!(normalized2, "5551234567");
    }

    #[test]
    fn test_context_validation() {
        let mut resolver = ContextResolver::new();
        resolver = resolver.add_validation_rule(ContextValidationRule {
            name: "test_rule".to_string(),
            required_variables: vec!["required_var".to_string()],
            optional_variables: vec![],
            conditions: vec![ContextCondition {
                variable: "required_var".to_string(),
                condition_type: ContextConditionType::NotEmpty,
                expected_values: vec![],
            }],
            error_message: "Required variable missing".to_string(),
        });

        let context = GenerationContextBuilder::new().build();
        let warnings = resolver.validate_context(&context);
        assert!(!warnings.is_empty());

        let context_with_var = GenerationContextBuilder::new()
            .template_variable("required_var", "value")
            .build();
        let warnings2 = resolver.validate_context(&context_with_var);
        assert!(warnings2.is_empty());
    }
}
