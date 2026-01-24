//! Document Entity Types
//!
//! Three-layer document model:
//! - Layer A: DocumentRequirement - what we need from entity
//! - Layer B: Document - logical identity
//! - Layer C: DocumentVersion - immutable submissions

use chrono::{DateTime, NaiveDate, Utc};
use serde::{Deserialize, Serialize};
use std::str::FromStr;
use uuid::Uuid;

/// Requirement status levels
/// Note: rejected/expired are NOT in the ordered progression - they're failure states
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "lowercase")]
pub enum RequirementState {
    Missing,
    Requested,
    Received, // Allegation received
    InQa,
    Verified, // QA passed
    Waived,   // Manual override
    Rejected, // QA failed - needs re-upload
    Expired,  // Validity lapsed
}

impl FromStr for RequirementState {
    type Err = RequirementStateError;

    fn from_str(s: &str) -> Result<Self, Self::Err> {
        match s {
            "missing" => Ok(Self::Missing),
            "requested" => Ok(Self::Requested),
            "received" => Ok(Self::Received),
            "in_qa" => Ok(Self::InQa),
            "verified" => Ok(Self::Verified),
            "waived" => Ok(Self::Waived),
            "rejected" => Ok(Self::Rejected),
            "expired" => Ok(Self::Expired),
            _ => Err(RequirementStateError::UnknownState(s.to_string())),
        }
    }
}

impl RequirementState {
    pub fn as_str(&self) -> &'static str {
        match self {
            Self::Missing => "missing",
            Self::Requested => "requested",
            Self::Received => "received",
            Self::InQa => "in_qa",
            Self::Verified => "verified",
            Self::Waived => "waived",
            Self::Rejected => "rejected",
            Self::Expired => "expired",
        }
    }

    /// Check if this state satisfies a minimum threshold.
    /// Rejected/expired do NOT satisfy any threshold (need re-upload).
    pub fn satisfies(&self, min_state: RequirementState) -> bool {
        use RequirementState::*;
        match (self, min_state) {
            // Failure states never satisfy any requirement
            (Rejected, _) | (Expired, _) => false,
            // Waived satisfies anything
            (Waived, _) => true,
            // Verified satisfies anything
            (Verified, _) => true,
            // Check ordered progression: missing < requested < received < in_qa < verified
            (_current, _threshold) => self.order() >= min_state.order(),
        }
    }

    /// Get the order value for state comparison
    fn order(&self) -> u8 {
        match self {
            Self::Missing => 0,
            Self::Requested => 1,
            Self::Received => 2,
            Self::InQa => 3,
            Self::Verified => 4,
            Self::Waived => 5,
            Self::Rejected | Self::Expired => 0, // Failure states have no order
        }
    }

    /// Is this a failure state that needs re-upload?
    pub fn is_failure(&self) -> bool {
        matches!(self, Self::Rejected | Self::Expired)
    }

    /// Is this a terminal success state?
    pub fn is_satisfied(&self) -> bool {
        matches!(self, Self::Verified | Self::Waived)
    }

    /// Can this state transition to the given state?
    pub fn can_transition_to(&self, target: RequirementState) -> bool {
        use RequirementState::*;
        match (self, target) {
            // From missing
            (Missing, Requested | Waived) => true,
            // From requested
            (Requested, Received | Expired | Waived) => true,
            // From received
            (Received, InQa | Waived) => true,
            // From in_qa
            (InQa, Verified | Rejected | Waived) => true,
            // From rejected (retry)
            (Rejected, Requested | Waived) => true,
            // From expired
            (Expired, Requested | Waived) => true,
            // Terminal states can't transition (except waive overrides anything)
            (Verified | Waived, _) => false,
            _ => false,
        }
    }
}

impl std::fmt::Display for RequirementState {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        write!(f, "{}", self.as_str())
    }
}

#[derive(Debug, thiserror::Error)]
pub enum RequirementStateError {
    #[error("Unknown requirement state: {0}")]
    UnknownState(String),
}

/// Document verification status (on version, not document)
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "lowercase")]
pub enum VerificationStatus {
    Pending,
    InQa,
    Verified,
    Rejected,
}

impl FromStr for VerificationStatus {
    type Err = ();

    fn from_str(s: &str) -> Result<Self, Self::Err> {
        match s {
            "pending" => Ok(Self::Pending),
            "in_qa" => Ok(Self::InQa),
            "verified" => Ok(Self::Verified),
            "rejected" => Ok(Self::Rejected),
            _ => Err(()),
        }
    }
}

impl VerificationStatus {
    pub fn as_str(&self) -> &'static str {
        match self {
            Self::Pending => "pending",
            Self::InQa => "in_qa",
            Self::Verified => "verified",
            Self::Rejected => "rejected",
        }
    }
}

