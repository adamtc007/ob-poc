//! Operation Handlers for Core DSL Operations
//!
//! This module contains concrete implementations of operation handlers for
//! the core DSL operations used across different business domains.

use anyhow::Result;
use async_trait::async_trait;
use serde_json::Value;
use std::collections::HashMap;
use std::sync::Arc;
use uuid::Uuid;

use super::{DslState, ExecutionContext, ExecutionMessage, ExecutionResult, OperationHandler};
use crate::data_dictionary::AttributeId;
use crate::dsl::operations::ExecutableDslOperation as DslOperation;

/// Handler for validation operations (validate)
pub struct ValidateHandler;

#[async_trait]
impl OperationHandler for ValidateHandler {
    fn handles(&self) -> &str {
        "validate"
    }

    async fn execute(
        &self,
        operation: &DslOperation,
        state: &DslState,
        context: &ExecutionContext,
    ) -> Result<ExecutionResult> {
        let mut messages = Vec::new();
        let mut external_responses = HashMap::new();

        // Extract validation parameters
        let attribute_id = operation
            .parameters
            .get("attribute_id")
            .and_then(|v| serde_json::from_value::<AttributeId>(v.clone()).ok())
            .ok_or_else(|| anyhow::anyhow!("Missing or invalid attribute_id for validation"))?;

        let value = operation
            .parameters
            .get("value")
            .ok_or_else(|| anyhow::anyhow!("Missing value for validation"))?;

        // Perform validation logic
        let validation_result = self
            .perform_validation(&attribute_id, value, context)
            .await?;

        if validation_result.is_valid {
            messages.push(ExecutionMessage::info(format!(
                "Validation successful for attribute {}",
                attribute_id
            )));
        } else {
            messages.push(ExecutionMessage::warning(format!(
                "Validation failed for attribute {}: {}",
                attribute_id, validation_result.message
            )));
        }

        // Apply operation to create new state
        let new_state = state.apply_operation(operation.clone());

        // Store validation result in external responses
        external_responses.insert(
            "validation_result".to_string(),
            serde_json::to_value(&validation_result)?,
        );

        Ok(ExecutionResult {
            success: validation_result.is_valid,
            operation: operation.clone(),
            new_state,
            messages,
            external_responses,
            duration_ms: 0, // Will be set by the engine
        })
    }

    async fn validate(
        &self,
        operation: &DslOperation,
        _state: &DslState,
    ) -> Result<Vec<ExecutionMessage>> {
        let mut messages = Vec::new();

        // Check required parameters
        if !operation.parameters.contains_key("attribute_id") {
            messages.push(ExecutionMessage::error(
                "Missing required parameter: attribute_id",
            ));
        }

        if !operation.parameters.contains_key("value") {
            messages.push(ExecutionMessage::error("Missing required parameter: value"));
        }

        Ok(messages)
    }
}

impl ValidateHandler {
    async fn perform_validation(
        &self,
        attribute_id: &AttributeId,
        value: &Value,
        _context: &ExecutionContext,
    ) -> Result<ValidationResult> {
        // This is a simplified validation - in reality, this would:
        // 1. Look up the attribute definition from the data dictionary
        // 2. Apply type validation, format validation, business rules
        // 3. Potentially call external validation services
        // 4. Check compliance requirements (PII handling, etc.)

        match attribute_id.to_string().as_str() {
            attr if attr.contains("email") => {
                let email_str = value
                    .as_str()
                    .ok_or_else(|| anyhow::anyhow!("Email must be a string"))?;
                Ok(ValidationResult {
                    is_valid: email_str.contains('@'),
                    message: if email_str.contains('@') {
                        "Valid email format".to_string()
                    } else {
                        "Invalid email format".to_string()
                    },
                    details: HashMap::new(),
                })
            }
            attr if attr.contains("risk_rating") => {
                let rating = value
                    .as_str()
                    .ok_or_else(|| anyhow::anyhow!("Risk rating must be a string"))?;
                let valid_ratings = ["LOW", "MEDIUM", "HIGH", "CRITICAL"];
                Ok(ValidationResult {
                    is_valid: valid_ratings.contains(&rating),
                    message: format!("Risk rating validation: {}", rating),
                    details: HashMap::new(),
                })
            }
            _ => Ok(ValidationResult {
                is_valid: true,
                message: "Default validation passed".to_string(),
                details: HashMap::new(),
            }),
        }
    }
}

