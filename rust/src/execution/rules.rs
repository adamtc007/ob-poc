//! Business Rules Engine for DSL Operation Validation
//!
//! This module implements the business rules engine that validates DSL operations
//! against business constraints, compliance requirements, and workflow rules.
//! It follows a plugin architecture where rules can be registered dynamically.

use anyhow::Result;
use async_trait::async_trait;
use std::sync::Arc;

use super::{BusinessRule, DslState, ExecutionContext, RuleResult};
use crate::data_dictionary::AttributeId;
use crate::dsl::operations::ExecutableDslOperation as DslOperation;

/// Registry for managing business rules
pub struct BusinessRuleRegistry {
    rules: Vec<Arc<dyn BusinessRule>>,
}

impl BusinessRuleRegistry {
    pub fn new() -> Self {
        Self { rules: Vec::new() }
    }

    pub fn register(&mut self, rule: Arc<dyn BusinessRule>) {
        self.rules.push(rule);
    }

    pub async fn validate_operation(
        &self,
        operation: &DslOperation,
        state: &DslState,
        context: &ExecutionContext,
    ) -> Result<Vec<RuleResult>> {
        let mut results = Vec::new();

        for rule in &self.rules {
            if rule.applies_to(operation) {
                let result = rule.validate(operation, state, context).await?;
                results.push(result);
            }
        }

        Ok(results)
    }

    pub(crate) fn get_blocking_violations<'a>(
        &self,
        results: &'a [RuleResult],
    ) -> Vec<&'a RuleResult> {
        results.iter().filter(|r| !r.valid && r.blocking).collect()
    }
}

impl Default for BusinessRuleRegistry {
    fn default() -> Self {
        Self::new()
    }
}

/// Rule that validates required attributes are present before certain operations
pub(crate) struct RequiredAttributesRule {
    name: String,
    operation_types: Vec<String>,
    required_attributes: Vec<AttributeId>,
}

impl RequiredAttributesRule {
    pub fn new(
        name: impl Into<String>,
        operation_types: Vec<String>,
        required_attributes: Vec<AttributeId>,
    ) -> Self {
        Self {
            name: name.into(),
            operation_types,
            required_attributes,
        }
    }
}

#[async_trait]
impl BusinessRule for RequiredAttributesRule {
    fn name(&self) -> &str {
        &self.name
    }

    fn applies_to(&self, operation: &DslOperation) -> bool {
        self.operation_types
            .iter()
            .any(|t| t == operation.operation_type.as_str())
    }

    async fn validate(
        &self,
        _operation: &DslOperation,
        state: &DslState,
        _context: &ExecutionContext,
    ) -> Result<RuleResult> {
        let mut missing_attributes = Vec::new();

        for required_attr in &self.required_attributes {
            if !state.current_state.contains_key(required_attr) {
                missing_attributes.push(required_attr.to_string());
            }
        }

        if missing_attributes.is_empty() {
            Ok(RuleResult {
                valid: true,
                message: Some("All required attributes present".to_string()),
                blocking: false,
                suggestions: vec![],
            })
        } else {
            Ok(RuleResult {
                valid: false,
                message: Some(format!(
                    "Missing required attributes: {}",
                    missing_attributes.join(", ")
                )),
                blocking: true,
                suggestions: vec![format!(
                    "Collect or validate the following attributes first: {}",
                    missing_attributes.join(", ")
                )],
            })
        }
    }
}

/// Rule that validates ownership percentages don't exceed 100%
pub(crate) struct OwnershipLimitsRule {
    name: String,
}

impl OwnershipLimitsRule {
    pub fn new() -> Self {
        Self {
            name: "ownership_limits".to_string(),
        }
    }
}

impl Default for OwnershipLimitsRule {
    fn default() -> Self {
        Self::new()
    }
}

#[async_trait]
impl BusinessRule for OwnershipLimitsRule {
    fn name(&self) -> &str {
        &self.name
    }

    fn applies_to(&self, operation: &DslOperation) -> bool {
        operation.operation_type == "create-edge"
            && operation
                .parameters
                .get("type")
                .and_then(|v| v.as_str())
                .is_some_and(|t| t.contains("OWNERSHIP"))
    }

