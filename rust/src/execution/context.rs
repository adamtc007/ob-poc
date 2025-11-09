//! Execution Context Utilities
//!
//! This module provides utilities for managing execution contexts, environment variables,
//! and session management for DSL execution. It includes context builders, environment
//! validation, and integration management helpers.

use anyhow::{Context, Result};
use chrono::{DateTime, Utc};
use serde::{Deserialize, Serialize};
use serde_json::Value;
use std::collections::HashMap;
use uuid::Uuid;

use super::ExecutionContext;

/// Builder for creating execution contexts
#[derive(Debug, Clone)]
pub struct ExecutionContextBuilder {
    session_id: Option<Uuid>,
    business_unit_id: Option<String>,
    domain: Option<String>,
    executor: Option<String>,
    started_at: Option<DateTime<Utc>>,
    environment: HashMap<String, Value>,
    integrations: Vec<String>,
}

impl ExecutionContextBuilder {
    /// Create a new context builder
    pub fn new() -> Self {
        Self {
            session_id: None,
            business_unit_id: None,
            domain: None,
            executor: None,
            started_at: None,
            environment: HashMap::new(),
            integrations: Vec::new(),
        }
    }

    /// Set the session ID
    pub fn with_session_id(mut self, session_id: Uuid) -> Self {
        self.session_id = Some(session_id);
        self
    }

    /// Set the business unit ID
    pub fn with_business_unit_id(mut self, business_unit_id: impl Into<String>) -> Self {
        self.business_unit_id = Some(business_unit_id.into());
        self
    }

    /// Set the domain
    pub fn with_domain(mut self, domain: impl Into<String>) -> Self {
        self.domain = Some(domain.into());
        self
    }

    /// Set the executor
    pub fn with_executor(mut self, executor: impl Into<String>) -> Self {
        self.executor = Some(executor.into());
        self
    }

    /// Set the start time
    pub fn with_started_at(mut self, started_at: DateTime<Utc>) -> Self {
        self.started_at = Some(started_at);
        self
    }

    /// Add an environment variable
    pub fn with_env(mut self, key: impl Into<String>, value: Value) -> Self {
        self.environment.insert(key.into(), value);
        self
    }

    /// Add multiple environment variables
    pub fn with_env_map(mut self, env_map: HashMap<String, Value>) -> Self {
        self.environment.extend(env_map);
        self
    }

    /// Add an available integration
    pub fn with_integration(mut self, integration_name: impl Into<String>) -> Self {
        self.integrations.push(integration_name.into());
        self
    }

    /// Add multiple integrations
    pub fn with_integrations(mut self, integrations: Vec<String>) -> Self {
        self.integrations.extend(integrations);
        self
    }

    /// Enable compliance mode with required environment flags
    pub fn with_compliance_mode(mut self, compliance_types: &[ComplianceType]) -> Self {
        for compliance_type in compliance_types {
            match compliance_type {
                ComplianceType::PII => {
                    self.environment
                        .insert("pii_compliant".to_string(), Value::Bool(true));
                }
                ComplianceType::PCI => {
                    self.environment
                        .insert("pci_compliant".to_string(), Value::Bool(true));
                }
                ComplianceType::PHI => {
                    self.environment
                        .insert("hipaa_compliant".to_string(), Value::Bool(true));
                }
                ComplianceType::SOX => {
                    self.environment
                        .insert("sox_compliant".to_string(), Value::Bool(true));
                }
                ComplianceType::GDPR => {
                    self.environment
                        .insert("gdpr_compliant".to_string(), Value::Bool(true));
                }
            }
        }
        self
    }

    /// Build the execution context
    pub fn build(self) -> Result<ExecutionContext> {
        let session_id = self.session_id.unwrap_or_else(Uuid::new_v4);
        let business_unit_id = self
            .business_unit_id
            .ok_or_else(|| anyhow::anyhow!("business_unit_id is required"))?;
        let domain = self
            .domain
            .ok_or_else(|| anyhow::anyhow!("domain is required"))?;
        let executor = self
            .executor
            .ok_or_else(|| anyhow::anyhow!("executor is required"))?;
        let started_at = self.started_at.unwrap_or_else(Utc::now);

        Ok(ExecutionContext {
            session_id,
            business_unit_id,
            domain,
            executor,
            started_at,
            environment: self.environment,
            integrations: self.integrations,
        })
    }
}

