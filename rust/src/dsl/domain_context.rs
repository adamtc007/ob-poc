//! Domain Context System for DSL Operation Routing
//!
//! This module provides the domain context system that enables unified DSL operations
//! across different business domains while maintaining domain-specific business logic.
//!
//! The context system supports the "DSL.Domain as context switch" pattern where
//! domain-specific operations are routed through a unified interface.

use serde::{Deserialize, Serialize};
use std::collections::HashMap;
use uuid::Uuid;

/// Domain context for DSL operations - enables domain switching in unified editor
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct DomainContext {
    /// Primary domain name (e.g., "onboarding", "kyc", "ubo")
    pub domain_name: String,

    /// Optional subdomain for fine-grained routing (e.g., "hedge_fund", "corporate")
    pub subdomain: Option<String>,

    /// Business context data specific to the operation
    pub business_context: HashMap<String, serde_json::Value>,

    /// State requirements and constraints for the operation
    pub state_requirements: StateRequirements,

    /// Domain version for compatibility checking
    pub domain_version: String,

    /// Optional request ID for audit trail linking
    pub request_id: Option<Uuid>,
}

impl DomainContext {
    /// Create a new domain context
    pub fn new(domain_name: impl Into<String>) -> Self {
        Self {
            domain_name: domain_name.into(),
            subdomain: None,
            business_context: HashMap::new(),
            state_requirements: StateRequirements::default(),
            domain_version: "1.0.0".to_string(),
            request_id: None,
        }
    }

    /// Create domain context for onboarding operations
    pub fn onboarding() -> Self {
        Self::new("onboarding")
    }

    /// Create domain context for KYC operations
    pub fn kyc() -> Self {
        Self::new("kyc")
    }

    /// Create domain context for UBO operations
    pub fn ubo() -> Self {
        Self::new("ubo")
    }

    /// Auto-detect domain from DSL content patterns
    pub fn auto_detect() -> Self {
        // Default to kyc for now - could be enhanced to analyze DSL content
        Self::kyc()
    }

    /// Set subdomain for fine-grained routing
    pub(crate) fn with_subdomain(mut self, subdomain: impl Into<String>) -> Self {
        self.subdomain = Some(subdomain.into());
        self
    }

    /// Set business context data
    pub fn with_context(mut self, key: impl Into<String>, value: serde_json::Value) -> Self {
        self.business_context.insert(key.into(), value);
        self
    }

    /// Set domain version
    pub(crate) fn with_version(mut self, version: impl Into<String>) -> Self {
        self.domain_version = version.into();
        self
    }

    /// Get business context value by key
    pub fn get_context<T>(&self, key: &str) -> Option<T>
    where
        T: for<'de> Deserialize<'de>,
    {
        self.business_context
            .get(key)
            .and_then(|v| serde_json::from_value(v.clone()).ok())
    }

    /// Get required business context value by key
    pub(crate) fn get_required_context<T>(&self, key: &str) -> Result<T, String>
    where
        T: for<'de> Deserialize<'de>,
    {
        self.get_context(key)
            .ok_or_else(|| format!("Required context '{}' not found", key))
    }

    /// Get full domain identifier including subdomain
    pub(crate) fn full_domain_name(&self) -> String {
        match &self.subdomain {
            Some(sub) => format!("{}.{}", self.domain_name, sub),
            None => self.domain_name.clone(),
        }
    }

    /// Check if this context matches a domain pattern
    pub(crate) fn matches_domain(&self, pattern: &str) -> bool {
        let full_name = self.full_domain_name();

        // Exact match
        if full_name == pattern {
            return true;
        }

        // Wildcard match (e.g., "onboarding.*" matches "onboarding.corporate")
        if let Some(prefix) = pattern.strip_suffix('*') {
            return full_name.starts_with(prefix);
        }

        // Domain-only match (ignores subdomain)
        if self.domain_name == pattern {
            return true;
        }

        false
    }
}

/// State requirements and constraints for DSL operations
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct StateRequirements {
    /// Current functional state (optional - can be derived)
    pub current_state: Option<String>,

    /// Required state before operation can execute
    pub required_state: Option<String>,

    /// Expected state after successful operation
    pub target_state: Option<String>,

    /// Whether state validation is strict (fail if requirements not met)
    pub strict_validation: bool,

    /// Custom state validation rules
    pub validation_rules: Vec<StateValidationRule>,

    /// State transition metadata
    pub transition_metadata: HashMap<String, serde_json::Value>,
}

impl Default for StateRequirements {
    fn default() -> Self {
        Self {
            current_state: None,
            required_state: None,
            target_state: None,
            strict_validation: true,
            validation_rules: Vec::new(),
            transition_metadata: HashMap::new(),
        }
    }
}

impl StateRequirements {
    /// Create permissive state requirements (no validation)
    pub(crate) fn permissive() -> Self {
        Self {
            strict_validation: false,
            ..Default::default()
        }
    }

    /// Create strict state requirements with current and target states
    pub fn strict(current: impl Into<String>, target: impl Into<String>) -> Self {
        Self {
            current_state: Some(current.into()),
            target_state: Some(target.into()),
            strict_validation: true,
            ..Default::default()
        }
    }

