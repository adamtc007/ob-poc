//! DSL Validation Module - v3.1 Compatible
//!
//! This module provides comprehensive validation for DSL v3.1, including:
//! - Syntax validation (S-expression structure)
//! - Semantic validation (business logic)
//! - Vocabulary validation (approved verbs only)
//! - Complete v3.1 verb support (Document Library + ISDA domains)
//!
//! # DSL v3.1 Features Supported
//!
//! - **Document Library Domain**: 8 verbs for document lifecycle management
//! - **ISDA Derivative Domain**: 12 verbs for derivative contract workflows
//! - **Core Domains**: KYC, UBO, Onboarding, Compliance, Case Management
//! - **Clojure-style keywords**: `:key value` syntax
//! - **Rich data types**: Maps, lists, AttributeID references
//!
//! # Migration from Go Agent
//!
//! This module consolidates and improves upon the Go agent validation logic:
//! - More comprehensive verb vocabulary (80+ approved verbs vs 70+ in Go)
//! - Better error reporting and suggestions
//! - Performance optimizations with compiled regex
//! - Type-safe error handling

use crate::agents::{AgentError, AgentResult, QualityMetrics};
use regex::Regex;
use serde::{Deserialize, Serialize};
use std::collections::{HashMap, HashSet};
use std::time::Instant;
use tracing::{debug, warn};

/// Comprehensive DSL validator for v3.1
pub struct DslValidator {
    verb_validator: VerbValidator,
    syntax_validator: SyntaxValidator,
    semantic_validator: SemanticValidator,
}

/// Validates DSL verb vocabulary against approved v3.1 verbs
pub struct VerbValidator {
    approved_verbs: HashSet<String>,
    verb_pattern: Regex,
}

/// Validates DSL syntax and structure
pub struct SyntaxValidator {
    s_expr_pattern: Regex,
    keyword_pattern: Regex,
    string_pattern: Regex,
}

/// Validates semantic correctness and business logic
pub struct SemanticValidator {
    required_patterns: HashMap<String, Vec<String>>,
    business_rules: Vec<BusinessRule>,
}

/// Validation result with detailed feedback
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ValidationResult {
    pub is_valid: bool,
    pub validation_score: f64,
    pub errors: Vec<ValidationError>,
    pub warnings: Vec<ValidationWarning>,
    pub suggestions: Vec<String>,
    pub metrics: QualityMetrics,
    pub summary: String,
}

/// Validation error with categorization
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ValidationError {
    pub category: ErrorCategory,
    pub message: String,
    pub location: Option<Location>,
    pub severity: Severity,
    pub suggestion: Option<String>,
}

/// Validation warning for potential issues
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ValidationWarning {
    pub category: WarningCategory,
    pub message: String,
    pub location: Option<Location>,
    pub suggestion: Option<String>,
}

/// Error categories for better organization
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub enum ErrorCategory {
    Syntax,
    Vocabulary,
    Semantic,
    Structure,
    BusinessRule,
    Compliance,
}

/// Warning categories
#[derive(Debug, Clone, Serialize, Deserialize)]
pub enum WarningCategory {
    Style,
    Performance,
    Deprecation,
    BestPractice,
    Completeness,
}

/// Error/warning severity levels
#[derive(Debug, Clone, Serialize, Deserialize)]
pub enum Severity {
    Critical,
    Error,
    Warning,
    Info,
}

/// Location information for errors/warnings
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct Location {
    pub line: usize,
    pub column: usize,
    pub length: usize,
}

/// Business rule for semantic validation
#[derive(Debug, Clone)]
pub struct BusinessRule {
    pub name: String,
    pub description: String,
    pub condition: fn(&str) -> bool,
    pub error_message: String,
}

impl DslValidator {
    /// Create a new DSL validator with v3.1 support
    pub fn new() -> AgentResult<Self> {
        let verb_validator = VerbValidator::new()?;
        let syntax_validator = SyntaxValidator::new()?;
        let semantic_validator = SemanticValidator::new();

        Ok(Self {
            verb_validator,
            syntax_validator,
            semantic_validator,
        })
    }

