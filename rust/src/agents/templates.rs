//! DSL Template Engine - Intelligent Template-Based DSL Generation
//!
//! This module provides sophisticated template-based DSL generation capabilities
//! that work with the DSL Agent to create consistent, validated DSL content.
//!
//! # Features
//!
//! - **Template-Based Generation**: Use predefined templates for consistent DSL structure
//! - **Variable Substitution**: Smart variable replacement with type checking
//! - **Business Logic Integration**: Templates include business rules and constraints
//! - **Multi-Domain Support**: Templates for all v3.1 domains (Document, ISDA, KYC, UBO, etc.)
//! - **Context-Aware**: Templates adapt based on business context and requirements
//!
//! # Template Types
//!
//! - **Onboarding Templates**: Complete onboarding workflows
//! - **KYC Templates**: Know Your Customer verification processes
//! - **Document Templates**: Document library operations
//! - **ISDA Templates**: Derivative contract workflows
//! - **UBO Templates**: Ultimate Beneficial Ownership discovery
//!
//! # Usage
//!
//! ```rust,ignore
//! let engine = DslTemplateEngine::new()?;
//! let context = TemplateContext::new()
//!     .with("cbu_name", "Alpha Holdings Singapore")
//!     .with("products", vec!["CUSTODY", "FUND_ACCOUNTING"]);
//!
//! let dsl = engine.generate_onboarding_dsl(context)?;
//! ```

use crate::agents::{AgentError, AgentResult};
use serde::{Deserialize, Serialize};
use std::collections::HashMap;
use tracing::{debug, info};
use uuid::Uuid;

/// DSL Template Engine for intelligent DSL generation
pub struct DslTemplateEngine {
    /// Loaded templates by domain and type
    templates: HashMap<String, DslTemplate>,

    /// Template metadata and relationships
    template_registry: TemplateRegistry,
}

/// Template context for variable substitution
#[derive(Debug, Clone)]
pub struct TemplateContext {
    variables: HashMap<String, TemplateValue>,
}

/// Template value that can be substituted into DSL
#[derive(Debug, Clone, Serialize, Deserialize)]
pub enum TemplateValue {
    String(String),
    Number(f64),
    Boolean(bool),
    Array(Vec<String>),
    Object(HashMap<String, String>),
    Uuid(Uuid),
    DateTime(chrono::DateTime<chrono::Utc>),
}

/// DSL Template definition
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct DslTemplate {
    /// Unique template identifier
    pub id: String,

    /// Domain this template belongs to
    pub domain: String,

    /// Template type/purpose
    pub template_type: String,

    /// Template content with variable placeholders
    pub content: String,

    /// Required variables for this template
    pub required_variables: Vec<String>,

    /// Optional variables with defaults
    pub optional_variables: HashMap<String, String>,

    /// Business rules and constraints
    pub constraints: Vec<TemplateConstraint>,

    /// Template metadata
    pub metadata: TemplateMetadata,
}

/// Template constraint for validation
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct TemplateConstraint {
    pub variable: String,
    pub constraint_type: ConstraintType,
    pub message: String,
}

/// Types of template constraints
#[derive(Debug, Clone, Serialize, Deserialize)]
pub enum ConstraintType {
    Required,
    MinLength(usize),
    MaxLength(usize),
    Pattern(String),
    OneOf(Vec<String>),
    Custom(String),
}

/// Template metadata
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct TemplateMetadata {
    pub version: String,
    pub description: String,
    pub author: String,
    pub created_at: String,
    pub tags: Vec<String>,
}

/// Template registry for managing relationships
#[derive(Debug, Clone)]
pub struct TemplateRegistry {
    /// Template dependencies (template_id -> dependencies)
    dependencies: HashMap<String, Vec<String>>,

    /// Template categories
    categories: HashMap<String, Vec<String>>,
}

