//! Domain Registry and Handler Trait System
//!
//! This module provides the unified domain handler interface that enables
//! domain-specific business logic while maintaining centralized DSL operations.
//! All domains implement the DomainHandler trait and register with the central
//! registry for operation routing.
//!
//! ## Key Components:
//! - `DomainHandler`: Core trait that all domain implementations must follow
//! - `DomainRegistry`: Central registry for domain lookup and management
//! - `DslVocabulary`: Shared vocabulary system (ONE DSL vocab principle)
//! - Domain validation and transformation interfaces
//!
//! ## Architecture Benefits:
//! - Unified interface across all domains
//! - Shared grammar and vocabulary validation
//! - Centralized domain discovery and routing
//! - Domain-specific business rule enforcement

use crate::dsl::{
    domain_context::DomainContext, operations::DslOperation, DslEditError, DslEditResult,
};
use async_trait::async_trait;

use serde::{Deserialize, Serialize};
use std::collections::HashMap;
use uuid::Uuid;

/// Core trait that all domain implementations must implement
/// Provides unified interface for domain-specific DSL operations
#[async_trait]
pub trait DomainHandler: Send + Sync {
    /// Get domain identity information
    fn domain_name(&self) -> &str;
    fn domain_version(&self) -> &str;
    fn domain_description(&self) -> &str;

    /// Get domain vocabulary extensions (builds on shared ONE DSL vocab)
    fn get_vocabulary(&self) -> &DslVocabulary;

    /// Transform a generic operation into domain-specific DSL fragment
    /// This is where domain-specific business logic is applied
    async fn transform_operation_to_dsl(
        &self,
        operation: &DslOperation,
        context: &DomainContext,
    ) -> DslEditResult<String>;

    /// Validate that an operation is allowed in this domain
    async fn validate_operation(
        &self,
        operation: &DslOperation,
        context: &DomainContext,
    ) -> DslEditResult<()>;

    /// Get valid state transitions for this domain
    fn get_valid_transitions(&self) -> &[StateTransition];

    /// Validate a specific state transition
    fn validate_state_transition(&self, from: &str, to: &str) -> DslEditResult<()>;

    /// Apply domain-specific business rules to DSL fragment
    async fn apply_business_rules(
        &self,
        dsl_fragment: &str,
        context: &DomainContext,
    ) -> DslEditResult<String>;

    /// Get supported operation types for this domain
    fn supported_operations(&self) -> &[String];

    /// Check if domain supports a specific operation type
    fn supports_operation(&self, operation_type: &str) -> bool {
        self.supported_operations()
            .contains(&operation_type.to_string())
    }

    /// Get domain-specific validation rules
    fn get_validation_rules(&self) -> &[ValidationRule];

    /// Extract domain context from existing DSL
    async fn extract_context_from_dsl(&self, dsl: &str) -> DslEditResult<DomainContext>;

    /// Get domain health status and metrics
    async fn health_check(&self) -> DomainHealthStatus;
}

/// Central registry for domain handlers
/// Manages domain discovery, routing, and lifecycle
pub struct DomainRegistry {
    /// Registered domain handlers
    domains: HashMap<String, Box<dyn DomainHandler>>,

    /// Shared DSL vocabulary (ONE DSL vocab principle)
    shared_vocabulary: DslVocabulary,

    /// Global validation rules that apply to all domains
    global_rules: Vec<ValidationRule>,

    /// Domain routing rules for operation dispatching
    routing_rules: Vec<RoutingRule>,
}

impl DomainRegistry {
    /// Create a new domain registry
    pub fn new() -> Self {
        Self {
            domains: HashMap::new(),
            shared_vocabulary: DslVocabulary::default(),
            global_rules: Vec::new(),
            routing_rules: Vec::new(),
        }
    }