    /// Validate DSL content comprehensively
    pub fn validate(&self, dsl: &str) -> AgentResult<ValidationResult> {
        let start_time = Instant::now();
        let mut errors = Vec::new();
        let mut warnings = Vec::new();
        let mut suggestions = Vec::new();

        // 1. Syntax validation
        let syntax_result = self.syntax_validator.validate(dsl)?;
        errors.extend(syntax_result.errors);
        warnings.extend(syntax_result.warnings);

        // 2. Vocabulary validation (verb checking)
        let vocab_result = self.verb_validator.validate_verbs(dsl)?;
        errors.extend(vocab_result.errors);
        warnings.extend(vocab_result.warnings);
        suggestions.extend(vocab_result.suggestions);

        // 3. Semantic validation
        let semantic_result = self.semantic_validator.validate(dsl)?;
        errors.extend(semantic_result.errors);
        warnings.extend(semantic_result.warnings);

        // Calculate overall metrics
        let processing_time = start_time.elapsed().as_millis() as u64;
        let is_valid = errors.is_empty();
        let validation_score = self.calculate_validation_score(&errors, &warnings);

        let metrics = QualityMetrics {
            confidence: if is_valid { 0.95 } else { 0.3 },
            validation_score,
            completeness: self.calculate_completeness_score(dsl),
            coherence: self.calculate_coherence_score(dsl),
            approved_verbs_count: self.verb_validator.count_approved_verbs(dsl),
            unapproved_verbs_count: self.verb_validator.find_unapproved_verbs(dsl).len(),
            processing_time_ms: processing_time,
        };

        let summary = self.generate_validation_summary(&errors, &warnings, &metrics);

        Ok(ValidationResult {
            is_valid,
            validation_score,
            errors,
            warnings,
            suggestions,
            metrics,
            summary,
        })
    }

    fn calculate_validation_score(
        &self,
        errors: &[ValidationError],
        warnings: &[ValidationWarning],
    ) -> f64 {
        let error_penalty = errors.len() as f64 * 0.2;
        let warning_penalty = warnings.len() as f64 * 0.05;
        let base_score = 1.0;

        (base_score - error_penalty - warning_penalty).max(0.0)
    }

    fn calculate_completeness_score(&self, dsl: &str) -> f64 {
        // Check for essential DSL components
        let has_case_create = dsl.contains("case.create");
        let has_business_context = dsl.contains("cbu.id") || dsl.contains("nature-purpose");
        let has_domain_actions = Regex::new(r"\([a-z]+\.[a-z]+")
            .map(|re| re.find_iter(dsl).count() > 1)
            .unwrap_or(false);

        let mut score = 0.0;
        if has_case_create {
            score += 0.4;
        }
        if has_business_context {
            score += 0.3;
        }
        if has_domain_actions {
            score += 0.3;
        }

        score
    }

    fn calculate_coherence_score(&self, dsl: &str) -> f64 {
        // Check for logical flow and consistency
        let verb_count = self.verb_validator.count_approved_verbs(dsl) as f64;
        let line_count = dsl.lines().count() as f64;

        // Higher coherence if verbs are well-distributed across lines
        if line_count > 0.0 {
            (verb_count / line_count).min(1.0)
        } else {
            0.0
        }
    }

    fn generate_validation_summary(
        &self,
        errors: &[ValidationError],
        warnings: &[ValidationWarning],
        metrics: &QualityMetrics,
    ) -> String {
        if errors.is_empty() && warnings.is_empty() {
            format!(
                "✅ DSL validation passed successfully. Quality score: {:.2}, {} approved verbs detected.",
                metrics.overall_score(),
                metrics.approved_verbs_count
            )
        } else {
            format!(
                "❌ DSL validation found {} error(s) and {} warning(s). Quality score: {:.2}. Please review and fix issues.",
                errors.len(),
                warnings.len(),
                metrics.overall_score()
            )
        }
    }
}