/// Layer A: Document requirement (what we need from entity)
#[derive(Debug, Clone, Serialize, Deserialize)]
#[cfg_attr(feature = "database", derive(sqlx::FromRow))]
pub struct DocumentRequirement {
    pub requirement_id: Uuid,
    pub workflow_instance_id: Option<Uuid>,
    pub subject_entity_id: Option<Uuid>,
    pub subject_cbu_id: Option<Uuid>,
    pub doc_type: String,
    pub required_state: String,
    pub status: String,
    pub attempt_count: i32,
    pub max_attempts: i32,
    pub current_task_id: Option<Uuid>,
    pub latest_document_id: Option<Uuid>,
    pub latest_version_id: Option<Uuid>,
    pub last_rejection_code: Option<String>,
    pub last_rejection_reason: Option<String>,
    pub due_date: Option<NaiveDate>,
    pub created_at: DateTime<Utc>,
    pub updated_at: Option<DateTime<Utc>>,
    pub satisfied_at: Option<DateTime<Utc>>,
}

impl DocumentRequirement {
    /// Check if requirement is satisfied (verified or waived)
    pub fn is_satisfied(&self) -> bool {
        matches!(self.status.as_str(), "verified" | "waived")
    }

    /// Check if requirement can be retried (rejected but under max attempts)
    pub fn can_retry(&self) -> bool {
        self.status == "rejected" && self.attempt_count < self.max_attempts
    }

    /// Get parsed status
    pub fn parsed_status(&self) -> Result<RequirementState, RequirementStateError> {
        RequirementState::from_str(&self.status)
    }

    /// Get parsed required state
    pub fn parsed_required_state(&self) -> Result<RequirementState, RequirementStateError> {
        RequirementState::from_str(&self.required_state)
    }

    /// Check if current status satisfies the required state
    pub fn meets_requirement(&self) -> bool {
        if let (Ok(current), Ok(required)) = (self.parsed_status(), self.parsed_required_state()) {
            current.satisfies(required)
        } else {
            false
        }
    }
}

/// Layer B: Document (logical identity)
#[derive(Debug, Clone, Serialize, Deserialize)]
#[cfg_attr(feature = "database", derive(sqlx::FromRow))]
pub struct Document {
    pub document_id: Uuid,
    pub document_type: String,
    pub subject_entity_id: Option<Uuid>,
    pub subject_cbu_id: Option<Uuid>,
    pub parent_document_id: Option<Uuid>,
    pub requirement_id: Option<Uuid>,
    pub source: String,
    pub source_ref: Option<String>,
    pub created_at: DateTime<Utc>,
    pub created_by: Option<String>,
}

/// Layer C: Document version (immutable submission)
#[derive(Debug, Clone, Serialize, Deserialize)]
#[cfg_attr(feature = "database", derive(sqlx::FromRow))]
pub struct DocumentVersion {
    pub version_id: Uuid,
    pub document_id: Uuid,
    pub version_no: i32,
    pub content_type: String,
    pub structured_data: Option<serde_json::Value>,
    pub blob_ref: Option<String>,
    pub ocr_extracted: Option<serde_json::Value>,
    pub task_id: Option<Uuid>,
    pub verification_status: String,
    pub rejection_code: Option<String>,
    pub rejection_reason: Option<String>,
    pub verified_by: Option<String>,
    pub verified_at: Option<DateTime<Utc>>,
    pub valid_from: Option<NaiveDate>,
    pub valid_to: Option<NaiveDate>,
    pub quality_score: Option<f64>,
    pub extraction_confidence: Option<f64>,
    pub created_at: DateTime<Utc>,
    pub created_by: Option<String>,
}

impl DocumentVersion {
    /// Check if this version is verified
    pub fn is_verified(&self) -> bool {
        self.verification_status == "verified"
    }

    /// Check if this version is rejected
    pub fn is_rejected(&self) -> bool {
        self.verification_status == "rejected"
    }

    /// Check if the document validity has expired
    pub fn is_validity_expired(&self) -> bool {
        self.valid_to
            .map(|d| d < chrono::Utc::now().date_naive())
            .unwrap_or(false)
    }

    /// Get parsed verification status
    pub fn parsed_verification_status(&self) -> Option<VerificationStatus> {
        self.verification_status.parse().ok()
    }
}

/// Document with current status (joined view)
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct DocumentWithStatus {
    pub document_id: Uuid,
    pub document_type: String,
    pub subject_entity_id: Option<Uuid>,
    pub subject_cbu_id: Option<Uuid>,
    pub requirement_id: Option<Uuid>,
    pub source: String,
    pub source_ref: Option<String>,
    pub latest_version_id: Option<Uuid>,
    pub latest_version_no: Option<i32>,
    pub latest_status: Option<String>,
    pub verified_at: Option<DateTime<Utc>>,
    pub valid_from: Option<NaiveDate>,
    pub valid_to: Option<NaiveDate>,
    pub created_at: DateTime<Utc>,
}

