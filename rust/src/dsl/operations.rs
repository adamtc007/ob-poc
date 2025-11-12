//! DSL Operations System
//!
//! This module defines standard DSL operation types that work across all domains
//! while supporting domain-specific extensions. Operations are the primary way
//! to interact with the central DSL editor and represent business actions
//! that need to be transformed into DSL fragments.
//!
//! ## Operation Types:
//! - Generic operations that work across all domains
//! - Domain-specific operations for specialized business logic
//! - Composite operations that combine multiple atomic operations
//!
//! ## Design Principles:
//! - Operations are domain-agnostic at the interface level
//! - Domain handlers are responsible for transforming operations to DSL
//! - Operations carry metadata for audit trails and validation

use crate::dsl::domain_context::OperationMetadata;
use serde::{Deserialize, Serialize};
use std::collections::HashMap;
use uuid::Uuid;

/// Standard DSL operation types that work across domains
#[derive(Debug, Clone, Serialize, Deserialize)]
pub enum DslOperation {
    /// Create a new entity with specified type and properties
    CreateEntity {
        entity_type: String,
        entity_id: Option<String>,
        properties: HashMap<String, serde_json::Value>,
        metadata: OperationMetadata,
    },

    /// Update an existing entity's properties
    UpdateEntity {
        entity_id: String,
        properties: HashMap<String, serde_json::Value>,
        metadata: OperationMetadata,
    },

    /// Add products to a case or entity
    AddProducts {
        target_id: String,
        products: Vec<String>,
        configuration: HashMap<String, serde_json::Value>,
        metadata: OperationMetadata,
    },

    /// Add services to a case or entity
    AddServices {
        target_id: String,
        services: Vec<ServiceDefinition>,
        metadata: OperationMetadata,
    },

    /// Update a specific attribute value
    UpdateAttribute {
        target_id: String,
        attribute_id: Uuid,
        attribute_name: String,
        value: serde_json::Value,
        validation_required: bool,
        metadata: OperationMetadata,
    },

    /// Transition from one functional state to another
    TransitionState {
        target_id: String,
        from_state: String,
        to_state: String,
        transition_data: HashMap<String, serde_json::Value>,
        metadata: OperationMetadata,
    },

    /// Collect or upload a document
    CollectDocument {
        target_id: String,
        document_type: String,
        document_id: Option<String>,
        requirements: DocumentRequirements,
        metadata: OperationMetadata,
    },

    /// Create a relationship between entities
    CreateRelationship {
        from_entity: String,
        to_entity: String,
        relationship_type: String,
        properties: HashMap<String, serde_json::Value>,
        metadata: OperationMetadata,
    },

    /// Validate data or state against rules
    ValidateData {
        target_id: String,
        validation_type: ValidationType,
        criteria: ValidationCriteria,
        metadata: OperationMetadata,
    },

    /// Execute a workflow step
    ExecuteWorkflowStep {
        target_id: String,
        workflow_id: String,
        step_id: String,
        step_data: HashMap<String, serde_json::Value>,
        metadata: OperationMetadata,
    },

    /// Send notification or communication
    SendNotification {
        target_id: String,
        notification_type: String,
        recipients: Vec<String>,
        content: NotificationContent,
        metadata: OperationMetadata,
    },

    /// Domain-specific operation for specialized business logic
    DomainSpecific {
        operation_type: String,
        target_id: String,
        payload: serde_json::Value,
        metadata: OperationMetadata,
    },

    /// Composite operation that combines multiple atomic operations
    Composite {
        operations: Vec<DslOperation>,
        execution_strategy: ExecutionStrategy,
        metadata: OperationMetadata,
    },
}

