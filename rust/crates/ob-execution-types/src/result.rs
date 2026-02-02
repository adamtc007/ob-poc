//! Execution Result Types
//!
//! `ExecutionResult` represents the outcome of executing a single DSL verb.
//! This is the high-level result type returned from verb execution.

use serde::{Deserialize, Serialize};
use serde_json::Value as JsonValue;
use uuid::Uuid;

/// Result of executing a verb
///
/// This enum captures all possible return types from DSL verb execution.
/// The variants align with the `returns.type` field in verb YAML definitions.
#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(tag = "type", rename_all = "snake_case")]
pub enum ExecutionResult {
    /// A UUID was returned (e.g., from INSERT RETURNING)
    Uuid(Uuid),

    /// A single record was returned as JSON
    Record(JsonValue),

    /// Multiple records were returned as JSON array
    RecordSet(Vec<JsonValue>),

    /// Count of affected rows (for UPDATE/DELETE)
    Affected(u64),

    /// No result (void operation)
    Void,
}

impl ExecutionResult {
    /// Create a UUID result
    pub fn uuid(id: Uuid) -> Self {
        Self::Uuid(id)
    }

    /// Create a record result from JSON value
    pub fn record(value: JsonValue) -> Self {
        Self::Record(value)
    }

    /// Create a record set result from JSON values
    pub fn record_set(values: Vec<JsonValue>) -> Self {
        Self::RecordSet(values)
    }

    /// Create an affected rows result
    pub fn affected(count: u64) -> Self {
        Self::Affected(count)
    }

    /// Create a void result
    pub fn void() -> Self {
        Self::Void
    }

    /// Try to extract UUID from result
    pub fn as_uuid(&self) -> Option<Uuid> {
        match self {
            Self::Uuid(id) => Some(*id),
            _ => None,
        }
    }

    /// Try to extract record from result
    pub fn as_record(&self) -> Option<&JsonValue> {
        match self {
            Self::Record(v) => Some(v),
            _ => None,
        }
    }

    /// Try to extract record set from result
    pub fn as_record_set(&self) -> Option<&[JsonValue]> {
        match self {
            Self::RecordSet(v) => Some(v),
            _ => None,
        }
    }

    /// Try to extract affected count from result
    pub fn as_affected(&self) -> Option<u64> {
        match self {
            Self::Affected(n) => Some(*n),
            _ => None,
        }
    }

    /// Check if this is a void result
    pub fn is_void(&self) -> bool {
        matches!(self, Self::Void)
    }

    /// Get a descriptive string for the result type
    pub fn type_name(&self) -> &'static str {
        match self {
            Self::Uuid(_) => "uuid",
            Self::Record(_) => "record",
            Self::RecordSet(_) => "record_set",
            Self::Affected(_) => "affected",
            Self::Void => "void",
        }
    }
}

impl From<Uuid> for ExecutionResult {
    fn from(id: Uuid) -> Self {
        Self::Uuid(id)
    }
}

impl From<JsonValue> for ExecutionResult {
    fn from(value: JsonValue) -> Self {
        Self::Record(value)
    }
}

impl From<Vec<JsonValue>> for ExecutionResult {
    fn from(values: Vec<JsonValue>) -> Self {
        Self::RecordSet(values)
    }
}

impl From<u64> for ExecutionResult {
    fn from(count: u64) -> Self {
        Self::Affected(count)
    }
}

impl From<()> for ExecutionResult {
    fn from(_: ()) -> Self {
        Self::Void
    }
}

// =============================================================================
// Tests
// =============================================================================

#[cfg(test)]
mod tests {
    use super::*;
    use serde_json::json;

    #[test]
    fn test_uuid_result() {
        let id = Uuid::now_v7();
        let result = ExecutionResult::uuid(id);

        assert_eq!(result.as_uuid(), Some(id));
        assert_eq!(result.type_name(), "uuid");
    }

    #[test]
    fn test_record_result() {
        let value = json!({"name": "Test", "id": 123});
        let result = ExecutionResult::record(value.clone());

        assert_eq!(result.as_record(), Some(&value));
        assert_eq!(result.type_name(), "record");
    }

    #[test]
    fn test_record_set_result() {
        let values = vec![json!({"id": 1}), json!({"id": 2})];
        let result = ExecutionResult::record_set(values.clone());

        assert_eq!(result.as_record_set(), Some(values.as_slice()));
        assert_eq!(result.type_name(), "record_set");
    }

    #[test]
    fn test_affected_result() {
        let result = ExecutionResult::affected(42);

        assert_eq!(result.as_affected(), Some(42));
        assert_eq!(result.type_name(), "affected");
    }

    #[test]
    fn test_void_result() {
        let result = ExecutionResult::void();

        assert!(result.is_void());
        assert_eq!(result.type_name(), "void");
    }

    #[test]
    fn test_from_uuid() {
        let id = Uuid::now_v7();
        let result: ExecutionResult = id.into();
        assert_eq!(result.as_uuid(), Some(id));
    }

    #[test]
    fn test_from_json() {
        let value = json!({"test": true});
        let result: ExecutionResult = value.clone().into();
        assert_eq!(result.as_record(), Some(&value));
    }

    #[test]
    fn test_serialization() {
        let result = ExecutionResult::uuid(Uuid::nil());
        let json = serde_json::to_string(&result).unwrap();
        assert!(json.contains("uuid"));
    }
}