impl DslTemplateEngine {
    /// Create new template engine with predefined templates
    pub fn new() -> AgentResult<Self> {
        info!("Initializing DSL Template Engine");

        let mut templates = HashMap::new();
        let template_registry = TemplateRegistry::new();

        // Load built-in templates
        Self::load_onboarding_templates(&mut templates)?;
        Self::load_kyc_templates(&mut templates)?;
        Self::load_document_templates(&mut templates)?;
        Self::load_isda_templates(&mut templates)?;

        info!(
            "Template engine initialized with {} templates",
            templates.len()
        );

        Ok(Self {
            templates,
            template_registry,
        })
    }

    /// Generate onboarding DSL from template and context
    pub fn generate_onboarding_dsl(&self, context: TemplateContext) -> AgentResult<String> {
        debug!("Generating onboarding DSL from template");

        let template = self.templates.get("onboarding_create_cbu").ok_or_else(|| {
            AgentError::TemplateError("Onboarding template not found".to_string())
        })?;

        self.apply_template(template, &context)
    }

    /// Generate KYC DSL from template and context
    pub fn generate_kyc_dsl(&self, context: TemplateContext) -> AgentResult<String> {
        debug!("Generating KYC DSL from template");

        let template = self
            .templates
            .get("kyc_verify_enhanced")
            .ok_or_else(|| AgentError::TemplateError("KYC template not found".to_string()))?;

        self.apply_template(template, &context)
    }

    /// Generate document library DSL from template and context
    pub fn generate_document_dsl(&self, context: TemplateContext) -> AgentResult<String> {
        debug!("Generating document DSL from template");

        let template = self
            .templates
            .get("document_catalog")
            .ok_or_else(|| AgentError::TemplateError("Document template not found".to_string()))?;

        self.apply_template(template, &context)
    }

    /// Generate ISDA derivative DSL from template and context
    pub fn generate_isda_dsl(&self, context: TemplateContext) -> AgentResult<String> {
        debug!("Generating ISDA DSL from template");

        let template = self
            .templates
            .get("isda_establish_master")
            .ok_or_else(|| AgentError::TemplateError("ISDA template not found".to_string()))?;

        self.apply_template(template, &context)
    }

    /// Apply template with context to generate DSL
    fn apply_template(
        &self,
        template: &DslTemplate,
        context: &TemplateContext,
    ) -> AgentResult<String> {
        debug!("Applying template: {}", template.id);

        // Validate required variables are present
        self.validate_template_context(template, context)?;

        // Apply variable substitution
        let mut result = template.content.clone();

        for (var_name, var_value) in &context.variables {
            let placeholder = format!("{{{}}}", var_name);
            let substitution = self.format_template_value(var_value)?;
            result = result.replace(&placeholder, &substitution);
        }

        // Apply optional variables with defaults
        for (var_name, default_value) in &template.optional_variables {
            let placeholder = format!("{{{}}}", var_name);
            if result.contains(&placeholder) {
                let value = context
                    .variables
                    .get(var_name)
                    .map(|v| self.format_template_value(v))
                    .transpose()?
                    .unwrap_or_else(|| default_value.clone());
                result = result.replace(&placeholder, &value);
            }
        }

        // Clean up any remaining placeholders
        result = self.clean_unused_placeholders(&result);

        debug!(
            "Template applied successfully, generated {} characters",
            result.len()
        );
        Ok(result)
    }

    fn validate_template_context(
        &self,
        template: &DslTemplate,
        context: &TemplateContext,
    ) -> AgentResult<()> {
        // Check required variables
        for required_var in &template.required_variables {
            if !context.variables.contains_key(required_var) {
                return Err(AgentError::TemplateError(format!(
                    "Required variable '{}' missing for template '{}'",
                    required_var, template.id
                )));
            }
        }

        // Apply constraints
        for constraint in &template.constraints {
            if let Some(value) = context.variables.get(&constraint.variable) {
                self.validate_constraint(constraint, value)?;
            }
        }

        Ok(())
    }

