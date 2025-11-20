//! Core Business Domain Handlers
//!
//! This module provides domain-specific handlers for the core business domains
//! that power the OB-POC system. These domains handle the essential workflows
//! for financial services onboarding and compliance.
//!
//! ## Core Domains
//! - **Onboarding**: Client onboarding workflows and case management
//! - **KYC**: Know Your Customer processes and verification
//! - **UBO**: Ultimate Beneficial Ownership analysis and calculations
//! - **ISDA**: ISDA derivatives, master agreements, and trade lifecycle
//! - **Entity**: Entity registration and classification
//! - **Products**: Product and service provisioning
//! - **Documents**: Document management and verification

// Domain modules will be implemented as needed
// For now, providing stub implementations

use crate::parser::ast::{PropertyMap, Value};
use async_trait::async_trait;
use std::collections::HashMap;

/// Core domain handler trait
#[async_trait]
pub trait DomainHandler: Send + Sync {
    /// Get the domain name
    fn domain_name(&self) -> &str;

    /// Get supported verbs for this domain
    fn supported_verbs(&self) -> Vec<&str>;

    /// Validate domain-specific DSL
    async fn validate_dsl(&self, verb: &str, properties: &PropertyMap) -> Result<(), String>;

    /// Process domain-specific operation
    async fn process_operation(
        &self,
        verb: &str,
        properties: &PropertyMap,
    ) -> Result<DomainResult, String>;

    /// Get domain-specific templates
    fn get_templates(&self) -> HashMap<String, String>;
}

/// Result from domain processing
#[derive(Debug, Clone)]
pub struct DomainResult {
    pub success: bool,
    pub domain: String,
    pub verb: String,
    pub result_data: HashMap<String, Value>,
    pub generated_artifacts: Vec<String>,
    pub next_actions: Vec<String>,
    pub compliance_flags: Vec<String>,
}

impl DomainResult {
    pub fn success(domain: &str, verb: &str) -> Self {
        Self {
            success: true,
            domain: domain.to_string(),
            verb: verb.to_string(),
            result_data: HashMap::new(),
            generated_artifacts: Vec::new(),
            next_actions: Vec::new(),
            compliance_flags: Vec::new(),
        }
    }

    pub fn failure(domain: &str, verb: &str, reason: &str) -> Self {
        let mut result = Self::success(domain, verb);
        result.success = false;
        result
            .result_data
            .insert("error".to_string(), Value::string(reason));
        result
    }

    pub fn with_data(mut self, key: &str, value: Value) -> Self {
        self.result_data.insert(key.to_string(), value);
        self
    }

    pub fn with_artifact(mut self, artifact: &str) -> Self {
        self.generated_artifacts.push(artifact.to_string());
        self
    }

    pub fn with_compliance_flag(mut self, flag: &str) -> Self {
        self.compliance_flags.push(flag.to_string());
        self
    }
}

/// Domain registry for managing all domain handlers
pub struct DomainRegistry {
    handlers: HashMap<String, Box<dyn DomainHandler>>,
}

impl DomainRegistry {
    pub fn new() -> Self {
        let mut registry = Self {
            handlers: HashMap::new(),
        };

        // Register core domain handlers (stub implementations for now)
        registry.register_handler(Box::new(StubDomainHandler::new("kyc")));
        registry.register_handler(Box::new(StubDomainHandler::new("onboarding")));
        registry.register_handler(Box::new(StubDomainHandler::new("ubo")));
        registry.register_handler(Box::new(StubDomainHandler::new("isda")));
        registry.register_handler(Box::new(StubDomainHandler::new("entity")));
        registry.register_handler(Box::new(StubDomainHandler::new("products")));
        registry.register_handler(Box::new(StubDomainHandler::new("documents")));

        registry
    }

    pub fn register_handler(&mut self, handler: Box<dyn DomainHandler>) {
        let domain_name = handler.domain_name().to_string();
        self.handlers.insert(domain_name, handler);
    }

    pub fn get_handler(&self, domain: &str) -> Option<&dyn DomainHandler> {
        self.handlers.get(domain).map(|h| h.as_ref())
    }

    pub fn detect_domain_from_verb(&self, verb: &str) -> Option<String> {
        for (domain_name, handler) in &self.handlers {
            if handler.supported_verbs().contains(&verb) {
                return Some(domain_name.clone());
            }
        }
        None
    }