impl Default for ExecutionContextBuilder {
    fn default() -> Self {
        Self::new()
    }
}

/// Supported compliance types
#[derive(Debug, Clone, Serialize, Deserialize)]
pub enum ComplianceType {
    PII,  // Personally Identifiable Information
    PCI,  // Payment Card Industry
    PHI,  // Protected Health Information
    SOX,  // Sarbanes-Oxley
    GDPR, // General Data Protection Regulation
}

/// Environment validator for execution contexts
pub struct EnvironmentValidator;

impl EnvironmentValidator {
    /// Validate that required environment variables are present
    pub fn validate_required_env(context: &ExecutionContext, required_vars: &[&str]) -> Result<()> {
        let missing_vars: Vec<&str> = required_vars
            .iter()
            .filter(|&var| !context.environment.contains_key(*var))
            .copied()
            .collect();

        if !missing_vars.is_empty() {
            return Err(anyhow::anyhow!(
                "Missing required environment variables: {}",
                missing_vars.join(", ")
            ));
        }

        Ok(())
    }

    /// Validate compliance environment settings
    pub fn validate_compliance_env(
        context: &ExecutionContext,
        required_compliance: &[ComplianceType],
    ) -> Result<()> {
        let mut missing_compliance = Vec::new();

        for compliance_type in required_compliance {
            let env_key = match compliance_type {
                ComplianceType::PII => "pii_compliant",
                ComplianceType::PCI => "pci_compliant",
                ComplianceType::PHI => "hipaa_compliant",
                ComplianceType::SOX => "sox_compliant",
                ComplianceType::GDPR => "gdpr_compliant",
            };

            if !context
                .environment
                .get(env_key)
                .and_then(|v| v.as_bool())
                .unwrap_or(false)
            {
                missing_compliance.push(compliance_type);
            }
        }

        if !missing_compliance.is_empty() {
            return Err(anyhow::anyhow!(
                "Missing compliance environment flags: {:?}",
                missing_compliance
            ));
        }

        Ok(())
    }

    /// Validate that required integrations are available
    pub fn validate_integrations(
        context: &ExecutionContext,
        required_integrations: &[&str],
    ) -> Result<()> {
        let missing_integrations: Vec<&str> = required_integrations
            .iter()
            .filter(|&integration| !context.integrations.contains(&integration.to_string()))
            .copied()
            .collect();

        if !missing_integrations.is_empty() {
            return Err(anyhow::anyhow!(
                "Missing required integrations: {}",
                missing_integrations.join(", ")
            ));
        }

        Ok(())
    }
}

/// Session management utilities
pub struct SessionManager;

impl SessionManager {
    /// Create a new session context for a business unit
    pub fn create_session(
        business_unit_id: impl Into<String>,
        domain: impl Into<String>,
        executor: impl Into<String>,
    ) -> Result<ExecutionContext> {
        ExecutionContextBuilder::new()
            .with_business_unit_id(business_unit_id)
            .with_domain(domain)
            .with_executor(executor)
            .with_started_at(Utc::now())
            .build()
    }

    /// Create a KYC session with appropriate compliance settings
    pub fn create_kyc_session(
        business_unit_id: impl Into<String>,
        executor: impl Into<String>,
        integrations: Vec<String>,
    ) -> Result<ExecutionContext> {
        ExecutionContextBuilder::new()
            .with_business_unit_id(business_unit_id)
            .with_domain("kyc")
            .with_executor(executor)
            .with_compliance_mode(&[ComplianceType::PII, ComplianceType::GDPR])
            .with_integrations(integrations)
            .with_env("kyc_mode", Value::Bool(true))
            .with_env("audit_required", Value::Bool(true))
            .build()
    }

    /// Create an onboarding session
    pub fn create_onboarding_session(
        business_unit_id: impl Into<String>,
        executor: impl Into<String>,
        integrations: Vec<String>,
    ) -> Result<ExecutionContext> {
        ExecutionContextBuilder::new()
            .with_business_unit_id(business_unit_id)
            .with_domain("onboarding")
            .with_executor(executor)
            .with_compliance_mode(&[ComplianceType::PII, ComplianceType::SOX])
            .with_integrations(integrations)
            .with_env("onboarding_mode", Value::Bool(true))
            .with_env("workflow_validation", Value::Bool(true))
            .build()
    }

