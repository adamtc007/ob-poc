//! UBO Domain Handler
//!
//! This module implements the UBO (Ultimate Beneficial Ownership) domain handler for
//! the centralized DSL editing system. It handles entity relationship modeling,
//! ownership calculations, compliance thresholds, and beneficial ownership discovery.
//!
//! ## Supported Operations:
//! - Entity declaration and relationship creation
//! - Ownership percentage calculations
//! - UBO threshold analysis
//! - Regulatory compliance checks (FATF, FinCEN, etc.)
//! - Entity graph modeling and traversal
//! - Evidence collection and documentation
//!
//! ## State Machine:
//! INITIAL → ENTITIES_DECLARED → RELATIONSHIPS_MAPPED → OWNERSHIP_CALCULATED →
//! THRESHOLDS_ANALYZED → COMPLIANCE_VERIFIED → UBO_IDENTIFIED

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

/// UBO domain handler implementation
pub struct UboDomainHandler {
    vocabulary: DslVocabulary,
    state_transitions: Vec<StateTransition>,
    validation_rules: Vec<ValidationRule>,
    supported_operations: Vec<String>,
}

impl UboDomainHandler {
    /// Create a new UBO domain handler
    pub fn new() -> Self {
        Self {
            vocabulary: create_ubo_vocabulary(),
            state_transitions: create_state_transitions(),
            validation_rules: create_validation_rules(),
            supported_operations: vec![
                "Create relationship".to_string(),
                "Create Company entity".to_string(),
                "Create proper person entity".to_string(),
                "Domain operation: declare_entity".to_string(),
                "Domain operation: create_ownership_edge".to_string(),
                "Domain operation: calculate_ubo".to_string(),
                "Domain operation: analyze_thresholds".to_string(),
                "Domain operation: verify_compliance".to_string(),
                "Domain operation: identify_ubos".to_string(),
            ],
        }
    }

    /// Get allowed state transitions for UBO discovery
    fn get_allowed_transitions() -> Vec<(String, String)> {
        vec![
            ("INITIAL".to_string(), "ENTITIES_DECLARED".to_string()),
            (
                "ENTITIES_DECLARED".to_string(),
                "RELATIONSHIPS_MAPPED".to_string(),
            ),
            (
                "RELATIONSHIPS_MAPPED".to_string(),
                "OWNERSHIP_CALCULATED".to_string(),
            ),
            (
                "OWNERSHIP_CALCULATED".to_string(),
                "THRESHOLDS_ANALYZED".to_string(),
            ),
            (
                "THRESHOLDS_ANALYZED".to_string(),
                "COMPLIANCE_VERIFIED".to_string(),
            ),
            (
                "COMPLIANCE_VERIFIED".to_string(),
                "UBO_IDENTIFIED".to_string(),
            ),
            // Allow re-analysis from any state
            ("ENTITIES_DECLARED".to_string(), "INITIAL".to_string()),
            (
                "RELATIONSHIPS_MAPPED".to_string(),
                "ENTITIES_DECLARED".to_string(),
            ),
            (
                "OWNERSHIP_CALCULATED".to_string(),
                "RELATIONSHIPS_MAPPED".to_string(),
            ),
        ]
    }

    /// Transform domain-specific UBO operations to DSL
    async fn transform_domain_specific(
        &self,
        operation_type: &str,
        payload: &serde_json::Value,
        context: &DomainContext,
    ) -> DslEditResult<String> {
        match operation_type {
            "declare_entity" => self.generate_entity_declaration_dsl(payload, context).await,
            "create_ownership_edge" => self.generate_ownership_edge_dsl(payload, context).await,
            "calculate_ubo" => self.generate_ubo_calculation_dsl(payload, context).await,
            "analyze_thresholds" => self.generate_threshold_analysis_dsl(payload, context).await,
            "verify_compliance" => {
                self.generate_compliance_verification_dsl(payload, context)
                    .await
            }
            "identify_ubos" => self.generate_ubo_identification_dsl(payload, context).await,
            _ => Err(DslEditError::UnsupportedOperation(
                operation_type.to_string(),
                "ubo".to_string(),
            )),
        }
    }