impl VerbValidator {
    pub fn new() -> AgentResult<Self> {
        let approved_verbs = Self::load_approved_verbs_v31();
        let verb_pattern = Regex::new(r"\(([a-z]+\.[a-z_][a-z_-]*)").map_err(|e| {
            AgentError::InitializationError(format!("Failed to compile verb regex: {}", e))
        })?;

        Ok(Self {
            approved_verbs,
            verb_pattern,
        })
    }

    pub fn validate_verbs(&self, dsl: &str) -> AgentResult<ValidationResult> {
        let mut errors = Vec::new();
        let mut warnings = Vec::new();
        let mut suggestions = Vec::new();

        let unapproved_verbs = self.find_unapproved_verbs(dsl);

        if !unapproved_verbs.is_empty() {
            for verb in &unapproved_verbs {
                errors.push(ValidationError {
                    category: ErrorCategory::Vocabulary,
                    message: format!("Unapproved verb '{}' detected. Only approved v3.1 vocabulary verbs are allowed.", verb),
                    location: self.find_verb_location(dsl, verb),
                    severity: Severity::Error,
                    suggestion: self.suggest_alternative_verb(verb),
                });
            }

            suggestions.push(
                "Review the approved DSL v3.1 vocabulary. Consider using approved alternatives."
                    .to_string(),
            );
        }

        // Check for deprecated verbs
        let deprecated_verbs = self.find_deprecated_verbs(dsl);
        for verb in deprecated_verbs {
            warnings.push(ValidationWarning {
                category: WarningCategory::Deprecation,
                message: format!(
                    "Verb '{}' is deprecated and may be removed in future versions.",
                    verb
                ),
                location: self.find_verb_location(dsl, &verb),
                suggestion: Some("Consider migrating to newer equivalent verbs.".to_string()),
            });
        }

        let is_valid = errors.is_empty();
        let validation_score = if is_valid { 1.0 } else { 0.0 };

        Ok(ValidationResult {
            is_valid,
            validation_score,
            errors,
            warnings,
            suggestions,
            metrics: QualityMetrics {
                confidence: if is_valid { 0.95 } else { 0.2 },
                validation_score,
                completeness: 1.0,
                coherence: 1.0,
                approved_verbs_count: self.count_approved_verbs(dsl),
                unapproved_verbs_count: unapproved_verbs.len(),
                processing_time_ms: 0,
            },
            summary: if is_valid {
                "✅ All verbs are approved v3.1 vocabulary".to_string()
            } else {
                format!("❌ Found {} unapproved verbs", unapproved_verbs.len())
            },
        })
    }

    pub fn find_unapproved_verbs(&self, dsl: &str) -> Vec<String> {
        let mut unapproved = Vec::new();
        let mut seen = HashSet::new();

        for captures in self.verb_pattern.captures_iter(dsl) {
            if let Some(verb_match) = captures.get(1) {
                let verb = verb_match.as_str();

                if seen.contains(verb) {
                    continue;
                }
                seen.insert(verb);

                // Skip non-verb constructs (parameters, attributes)
                if self.is_parameter_not_verb(verb) {
                    continue;
                }

                if !self.approved_verbs.contains(verb) {
                    unapproved.push(verb.to_string());
                }
            }
        }

        unapproved
    }

    fn find_deprecated_verbs(&self, _dsl: &str) -> Vec<String> {
        // For now, no deprecated verbs in v3.1
        // This would be populated from database in real implementation
        Vec::new()
    }

    fn is_parameter_not_verb(&self, verb: &str) -> bool {
        // These look like verbs but are actually parameters or attributes
        matches!(
            verb,
            "cbu.id"
                | "attr.id"
                | "for.product"
                | "resource.create"
                | "bind"
                | "nature.purpose"
                | "party.a"
                | "party.b"
        )
    }

    pub fn count_approved_verbs(&self, dsl: &str) -> usize {
        self.verb_pattern
            .captures_iter(dsl)
            .filter_map(|cap| cap.get(1))
            .map(|m| m.as_str())
            .filter(|verb| !self.is_parameter_not_verb(verb))
            .filter(|verb| self.approved_verbs.contains(*verb))
            .count()
    }

