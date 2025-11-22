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
pub(crate) struct ExecutionContextBuilder {
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
    pub(crate) fn with_session_id(mut self, session_id: Uuid) -> Self {
        self.session_id = Some(session_id);
        self
    }

    /// Set the business unit ID
    pub(crate) fn with_business_unit_id(mut self, business_unit_id: impl Into<String>) -> Self {
        self.business_unit_id = Some(business_unit_id.into());
        self
    }

    /// Set the domain
    pub fn with_domain(mut self, domain: impl Into<String>) -> Self {
        self.domain = Some(domain.into());
        self
    }

    /// Set the executor
    pub(crate) fn with_executor(mut self, executor: impl Into<String>) -> Self {
        self.executor = Some(executor.into());
        self
    }

    /// Set the start time
    pub(crate) fn with_started_at(mut self, started_at: DateTime<Utc>) -> Self {
        self.started_at = Some(started_at);
        self
    }

    /// Add an environment variable
    pub fn with_env(mut self, key: impl Into<String>, value: Value) -> Self {
        self.environment.insert(key.into(), value);
        self
    }

    /// Add multiple environment variables
    pub(crate) fn with_env_map(mut self, env_map: HashMap<String, Value>) -> Self {
        self.environment.extend(env_map);
        self
    }

    /// Add an available integration
    pub(crate) fn with_integration(mut self, integration_name: impl Into<String>) -> Self {
        self.integrations.push(integration_name.into());
        self
    }

    /// Add multiple integrations
    pub(crate) fn with_integrations(mut self, integrations: Vec<String>) -> Self {
        self.integrations.extend(integrations);
        self
    }

    /// Enable compliance mode with required environment flags
    pub(crate) fn with_compliance_mode(mut self, compliance_types: &[ComplianceType]) -> Self {
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
pub(crate) enum ComplianceType {
    PII,  // Personally Identifiable Information
    PCI,  // Payment Card Industry
    PHI,  // Protected Health Information
    SOX,  // Sarbanes-Oxley
    GDPR, // General Data Protection Regulation
}

/// Environment validator for execution contexts
pub(crate) struct EnvironmentValidator;

impl EnvironmentValidator {
    /// Validate that required environment variables are present
    pub(crate) fn validate_required_env(context: &ExecutionContext, required_vars: &[&str]) -> Result<()> {
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
    pub(crate) fn validate_compliance_env(
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
    pub(crate) fn validate_integrations(
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
pub(crate) struct SessionManager;

impl SessionManager {
    /// Create a new session context for a business unit
    pub(crate) fn create_session(
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
    pub(crate) fn create_kyc_session(
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
    pub(crate) fn create_onboarding_session(
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
    pub(crate) fn create_ubo_session(
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
    pub(crate) fn extend_session(
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
pub(crate) struct ContextUtils;

impl ContextUtils {
    /// Check if context is in a specific mode
    pub(crate) fn is_mode_enabled(context: &ExecutionContext, mode: &str) -> bool {
        context
            .environment
            .get(mode)
            .and_then(|v| v.as_bool())
            .unwrap_or(false)
    }

    /// Get environment variable as string
    pub(crate) fn get_env_string(context: &ExecutionContext, key: &str) -> Option<String> {
        context
            .environment
            .get(key)
            .and_then(|v| v.as_str())
            .map(|s| s.to_string())
    }

    /// Get environment variable as boolean
    pub(crate) fn get_env_bool(context: &ExecutionContext, key: &str) -> Option<bool> {
        context.environment.get(key).and_then(|v| v.as_bool())
    }

    /// Get environment variable as number
    pub(crate) fn get_env_number(context: &ExecutionContext, key: &str) -> Option<f64> {
        context.environment.get(key).and_then(|v| v.as_f64())
    }

    /// Check if integration is available
    pub(crate) fn has_integration(context: &ExecutionContext, integration_name: &str) -> bool {
        context.integrations.contains(&integration_name.to_string())
    }

    /// Get session duration
    pub(crate) fn session_duration(context: &ExecutionContext) -> chrono::Duration {
        Utc::now() - context.started_at
    }

    /// Create a child context for sub-operations
    pub(crate) fn create_child_context(
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
    pub(crate) fn serialize_for_audit(context: &ExecutionContext) -> Result<String> {
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

