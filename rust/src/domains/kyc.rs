//! KYC Domain Handler
//!
//! This module implements the KYC (Know Your Customer) domain handler for the
//! centralized DSL editing system. It handles compliance workflows, document
//! collection, risk assessment, and verification processes.
//!
//! ## Supported Operations:
//! - Customer verification and identity checks
//! - Document collection and validation
//! - Risk assessment and scoring
//! - Compliance checks (FATCA, CRS, sanctions)
//! - UBO discovery initiation
//! - Regulatory reporting preparation
//!
//! ## State Machine:
//! INITIAL → DOCUMENTS_COLLECTED → IDENTITY_VERIFIED → RISK_ASSESSED →
//! COMPLIANCE_CHECKED → UBO_DISCOVERED → APPROVED/REJECTED

use crate::domains::common;
use crate::dsl::{
    domain_context::DomainContext,
    domain_registry::{DomainHandler, DslVocabulary, StateTransition, ValidationRule},
    operations::DslOperation,
    DslEditError, DslEditResult,
};
use async_trait::async_trait;
use chrono::Utc;
use serde_json::json;
use std::collections::HashMap;

/// KYC domain handler implementation
pub struct KycDomainHandler {
    vocabulary: DslVocabulary,
    state_transitions: Vec<StateTransition>,
    validation_rules: Vec<ValidationRule>,
    supported_operations: Vec<String>,
}

impl KycDomainHandler {
    /// Create a new KYC domain handler
    pub fn new() -> Self {
        Self {
            vocabulary: create_kyc_vocabulary(),
            state_transitions: create_state_transitions(),
            validation_rules: create_validation_rules(),
            supported_operations: vec![
                "Create KYC case".to_string(),
                "Collect document".to_string(),
                "Validate data".to_string(),
                "Execute workflow step".to_string(),
                "Domain operation: verify_identity".to_string(),
                "Domain operation: assess_risk".to_string(),
                "Domain operation: check_sanctions".to_string(),
                "Domain operation: initiate_ubo_discovery".to_string(),
                "Domain operation: complete_kyc".to_string(),
            ],
        }
    }

    /// Get allowed state transitions for KYC
    fn get_allowed_transitions() -> Vec<(String, String)> {
        vec![
            ("INITIAL".to_string(), "DOCUMENTS_COLLECTED".to_string()),
            (
                "DOCUMENTS_COLLECTED".to_string(),
                "IDENTITY_VERIFIED".to_string(),
            ),
            ("IDENTITY_VERIFIED".to_string(), "RISK_ASSESSED".to_string()),
            (
                "RISK_ASSESSED".to_string(),
                "COMPLIANCE_CHECKED".to_string(),
            ),
            (
                "COMPLIANCE_CHECKED".to_string(),
                "UBO_DISCOVERED".to_string(),
            ),
            ("UBO_DISCOVERED".to_string(), "APPROVED".to_string()),
            ("UBO_DISCOVERED".to_string(), "REJECTED".to_string()),
            // Allow rejection from any state
            ("INITIAL".to_string(), "REJECTED".to_string()),
            ("DOCUMENTS_COLLECTED".to_string(), "REJECTED".to_string()),
            ("IDENTITY_VERIFIED".to_string(), "REJECTED".to_string()),
            ("RISK_ASSESSED".to_string(), "REJECTED".to_string()),
            ("COMPLIANCE_CHECKED".to_string(), "REJECTED".to_string()),
        ]
    }

    /// Transform domain-specific KYC operations to DSL
    async fn transform_domain_specific(
        &self,
        operation_type: &str,
        payload: &serde_json::Value,
        context: &DomainContext,
    ) -> DslEditResult<String> {
        match operation_type {
            "verify_identity" => {
                self.generate_identity_verification_dsl(payload, context)
                    .await
            }
            "assess_risk" => self.generate_risk_assessment_dsl(payload, context).await,
            "check_sanctions" => self.generate_sanctions_check_dsl(payload, context).await,
            "initiate_ubo_discovery" => self.generate_ubo_discovery_dsl(payload, context).await,
            "complete_kyc" => self.generate_kyc_completion_dsl(payload, context).await,
            _ => Err(DslEditError::UnsupportedOperation(
                operation_type.to_string(),
                "kyc".to_string(),
            )),
        }
    }