#[derive(Debug, Clone, serde::Serialize, serde::Deserialize)]
struct ValidationResult {
    is_valid: bool,
    message: String,
    details: HashMap<String, Value>,
}

/// Handler for collection operations (collect)
pub struct CollectHandler;

#[async_trait]
impl OperationHandler for CollectHandler {
    fn handles(&self) -> &str {
        "collect"
    }

    async fn execute(
        &self,
        operation: &DslOperation,
        state: &DslState,
        context: &ExecutionContext,
    ) -> Result<ExecutionResult> {
        let mut messages = Vec::new();
        let mut external_responses = HashMap::new();

        // Extract collection parameters
        let attribute_id = operation
            .parameters
            .get("attribute_id")
            .and_then(|v| serde_json::from_value::<AttributeId>(v.clone()).ok())
            .ok_or_else(|| anyhow::anyhow!("Missing or invalid attribute_id for collection"))?;

        let source = operation
            .parameters
            .get("from")
            .and_then(|v| v.as_str())
            .ok_or_else(|| anyhow::anyhow!("Missing 'from' parameter for collection"))?;

        // Perform data collection from external source
        let collected_data = self
            .collect_from_source(&attribute_id, source, context)
            .await?;

        messages.push(ExecutionMessage::info(format!(
            "Successfully collected data for attribute {} from {}",
            attribute_id, source
        )));

        // Apply operation to create new state
        let mut new_operation = operation.clone();
        new_operation
            .parameters
            .insert("value".to_string(), collected_data.clone());

        let new_state = state.apply_operation(new_operation.clone());

        // Store collected data in external responses
        external_responses.insert("collected_data".to_string(), collected_data);
        external_responses.insert("source".to_string(), Value::String(source.to_string()));

        Ok(ExecutionResult {
            success: true,
            operation: new_operation,
            new_state,
            messages,
            external_responses,
            duration_ms: 0,
        })
    }

    async fn validate(
        &self,
        operation: &DslOperation,
        _state: &DslState,
    ) -> Result<Vec<ExecutionMessage>> {
        let mut messages = Vec::new();

        if !operation.parameters.contains_key("attribute_id") {
            messages.push(ExecutionMessage::error(
                "Missing required parameter: attribute_id",
            ));
        }

        if !operation.parameters.contains_key("from") {
            messages.push(ExecutionMessage::error("Missing required parameter: from"));
        }

        Ok(messages)
    }
}

impl CollectHandler {
    async fn collect_from_source(
        &self,
        attribute_id: &AttributeId,
        source: &str,
        _context: &ExecutionContext,
    ) -> Result<Value> {
        // This is a mock implementation - in reality, this would:
        // 1. Connect to the specified external system
        // 2. Execute queries or API calls to collect the data
        // 3. Apply any necessary data transformations
        // 4. Handle errors and retry logic

        match source {
            "risk-engine" => {
                // Mock risk engine response
                Ok(serde_json::json!({
                    "risk_score": 2.5,
                    "risk_rating": "LOW",
                    "factors": ["clean_sanctions", "good_credit_history"],
                    "last_updated": chrono::Utc::now().to_rfc3339()
                }))
            }
            "document-store" => {
                // Mock document store response
                Ok(serde_json::json!({
                    "document_id": Uuid::new_v4(),
                    "document_type": "certificate_of_incorporation",
                    "status": "verified",
                    "url": "https://docs.example.com/cert123.pdf"
                }))
            }
            "crs-check" => {
                // Mock CRS compliance check
                Ok(serde_json::json!({
                    "crs_status": "REPORTABLE",
                    "jurisdiction": "US",
                    "tax_residence": ["US", "GB"],
                    "checked_at": chrono::Utc::now().to_rfc3339()
                }))
            }
            _ => {
                // Default mock response for unknown sources
                Ok(serde_json::json!({
                    "source": source,
                    "attribute": attribute_id.to_string(),
                    "value": "mock_collected_value",
                    "timestamp": chrono::Utc::now().to_rfc3339()
                }))
            }
        }
    }
}

