//! Execution Result Types
//!
//! Clean result types for DSL execution, replacing the `last_created_pk()` coupling pattern.
//! Each step produces a `StepResult` that explicitly captures what happened.

use serde::{Deserialize, Serialize};
use std::collections::HashMap;
use uuid::Uuid;

/// Result of executing a single DSL step
#[derive(Clone, Debug, Serialize, Deserialize)]
pub enum StepResult {
    /// Entity was created, here's the PK
    Created { pk: Uuid, entity_type: String },

    /// Entity was updated
    Updated { pk: Uuid, entity_type: String },

    /// Entity was deleted
    Deleted { pk: Uuid, entity_type: String },

    /// Read operation returned data
    Read {
        pk: Uuid,
        entity_type: String,
        data: serde_json::Value,
    },

    /// List operation returned multiple records
    List {
        entity_type: String,
        data: Vec<serde_json::Value>,
        count: usize,
    },

    /// Operation completed but produced no entity (e.g., linking existing entities)
    Linked {
        source_pk: Uuid,
        target_pk: Uuid,
        relationship: String,
    },

    /// Operation completed with no persistent change
    NoOp,

    /// Custom operation result
    Custom {
        op_id: String,
        data: serde_json::Value,
        produced_pk: Option<Uuid>,
    },

    /// Operation was skipped (e.g., already exists in upsert)
    Skipped { reason: String },
}

impl StepResult {
    /// Extract PK if this result produced one (for binding)
    pub fn produced_pk(&self) -> Option<Uuid> {
        match self {
            StepResult::Created { pk, .. } => Some(*pk),
            StepResult::Updated { pk, .. } => Some(*pk),
            StepResult::Read { pk, .. } => Some(*pk),
            StepResult::Custom { produced_pk, .. } => *produced_pk,
            StepResult::Deleted { .. }
            | StepResult::NoOp
            | StepResult::Skipped { .. }
            | StepResult::List { .. }
            | StepResult::Linked { .. } => None,
        }
    }

    /// Get entity type if applicable
    pub fn entity_type(&self) -> Option<&str> {
        match self {
            StepResult::Created { entity_type, .. }
            | StepResult::Updated { entity_type, .. }
            | StepResult::Deleted { entity_type, .. }
            | StepResult::Read { entity_type, .. }
            | StepResult::List { entity_type, .. } => Some(entity_type),
            _ => None,
        }
    }

    /// Check if this is a create operation
    pub fn is_create(&self) -> bool {
        matches!(self, StepResult::Created { .. })
    }

    /// Check if this is an update operation
    pub fn is_update(&self) -> bool {
        matches!(self, StepResult::Updated { .. })
    }

    /// Check if this result can produce a binding
    pub fn can_bind(&self) -> bool {
        self.produced_pk().is_some()
    }
}

/// Accumulated results from executing a plan
#[derive(Clone, Debug, Default)]
pub struct ExecutionResults {
    /// Results indexed by step index
    pub step_results: Vec<(usize, StepResult)>,
    /// Bindings created during execution: symbol name → UUID
    pub bindings_created: HashMap<String, Uuid>,
    /// Entity types for bindings: symbol name → entity type
    pub binding_types: HashMap<String, String>,
    /// Errors encountered: step index → error message
    pub errors: Vec<(usize, String)>,
}

impl ExecutionResults {
    pub fn new() -> Self {
        Self::default()
    }

    /// Record a successful step result
    pub fn record_step(&mut self, step_index: usize, result: StepResult, binding: Option<&str>) {
        // Record binding if provided and result produced a PK
        if let Some(bind_name) = binding {
            if let Some(pk) = result.produced_pk() {
                self.bindings_created.insert(bind_name.to_string(), pk);
            }
            if let Some(entity_type) = result.entity_type() {
                self.binding_types
                    .insert(bind_name.to_string(), entity_type.to_string());
            }
        }
        self.step_results.push((step_index, result));
    }

    /// Record an error for a step
    pub fn record_error(&mut self, step_index: usize, error: impl Into<String>) {
        self.errors.push((step_index, error.into()));
    }

    /// Check if execution was successful (no errors)
    pub fn is_success(&self) -> bool {
        self.errors.is_empty()
    }

    /// Get the number of successful steps
    pub fn success_count(&self) -> usize {
        self.step_results.len()
    }

    /// Get the number of errors
    pub fn error_count(&self) -> usize {
        self.errors.len()
    }

    /// Get binding by name
    pub fn get_binding(&self, name: &str) -> Option<Uuid> {
        self.bindings_created.get(name).copied()
    }

    /// Check if a binding exists
    pub fn has_binding(&self, name: &str) -> bool {
        self.bindings_created.contains_key(name)
    }

    /// Get entity type for a binding
    pub fn binding_type(&self, name: &str) -> Option<&str> {
        self.binding_types.get(name).map(|s| s.as_str())
    }

    /// Merge results from another execution
    pub fn merge(&mut self, other: ExecutionResults) {
        self.step_results.extend(other.step_results);
        self.bindings_created.extend(other.bindings_created);
        self.binding_types.extend(other.binding_types);
        self.errors.extend(other.errors);
    }

    /// Create a summary string for logging
    pub fn summary(&self) -> String {
        let creates = self
            .step_results
            .iter()
            .filter(|(_, r)| r.is_create())
            .count();
        let updates = self
            .step_results
            .iter()
            .filter(|(_, r)| r.is_update())
            .count();

        format!(
            "{} steps executed ({} creates, {} updates), {} bindings, {} errors",
            self.step_results.len(),
            creates,
            updates,
            self.bindings_created.len(),
            self.errors.len()
        )
    }
}

// =============================================================================
// Tests
// =============================================================================

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_step_result_produced_pk() {
        let pk = Uuid::now_v7();

        let created = StepResult::Created {
            pk,
            entity_type: "cbu".to_string(),
        };
        assert_eq!(created.produced_pk(), Some(pk));
        assert!(created.is_create());

        let noop = StepResult::NoOp;
        assert_eq!(noop.produced_pk(), None);
    }

    #[test]
    fn test_execution_results_record() {
        let mut results = ExecutionResults::new();
        let pk = Uuid::now_v7();

        results.record_step(
            0,
            StepResult::Created {
                pk,
                entity_type: "cbu".to_string(),
            },
            Some("fund"),
        );

        assert!(results.is_success());
        assert_eq!(results.get_binding("fund"), Some(pk));
        assert_eq!(results.binding_type("fund"), Some("cbu"));
    }

    #[test]
    fn test_execution_results_errors() {
        let mut results = ExecutionResults::new();
        results.record_error(0, "something went wrong");

        assert!(!results.is_success());
        assert_eq!(results.error_count(), 1);
    }

    #[test]
    fn test_execution_results_merge() {
        let mut results1 = ExecutionResults::new();
        let pk1 = Uuid::now_v7();
        results1.record_step(
            0,
            StepResult::Created {
                pk: pk1,
                entity_type: "cbu".to_string(),
            },
            Some("fund"),
        );

        let mut results2 = ExecutionResults::new();
        let pk2 = Uuid::now_v7();
        results2.record_step(
            1,
            StepResult::Created {
                pk: pk2,
                entity_type: "proper_person".to_string(),
            },
            Some("person"),
        );

        results1.merge(results2);

        assert_eq!(results1.success_count(), 2);
        assert!(results1.has_binding("fund"));
        assert!(results1.has_binding("person"));
    }
}
