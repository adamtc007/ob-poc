//! Onboarding Domain Handler
//!
//! This module implements the onboarding domain handler for the centralized DSL
//! editing system. It handles client onboarding workflows including case creation,
//! product configuration, KYC initiation, and workflow progression.
//!
//! ## Supported Operations:
//! - Case creation and management
//! - Product and service addition
//! - CBU association
//! - Service discovery and planning
//! - Resource provisioning
//! - State transitions through onboarding lifecycle
//!
//! ## State Machine:
//! CREATE → PRODUCTS_ADDED → KYC_STARTED → SERVICES_DISCOVERED →
//! RESOURCES_PLANNED → ATTRIBUTES_BOUND → WORKFLOW_ACTIVE → COMPLETE

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

/// Onboarding domain handler implementation
pub struct OnboardingDomainHandler {
    vocabulary: DslVocabulary,
    state_transitions: Vec<StateTransition>,
    validation_rules: Vec<ValidationRule>,
    supported_operations: Vec<String>,
}

impl OnboardingDomainHandler {
    /// Create a new onboarding domain handler
    pub fn new() -> Self {
        let mut handler = Self {
            vocabulary: create_onboarding_vocabulary(),
            state_transitions: create_state_transitions(),
            validation_rules: create_validation_rules(),
            supported_operations: vec![
                "Create Company entity".to_string(),
                "Create proper person entity".to_string(),
                "Add products".to_string(),
                "Add services".to_string(),
                "Transition from CREATE to PRODUCTS_ADDED".to_string(),
                "Transition from PRODUCTS_ADDED to KYC_STARTED".to_string(),
                "Domain operation: associate_cbu".to_string(),
                "Domain operation: discover_services".to_string(),
                "Domain operation: discover_resources".to_string(),
                "Domain operation: complete_onboarding".to_string(),
                "Domain operation: archive_onboarding".to_string(),
            ],
        };

        // Add domain-specific operations
        handler.add_supported_operation("associate_cbu");
        handler.add_supported_operation("discover_services");
        handler.add_supported_operation("discover_resources");
        handler.add_supported_operation("complete_onboarding");
        handler.add_supported_operation("archive_onboarding");

        handler
    }

    fn add_supported_operation(&mut self, operation: &str) {
        if !self.supported_operations.contains(&operation.to_string()) {
            self.supported_operations.push(operation.to_string());
        }
    }

    /// Get allowed state transitions for onboarding
    fn get_allowed_transitions() -> Vec<(String, String)> {
        vec![
            ("CREATE".to_string(), "PRODUCTS_ADDED".to_string()),
            ("PRODUCTS_ADDED".to_string(), "KYC_STARTED".to_string()),
            ("KYC_STARTED".to_string(), "SERVICES_DISCOVERED".to_string()),
            (
                "SERVICES_DISCOVERED".to_string(),
                "RESOURCES_PLANNED".to_string(),
            ),
            (
                "RESOURCES_PLANNED".to_string(),
                "ATTRIBUTES_BOUND".to_string(),
            ),
            (
                "ATTRIBUTES_BOUND".to_string(),
                "WORKFLOW_ACTIVE".to_string(),
            ),
            ("WORKFLOW_ACTIVE".to_string(), "COMPLETE".to_string()),
            // Allow archival from any state
            ("CREATE".to_string(), "ARCHIVED".to_string()),
            ("PRODUCTS_ADDED".to_string(), "ARCHIVED".to_string()),
            ("KYC_STARTED".to_string(), "ARCHIVED".to_string()),
            ("SERVICES_DISCOVERED".to_string(), "ARCHIVED".to_string()),
            ("RESOURCES_PLANNED".to_string(), "ARCHIVED".to_string()),
            ("ATTRIBUTES_BOUND".to_string(), "ARCHIVED".to_string()),
            ("WORKFLOW_ACTIVE".to_string(), "ARCHIVED".to_string()),
            ("COMPLETE".to_string(), "ARCHIVED".to_string()),
        ]
    }

    /// Transform domain-specific operations to DSL
    async fn transform_domain_specific(
        &self,
        operation_type: &str,
        payload: &serde_json::Value,
        context: &DomainContext,
    ) -> DslEditResult<String> {
        match operation_type {
            "associate_cbu" => self.generate_cbu_association_dsl(payload, context).await,
            "discover_services" => self.generate_service_discovery_dsl(payload, context).await,
            "discover_resources" => self.generate_resource_discovery_dsl(payload, context).await,
            "complete_onboarding" => self.generate_completion_dsl(payload, context).await,
            "archive_onboarding" => self.generate_archival_dsl(payload, context).await,
            _ => Err(DslEditError::UnsupportedOperation(
                operation_type.to_string(),
                "onboarding".to_string(),
            )),
        }
    }