    fn find_verb_location(&self, dsl: &str, verb: &str) -> Option<Location> {
        // Find the location of the verb in the DSL
        for (line_no, line) in dsl.lines().enumerate() {
            if let Some(column) = line.find(verb) {
                return Some(Location {
                    line: line_no + 1,
                    column: column + 1,
                    length: verb.len(),
                });
            }
        }
        None
    }

    fn suggest_alternative_verb(&self, verb: &str) -> Option<String> {
        // Suggest alternatives based on common patterns
        match verb {
            "case.new" => Some("case.create".to_string()),
            "entity.create" => Some("entity.register".to_string()),
            "product.add" => Some("products.add".to_string()),
            "kyc.initialize" => Some("kyc.start".to_string()),
            "document.add" => Some("document.catalog".to_string()),
            _ => None,
        }
    }

    /// Load the complete DSL v3.1 approved vocabulary from actual EBNF specification
    fn load_approved_verbs_v31() -> HashSet<String> {
        let mut verbs = HashSet::new();

        // Core Case Management (5 verbs)
        verbs.insert("case.create".to_string());
        verbs.insert("case.update".to_string());
        verbs.insert("case.validate".to_string());
        verbs.insert("case.approve".to_string());
        verbs.insert("case.close".to_string());

        // Entity and Identity Management (5 verbs)
        verbs.insert("entity.register".to_string());
        verbs.insert("entity.classify".to_string());
        verbs.insert("entity.link".to_string());
        verbs.insert("identity.verify".to_string());
        verbs.insert("identity.attest".to_string());

        // Product and Service Management (6 verbs)
        verbs.insert("products.add".to_string());
        verbs.insert("products.configure".to_string());
        verbs.insert("services.discover".to_string());
        verbs.insert("services.provision".to_string());
        verbs.insert("services.activate".to_string());

        // KYC and Compliance (6 verbs)
        verbs.insert("kyc.start".to_string());
        verbs.insert("kyc.collect".to_string());
        verbs.insert("kyc.verify".to_string());
        verbs.insert("kyc.assess".to_string());
        verbs.insert("compliance.screen".to_string());
        verbs.insert("compliance.monitor".to_string());

        // Additional KYC verbs from EBNF
        verbs.insert("kyc.assess_risk".to_string());
        verbs.insert("kyc.collect_document".to_string());
        verbs.insert("kyc.screen_sanctions".to_string());
        verbs.insert("kyc.check_pep".to_string());
        verbs.insert("kyc.validate_address".to_string());

        // Additional Compliance verbs from EBNF
        verbs.insert("compliance.fatca_check".to_string());
        verbs.insert("compliance.crs_check".to_string());
        verbs.insert("compliance.aml_check".to_string());
        verbs.insert("compliance.generate_sar".to_string());
        verbs.insert("compliance.verify".to_string());

        // UBO Discovery (19 verbs from Go implementation)
        verbs.insert("ubo.collect-entity-data".to_string());
        verbs.insert("ubo.get-ownership-structure".to_string());
        verbs.insert("ubo.unroll-structure".to_string());
        verbs.insert("ubo.resolve-ubos".to_string());
        verbs.insert("ubo.calculate-indirect-ownership".to_string());
        verbs.insert("ubo.identify-control-prong".to_string());
        verbs.insert("ubo.apply-thresholds".to_string());
        verbs.insert("ubo.identify-trust-parties".to_string());
        verbs.insert("ubo.resolve-trust-ubos".to_string());
        verbs.insert("ubo.identify-ownership-prong".to_string());
        verbs.insert("ubo.resolve-partnership-ubos".to_string());
        verbs.insert("ubo.recursive-entity-resolve".to_string());
        verbs.insert("ubo.identify-fincen-control-roles".to_string());
        verbs.insert("ubo.apply-fincen-control-prong".to_string());
        verbs.insert("ubo.verify-identity".to_string());
        verbs.insert("ubo.screen-person".to_string());
        verbs.insert("ubo.assess-risk".to_string());
        verbs.insert("ubo.monitor-changes".to_string());
        verbs.insert("ubo.refresh-data".to_string());
        verbs.insert("ubo.trigger-review".to_string());

        // UBO calculation verbs from EBNF
        verbs.insert("ubo.calc".to_string());
        verbs.insert("ubo.outcome".to_string());

        // Role assignment
        verbs.insert("role.assign".to_string());

        // Resource Management (5 verbs)
        verbs.insert("resources.plan".to_string());
        verbs.insert("resources.provision".to_string());
        verbs.insert("resources.configure".to_string());
        verbs.insert("resources.test".to_string());
        verbs.insert("resources.deploy".to_string());

        // Data and Attribute Management (5 verbs)
        verbs.insert("attributes.define".to_string());
        verbs.insert("attributes.resolve".to_string());
        verbs.insert("values.bind".to_string());
        verbs.insert("values.validate".to_string());
        verbs.insert("values.encrypt".to_string());

        // Workflow and Task Management (6 verbs)
        verbs.insert("workflow.transition".to_string());
        verbs.insert("workflow.gate".to_string());
        verbs.insert("tasks.create".to_string());
        verbs.insert("tasks.assign".to_string());
        verbs.insert("tasks.complete".to_string());

        // Communication and Notifications (4 verbs)
        verbs.insert("notify.send".to_string());
        verbs.insert("communicate.request".to_string());
        verbs.insert("escalate.trigger".to_string());
        verbs.insert("audit.log".to_string());

        // External Integration (4 verbs)
        verbs.insert("external.query".to_string());
        verbs.insert("external.sync".to_string());
        verbs.insert("api.call".to_string());
        verbs.insert("webhook.register".to_string());

        // Document Library Domain (8 verbs - NEW in v3.1)
        verbs.insert("document.catalog".to_string());
        verbs.insert("document.verify".to_string());
        verbs.insert("document.extract".to_string());
        verbs.insert("document.link".to_string());
        verbs.insert("document.use".to_string());
        verbs.insert("document.amend".to_string());
        verbs.insert("document.expire".to_string());
        verbs.insert("document.query".to_string());

        // ISDA Derivative Domain (12 verbs - NEW in v3.1)
        verbs.insert("isda.establish_master".to_string());
        verbs.insert("isda.establish_csa".to_string());
        verbs.insert("isda.execute_trade".to_string());
        verbs.insert("isda.margin_call".to_string());
        verbs.insert("isda.post_collateral".to_string());
        verbs.insert("isda.value_portfolio".to_string());
        verbs.insert("isda.declare_termination_event".to_string());
        verbs.insert("isda.close_out".to_string());
        verbs.insert("isda.amend_agreement".to_string());
        verbs.insert("isda.novate_trade".to_string());
        verbs.insert("isda.dispute".to_string());
        verbs.insert("isda.manage_netting_set".to_string());

        // Workflow management verbs from EBNF
        verbs.insert("define-kyc-investigation".to_string());

        // Graph construction verbs from EBNF
        verbs.insert("entity".to_string());
        verbs.insert("edge".to_string());

        verbs
    }
}