    /// Generate entity declaration DSL fragment
    async fn generate_entity_declaration_dsl(
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

        let entity_type = payload
            .get("entity_type")
            .and_then(|v| v.as_str())
            .unwrap_or("Company");

        let default_properties = json!({});
        let properties = payload.get("properties").unwrap_or(&default_properties);

        let mut property_fragments = Vec::new();

        if let Some(legal_name) = properties.get("legal_name").and_then(|v| v.as_str()) {
            property_fragments.push(format!(":legal-name \"{}\"", legal_name));
        }

        if let Some(jurisdiction) = properties.get("jurisdiction").and_then(|v| v.as_str()) {
            property_fragments.push(format!(":jurisdiction \"{}\"", jurisdiction));
        }

        if let Some(reg_number) = properties
            .get("registration_number")
            .and_then(|v| v.as_str())
        {
            property_fragments.push(format!(":registration-number \"{}\"", reg_number));
        }

        let properties_str = if property_fragments.is_empty() {
            String::new()
        } else {
            format!(" :properties {{{}}}", property_fragments.join(" "))
        };

        Ok(format!(
            "(declare-entity :node-id \"{}\" :label {}{})",
            entity_id, entity_type, properties_str
        ))
    }

    /// Generate ownership edge DSL fragment
    async fn generate_ownership_edge_dsl(
        &self,
        payload: &serde_json::Value,
        _context: &DomainContext,
    ) -> DslEditResult<String> {
        let from_entity = payload
            .get("from_entity")
            .and_then(|v| v.as_str())
            .ok_or_else(|| {
                DslEditError::DomainValidationError("Missing from_entity in payload".to_string())
            })?;

        let to_entity = payload
            .get("to_entity")
            .and_then(|v| v.as_str())
            .ok_or_else(|| {
                DslEditError::DomainValidationError("Missing to_entity in payload".to_string())
            })?;

        let percentage = payload
            .get("percentage")
            .and_then(|v| v.as_f64())
            .unwrap_or(0.0);

        let share_class = payload
            .get("share_class")
            .and_then(|v| v.as_str())
            .unwrap_or("Ordinary");

        let default_evidence = vec![];
        let evidence = payload
            .get("evidence")
            .and_then(|v| v.as_array())
            .unwrap_or(&default_evidence);

        let evidence_str = if evidence.is_empty() {
            String::new()
        } else {
            let evidence_ids: Vec<String> = evidence
                .iter()
                .filter_map(|e| e.as_str())
                .map(|e| format!("\"{}\"", e))
                .collect();
            format!(" :evidenced-by [{}]", evidence_ids.join(" "))
        };

        Ok(format!(
            "(create-edge :from \"{}\" :to \"{}\" :type HAS_OWNERSHIP :properties {{:percent {} :share-class \"{}\"}}{})",
            from_entity, to_entity, percentage, share_class, evidence_str
        ))
    }

    /// Generate UBO calculation DSL fragment
    async fn generate_ubo_calculation_dsl(
        &self,
        payload: &serde_json::Value,
        _context: &DomainContext,
    ) -> DslEditResult<String> {
        let target_entity = payload
            .get("target_entity")
            .and_then(|v| v.as_str())
            .ok_or_else(|| {
                DslEditError::DomainValidationError("Missing target_entity in payload".to_string())
            })?;

        let threshold = payload
            .get("threshold")
            .and_then(|v| v.as_f64())
            .unwrap_or(25.0);

        let jurisdiction = payload
            .get("jurisdiction")
            .and_then(|v| v.as_str())
            .unwrap_or("US");

        let calculation_method = payload
            .get("calculation_method")
            .and_then(|v| v.as_str())
            .unwrap_or("direct_and_indirect");

        let timestamp = common::generate_timestamp();

        Ok(format!(
            "(calculate-ubo :target-entity \"{}\" :jurisdiction \"{}\" :ubo-threshold {} :calculation-method \"{}\" :calculated-at \"{}\")",
            target_entity, jurisdiction, threshold, calculation_method, timestamp
        ))
    }