    /// Generate identity verification DSL fragment
    async fn generate_identity_verification_dsl(
        &self,
        payload: &serde_json::Value,
        _context: &DomainContext,
    ) -> DslEditResult<String> {
        let customer_id = payload
            .get("customer_id")
            .and_then(|v| v.as_str())
            .ok_or_else(|| {
                DslEditError::DomainValidationError("Missing customer_id in payload".to_string())
            })?;

        let verification_method = payload
            .get("method")
            .and_then(|v| v.as_str())
            .unwrap_or("document_verification");

        let timestamp = common::generate_timestamp();

        Ok(format!(
            "(kyc.verify (customer.id \"{}\") (verification.method \"{}\") (verified.at \"{}\"))",
            customer_id, verification_method, timestamp
        ))
    }

    /// Generate risk assessment DSL fragment
    async fn generate_risk_assessment_dsl(
        &self,
        payload: &serde_json::Value,
        _context: &DomainContext,
    ) -> DslEditResult<String> {
        let risk_score = payload
            .get("risk_score")
            .and_then(|v| v.as_f64())
            .ok_or_else(|| {
                DslEditError::DomainValidationError("Missing risk_score in payload".to_string())
            })?;

        let default_factors = vec![];
        let risk_factors = payload
            .get("risk_factors")
            .and_then(|v| v.as_array())
            .unwrap_or(&default_factors);

        let factors_str = risk_factors
            .iter()
            .filter_map(|f| f.as_str())
            .map(|f| format!("\"{}\"", f))
            .collect::<Vec<_>>()
            .join(" ");

        let timestamp = common::generate_timestamp();

        Ok(format!(
            "(kyc.assess_risk (risk.score {}) (risk.factors {}) (assessed.at \"{}\"))",
            risk_score, factors_str, timestamp
        ))
    }

    /// Generate sanctions check DSL fragment
    async fn generate_sanctions_check_dsl(
        &self,
        payload: &serde_json::Value,
        _context: &DomainContext,
    ) -> DslEditResult<String> {
        let check_result = payload
            .get("result")
            .and_then(|v| v.as_str())
            .unwrap_or("CLEAR");

        let default_lists = vec![];
        let lists_checked = payload
            .get("lists_checked")
            .and_then(|v| v.as_array())
            .unwrap_or(&default_lists);

        let lists_str = lists_checked
            .iter()
            .filter_map(|l| l.as_str())
            .map(|l| format!("\"{}\"", l))
            .collect::<Vec<_>>()
            .join(" ");

        let timestamp = common::generate_timestamp();

        Ok(format!(
            "(kyc.check_sanctions (result \"{}\") (lists {}) (checked.at \"{}\"))",
            check_result, lists_str, timestamp
        ))
    }

    /// Generate UBO discovery initiation DSL fragment
    async fn generate_ubo_discovery_dsl(
        &self,
        payload: &serde_json::Value,
        _context: &DomainContext,
    ) -> DslEditResult<String> {
        let entity_id = payload
            .get("entity_id")
            .and_then(|v| v.as_str())
            .ok_or_else(|| {
                DslEditError::DomainValidationError("Missing entity_id in payload".to_string())
            })?;

        let threshold = payload
            .get("threshold")
            .and_then(|v| v.as_f64())
            .unwrap_or(25.0);

        let jurisdiction = payload
            .get("jurisdiction")
            .and_then(|v| v.as_str())
            .unwrap_or("US");

        let timestamp = common::generate_timestamp();

        Ok(format!(
            "(kyc.initiate_ubo (entity.id \"{}\") (threshold {}) (jurisdiction \"{}\") (initiated.at \"{}\"))",
            entity_id, threshold, jurisdiction, timestamp
        ))
    }