/// Handler for entity declaration operations (declare-entity)
pub struct DeclareEntityHandler;

#[async_trait]
impl OperationHandler for DeclareEntityHandler {
    fn handles(&self) -> &str {
        "declare-entity"
    }

    async fn execute(
        &self,
        operation: &DslOperation,
        state: &DslState,
        _context: &ExecutionContext,
    ) -> Result<ExecutionResult> {
        let mut messages = Vec::new();
        let external_responses = HashMap::new();

        // Extract entity parameters
        let node_id = operation
            .parameters
            .get("node-id")
            .and_then(|v| v.as_str())
            .ok_or_else(|| anyhow::anyhow!("Missing node-id for entity declaration"))?;

        let label = operation
            .parameters
            .get("label")
            .and_then(|v| v.as_str())
            .ok_or_else(|| anyhow::anyhow!("Missing label for entity declaration"))?;

        messages.push(ExecutionMessage::info(format!(
            "Declared entity: {} with label {}",
            node_id, label
        )));

        // Validate entity properties if present
        if let Some(properties) = operation.parameters.get("properties") {
            if let Some(props_obj) = properties.as_object() {
                messages.push(ExecutionMessage::info(format!(
                    "Entity properties: {} fields",
                    props_obj.len()
                )));
            }
        }

        // Apply operation to create new state
        let new_state = state.apply_operation(operation.clone());

        Ok(ExecutionResult {
            success: true,
            operation: operation.clone(),
            new_state,
            messages,
            external_responses,
            duration_ms: 0,
        })
    }

    async fn validate(
        &self,
        operation: &DslOperation,
        _state: &DslState,
    ) -> Result<Vec<ExecutionMessage>> {
        let mut messages = Vec::new();

        if !operation.parameters.contains_key("node-id") {
            messages.push(ExecutionMessage::error(
                "Missing required parameter: node-id",
            ));
        }

        if !operation.parameters.contains_key("label") {
            messages.push(ExecutionMessage::error("Missing required parameter: label"));
        }

        Ok(messages)
    }
}

/// Handler for edge creation operations (create-edge)
pub struct CreateEdgeHandler;

#[async_trait]
impl OperationHandler for CreateEdgeHandler {
    fn handles(&self) -> &str {
        "create-edge"
    }

    async fn execute(
        &self,
        operation: &DslOperation,
        state: &DslState,
        _context: &ExecutionContext,
    ) -> Result<ExecutionResult> {
        let mut messages = Vec::new();
        let external_responses = HashMap::new();

        // Extract edge parameters
        let from = operation
            .parameters
            .get("from")
            .and_then(|v| v.as_str())
            .ok_or_else(|| anyhow::anyhow!("Missing 'from' parameter for edge creation"))?;

        let to = operation
            .parameters
            .get("to")
            .and_then(|v| v.as_str())
            .ok_or_else(|| anyhow::anyhow!("Missing 'to' parameter for edge creation"))?;

        let edge_type = operation
            .parameters
            .get("type")
            .and_then(|v| v.as_str())
            .ok_or_else(|| anyhow::anyhow!("Missing 'type' parameter for edge creation"))?;

        messages.push(ExecutionMessage::info(format!(
            "Created edge: {} --[{}]--> {}",
            from, edge_type, to
        )));

        // Handle ownership percentage if present
        if let Some(properties) = operation.parameters.get("properties") {
            if let Some(props_obj) = properties.as_object() {
                if let Some(percent) = props_obj.get("percent").and_then(|v| v.as_f64()) {
                    messages.push(ExecutionMessage::info(format!(
                        "Ownership percentage: {}%",
                        percent
                    )));
                }
            }
        }

        // Apply operation to create new state
        let new_state = state.apply_operation(operation.clone());

        Ok(ExecutionResult {
            success: true,
            operation: operation.clone(),
            new_state,
            messages,
            external_responses,
            duration_ms: 0,
        })
    }