impl SyntaxValidator {
    pub fn new() -> AgentResult<Self> {
        let s_expr_pattern = Regex::new(r"\([^()]*\)").map_err(|e| {
            AgentError::InitializationError(format!("Failed to compile S-expression regex: {}", e))
        })?;

        let keyword_pattern = Regex::new(r":[a-z][a-z0-9-_]*").map_err(|e| {
            AgentError::InitializationError(format!("Failed to compile keyword regex: {}", e))
        })?;

        let string_pattern = Regex::new(r#""[^"]*""#).map_err(|e| {
            AgentError::InitializationError(format!("Failed to compile string regex: {}", e))
        })?;

        Ok(Self {
            s_expr_pattern,
            keyword_pattern,
            string_pattern,
        })
    }

    pub fn validate(&self, dsl: &str) -> AgentResult<ValidationResult> {
        let mut errors = Vec::new();
        let mut warnings = Vec::new();

        // Check balanced parentheses
        if let Err(error) = self.check_balanced_parentheses(dsl) {
            errors.push(error);
        }

        // Check keyword syntax
        self.check_keyword_syntax(dsl, &mut warnings);

        // Check string literals
        self.check_string_literals(dsl, &mut warnings);

        let is_valid = errors.is_empty();
        let validation_score = if is_valid { 1.0 } else { 0.0 };

        Ok(ValidationResult {
            is_valid,
            validation_score,
            errors,
            warnings,
            suggestions: vec![
                "Ensure all parentheses are balanced".to_string(),
                "Use :keyword-style for parameters".to_string(),
                "Wrap strings in double quotes".to_string(),
            ],
            metrics: QualityMetrics {
                confidence: if is_valid { 0.9 } else { 0.1 },
                validation_score,
                completeness: 1.0,
                coherence: if is_valid { 0.9 } else { 0.3 },
                approved_verbs_count: 0,
                unapproved_verbs_count: 0,
                processing_time_ms: 0,
            },
            summary: if is_valid {
                "✅ DSL syntax is valid".to_string()
            } else {
                "❌ DSL syntax errors detected".to_string()
            },
        })
    }

    fn check_balanced_parentheses(&self, dsl: &str) -> Result<(), ValidationError> {
        let mut depth = 0;
        let mut line_no = 1;
        let mut col_no = 1;

        for ch in dsl.chars() {
            match ch {
                '(' => depth += 1,
                ')' => {
                    depth -= 1;
                    if depth < 0 {
                        return Err(ValidationError {
                            category: ErrorCategory::Syntax,
                            message: "Unmatched closing parenthesis".to_string(),
                            location: Some(Location {
                                line: line_no,
                                column: col_no,
                                length: 1,
                            }),
                            severity: Severity::Error,
                            suggestion: Some("Check for missing opening parenthesis".to_string()),
                        });
                    }
                }
                '\n' => {
                    line_no += 1;
                    col_no = 0;
                }
                _ => {}
            }
            col_no += 1;
        }

        if depth > 0 {
            return Err(ValidationError {
                category: ErrorCategory::Syntax,
                message: format!("Unmatched opening parentheses: {} unclosed", depth),
                location: None,
                severity: Severity::Error,
                suggestion: Some("Add missing closing parentheses".to_string()),
            });
        }

        Ok(())
    }

    fn check_keyword_syntax(&self, dsl: &str, warnings: &mut Vec<ValidationWarning>) {
        // Check for parameters that should be keywords but aren't
        let parameter_pattern = Regex::new(r"\b([a-z][a-z-]*)\s+").unwrap();
        for captures in parameter_pattern.captures_iter(dsl) {
            if let Some(param_match) = captures.get(1) {
                let param = param_match.as_str();
                if matches!(param, "cbu-id" | "nature-purpose" | "risk-level") {
                    warnings.push(ValidationWarning {
                        category: WarningCategory::Style,
                        message: format!("Consider using keyword syntax: :{}", param),
                        location: Some(Location {
                            line: 1, // Simplified for now
                            column: param_match.start() + 1,
                            length: param.len(),
                        }),
                        suggestion: Some(format!("Replace '{}' with ':{}'", param, param)),
                    });
                }
            }
        }
    }

    fn check_string_literals(&self, dsl: &str, warnings: &mut Vec<ValidationWarning>) {
        // Check for unquoted string values that should be quoted
        let unquoted_pattern = Regex::new(r"\s([A-Z][A-Z_]+)\s").unwrap();
        for captures in unquoted_pattern.captures_iter(dsl) {
            if let Some(value_match) = captures.get(1) {
                let value = value_match.as_str();
                warnings.push(ValidationWarning {
                    category: WarningCategory::Style,
                    message: format!("Consider quoting string literal: '{}'", value),
                    location: Some(Location {
                        line: 1, // Simplified for now
                        column: value_match.start() + 1,
                        length: value.len(),
                    }),
                    suggestion: Some(format!("Replace '{}' with \"{}\"", value, value)),
                });
            }
        }
    }
}