    /// Generate threshold analysis DSL fragment
    async fn generate_threshold_analysis_dsl(
        &self,
        payload: &serde_json::Value,
        _context: &DomainContext,
    ) -> DslEditResult<String> {
        let entities = payload
            .get("entities")
            .and_then(|v| v.as_array())
            .ok_or_else(|| {
                DslEditError::DomainValidationError("Missing entities array in payload".to_string())
            })?;

        let threshold = payload
            .get("threshold")
            .and_then(|v| v.as_f64())
            .unwrap_or(25.0);

        let mut analysis_fragments = Vec::new();

        for entity in entities {
            if let Some(entity_id) = entity.get("entity_id").and_then(|v| v.as_str()) {
                let ownership = entity
                    .get("ownership")
                    .and_then(|v| v.as_f64())
                    .unwrap_or(0.0);
                let is_ubo = ownership >= threshold;

                analysis_fragments.push(format!(
                    "  (entity \"{}\" (ownership {}) (is-ubo {}))",
                    entity_id, ownership, is_ubo
                ));
            }
        }

        let timestamp = common::generate_timestamp();

        Ok(format!(
            "(analyze-thresholds :threshold {} :analyzed-at \"{}\" :results (\n{}\n))",
            threshold,
            timestamp,
            analysis_fragments.join("\n")
        ))
    }

    /// Generate compliance verification DSL fragment
    async fn generate_compliance_verification_dsl(
        &self,
        payload: &serde_json::Value,
        _context: &DomainContext,
    ) -> DslEditResult<String> {
        let compliance_framework = payload
            .get("framework")
            .and_then(|v| v.as_str())
            .unwrap_or("FATF");

        let jurisdiction = payload
            .get("jurisdiction")
            .and_then(|v| v.as_str())
            .unwrap_or("US");

        let verification_status = payload
            .get("status")
            .and_then(|v| v.as_str())
            .unwrap_or("COMPLIANT");

        let default_checks = vec![];
        let checks_performed = payload
            .get("checks")
            .and_then(|v| v.as_array())
            .unwrap_or(&default_checks);

        let checks_str = checks_performed
            .iter()
            .filter_map(|c| c.as_str())
            .map(|c| format!("\"{}\"", c))
            .collect::<Vec<_>>()
            .join(" ");

        let timestamp = common::generate_timestamp();

        Ok(format!(
            "(verify-compliance :framework \"{}\" :jurisdiction \"{}\" :status \"{}\" :checks [{}] :verified-at \"{}\")",
            compliance_framework, jurisdiction, verification_status, checks_str, timestamp
        ))
    }

    /// Generate UBO identification DSL fragment
    async fn generate_ubo_identification_dsl(
        &self,
        payload: &serde_json::Value,
        _context: &DomainContext,
    ) -> DslEditResult<String> {
        let identified_ubos = payload
            .get("ubos")
            .and_then(|v| v.as_array())
            .ok_or_else(|| {
                DslEditError::DomainValidationError("Missing ubos array in payload".to_string())
            })?;

        let mut ubo_fragments = Vec::new();

        for ubo in identified_ubos {
            let entity_id = ubo
                .get("entity_id")
                .and_then(|v| v.as_str())
                .unwrap_or("unknown");
            let ownership = ubo.get("ownership").and_then(|v| v.as_f64()).unwrap_or(0.0);
            let control_type = ubo
                .get("control_type")
                .and_then(|v| v.as_str())
                .unwrap_or("ownership");

            ubo_fragments.push(format!(
                "  (ubo :entity \"{}\" :ownership {} :control-type \"{}\")",
                entity_id, ownership, control_type
            ));
        }

        let timestamp = common::generate_timestamp();

        Ok(format!(
            "(identify-ubos :identified-at \"{}\" :results (\n{}\n))",
            timestamp,
            ubo_fragments.join("\n")
        ))
    }
}

#[async_trait]
impl DomainHandler for UboDomainHandler {
    fn domain_name(&self) -> &str {
        "ubo"
    }

    fn domain_version(&self) -> &str {
        "1.0.0"
    }