    async fn validate(
        &self,
        operation: &DslOperation,
        _state: &DslState,
    ) -> Result<Vec<ExecutionMessage>> {
        let mut messages = Vec::new();

        let required_params = vec!["from", "to", "type"];
        for param in required_params {
            if !operation.parameters.contains_key(param) {
                messages.push(ExecutionMessage::error(format!(
                    "Missing required parameter: {}",
                    param
                )));
            }
        }

        Ok(messages)
    }
}

/// Handler for checking operations (check)
pub struct CheckHandler;

#[async_trait]
impl OperationHandler for CheckHandler {
    fn handles(&self) -> &str {
        "check"
    }

    async fn execute(
        &self,
        operation: &DslOperation,
        state: &DslState,
        _context: &ExecutionContext,
    ) -> Result<ExecutionResult> {
        let mut messages = Vec::new();
        let mut external_responses = HashMap::new();

        // Extract check parameters
        let attribute_id = operation
            .parameters
            .get("attribute_id")
            .and_then(|v| serde_json::from_value::<AttributeId>(v.clone()).ok())
            .ok_or_else(|| anyhow::anyhow!("Missing or invalid attribute_id for check"))?;

        let condition = operation
            .parameters
            .get("condition")
            .and_then(|v| v.as_str())
            .unwrap_or("exists");

        let expected_value = operation.parameters.get("equals");

        // Perform the check
        let check_result = self
            .perform_check(&attribute_id, condition, expected_value, state)
            .await?;

        if check_result.passed {
            messages.push(ExecutionMessage::info(format!(
                "Check passed for attribute {}: {}",
                attribute_id, check_result.message
            )));
        } else {
            messages.push(ExecutionMessage::warning(format!(
                "Check failed for attribute {}: {}",
                attribute_id, check_result.message
            )));
        }

        // Apply operation to create new state
        let new_state = state.apply_operation(operation.clone());

        // Store check result
        external_responses.insert(
            "check_result".to_string(),
            serde_json::to_value(&check_result)?,
        );

        Ok(ExecutionResult {
            success: check_result.passed,
            operation: operation.clone(),
            new_state,
            messages,
            external_responses,
            duration_ms: 0,
        })
    }

    async fn validate(
        &self,
        operation: &DslOperation,
        _state: &DslState,
    ) -> Result<Vec<ExecutionMessage>> {
        let mut messages = Vec::new();

        if !operation.parameters.contains_key("attribute_id") {
            messages.push(ExecutionMessage::error(
                "Missing required parameter: attribute_id",
            ));
        }

        Ok(messages)
    }
}

impl CheckHandler {
    async fn perform_check(
        &self,
        attribute_id: &AttributeId,
        condition: &str,
        expected_value: Option<&Value>,
        state: &DslState,
    ) -> Result<CheckResult> {
        // Get current value from state
        let current_value = state.current_state.get(attribute_id);

        match condition {
            "exists" => Ok(CheckResult {
                passed: current_value.is_some(),
                message: format!(
                    "Attribute {} {}",
                    attribute_id,
                    if current_value.is_some() {
                        "exists"
                    } else {
                        "does not exist"
                    }
                ),
            }),
            "equals" => {
                if let (Some(current), Some(expected)) = (current_value, expected_value) {
                    Ok(CheckResult {
                        passed: current == expected,
                        message: format!(
                            "Value {} for attribute {}",
                            if current == expected {
                                "matches expected"
                            } else {
                                "does not match expected"
                            },
                            attribute_id
                        ),
                    })
                } else {
                    Ok(CheckResult {
                        passed: false,
                        message:
                            "Cannot perform equals check - missing current value or expected value"
                                .to_string(),
                    })
                }
            }
            _ => Ok(CheckResult {
                passed: false,
                message: format!("Unknown check condition: {}", condition),
            }),
        }
    }
}