impl Default for SemanticValidator {
    fn default() -> Self {
        Self::new()
    }
}

impl SemanticValidator {
    pub fn new() -> Self {
        let required_patterns = Self::load_required_patterns();
        let business_rules = Self::load_business_rules();

        Self {
            required_patterns,
            business_rules,
        }
    }

    pub fn validate(&self, dsl: &str) -> AgentResult<ValidationResult> {
        let mut errors = Vec::new();
        let mut warnings = Vec::new();

        // Check required patterns for different domains
        self.validate_onboarding_patterns(dsl, &mut errors, &mut warnings);
        self.validate_kyc_patterns(dsl, &mut errors, &mut warnings);

        // Apply business rules
        for rule in &self.business_rules {
            if !(rule.condition)(dsl) {
                errors.push(ValidationError {
                    category: ErrorCategory::BusinessRule,
                    message: rule.error_message.clone(),
                    location: None,
                    severity: Severity::Error,
                    suggestion: Some(format!("Ensure compliance with rule: {}", rule.name)),
                });
            }
        }

        let is_valid = errors.is_empty();
        let validation_score = if is_valid { 1.0 } else { 0.0 };

        Ok(ValidationResult {
            is_valid,
            validation_score,
            errors,
            warnings,
            suggestions: vec![
                "Ensure required workflow patterns are present".to_string(),
                "Validate business rule compliance".to_string(),
            ],
            metrics: QualityMetrics {
                confidence: if is_valid { 0.85 } else { 0.2 },
                validation_score,
                completeness: 1.0,
                coherence: 1.0,
                approved_verbs_count: 0,
                unapproved_verbs_count: 0,
                processing_time_ms: 0,
            },
            summary: if is_valid {
                "✅ Semantic validation passed".to_string()
            } else {
                "❌ Semantic validation failed".to_string()
            },
        })
    }