    async fn validate(
        &self,
        operation: &DslOperation,
        state: &DslState,
        _context: &ExecutionContext,
    ) -> Result<RuleResult> {
        // Extract the target entity and new ownership percentage
        let to_entity = operation
            .parameters
            .get("to")
            .and_then(|v| v.as_str())
            .ok_or_else(|| anyhow::anyhow!("Missing 'to' parameter in ownership edge"))?;

        let new_percentage = operation
            .parameters
            .get("properties")
            .and_then(|props| props.get("percent"))
            .and_then(|p| p.as_f64())
            .unwrap_or(0.0);

        // Calculate existing ownership for this entity
        let existing_ownership = self.calculate_existing_ownership(to_entity, state).await?;
        let total_ownership = existing_ownership + new_percentage;

        if total_ownership > 100.0 {
            Ok(RuleResult {
                valid: false,
                message: Some(format!(
                    "Ownership would exceed 100% for entity {}: existing {}% + new {}% = {}%",
                    to_entity, existing_ownership, new_percentage, total_ownership
                )),
                blocking: true,
                suggestions: vec![format!(
                    "Reduce ownership percentage to maximum {}%",
                    100.0 - existing_ownership
                )],
            })
        } else if total_ownership < 0.0 {
            Ok(RuleResult {
                valid: false,
                message: Some("Ownership percentage cannot be negative".to_string()),
                blocking: true,
                suggestions: vec!["Use positive ownership percentage".to_string()],
            })
        } else {
            Ok(RuleResult {
                valid: true,
                message: Some(format!(
                    "Ownership validation passed: {}% total for entity {}",
                    total_ownership, to_entity
                )),
                blocking: false,
                suggestions: vec![],
            })
        }
    }
}

impl OwnershipLimitsRule {
    async fn calculate_existing_ownership(&self, entity: &str, state: &DslState) -> Result<f64> {
        let mut total_ownership = 0.0;

        // Look through all operations to find existing ownership edges to this entity
        for operation in &state.operations {
            if operation.operation_type.matches_str("create-edge") {
                if let Some(to) = operation.parameters.get("to").and_then(|v| v.as_str()) {
                    if to == entity {
                        if let Some(props) = operation.parameters.get("properties") {
                            if let Some(percent) = props.get("percent").and_then(|p| p.as_f64()) {
                                total_ownership += percent;
                            }
                        }
                    }
                }
            }
        }

        Ok(total_ownership)
    }
}

/// Rule that enforces compliance workflow sequences
pub(crate) struct ComplianceWorkflowRule {
    name: String,
    required_sequence: Vec<String>,
}

impl ComplianceWorkflowRule {
    pub fn new(name: impl Into<String>, required_sequence: Vec<String>) -> Self {
        Self {
            name: name.into(),
            required_sequence,
        }
    }

    pub(crate) fn kyc_workflow() -> Self {
        Self::new(
            "kyc_compliance_workflow",
            vec![
                "declare-entity".to_string(),
                "validate".to_string(),
                "collect".to_string(),
                "check".to_string(),
            ],
        )
    }

    pub(crate) fn onboarding_workflow() -> Self {
        Self::new(
            "onboarding_workflow",
            vec![
                "case.create".to_string(),
                "kyc.start".to_string(),
                "products.add".to_string(),
                "services.plan".to_string(),
            ],
        )
    }
}

#[async_trait]
impl BusinessRule for ComplianceWorkflowRule {
    fn name(&self) -> &str {
        &self.name
    }

    fn applies_to(&self, operation: &DslOperation) -> bool {
        self.required_sequence
            .iter()
            .any(|t| t == operation.operation_type.as_str())
    }