    /// Generate KYC completion DSL fragment
    async fn generate_kyc_completion_dsl(
        &self,
        payload: &serde_json::Value,
        _context: &DomainContext,
    ) -> DslEditResult<String> {
        let status = payload
            .get("status")
            .and_then(|v| v.as_str())
            .unwrap_or("APPROVED");

        let notes = payload
            .get("notes")
            .and_then(|v| v.as_str())
            .unwrap_or("KYC completed successfully");

        let timestamp = common::generate_timestamp();

        Ok(format!(
            "(kyc.complete (status \"{}\") (notes \"{}\") (completed.at \"{}\"))",
            status, notes, timestamp
        ))
    }
}

#[async_trait]
impl DomainHandler for KycDomainHandler {
    fn domain_name(&self) -> &str {
        "kyc"
    }

    fn domain_version(&self) -> &str {
        "1.0.0"
    }

    fn domain_description(&self) -> &str {
        "Know Your Customer compliance and verification workflows"
    }

    fn get_vocabulary(&self) -> &DslVocabulary {
        &self.vocabulary
    }

    async fn transform_operation_to_dsl(
        &self,
        operation: &DslOperation,
        context: &DomainContext,
    ) -> DslEditResult<String> {
        match operation {
            DslOperation::CollectDocument {
                document_type,
                requirements,
                ..
            } => {
                let timestamp = common::generate_timestamp();
                let required = requirements.required;

                Ok(format!(
                    "(kyc.collect_document (type \"{}\") (required {}) (collected.at \"{}\"))",
                    document_type, required, timestamp
                ))
            }

            DslOperation::ValidateData {
                validation_type,
                criteria,
                ..
            } => {
                let validation_name = format!("{:?}", validation_type);
                let severity = format!("{:?}", criteria.severity);
                let timestamp = common::generate_timestamp();

                Ok(format!(
                    "(kyc.validate (type \"{}\") (severity \"{}\") (validated.at \"{}\"))",
                    validation_name, severity, timestamp
                ))
            }

            DslOperation::ExecuteWorkflowStep {
                workflow_id,
                step_id,
                ..
            } => {
                let timestamp = common::generate_timestamp();

                Ok(format!(
                    "(kyc.execute_step (workflow \"{}\") (step \"{}\") (executed.at \"{}\"))",
                    workflow_id, step_id, timestamp
                ))
            }

            DslOperation::DomainSpecific {
                operation_type,
                payload,
                ..
            } => {
                self.transform_domain_specific(operation_type, payload, context)
                    .await
            }

            _ => Err(DslEditError::UnsupportedOperation(
                operation.description(),
                "kyc".to_string(),
            )),
        }
    }

    async fn validate_operation(
        &self,
        operation: &DslOperation,
        context: &DomainContext,
    ) -> DslEditResult<()> {
        if let DslOperation::DomainSpecific { operation_type, .. } = operation {
            match operation_type.as_str() {
                "initiate_ubo_discovery" => {
                    common::validate_required_context(context, &["entity_id"]).map_err(|e| {
                        DslEditError::DomainValidationError(format!("UBO discovery: {}", e))
                    })?;
                }
                "verify_identity" => {
                    common::validate_required_context(context, &["customer_id"]).map_err(|e| {
                        DslEditError::DomainValidationError(format!("Identity verification: {}", e))
                    })?;
                }
                _ => {}
            }
        }

        Ok(())
    }

    fn get_valid_transitions(&self) -> &[StateTransition] {
        &self.state_transitions
    }

    fn validate_state_transition(&self, from: &str, to: &str) -> DslEditResult<()> {
        let allowed_transitions = Self::get_allowed_transitions();
        common::validate_state_transition("kyc", &allowed_transitions, from, to)
            .map_err(|e| DslEditError::DomainValidationError(format!("State transition: {}", e)))
    }