    /// Create a UBO discovery session
    pub fn create_ubo_session(
        business_unit_id: impl Into<String>,
        executor: impl Into<String>,
        integrations: Vec<String>,
    ) -> Result<ExecutionContext> {
        ExecutionContextBuilder::new()
            .with_business_unit_id(business_unit_id)
            .with_domain("ubo")
            .with_executor(executor)
            .with_compliance_mode(&[
                ComplianceType::PII,
                ComplianceType::GDPR,
                ComplianceType::SOX,
            ])
            .with_integrations(integrations)
            .with_env("ubo_discovery_mode", Value::Bool(true))
            .with_env("ownership_validation", Value::Bool(true))
            .with_env("document_evidence_required", Value::Bool(true))
            .build()
    }

    /// Extend an existing session with additional environment variables
    pub fn extend_session(
        mut context: ExecutionContext,
        additional_env: HashMap<String, Value>,
        additional_integrations: Vec<String>,
    ) -> ExecutionContext {
        context.environment.extend(additional_env);
        context.integrations.extend(additional_integrations);
        context
    }
}

/// Context utilities for common operations
pub struct ContextUtils;

impl ContextUtils {
    /// Check if context is in a specific mode
    pub fn is_mode_enabled(context: &ExecutionContext, mode: &str) -> bool {
        context
            .environment
            .get(mode)
            .and_then(|v| v.as_bool())
            .unwrap_or(false)
    }

    /// Get environment variable as string
    pub fn get_env_string(context: &ExecutionContext, key: &str) -> Option<String> {
        context
            .environment
            .get(key)
            .and_then(|v| v.as_str())
            .map(|s| s.to_string())
    }

    /// Get environment variable as boolean
    pub fn get_env_bool(context: &ExecutionContext, key: &str) -> Option<bool> {
        context.environment.get(key).and_then(|v| v.as_bool())
    }

    /// Get environment variable as number
    pub fn get_env_number(context: &ExecutionContext, key: &str) -> Option<f64> {
        context.environment.get(key).and_then(|v| v.as_f64())
    }

    /// Check if integration is available
    pub fn has_integration(context: &ExecutionContext, integration_name: &str) -> bool {
        context.integrations.contains(&integration_name.to_string())
    }

    /// Get session duration
    pub fn session_duration(context: &ExecutionContext) -> chrono::Duration {
        Utc::now() - context.started_at
    }

    /// Create a child context for sub-operations
    pub fn create_child_context(
        parent: &ExecutionContext,
        sub_operation_type: &str,
    ) -> ExecutionContext {
        let mut child_env = parent.environment.clone();
        child_env.insert(
            "parent_session_id".to_string(),
            Value::String(parent.session_id.to_string()),
        );
        child_env.insert(
            "sub_operation_type".to_string(),
            Value::String(sub_operation_type.to_string()),
        );

        ExecutionContext {
            session_id: Uuid::new_v4(),
            business_unit_id: parent.business_unit_id.clone(),
            domain: parent.domain.clone(),
            executor: parent.executor.clone(),
            started_at: Utc::now(),
            environment: child_env,
            integrations: parent.integrations.clone(),
        }
    }

