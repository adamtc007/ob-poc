//! DSL Execution Engine
//!
//! This module implements the core execution engine that transforms parsed DSL operations
//! into actual business logic execution. It follows the DSL-as-State pattern where
//! accumulated DSL documents represent the current state of business processes.

use anyhow::{Context, Result};
use serde::{Deserialize, Serialize};
use std::collections::HashMap;
use std::sync::Arc;
use tokio::sync::RwLock;
use uuid::Uuid;

pub mod context;
pub mod engine;
pub mod integrations;
pub mod operations;
pub mod rules;
pub mod state;

use crate::data_dictionary::AttributeId;
use crate::dsl::operations::ExecutableDslOperation as DslOperation;

/// Represents the execution context for DSL operations
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ExecutionContext {
    /// Unique identifier for this execution session
    pub session_id: Uuid,
    /// The business unit identifier (CBU, case ID, etc.)
    pub business_unit_id: String,
    /// Current domain context (KYC, Onboarding, etc.)
    pub domain: String,
    /// User or system executing the operations
    pub executor: String,
    /// Timestamp of execution start
    pub started_at: chrono::DateTime<chrono::Utc>,
    /// Environment variables and configuration
    pub environment: HashMap<String, serde_json::Value>,
    /// External system integrations available
    pub integrations: Vec<String>,
}

/// Represents the accumulated state from DSL operations
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct DslState {
    /// The business unit this state belongs to
    pub business_unit_id: String,
    /// Complete history of DSL operations (immutable event sourcing)
    pub operations: Vec<DslOperation>,
    /// Current computed state derived from operations
    pub current_state: HashMap<AttributeId, serde_json::Value>,
    /// Metadata about the state
    pub metadata: StateMetadata,
    /// Version of this state (increments with each operation)
    pub version: u64,
}

/// Metadata about the DSL state
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct StateMetadata {
    /// When this state was created
    pub created_at: chrono::DateTime<chrono::Utc>,
    /// When this state was last updated
    pub updated_at: chrono::DateTime<chrono::Utc>,
    /// Domain this state belongs to
    pub domain: String,
    /// Current workflow status
    pub status: String,
    /// Tags for categorization
    pub tags: Vec<String>,
    /// Compliance and audit flags
    pub compliance_flags: Vec<String>,
}

/// Result of executing a DSL operation
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ExecutionResult {
    /// Whether the execution was successful
    pub success: bool,
    /// The operation that was executed
    pub operation: DslOperation,
    /// New state after execution
    pub new_state: DslState,
    /// Messages, warnings, or errors from execution
    pub messages: Vec<ExecutionMessage>,
    /// External system responses
    pub external_responses: HashMap<String, serde_json::Value>,
    /// Execution duration in milliseconds
    pub duration_ms: u64,
}