    fn validate_constraint(
        &self,
        constraint: &TemplateConstraint,
        value: &TemplateValue,
    ) -> AgentResult<()> {
        match &constraint.constraint_type {
            ConstraintType::Required => {
                // Always satisfied if value exists
                Ok(())
            }
            ConstraintType::MinLength(min_len) => {
                let str_value = self.format_template_value(value)?;
                if str_value.len() < *min_len {
                    Err(AgentError::TemplateError(constraint.message.clone()))
                } else {
                    Ok(())
                }
            }
            ConstraintType::MaxLength(max_len) => {
                let str_value = self.format_template_value(value)?;
                if str_value.len() > *max_len {
                    Err(AgentError::TemplateError(constraint.message.clone()))
                } else {
                    Ok(())
                }
            }
            ConstraintType::Pattern(_pattern) => {
                // TODO: Implement regex pattern validation
                Ok(())
            }
            ConstraintType::OneOf(options) => {
                let str_value = self.format_template_value(value)?;
                if options.contains(&str_value) {
                    Ok(())
                } else {
                    Err(AgentError::TemplateError(constraint.message.clone()))
                }
            }
            ConstraintType::Custom(_custom) => {
                // TODO: Implement custom validation logic
                Ok(())
            }
        }
    }

    fn format_template_value(&self, value: &TemplateValue) -> AgentResult<String> {
        match value {
            TemplateValue::String(s) => Ok(s.clone()),
            TemplateValue::Number(n) => Ok(n.to_string()),
            TemplateValue::Boolean(b) => Ok(b.to_string()),
            TemplateValue::Array(arr) => {
                let formatted = arr
                    .iter()
                    .map(|item| format!("\"{}\"", item))
                    .collect::<Vec<_>>()
                    .join(" ");
                Ok(formatted)
            }
            TemplateValue::Object(obj) => {
                let formatted = obj
                    .iter()
                    .map(|(k, v)| format!(":{} \"{}\"", k, v))
                    .collect::<Vec<_>>()
                    .join(" ");
                Ok(format!("{{{}}}", formatted))
            }
            TemplateValue::Uuid(uuid) => Ok(uuid.to_string()),
            TemplateValue::DateTime(dt) => Ok(dt.to_rfc3339()),
        }
    }

    fn clean_unused_placeholders(&self, content: &str) -> String {
        // Remove any remaining {variable} placeholders that weren't substituted
        let mut result = content.to_string();

        // Simple regex-like replacement for unused placeholders
        while let Some(start) = result.find('{') {
            if let Some(end) = result[start..].find('}') {
                let placeholder = &result[start..start + end + 1];
                result = result.replace(placeholder, "");
            } else {
                break;
            }
        }

        result
    }

    // Template loading methods
    fn load_onboarding_templates(templates: &mut HashMap<String, DslTemplate>) -> AgentResult<()> {
        let onboarding_template = DslTemplate {
            id: "onboarding_create_cbu".to_string(),
            domain: "onboarding".to_string(),
            template_type: "create_cbu".to_string(),
            content: r#";; Onboarding DSL for {cbu_name}
;; Generated: {timestamp}
;; Business Reference: {business_reference}

(case.create
  :cbu-id "{cbu_name}"
  :nature-purpose "{nature_purpose}"
  :jurisdiction "{jurisdiction}"
  :created-by "{created_by}")

(products.add {products})

{services_section}

(kyc.start
  :target "{cbu_name}"
  :risk-level "MEDIUM"
  :documents [(document "CertificateOfIncorporation")]
  :jurisdictions [(jurisdiction "{jurisdiction}")])

(compliance.screen
  :target "{cbu_name}"
  :screening-lists ["OFAC" "EU_SANCTIONS"]
  :result-required true)

(workflow.transition
  :from "CREATED"
  :to "KYC_PENDING"
  :reason "Initial onboarding workflow")"#
                .to_string(),
            required_variables: vec![
                "cbu_name".to_string(),
                "nature_purpose".to_string(),
                "jurisdiction".to_string(),
                "created_by".to_string(),
                "products".to_string(),
            ],
            optional_variables: {
                let mut optional = HashMap::new();
                optional.insert(
                    "services_section".to_string(),
                    "(services.discover)".to_string(),
                );
                optional.insert("timestamp".to_string(), chrono::Utc::now().to_rfc3339());
                optional.insert(
                    "business_reference".to_string(),
                    "AUTO_GENERATED".to_string(),
                );
                optional
            },
            constraints: vec![
                TemplateConstraint {
                    variable: "cbu_name".to_string(),
                    constraint_type: ConstraintType::MinLength(3),
                    message: "CBU name must be at least 3 characters".to_string(),
                },
                TemplateConstraint {
                    variable: "jurisdiction".to_string(),
                    constraint_type: ConstraintType::OneOf(vec![
                        "US".to_string(),
                        "LU".to_string(),
                        "SG".to_string(),
                        "KY".to_string(),
                        "UK".to_string(),
                        "HK".to_string(),
                    ]),
                    message: "Jurisdiction must be one of: US, LU, SG, KY, UK, HK".to_string(),
                },
            ],
            metadata: TemplateMetadata {
                version: "3.1.0".to_string(),
                description: "Standard onboarding template for creating new CBU cases".to_string(),
                author: "DSL Agent System".to_string(),
                created_at: chrono::Utc::now().to_rfc3339(),
                tags: vec![
                    "onboarding".to_string(),
                    "cbu".to_string(),
                    "create".to_string(),
                ],
            },
        };

        templates.insert(onboarding_template.id.clone(), onboarding_template);
        Ok(())
    }