    /// Generate CBU association DSL fragment
    async fn generate_cbu_association_dsl(
        &self,
        payload: &serde_json::Value,
        _context: &DomainContext,
    ) -> DslEditResult<String> {
        let cbu_id = payload
            .get("cbu_id")
            .and_then(|v| v.as_str())
            .ok_or_else(|| {
                DslEditError::DomainValidationError("Missing cbu_id in payload".to_string())
            })?;

        let association_type = payload
            .get("association_type")
            .and_then(|v| v.as_str())
            .unwrap_or("primary");

        let default_details = json!("Associated via onboarding");
        let details = payload.get("details").unwrap_or(&default_details);

        let timestamp = common::generate_timestamp();

        Ok(format!(
            "(cbu.associate (cbu.id \"{}\") (association.type \"{}\") (cbu.details {}) (associated.at \"{}\"))",
            cbu_id, association_type, details, timestamp
        ))
    }

    /// Generate service discovery DSL fragment
    async fn generate_service_discovery_dsl(
        &self,
        payload: &serde_json::Value,
        _context: &DomainContext,
    ) -> DslEditResult<String> {
        let services = payload
            .get("services")
            .and_then(|v| v.as_array())
            .ok_or_else(|| {
                DslEditError::DomainValidationError("Missing services array in payload".to_string())
            })?;

        let mut service_fragments = Vec::new();

        for service in services {
            let service_name = service
                .get("name")
                .and_then(|v| v.as_str())
                .ok_or_else(|| {
                    DslEditError::DomainValidationError("Missing service name".to_string())
                })?;

            let sla = service
                .get("sla")
                .and_then(|v| v.as_str())
                .unwrap_or("standard");

            service_fragments.push(format!(
                "  (service \"{}\" (sla \"{}\"))",
                service_name, sla
            ));
        }

        Ok(format!("(services.plan\n{})", service_fragments.join("\n")))
    }

    /// Generate resource discovery DSL fragment
    async fn generate_resource_discovery_dsl(
        &self,
        payload: &serde_json::Value,
        _context: &DomainContext,
    ) -> DslEditResult<String> {
        let resources = payload
            .get("resources")
            .and_then(|v| v.as_array())
            .ok_or_else(|| {
                DslEditError::DomainValidationError(
                    "Missing resources array in payload".to_string(),
                )
            })?;

        let mut resource_fragments = Vec::new();

        for resource in resources {
            let resource_type = resource
                .get("type")
                .and_then(|v| v.as_str())
                .ok_or_else(|| {
                    DslEditError::DomainValidationError("Missing resource type".to_string())
                })?;

            let owner = resource
                .get("owner")
                .and_then(|v| v.as_str())
                .unwrap_or("system");

            resource_fragments.push(format!(
                "  (resource \"{}\" (owner \"{}\"))",
                resource_type, owner
            ));
        }

        Ok(format!(
            "(resources.plan\n{})",
            resource_fragments.join("\n")
        ))
    }

    /// Generate completion DSL fragment
    async fn generate_completion_dsl(
        &self,
        payload: &serde_json::Value,
        _context: &DomainContext,
    ) -> DslEditResult<String> {
        let completion_notes = payload
            .get("notes")
            .and_then(|v| v.as_str())
            .unwrap_or("Onboarding completed successfully");

        let timestamp = common::generate_timestamp();

        Ok(format!(
            "(onboarding.complete (completion.status \"SUCCESS\") (completion.notes \"{}\") (completed.at \"{}\"))",
            completion_notes, timestamp
        ))
    }

    /// Generate archival DSL fragment
    async fn generate_archival_dsl(
        &self,
        payload: &serde_json::Value,
        _context: &DomainContext,
    ) -> DslEditResult<String> {
        let reason = payload
            .get("reason")
            .and_then(|v| v.as_str())
            .unwrap_or("Archived by user");

        let timestamp = common::generate_timestamp();

        Ok(format!(
            "(onboarding.archive (archival.reason \"{}\") (archived.at \"{}\"))",
            reason, timestamp
        ))
    }
}

#[async_trait]
impl DomainHandler for OnboardingDomainHandler {
    fn domain_name(&self) -> &str {
        "onboarding"
    }

