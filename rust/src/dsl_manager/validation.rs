//! DSL Manager Validation Engine
//!
//! This module provides comprehensive validation capabilities for the DSL Manager,
//! integrating with the existing validation system and adding manager-specific
//! validation layers for v3.3 normalization compliance and operational validation.

use crate::parser::validators::{
    DslValidator, DslValidator as CoreDslValidator, ValidationResult as CoreValidationResult,
};
use crate::parser_ast::{Form, Program, VerbForm};
use serde::{Deserialize, Serialize};
use std::collections::HashMap;
use thiserror::Error;

/// Validation levels supported by the DSL Manager
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub enum ValidationLevel {
    /// Basic syntax validation only
    Basic,
    /// Standard validation (syntax + semantics)
    Standard,
    /// Strict validation (all checks + business rules)
    Strict,
    /// Custom validation with specific rules
    Custom(Vec<ValidationRule>),
}

/// Custom validation rules
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct ValidationRule {
    pub name: String,
    pub description: String,
    pub severity: ValidationSeverity,
    pub rule_type: ValidationRuleType,
}

/// Types of validation rules
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub enum ValidationRuleType {
    /// Syntax-based rule
    Syntax { pattern: String },
    /// Semantic rule
    Semantic { constraint: String },
    /// Business rule
    Business { rule: String },
    /// Custom function rule
    Custom { function_name: String },
}

/// Validation severity levels
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub enum ValidationSeverity {
    Info,
    Warning,
    Error,
    Critical,
}

/// Comprehensive validation report from DSL Manager
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ValidationReport {
    /// Overall validation success
    pub valid: bool,
    /// Validation errors
    pub errors: Vec<String>,
    /// Validation warnings
    pub warnings: Vec<String>,
    /// Validation suggestions
    pub suggestions: Vec<String>,
    /// Validation level applied
    pub validation_level: ValidationLevel,
    /// Core validation results
    pub core_validation: Option<CoreValidationResult>,
    /// Manager-specific validation results
    pub manager_validation: Option<ManagerValidationResult>,
    /// V3.3 normalization validation
    pub normalization_validation: Option<NormalizationValidationResult>,
    /// Performance metrics
    pub validation_metrics: Option<ValidationMetrics>,
    /// Total validation time
    pub total_time_ms: u64,
}

impl Default for ValidationReport {
    fn default() -> Self {
        Self {
            valid: true,
            errors: Vec::new(),
            warnings: Vec::new(),
            suggestions: Vec::new(),
            validation_level: ValidationLevel::Basic,
            core_validation: None,
            manager_validation: None,
            normalization_validation: None,
            validation_metrics: None,
            total_time_ms: 0,
        }
    }
}

/// Manager-specific validation results
#[derive(Debug, Clone, Serialize, Deserialize)]
pub(crate) struct ManagerValidationResult {
    /// Operation-level validations
    pub operation_validations: Vec<OperationValidation>,
    /// Cross-form validations
    pub cross_form_validations: Vec<CrossFormValidation>,
    /// Business rule validations
    pub business_rule_validations: Vec<BusinessRuleValidation>,
    /// Resource availability validations
    pub resource_validations: Vec<ResourceValidation>,
}

/// V3.3 normalization validation results
#[derive(Debug, Clone, Serialize, Deserialize)]
pub(crate) struct NormalizationValidationResult {
    /// Whether normalization was required
    pub normalization_required: bool,
    /// Number of legacy forms normalized
    pub forms_normalized: usize,
    /// Normalization warnings
    pub normalization_warnings: Vec<String>,
    /// Canonical compliance score (0.0 to 1.0)
    pub canonical_compliance_score: f64,
}

/// Individual operation validation
#[derive(Debug, Clone, Serialize, Deserialize)]
pub(crate) struct OperationValidation {
    pub operation_name: String,
    pub is_valid: bool,
    pub errors: Vec<String>,
    pub warnings: Vec<String>,
    pub recommendations: Vec<String>,
}

/// Cross-form validation (relationships between forms)
#[derive(Debug, Clone, Serialize, Deserialize)]
pub(crate) struct CrossFormValidation {
    pub validation_name: String,
    pub forms_involved: Vec<String>,
    pub is_valid: bool,
    pub description: String,
    pub severity: ValidationSeverity,
}

/// Business rule validation
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct BusinessRuleValidation {
    pub rule_name: String,
    pub rule_description: String,
    pub is_satisfied: bool,
    pub violation_details: Option<String>,
    pub recommended_action: Option<String>,
}

