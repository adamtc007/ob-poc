use serde::{Deserialize, Serialize};
use std::collections::HashMap;
use std::fmt;
use uuid::Uuid;

/// Executable DSL Operation that can be processed by the execution engine
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ExecutableDslOperation {
    /// Unique identifier for this operation
    pub operation_id: Uuid,

    /// The DSL content to execute
    pub dsl_content: String,

    /// Operation type (derived from DSL content)
    pub operation_type: DslOperationType,

    /// Context for execution
    pub context: OperationContext,

    /// Metadata associated with the operation
    pub metadata: HashMap<String, String>,
}

/// Types of DSL operations supported
#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq, Hash)]
pub enum DslOperationType {
    /// Case management operations
    CaseCreate,
    CaseUpdate,
    CaseClose,

    /// Entity operations
    EntityRegister,
    EntityUpdate,
    EntityLink,

    /// KYC operations
    KycStart,
    KycCollect,
    KycVerify,

    /// UBO operations
    UboCollect,
    UboResolve,
    UboCalculate,

    /// Document operations
    DocumentCatalog,
    DocumentVerify,
    DocumentExtract,

    /// Generic operation for unknown types
    Unknown,
}

impl fmt::Display for DslOperationType {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        write!(f, "{:?}", self)
    }
}

/// Context for DSL operation execution
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct OperationContext {
    /// Case ID associated with this operation
    pub case_id: Option<String>,

    /// User executing the operation
    pub user_id: String,

    /// Domain context
    pub domain: String,

    /// Client Business Unit ID
    pub cbu_id: Option<String>,

    /// Additional context data
    pub context_data: HashMap<String, String>,
}

/// Result of DSL operation execution
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct OperationResult {
    /// Whether the operation succeeded
    pub success: bool,

    /// Operation ID
    pub operation_id: Uuid,

    /// Result data (JSON serializable)
    pub result_data: Option<serde_json::Value>,

    /// Any errors that occurred
    pub errors: Vec<String>,

    /// Warnings generated
    pub warnings: Vec<String>,

    /// Execution time in milliseconds
    pub execution_time_ms: u64,

    /// Entities created/modified
    pub affected_entities: Vec<String>,
}

impl ExecutableDslOperation {
    /// Create a new executable DSL operation
    pub fn new(dsl_content: String, context: OperationContext) -> Self {
        let operation_type = Self::determine_operation_type(&dsl_content);

        Self {
            operation_id: Uuid::new_v4(),
            dsl_content,
            operation_type,
            context,
            metadata: HashMap::new(),
        }
    }

    /// Determine the operation type from DSL content
    fn determine_operation_type(dsl_content: &str) -> DslOperationType {
        if dsl_content.contains("case.create") {
            DslOperationType::CaseCreate
        } else if dsl_content.contains("case.update") {
            DslOperationType::CaseUpdate
        } else if dsl_content.contains("case.close") {
            DslOperationType::CaseClose
        } else if dsl_content.contains("entity.register") {
            DslOperationType::EntityRegister
        } else if dsl_content.contains("entity.update") {
            DslOperationType::EntityUpdate
        } else if dsl_content.contains("entity.link") {
            DslOperationType::EntityLink
        } else if dsl_content.contains("kyc.start") {
            DslOperationType::KycStart
        } else if dsl_content.contains("kyc.collect") {
            DslOperationType::KycCollect
        } else if dsl_content.contains("kyc.verify") {
            DslOperationType::KycVerify
        } else if dsl_content.contains("ubo.collect") {
            DslOperationType::UboCollect
        } else if dsl_content.contains("ubo.resolve") {
            DslOperationType::UboResolve
        } else if dsl_content.contains("ubo.calc") {
            DslOperationType::UboCalculate
        } else if dsl_content.contains("document.catalog") {
            DslOperationType::DocumentCatalog
        } else if dsl_content.contains("document.verify") {
            DslOperationType::DocumentVerify
        } else if dsl_content.contains("document.extract") {
            DslOperationType::DocumentExtract
        } else {
            DslOperationType::Unknown
        }
    }