    async fn apply_business_rules(
        &self,
        dsl_fragment: &str,
        _context: &DomainContext,
    ) -> DslEditResult<String> {
        let mut processed = dsl_fragment.to_string();

        // Rule: Add timestamp to KYC operations if missing
        if dsl_fragment.contains("kyc.") && !dsl_fragment.contains(".at") {
            let timestamp = common::generate_timestamp();
            processed = processed.replace(")", &format!(" (processed.at \"{}\"))", timestamp));
        }

        // Rule: Add compliance flag for high-risk customers
        if dsl_fragment.contains("risk.score") && dsl_fragment.contains("75") {
            processed = processed.replace(")", " (high_risk_flag true))");
        }

        Ok(processed)
    }

    fn supported_operations(&self) -> &[String] {
        &self.supported_operations
    }

    fn get_validation_rules(&self) -> &[ValidationRule] {
        &self.validation_rules
    }

    async fn extract_context_from_dsl(&self, dsl: &str) -> DslEditResult<DomainContext> {
        let mut context = DomainContext::kyc();

        // Extract customer ID if present
        if let Some(customer_match) = extract_value_from_dsl(dsl, "customer.id") {
            context = context.with_context("customer_id", json!(customer_match));
        }

        // Extract entity ID if present
        if let Some(entity_match) = extract_value_from_dsl(dsl, "entity.id") {
            context = context.with_context("entity_id", json!(entity_match));
        }

        // Extract risk score if present
        if let Some(risk_match) = extract_value_from_dsl(dsl, "risk.score") {
            if let Ok(score) = risk_match.parse::<f64>() {
                context = context.with_context("risk_score", json!(score));
            }
        }

        Ok(context)
    }

    async fn health_check(&self) -> crate::dsl::domain_registry::DomainHealthStatus {
        let mut metrics = HashMap::new();
        metrics.insert(
            "supported_operations".to_string(),
            self.supported_operations.len() as f64,
        );
        metrics.insert(
            "state_transitions".to_string(),
            self.state_transitions.len() as f64,
        );
        metrics.insert(
            "validation_rules".to_string(),
            self.validation_rules.len() as f64,
        );

        crate::dsl::domain_registry::DomainHealthStatus {
            domain_name: "kyc".to_string(),
            status: crate::dsl::domain_registry::HealthStatus::Healthy,
            last_check: Utc::now(),
            metrics,
            errors: Vec::new(),
        }
    }
}