    /// Serialize context for logging/audit
    pub fn serialize_for_audit(context: &ExecutionContext) -> Result<String> {
        let audit_data = serde_json::json!({
            "session_id": context.session_id,
            "business_unit_id": context.business_unit_id,
            "domain": context.domain,
            "executor": context.executor,
            "started_at": context.started_at,
            "integrations": context.integrations,
            "environment_keys": context.environment.keys().collect::<Vec<_>>()
        });

        serde_json::to_string_pretty(&audit_data)
            .context("Failed to serialize execution context for audit")
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_execution_context_builder() {
        let context = ExecutionContextBuilder::new()
            .with_business_unit_id("TEST-001")
            .with_domain("test")
            .with_executor("test_user")
            .with_env("test_key", Value::String("test_value".to_string()))
            .with_integration("test_integration")
            .build();

        assert!(context.is_ok());
        let context = context.unwrap();
        assert_eq!(context.business_unit_id, "TEST-001");
        assert_eq!(context.domain, "test");
        assert_eq!(context.executor, "test_user");
        assert!(context.environment.contains_key("test_key"));
        assert!(context
            .integrations
            .contains(&"test_integration".to_string()));
    }

    #[test]
    fn test_compliance_mode() {
        let context = ExecutionContextBuilder::new()
            .with_business_unit_id("TEST-001")
            .with_domain("kyc")
            .with_executor("test_user")
            .with_compliance_mode(&[ComplianceType::PII, ComplianceType::GDPR])
            .build()
            .unwrap();

        assert!(ContextUtils::get_env_bool(&context, "pii_compliant").unwrap_or(false));
        assert!(ContextUtils::get_env_bool(&context, "gdpr_compliant").unwrap_or(false));
        assert!(!ContextUtils::get_env_bool(&context, "pci_compliant").unwrap_or(false));
    }

    #[test]
    fn test_session_manager() {
        let kyc_session = SessionManager::create_kyc_session(
            "KYC-001",
            "kyc_analyst",
            vec!["risk_engine".to_string(), "document_store".to_string()],
        );

        assert!(kyc_session.is_ok());
        let session = kyc_session.unwrap();
        assert_eq!(session.domain, "kyc");
        assert!(ContextUtils::is_mode_enabled(&session, "kyc_mode"));
        assert!(ContextUtils::has_integration(&session, "risk_engine"));
    }

    #[test]
    fn test_environment_validator() {
        let context = ExecutionContextBuilder::new()
            .with_business_unit_id("TEST-001")
            .with_domain("test")
            .with_executor("test_user")
            .with_env("required_var", Value::String("present".to_string()))
            .with_compliance_mode(&[ComplianceType::PII])
            .with_integration("test_integration")
            .build()
            .unwrap();

        // Test required environment validation
        let result = EnvironmentValidator::validate_required_env(&context, &["required_var"]);
        assert!(result.is_ok());

        let result = EnvironmentValidator::validate_required_env(&context, &["missing_var"]);
        assert!(result.is_err());

        // Test compliance validation
        let result =
            EnvironmentValidator::validate_compliance_env(&context, &[ComplianceType::PII]);
        assert!(result.is_ok());

        let result =
            EnvironmentValidator::validate_compliance_env(&context, &[ComplianceType::PCI]);
        assert!(result.is_err());

        // Test integration validation
        let result = EnvironmentValidator::validate_integrations(&context, &["test_integration"]);
        assert!(result.is_ok());

        let result =
            EnvironmentValidator::validate_integrations(&context, &["missing_integration"]);
        assert!(result.is_err());
    }

    #[test]
    fn test_context_utils() {
        let context = ExecutionContextBuilder::new()
            .with_business_unit_id("TEST-001")
            .with_domain("test")
            .with_executor("test_user")
            .with_env("string_var", Value::String("test".to_string()))
            .with_env("bool_var", Value::Bool(true))
            .with_env("number_var", Value::Number(serde_json::Number::from(42)))
            .with_integration("test_integration")
            .build()
            .unwrap();

        assert_eq!(
            ContextUtils::get_env_string(&context, "string_var"),
            Some("test".to_string())
        );
        assert_eq!(ContextUtils::get_env_bool(&context, "bool_var"), Some(true));
        assert_eq!(
            ContextUtils::get_env_number(&context, "number_var"),
            Some(42.0)
        );
        assert!(ContextUtils::has_integration(&context, "test_integration"));
        assert!(!ContextUtils::has_integration(
            &context,
            "missing_integration"
        ));

        let child_context = ContextUtils::create_child_context(&context, "sub_operation");
        assert_ne!(child_context.session_id, context.session_id);
        assert_eq!(child_context.business_unit_id, context.business_unit_id);
        assert!(child_context.environment.contains_key("parent_session_id"));
    }

    #[test]
    fn test_context_serialization() {
        let context = ExecutionContextBuilder::new()
            .with_business_unit_id("TEST-001")
            .with_domain("test")
            .with_executor("test_user")
            .build()
            .unwrap();

        let serialized = ContextUtils::serialize_for_audit(&context);
        assert!(serialized.is_ok());

        let audit_str = serialized.unwrap();
        assert!(audit_str.contains("TEST-001"));
        assert!(audit_str.contains("test"));
    }
}