    /// Register a domain handler
    pub fn register_domain(&mut self, handler: Box<dyn DomainHandler>) -> DslEditResult<()> {
        let domain_name = handler.domain_name().to_string();

        // Validate domain doesn't already exist
        if self.domains.contains_key(&domain_name) {
            return Err(DslEditError::DomainValidationError(format!(
                "Domain '{}' already registered",
                domain_name
            )));
        }

        // Validate domain vocabulary compatibility with shared vocab
        self.validate_vocabulary_compatibility(handler.get_vocabulary())?;

        // Add domain to registry
        self.domains.insert(domain_name.clone(), handler);

        log::info!("Registered domain: {}", domain_name);
        Ok(())
    }

    /// Get a domain handler by name
    pub fn get_domain(&self, domain_name: &str) -> DslEditResult<&dyn DomainHandler> {
        self.domains
            .get(domain_name)
            .map(|handler| handler.as_ref())
            .ok_or_else(|| DslEditError::DomainNotFound(domain_name.to_string()))
    }

    /// Get all registered domain names
    pub fn list_domains(&self) -> Vec<String> {
        self.domains.keys().cloned().collect()
    }

    /// Route an operation to the appropriate domain
    pub async fn route_operation(
        &self,
        operation: &DslOperation,
        context: &DomainContext,
    ) -> DslEditResult<&dyn DomainHandler> {
        // First, try direct domain lookup from context
        if let Ok(handler) = self.get_domain(&context.domain_name) {
            // Validate that domain supports this operation
            if !handler.supports_operation(&operation.description()) {
                return Err(DslEditError::UnsupportedOperation(
                    operation.description(),
                    context.domain_name.clone(),
                ));
            }
            return Ok(handler);
        }

        // Fall back to routing rules if direct lookup fails
        for rule in &self.routing_rules {
            if rule.matches(operation, context) {
                return self.get_domain(&rule.target_domain);
            }
        }

        Err(DslEditError::DomainNotFound(format!(
            "No domain found for operation: {} in context: {}",
            operation.description(),
            context.domain_name
        )))
    }

    /// Get the shared vocabulary
    pub fn shared_vocabulary(&self) -> &DslVocabulary {
        &self.shared_vocabulary
    }

    /// Perform health check on all domains
    pub async fn health_check_all(&self) -> HashMap<String, DomainHealthStatus> {
        let mut results = HashMap::new();

        for (domain_name, handler) in &self.domains {
            let health = handler.health_check().await;
            results.insert(domain_name.clone(), health);
        }

        results
    }

    /// Validate vocabulary compatibility
    fn validate_vocabulary_compatibility(&self, domain_vocab: &DslVocabulary) -> DslEditResult<()> {
        if !self.is_vocabulary_compatible(&self.shared_vocabulary, domain_vocab) {
            return Err(DslEditError::DomainValidationError(
                "Domain vocabulary incompatible with shared vocabulary".to_string(),
            ));
        }
        Ok(())
    }

    /// Check if two vocabularies are compatible
    fn is_vocabulary_compatible(&self, shared: &DslVocabulary, domain: &DslVocabulary) -> bool {
        // Check for conflicting verb definitions
        for verb in &domain.verbs {
            if let Some(shared_verb) = shared.verbs.iter().find(|v| v.name == verb.name) {
                if shared_verb.signature != verb.signature {
                    return false;
                }
            }
        }
        true
    }
}

impl Default for DomainRegistry {
    fn default() -> Self {
        Self::new()
    }
}

/// DSL Vocabulary system (ONE DSL vocab principle)
#[derive(Debug, Clone, Default, Serialize, Deserialize)]
pub struct DslVocabulary {
    /// Core verbs available in the DSL
    pub verbs: Vec<VerbDefinition>,

    /// Attribute definitions with UUID references
    pub attributes: Vec<AttributeDefinition>,

    /// Data type definitions
    pub types: Vec<TypeDefinition>,

    /// Grammar extensions for this vocabulary
    pub grammar_extensions: Vec<String>,
}