    fn domain_description(&self) -> &str {
        "Ultimate Beneficial Ownership discovery and compliance analysis"
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
            DslOperation::CreateEntity {
                entity_type,
                properties,
                ..
            } => {
                let entity_id = properties
                    .get("entity_id")
                    .and_then(|v| v.as_str())
                    .unwrap_or("generated-id");

                let legal_name = properties
                    .get("legal_name")
                    .and_then(|v| v.as_str())
                    .unwrap_or("Unknown Entity");

                let jurisdiction = properties
                    .get("jurisdiction")
                    .and_then(|v| v.as_str())
                    .unwrap_or("US");

                Ok(format!(
                    "(declare-entity :node-id \"{}\" :label {} :properties {{:legal-name \"{}\" :jurisdiction \"{}\"}})",
                    entity_id, entity_type, legal_name, jurisdiction
                ))
            }

            DslOperation::CreateRelationship {
                from_entity,
                to_entity,
                relationship_type,
                properties,
                ..
            } => {
                let percentage = properties
                    .get("percentage")
                    .and_then(|v| v.as_f64())
                    .unwrap_or(0.0);

                let share_class = properties
                    .get("share_class")
                    .and_then(|v| v.as_str())
                    .unwrap_or("Ordinary");

                Ok(format!(
                    "(create-edge :from \"{}\" :to \"{}\" :type {} :properties {{:percent {} :share-class \"{}\"}})",
                    from_entity, to_entity, relationship_type, percentage, share_class
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
                "ubo".to_string(),
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
                "calculate_ubo" => {
                    common::validate_required_context(context, &["target_entity"]).map_err(
                        |e| DslEditError::DomainValidationError(format!("UBO calculation: {}", e)),
                    )?;
                }
                "create_ownership_edge" => {
                    common::validate_required_context(context, &["from_entity", "to_entity"])
                        .map_err(|e| {
                            DslEditError::DomainValidationError(format!("Ownership edge: {}", e))
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
        common::validate_state_transition("ubo", &allowed_transitions, from, to)
            .map_err(|e| DslEditError::DomainValidationError(format!("State transition: {}", e)))
    }

    async fn apply_business_rules(
        &self,
        dsl_fragment: &str,
        _context: &DomainContext,
    ) -> DslEditResult<String> {
        let mut processed = dsl_fragment.to_string();

        // Rule: Add default threshold if missing from UBO calculation
        if dsl_fragment.contains("calculate-ubo") && !dsl_fragment.contains("ubo-threshold") {
            processed = processed.replace(")", " :ubo-threshold 25.0)");
        }

        // Rule: Add jurisdiction if missing from entity declaration
        if dsl_fragment.contains("declare-entity") && !dsl_fragment.contains("jurisdiction") {
            processed = processed.replace("}}", " :jurisdiction \"US\"}}");
        }

        // Rule: Add evidence requirement flag for high ownership percentages
        if dsl_fragment.contains(":percent") {
            if let Some(percent_match) = extract_percentage_from_dsl(&processed) {
                if percent_match >= 25.0 && !processed.contains("evidenced-by") {
                    processed = processed.replace(")", " :evidence-required true)");
                }
            }
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
        let mut context = DomainContext::ubo();

        // Extract target entity if present
        if let Some(target_match) = extract_value_from_dsl(dsl, "target-entity") {
            context = context.with_context("target_entity", json!(target_match));
        }

        // Extract threshold if present
        if let Some(threshold_match) = extract_value_from_dsl(dsl, "ubo-threshold") {
            if let Ok(threshold) = threshold_match.parse::<f64>() {
                context = context.with_context("threshold", json!(threshold));
            }
        }

        // Extract jurisdiction if present
        if let Some(jurisdiction_match) = extract_value_from_dsl(dsl, "jurisdiction") {
            context = context.with_context("jurisdiction", json!(jurisdiction_match));
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
            domain_name: "ubo".to_string(),
            status: crate::dsl::domain_registry::HealthStatus::Healthy,
            last_check: Utc::now(),
            metrics,
            errors: Vec::new(),
        }
    }
}

/// Create UBO-specific vocabulary extensions
fn create_ubo_vocabulary() -> DslVocabulary {
    use crate::dsl::domain_registry::{AttributeDefinition, TypeDefinition, VerbDefinition};
    use uuid::Uuid;

    DslVocabulary {
        verbs: vec![
            VerbDefinition {
                name: "declare-entity".to_string(),
                description: "Declare an entity in the UBO graph".to_string(),
                signature: "(declare-entity :node-id string :label symbol :properties map)".to_string(),
                category: "entity_management".to_string(),
                examples: vec![
                    "(declare-entity :node-id \"company-001\" :label Company :properties {:legal-name \"Acme Corp\"})".to_string(),
                ],
                validation_rules: vec!["require_node_id".to_string(), "require_label".to_string()],
            },
            VerbDefinition {
                name: "create-edge".to_string(),
                description: "Create ownership relationship between entities".to_string(),
                signature: "(create-edge :from string :to string :type symbol :properties map)".to_string(),
                category: "relationship_management".to_string(),
                examples: vec![
                    "(create-edge :from \"person-001\" :to \"company-001\" :type HAS_OWNERSHIP :properties {:percent 51.0})".to_string(),
                ],
                validation_rules: vec!["require_from_to".to_string(), "validate_percentage".to_string()],
            },
            VerbDefinition {
                name: "calculate-ubo".to_string(),
                description: "Calculate ultimate beneficial ownership".to_string(),
                signature: "(calculate-ubo :target-entity string :jurisdiction string :ubo-threshold number)".to_string(),
                category: "ubo_calculation".to_string(),
                examples: vec![
                    "(calculate-ubo :target-entity \"company-001\" :jurisdiction \"US\" :ubo-threshold 25.0)".to_string(),
                ],
                validation_rules: vec!["require_target_entity".to_string(), "validate_threshold".to_string()],
            },
        ],
        attributes: vec![
            AttributeDefinition {
                attribute_id: Uuid::parse_str("789abcde-f012-3456-7890-abcdef123401").unwrap(),
                name: "ubo.ownership_percentage".to_string(),
                data_type: "decimal".to_string(),
                domain: "ubo".to_string(),
                validation_rules: vec!["range_0_100".to_string()],
            },
            AttributeDefinition {
                attribute_id: Uuid::parse_str("789abcde-f012-3456-7890-abcdef123402").unwrap(),
                name: "ubo.threshold".to_string(),
                data_type: "decimal".to_string(),
                domain: "ubo".to_string(),
                validation_rules: vec!["range_0_100".to_string()],
            },
        ],
        types: vec![
            TypeDefinition {
                type_name: "ownership_percentage".to_string(),
                base_type: "decimal".to_string(),
                constraints: vec!["min:0.0".to_string(), "max:100.0".to_string()],
                validation_pattern: Some("^\\d{1,3}(\\.\\d{1,2})?$".to_string()),
            },
        ],
        grammar_extensions: vec![
            "ubo_entity ::= \"(\" \"declare-entity\" entity_params+ \")\"".to_string(),
            "ubo_edge ::= \"(\" \"create-edge\" edge_params+ \")\"".to_string(),
            "entity_params ::= \":\" identifier value".to_string(),
        ],
    }
}

/// Create state transitions for UBO domain
fn create_state_transitions() -> Vec<StateTransition> {
    vec![
        StateTransition {
            from_state: "INITIAL".to_string(),
            to_state: "ENTITIES_DECLARED".to_string(),
            transition_name: "declare_entities".to_string(),
            required_conditions: vec!["has_target_entity".to_string()],
            side_effects: vec!["create_entity_graph".to_string()],
        },
        StateTransition {
            from_state: "ENTITIES_DECLARED".to_string(),
            to_state: "RELATIONSHIPS_MAPPED".to_string(),
            transition_name: "map_relationships".to_string(),
            required_conditions: vec!["has_entities".to_string()],
            side_effects: vec!["create_ownership_edges".to_string()],
        },
        StateTransition {
            from_state: "RELATIONSHIPS_MAPPED".to_string(),
            to_state: "OWNERSHIP_CALCULATED".to_string(),
            transition_name: "calculate_ownership".to_string(),
            required_conditions: vec!["has_relationships".to_string()],
            side_effects: vec!["compute_ownership_percentages".to_string()],
        },
    ]
}

/// Create validation rules for UBO domain
fn create_validation_rules() -> Vec<ValidationRule> {
    use crate::dsl::domain_registry::{ValidationRule, ValidationRuleType, ValidationSeverity};

    vec![
        ValidationRule {
            rule_id: "require_target_entity".to_string(),
            rule_name: "Target Entity Required".to_string(),
            rule_type: ValidationRuleType::BusinessRuleValidation,
            description: "UBO calculations must specify a target entity".to_string(),
            parameters: HashMap::new(),
            severity: ValidationSeverity::Error,
        },
        ValidationRule {
            rule_id: "validate_percentage".to_string(),
            rule_name: "Ownership Percentage Validation".to_string(),
            rule_type: ValidationRuleType::DataIntegrityValidation,
            description: "Ownership percentages must be between 0 and 100".to_string(),
            parameters: HashMap::new(),
            severity: ValidationSeverity::Error,
        },
        ValidationRule {
            rule_id: "validate_threshold".to_string(),
            rule_name: "UBO Threshold Validation".to_string(),
            rule_type: ValidationRuleType::ComplianceValidation,
            description: "UBO thresholds must be valid regulatory values".to_string(),
            parameters: HashMap::new(),
            severity: ValidationSeverity::Warning,
        },
    ]
}

/// Extract value from DSL using simple pattern matching
fn extract_value_from_dsl(dsl: &str, key: &str) -> Option<String> {
    let pattern = format!(":{} ", key);
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
        else if let Some(end) =
            remaining.find(|c: char| c.is_whitespace() || c == ')' || c == '}')
        {
            return Some(remaining[..end].to_string());
        }
    }
    None
}

/// Extract percentage value from DSL
fn extract_percentage_from_dsl(dsl: &str) -> Option<f64> {
    if let Some(percent_str) = extract_value_from_dsl(dsl, "percent") {
        percent_str.parse().ok()
    } else {
        None
    }
}

impl Default for UboDomainHandler {
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
    async fn test_ubo_domain_creation() {
        let handler = UboDomainHandler::new();
        assert_eq!(handler.domain_name(), "ubo");
        assert_eq!(handler.domain_version(), "1.0.0");
        assert!(!handler.supported_operations().is_empty());
    }

    #[tokio::test]
    async fn test_entity_declaration_dsl() {
        let handler = UboDomainHandler::new();
        let context = DomainContext::ubo();

        let payload = json!({
            "entity_id": "company-001",
            "entity_type": "Company",
            "properties": {
                "legal_name": "Acme Corporation",
                "jurisdiction": "DE",
                "registration_number": "HRB 123456"
            }
        });

        let result = handler
            .generate_entity_declaration_dsl(&payload, &context)
            .await;
        assert!(result.is_ok());

        let dsl = result.unwrap();
        assert!(dsl.contains("declare-entity"));
        assert!(dsl.contains("company-001"));
        assert!(dsl.contains("Acme Corporation"));
        assert!(dsl.contains("DE"));
    }

    #[tokio::test]
    async fn test_ownership_edge_dsl() {
        let handler = UboDomainHandler::new();
        let context = DomainContext::ubo();

        let payload = json!({
            "from_entity": "person-001",
            "to_entity": "company-001",
            "percentage": 51.0,
            "share_class": "Class A Ordinary",
            "evidence": ["doc-001", "doc-002"]
        });

        let result = handler
            .generate_ownership_edge_dsl(&payload, &context)
            .await;
        assert!(result.is_ok());

        let dsl = result.unwrap();
        assert!(dsl.contains("create-edge"));
        assert!(dsl.contains("person-001"));
        assert!(dsl.contains("company-001"));
        assert!(dsl.contains("51"));
        assert!(dsl.contains("Class A Ordinary"));
        assert!(dsl.contains("evidenced-by"));
    }

    #[tokio::test]
    async fn test_ubo_calculation_dsl() {
        let handler = UboDomainHandler::new();
        let context = DomainContext::ubo();

        let payload = json!({
            "target_entity": "company-001",
            "threshold": 25.0,
            "jurisdiction": "US",
            "calculation_method": "direct_and_indirect"
        });

        let result = handler
            .generate_ubo_calculation_dsl(&payload, &context)
            .await;
        assert!(result.is_ok());

        let dsl = result.unwrap();
        assert!(dsl.contains("calculate-ubo"));
        assert!(dsl.contains("company-001"));
        assert!(dsl.contains("25"));
        assert!(dsl.contains("US"));
        assert!(dsl.contains("direct_and_indirect"));
    }

    #[tokio::test]
    async fn test_create_relationship_transformation() {
        let handler = UboDomainHandler::new();
        let context = DomainContext::ubo();

        let mut properties = HashMap::new();
        properties.insert("percentage".to_string(), json!(75.5));
        properties.insert("share_class".to_string(), json!("Preferred"));

        let operation = DslOperation::CreateRelationship {
            from_entity: "investor-001".to_string(),
            to_entity: "target-company".to_string(),
            relationship_type: "HAS_OWNERSHIP".to_string(),
            properties,
            metadata: crate::dsl::domain_context::OperationMetadata::default(),
        };

        let result = handler
            .transform_operation_to_dsl(&operation, &context)
            .await;
        assert!(result.is_ok());

        let dsl = result.unwrap();
        assert!(dsl.contains("create-edge"));
        assert!(dsl.contains("investor-001"));
        assert!(dsl.contains("target-company"));
        assert!(dsl.contains("HAS_OWNERSHIP"));
        assert!(dsl.contains("75.5"));
        assert!(dsl.contains("Preferred"));
    }

    #[tokio::test]
    async fn test_state_transition_validation() {
        let handler = UboDomainHandler::new();

        let result = handler.validate_state_transition("INITIAL", "ENTITIES_DECLARED");
        assert!(result.is_ok());

        let result = handler.validate_state_transition("INITIAL", "UBO_IDENTIFIED");
        assert!(result.is_err());
    }

    #[tokio::test]
    async fn test_business_rules_application() {
        let handler = UboDomainHandler::new();
        let context = DomainContext::ubo();

        // Test default threshold addition
        let dsl_fragment = "(calculate-ubo :target-entity \"company-001\")";
        let result = handler.apply_business_rules(dsl_fragment, &context).await;
        assert!(result.is_ok());
        let processed = result.unwrap();
        assert!(processed.contains("ubo-threshold 25.0"));

        // Test evidence requirement for high ownership
        let ownership_fragment =
            "(create-edge :from \"person-001\" :to \"company-001\" :properties {:percent 51.0})";
        let result = handler
            .apply_business_rules(ownership_fragment, &context)
            .await;
        assert!(result.is_ok());
        let processed = result.unwrap();
        assert!(processed.contains("evidence-required true"));
    }

    #[tokio::test]
    async fn test_context_extraction() {
        let handler = UboDomainHandler::new();
        let dsl = r#"
            (calculate-ubo :target-entity "company-zenith-001" :jurisdiction "KY" :ubo-threshold 25.0)
            (declare-entity :node-id "person-001" :label Person)
        "#;

        let result = handler.extract_context_from_dsl(dsl).await;
        assert!(result.is_ok());

        let context = result.unwrap();
        assert_eq!(context.domain_name, "ubo");

        let target_entity: Option<String> = context.get_context("target_entity");
        assert_eq!(target_entity, Some("company-zenith-001".to_string()));

        let threshold: Option<f64> = context.get_context("threshold");
        assert_eq!(threshold, Some(25.0));

        let jurisdiction: Option<String> = context.get_context("jurisdiction");
        assert_eq!(jurisdiction, Some("KY".to_string()));
    }

    #[test]
    fn test_percentage_extraction() {
        let dsl = "(create-edge :properties {:percent 45.5})";
        let result = extract_percentage_from_dsl(dsl);
        assert_eq!(result, Some(45.5));

        let dsl_no_percent = "(declare-entity :node-id \"test\")";
        let result = extract_percentage_from_dsl(dsl_no_percent);
        assert_eq!(result, None);
    }
}
