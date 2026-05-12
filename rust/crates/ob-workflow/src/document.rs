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
pub(crate) enum RequirementState {
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
    pub(crate) fn as_str(&self) -> &'static str {
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
    pub(crate) fn satisfies(&self, min_state: RequirementState) -> bool {
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
    pub(crate) fn is_failure(&self) -> bool {
        matches!(self, Self::Rejected | Self::Expired)
    }

    /// Is this a terminal success state?
    pub(crate) fn is_satisfied(&self) -> bool {
        matches!(self, Self::Verified | Self::Waived)
    }

    /// Can this state transition to the given state?
    pub(crate) fn can_transition_to(&self, target: RequirementState) -> bool {
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
pub(crate) enum RequirementStateError {
    #[error("Unknown requirement state: {0}")]
    UnknownState(String),
}

/// Document verification status (on version, not document)
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "lowercase")]
pub(crate) enum VerificationStatus {
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
    pub(crate) fn as_str(&self) -> &'static str {
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
pub(crate) struct DocumentRequirement {
    pub(crate) requirement_id: Uuid,
    pub(crate) workflow_instance_id: Option<Uuid>,
    pub(crate) subject_entity_id: Option<Uuid>,
    pub(crate) subject_cbu_id: Option<Uuid>,
    pub(crate) doc_type: String,
    pub(crate) required_state: String,
    pub(crate) status: String,
    pub(crate) attempt_count: i32,
    pub(crate) max_attempts: i32,
    pub(crate) current_task_id: Option<Uuid>,
    pub(crate) latest_document_id: Option<Uuid>,
    pub(crate) latest_version_id: Option<Uuid>,
    pub(crate) last_rejection_code: Option<String>,
    pub(crate) last_rejection_reason: Option<String>,
    pub(crate) due_date: Option<NaiveDate>,
    pub(crate) created_at: DateTime<Utc>,
    pub(crate) updated_at: Option<DateTime<Utc>>,
    pub(crate) satisfied_at: Option<DateTime<Utc>>,
}

impl DocumentRequirement {
    /// Check if requirement is satisfied (verified or waived)
    pub(crate) fn is_satisfied(&self) -> bool {
        matches!(self.status.as_str(), "verified" | "waived")
    }

    /// Check if requirement can be retried (rejected but under max attempts)
    pub(crate) fn can_retry(&self) -> bool {
        self.status == "rejected" && self.attempt_count < self.max_attempts
    }

    /// Get parsed status
    pub(crate) fn parsed_status(&self) -> Result<RequirementState, RequirementStateError> {
        RequirementState::from_str(&self.status)
    }

    /// Get parsed required state
    pub(crate) fn parsed_required_state(&self) -> Result<RequirementState, RequirementStateError> {
        RequirementState::from_str(&self.required_state)
    }