/// Rejection reason code (from reference table)
#[derive(Debug, Clone, Serialize, Deserialize)]
#[cfg_attr(feature = "database", derive(sqlx::FromRow))]
pub struct RejectionCode {
    pub code: String,
    pub category: String,
    pub client_message: String,
    pub ops_message: String,
    pub next_action: String,
    pub is_retryable: bool,
}

impl RejectionCode {
    /// Generate full client-facing message with next action
    pub fn full_client_message(&self) -> String {
        format!("{} {}", self.client_message, self.next_action)
    }
}

/// Document event types for audit trail
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum DocumentEventType {
    Created,
    VersionUploaded,
    Verified,
    Rejected,
    Expired,
    StatusChanged,
}

impl DocumentEventType {
    pub fn as_str(&self) -> &'static str {
        match self {
            Self::Created => "created",
            Self::VersionUploaded => "version_uploaded",
            Self::Verified => "verified",
            Self::Rejected => "rejected",
            Self::Expired => "expired",
            Self::StatusChanged => "status_changed",
        }
    }
}

/// Document event for audit trail
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct DocumentEvent {
    pub event_id: Uuid,
    pub document_id: Uuid,
    pub version_id: Option<Uuid>,
    pub event_type: String,
    pub old_status: Option<String>,
    pub new_status: Option<String>,
    pub rejection_code: Option<String>,
    pub notes: Option<String>,
    pub actor: Option<String>,
    pub occurred_at: DateTime<Utc>,
}

/// Unsatisfied requirement (for blocker generation)
#[derive(Debug, Clone, Serialize, Deserialize)]
#[cfg_attr(feature = "database", derive(sqlx::FromRow))]
pub struct UnsatisfiedRequirement {
    pub requirement_id: Uuid,
    pub doc_type: String,
    pub subject_entity_id: Option<Uuid>,
    pub status: String,
    pub required_state: String,
    pub attempt_count: i32,
    pub last_rejection_code: Option<String>,
    pub last_rejection_reason: Option<String>,
}

impl UnsatisfiedRequirement {
    /// Generate client-facing message for re-request using rejection code lookup
    pub fn rejection_message(
        &self,
        codes: &std::collections::HashMap<String, RejectionCode>,
    ) -> Option<String> {
        self.last_rejection_code
            .as_ref()
            .and_then(|code| codes.get(code).map(|rc| rc.full_client_message()))
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_requirement_state_satisfies() {
        use RequirementState::*;

        // Verified satisfies anything
        assert!(Verified.satisfies(Missing));
        assert!(Verified.satisfies(Received));
        assert!(Verified.satisfies(Verified));

        // Waived satisfies anything
        assert!(Waived.satisfies(Missing));
        assert!(Waived.satisfies(Verified));

        // Received satisfies received or lower
        assert!(Received.satisfies(Missing));
        assert!(Received.satisfies(Requested));
        assert!(Received.satisfies(Received));
        assert!(!Received.satisfies(InQa));
        assert!(!Received.satisfies(Verified));

        // Failure states never satisfy
        assert!(!Rejected.satisfies(Missing));
        assert!(!Expired.satisfies(Missing));
    }

    #[test]
    fn test_requirement_state_is_failure() {
        assert!(RequirementState::Rejected.is_failure());
        assert!(RequirementState::Expired.is_failure());
        assert!(!RequirementState::Verified.is_failure());
        assert!(!RequirementState::Missing.is_failure());
    }

    #[test]
    fn test_requirement_state_transitions() {
        use RequirementState::*;

        assert!(Missing.can_transition_to(Requested));
        assert!(Missing.can_transition_to(Waived));
        assert!(!Missing.can_transition_to(Verified));

        assert!(Rejected.can_transition_to(Requested)); // Retry
        assert!(!Verified.can_transition_to(Rejected)); // Terminal
    }

    #[test]
    fn test_document_requirement_can_retry() {
        let mut req = DocumentRequirement {
            requirement_id: Uuid::new_v4(),
            workflow_instance_id: None,
            subject_entity_id: None,
            subject_cbu_id: None,
            doc_type: "passport".to_string(),
            required_state: "verified".to_string(),
            status: "rejected".to_string(),
            attempt_count: 1,
            max_attempts: 3,
            current_task_id: None,
            latest_document_id: None,
            latest_version_id: None,
            last_rejection_code: Some("UNREADABLE".to_string()),
            last_rejection_reason: None,
            due_date: None,
            created_at: Utc::now(),
            updated_at: None,
            satisfied_at: None,
        };

        assert!(req.can_retry());

        req.attempt_count = 3;
        assert!(!req.can_retry());

        req.status = "verified".to_string();
        assert!(!req.can_retry());
    }
}