    fn load_kyc_templates(templates: &mut HashMap<String, DslTemplate>) -> AgentResult<()> {
        let kyc_template = DslTemplate {
            id: "kyc_verify_enhanced".to_string(),
            domain: "kyc".to_string(),
            template_type: "verify".to_string(),
            content: r#";; KYC Verification DSL
;; Parent Onboarding: {parent_onboarding_id}
;; Generated: {timestamp}
;; Business Reference: {business_reference}

(kyc.verify
  :customer-id "{parent_onboarding_id}"
  :method "{verification_method}"
  :doc-types {required_documents}
  :verified-at "{timestamp}")

(kyc.collect_document
  :target "{parent_onboarding_id}"
  :doc-type "{kyc_type}"
  :status "PENDING"
  :collected-at "{timestamp}")

{special_instructions_section}

(kyc.screen_sanctions
  :target "{parent_onboarding_id}"
  :databases ["OFAC" "UN" "EU" "HMT"]
  :status "PENDING"
  :screened-at "{timestamp}")

(workflow.transition
  :from "KYC_INITIATED"
  :to "DOCUMENT_COLLECTION"
  :reason "KYC verification started")"#
                .to_string(),
            required_variables: vec![
                "parent_onboarding_id".to_string(),
                "kyc_type".to_string(),
                "risk_level".to_string(),
                "verification_method".to_string(),
                "required_documents".to_string(),
            ],
            optional_variables: {
                let mut optional = HashMap::new();
                optional.insert("special_instructions_section".to_string(), "".to_string());
                optional.insert("timestamp".to_string(), chrono::Utc::now().to_rfc3339());
                optional.insert(
                    "business_reference".to_string(),
                    "AUTO_GENERATED".to_string(),
                );
                optional
            },
            constraints: vec![
                TemplateConstraint {
                    variable: "kyc_type".to_string(),
                    constraint_type: ConstraintType::OneOf(vec![
                        "Enhanced DD".to_string(),
                        "Standard DD".to_string(),
                        "Simplified DD".to_string(),
                    ]),
                    message: "KYC type must be Enhanced DD, Standard DD, or Simplified DD"
                        .to_string(),
                },
                TemplateConstraint {
                    variable: "risk_level".to_string(),
                    constraint_type: ConstraintType::OneOf(vec![
                        "LOW".to_string(),
                        "MEDIUM".to_string(),
                        "HIGH".to_string(),
                    ]),
                    message: "Risk level must be LOW, MEDIUM, or HIGH".to_string(),
                },
            ],
            metadata: TemplateMetadata {
                version: "3.1.0".to_string(),
                description: "KYC verification template for enhanced due diligence".to_string(),
                author: "DSL Agent System".to_string(),
                created_at: chrono::Utc::now().to_rfc3339(),
                tags: vec![
                    "kyc".to_string(),
                    "verification".to_string(),
                    "due_diligence".to_string(),
                ],
            },
        };

        templates.insert(kyc_template.id.clone(), kyc_template);
        Ok(())
    }