/// Definition of a DSL verb
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct VerbDefinition {
    pub name: String,
    pub description: String,
    pub signature: String,
    pub category: String,
    pub examples: Vec<String>,
    pub validation_rules: Vec<String>,
}

/// Definition of an attribute (linked to data dictionary)
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct AttributeDefinition {
    pub attribute_id: Uuid,
    pub name: String,
    pub data_type: String,
    pub domain: String,
    pub validation_rules: Vec<String>,
}

/// Definition of a data type
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct TypeDefinition {
    pub type_name: String,
    pub base_type: String,
    pub constraints: Vec<String>,
    pub validation_pattern: Option<String>,
}

/// State transition definition for domain state machines
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct StateTransition {
    pub from_state: String,
    pub to_state: String,
    pub transition_name: String,
    pub required_conditions: Vec<String>,
    pub side_effects: Vec<String>,
}

/// Validation rule definition
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ValidationRule {
    pub rule_id: String,
    pub rule_name: String,
    pub rule_type: ValidationRuleType,
    pub description: String,
    pub parameters: HashMap<String, serde_json::Value>,
    pub severity: ValidationSeverity,
}

/// Types of validation rules
#[derive(Debug, Clone, Serialize, Deserialize)]
pub enum ValidationRuleType {
    SyntaxValidation,
    SemanticValidation,
    BusinessRuleValidation,
    ComplianceValidation,
    DataIntegrityValidation,
}

/// Severity levels for validation
#[derive(Debug, Clone, Serialize, Deserialize)]
pub enum ValidationSeverity {
    Info,
    Warning,
    Error,
    Critical,
}

/// Routing rule for operation dispatching
#[derive(Debug, Clone)]
pub struct RoutingRule {
    pub rule_id: String,
    pub target_domain: String,
    pub operation_patterns: Vec<String>,
    pub context_conditions: Vec<String>,
}

impl RoutingRule {
    /// Check if this routing rule matches an operation and context
    pub fn matches(&self, operation: &DslOperation, context: &DomainContext) -> bool {
        // Check operation pattern matching
        let operation_matches = self.operation_patterns.is_empty()
            || self
                .operation_patterns
                .iter()
                .any(|pattern| self.matches_pattern(&operation.description(), pattern));

        // Check context conditions
        let context_matches = self.context_conditions.is_empty()
            || self
                .context_conditions
                .iter()
                .all(|condition| self.matches_context_condition(context, condition));

        operation_matches && context_matches
    }

    fn matches_pattern(&self, text: &str, pattern: &str) -> bool {
        // Simple pattern matching - could be enhanced with regex
        if pattern.contains('*') {
            let parts: Vec<&str> = pattern.split('*').collect();
            if parts.len() == 2 {
                text.starts_with(parts[0]) && text.ends_with(parts[1])
            } else {
                false
            }
        } else {
            text == pattern
        }
    }

    fn matches_context_condition(&self, context: &DomainContext, condition: &str) -> bool {
        // Simple condition matching - could be enhanced with more complex expressions
        if let Some((key, value)) = condition.split_once('=') {
            match key.trim() {
                "domain" => context.domain_name == value.trim(),
                "subdomain" => context.subdomain.as_deref() == Some(value.trim()),
                _ => context
                    .business_context
                    .get(key.trim())
                    .and_then(|v| v.as_str())
                    .map(|v| v == value.trim())
                    .unwrap_or(false),
            }
        } else {
            false
        }
    }
}

/// Health status for domain handlers
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct DomainHealthStatus {
    pub domain_name: String,
    pub status: HealthStatus,
    pub last_check: chrono::DateTime<chrono::Utc>,
    pub metrics: HashMap<String, f64>,
    pub errors: Vec<String>,
}

/// Health status enumeration
#[derive(Debug, Clone, Serialize, Deserialize)]
pub enum HealthStatus {
    Healthy,
    Degraded,
    Unhealthy,
    Unknown,
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::dsl::operations::OperationBuilder;