    async fn validate(
        &self,
        operation: &DslOperation,
        state: &DslState,
        _context: &ExecutionContext,
    ) -> Result<RuleResult> {
        let current_operation_index = self
            .required_sequence
            .iter()
            .position(|op| op == operation.operation_type.as_str());

        if let Some(current_index) = current_operation_index {
            // Check if all previous steps in the sequence have been completed
            let mut missing_steps = Vec::new();

            for (i, required_op) in self.required_sequence.iter().enumerate() {
                if i < current_index {
                    let step_completed = state
                        .operations
                        .iter()
                        .any(|op| op.operation_type.as_str() == required_op);

                    if !step_completed {
                        missing_steps.push(required_op.clone());
                    }
                }
            }

            if missing_steps.is_empty() {
                Ok(RuleResult {
                    valid: true,
                    message: Some(format!(
                        "Workflow sequence validation passed for operation: {}",
                        operation.operation_type
                    )),
                    blocking: false,
                    suggestions: vec![],
                })
            } else {
                Ok(RuleResult {
                    valid: false,
                    message: Some(format!(
                        "Workflow sequence violation: missing prerequisite steps: {}",
                        missing_steps.join(", ")
                    )),
                    blocking: true,
                    suggestions: vec![format!(
                        "Complete the following operations first: {}",
                        missing_steps.join(", ")
                    )],
                })
            }
        } else {
            // Operation not in sequence, allow it
            Ok(RuleResult {
                valid: true,
                message: Some("Operation not part of workflow sequence".to_string()),
                blocking: false,
                suggestions: vec![],
            })
        }
    }
}

/// Rule that validates data privacy and compliance classifications
pub(crate) struct DataPrivacyRule {
    name: String,
    pii_attributes: Vec<AttributeId>,
    pci_attributes: Vec<AttributeId>,
    phi_attributes: Vec<AttributeId>,
}

impl DataPrivacyRule {
    pub fn new(
        name: impl Into<String>,
        pii_attributes: Vec<AttributeId>,
        pci_attributes: Vec<AttributeId>,
        phi_attributes: Vec<AttributeId>,
    ) -> Self {
        Self {
            name: name.into(),
            pii_attributes,
            pci_attributes,
            phi_attributes,
        }
    }

    pub(crate) fn default_privacy_rule() -> Self {
        Self::new(
            "data_privacy_compliance",
            vec![], // PII attributes would be loaded from data dictionary
            vec![], // PCI attributes would be loaded from data dictionary
            vec![], // PHI attributes would be loaded from data dictionary
        )
    }
}

#[async_trait]
impl BusinessRule for DataPrivacyRule {
    fn name(&self) -> &str {
        &self.name
    }

    fn applies_to(&self, operation: &DslOperation) -> bool {
        matches!(
            operation.operation_type.as_str(),
            "validate" | "collect" | "set"
        )
    }

    async fn validate(
        &self,
        operation: &DslOperation,
        _state: &DslState,
        context: &ExecutionContext,
    ) -> Result<RuleResult> {
        if let Some(attribute_id) = operation
            .parameters
            .get("attribute_id")
            .and_then(|v| serde_json::from_value::<AttributeId>(v.clone()).ok())
        {
            let mut privacy_violations = Vec::new();
            let mut privacy_level = "PUBLIC";

            // Check privacy classifications
            if self.phi_attributes.contains(&attribute_id) {
                privacy_level = "PHI";
                if !self.validate_phi_handling(context).await? {
                    privacy_violations.push("PHI data requires healthcare compliance context");
                }
            } else if self.pci_attributes.contains(&attribute_id) {
                privacy_level = "PCI";
                if !self.validate_pci_handling(context).await? {
                    privacy_violations.push("PCI data requires payment card compliance context");
                }
            } else if self.pii_attributes.contains(&attribute_id) {
                privacy_level = "PII";
                if !self.validate_pii_handling(context).await? {
                    privacy_violations.push("PII data requires appropriate privacy safeguards");
                }
            }

            if privacy_violations.is_empty() {
                Ok(RuleResult {
                    valid: true,
                    message: Some(format!(
                        "Data privacy validation passed for {} attribute",
                        privacy_level
                    )),
                    blocking: false,
                    suggestions: vec![],
                })
            } else {
                Ok(RuleResult {
                    valid: false,
                    message: Some(format!(
                        "Data privacy violations: {}",
                        privacy_violations.join(", ")
                    )),
                    blocking: true,
                    suggestions: privacy_violations
                        .iter()
                        .map(|v| format!("Address: {}", v))
                        .collect(),
                })
            }
        } else {
            Ok(RuleResult {
                valid: true,
                message: Some("No privacy-sensitive attributes detected".to_string()),
                blocking: false,
                suggestions: vec![],
            })
        }
    }
}

impl DataPrivacyRule {
    async fn validate_pii_handling(&self, context: &ExecutionContext) -> Result<bool> {
        // Check if execution context has appropriate PII handling flags
        Ok(context
            .environment
            .get("pii_compliant")
            .and_then(|v| v.as_bool())
            .unwrap_or(false))
    }