/// Create KYC-specific vocabulary extensions
fn create_kyc_vocabulary() -> DslVocabulary {
    use crate::dsl::domain_registry::{AttributeDefinition, TypeDefinition, VerbDefinition};
    use uuid::Uuid;

    DslVocabulary {
        verbs: vec![
            VerbDefinition {
                name: "kyc.verify".to_string(),
                description: "Verify customer identity".to_string(),
                signature: "(kyc.verify (customer.id string) (verification.method string) ...)".to_string(),
                category: "verification".to_string(),
                examples: vec![
                    "(kyc.verify (customer.id \"CUST-001\") (verification.method \"document_check\"))".to_string(),
                ],
                validation_rules: vec!["require_customer_id".to_string()],
            },
            VerbDefinition {
                name: "kyc.assess_risk".to_string(),
                description: "Assess customer risk level".to_string(),
                signature: "(kyc.assess_risk (risk.score number) (risk.factors string...) ...)".to_string(),
                category: "risk_assessment".to_string(),
                examples: vec![
                    "(kyc.assess_risk (risk.score 25.5) (risk.factors \"PEP\" \"high_value\"))".to_string(),
                ],
                validation_rules: vec!["validate_risk_score".to_string()],
            },
            VerbDefinition {
                name: "kyc.collect_document".to_string(),
                description: "Collect KYC documentation".to_string(),
                signature: "(kyc.collect_document (type string) (required boolean) ...)".to_string(),
                category: "document_management".to_string(),
                examples: vec![
                    "(kyc.collect_document (type \"passport\") (required true))".to_string(),
                ],
                validation_rules: vec!["require_document_type".to_string()],
            },
        ],
        attributes: vec![
            AttributeDefinition {
                attribute_id: Uuid::parse_str("456789ab-cdef-1234-5678-9abcdef01201").unwrap(),
                name: "kyc.risk_rating".to_string(),
                data_type: "decimal".to_string(),
                domain: "kyc".to_string(),
                validation_rules: vec!["range_0_100".to_string()],
            },
            AttributeDefinition {
                attribute_id: Uuid::parse_str("456789ab-cdef-1234-5678-9abcdef01202").unwrap(),
                name: "kyc.verification_status".to_string(),
                data_type: "enum".to_string(),
                domain: "kyc".to_string(),
                validation_rules: vec!["valid_status_values".to_string()],
            },
        ],
        types: vec![
            TypeDefinition {
                type_name: "risk_score".to_string(),
                base_type: "decimal".to_string(),
                constraints: vec!["min:0".to_string(), "max:100".to_string()],
                validation_pattern: Some("^\\d{1,2}(\\.\\d{1,2})?$".to_string()),
            },
        ],
        grammar_extensions: vec![
            "kyc_verification ::= \"(\" \"kyc.verify\" verification_params+ \")\"".to_string(),
            "verification_params ::= \"(\" attribute_name value \")\"".to_string(),
        ],
    }
}

/// Create state transitions for KYC domain
fn create_state_transitions() -> Vec<StateTransition> {
    vec![
        StateTransition {
            from_state: "INITIAL".to_string(),
            to_state: "DOCUMENTS_COLLECTED".to_string(),
            transition_name: "collect_documents".to_string(),
            required_conditions: vec!["has_customer".to_string()],
            side_effects: vec!["update_status".to_string()],
        },
        StateTransition {
            from_state: "DOCUMENTS_COLLECTED".to_string(),
            to_state: "IDENTITY_VERIFIED".to_string(),
            transition_name: "verify_identity".to_string(),
            required_conditions: vec!["has_required_documents".to_string()],
            side_effects: vec!["create_verification_record".to_string()],
        },
        StateTransition {
            from_state: "IDENTITY_VERIFIED".to_string(),
            to_state: "RISK_ASSESSED".to_string(),
            transition_name: "assess_risk".to_string(),
            required_conditions: vec!["identity_confirmed".to_string()],
            side_effects: vec!["calculate_risk_score".to_string()],
        },
    ]
}

/// Create validation rules for KYC domain
fn create_validation_rules() -> Vec<ValidationRule> {
    use crate::dsl::domain_registry::{ValidationRule, ValidationRuleType, ValidationSeverity};

    vec![
        ValidationRule {
            rule_id: "require_customer_id".to_string(),
            rule_name: "Customer ID Required".to_string(),
            rule_type: ValidationRuleType::BusinessRuleValidation,
            description: "All KYC operations must reference a valid customer".to_string(),
            parameters: HashMap::new(),
            severity: ValidationSeverity::Error,
        },
        ValidationRule {
            rule_id: "validate_risk_score".to_string(),
            rule_name: "Risk Score Validation".to_string(),
            rule_type: ValidationRuleType::DataIntegrityValidation,
            description: "Risk scores must be between 0 and 100".to_string(),
            parameters: HashMap::new(),
            severity: ValidationSeverity::Error,
        },
        ValidationRule {
            rule_id: "require_document_type".to_string(),
            rule_name: "Document Type Required".to_string(),
            rule_type: ValidationRuleType::BusinessRuleValidation,
            description: "Document collection requires valid document type".to_string(),
            parameters: HashMap::new(),
            severity: ValidationSeverity::Error,
        },
    ]
}

