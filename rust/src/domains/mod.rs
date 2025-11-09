//! Domain Implementations Module
//!
//! This module contains domain-specific implementations that work with the
//! centralized DSL editing system. Each domain provides specialized business
//! logic while adhering to the unified DomainHandler interface.
//!
//! ## Domain Structure:
//! - Each domain implements the `DomainHandler` trait
//! - Domains provide vocabulary extensions to the shared DSL vocabulary
//! - Domain-specific state machines and validation rules
//! - Business rule transformations for DSL generation
//!
//! ## Available Domains:
//! - `onboarding`: Client onboarding workflows and case management
//! - `kyc`: Know Your Customer compliance and verification
//! - `ubo`: Ultimate Beneficial Ownership discovery and analysis
//!
//! ## Architecture Benefits:
//! - Centralized interface with domain-specific logic
//! - Shared grammar and dictionary validation
//! - Consistent error handling across domains
//! - Easy domain registration and discovery

pub mod kyc;
pub mod onboarding;
pub mod ubo;

// Re-export domain handler implementations for easy registration
pub use kyc::KycDomainHandler;
pub use onboarding::OnboardingDomainHandler;
pub use ubo::UboDomainHandler;

/// Register all standard domains with the domain registry
pub fn register_all_domains(
    registry: &mut crate::dsl::domain_registry::DomainRegistry,
) -> Result<(), crate::dsl::DslEditError> {
    registry.register_domain(Box::new(OnboardingDomainHandler::new()))?;
    registry.register_domain(Box::new(KycDomainHandler::new()))?;
    registry.register_domain(Box::new(UboDomainHandler::new()))?;
    Ok(())
}

/// Get list of all available domain names
pub fn available_domains() -> Vec<&'static str> {
    vec!["onboarding", "kyc", "ubo"]
}

/// Domain-specific error types that can occur during domain operations
#[derive(Debug, thiserror::Error)]
pub enum DomainError {
    #[error("Invalid state transition in domain '{domain}': {from} -> {to}")]
    InvalidStateTransition {
        domain: String,
        from: String,
        to: String,
    },

    #[error("Missing required context in domain '{domain}': {context_key}")]
    MissingRequiredContext { domain: String, context_key: String },

    #[error("Business rule violation in domain '{domain}': {rule}")]
    BusinessRuleViolation { domain: String, rule: String },

    #[error("Unsupported operation in domain '{domain}': {operation}")]
    UnsupportedOperation { domain: String, operation: String },

    #[error("Validation failed in domain '{domain}': {message}")]
    ValidationFailed { domain: String, message: String },
}

/// Common domain utilities and helper functions
pub mod common {
    use super::*;
    use crate::dsl::domain_context::DomainContext;
    use chrono::Utc;
    use std::collections::HashMap;
    use uuid::Uuid;

    /// Generate a timestamp string for DSL operations
    pub fn generate_timestamp() -> String {
        Utc::now().to_rfc3339()
    }

    /// Generate a unique operation ID
    pub fn generate_operation_id() -> String {
        Uuid::new_v4().to_string()
    }

    /// Extract CBU ID from domain context if available
    pub fn extract_cbu_id(context: &DomainContext) -> Option<String> {
        context.get_context::<String>("cbu_id")
    }

    /// Extract entity ID from domain context if available
    pub fn extract_entity_id(context: &DomainContext) -> Option<String> {
        context.get_context::<String>("entity_id")
    }

    /// Create a standard DSL fragment wrapper
    pub fn wrap_dsl_fragment(
        verb: &str,
        attributes: &HashMap<String, serde_json::Value>,
    ) -> String {
        let mut parts = vec![format!("({}.", verb)];

        for (key, value) in attributes {
            let formatted_value = match value {
                serde_json::Value::String(s) => format!("\"{}\"", s),
                serde_json::Value::Number(n) => n.to_string(),
                serde_json::Value::Bool(b) => b.to_string(),
                _ => format!("\"{}\"", value),
            };
            parts.push(format!("{} {}", key, formatted_value));
        }

        parts.push(")".to_string());
        parts.join(" ")
    }

