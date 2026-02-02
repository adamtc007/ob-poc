//! Research context for session state management
//!
//! Tracks research macro execution state across conversation turns.
//! Implements a state machine: Idle → PendingReview → VerbsReady → Executed

use std::collections::HashMap;

use chrono::Utc;
use serde::{Deserialize, Serialize};
use serde_json::Value;
use uuid::Uuid;

use crate::research::{ApprovedResearch, ResearchResult};

/// Research state within a session
#[derive(Debug, Clone, Default, Serialize, Deserialize)]
pub struct ResearchContext {
    /// Current pending result awaiting review
    pub pending: Option<ResearchResult>,

    /// History of approved research results (keyed by result_id)
    pub approved: HashMap<Uuid, ApprovedResearch>,

    /// Generated verbs from most recent approval (ready for execution)
    pub generated_verbs: Option<String>,

    /// Current state in research workflow
    pub state: ResearchState,
}

/// Research workflow state machine
#[derive(Debug, Clone, Copy, Default, Serialize, Deserialize, PartialEq, Eq)]
#[serde(rename_all = "snake_case")]
pub enum ResearchState {
    /// No active research
    #[default]
    Idle,

    /// Research executed, awaiting human review
    PendingReview,

    /// Research approved, verbs generated and ready
    VerbsReady,

    /// Verbs have been executed
    Executed,
}

impl std::fmt::Display for ResearchState {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            ResearchState::Idle => write!(f, "idle"),
            ResearchState::PendingReview => write!(f, "pending_review"),
            ResearchState::VerbsReady => write!(f, "verbs_ready"),
            ResearchState::Executed => write!(f, "executed"),
        }
    }
}

impl ResearchContext {
    /// Create a new empty research context
    pub fn new() -> Self {
        Self::default()
    }

    /// Set pending research result (transitions to PendingReview)
    pub fn set_pending(&mut self, result: ResearchResult) {
        self.pending = Some(result);
        self.state = ResearchState::PendingReview;
        self.generated_verbs = None;
    }

    /// Approve pending result with optional edits
    ///
    /// Transitions: PendingReview → VerbsReady
    ///
    /// # Arguments
    /// * `edits` - Optional edited data to replace the original
    ///
    /// # Returns
    /// Reference to the approved research, or error if no pending research
    pub fn approve(&mut self, edits: Option<Value>) -> Result<&ApprovedResearch, &'static str> {
        let edits_made = edits.is_some();
        let result = self
            .pending
            .take()
            .ok_or("No pending research to approve")?;

        let approved_data = edits.unwrap_or_else(|| result.data.clone());
        let generated_verbs = result.suggested_verbs.clone().unwrap_or_default();

        let approved = ApprovedResearch {
            result_id: result.result_id,
            approved_at: Utc::now(),
            approved_data,
            generated_verbs: generated_verbs.clone(),
            edits_made,
        };

        self.generated_verbs = Some(generated_verbs);
        self.state = ResearchState::VerbsReady;
        self.approved.insert(result.result_id, approved);