    fn domain_version(&self) -> &str {
        "1.0.0"
    }

    fn domain_description(&self) -> &str {
        "Client onboarding workflows and case management"
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

                let mut attrs = HashMap::new();
                for (key, value) in properties {
                    if key != "entity_id" {
                        attrs.insert(key.clone(), value.clone());
                    }
                }

                let attrs_str = attrs
                    .iter()
                    .map(|(k, v)| format!(":{} \"{}\"", k, v))
                    .collect::<Vec<_>>()
                    .join(" ");

                Ok(format!(
                    "(case.create (entity.id \"{}\") (entity.type \"{}\") {})",
                    entity_id, entity_type, attrs_str
                ))
            }

            DslOperation::AddProducts { products, .. } => {
                let products_str = products
                    .iter()
                    .map(|p| format!("\"{}\"", p))
                    .collect::<Vec<_>>()
                    .join(" ");

                Ok(format!("(products.add {})", products_str))
            }

            DslOperation::TransitionState {
                from_state,
                to_state,
                transition_data,
                ..
            } => {
                // Validate transition is allowed
                self.validate_state_transition(from_state, to_state)?;

                let timestamp = common::generate_timestamp();
                let reason = transition_data
                    .get("reason")
                    .and_then(|v| v.as_str())
                    .unwrap_or("State transition");

                Ok(format!(
                    "(state.transition (from \"{}\") (to \"{}\") (reason \"{}\") (timestamp \"{}\"))",
                    from_state, to_state, reason, timestamp
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
                "onboarding".to_string(),
            )),
        }
    }

    async fn validate_operation(
        &self,
        operation: &DslOperation,
        context: &DomainContext,
    ) -> DslEditResult<()> {
        // Validate required context for certain operations
        if let DslOperation::DomainSpecific { operation_type, .. } = operation { match operation_type.as_str() {
            "associate_cbu" => {
                common::validate_required_context(context, &["cbu_id"]).map_err(|e| {
                    DslEditError::DomainValidationError(format!("CBU association: {}", e))
                })?;
            }
            "complete_onboarding" | "archive_onboarding" => {
                // These operations can be performed in any context
            }
            _ => {}
        } }

        Ok(())
    }

    fn get_valid_transitions(&self) -> &[StateTransition] {
        &self.state_transitions
    }

    fn validate_state_transition(&self, from: &str, to: &str) -> DslEditResult<()> {
        let allowed_transitions = Self::get_allowed_transitions();
        common::validate_state_transition("onboarding", &allowed_transitions, from, to)
            .map_err(|e| DslEditError::DomainValidationError(format!("State transition: {}", e)))
    }

    async fn apply_business_rules(
        &self,
        dsl_fragment: &str,
        _context: &DomainContext,
    ) -> DslEditResult<String> {
        // Apply onboarding-specific business rules
        let mut processed = dsl_fragment.to_string();

        // Rule: Add timestamp to case creation if missing
        if dsl_fragment.contains("case.create") && !dsl_fragment.contains("created.at") {
            let timestamp = common::generate_timestamp();
            processed = processed.replace(")", &format!(" (created.at \"{}\"))", timestamp));
        }

        // Rule: Add default nature and purpose if missing
        if dsl_fragment.contains("case.create")
            && !dsl_fragment.contains("nature-purpose")
            && !dsl_fragment.contains("nature.purpose")
        {
            processed = processed.replace(")", " (nature.purpose \"Client onboarding\"))");
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
        let mut context = DomainContext::onboarding();

        // Extract CBU ID if present
        if let Some(cbu_match) = extract_value_from_dsl(dsl, "cbu.id") {
            context = context.with_context("cbu_id", json!(cbu_match));
        }

        // Extract entity ID if present
        if let Some(entity_match) = extract_value_from_dsl(dsl, "entity.id") {
            context = context.with_context("entity_id", json!(entity_match));
        }

        // Extract current state from state transitions
        if let Some(state_match) = extract_value_from_dsl(dsl, "to") {
            context.state_requirements.current_state = Some(state_match);
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
            domain_name: "onboarding".to_string(),
            status: crate::dsl::domain_registry::HealthStatus::Healthy,
            last_check: Utc::now(),
            metrics,
            errors: Vec::new(),
        }
    }
}