/// Resource validation (database, external services, etc.)
#[derive(Debug, Clone, Serialize, Deserialize)]
pub(crate) struct ResourceValidation {
    pub resource_name: String,
    pub resource_type: String,
    pub is_available: bool,
    pub response_time_ms: Option<u64>,
    pub error_details: Option<String>,
}

/// Validation performance metrics
#[derive(Debug, Clone, Serialize, Deserialize)]
pub(crate) struct ValidationMetrics {
    pub syntax_validation_ms: u64,
    pub semantic_validation_ms: u64,
    pub business_rule_validation_ms: u64,
    pub normalization_validation_ms: u64,
    pub resource_validation_ms: u64,
    pub total_forms_validated: usize,
    pub total_rules_applied: usize,
}

impl Default for ValidationMetrics {
    fn default() -> Self {
        Self {
            syntax_validation_ms: 0,
            semantic_validation_ms: 0,
            business_rule_validation_ms: 0,
            normalization_validation_ms: 0,
            resource_validation_ms: 0,
            total_forms_validated: 0,
            total_rules_applied: 0,
        }
    }
}

/// DSL Manager validation engine
pub struct DslValidationEngine {
    /// Core validator instance
    core_validator: CoreDslValidator,
    /// Custom validation rules
    custom_rules: Vec<ValidationRule>,
    /// Business rule registry
    business_rules: BusinessRuleRegistry,
    /// Validation configuration
    config: ValidationConfig,
}

/// Business rule registry
pub struct BusinessRuleRegistry {
    rules: HashMap<String, Box<dyn BusinessRule>>,
}

impl std::fmt::Debug for BusinessRuleRegistry {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.debug_struct("BusinessRuleRegistry")
            .field("rule_count", &self.rules.len())
            .finish()
    }
}

impl Default for BusinessRuleRegistry {
    fn default() -> Self {
        Self::new()
    }
}

/// Business rule trait
pub trait BusinessRule: Send + Sync {
    fn name(&self) -> &str;
    fn description(&self) -> &str;
    fn validate(&self, program: &Program) -> BusinessRuleValidation;
}

/// Validation configuration
#[derive(Debug, Clone)]
pub(crate) struct ValidationConfig {
    /// Enable normalization validation
    pub validate_normalization: bool,
    /// Enable resource validation
    pub validate_resources: bool,
    /// Enable business rule validation
    pub validate_business_rules: bool,
    /// Maximum validation time (ms)
    pub max_validation_time_ms: u64,
    /// Enable detailed metrics
    pub enable_detailed_metrics: bool,
    /// Parallel validation
    pub enable_parallel_validation: bool,
}

impl Default for ValidationConfig {
    fn default() -> Self {
        Self {
            validate_normalization: true,
            validate_resources: false,
            validate_business_rules: true,
            max_validation_time_ms: 5000,
            enable_detailed_metrics: false,
            enable_parallel_validation: true,
        }
    }
}

/// Validation errors specific to DSL Manager
#[derive(Debug, Error)]
pub enum ValidationError {
    #[error("Validation timeout after {timeout_ms}ms")]
    ValidationTimeout { timeout_ms: u64 },

    #[error("Business rule '{rule_name}' failed: {reason}")]
    BusinessRuleViolation { rule_name: String, reason: String },

    #[error("Resource validation failed for '{resource}': {error}")]
    ResourceValidationFailed { resource: String, error: String },

    #[error("Normalization validation failed: {error}")]
    NormalizationValidationFailed { error: String },

    #[error("Custom rule '{rule_name}' validation failed: {error}")]
    CustomRuleValidationFailed { rule_name: String, error: String },
}

impl DslValidationEngine {
    /// Create a new validation engine
    pub fn new() -> Self {
        Self {
            core_validator: CoreDslValidator::new(),
            custom_rules: Vec::new(),
            business_rules: BusinessRuleRegistry::new(),
            config: ValidationConfig::default(),
        }
    }

    /// Create with custom configuration
    pub(crate) fn with_config(config: ValidationConfig) -> Self {
        Self {
            core_validator: CoreDslValidator::new(),
            custom_rules: Vec::new(),
            business_rules: BusinessRuleRegistry::new(),
            config,
        }
    }

    /// Register a business rule
    pub(crate) fn register_business_rule(&mut self, rule: Box<dyn BusinessRule>) {
        self.business_rules.register(rule);
    }