impl DslOperation {
    /// Get the operation metadata
    pub fn metadata(&self) -> &OperationMetadata {
        match self {
            DslOperation::CreateEntity { metadata, .. } => metadata,
            DslOperation::UpdateEntity { metadata, .. } => metadata,
            DslOperation::AddProducts { metadata, .. } => metadata,
            DslOperation::AddServices { metadata, .. } => metadata,
            DslOperation::UpdateAttribute { metadata, .. } => metadata,
            DslOperation::TransitionState { metadata, .. } => metadata,
            DslOperation::CollectDocument { metadata, .. } => metadata,
            DslOperation::CreateRelationship { metadata, .. } => metadata,
            DslOperation::ValidateData { metadata, .. } => metadata,
            DslOperation::ExecuteWorkflowStep { metadata, .. } => metadata,
            DslOperation::SendNotification { metadata, .. } => metadata,
            DslOperation::DomainSpecific { metadata, .. } => metadata,
            DslOperation::Composite { metadata, .. } => metadata,
        }
    }

    /// Get the target ID that this operation affects
    pub(crate) fn target_id(&self) -> &str {
        match self {
            DslOperation::CreateEntity { entity_id, .. } => {
                entity_id.as_deref().unwrap_or("unknown")
            }
            DslOperation::UpdateEntity { entity_id, .. } => entity_id,
            DslOperation::AddProducts { target_id, .. } => target_id,
            DslOperation::AddServices { target_id, .. } => target_id,
            DslOperation::UpdateAttribute { target_id, .. } => target_id,
            DslOperation::TransitionState { target_id, .. } => target_id,
            DslOperation::CollectDocument { target_id, .. } => target_id,
            DslOperation::CreateRelationship { from_entity, .. } => from_entity,
            DslOperation::ValidateData { target_id, .. } => target_id,
            DslOperation::ExecuteWorkflowStep { target_id, .. } => target_id,
            DslOperation::SendNotification { target_id, .. } => target_id,
            DslOperation::DomainSpecific { target_id, .. } => target_id,
            DslOperation::Composite { .. } => "composite",
        }
    }

    /// Get a human-readable description of the operation
    pub fn description(&self) -> String {
        match self {
            DslOperation::CreateEntity { entity_type, .. } => {
                format!("Create {} entity", entity_type)
            }
            DslOperation::UpdateEntity { entity_id, .. } => {
                format!("Update entity {}", entity_id)
            }
            DslOperation::AddProducts { products, .. } => {
                format!("Add products: {}", products.join(", "))
            }
            DslOperation::AddServices { services, .. } => {
                let service_names: Vec<String> = services.iter().map(|s| s.name.clone()).collect();
                format!("Add services: {}", service_names.join(", "))
            }
            DslOperation::UpdateAttribute { attribute_name, .. } => {
                format!("Update attribute {}", attribute_name)
            }
            DslOperation::TransitionState {
                from_state,
                to_state,
                ..
            } => {
                format!("Transition from {} to {}", from_state, to_state)
            }
            DslOperation::CollectDocument { document_type, .. } => {
                format!("Collect document: {}", document_type)
            }
            DslOperation::CreateRelationship {
                relationship_type, ..
            } => {
                format!("Create {} relationship", relationship_type)
            }
            DslOperation::ValidateData {
                validation_type, ..
            } => {
                format!("Validate data: {:?}", validation_type)
            }
            DslOperation::ExecuteWorkflowStep { step_id, .. } => {
                format!("Execute workflow step: {}", step_id)
            }
            DslOperation::SendNotification {
                notification_type, ..
            } => {
                format!("Send notification: {}", notification_type)
            }
            DslOperation::DomainSpecific { operation_type, .. } => {
                format!("Domain operation: {}", operation_type)
            }
            DslOperation::Composite { operations, .. } => {
                format!("Composite operation with {} steps", operations.len())
            }
        }
    }