/// Create onboarding-specific vocabulary extensions
fn create_onboarding_vocabulary() -> DslVocabulary {
    use crate::dsl::domain_registry::{AttributeDefinition, TypeDefinition, VerbDefinition};
    use uuid::Uuid;

    DslVocabulary {
        verbs: vec![
            VerbDefinition {
                name: "case.create".to_string(),
                description: "Create a new onboarding case".to_string(),
                signature: "(case.create (entity.id string) (entity.type string) ...)".to_string(),
                category: "case_management".to_string(),
                examples: vec![
                    "(case.create (entity.id \"ENT-001\") (entity.type \"Company\"))".to_string(),
                ],
                validation_rules: vec![
                    "require_entity_id".to_string(),
                    "require_entity_type".to_string(),
                ],
            },
            VerbDefinition {
                name: "products.add".to_string(),
                description: "Add products to an onboarding case".to_string(),
                signature: "(products.add product...)".to_string(),
                category: "product_management".to_string(),
                examples: vec!["(products.add \"CUSTODY\" \"FUND_ACCOUNTING\")".to_string()],
                validation_rules: vec!["require_valid_products".to_string()],
            },
            VerbDefinition {
                name: "cbu.associate".to_string(),
                description: "Associate a CBU with an entity".to_string(),
                signature: "(cbu.associate (cbu.id string) (association.type string) ...)"
                    .to_string(),
                category: "cbu_management".to_string(),
                examples: vec![
                    "(cbu.associate (cbu.id \"CBU-1234\") (association.type \"primary\"))"
                        .to_string(),
                ],
                validation_rules: vec!["require_cbu_id".to_string()],
            },
        ],
        attributes: vec![
            AttributeDefinition {
                attribute_id: Uuid::parse_str("123e4567-e89b-12d3-a456-426614174001").unwrap(),
                name: "onboard.cbu_id".to_string(),
                data_type: "string".to_string(),
                domain: "onboarding".to_string(),
                validation_rules: vec!["format_cbu_id".to_string()],
            },
            AttributeDefinition {
                attribute_id: Uuid::parse_str("123e4567-e89b-12d3-a456-426614174002").unwrap(),
                name: "onboard.nature_purpose".to_string(),
                data_type: "string".to_string(),
                domain: "onboarding".to_string(),
                validation_rules: vec!["min_length_10".to_string()],
            },
        ],
        types: vec![TypeDefinition {
            type_name: "cbu_id".to_string(),
            base_type: "string".to_string(),
            constraints: vec!["pattern:CBU-\\d{4}-\\d{3}".to_string()],
            validation_pattern: Some("^CBU-\\d{4}-\\d{3}$".to_string()),
        }],
        grammar_extensions: vec![
            "onboarding_case ::= \"(\" \"case.create\" entity_definition+ \")\"".to_string(),
            "entity_definition ::= \"(\" attribute_name value \")\"".to_string(),
        ],
    }
}

/// Create state transitions for onboarding domain
fn create_state_transitions() -> Vec<StateTransition> {
    vec![
        StateTransition {
            from_state: "CREATE".to_string(),
            to_state: "PRODUCTS_ADDED".to_string(),
            transition_name: "add_products".to_string(),
            required_conditions: vec!["has_entity".to_string()],
            side_effects: vec!["update_timestamp".to_string()],
        },
        StateTransition {
            from_state: "PRODUCTS_ADDED".to_string(),
            to_state: "KYC_STARTED".to_string(),
            transition_name: "start_kyc".to_string(),
            required_conditions: vec!["has_products".to_string()],
            side_effects: vec!["create_kyc_case".to_string()],
        },
        // Add more transitions as needed
    ]
}

/// Create validation rules for onboarding domain
fn create_validation_rules() -> Vec<ValidationRule> {
    use crate::dsl::domain_registry::{ValidationRule, ValidationRuleType, ValidationSeverity};

    vec![
        ValidationRule {
            rule_id: "require_entity_id".to_string(),
            rule_name: "Entity ID Required".to_string(),
            rule_type: ValidationRuleType::BusinessRuleValidation,
            description: "All onboarding cases must have an entity ID".to_string(),
            parameters: HashMap::new(),
            severity: ValidationSeverity::Error,
        },
        ValidationRule {
            rule_id: "require_cbu_id".to_string(),
            rule_name: "CBU ID Required".to_string(),
            rule_type: ValidationRuleType::BusinessRuleValidation,
            description: "CBU association requires valid CBU ID".to_string(),
            parameters: HashMap::new(),
            severity: ValidationSeverity::Error,
        },
    ]
}

