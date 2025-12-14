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
    // ─────────────────────────────────────────────────────────────────────────────
    // Core / Entity Structure
    // ─────────────────────────────────────────────────────────────────────────────
    /// Missing a required role
    MissingRole {
        role: String,
        required: u32,
        current: u32,
    },

    /// Required field is missing or empty
    FieldMissing { field: String },

    /// Product not assigned or insufficient products
    MissingProduct {
        #[serde(skip_serializing_if = "Option::is_none")]
        product_type: Option<String>,
        required: u32,
        current: u32,
    },

    /// Required relationship does not exist
    MissingRelationship {
        relationship_type: String,
        #[serde(skip_serializing_if = "Option::is_none")]
        from_entity: Option<Uuid>,
        #[serde(skip_serializing_if = "Option::is_none")]
        to_entity: Option<Uuid>,
    },

    // ─────────────────────────────────────────────────────────────────────────────
    // Documents
    // ─────────────────────────────────────────────────────────────────────────────
    /// Missing a required document
    MissingDocument {
        document_type: String,
        #[serde(skip_serializing_if = "Option::is_none")]
        for_entity: Option<Uuid>,
    },

    /// Document not reviewed
    DocumentNotReviewed {
        document_id: Uuid,
        document_type: String,
    },

    // ─────────────────────────────────────────────────────────────────────────────
    // KYC Case
    // ─────────────────────────────────────────────────────────────────────────────
    /// No KYC case exists for this subject
    NoCaseExists {
        #[serde(skip_serializing_if = "Option::is_none")]
        case_type: Option<String>,
    },

    /// Case has no analyst assigned
    NoAnalystAssigned { case_id: Uuid },

    /// Risk rating has not been set
    RiskRatingNotSet { case_id: Uuid },

    /// Approval not recorded
    ApprovalNotRecorded { case_id: Uuid },

    /// Rejection not recorded (for terminal rejection state)
    RejectionNotRecorded { case_id: Uuid },

    // ─────────────────────────────────────────────────────────────────────────────
    // Workstreams
    // ─────────────────────────────────────────────────────────────────────────────
    /// Entity workstream missing
    WorkstreamMissing { entity_id: Uuid },

    /// Workstream data incomplete
    WorkstreamIncomplete {
        workstream_id: Uuid,
        entity_id: Uuid,
        missing_fields: Vec<String>,
    },

    // ─────────────────────────────────────────────────────────────────────────────
    // Screening
    // ─────────────────────────────────────────────────────────────────────────────
    /// Entity needs screening
    PendingScreening { entity_id: Uuid },

    /// Screening is stale/expired
    StaleScreening {
        entity_id: Uuid,
        screening_type: String,
        #[serde(skip_serializing_if = "Option::is_none")]
        last_screened_at: Option<DateTime<Utc>>,
        max_age_days: u32,
    },

    /// Unresolved screening alert
    UnresolvedAlert { alert_id: Uuid, entity_id: Uuid },

    /// Pending hit requires review
    PendingHit {
        screening_id: Uuid,
        entity_id: Uuid,
        hit_type: String,
    },

    // ─────────────────────────────────────────────────────────────────────────────
    // UBO / Ownership
    // ─────────────────────────────────────────────────────────────────────────────
    /// Ownership structure incomplete
    IncompleteOwnership { current_total: f64, required: f64 },

    /// UBO not verified
    UnverifiedUbo { ubo_id: Uuid, person_name: String },

    /// Ownership chains not resolved to natural persons
    UnresolvedOwnershipChain {
        entity_id: Uuid,
        #[serde(skip_serializing_if = "Option::is_none")]
        chain_depth: Option<u32>,
    },

    /// UBO threshold not applied
    UboThresholdNotApplied { cbu_id: Uuid },

    // ─────────────────────────────────────────────────────────────────────────────
    // Periodic Review / Freshness
    // ─────────────────────────────────────────────────────────────────────────────
    /// Entity data is stale
    EntityDataStale {
        entity_id: Uuid,
        last_updated: DateTime<Utc>,
        max_age_days: u32,
    },

    /// Change log not reviewed
    ChangeLogNotReviewed {
        #[serde(skip_serializing_if = "Option::is_none")]
        changes_since: Option<DateTime<Utc>>,
        pending_count: u32,
    },

    // ─────────────────────────────────────────────────────────────────────────────
    // Sign-off / Completion
    // ─────────────────────────────────────────────────────────────────────────────
    /// Sign-off not recorded
    SignOffMissing {
        #[serde(skip_serializing_if = "Option::is_none")]
        required_role: Option<String>,
    },

    /// Next review date not scheduled
    NextReviewNotScheduled,

    // ─────────────────────────────────────────────────────────────────────────────
    // Generic
    // ─────────────────────────────────────────────────────────────────────────────
    /// Requires manual approval
    ManualApprovalRequired,

    /// Custom blocker type
    Custom { code: String },
}

impl BlockerType {
    /// Get a suggested DSL verb to resolve this blocker
    pub fn suggested_verb(&self) -> Option<&'static str> {
        match self {
            // Core / Entity Structure
            BlockerType::MissingRole { .. } => Some("cbu.assign-role"),
            BlockerType::FieldMissing { .. } => Some("cbu.update"),
            BlockerType::MissingProduct { .. } => Some("cbu.add-product"),
            BlockerType::MissingRelationship { .. } => Some("ubo.add-ownership"),

            // Documents
            BlockerType::MissingDocument { .. } => Some("document.catalog"),
            BlockerType::DocumentNotReviewed { .. } => Some("doc-request.verify"),

            // KYC Case
            BlockerType::NoCaseExists { .. } => Some("kyc-case.create"),
            BlockerType::NoAnalystAssigned { .. } => Some("kyc-case.assign"),
            BlockerType::RiskRatingNotSet { .. } => Some("kyc-case.set-risk-rating"),
            BlockerType::ApprovalNotRecorded { .. } => Some("kyc-case.update-status"),
            BlockerType::RejectionNotRecorded { .. } => Some("kyc-case.update-status"),

            // Workstreams
            BlockerType::WorkstreamMissing { .. } => Some("entity-workstream.create"),
            BlockerType::WorkstreamIncomplete { .. } => Some("entity-workstream.update-status"),

            // Screening
            BlockerType::PendingScreening { .. } => Some("case-screening.run"),
            BlockerType::StaleScreening { .. } => Some("case-screening.run"),
            BlockerType::UnresolvedAlert { .. } => Some("case-screening.review-hit"),
            BlockerType::PendingHit { .. } => Some("case-screening.review-hit"),

            // UBO / Ownership
            BlockerType::IncompleteOwnership { .. } => Some("ubo.add-ownership"),
            BlockerType::UnverifiedUbo { .. } => Some("ubo.verify-ubo"),
            BlockerType::UnresolvedOwnershipChain { .. } => Some("ubo.trace-chains"),
            BlockerType::UboThresholdNotApplied { .. } => Some("threshold.derive"),

            // Periodic Review / Freshness
            BlockerType::EntityDataStale { .. } => Some("entity.update"),
            BlockerType::ChangeLogNotReviewed { .. } => None, // UI action, no DSL verb

            // Sign-off / Completion
            BlockerType::SignOffMissing { .. } => Some("kyc-case.update-status"),
            BlockerType::NextReviewNotScheduled => Some("kyc-case.update-status"),

            // Generic
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