    async fn validate_pci_handling(&self, context: &ExecutionContext) -> Result<bool> {
        // Check if execution context has appropriate PCI compliance flags
        Ok(context
            .environment
            .get("pci_compliant")
            .and_then(|v| v.as_bool())
            .unwrap_or(false))
    }

    async fn validate_phi_handling(&self, context: &ExecutionContext) -> Result<bool> {
        // Check if execution context has appropriate PHI/HIPAA compliance flags
        Ok(context
            .environment
            .get("hipaa_compliant")
            .and_then(|v| v.as_bool())
            .unwrap_or(false))
    }
}

/// Rule that validates document evidence requirements
pub(crate) struct DocumentEvidenceRule {
    name: String,
}

impl DocumentEvidenceRule {
    pub fn new() -> Self {
        Self {
            name: "document_evidence_rule".to_string(),
        }
    }
}

impl Default for DocumentEvidenceRule {
    fn default() -> Self {
        Self::new()
    }
}

#[async_trait]
impl BusinessRule for DocumentEvidenceRule {
    fn name(&self) -> &str {
        &self.name
    }

    fn applies_to(&self, operation: &DslOperation) -> bool {
        operation.operation_type == "create-edge"
            && operation.parameters.contains_key("evidenced-by")
    }

    async fn validate(
        &self,
        operation: &DslOperation,
        state: &DslState,
        _context: &ExecutionContext,
    ) -> Result<RuleResult> {
        if let Some(evidence) = operation.parameters.get("evidenced-by") {
            let evidence_docs = evidence
                .as_array()
                .ok_or_else(|| anyhow::anyhow!("evidenced-by must be an array"))?;

            if evidence_docs.is_empty() {
                return Ok(RuleResult {
                    valid: false,
                    message: Some("No evidence documents provided for relationship".to_string()),
                    blocking: true,
                    suggestions: vec![
                        "Provide at least one evidence document for the relationship".to_string(),
                    ],
                });
            }

            // Validate that evidence documents exist (this would check document store in real implementation)
            let mut missing_docs = Vec::new();
            for doc in evidence_docs {
                if let Some(doc_id) = doc.as_str() {
                    if !self.document_exists(doc_id, state).await? {
                        missing_docs.push(doc_id.to_string());
                    }
                }
            }

            if missing_docs.is_empty() {
                Ok(RuleResult {
                    valid: true,
                    message: Some(format!(
                        "All {} evidence documents validated",
                        evidence_docs.len()
                    )),
                    blocking: false,
                    suggestions: vec![],
                })
            } else {
                Ok(RuleResult {
                    valid: false,
                    message: Some(format!(
                        "Missing evidence documents: {}",
                        missing_docs.join(", ")
                    )),
                    blocking: true,
                    suggestions: vec![format!(
                        "Upload or reference the following documents: {}",
                        missing_docs.join(", ")
                    )],
                })
            }
        } else {
            Ok(RuleResult {
                valid: true,
                message: Some("No evidence requirement for this operation".to_string()),
                blocking: false,
                suggestions: vec![],
            })
        }
    }
}

impl DocumentEvidenceRule {
    async fn document_exists(&self, doc_id: &str, state: &DslState) -> Result<bool> {
        // In a real implementation, this would check the document store
        // For now, we'll check if the document was referenced in previous operations
        for operation in &state.operations {
            if operation.operation_type.matches_str("collect") {
                if let Some(value) = operation.parameters.get("value") {
                    if let Some(obj) = value.as_object() {
                        if let Some(document_id) = obj.get("document_id") {
                            if document_id.as_str() == Some(doc_id) {
                                return Ok(true);
                            }
                        }
                    }
                }
            }
        }

        // For demo purposes, assume documents starting with "doc-" exist
        Ok(doc_id.starts_with("doc-"))
    }
}

/// Factory function to create standard business rules
pub(crate) fn create_standard_rules() -> Vec<Arc<dyn BusinessRule>> {
    vec![
        Arc::new(OwnershipLimitsRule::new()),
        Arc::new(ComplianceWorkflowRule::kyc_workflow()),
        Arc::new(ComplianceWorkflowRule::onboarding_workflow()),
        Arc::new(DataPrivacyRule::default_privacy_rule()),
        Arc::new(DocumentEvidenceRule::new()),
    ]
}