    // Mock domain handler for testing
    struct MockDomainHandler {
        name: String,
        vocabulary: DslVocabulary,
        operations: Vec<String>,
    }

    #[async_trait]
    impl DomainHandler for MockDomainHandler {
        fn domain_name(&self) -> &str {
            &self.name
        }

        fn domain_version(&self) -> &str {
            "1.0.0"
        }

        fn domain_description(&self) -> &str {
            "Mock domain for testing"
        }

        fn get_vocabulary(&self) -> &DslVocabulary {
            &self.vocabulary
        }

        async fn transform_operation_to_dsl(
            &self,
            operation: &DslOperation,
            _context: &DomainContext,
        ) -> DslEditResult<String> {
            Ok(format!("(mock.operation \"{}\")", operation.description()))
        }

        async fn validate_operation(
            &self,
            _operation: &DslOperation,
            _context: &DomainContext,
        ) -> DslEditResult<()> {
            Ok(())
        }

        fn get_valid_transitions(&self) -> &[StateTransition] {
            &[]
        }

        fn validate_state_transition(&self, _from: &str, _to: &str) -> DslEditResult<()> {
            Ok(())
        }

        async fn apply_business_rules(
            &self,
            dsl_fragment: &str,
            _context: &DomainContext,
        ) -> DslEditResult<String> {
            Ok(dsl_fragment.to_string())
        }

        fn supported_operations(&self) -> &[String] {
            &self.operations
        }

        fn get_validation_rules(&self) -> &[ValidationRule] {
            &[]
        }

        async fn extract_context_from_dsl(&self, _dsl: &str) -> DslEditResult<DomainContext> {
            Ok(DomainContext::new(&self.name))
        }

        async fn health_check(&self) -> DomainHealthStatus {
            DomainHealthStatus {
                domain_name: self.name.clone(),
                status: HealthStatus::Healthy,
                last_check: chrono::Utc::now(),
                metrics: HashMap::new(),
                errors: Vec::new(),
            }
        }
    }

    #[test]
    fn test_domain_registration() {
        let mut registry = DomainRegistry::new();

        let mock_domain = Box::new(MockDomainHandler {
            name: "test".to_string(),
            vocabulary: DslVocabulary::default(),
            operations: vec!["create_entity".to_string()],
        });

        assert!(registry.register_domain(mock_domain).is_ok());
        assert!(registry.get_domain("test").is_ok());
        assert!(registry.get_domain("nonexistent").is_err());
    }

    #[tokio::test]
    async fn test_operation_routing() {
        let mut registry = DomainRegistry::new();

        let mock_domain = Box::new(MockDomainHandler {
            name: "onboarding".to_string(),
            vocabulary: DslVocabulary::default(),
            operations: vec!["Create Company entity".to_string()],
        });

        registry.register_domain(mock_domain).unwrap();

        let operation = OperationBuilder::new("test_user").create_entity(
            "Company",
            [("name".to_string(), serde_json::json!("Test Corp"))]
                .iter()
                .cloned()
                .collect(),
        );

        let context = DomainContext::onboarding();

        let handler = registry.route_operation(&operation, &context).await;
        assert!(handler.is_ok());
        assert_eq!(handler.unwrap().domain_name(), "onboarding");
    }

    #[test]
    fn test_routing_rule_matching() {
        let rule = RoutingRule {
            rule_id: "test_rule".to_string(),
            target_domain: "onboarding".to_string(),
            operation_patterns: vec!["Create * entity".to_string()],
            context_conditions: vec!["domain=onboarding".to_string()],
        };

        let operation = OperationBuilder::new("test_user").create_entity(
            "Company",
            [("name".to_string(), serde_json::json!("Test Corp"))]
                .iter()
                .cloned()
                .collect(),
        );

        let context = DomainContext::onboarding();

        assert!(rule.matches(&operation, &context));
    }
}