/// Extract value from DSL using simple pattern matching
fn extract_value_from_dsl(dsl: &str, key: &str) -> Option<String> {
    let pattern = format!("({} \"", key);
    if let Some(start) = dsl.find(&pattern) {
        let start_pos = start + pattern.len();
        if let Some(end) = dsl[start_pos..].find('"') {
            return Some(dsl[start_pos..start_pos + end].to_string());
        }
    }
    None
}

impl Default for OnboardingDomainHandler {
    fn default() -> Self {
        Self::new()
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::dsl::operations::OperationBuilder;
    use serde_json::json;

    #[tokio::test]
    async fn test_onboarding_domain_creation() {
        let handler = OnboardingDomainHandler::new();
        assert_eq!(handler.domain_name(), "onboarding");
        assert_eq!(handler.domain_version(), "1.0.0");
        assert!(!handler.supported_operations().is_empty());
    }

    #[tokio::test]
    async fn test_create_entity_transformation() {
        let handler = OnboardingDomainHandler::new();
        let context = DomainContext::onboarding();

        let operation = OperationBuilder::new("test_user").create_entity(
            "Company",
            [
                ("entity_id".to_string(), json!("ENT-001")),
                ("name".to_string(), json!("Test Corp")),
            ]
            .iter()
            .cloned()
            .collect(),
        );

        let result = handler
            .transform_operation_to_dsl(&operation, &context)
            .await;
        assert!(result.is_ok());

        let dsl = result.unwrap();
        assert!(dsl.contains("case.create"));
        assert!(dsl.contains("ENT-001"));
        assert!(dsl.contains("Company"));
    }

    #[tokio::test]
    async fn test_add_products_transformation() {
        let handler = OnboardingDomainHandler::new();
        let context = DomainContext::onboarding();

        let operation = OperationBuilder::new("test_user").add_products(
            "CBU-1234",
            vec!["CUSTODY".to_string(), "FUND_ACCOUNTING".to_string()],
        );

        let result = handler
            .transform_operation_to_dsl(&operation, &context)
            .await;
        assert!(result.is_ok());

        let dsl = result.unwrap();
        assert!(dsl.contains("products.add"));
        assert!(dsl.contains("CUSTODY"));
        assert!(dsl.contains("FUND_ACCOUNTING"));
    }

    #[tokio::test]
    async fn test_state_transition_validation() {
        let handler = OnboardingDomainHandler::new();

        let result = handler.validate_state_transition("CREATE", "PRODUCTS_ADDED");
        assert!(result.is_ok());

        let result = handler.validate_state_transition("CREATE", "COMPLETE");
        assert!(result.is_err());
    }

    #[tokio::test]
    async fn test_business_rules_application() {
        let handler = OnboardingDomainHandler::new();
        let context = DomainContext::onboarding();

        let dsl_fragment = "(case.create (entity.id \"ENT-001\"))";
        let result = handler.apply_business_rules(dsl_fragment, &context).await;

        assert!(result.is_ok());
        let processed = result.unwrap();
        assert!(processed.contains("created.at"));
        assert!(processed.contains("nature.purpose"));
    }

    #[tokio::test]
    async fn test_domain_specific_operations() {
        let handler = OnboardingDomainHandler::new();
        let context = DomainContext::onboarding().with_context("cbu_id", json!("CBU-1234"));

        // Test CBU association
        let payload = json!({
            "cbu_id": "CBU-1234",
            "association_type": "primary"
        });

        let result = handler
            .generate_cbu_association_dsl(&payload, &context)
            .await;
        assert!(result.is_ok());

        let dsl = result.unwrap();
        assert!(dsl.contains("cbu.associate"));
        assert!(dsl.contains("CBU-1234"));
        assert!(dsl.contains("primary"));
    }

    #[tokio::test]
    async fn test_context_extraction() {
        let handler = OnboardingDomainHandler::new();
        let dsl = r#"
            (case.create (entity.id "ENT-001"))
            (cbu.associate (cbu.id "CBU-1234"))
            (state.transition (from "CREATE") (to "PRODUCTS_ADDED"))
        "#;

        let result = handler.extract_context_from_dsl(dsl).await;
        assert!(result.is_ok());

        let context = result.unwrap();
        assert_eq!(context.domain_name, "onboarding");

        let entity_id: Option<String> = context.get_context("entity_id");
        assert_eq!(entity_id, Some("ENT-001".to_string()));

        let cbu_id: Option<String> = context.get_context("cbu_id");
        assert_eq!(cbu_id, Some("CBU-1234".to_string()));

        assert_eq!(
            context.state_requirements.current_state,
            Some("PRODUCTS_ADDED".to_string())
        );
    }
}