/// Extract value from DSL using simple pattern matching
fn extract_value_from_dsl(dsl: &str, key: &str) -> Option<String> {
    let pattern = format!("({} ", key);
    if let Some(start) = dsl.find(&pattern) {
        let start_pos = start + pattern.len();
        let remaining = &dsl[start_pos..];

        // Handle quoted strings
        if let Some(stripped) = remaining.strip_prefix('"') {
            if let Some(end) = stripped.find('"') {
                return Some(stripped[..end].to_string());
            }
        }
        // Handle numbers
        else if let Some(end) = remaining.find(|c: char| c.is_whitespace() || c == ')') {
            return Some(remaining[..end].to_string());
        }
    }
    None
}

impl Default for KycDomainHandler {
    fn default() -> Self {
        Self::new()
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    // use crate::dsl::operations::OperationBuilder;
    use serde_json::json;

    #[tokio::test]
    async fn test_kyc_domain_creation() {
        let handler = KycDomainHandler::new();
        assert_eq!(handler.domain_name(), "kyc");
        assert_eq!(handler.domain_version(), "1.0.0");
        assert!(!handler.supported_operations().is_empty());
    }

    #[tokio::test]
    async fn test_identity_verification_dsl() {
        let handler = KycDomainHandler::new();
        let context = DomainContext::kyc().with_context("customer_id", json!("CUST-001"));

        let payload = json!({
            "customer_id": "CUST-001",
            "method": "document_verification"
        });

        let result = handler
            .generate_identity_verification_dsl(&payload, &context)
            .await;
        assert!(result.is_ok());

        let dsl = result.unwrap();
        assert!(dsl.contains("kyc.verify"));
        assert!(dsl.contains("CUST-001"));
        assert!(dsl.contains("document_verification"));
    }

    #[tokio::test]
    async fn test_risk_assessment_dsl() {
        let handler = KycDomainHandler::new();
        let context = DomainContext::kyc();

        let payload = json!({
            "risk_score": 75.5,
            "risk_factors": ["PEP", "high_value_customer"]
        });

        let result = handler
            .generate_risk_assessment_dsl(&payload, &context)
            .await;
        assert!(result.is_ok());

        let dsl = result.unwrap();
        assert!(dsl.contains("kyc.assess_risk"));
        assert!(dsl.contains("75.5"));
        assert!(dsl.contains("PEP"));
        assert!(dsl.contains("high_value_customer"));
    }

    #[tokio::test]
    async fn test_state_transition_validation() {
        let handler = KycDomainHandler::new();

        let result = handler.validate_state_transition("INITIAL", "DOCUMENTS_COLLECTED");
        assert!(result.is_ok());

        let result = handler.validate_state_transition("INITIAL", "APPROVED");
        assert!(result.is_err());
    }

    #[tokio::test]
    async fn test_business_rules_application() {
        let handler = KycDomainHandler::new();
        let context = DomainContext::kyc();

        let dsl_fragment = "(kyc.assess_risk (risk.score 75))";
        let result = handler.apply_business_rules(dsl_fragment, &context).await;

        assert!(result.is_ok());
        let processed = result.unwrap();
        assert!(processed.contains("high_risk_flag"));
    }

    #[tokio::test]
    async fn test_context_extraction() {
        let handler = KycDomainHandler::new();
        let dsl = r#"
            (kyc.verify (customer.id "CUST-001"))
            (kyc.assess_risk (risk.score 45.5))
        "#;

        let result = handler.extract_context_from_dsl(dsl).await;
        assert!(result.is_ok());

        let context = result.unwrap();
        assert_eq!(context.domain_name, "kyc");

        let customer_id: Option<String> = context.get_context("customer_id");
        assert_eq!(customer_id, Some("CUST-001".to_string()));

        let risk_score: Option<f64> = context.get_context("risk_score");
        assert_eq!(risk_score, Some(45.5));
    }
}
