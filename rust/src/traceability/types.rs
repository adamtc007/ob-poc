//! Core types for first-class utterance trace persistence.

use chrono::{DateTime, Utc};
use serde::{Deserialize, Serialize};
use uuid::Uuid;

/// Kind of trace node in a clarification/execution lineage.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum TraceKind {
    Original,
    ClarificationPrompt,
    ClarificationResponse,
    ResumedExecution,
}

impl TraceKind {
    /// Returns the database label for the trace kind.
    ///
    /// # Examples
    /// ```rust
    /// use ob_poc::traceability::TraceKind;
    ///
    /// assert_eq!(TraceKind::Original.as_str(), "original");
    /// ```
    pub fn as_str(self) -> &'static str {
        match self {
            Self::Original => "original",
            Self::ClarificationPrompt => "clarification_prompt",
            Self::ClarificationResponse => "clarification_response",
            Self::ResumedExecution => "resumed_execution",
        }
    }
}

/// Terminal outcome of a persisted utterance trace.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum TraceOutcome {
    InProgress,
    ExecutedSuccessfully,
    ExecutedWithCorrection,
    HaltedAtPhase,
    ClarificationTriggered,
    NoMatch,
}

impl TraceOutcome {
    /// Returns the database label for the trace outcome.
    ///
    /// # Examples
    /// ```rust
    /// use ob_poc::traceability::TraceOutcome;
    ///
    /// assert_eq!(TraceOutcome::InProgress.as_str(), "in_progress");
    /// ```
    pub fn as_str(self) -> &'static str {
        match self {
            Self::InProgress => "in_progress",
            Self::ExecutedSuccessfully => "executed_successfully",
            Self::ExecutedWithCorrection => "executed_with_correction",
            Self::HaltedAtPhase => "halted_at_phase",
            Self::ClarificationTriggered => "clarification_triggered",
            Self::NoMatch => "no_match",
        }
    }
}

/// Version pins captured at utterance resolution time.
#[derive(Debug, Clone, Default, Serialize, Deserialize)]
pub struct SurfaceVersions {
    pub verb_surface_version: Option<String>,
    pub concept_registry_version: Option<String>,
    pub entity_fsm_version: Option<String>,
    pub constellation_template_version: Option<String>,
    pub embedding_model_version: Option<String>,
    pub threshold_policy_version: Option<String>,
    pub parser_version: Option<String>,
    pub macro_compiler_version: Option<String>,
    pub pattern_catalogue_version: Option<String>,
}

impl SurfaceVersions {
    /// Builds a minimal runtime version snapshot from the currently loaded binary.
    ///
    /// # Examples
    /// ```rust
    /// use ob_poc::traceability::SurfaceVersions;
    ///
    /// let versions = SurfaceVersions::current_defaults();
    /// assert!(versions.parser_version.is_some());
    /// ```
    pub fn current_defaults() -> Self {
        Self {
            verb_surface_version: Some("session_verb_surface/v1".to_string()),
            concept_registry_version: None,
            entity_fsm_version: None,
            constellation_template_version: None,
            #[cfg(feature = "database")]
            embedding_model_version: Some(
                crate::agent::learning::embedder::EMBEDDING_MODEL_VERSION.to_string(),
            ),
            #[cfg(not(feature = "database"))]
            embedding_model_version: None,
            threshold_policy_version: Some("policy_gate/v1".to_string()),
            parser_version: Some(env!("CARGO_PKG_VERSION").to_string()),
            macro_compiler_version: Some(env!("CARGO_PKG_VERSION").to_string()),
            pattern_catalogue_version: None,
        }
    }
}

/// Persisted utterance trace row.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct UtteranceTraceRecord {
    pub trace_id: Uuid,
    pub utterance_id: Uuid,
    pub session_id: Uuid,
    pub correlation_id: Option<Uuid>,
    pub trace_kind: TraceKind,
    pub parent_trace_id: Option<Uuid>,
    pub timestamp: DateTime<Utc>,
    pub raw_utterance: String,
    pub is_synthetic: bool,
    pub outcome: TraceOutcome,
    pub halt_reason_code: Option<String>,
    pub halt_phase: Option<i16>,
    pub resolved_verb: Option<String>,
    pub plane: Option<String>,
    pub polarity: Option<String>,
    pub execution_shape_kind: Option<String>,
    pub fallback_invoked: bool,
    pub fallback_reason_code: Option<String>,
    pub situation_signature_hash: Option<i64>,
    pub template_id: Option<String>,
    pub template_version: Option<String>,
    pub surface_versions: SurfaceVersions,
    pub trace_payload: serde_json::Value,
}

/// Insertable utterance trace payload.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct NewUtteranceTrace {
    pub trace_id: Uuid,
    pub utterance_id: Uuid,
    pub session_id: Uuid,
    pub correlation_id: Option<Uuid>,
    pub trace_kind: TraceKind,
    pub parent_trace_id: Option<Uuid>,
    pub timestamp: DateTime<Utc>,
    pub raw_utterance: String,
    pub is_synthetic: bool,
    pub outcome: TraceOutcome,
    pub halt_reason_code: Option<String>,
    pub halt_phase: Option<i16>,
    pub resolved_verb: Option<String>,
    pub plane: Option<String>,
    pub polarity: Option<String>,
    pub execution_shape_kind: Option<String>,
    pub fallback_invoked: bool,
    pub fallback_reason_code: Option<String>,
    pub situation_signature_hash: Option<i64>,
    pub template_id: Option<String>,
    pub template_version: Option<String>,
    pub surface_versions: SurfaceVersions,
    pub trace_payload: serde_json::Value,
}

impl NewUtteranceTrace {
    /// Creates an in-progress trace scaffold for a raw utterance.
    ///
    /// # Examples
    /// ```rust
    /// use ob_poc::traceability::{NewUtteranceTrace, TraceKind};
    /// use uuid::Uuid;
    ///
    /// let trace = NewUtteranceTrace::in_progress(
    ///     Uuid::nil(),
    ///     Uuid::nil(),
    ///     "show me the case graph",
    ///     TraceKind::Original,
    ///     false,
    /// );
    /// assert_eq!(trace.raw_utterance, "show me the case graph");
    /// ```
    pub fn in_progress(
        session_id: Uuid,
        utterance_id: Uuid,
        raw_utterance: impl Into<String>,
        trace_kind: TraceKind,
        is_synthetic: bool,
    ) -> Self {
        Self {
            trace_id: Uuid::new_v4(),
            utterance_id,
            session_id,
            correlation_id: None,
            trace_kind,
            parent_trace_id: None,
            timestamp: Utc::now(),
            raw_utterance: raw_utterance.into(),
            is_synthetic,
            outcome: TraceOutcome::InProgress,
            halt_reason_code: None,
            halt_phase: None,
            resolved_verb: None,
            plane: None,
            polarity: None,
            execution_shape_kind: None,
            fallback_invoked: false,
            fallback_reason_code: None,
            situation_signature_hash: None,
            template_id: None,
            template_version: None,
            surface_versions: SurfaceVersions::current_defaults(),
            trace_payload: serde_json::json!({}),
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn in_progress_trace_preserves_synthetic_flag() {
        let trace = NewUtteranceTrace::in_progress(
            Uuid::nil(),
            Uuid::nil(),
            "show me the case",
            TraceKind::Original,
            true,
        );
        assert!(trace.is_synthetic);
        assert_eq!(trace.outcome, TraceOutcome::InProgress);
    }
}