    fn load_document_templates(templates: &mut HashMap<String, DslTemplate>) -> AgentResult<()> {
        let document_template = DslTemplate {
            id: "document_catalog".to_string(),
            domain: "document".to_string(),
            template_type: "catalog".to_string(),
            content: r#";; Document Library DSL
;; Generated: {timestamp}

(document.catalog
  :document-id "{doc_id}"
  :document-type "{doc_type}"
  :title "{doc_type} Document"
  :issuer "{created_by}"
  :issue-date "{timestamp}")

(document.verify
  :document-id "{doc_id}"
  :verification-method "{verification_method}"
  :status "VERIFIED"
  :confidence 0.95
  :verified-at "{timestamp}")

(document.extract
  :document-id "{doc_id}"
  :extraction-method "AI_POWERED"
  :extracted-data {extracted_data}
  :confidence 0.90)"#
                .to_string(),
            required_variables: vec![
                "doc_id".to_string(),
                "doc_type".to_string(),
                "created_by".to_string(),
                "verification_method".to_string(),
                "extracted_data".to_string(),
            ],
            optional_variables: {
                let mut optional = HashMap::new();
                optional.insert("timestamp".to_string(), chrono::Utc::now().to_rfc3339());
                optional
            },
            constraints: vec![],
            metadata: TemplateMetadata {
                version: "3.1.0".to_string(),
                description: "Document library cataloging template".to_string(),
                author: "DSL Agent System".to_string(),
                created_at: chrono::Utc::now().to_rfc3339(),
                tags: vec![
                    "document".to_string(),
                    "catalog".to_string(),
                    "library".to_string(),
                ],
            },
        };

        templates.insert(document_template.id.clone(), document_template);
        Ok(())
    }

    fn load_isda_templates(templates: &mut HashMap<String, DslTemplate>) -> AgentResult<()> {
        let isda_template = DslTemplate {
            id: "isda_establish_master".to_string(),
            domain: "isda".to_string(),
            template_type: "establish_master".to_string(),
            content: r#";; ISDA Master Agreement DSL
;; Generated: {timestamp}

(isda.establish_master
  :agreement-id "{agreement_id}"
  :party-a "{party_a}"
  :party-b "{party_b}"
  :version "2002"
  :governing-law "{governing_law}"
  :agreement-date "{timestamp}")

(isda.establish_csa
  :csa-id "{csa_id}"
  :master-agreement-id "{agreement_id}"
  :base-currency "USD"
  :threshold-party-a {threshold_a}
  :threshold-party-b {threshold_b})

(isda.execute_trade
  :trade-id "{trade_id}"
  :master-agreement-id "{agreement_id}"
  :product-type "{product_type}"
  :notional-amount {notional_amount})"#
                .to_string(),
            required_variables: vec![
                "agreement_id".to_string(),
                "party_a".to_string(),
                "party_b".to_string(),
                "governing_law".to_string(),
                "csa_id".to_string(),
                "threshold_a".to_string(),
                "threshold_b".to_string(),
                "trade_id".to_string(),
                "product_type".to_string(),
                "notional_amount".to_string(),
            ],
            optional_variables: {
                let mut optional = HashMap::new();
                optional.insert("timestamp".to_string(), chrono::Utc::now().to_rfc3339());
                optional
            },
            constraints: vec![],
            metadata: TemplateMetadata {
                version: "3.1.0".to_string(),
                description: "ISDA master agreement establishment template".to_string(),
                author: "DSL Agent System".to_string(),
                created_at: chrono::Utc::now().to_rfc3339(),
                tags: vec![
                    "isda".to_string(),
                    "derivatives".to_string(),
                    "master_agreement".to_string(),
                ],
            },
        };

        templates.insert(isda_template.id.clone(), isda_template);
        Ok(())
    }
}