        Ok(self.approved.get(&result.result_id).unwrap())
    }

    /// Reject pending result (transitions to Idle)
    pub fn reject(&mut self) {
        self.pending = None;
        self.state = ResearchState::Idle;
    }

    /// Mark verbs as executed (transitions to Executed)
    pub fn mark_executed(&mut self) {
        self.generated_verbs = None;
        self.state = ResearchState::Executed;
    }

    /// Clear and return to idle state
    pub fn clear(&mut self) {
        self.pending = None;
        self.generated_verbs = None;
        self.state = ResearchState::Idle;
    }

    /// Get the most recently approved research
    pub fn last_approved(&self) -> Option<&ApprovedResearch> {
        self.approved.values().max_by_key(|a| a.approved_at)
    }

    /// Get approved research by ID
    pub fn get_approved(&self, result_id: Uuid) -> Option<&ApprovedResearch> {
        self.approved.get(&result_id)
    }

    /// Check if there's pending research
    pub fn has_pending(&self) -> bool {
        self.pending.is_some()
    }

    /// Check if verbs are ready for execution
    pub fn has_verbs_ready(&self) -> bool {
        self.generated_verbs.is_some()
    }

    /// Get pending macro name
    pub fn pending_macro_name(&self) -> Option<&str> {
        self.pending.as_ref().map(|r| r.macro_name.as_str())
    }

    /// Get count of approved research results
    pub fn approved_count(&self) -> usize {
        self.approved.len()
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::research::SearchQuality;

    fn create_test_result() -> ResearchResult {
        ResearchResult {
            result_id: Uuid::now_v7(),
            macro_name: "client-discovery".to_string(),
            params: serde_json::json!({"client_name": "Test Corp"}),
            data: serde_json::json!({"apex": {"name": "Test Corp"}}),
            schema_valid: true,
            validation_errors: vec![],
            review_required: true,
            suggested_verbs: Some("(gleif.enrich :lei \"123\")".to_string()),
            search_quality: Some(SearchQuality::High),
            sources: vec![],
            created_at: Utc::now(),
        }
    }

    #[test]
    fn test_new_context_is_idle() {
        let ctx = ResearchContext::new();
        assert_eq!(ctx.state, ResearchState::Idle);
        assert!(!ctx.has_pending());
        assert!(!ctx.has_verbs_ready());
    }

    #[test]
    fn test_set_pending_transitions_to_pending_review() {
        let mut ctx = ResearchContext::new();
        let result = create_test_result();

        ctx.set_pending(result);

        assert_eq!(ctx.state, ResearchState::PendingReview);
        assert!(ctx.has_pending());
        assert_eq!(ctx.pending_macro_name(), Some("client-discovery"));
    }

    #[test]
    fn test_approve_transitions_to_verbs_ready() {
        let mut ctx = ResearchContext::new();
        ctx.set_pending(create_test_result());

        // Extract edits_made before dropping the borrow
        let edits_made = {
            let approved = ctx.approve(None).unwrap();
            approved.edits_made
        };

        assert_eq!(ctx.state, ResearchState::VerbsReady);
        assert!(!ctx.has_pending());
        assert!(ctx.has_verbs_ready());
        assert!(!edits_made);
        assert_eq!(ctx.approved_count(), 1);
    }

    #[test]
    fn test_approve_with_edits() {
        let mut ctx = ResearchContext::new();
        ctx.set_pending(create_test_result());

        let edits = serde_json::json!({"apex": {"name": "Edited Corp"}});

        // Extract values before dropping the borrow
        let (edits_made, approved_data) = {
            let approved = ctx.approve(Some(edits.clone())).unwrap();
            (approved.edits_made, approved.approved_data.clone())
        };

        assert!(edits_made);
        assert_eq!(approved_data, edits);
    }

    #[test]
    fn test_reject_transitions_to_idle() {
        let mut ctx = ResearchContext::new();
        ctx.set_pending(create_test_result());

        ctx.reject();

        assert_eq!(ctx.state, ResearchState::Idle);
        assert!(!ctx.has_pending());
    }

    #[test]
    fn test_mark_executed_transitions_to_executed() {
        let mut ctx = ResearchContext::new();
        ctx.set_pending(create_test_result());
        ctx.approve(None).unwrap();

        ctx.mark_executed();

        assert_eq!(ctx.state, ResearchState::Executed);
        assert!(!ctx.has_verbs_ready());
    }

    #[test]
    fn test_approve_without_pending_fails() {
        let mut ctx = ResearchContext::new();

        let result = ctx.approve(None);

        assert!(result.is_err());
        assert_eq!(result.unwrap_err(), "No pending research to approve");
    }

    #[test]
    fn test_clear_returns_to_idle() {
        let mut ctx = ResearchContext::new();
        ctx.set_pending(create_test_result());
        ctx.approve(None).unwrap();

        ctx.clear();

        assert_eq!(ctx.state, ResearchState::Idle);
        assert!(!ctx.has_pending());
        assert!(!ctx.has_verbs_ready());
        // Approved history is preserved
        assert_eq!(ctx.approved_count(), 1);
    }

    #[test]
    fn test_state_display() {
        assert_eq!(format!("{}", ResearchState::Idle), "idle");
        assert_eq!(
            format!("{}", ResearchState::PendingReview),
            "pending_review"
        );
        assert_eq!(format!("{}", ResearchState::VerbsReady), "verbs_ready");
        assert_eq!(format!("{}", ResearchState::Executed), "executed");
    }
}