    fn validate_onboarding_patterns(
        &self,
        dsl: &str,
        errors: &mut Vec<ValidationError>,
        _warnings: &mut Vec<ValidationWarning>,
    ) {
        if dsl.contains("case.create") {
            // Onboarding must have CBU context
            if !dsl.contains("cbu") && !dsl.contains("CBU") {
                errors.push(ValidationError {
                    category: ErrorCategory::Semantic,
                    message: "Onboarding case must specify CBU identifier".to_string(),
                    location: None,
                    severity: Severity::Error,
                    suggestion: Some("Add CBU identifier to case.create".to_string()),
                });
            }
        }
    }

    fn validate_kyc_patterns(
        &self,
        dsl: &str,
        errors: &mut Vec<ValidationError>,
        _warnings: &mut Vec<ValidationWarning>,
    ) {
        if dsl.contains("kyc.") {
            // KYC must have entity reference
            if !dsl.contains("entity-id") && !dsl.contains("target") {
                errors.push(ValidationError {
                    category: ErrorCategory::Semantic,
                    message: "KYC operations must reference target entity".to_string(),
                    location: None,
                    severity: Severity::Error,
                    suggestion: Some("Add entity-id or target reference".to_string()),
                });
            }
        }
    }

    fn load_required_patterns() -> HashMap<String, Vec<String>> {
        let mut patterns = HashMap::new();

        patterns.insert(
            "onboarding".to_string(),
            vec![
                "case.create".to_string(),
                "cbu".to_string(),
                "nature-purpose".to_string(),
            ],
        );

        patterns.insert(
            "kyc".to_string(),
            vec!["kyc.".to_string(), "entity".to_string()],
        );

        patterns
    }