    /// Check if this operation requires specific permissions
    pub(crate) fn required_permissions(&self) -> Vec<String> {
        match self {
            DslOperation::CreateEntity { entity_type, .. } => {
                vec![format!("entity.create.{}", entity_type)]
            }
            DslOperation::UpdateEntity { .. } => vec!["entity.update".to_string()],
            DslOperation::AddProducts { .. } => vec!["products.add".to_string()],
            DslOperation::AddServices { .. } => vec!["services.add".to_string()],
            DslOperation::UpdateAttribute { .. } => vec!["attributes.update".to_string()],
            DslOperation::TransitionState { .. } => vec!["state.transition".to_string()],
            DslOperation::CollectDocument { .. } => vec!["documents.collect".to_string()],
            DslOperation::CreateRelationship { .. } => vec!["relationships.create".to_string()],
            DslOperation::ValidateData { .. } => vec!["data.validate".to_string()],
            DslOperation::ExecuteWorkflowStep { .. } => vec!["workflow.execute".to_string()],
            DslOperation::SendNotification { .. } => vec!["notifications.send".to_string()],
            DslOperation::DomainSpecific { operation_type, .. } => {
                vec![format!("domain.{}", operation_type)]
            }
            DslOperation::Composite { operations, .. } => operations
                .iter()
                .flat_map(|op| op.required_permissions())
                .collect(),
        }
    }
}

/// Service definition for adding services to entities
#[derive(Debug, Clone, Serialize, Deserialize)]
pub(crate) struct ServiceDefinition {
    pub name: String,
    pub service_type: String,
    pub configuration: HashMap<String, serde_json::Value>,
    pub sla_requirements: Option<SlaRequirements>,
}

/// SLA requirements for services
#[derive(Debug, Clone, Serialize, Deserialize)]
pub(crate) struct SlaRequirements {
    pub response_time: Option<String>,
    pub availability: Option<f64>,
    pub throughput: Option<u64>,
    pub custom_requirements: HashMap<String, serde_json::Value>,
}

/// Document requirements for collection operations
#[derive(Debug, Clone, Serialize, Deserialize)]
pub(crate) struct DocumentRequirements {
    pub required: bool,
    pub accepted_formats: Vec<String>,
    pub max_size_mb: Option<u64>,
    pub validation_rules: Vec<String>,
    pub retention_policy: Option<String>,
}

/// Types of validation operations
#[derive(Debug, Clone, Serialize, Deserialize)]
pub(crate) enum ValidationType {
    AttributeValidation,
    StateValidation,
    BusinessRuleValidation,
    ComplianceValidation,
    DataIntegrityValidation,
    CustomValidation(String),
}

/// Criteria for validation operations
#[derive(Debug, Clone, Serialize, Deserialize)]
pub(crate) struct ValidationCriteria {
    pub rules: Vec<String>,
    pub severity: ValidationSeverity,
    pub parameters: HashMap<String, serde_json::Value>,
}

/// Severity levels for validation
#[derive(Debug, Clone, Serialize, Deserialize)]
pub enum ValidationSeverity {
    Info,
    Warning,
    Error,
    Critical,
}

/// Notification content structure
#[derive(Debug, Clone, Serialize, Deserialize)]
pub(crate) struct NotificationContent {
    pub subject: String,
    pub body: String,
    pub template_id: Option<String>,
    pub variables: HashMap<String, serde_json::Value>,
    pub attachments: Vec<String>,
}

/// Execution strategy for composite operations
#[derive(Debug, Clone, Serialize, Deserialize)]
pub(crate) enum ExecutionStrategy {
    Sequential,
    Parallel,
    ConditionalSequential,
    Custom(String),
}

/// Builder for creating DslOperation instances with proper metadata
pub struct OperationBuilder {
    initiated_by: String,
    tags: Vec<String>,
    custom_data: HashMap<String, serde_json::Value>,
}

impl OperationBuilder {
    /// Create a new operation builder
    pub fn new(initiated_by: impl Into<String>) -> Self {
        Self {
            initiated_by: initiated_by.into(),
            tags: Vec::new(),
            custom_data: HashMap::new(),
        }
    }

    /// Add a tag to the operation
    pub(crate) fn with_tag(mut self, tag: impl Into<String>) -> Self {
        self.tags.push(tag.into());
        self
    }

    /// Add custom metadata
    pub(crate) fn with_metadata(
        mut self,
        key: impl Into<String>,
        value: serde_json::Value,
    ) -> Self {
        self.custom_data.insert(key.into(), value);
        self
    }

    /// Build a CreateEntity operation
    pub fn create_entity(
        self,
        entity_type: impl Into<String>,
        properties: HashMap<String, serde_json::Value>,
    ) -> DslOperation {
        DslOperation::CreateEntity {
            entity_type: entity_type.into(),
            entity_id: None,
            properties,
            metadata: self.build_metadata(),
        }
    }