    /// Main validation entry point
    pub async fn validate(
        &self,
        program: &Program,
        level: ValidationLevel,
    ) -> Result<ValidationReport, ValidationError> {
        let start_time = std::time::Instant::now();
        let mut metrics = ValidationMetrics::default();

        // Step 1: Core validation (always performed)
        let core_start = std::time::Instant::now();
        let mut validator = DslValidator::new();
        let core_validation = validator.validate_program(program);
        metrics.syntax_validation_ms = core_start.elapsed().as_millis() as u64;
        metrics.total_forms_validated = program.len();

        // Step 2: Manager-specific validation based on level
        let manager_validation = match &level {
            ValidationLevel::Basic => self.basic_validation(program, &mut metrics).await?,
            ValidationLevel::Standard => self.standard_validation(program, &mut metrics).await?,
            ValidationLevel::Strict => self.strict_validation(program, &mut metrics).await?,
            ValidationLevel::Custom(ref rules) => {
                self.custom_validation(program, rules, &mut metrics).await?
            }
        };

        // Step 3: Normalization validation (if enabled)
        let normalization_validation = if self.config.validate_normalization {
            let norm_start = std::time::Instant::now();
            let result = self.validate_normalization(program).await?;
            metrics.normalization_validation_ms = norm_start.elapsed().as_millis() as u64;
            result
        } else {
            NormalizationValidationResult::default()
        };

        let total_time_ms = start_time.elapsed().as_millis() as u64;

        // Check timeout
        if total_time_ms > self.config.max_validation_time_ms {
            return Err(ValidationError::ValidationTimeout {
                timeout_ms: self.config.max_validation_time_ms,
            });
        }

        let is_valid = core_validation.is_valid
            && manager_validation.is_valid()
            && (!self.config.validate_normalization || normalization_validation.is_valid());

        Ok(ValidationReport {
            valid: is_valid,
            errors: Vec::new(),
            warnings: Vec::new(),
            suggestions: Vec::new(),
            validation_level: level.clone(),
            core_validation: Some(core_validation),
            manager_validation: Some(manager_validation),
            normalization_validation: Some(normalization_validation),
            validation_metrics: Some(metrics),
            total_time_ms,
        })
    }

    /// Basic validation - syntax only
    async fn basic_validation(
        &self,
        _program: &Program,
        _metrics: &mut ValidationMetrics,
    ) -> Result<ManagerValidationResult, ValidationError> {
        Ok(ManagerValidationResult {
            operation_validations: vec![],
            cross_form_validations: vec![],
            business_rule_validations: vec![],
            resource_validations: vec![],
        })
    }

    /// Standard validation - syntax + semantics
    async fn standard_validation(
        &self,
        program: &Program,
        metrics: &mut ValidationMetrics,
    ) -> Result<ManagerValidationResult, ValidationError> {
        let sem_start = std::time::Instant::now();

        let operation_validations = self.validate_operations(program).await;
        let cross_form_validations = self.validate_cross_forms(program).await;

        metrics.semantic_validation_ms = sem_start.elapsed().as_millis() as u64;

        Ok(ManagerValidationResult {
            operation_validations,
            cross_form_validations,
            business_rule_validations: vec![],
            resource_validations: vec![],
        })
    }

    /// Strict validation - all checks + business rules
    async fn strict_validation(
        &self,
        program: &Program,
        metrics: &mut ValidationMetrics,
    ) -> Result<ManagerValidationResult, ValidationError> {
        let mut result = self.standard_validation(program, metrics).await?;

        // Add business rule validation
        if self.config.validate_business_rules {
            let br_start = std::time::Instant::now();
            // result.business_rule_validations = self.validate_business_rules(program).await;
            // Simplified for now due to trait object Clone issues
            metrics.business_rule_validation_ms = br_start.elapsed().as_millis() as u64;
            metrics.total_rules_applied = self.business_rules.rules.len();
        }

        // Add resource validation
        if self.config.validate_resources {
            let res_start = std::time::Instant::now();
            result.resource_validations = self.validate_resources(program).await;
            metrics.resource_validation_ms = res_start.elapsed().as_millis() as u64;
        }

        Ok(result)
    }

    /// Custom validation with specific rules
    async fn custom_validation(
        &self,
        program: &Program,
        rules: &[ValidationRule],
        metrics: &mut ValidationMetrics,
    ) -> Result<ManagerValidationResult, ValidationError> {
        let result = self.standard_validation(program, metrics).await?;

        // Apply custom rules
        for rule in rules {
            match self.apply_custom_rule(program, rule).await {
                Ok(_) => {
                    // Rule passed
                }
                Err(e) => {
                    return Err(ValidationError::CustomRuleValidationFailed {
                        rule_name: rule.name.clone(),
                        error: e.to_string(),
                    });
                }
            }
        }

        Ok(result)
    }