    /// Add a state validation rule
    pub(crate) fn with_rule(mut self, rule: StateValidationRule) -> Self {
        self.validation_rules.push(rule);
        self
    }

    /// Add transition metadata
    pub(crate) fn with_metadata(
        mut self,
        key: impl Into<String>,
        value: serde_json::Value,
    ) -> Self {
        self.transition_metadata.insert(key.into(), value);
        self
    }
}

/// State validation rule for custom domain logic
#[derive(Debug, Clone, Serialize, Deserialize)]
pub(crate) struct StateValidationRule {
    /// Rule identifier
    pub rule_id: String,

    /// Rule description for debugging
    pub description: String,

    /// Rule type for handler routing
    pub rule_type: StateValidationRuleType,

    /// Rule parameters (domain-specific)
    pub parameters: HashMap<String, serde_json::Value>,
}

/// Types of state validation rules
#[derive(Debug, Clone, Serialize, Deserialize)]
pub(crate) enum StateValidationRuleType {
    /// Require specific attribute to have a value
    RequireAttribute { attribute_id: Uuid },

    /// Require minimum number of completed steps
    RequireCompletedSteps { minimum_count: u32 },

    /// Require specific documents to be collected
    RequireDocuments { document_types: Vec<String> },

    /// Custom business rule validation
    BusinessRule { rule_name: String },

    /// Time-based validation (e.g., cooling-off periods)
    TemporalConstraint { constraint_type: String },
}

/// Domain operation metadata for tracking and audit
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct OperationMetadata {
    /// Unique operation identifier
    pub operation_id: Uuid,

    /// Operation timestamp
    pub timestamp: chrono::DateTime<chrono::Utc>,

    /// User who initiated the operation
    pub initiated_by: String,

    /// Operation priority level
    pub priority: OperationPriority,

    /// Tags for categorization and search
    pub tags: Vec<String>,

    /// Custom metadata
    pub custom_data: HashMap<String, serde_json::Value>,
}

impl Default for OperationMetadata {
    fn default() -> Self {
        Self {
            operation_id: Uuid::new_v4(),
            timestamp: chrono::Utc::now(),
            initiated_by: "system".to_string(),
            priority: OperationPriority::Normal,
            tags: Vec::new(),
            custom_data: HashMap::new(),
        }
    }
}

/// Priority levels for DSL operations
#[derive(Debug, Clone, Serialize, Deserialize)]
pub enum OperationPriority {
    Low,
    Normal,
    High,
    Critical,
}

#[cfg(test)]
mod tests {
    use super::*;
    use serde_json::json;

    #[test]
    fn test_domain_context_creation() {
        let context = DomainContext::onboarding()
            .with_subdomain("corporate")
            .with_context("cbu_id", json!("CBU-1234"))
            .with_version("2.0.0");

        assert_eq!(context.domain_name, "onboarding");
        assert_eq!(context.subdomain, Some("corporate".to_string()));
        assert_eq!(context.domain_version, "2.0.0");
        assert_eq!(context.full_domain_name(), "onboarding.corporate");

        let cbu_id: String = context.get_context("cbu_id").unwrap();
        assert_eq!(cbu_id, "CBU-1234");
    }

    #[test]
    fn test_domain_matching() {
        let context = DomainContext::onboarding().with_subdomain("hedge_fund");

        assert!(context.matches_domain("onboarding.hedge_fund"));
        assert!(context.matches_domain("onboarding"));
        assert!(context.matches_domain("onboarding.*"));
        assert!(!context.matches_domain("kyc"));
        assert!(!context.matches_domain("onboarding.corporate"));
    }

    #[test]
    fn test_state_requirements() {
        let requirements = StateRequirements::strict("PRODUCTS_ADDED", "KYC_STARTED").with_rule(
            StateValidationRule {
                rule_id: "require_products".to_string(),
                description: "At least one product must be added".to_string(),
                rule_type: StateValidationRuleType::RequireCompletedSteps { minimum_count: 1 },
                parameters: HashMap::new(),
            },
        );

        assert_eq!(
            requirements.current_state,
            Some("PRODUCTS_ADDED".to_string())
        );
        assert_eq!(requirements.target_state, Some("KYC_STARTED".to_string()));
        assert!(requirements.strict_validation);
        assert_eq!(requirements.validation_rules.len(), 1);
    }

    #[test]
    fn test_context_value_retrieval() {
        let context = DomainContext::kyc()
            .with_context("customer_id", json!(12345))
            .with_context("risk_score", json!(75.5))
            .with_context("documents", json!(["passport", "address_proof"]));

        let customer_id: u64 = context.get_context("customer_id").unwrap();
        assert_eq!(customer_id, 12345);

        let risk_score: f64 = context.get_context("risk_score").unwrap();
        assert_eq!(risk_score, 75.5);

        let documents: Vec<String> = context.get_context("documents").unwrap();
        assert_eq!(documents, vec!["passport", "address_proof"]);

        // Test required context
        let result: Result<String, String> = context.get_required_context("nonexistent");
        assert!(result.is_err());
    }
}
