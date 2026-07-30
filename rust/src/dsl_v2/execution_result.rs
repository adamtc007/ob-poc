//! Execution Result Types
//!
//! Clean result types for DSL execution, replacing the `last_created_pk()` coupling pattern.
//! Each step produces a `StepResult` that explicitly captures what happened.

use serde::{Deserialize, Serialize};
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

// =============================================================================
// Tests
// =============================================================================

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_step_result_produced_pk() {
        let pk = Uuid::new_v4();

        let created = StepResult::Created {
            pk,
            entity_type: "cbu".to_string(),
        };
        assert_eq!(created.produced_pk(), Some(pk));
        assert!(created.is_create());

        let noop = StepResult::NoOp;
        assert_eq!(noop.produced_pk(), None);
    }
}