    /// Check if current status satisfies the required state
    pub(crate) fn meets_requirement(&self) -> bool {
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
pub(crate) struct Document {
    pub(crate) document_id: Uuid,
    pub(crate) document_type: String,
    pub(crate) subject_entity_id: Option<Uuid>,
    pub(crate) subject_cbu_id: Option<Uuid>,
    pub(crate) parent_document_id: Option<Uuid>,
    pub(crate) requirement_id: Option<Uuid>,
    pub(crate) source: String,
    pub(crate) source_ref: Option<String>,
    pub(crate) created_at: DateTime<Utc>,
    pub(crate) created_by: Option<String>,
}

/// Layer C: Document version (immutable submission)
#[derive(Debug, Clone, Serialize, Deserialize)]
#[cfg_attr(feature = "database", derive(sqlx::FromRow))]
pub(crate) struct DocumentVersion {
    pub(crate) version_id: Uuid,
    pub(crate) document_id: Uuid,
    pub(crate) version_no: i32,
    pub(crate) content_type: String,
    pub(crate) structured_data: Option<serde_json::Value>,
    pub(crate) blob_ref: Option<String>,
    pub(crate) ocr_extracted: Option<serde_json::Value>,
    pub(crate) task_id: Option<Uuid>,
    pub(crate) verification_status: String,
    pub(crate) rejection_code: Option<String>,
    pub(crate) rejection_reason: Option<String>,
    pub(crate) verified_by: Option<String>,
    pub(crate) verified_at: Option<DateTime<Utc>>,
    pub(crate) valid_from: Option<NaiveDate>,
    pub(crate) valid_to: Option<NaiveDate>,
    pub(crate) quality_score: Option<f64>,
    pub(crate) extraction_confidence: Option<f64>,
    pub(crate) created_at: DateTime<Utc>,
    pub(crate) created_by: Option<String>,
}

impl DocumentVersion {
    /// Check if this version is verified
    pub(crate) fn is_verified(&self) -> bool {
        self.verification_status == "verified"
    }

    /// Check if this version is rejected
    pub(crate) fn is_rejected(&self) -> bool {
        self.verification_status == "rejected"
    }

    /// Check if the document validity has expired
    pub(crate) fn is_validity_expired(&self) -> bool {
        self.valid_to
            .map(|d| d < chrono::Utc::now().date_naive())
            .unwrap_or(false)
    }

    /// Get parsed verification status
    pub(crate) fn parsed_verification_status(&self) -> Option<VerificationStatus> {
        self.verification_status.parse().ok()
    }
}

/// Document with current status (joined view)
#[derive(Debug, Clone, Serialize, Deserialize)]
pub(crate) struct DocumentWithStatus {
    pub(crate) document_id: Uuid,
    pub(crate) document_type: String,
    pub(crate) subject_entity_id: Option<Uuid>,
    pub(crate) subject_cbu_id: Option<Uuid>,
    pub(crate) requirement_id: Option<Uuid>,
    pub(crate) source: String,
    pub(crate) source_ref: Option<String>,
    pub(crate) latest_version_id: Option<Uuid>,
    pub(crate) latest_version_no: Option<i32>,
    pub(crate) latest_status: Option<String>,
    pub(crate) verified_at: Option<DateTime<Utc>>,
    pub(crate) valid_from: Option<NaiveDate>,
    pub(crate) valid_to: Option<NaiveDate>,
    pub(crate) created_at: DateTime<Utc>,
}

/// Rejection reason code (from reference table)
#[derive(Debug, Clone, Serialize, Deserialize)]
#[cfg_attr(feature = "database", derive(sqlx::FromRow))]
pub(crate) struct RejectionCode {
    pub(crate) code: String,
    pub(crate) category: String,
    pub(crate) client_message: String,
    pub(crate) ops_message: String,
    pub(crate) next_action: String,
    pub(crate) is_retryable: bool,
}

impl RejectionCode {
    /// Generate full client-facing message with next action
    pub(crate) fn full_client_message(&self) -> String {
        format!("{} {}", self.client_message, self.next_action)
    }
}

/// Document event types for audit trail
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub(crate) enum DocumentEventType {
    Created,
    VersionUploaded,
    Verified,
    Rejected,
    Expired,
    StatusChanged,
}

impl DocumentEventType {
    pub(crate) fn as_str(&self) -> &'static str {
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
pub(crate) struct DocumentEvent {
    pub(crate) event_id: Uuid,
    pub(crate) document_id: Uuid,
    pub(crate) version_id: Option<Uuid>,
    pub(crate) event_type: String,
    pub(crate) old_status: Option<String>,
    pub(crate) new_status: Option<String>,
    pub(crate) rejection_code: Option<String>,
    pub(crate) notes: Option<String>,
    pub(crate) actor: Option<String>,
    pub(crate) occurred_at: DateTime<Utc>,
}

/// Unsatisfied requirement (for blocker generation)
#[derive(Debug, Clone, Serialize, Deserialize)]
#[cfg_attr(feature = "database", derive(sqlx::FromRow))]
pub(crate) struct UnsatisfiedRequirement {
    pub(crate) requirement_id: Uuid,
    pub(crate) doc_type: String,
    pub(crate) subject_entity_id: Option<Uuid>,
    pub(crate) status: String,
    pub(crate) required_state: String,
    pub(crate) attempt_count: i32,
    pub(crate) last_rejection_code: Option<String>,
    pub(crate) last_rejection_reason: Option<String>,
}

impl UnsatisfiedRequirement {
    /// Generate client-facing message for re-request using rejection code lookup
    pub(crate) fn rejection_message(
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