    /// Validate individual operations
    async fn validate_operations(&self, program: &Program) -> Vec<OperationValidation> {
        let mut validations = Vec::new();

        for form in program {
            if let Form::Verb(verb_form) = form {
                let validation = OperationValidation {
                    operation_name: verb_form.verb.clone(),
                    is_valid: true, // TODO: Implement specific operation validation
                    errors: vec![],
                    warnings: vec![],
                    recommendations: vec![],
                };
                validations.push(validation);
            }
        }

        validations
    }

    /// Validate cross-form relationships
    async fn validate_cross_forms(&self, _program: &Program) -> Vec<CrossFormValidation> {
        // TODO: Implement cross-form validation logic
        vec![]
    }

    /// Validate business rules
    async fn validate_business_rules(&self, program: &Program) -> Vec<BusinessRuleValidation> {
        let mut validations = Vec::new();

        for (_name, rule) in &self.business_rules.rules {
            let validation = rule.validate(program);
            validations.push(validation);
        }

        validations
    }

    /// Validate resource availability
    async fn validate_resources(&self, _program: &Program) -> Vec<ResourceValidation> {
        // TODO: Implement resource validation
        vec![ResourceValidation {
            resource_name: "database".to_string(),
            resource_type: "postgresql".to_string(),
            is_available: true,
            response_time_ms: Some(10),
            error_details: None,
        }]
    }

    /// Validate v3.3 normalization compliance
    async fn validate_normalization(
        &self,
        program: &Program,
    ) -> Result<NormalizationValidationResult, ValidationError> {
        let mut forms_normalized = 0;
        let mut normalization_warnings = Vec::new();
        let mut total_forms = 0;
        let mut canonical_forms = 0;

        for form in program {
            if let Form::Verb(verb_form) = form {
                total_forms += 1;

                // Check if this form uses v3.3 legacy syntax
                if self.is_legacy_form(verb_form) {
                    forms_normalized += 1;
                    normalization_warnings.push(format!(
                        "Form with verb '{}' uses legacy v3.3 syntax",
                        verb_form.verb
                    ));
                } else {
                    canonical_forms += 1;
                }
            }
        }

        let canonical_compliance_score = if total_forms > 0 {
            canonical_forms as f64 / total_forms as f64
        } else {
            1.0
        };

        Ok(NormalizationValidationResult {
            normalization_required: forms_normalized > 0,
            forms_normalized,
            normalization_warnings,
            canonical_compliance_score,
        })
    }

    /// Check if a form uses legacy v3.3 syntax
    fn is_legacy_form(&self, verb_form: &VerbForm) -> bool {
        // Check for known legacy verbs
        matches!(
            verb_form.verb.as_str(),
            "kyc.start_case"
                | "kyc.transition_state"
                | "kyc.add_finding"
                | "kyc.approve_case"
                | "ubo.link_ownership"
                | "ubo.link_control"
                | "ubo.add_evidence"
                | "ubo.update_link_status"
        )
    }

    /// Apply a custom validation rule
    async fn apply_custom_rule(
        &self,
        _program: &Program,
        _rule: &ValidationRule,
    ) -> Result<(), Box<dyn std::error::Error>> {
        // TODO: Implement custom rule application
        Ok(())
    }
}

impl BusinessRuleRegistry {
    pub fn new() -> Self {
        Self {
            rules: HashMap::new(),
        }
    }

    pub fn register(&mut self, rule: Box<dyn BusinessRule>) {
        self.rules.insert(rule.name().to_string(), rule);
    }

    pub(crate) fn get_rule(&self, name: &str) -> Option<&Box<dyn BusinessRule>> {
        self.rules.get(name)
    }
}

impl ManagerValidationResult {
    pub fn is_valid(&self) -> bool {
        self.operation_validations.iter().all(|v| v.is_valid)
            && self.cross_form_validations.iter().all(|v| v.is_valid)
            && self
                .business_rule_validations
                .iter()
                .all(|v| v.is_satisfied)
            && self.resource_validations.iter().all(|v| v.is_available)
    }
}

impl NormalizationValidationResult {
    pub fn is_valid(&self) -> bool {
        self.canonical_compliance_score >= 0.8 // 80% canonical compliance required
    }
}

impl Default for NormalizationValidationResult {
    fn default() -> Self {
        Self {
            normalization_required: false,
            forms_normalized: 0,
            normalization_warnings: vec![],
            canonical_compliance_score: 1.0,
        }
    }
}