#[derive(Debug, Clone, serde::Serialize, serde::Deserialize)]
struct CheckResult {
    passed: bool,
    message: String,
}

/// Handler for the new ubo.outcome declarative verb
pub struct UboOutcomeHandler;

impl OperationHandler for UboOutcomeHandler {
    fn handles(&self) -> Vec<String> {
        vec!["ubo.outcome".to_string()]
    }

    async fn execute(
        &self,
        verb: &str,
        pairs: &std::collections::HashMap<crate::Key, crate::Value>,
        context: &mut crate::execution::context::ExecutionContext,
    ) -> Result<serde_json::Value, crate::execution::context::ExecutionError> {
        use crate::execution::context::ExecutionError;

        // Extract required fields from key-value pairs
        let target = pairs
            .get(&crate::Key::new("target"))
            .and_then(|v| match v {
                crate::Value::Literal(crate::Literal::String(s)) => Some(s.clone()),
                _ => None,
            })
            .ok_or_else(|| {
                ExecutionError::ValidationError("Missing :target in ubo.outcome".to_string())
            })?;

        let timestamp = pairs
            .get(&crate::Key::new("at"))
            .and_then(|v| match v {
                crate::Value::Literal(crate::Literal::String(s)) => Some(s.clone()),
                _ => None,
            })
            .ok_or_else(|| {
                ExecutionError::ValidationError("Missing :at in ubo.outcome".to_string())
            })?;

        let threshold = pairs
            .get(&crate::Key::new("threshold"))
            .and_then(|v| match v {
                crate::Value::Literal(crate::Literal::Number(n)) => Some(*n),
                _ => None,
            })
            .ok_or_else(|| {
                ExecutionError::ValidationError("Missing :threshold in ubo.outcome".to_string())
            })?;

        // Store the UBO outcome as declarative state
        let outcome = serde_json::json!({
            "verb": "ubo.outcome",
            "target": target,
            "at": timestamp,
            "threshold": threshold,
            "ubos": pairs.get(&crate::Key::new("ubos")).unwrap_or(&crate::Value::List(vec![])),
            "unresolved": pairs.get(&crate::Key::new("unresolved")).unwrap_or(&crate::Value::List(vec![])),
        });

        // Update execution context with the outcome
        context.set_state(&format!("ubo_outcome_{}", target), outcome.clone());

        Ok(outcome)
    }

    async fn validate(
        &self,
        verb: &str,
        pairs: &std::collections::HashMap<crate::Key, crate::Value>,
    ) -> Result<bool, String> {
        // Validate required fields exist
        if !pairs.contains_key(&crate::Key::new("target")) {
            return Err("ubo.outcome requires :target".to_string());
        }
        if !pairs.contains_key(&crate::Key::new("at")) {
            return Err("ubo.outcome requires :at".to_string());
        }
        if !pairs.contains_key(&crate::Key::new("threshold")) {
            return Err("ubo.outcome requires :threshold".to_string());
        }
        Ok(true)
    }
}

/// Handler for the new role.assign declarative verb
pub struct RoleAssignHandler;

impl OperationHandler for RoleAssignHandler {
    fn handles(&self) -> Vec<String> {
        vec!["role.assign".to_string()]
    }

