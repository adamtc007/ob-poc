//! Workflow State Types
//!
//! Defines the core state types for workflow instances.

use chrono::{DateTime, Utc};
use serde::{Deserialize, Serialize};
use std::collections::HashMap;
use uuid::Uuid;

/// A running instance of a workflow
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct WorkflowInstance {
    /// Unique instance ID
    pub instance_id: Uuid,
    /// Workflow definition ID (e.g., "kyc_onboarding")
    pub workflow_id: String,
    /// Workflow version
    pub version: u32,

    /// Type of entity this workflow is for (e.g., "cbu", "entity", "case")
    pub subject_type: String,
    /// UUID of the subject entity
    pub subject_id: Uuid,

    /// Current state in the workflow state machine
    pub current_state: String,
    /// When the current state was entered
    pub state_entered_at: DateTime<Utc>,

    /// History of state transitions
    pub history: Vec<StateTransition>,

    /// Current blockers preventing advancement
    pub blockers: Vec<Blocker>,

    /// Additional metadata
    #[serde(default)]
    pub metadata: HashMap<String, serde_json::Value>,

    /// Creation timestamp
    pub created_at: DateTime<Utc>,
    /// Last update timestamp
    pub updated_at: DateTime<Utc>,
    /// Who created this instance
    pub created_by: Option<String>,
}

impl WorkflowInstance {
    /// Create a new workflow instance
    pub fn new(
        workflow_id: String,
        version: u32,
        subject_type: String,
        subject_id: Uuid,
        initial_state: String,
        created_by: Option<String>,
    ) -> Self {
        let now = Utc::now();
        Self {
            instance_id: Uuid::new_v4(),
            workflow_id,
            version,
            subject_type,
            subject_id,
            current_state: initial_state,
            state_entered_at: now,
            history: Vec::new(),
            blockers: Vec::new(),
            metadata: HashMap::new(),
            created_at: now,
            updated_at: now,
            created_by,
        }
    }

    /// Record a state transition
    pub fn transition_to(&mut self, to_state: String, by: Option<String>, reason: Option<String>) {
        let from_state = std::mem::replace(&mut self.current_state, to_state.clone());
        let now = Utc::now();

        self.history.push(StateTransition {
            from_state,
            to_state,
            transitioned_at: now,
            transitioned_by: by,
            reason,
        });

        self.state_entered_at = now;
        self.updated_at = now;
        self.blockers.clear(); // Will be re-evaluated
    }

    /// Check if the workflow is in a terminal state
    pub fn is_terminal(&self, terminal_states: &[String]) -> bool {
        terminal_states.contains(&self.current_state)
    }
}

/// Record of a state transition
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct StateTransition {
    /// State transitioned from
    pub from_state: String,
    /// State transitioned to
    pub to_state: String,
    /// When the transition occurred
    pub transitioned_at: DateTime<Utc>,
    /// Who triggered the transition (user ID or "system")
    pub transitioned_by: Option<String>,
    /// Optional reason for the transition
    pub reason: Option<String>,
}

/// A blocker preventing workflow advancement
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct Blocker {
    /// Type of blocker with details
    pub blocker_type: BlockerType,
    /// Human-readable description
    pub description: String,
    /// DSL verb that can resolve this blocker
    pub resolution_action: Option<String>,
    /// Additional context for resolution
    #[serde(default)]
    pub details: HashMap<String, serde_json::Value>,
}

impl Blocker {
    /// Create a new blocker
    pub fn new(blocker_type: BlockerType, description: impl Into<String>) -> Self {
        Self {
            blocker_type,
            description: description.into(),
            resolution_action: None,
            details: HashMap::new(),
        }
    }

    /// Set the resolution action (DSL verb)
    pub fn with_resolution(mut self, action: impl Into<String>) -> Self {
        self.resolution_action = Some(action.into());
        self
    }

    /// Add a detail
    pub fn with_detail(mut self, key: impl Into<String>, value: serde_json::Value) -> Self {
        self.details.insert(key.into(), value);
        self
    }
}

/// Types of blockers with their specific data
#[derive(Debug, Clone, Serialize, Deserialize, PartialEq)]
#[serde(rename_all = "snake_case", tag = "type")]
pub enum BlockerType {
    /// Missing a required role
    MissingRole {
        role: String,
        required: u32,
        current: u32,
    },

    /// Missing a required document
    MissingDocument {
        document_type: String,
        #[serde(skip_serializing_if = "Option::is_none")]
        for_entity: Option<Uuid>,
    },

    /// Entity needs screening
    PendingScreening { entity_id: Uuid },

    /// Unresolved screening alert
    UnresolvedAlert { alert_id: Uuid, entity_id: Uuid },

    /// Ownership structure incomplete
    IncompleteOwnership { current_total: f64, required: f64 },

    /// UBO not verified
    UnverifiedUbo { ubo_id: Uuid, person_name: String },

    /// Requires manual approval
    ManualApprovalRequired,

    /// Custom blocker type
    Custom { code: String },
}

impl BlockerType {
    /// Get a suggested DSL verb to resolve this blocker
    pub fn suggested_verb(&self) -> Option<&'static str> {
        match self {
            BlockerType::MissingRole { .. } => Some("cbu.assign-role"),
            BlockerType::MissingDocument { .. } => Some("document.catalog"),
            BlockerType::PendingScreening { .. } => Some("case-screening.run"),
            BlockerType::UnresolvedAlert { .. } => Some("case-screening.review-hit"),
            BlockerType::IncompleteOwnership { .. } => Some("ubo.add-ownership"),
            BlockerType::UnverifiedUbo { .. } => Some("ubo.verify-ubo"),
            BlockerType::ManualApprovalRequired => Some("kyc-case.update-status"),
            BlockerType::Custom { .. } => None,
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_workflow_instance_creation() {
        let instance = WorkflowInstance::new(
            "kyc_onboarding".to_string(),
            1,
            "cbu".to_string(),
            Uuid::new_v4(),
            "INTAKE".to_string(),
            Some("user@example.com".to_string()),
        );

        assert_eq!(instance.workflow_id, "kyc_onboarding");
        assert_eq!(instance.current_state, "INTAKE");
        assert!(instance.history.is_empty());
        assert!(instance.blockers.is_empty());
    }

    #[test]
    fn test_state_transition() {
        let mut instance = WorkflowInstance::new(
            "kyc_onboarding".to_string(),
            1,
            "cbu".to_string(),
            Uuid::new_v4(),
            "INTAKE".to_string(),
            None,
        );

        instance.transition_to(
            "ENTITY_COLLECTION".to_string(),
            Some("system".to_string()),
            None,
        );

        assert_eq!(instance.current_state, "ENTITY_COLLECTION");
        assert_eq!(instance.history.len(), 1);
        assert_eq!(instance.history[0].from_state, "INTAKE");
        assert_eq!(instance.history[0].to_state, "ENTITY_COLLECTION");
    }

    #[test]
    fn test_blocker_builder() {
        let blocker = Blocker::new(
            BlockerType::MissingRole {
                role: "DIRECTOR".to_string(),
                required: 1,
                current: 0,
            },
            "At least one director required",
        )
        .with_resolution("cbu.assign-role")
        .with_detail("role", serde_json::json!("DIRECTOR"));

        assert_eq!(
            blocker.resolution_action,
            Some("cbu.assign-role".to_string())
        );
        assert!(blocker.details.contains_key("role"));
    }
}