    /// Build an AddProducts operation
    pub(crate) fn add_products(
        self,
        target_id: impl Into<String>,
        products: Vec<String>,
    ) -> DslOperation {
        DslOperation::AddProducts {
            target_id: target_id.into(),
            products,
            configuration: HashMap::new(),
            metadata: self.build_metadata(),
        }
    }

    /// Build a TransitionState operation
    pub fn transition_state(
        self,
        target_id: impl Into<String>,
        from_state: impl Into<String>,
        to_state: impl Into<String>,
    ) -> DslOperation {
        DslOperation::TransitionState {
            target_id: target_id.into(),
            from_state: from_state.into(),
            to_state: to_state.into(),
            transition_data: HashMap::new(),
            metadata: self.build_metadata(),
        }
    }

    /// Build a DomainSpecific operation
    pub fn domain_specific(
        self,
        operation_type: impl Into<String>,
        target_id: impl Into<String>,
        payload: serde_json::Value,
    ) -> DslOperation {
        DslOperation::DomainSpecific {
            operation_type: operation_type.into(),
            target_id: target_id.into(),
            payload,
            metadata: self.build_metadata(),
        }
    }

    /// Build the operation metadata
    fn build_metadata(self) -> OperationMetadata {
        OperationMetadata {
            operation_id: Uuid::new_v4(),
            timestamp: chrono::Utc::now(),
            initiated_by: self.initiated_by,
            priority: crate::dsl::domain_context::OperationPriority::Normal,
            tags: self.tags,
            custom_data: self.custom_data,
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use serde_json::json;

    #[test]
    fn test_operation_builder() {
        let operation = OperationBuilder::new("test_user")
            .with_tag("onboarding")
            .with_tag("corporate")
            .with_metadata("source", json!("api"))
            .create_entity(
                "Company",
                [("name".to_string(), json!("Test Corp"))]
                    .iter()
                    .cloned()
                    .collect(),
            );

        assert_eq!(operation.description(), "Create Company entity");
        assert_eq!(
            operation.required_permissions(),
            vec!["entity.create.Company"]
        );

        let metadata = operation.metadata();
        assert_eq!(metadata.initiated_by, "test_user");
        assert!(metadata.tags.contains(&"onboarding".to_string()));
        assert!(metadata.tags.contains(&"corporate".to_string()));
    }

    #[test]
    fn test_composite_operation() {
        let op1 =
            OperationBuilder::new("user1").add_products("CBU-1234", vec!["CUSTODY".to_string()]);
        let op2 =
            OperationBuilder::new("user1").transition_state("CBU-1234", "CREATE", "PRODUCTS_ADDED");

        let composite = DslOperation::Composite {
            operations: vec![op1, op2],
            execution_strategy: ExecutionStrategy::Sequential,
            metadata: OperationMetadata::default(),
        };

        assert_eq!(composite.description(), "Composite operation with 2 steps");
        let permissions = composite.required_permissions();
        assert!(permissions.contains(&"products.add".to_string()));
        assert!(permissions.contains(&"state.transition".to_string()));
    }

    #[test]
    fn test_operation_metadata_extraction() {
        let operation = OperationBuilder::new("test_user")
            .with_tag("urgent")
            .domain_specific("associate_cbu", "CBU-1234", json!({"cbu": "NEW-CBU"}));

        let metadata = operation.metadata();
        assert_eq!(metadata.initiated_by, "test_user");
        assert!(metadata.tags.contains(&"urgent".to_string()));
        assert_eq!(operation.target_id(), "CBU-1234");
    }
}

/// Simple DSL operation structure for execution engine compatibility
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ExecutableDslOperation {
    pub operation_type: String,
    pub parameters: HashMap<String, serde_json::Value>,
    pub metadata: HashMap<String, serde_json::Value>,
}

impl ExecutableDslOperation {
    /// Create a new executable DSL operation
    pub fn new(
        operation_type: impl Into<String>,
        parameters: HashMap<String, serde_json::Value>,
    ) -> Self {
        Self {
            operation_type: operation_type.into(),
            parameters,
            metadata: HashMap::new(),
        }
    }

    /// Add metadata to the operation
    pub(crate) fn with_metadata(
        mut self,
        key: impl Into<String>,
        value: serde_json::Value,
    ) -> Self {
        self.metadata.insert(key.into(), value);
        self
    }

    /// Convert operation to DSL string representation
    pub(crate) fn to_dsl_string(&self) -> String {
        match self.operation_type.as_str() {
            "validate" => {
                let attr = self
                    .parameters
                    .get("attribute_id")
                    .and_then(|v| v.as_str())
                    .unwrap_or("unknown_attr");
                let value = self
                    .parameters
                    .get("value")
                    .map(|v| format!("{}", v))
                    .unwrap_or_else(|| "null".to_string());
                format!("(validate {} {})", attr, value)
            }
            "collect" => {
                let attr = self
                    .parameters
                    .get("attribute_id")
                    .and_then(|v| v.as_str())
                    .unwrap_or("unknown_attr");
                let from = self
                    .parameters
                    .get("from")
                    .and_then(|v| v.as_str())
                    .unwrap_or("unknown_source");
                format!("(collect {} :from \"{}\")", attr, from)
            }
            "declare-entity" => {
                let node_id = self
                    .parameters
                    .get("node-id")
                    .and_then(|v| v.as_str())
                    .unwrap_or("unknown_node");
                let label = self
                    .parameters
                    .get("label")
                    .and_then(|v| v.as_str())
                    .unwrap_or("Unknown");
                format!(
                    "(declare-entity\n  :node-id \"{}\"\n  :label {})",
                    node_id, label
                )
            }
            "create-edge" => {
                let from = self
                    .parameters
                    .get("from")
                    .and_then(|v| v.as_str())
                    .unwrap_or("unknown_from");
                let to = self
                    .parameters
                    .get("to")
                    .and_then(|v| v.as_str())
                    .unwrap_or("unknown_to");
                let edge_type = self
                    .parameters
                    .get("type")
                    .and_then(|v| v.as_str())
                    .unwrap_or("UNKNOWN_RELATION");
                format!(
                    "(create-edge\n  :from \"{}\"\n  :to \"{}\"\n  :type {})",
                    from, to, edge_type
                )
            }
            "check" => {
                let attr = self
                    .parameters
                    .get("attribute_id")
                    .and_then(|v| v.as_str())
                    .unwrap_or("unknown_attr");
                let condition = self
                    .parameters
                    .get("condition")
                    .and_then(|v| v.as_str())
                    .unwrap_or("exists");
                format!("(check {} :{})", attr, condition)
            }
            _ => {
                format!(
                    "({} {})",
                    self.operation_type,
                    serde_json::to_string(&self.parameters).unwrap_or_else(|_| "{}".to_string())
                )
            }
        }
    }
}

/// Chain of DSL operations that should be executed together
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct OperationChain {
    /// Chain identifier
    pub chain_id: Uuid,
    /// Operations in execution order
    pub operations: Vec<DslOperation>,
    /// Chain metadata
    pub metadata: OperationMetadata,
    /// Whether the chain should be atomic (all-or-nothing)
    pub atomic: bool,
}

impl OperationChain {
    /// Create a new operation chain
    pub fn new() -> Self {
        Self {
            chain_id: Uuid::new_v4(),
            operations: Vec::new(),
            metadata: OperationMetadata::default(),
            atomic: true,
        }
    }

    /// Add an operation to the chain
    pub fn add_operation(&mut self, operation: DslOperation) {
        self.operations.push(operation);
    }

    /// Set whether the chain should be atomic
    pub fn set_atomic(&mut self, atomic: bool) {
        self.atomic = atomic;
    }

    /// Get the number of operations in the chain
    pub fn len(&self) -> usize {
        self.operations.len()
    }

    /// Check if the chain is empty
    pub fn is_empty(&self) -> bool {
        self.operations.is_empty()
    }
}

impl Default for OperationChain {
    fn default() -> Self {
        Self::new()
    }
}