    /// Validate required context keys are present
    pub fn validate_required_context(
        context: &DomainContext,
        required_keys: &[&str],
    ) -> Result<(), DomainError> {
        for key in required_keys {
            if !context.business_context.contains_key(*key) {
                return Err(DomainError::MissingRequiredContext {
                    domain: context.domain_name.clone(),
                    context_key: key.to_string(),
                });
            }
        }
        Ok(())
    }

    /// Standard state transition validation
    pub fn validate_state_transition(
        domain_name: &str,
        allowed_transitions: &[(String, String)],
        from: &str,
        to: &str,
    ) -> Result<(), DomainError> {
        let transition = (from.to_string(), to.to_string());

        if !allowed_transitions.contains(&transition) {
            return Err(DomainError::InvalidStateTransition {
                domain: domain_name.to_string(),
                from: from.to_string(),
                to: to.to_string(),
            });
        }

        Ok(())
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::dsl::domain_registry::DomainRegistry;

    #[test]
    fn test_domain_registration() {
        let mut registry = DomainRegistry::new();
        let result = register_all_domains(&mut registry);
        assert!(result.is_ok());

        let domains = registry.list_domains();
        assert_eq!(domains.len(), 3);
        assert!(domains.contains(&"onboarding".to_string()));
        assert!(domains.contains(&"kyc".to_string()));
        assert!(domains.contains(&"ubo".to_string()));
    }

    #[test]
    fn test_available_domains() {
        let domains = available_domains();
        assert_eq!(domains.len(), 3);
        assert!(domains.contains(&"onboarding"));
        assert!(domains.contains(&"kyc"));
        assert!(domains.contains(&"ubo"));
    }

    #[test]
    fn test_common_utilities() {
        use crate::dsl::domain_context::DomainContext;
        use serde_json::json;

        let timestamp = common::generate_timestamp();
        assert!(!timestamp.is_empty());

        let op_id = common::generate_operation_id();
        assert!(!op_id.is_empty());

        let context = DomainContext::new("test")
            .with_context("cbu_id", json!("CBU-1234"))
            .with_context("entity_id", json!("ENT-5678"));

        assert_eq!(
            common::extract_cbu_id(&context),
            Some("CBU-1234".to_string())
        );
        assert_eq!(
            common::extract_entity_id(&context),
            Some("ENT-5678".to_string())
        );
    }

    #[test]
    fn test_dsl_fragment_wrapping() {
        use serde_json::json;
        use std::collections::HashMap;

        let mut attrs = HashMap::new();
        attrs.insert("name".to_string(), json!("Test Corp"));
        attrs.insert("status".to_string(), json!("active"));

        let fragment = common::wrap_dsl_fragment("create", &attrs);
        assert!(fragment.starts_with("(create."));
        assert!(fragment.contains("name \"Test Corp\""));
        assert!(fragment.contains("status \"active\""));
        assert!(fragment.ends_with(')'));
    }

    #[test]
    fn test_context_validation() {
        use crate::dsl::domain_context::DomainContext;
        use serde_json::json;

        let context = DomainContext::new("test").with_context("required_key", json!("value"));

        let result = common::validate_required_context(&context, &["required_key"]);
        assert!(result.is_ok());

        let result = common::validate_required_context(&context, &["missing_key"]);
        assert!(result.is_err());
    }

    #[test]
    fn test_state_transition_validation() {
        let allowed = vec![
            ("CREATE".to_string(), "PRODUCTS_ADDED".to_string()),
            ("PRODUCTS_ADDED".to_string(), "KYC_STARTED".to_string()),
        ];

        let result =
            common::validate_state_transition("test", &allowed, "CREATE", "PRODUCTS_ADDED");
        assert!(result.is_ok());

        let result = common::validate_state_transition("test", &allowed, "CREATE", "KYC_STARTED");
        assert!(result.is_err());
    }
}
