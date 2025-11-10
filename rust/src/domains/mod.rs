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
    use crate::{Key, Literal, Value}; // Import new AST types
    use chrono::Utc;
    use std::collections::HashMap;
    use uuid::Uuid;

    /// Generate a timestamp string for DSL operations
    pub fn generate_timestamp() -> String {
        Utc::now().to_rfc3339()
    }

    /// Generate unique operation ID
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

    /// Create a verb form fragment from verb name and key-value attributes
    pub fn create_verb_form_fragment(verb: &str, attributes: &HashMap<Key, Value>) -> String {
        let mut parts = vec![verb.to_string()];

        for (key, value) in attributes {
            let key_str = format!(":{}", key.as_str());
            let value_str = format_value_to_dsl(value);
            parts.push(format!("{} {}", key_str, value_str));
        }

        format!("({})", parts.join(" "))
    }

    /// Validate that required context keys are present
    pub fn validate_required_context(
        context: &DomainContext,
        required_keys: &[&str],
    ) -> Result<(), String> {
        for key in required_keys {
            if context.get_context::<String>(key).is_none() {
                return Err(format!("Missing required context key: {}", key));
            }
        }
        Ok(())
    }

    /// Validate state transition is allowed
    pub fn validate_state_transition(
        domain: &str,
        allowed_transitions: &[(String, String)],
        from: &str,
        to: &str,
    ) -> Result<(), String> {
        let transition = (from.to_string(), to.to_string());
        if allowed_transitions.contains(&transition) {
            Ok(())
        } else {
            Err(format!(
                "Invalid state transition in {}: {} -> {}",
                domain, from, to
            ))
        }
    }

    /// Extract a value from DSL text using simple pattern matching
    pub fn extract_value_from_dsl(dsl: &str, key: &str) -> Option<String> {
        let patterns = [
            format!("(:{} \"", key),
            format!("({} \"", key),
            format!(" :{} \"", key),
            format!(" {} \"", key),
        ];

        for pattern in &patterns {
            if let Some(start) = dsl.find(pattern) {
                let value_start = start + pattern.len();
                if let Some(end) = dsl[value_start..].find('"') {
                    return Some(dsl[value_start..value_start + end].to_string());
                }
            }
        }
        None
    }

    /// Legacy wrapper function for compatibility
    pub fn wrap_dsl_fragment(
        verb: &str,
        attrs: &std::collections::HashMap<String, serde_json::Value>,
    ) -> String {
        let mut attributes = HashMap::new();

        for (key, value) in attrs {
            let dsl_key = Key::new(key);
            let dsl_value = match value {
                serde_json::Value::String(s) => Value::Literal(Literal::String(s.clone())),
                serde_json::Value::Number(n) => {
                    Value::Literal(Literal::Number(n.as_f64().unwrap_or(0.0)))
                }
                serde_json::Value::Bool(b) => Value::Literal(Literal::Boolean(*b)),
                _ => Value::Literal(Literal::String(value.to_string())),
            };
            attributes.insert(dsl_key, dsl_value);
        }

        create_verb_form_fragment(verb, &attributes)
    }

    /// Helper to format a Value into its DSL string representation
    fn format_value_to_dsl(value: &Value) -> String {
        match value {
            Value::Literal(literal) => match literal {
                Literal::String(s) => format!("\"{}\"", s),
                Literal::Number(n) => n.to_string(),
                Literal::Boolean(b) => b.to_string(),
                Literal::Date(s) => format!("\"{}\"", s), // Dates are strings
                Literal::Uuid(s) => format!("\"{}\"", s), // UUIDs are strings
            },
            Value::Identifier(s) => s.clone(), // Identifiers are not quoted
            Value::List(items) => {
                let formatted_items: Vec<String> = items.iter().map(format_value_to_dsl).collect();
                format!("[{}]", formatted_items.join(", "))
            }
            Value::Map(map) => {
                let formatted_pairs: Vec<String> = map
                    .iter()
                    .map(|(key, val)| format!(":{} {}", key.as_str(), format_value_to_dsl(val)))
                    .collect();
                format!("{{{}}}", formatted_pairs.join(", "))
            }
            Value::AttrRef(uuid_str) => format!("@attr{{{}}}", uuid_str),
        }
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
        assert!(fragment.starts_with("(create"));
        assert!(fragment.contains(":name \"Test Corp\""));
        assert!(fragment.contains(":status \"active\""));
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