    async fn execute(
        &self,
        verb: &str,
        pairs: &std::collections::HashMap<crate::Key, crate::Value>,
        context: &mut crate::execution::context::ExecutionContext,
    ) -> Result<serde_json::Value, crate::execution::context::ExecutionError> {
        use crate::execution::context::ExecutionError;

        // Extract required fields from key-value pairs
        let entity = pairs
            .get(&crate::Key::new("entity"))
            .and_then(|v| match v {
                crate::Value::Literal(crate::Literal::String(s)) => Some(s.clone()),
                _ => None,
            })
            .ok_or_else(|| {
                ExecutionError::ValidationError("Missing :entity in role.assign".to_string())
            })?;

        let role = pairs
            .get(&crate::Key::new("role"))
            .and_then(|v| match v {
                crate::Value::Literal(crate::Literal::String(s)) => Some(s.clone()),
                _ => None,
            })
            .ok_or_else(|| {
                ExecutionError::ValidationError("Missing :role in role.assign".to_string())
            })?;

        let cbu = pairs
            .get(&crate::Key::new("cbu"))
            .and_then(|v| match v {
                crate::Value::Literal(crate::Literal::String(s)) => Some(s.clone()),
                _ => None,
            })
            .ok_or_else(|| {
                ExecutionError::ValidationError("Missing :cbu in role.assign".to_string())
            })?;

        // Store the role assignment as declarative state
        let assignment = serde_json::json!({
            "verb": "role.assign",
            "entity": entity,
            "role": role,
            "cbu": cbu,
            "period": pairs.get(&crate::Key::new("period")),
            "evidence": pairs.get(&crate::Key::new("evidence")).unwrap_or(&crate::Value::List(vec![])),
        });

        // Update CBU graph state
        let cbu_graph_key = format!("cbu_graph_{}", cbu);
        let mut cbu_graph = context
            .get_state(&cbu_graph_key)
            .unwrap_or_else(|| serde_json::json!({"assignments": []}));

        if let Some(assignments) = cbu_graph["assignments"].as_array_mut() {
            assignments.push(assignment.clone());
        }

        context.set_state(&cbu_graph_key, cbu_graph);

        Ok(assignment)
    }

    async fn validate(
        &self,
        verb: &str,
        pairs: &std::collections::HashMap<crate::Key, crate::Value>,
    ) -> Result<bool, String> {
        // Validate required fields exist
        if !pairs.contains_key(&crate::Key::new("entity")) {
            return Err("role.assign requires :entity".to_string());
        }
        if !pairs.contains_key(&crate::Key::new("role")) {
            return Err("role.assign requires :role".to_string());
        }
        if !pairs.contains_key(&crate::Key::new("cbu")) {
            return Err("role.assign requires :cbu".to_string());
        }
        Ok(true)
    }
}

/// Factory function to create all standard operation handlers
pub fn create_standard_handlers() -> Vec<Arc<dyn OperationHandler>> {
    vec![
        Arc::new(ValidateHandler),
        Arc::new(CollectHandler),
        Arc::new(DeclareEntityHandler),
        Arc::new(CreateEdgeHandler),
        Arc::new(CheckHandler),
        Arc::new(UboOutcomeHandler),
        Arc::new(RoleAssignHandler),
    ]
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::dsl::operations::ExecutableDslOperation as DslOperation;
    use std::collections::HashMap;

    #[tokio::test]
    async fn test_validate_handler() {
        let handler = ValidateHandler;
        let mut params = HashMap::new();
        params.insert(
            "attribute_id".to_string(),
            serde_json::to_value(AttributeId::new()).unwrap(),
        );
        params.insert(
            "value".to_string(),
            Value::String("test@example.com".to_string()),
        );

        let operation = DslOperation {
            operation_type: "validate".to_string(),
            parameters: params,
            metadata: HashMap::new(),
        };

        let state = DslState {
            business_unit_id: "test".to_string(),
            operations: vec![],
            current_state: HashMap::new(),
            metadata: super::super::StateMetadata {
                created_at: chrono::Utc::now(),
                updated_at: chrono::Utc::now(),
                domain: "test".to_string(),
                status: "active".to_string(),
                tags: vec![],
                compliance_flags: vec![],
            },
            version: 0,
        };

        let context = ExecutionContext {
            session_id: uuid::Uuid::new_v4(),
            business_unit_id: "test".to_string(),
            domain: "test".to_string(),
            executor: "test".to_string(),
            started_at: chrono::Utc::now(),
            environment: HashMap::new(),
            integrations: vec![],
        };

        let result = handler.execute(&operation, &state, &context).await;
        assert!(result.is_ok());
    }
}