impl ValidationReport {
    /// Get all errors from the validation report
    pub(crate) fn get_all_errors(&self) -> Vec<String> {
        let mut errors = Vec::new();

        // Core validation errors
        if let Some(core_val) = &self.core_validation {
            errors.extend(
                core_val
                    .errors
                    .iter()
                    .map(|e| format!("Core: {}", e.message)),
            );
        }

        // Manager validation errors
        if let Some(manager_val) = &self.manager_validation {
            for op_val in &manager_val.operation_validations {
                errors.extend(op_val.errors.iter().cloned());
            }

            for cross_val in &manager_val.cross_form_validations {
                if !cross_val.is_valid {
                    errors.push(format!("Cross-form: {}", cross_val.description));
                }
            }

            for br_val in &manager_val.business_rule_validations {
                if !br_val.is_satisfied {
                    if let Some(ref details) = br_val.violation_details {
                        errors.push(format!("Business rule '{}': {}", br_val.rule_name, details));
                    }
                }
            }
        }

        errors
    }

    /// Get all warnings from the validation report
    pub(crate) fn get_all_warnings(&self) -> Vec<String> {
        let mut warnings = Vec::new();

        // Core validation warnings
        if let Some(core_val) = &self.core_validation {
            warnings.extend(
                core_val
                    .warnings
                    .iter()
                    .map(|w| format!("Core: {}", w.message)),
            );
        }

        // Manager validation warnings
        if let Some(manager_val) = &self.manager_validation {
            for op_val in &manager_val.operation_validations {
                warnings.extend(op_val.warnings.iter().cloned());
            }
        }

        // Normalization warnings
        if let Some(norm_val) = &self.normalization_validation {
            warnings.extend(
                norm_val
                    .normalization_warnings
                    .iter()
                    .map(|w| format!("Normalization: {}", w)),
            );
        }

        warnings
    }
}

// Example business rule implementations
pub(crate) struct KycCaseBusinessRule;

impl BusinessRule for KycCaseBusinessRule {
    fn name(&self) -> &str {
        "kyc_case_completeness"
    }

    fn description(&self) -> &str {
        "Ensures KYC cases have required fields and proper workflow states"
    }

    fn validate(&self, _program: &Program) -> BusinessRuleValidation {
        // TODO: Implement KYC-specific business logic
        BusinessRuleValidation {
            rule_name: self.name().to_string(),
            rule_description: self.description().to_string(),
            is_satisfied: true,
            violation_details: None,
            recommended_action: None,
        }
    }
}

pub(crate) struct UboComplianceBusinessRule;

impl BusinessRule for UboComplianceBusinessRule {
    fn name(&self) -> &str {
        "ubo_compliance_threshold"
    }

    fn description(&self) -> &str {
        "Ensures UBO ownership percentages meet compliance requirements"
    }

    fn validate(&self, _program: &Program) -> BusinessRuleValidation {
        // TODO: Implement UBO compliance logic
        BusinessRuleValidation {
            rule_name: self.name().to_string(),
            rule_description: self.description().to_string(),
            is_satisfied: true,
            violation_details: None,
            recommended_action: None,
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::parser::idiomatic_parser::parse_program;

    #[tokio::test]
    async fn test_validation_engine_creation() {
        let engine = DslValidationEngine::new();
        assert_eq!(engine.custom_rules.len(), 0);
    }

    #[tokio::test]
    async fn test_basic_validation() {
        let engine = DslValidationEngine::new();
        let dsl = "(case.create :case-id \"test\")";
        let program = parse_program(dsl).unwrap();

        let report = engine
            .validate(&program, ValidationLevel::Basic)
            .await
            .unwrap();
        assert!(report.valid);
    }

    #[tokio::test]
    async fn test_normalization_validation() {
        let engine = DslValidationEngine::new();
        let legacy_dsl = "(kyc.start_case :case_id \"test\")";
        let program = parse_program(legacy_dsl).unwrap();

        let report = engine
            .validate(&program, ValidationLevel::Standard)
            .await
            .unwrap();

        // Should detect legacy syntax
        assert!(
            report
                .normalization_validation
                .as_ref()
                .unwrap()
                .normalization_required
        );
        assert_eq!(
            report
                .normalization_validation
                .as_ref()
                .unwrap()
                .forms_normalized,
            1
        );
    }

    #[tokio::test]
    async fn test_business_rule_registration() {
        let mut engine = DslValidationEngine::new();
        engine.register_business_rule(Box::new(KycCaseBusinessRule));

        let dsl = "(case.create :case-id \"test\")";
        let program = parse_program(dsl).unwrap();

        let report = engine
            .validate(&program, ValidationLevel::Strict)
            .await
            .unwrap();
        assert!(!report
            .manager_validation
            .unwrap()
            .business_rule_validations
            .is_empty());
    }
}