    /// Add metadata to the operation
    pub fn with_metadata(mut self, key: String, value: String) -> Self {
        self.metadata.insert(key, value);
        self
    }

    /// Set the case ID in the context
    pub fn with_case_id(mut self, case_id: String) -> Self {
        self.context.case_id = Some(case_id);
        self
    }

    /// Set the CBU ID in the context
    pub fn with_cbu_id(mut self, cbu_id: String) -> Self {
        self.context.cbu_id = Some(cbu_id);
        self
    }
}

impl Default for OperationContext {
    fn default() -> Self {
        Self {
            case_id: None,
            user_id: "system".to_string(),
            domain: "general".to_string(),
            cbu_id: None,
            context_data: HashMap::new(),
        }
    }
}

impl OperationResult {
    /// Create a successful operation result
    pub fn success(operation_id: Uuid, execution_time_ms: u64) -> Self {
        Self {
            success: true,
            operation_id,
            result_data: None,
            errors: Vec::new(),
            warnings: Vec::new(),
            execution_time_ms,
            affected_entities: Vec::new(),
        }
    }

    /// Create a failed operation result
    pub fn failure(operation_id: Uuid, errors: Vec<String>, execution_time_ms: u64) -> Self {
        Self {
            success: false,
            operation_id,
            result_data: None,
            errors,
            warnings: Vec::new(),
            execution_time_ms,
            affected_entities: Vec::new(),
        }
    }

    /// Add result data
    pub fn with_data(mut self, data: serde_json::Value) -> Self {
        self.result_data = Some(data);
        self
    }

    /// Add affected entity
    pub fn with_affected_entity(mut self, entity_id: String) -> Self {
        self.affected_entities.push(entity_id);
        self
    }

    /// Add warning
    pub fn with_warning(mut self, warning: String) -> Self {
        self.warnings.push(warning);
        self
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_operation_type_detection() {
        let cases = vec![
            (
                "(case.create :case-id \"test\")",
                DslOperationType::CaseCreate,
            ),
            (
                "(case.update :case-id \"test\")",
                DslOperationType::CaseUpdate,
            ),
            (
                "(entity.register :entity-id \"test\")",
                DslOperationType::EntityRegister,
            ),
            ("(kyc.start :case-id \"test\")", DslOperationType::KycStart),
            (
                "(ubo.collect :entity-id \"test\")",
                DslOperationType::UboCollect,
            ),
            (
                "(document.catalog :doc-id \"test\")",
                DslOperationType::DocumentCatalog,
            ),
            ("(unknown.operation)", DslOperationType::Unknown),
        ];

        for (dsl_content, expected_type) in cases {
            let actual_type = ExecutableDslOperation::determine_operation_type(dsl_content);
            assert_eq!(
                actual_type, expected_type,
                "Failed for DSL: {}",
                dsl_content
            );
        }
    }

    #[test]
    fn test_executable_operation_creation() {
        let context = OperationContext::default();
        let dsl_content = "(case.create :case-id \"TEST-001\")".to_string();

        let operation = ExecutableDslOperation::new(dsl_content.clone(), context);

        assert_eq!(operation.dsl_content, dsl_content);
        assert_eq!(operation.operation_type, DslOperationType::CaseCreate);
        assert!(!operation.operation_id.is_nil());
    }

    #[test]
    fn test_operation_result_creation() {
        let operation_id = Uuid::new_v4();

        let success_result = OperationResult::success(operation_id, 100);
        assert!(success_result.success);
        assert_eq!(success_result.operation_id, operation_id);
        assert_eq!(success_result.execution_time_ms, 100);

        let failure_result =
            OperationResult::failure(operation_id, vec!["Test error".to_string()], 200);
        assert!(!failure_result.success);
        assert_eq!(failure_result.errors.len(), 1);
        assert_eq!(failure_result.execution_time_ms, 200);
    }
}