/// Messages generated during execution
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ExecutionMessage {
    pub level: MessageLevel,
    pub message: String,
    pub context: Option<String>,
    pub timestamp: chrono::DateTime<chrono::Utc>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub enum MessageLevel {
    Debug,
    Info,
    Warning,
    Error,
    Critical,
}

/// Trait for external system integrations
#[async_trait::async_trait]
pub trait ExternalIntegration: Send + Sync {
    fn name(&self) -> &str;
    async fn execute(
        &self,
        operation: &DslOperation,
        context: &ExecutionContext,
    ) -> Result<serde_json::Value>;
    async fn validate(&self, operation: &DslOperation) -> Result<bool>;
}

/// Trait for business rule validation
#[async_trait::async_trait]
pub trait BusinessRule: Send + Sync {
    fn name(&self) -> &str;
    fn applies_to(&self, operation: &DslOperation) -> bool;
    async fn validate(
        &self,
        operation: &DslOperation,
        state: &DslState,
        context: &ExecutionContext,
    ) -> Result<RuleResult>;
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct RuleResult {
    pub valid: bool,
    pub message: Option<String>,
    pub blocking: bool, // If true, execution cannot continue
    pub suggestions: Vec<String>,
}

/// Main DSL execution engine
pub struct DslExecutionEngine {
    /// External system integrations
    integrations: Arc<RwLock<HashMap<String, Arc<dyn ExternalIntegration>>>>,
    /// Business rules
    rules: Arc<RwLock<Vec<Arc<dyn BusinessRule>>>>,
    /// Operation handlers
    operation_handlers: Arc<RwLock<HashMap<String, Arc<dyn OperationHandler>>>>,
    /// State storage
    state_store: Arc<dyn StateStore>,
}

/// Trait for handling specific DSL operations
#[async_trait::async_trait]
pub trait OperationHandler: Send + Sync {
    fn handles(&self) -> &str; // Operation type this handler supports
    async fn execute(
        &self,
        operation: &DslOperation,
        state: &DslState,
        context: &ExecutionContext,
    ) -> Result<ExecutionResult>;
    async fn validate(
        &self,
        operation: &DslOperation,
        state: &DslState,
    ) -> Result<Vec<ExecutionMessage>>;
}

/// Trait for persisting and retrieving DSL state
#[async_trait::async_trait]
pub trait StateStore: Send + Sync {
    async fn get_state(&self, business_unit_id: &str) -> Result<Option<DslState>>;
    async fn save_state(&self, state: &DslState) -> Result<()>;
    async fn get_state_history(
        &self,
        business_unit_id: &str,
        limit: Option<u32>,
    ) -> Result<Vec<DslState>>;
    async fn create_snapshot(&self, business_unit_id: &str) -> Result<Uuid>;
    async fn restore_from_snapshot(&self, snapshot_id: Uuid) -> Result<DslState>;
}

impl DslExecutionEngine {
    /// Create a new execution engine
    pub fn new(state_store: Arc<dyn StateStore>) -> Self {
        Self {
            integrations: Arc::new(RwLock::new(HashMap::new())),
            rules: Arc::new(RwLock::new(Vec::new())),
            operation_handlers: Arc::new(RwLock::new(HashMap::new())),
            state_store,
        }
    }

    /// Register an external integration
    pub async fn register_integration(&self, integration: Arc<dyn ExternalIntegration>) {
        let mut integrations = self.integrations.write().await;
        integrations.insert(integration.name().to_string(), integration);
    }

    /// Register a business rule
    pub async fn register_rule(&self, rule: Arc<dyn BusinessRule>) {
        let mut rules = self.rules.write().await;
        rules.push(rule);
    }

    /// Register an operation handler
    pub async fn register_handler(&self, handler: Arc<dyn OperationHandler>) {
        let mut handlers = self.operation_handlers.write().await;
        handlers.insert(handler.handles().to_string(), handler);
    }

    /// Execute a single DSL operation
    pub async fn execute_operation(
        &self,
        operation: DslOperation,
        context: ExecutionContext,
    ) -> Result<ExecutionResult> {
        let start_time = std::time::Instant::now();

        // Get current state
        let current_state = self
            .get_or_create_state(&context.business_unit_id, &context.domain)
            .await?;

        // Validate business rules
        self.validate_rules(&operation, &current_state, &context)
            .await?;

        // Find and execute operation handler
        let result = self
            .execute_with_handler(&operation, &current_state, &context)
            .await?;

        // Save new state
        self.state_store
            .save_state(&result.new_state)
            .await
            .context("Failed to save new state after operation execution")?;

        let duration = start_time.elapsed().as_millis() as u64;

        Ok(ExecutionResult {
            success: true,
            operation,
            new_state: result.new_state,
            messages: result.messages,
            external_responses: result.external_responses,
            duration_ms: duration,
        })
    }

    /// Execute a sequence of DSL operations (batch)
    pub async fn execute_batch(
        &self,
        operations: Vec<DslOperation>,
        context: ExecutionContext,
    ) -> Result<Vec<ExecutionResult>> {
        let mut results = Vec::new();
        let mut current_context = context;

        for operation in operations {
            match self
                .execute_operation(operation, current_context.clone())
                .await
            {
                Ok(result) => {
                    // Update context with new state for next operation
                    current_context.session_id = Uuid::new_v4(); // New session for each op in batch
                    results.push(result);
                }
                Err(e) => {
                    // On error, return partial results with error info
                    return Err(e.context(format!(
                        "Batch execution failed at operation {}",
                        results.len()
                    )));
                }
            }
        }

        Ok(results)
    }

    /// Get current state for a business unit
    pub async fn get_current_state(&self, business_unit_id: &str) -> Result<Option<DslState>> {
        self.state_store.get_state(business_unit_id).await
    }

    /// Internal helper to get or create initial state
    async fn get_or_create_state(&self, business_unit_id: &str, domain: &str) -> Result<DslState> {
        match self.state_store.get_state(business_unit_id).await? {
            Some(state) => Ok(state),
            None => {
                // Create initial empty state
                let now = chrono::Utc::now();
                Ok(DslState {
                    business_unit_id: business_unit_id.to_string(),
                    operations: Vec::new(),
                    current_state: HashMap::new(),
                    metadata: StateMetadata {
                        created_at: now,
                        updated_at: now,
                        domain: domain.to_string(),
                        status: "initialized".to_string(),
                        tags: Vec::new(),
                        compliance_flags: Vec::new(),
                    },
                    version: 0,
                })
            }
        }
    }

    /// Validate all applicable business rules
    async fn validate_rules(
        &self,
        operation: &DslOperation,
        state: &DslState,
        context: &ExecutionContext,
    ) -> Result<()> {
        let rules = self.rules.read().await;

        for rule in rules.iter() {
            if rule.applies_to(operation) {
                let result = rule.validate(operation, state, context).await?;
                if !result.valid && result.blocking {
                    return Err(anyhow::anyhow!(
                        "Business rule '{}' blocked operation: {}",
                        rule.name(),
                        result
                            .message
                            .unwrap_or_else(|| "Rule validation failed".to_string())
                    ));
                }
            }
        }

        Ok(())
    }

    /// Execute operation with appropriate handler
    async fn execute_with_handler(
        &self,
        operation: &DslOperation,
        state: &DslState,
        context: &ExecutionContext,
    ) -> Result<ExecutionResult> {
        let handlers = self.operation_handlers.read().await;

        // Find handler for this operation type
        let handler = handlers.get(&operation.operation_type).ok_or_else(|| {
            anyhow::anyhow!(
                "No handler registered for operation type: {}",
                operation.operation_type
            )
        })?;

        handler.execute(operation, state, context).await
    }
}

impl DslState {
    /// Apply a new operation to the state, creating a new version
    pub fn apply_operation(&self, operation: DslOperation) -> Self {
        let mut new_operations = self.operations.clone();
        new_operations.push(operation.clone());

        let mut new_state = self.current_state.clone();

        // Apply operation effects to current state
        // This is a simplified version - real implementation would have
        // complex state transformation logic based on operation type
        match operation.operation_type.as_str() {
            "validate" | "collect" | "set" => {
                if let Some(attribute_id) = operation.parameters.get("attribute_id") {
                    if let Ok(attr_id) = serde_json::from_value::<AttributeId>(attribute_id.clone())
                    {
                        new_state.insert(
                            attr_id,
                            operation
                                .parameters
                                .get("value")
                                .cloned()
                                .unwrap_or_default(),
                        );
                    }
                }
            }
            "create-edge" | "declare-entity" => {
                // Graph operations would be handled differently
                // For now, just store the operation data
                let key = AttributeId::new(); // Generate new ID for graph elements
                new_state.insert(
                    key,
                    serde_json::to_value(&operation.parameters).unwrap_or_default(),
                );
            }
            _ => {
                // Unknown operation types are stored as-is for audit purposes
            }
        }

        Self {
            business_unit_id: self.business_unit_id.clone(),
            operations: new_operations,
            current_state: new_state,
            metadata: StateMetadata {
                created_at: self.metadata.created_at,
                updated_at: chrono::Utc::now(),
                domain: self.metadata.domain.clone(),
                status: self.metadata.status.clone(),
                tags: self.metadata.tags.clone(),
                compliance_flags: self.metadata.compliance_flags.clone(),
            },
            version: self.version + 1,
        }
    }

    /// Get the accumulated DSL document as a string
    pub fn to_dsl_document(&self) -> String {
        self.operations
            .iter()
            .map(|op| op.to_dsl_string())
            .collect::<Vec<_>>()
            .join("\n\n")
    }

    /// Rebuild state from operations (event sourcing)
    pub fn rebuild_from_operations(
        business_unit_id: String,
        operations: Vec<DslOperation>,
        domain: String,
    ) -> Self {
        let now = chrono::Utc::now();
        let mut state = Self {
            business_unit_id,
            operations: Vec::new(),
            current_state: HashMap::new(),
            metadata: StateMetadata {
                created_at: now,
                updated_at: now,
                domain,
                status: "rebuilding".to_string(),
                tags: Vec::new(),
                compliance_flags: Vec::new(),
            },
            version: 0,
        };

        // Apply each operation in sequence
        for operation in operations {
            state = state.apply_operation(operation);
        }

        // Update final status
        state.metadata.status = "active".to_string();
        state
    }
}

impl ExecutionMessage {
    pub fn info(message: impl Into<String>) -> Self {
        Self {
            level: MessageLevel::Info,
            message: message.into(),
            context: None,
            timestamp: chrono::Utc::now(),
        }
    }

    pub fn warning(message: impl Into<String>) -> Self {
        Self {
            level: MessageLevel::Warning,
            message: message.into(),
            context: None,
            timestamp: chrono::Utc::now(),
        }
    }

    pub fn error(message: impl Into<String>) -> Self {
        Self {
            level: MessageLevel::Error,
            message: message.into(),
            context: None,
            timestamp: chrono::Utc::now(),
        }
    }

    pub fn with_context(mut self, context: impl Into<String>) -> Self {
        self.context = Some(context.into());
        self
    }
}