    pub fn get_all_domains(&self) -> Vec<String> {
        self.handlers.keys().cloned().collect()
    }

    pub async fn validate_verb(
        &self,
        domain: &str,
        verb: &str,
        properties: &PropertyMap,
    ) -> Result<(), String> {
        match self.get_handler(domain) {
            Some(handler) => handler.validate_dsl(verb, properties).await,
            None => Err(format!("Unknown domain: {}", domain)),
        }
    }

    pub async fn process_domain_operation(
        &self,
        domain: &str,
        verb: &str,
        properties: &PropertyMap,
    ) -> Result<DomainResult, String> {
        match self.get_handler(domain) {
            Some(handler) => handler.process_operation(verb, properties).await,
            None => Err(format!("Unknown domain: {}", domain)),
        }
    }
}

impl Default for DomainRegistry {
    fn default() -> Self {
        Self::new()
    }
}

/// Extract domain from verb (e.g., "kyc.collect" -> "kyc")
pub fn extract_domain_from_verb(verb: &str) -> Option<&str> {
    verb.split('.').next()
}

/// Check if a verb belongs to a specific domain
pub fn verb_belongs_to_domain(verb: &str, domain: &str) -> bool {
    verb.starts_with(&format!("{}.", domain))
}

/// Get all supported verbs across all domains
pub fn get_all_supported_verbs() -> Vec<String> {
    let registry = DomainRegistry::new();
    let mut all_verbs = Vec::new();

    for domain_name in registry.get_all_domains() {
        if let Some(handler) = registry.get_handler(&domain_name) {
            for verb in handler.supported_verbs() {
                all_verbs.push(format!("{}.{}", domain_name, verb));
            }
        }
    }

    all_verbs.sort();
    all_verbs
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_domain_registry() {
        let registry = DomainRegistry::new();
        let domains = registry.get_all_domains();

        // Should have core domains
        assert!(domains.contains(&"kyc".to_string()));
        assert!(domains.contains(&"onboarding".to_string()));
        assert!(domains.contains(&"ubo".to_string()));
        assert!(domains.contains(&"isda".to_string()));
    }

    #[test]
    fn test_verb_domain_extraction() {
        assert_eq!(extract_domain_from_verb("kyc.collect"), Some("kyc"));
        assert_eq!(extract_domain_from_verb("ubo.analyze"), Some("ubo"));
        assert_eq!(
            extract_domain_from_verb("isda.establish_master"),
            Some("isda")
        );
        assert_eq!(extract_domain_from_verb("invalid"), Some("invalid"));
    }

    #[test]
    fn test_verb_belongs_to_domain() {
        assert!(verb_belongs_to_domain("kyc.collect", "kyc"));
        assert!(verb_belongs_to_domain("ubo.calculate", "ubo"));
        assert!(!verb_belongs_to_domain("kyc.collect", "ubo"));
    }
}

/// Stub domain handler for testing and compilation
pub struct StubDomainHandler {
    domain: String,
}

impl StubDomainHandler {
    pub fn new(domain: &str) -> Self {
        Self {
            domain: domain.to_string(),
        }
    }
}

#[async_trait]
impl DomainHandler for StubDomainHandler {
    fn domain_name(&self) -> &str {
        &self.domain
    }

    fn supported_verbs(&self) -> Vec<&str> {
        match self.domain.as_str() {
            "kyc" => vec!["collect", "verify", "assess"],
            "ubo" => vec!["collect-entity-data", "analyze", "calculate"],
            "isda" => vec!["establish_master", "execute_trade", "margin_call"],
            "onboarding" => vec!["create", "update", "approve"],
            "entity" => vec!["register", "classify", "link"],
            "products" => vec!["add", "configure", "provision"],
            "documents" => vec!["catalog", "verify", "extract"],
            _ => vec!["process"],
        }
    }

    async fn validate_dsl(&self, _verb: &str, _properties: &PropertyMap) -> Result<(), String> {
        Ok(()) // Stub validation always passes
    }

    async fn process_operation(
        &self,
        verb: &str,
        _properties: &PropertyMap,
    ) -> Result<DomainResult, String> {
        Ok(DomainResult::success(&self.domain, verb))
    }

    fn get_templates(&self) -> HashMap<String, String> {
        HashMap::new()
    }
}