    fn load_business_rules() -> Vec<BusinessRule> {
        vec![
            BusinessRule {
                name: "CBU_NAME_REQUIRED".to_string(),
                description: "CBU name must be specified for onboarding cases".to_string(),
                condition: |dsl| !dsl.contains("case.create") || dsl.contains("cbu"),
                error_message: "CBU identifier required for case creation".to_string(),
            },
            BusinessRule {
                name: "KYC_ENTITY_REQUIRED".to_string(),
                description: "KYC operations must reference an entity".to_string(),
                condition: |dsl| {
                    !dsl.contains("kyc.") || dsl.contains("entity") || dsl.contains("target")
                },
                error_message: "Entity reference required for KYC operations".to_string(),
            },
        ]
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_validator_creation() {
        let validator = DslValidator::new().unwrap();
        assert!(validator.verb_validator.approved_verbs.len() > 70);
    }

    #[test]
    fn test_valid_onboarding_dsl() {
        let validator = DslValidator::new().unwrap();
        let valid_dsl = r#"
            (case.create :cbu-id "CBU-1234" :nature-purpose "Investment fund")
            (products.add "CUSTODY")
            (kyc.start :target "CBU-1234")
        "#;

        let result = validator.validate(valid_dsl).unwrap();
        assert!(result.is_valid);
        assert!(result.validation_score > 0.8);
    }

    #[test]
    fn test_invalid_verb_detection() {
        let validator = DslValidator::new().unwrap();
        let invalid_dsl = r#"
            (case.create :cbu-id "CBU-1234")
            (invalid.verb "test")
            (another.bad.verb "value")
        "#;

        let result = validator.validate(invalid_dsl).unwrap();
        assert!(!result.is_valid);
        assert!(result.errors.len() >= 2);
        assert!(result
            .errors
            .iter()
            .any(|e| e.message.contains("invalid.verb")));
    }

    #[test]
    fn test_syntax_validation() {
        let validator = DslValidator::new().unwrap();
        let unbalanced_dsl = "(case.create (cbu.id \"test\"";

        let result = validator.validate(unbalanced_dsl).unwrap();
        assert!(!result.is_valid);
        assert!(result
            .errors
            .iter()
            .any(|e| e.category == ErrorCategory::Syntax));
    }

    #[test]
    fn test_approved_verbs_count() {
        let verb_validator = VerbValidator::new().unwrap();
        assert!(verb_validator.approved_verbs.len() >= 70);
        assert!(verb_validator.approved_verbs.contains("case.create"));
        assert!(verb_validator.approved_verbs.contains("ubo.resolve-ubos"));
        assert!(verb_validator.approved_verbs.contains("document.catalog"));
        assert!(verb_validator
            .approved_verbs
            .contains("isda.establish_master"));
    }

    #[test]
    fn test_unapproved_verb_detection() {
        let verb_validator = VerbValidator::new().unwrap();
        let dsl_with_bad_verbs = "(case.create) (bad.verb) (another.invalid)";

        let unapproved = verb_validator.find_unapproved_verbs(dsl_with_bad_verbs);
        assert!(unapproved.contains(&"bad.verb".to_string()));
        assert!(unapproved.contains(&"another.invalid".to_string()));
        assert!(!unapproved.contains(&"case.create".to_string()));
    }

    #[test]
    fn test_document_library_verbs() {
        let verb_validator = VerbValidator::new().unwrap();
        let dsl = r#"
            (document.catalog :doc-id "doc-001" :doc-type "certificate")
            (document.verify :doc-id "doc-001" :status "verified")
            (document.extract :doc-id "doc-001" :fields {:name "Test Corp"})
        "#;

        let result = verb_validator.validate_verbs(dsl).unwrap();
        assert!(result.is_valid, "Document library verbs should be valid");
        assert!(result.metrics.approved_verbs_count >= 3);
    }

    #[test]
    fn test_isda_derivative_verbs() {
        let verb_validator = VerbValidator::new().unwrap();
        let dsl = r#"
            (isda.establish_master :agreement-id "ISDA-001" :party-a "entity-a")
            (isda.execute_trade :trade-id "TRADE-001" :product-type "IRS")
            (isda.margin_call :call-id "MC-001" :call-amount 50000)
        "#;

        let result = verb_validator.validate_verbs(dsl).unwrap();
        assert!(result.is_valid, "ISDA derivative verbs should be valid");
        assert!(result.metrics.approved_verbs_count >= 3);
    }
}