impl Default for TemplateContext {
    fn default() -> Self {
        Self::new()
    }
}

impl TemplateContext {
    /// Create new empty template context
    pub fn new() -> Self {
        Self {
            variables: HashMap::new(),
        }
    }

    /// Add a variable to the context
    pub fn insert(&mut self, name: String, value: TemplateValue) {
        self.variables.insert(name, value);
    }

    /// Builder pattern for adding variables
    pub fn with(mut self, name: &str, value: impl Into<TemplateValue>) -> Self {
        self.variables.insert(name.to_string(), value.into());
        self
    }

    /// Get a variable from the context
    pub fn get(&self, name: &str) -> Option<&TemplateValue> {
        self.variables.get(name)
    }

    /// Check if context contains a variable
    pub fn contains(&self, name: &str) -> bool {
        self.variables.contains_key(name)
    }
}

impl Default for TemplateRegistry {
    fn default() -> Self {
        Self::new()
    }
}

impl TemplateRegistry {
    /// Create new template registry
    pub fn new() -> Self {
        Self {
            dependencies: HashMap::new(),
            categories: HashMap::new(),
        }
    }
}

// Conversion implementations for TemplateValue
impl From<String> for TemplateValue {
    fn from(s: String) -> Self {
        TemplateValue::String(s)
    }
}

impl From<&str> for TemplateValue {
    fn from(s: &str) -> Self {
        TemplateValue::String(s.to_string())
    }
}

impl From<f64> for TemplateValue {
    fn from(n: f64) -> Self {
        TemplateValue::Number(n)
    }
}

impl From<bool> for TemplateValue {
    fn from(b: bool) -> Self {
        TemplateValue::Boolean(b)
    }
}

impl From<Vec<String>> for TemplateValue {
    fn from(arr: Vec<String>) -> Self {
        TemplateValue::Array(arr)
    }
}

impl From<HashMap<String, String>> for TemplateValue {
    fn from(obj: HashMap<String, String>) -> Self {
        TemplateValue::Object(obj)
    }
}

impl From<Uuid> for TemplateValue {
    fn from(uuid: Uuid) -> Self {
        TemplateValue::Uuid(uuid)
    }
}

impl From<chrono::DateTime<chrono::Utc>> for TemplateValue {
    fn from(dt: chrono::DateTime<chrono::Utc>) -> Self {
        TemplateValue::DateTime(dt)
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_template_engine_new() {
        let engine = DslTemplateEngine::new().unwrap();
        assert!(engine.templates.len() > 0);
    }

    #[test]
    fn test_template_context_builder() {
        let context = TemplateContext::new()
            .with("test_string", "value")
            .with("test_number", 42.0)
            .with("test_bool", true);

        assert!(context.contains("test_string"));
        assert!(context.contains("test_number"));
        assert!(context.contains("test_bool"));
    }

    #[test]
    fn test_generate_onboarding_dsl() {
        let engine = DslTemplateEngine::new().unwrap();
        let context = TemplateContext::new()
            .with("cbu_name", "Test Company")
            .with("nature_purpose", "Investment management")
            .with("jurisdiction", "SG")
            .with("created_by", "test_user")
            .with("products", "\"CUSTODY\" \"FUND_ACCOUNTING\"")
            .with("business_reference", "OB-TEST-20241201")
            .with("timestamp", "2024-12-01T10:00:00Z");

        let result = engine.generate_onboarding_dsl(context);
        assert!(result.is_ok());
        let dsl = result.unwrap();
        assert!(dsl.contains("Test Company"));
        assert!(dsl.contains("Investment management"));
        assert!(dsl.contains("case.create"));
    }

    #[test]
    fn test_template_value_conversions() {
        let _string_val: TemplateValue = "test".into();
        let _number_val: TemplateValue = 42.0.into();
        let _bool_val: TemplateValue = true.into();
        let _array_val: TemplateValue = vec!["a".to_string(), "b".to_string()].into();
        let _uuid_val: TemplateValue = Uuid::new_v4().into();
    }
}
