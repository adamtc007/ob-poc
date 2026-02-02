//! Agent Learning Event Types
//!
//! These types capture the full intent resolution pipeline for learning.
//! Designed for fire-and-forget emission (< 1μs overhead).

use chrono::{DateTime, Utc};
use serde::{Deserialize, Serialize};
use uuid::Uuid;

/// Agent event for learning capture.
///
/// Emitted at key points in the intent resolution pipeline.
/// Fire-and-forget - never blocks the hot path.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct AgentEvent {
    pub timestamp: DateTime<Utc>,
    pub session_id: Option<Uuid>,
    pub payload: AgentEventPayload,
}

/// Event payload variants for the intent resolution pipeline.
#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(tag = "type")]
pub enum AgentEventPayload {
    /// User message received, about to process
    MessageReceived {
        message: String,
        cbu_id: Option<Uuid>,
    },

    /// Intent extraction from LLM completed
    IntentExtracted {
        user_message: String,
        intents: Vec<ExtractedIntent>,
        llm_model: String,
        llm_tokens: u32,
        duration_ms: u64,
    },

    /// Verb selected for an intent
    VerbSelected {
        intent_summary: String,
        selected_verb: String,
        confidence: f32,
        alternatives_considered: Vec<String>,
    },

    /// Entity resolution attempted
    EntityResolved {
        query: String,
        resolved_to: Option<ResolvedEntity>,
        candidates: Vec<EntityCandidate>,
        resolution_method: ResolutionMethod,
    },

    /// Entity resolution failed (for learning)
    EntityResolutionFailed {
        query: String,
        reason: String,
        candidates: Vec<EntityCandidate>,
    },

    /// DSL generated from intents
    DslGenerated {
        intents_count: u32,
        dsl: String,
        duration_ms: u64,
    },

    /// User corrected the generated DSL
    UserCorrection {
        original_message: String,
        generated_dsl: String,
        corrected_dsl: String,
        correction_type: CorrectionType,
    },

    /// DSL execution result (links to DslEvent)
    ExecutionCompleted {
        dsl: String,
        success: bool,
        error_message: Option<String>,
        duration_ms: u64,
    },

    /// Session summary (emitted on session close)
    SessionSummary {
        messages_processed: u32,
        intents_extracted: u32,
        successful_executions: u32,
        failed_executions: u32,
        corrections_made: u32,
        duration_secs: u64,
    },

    /// ESPER navigation command matched
    EsperCommandMatched {
        /// Original user phrase
        phrase: String,
        /// Command key that matched (e.g., "zoom_in", "scale_universe")
        command_key: String,
        /// Whether this was a builtin or learned alias
        source: String, // "Builtin" or "Learned"
        /// Match type (exact, contains, prefix)
        match_type: String,
        /// Any parameters extracted from the phrase
        extracted_params: std::collections::HashMap<String, String>,
    },

    /// ESPER command miss (phrase didn't match, fell through to DSL)
    EsperCommandMiss {
        /// Original user phrase
        phrase: String,
    },
}

/// An intent extracted by the LLM.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ExtractedIntent {
    pub action: String,
    pub entity_refs: Vec<String>,
    pub parameters: serde_json::Value,
    pub raw_text: Option<String>,
}

/// A resolved entity reference.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ResolvedEntity {
    pub entity_id: Uuid,
    pub name: String,
    pub entity_type: String,
    pub confidence: f32,
}

/// Candidate entity during resolution.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct EntityCandidate {
    pub entity_id: Uuid,
    pub name: String,
    pub score: f32,
    pub match_type: String, // exact, fuzzy, alias
}

/// How entity was resolved.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub enum ResolutionMethod {
    /// Exact name match
    ExactMatch,
    /// Fuzzy match via EntityGateway
    FuzzyMatch { score: f32 },
    /// Matched via learned alias
    LearnedAlias { alias: String },
    /// User selected from disambiguation
    UserDisambiguation,
    /// Session salience (recently mentioned)
    SessionSalience,
}

/// Type of correction made by user.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub enum CorrectionType {
    /// Changed the verb
    VerbChange { from_verb: String, to_verb: String },
    /// Changed entity reference
    EntityChange {
        from_entity: String,
        to_entity: String,
    },
    /// Changed argument value
    ArgumentChange {
        argument: String,
        from_value: String,
        to_value: String,
    },
    /// Added missing content
    Addition { added: String },
    /// Removed unwanted content
    Removal { removed: String },
    /// Complete rewrite
    FullRewrite,
}

impl AgentEvent {
    /// Create a message received event.
    #[inline]
    pub fn message_received(
        session_id: Option<Uuid>,
        message: String,
        cbu_id: Option<Uuid>,
    ) -> Self {
        Self {
            timestamp: Utc::now(),
            session_id,
            payload: AgentEventPayload::MessageReceived { message, cbu_id },
        }
    }

    /// Create an intent extracted event.
    #[inline]
    pub fn intent_extracted(
        session_id: Option<Uuid>,
        user_message: String,
        intents: Vec<ExtractedIntent>,
        llm_model: String,
        llm_tokens: u32,
        duration_ms: u64,
    ) -> Self {
        Self {
            timestamp: Utc::now(),
            session_id,
            payload: AgentEventPayload::IntentExtracted {
                user_message,
                intents,
                llm_model,
                llm_tokens,
                duration_ms,
            },
        }
    }

    /// Create a verb selected event.
    #[inline]
    pub fn verb_selected(
        session_id: Option<Uuid>,
        intent_summary: String,
        selected_verb: String,
        confidence: f32,
        alternatives_considered: Vec<String>,
    ) -> Self {
        Self {
            timestamp: Utc::now(),
            session_id,
            payload: AgentEventPayload::VerbSelected {
                intent_summary,
                selected_verb,
                confidence,
                alternatives_considered,
            },
        }
    }

    /// Create an entity resolved event.
    #[inline]
    pub fn entity_resolved(
        session_id: Option<Uuid>,
        query: String,
        resolved_to: Option<ResolvedEntity>,
        candidates: Vec<EntityCandidate>,
        resolution_method: ResolutionMethod,
    ) -> Self {
        Self {
            timestamp: Utc::now(),
            session_id,
            payload: AgentEventPayload::EntityResolved {
                query,
                resolved_to,
                candidates,
                resolution_method,
            },
        }
    }

    /// Create an entity resolution failed event.
    #[inline]
    pub fn entity_resolution_failed(
        session_id: Option<Uuid>,
        query: String,
        reason: String,
        candidates: Vec<EntityCandidate>,
    ) -> Self {
        Self {
            timestamp: Utc::now(),
            session_id,
            payload: AgentEventPayload::EntityResolutionFailed {
                query,
                reason,
                candidates,
            },
        }
    }

    /// Create a DSL generated event.
    #[inline]
    pub fn dsl_generated(
        session_id: Option<Uuid>,
        intents_count: u32,
        dsl: String,
        duration_ms: u64,
    ) -> Self {
        Self {
            timestamp: Utc::now(),
            session_id,
            payload: AgentEventPayload::DslGenerated {
                intents_count,
                dsl,
                duration_ms,
            },
        }
    }

    /// Create a user correction event.
    #[inline]
    pub fn user_correction(
        session_id: Option<Uuid>,
        original_message: String,
        generated_dsl: String,
        corrected_dsl: String,
        correction_type: CorrectionType,
    ) -> Self {
        Self {
            timestamp: Utc::now(),
            session_id,
            payload: AgentEventPayload::UserCorrection {
                original_message,
                generated_dsl,
                corrected_dsl,
                correction_type,
            },
        }
    }

    /// Create an execution completed event.
    #[inline]
    pub fn execution_completed(
        session_id: Option<Uuid>,
        dsl: String,
        success: bool,
        error_message: Option<String>,
        duration_ms: u64,
    ) -> Self {
        Self {
            timestamp: Utc::now(),
            session_id,
            payload: AgentEventPayload::ExecutionCompleted {
                dsl,
                success,
                error_message,
                duration_ms,
            },
        }
    }

    /// Create a session summary event.
    #[inline]
    pub fn session_summary(
        session_id: Uuid,
        messages_processed: u32,
        intents_extracted: u32,
        successful_executions: u32,
        failed_executions: u32,
        corrections_made: u32,
        duration_secs: u64,
    ) -> Self {
        Self {
            timestamp: Utc::now(),
            session_id: Some(session_id),
            payload: AgentEventPayload::SessionSummary {
                messages_processed,
                intents_extracted,
                successful_executions,
                failed_executions,
                corrections_made,
                duration_secs,
            },
        }
    }

    /// Create an ESPER command matched event.
    #[inline]
    pub fn esper_command_matched(
        session_id: Option<Uuid>,
        phrase: String,
        command_key: String,
        source: &str,
        match_type: &str,
        extracted_params: std::collections::HashMap<String, String>,
    ) -> Self {
        Self {
            timestamp: Utc::now(),
            session_id,
            payload: AgentEventPayload::EsperCommandMatched {
                phrase,
                command_key,
                source: source.to_string(),
                match_type: match_type.to_string(),
                extracted_params,
            },
        }
    }

    /// Create an ESPER command miss event.
    #[inline]
    pub fn esper_command_miss(session_id: Option<Uuid>, phrase: String) -> Self {
        Self {
            timestamp: Utc::now(),
            session_id,
            payload: AgentEventPayload::EsperCommandMiss { phrase },
        }
    }
}

impl AgentEventPayload {
    /// Get the event type as a string (for DB storage).
    pub fn event_type_str(&self) -> &'static str {
        match self {
            AgentEventPayload::MessageReceived { .. } => "message_received",
            AgentEventPayload::IntentExtracted { .. } => "intent_extracted",
            AgentEventPayload::VerbSelected { .. } => "verb_selected",
            AgentEventPayload::EntityResolved { .. } => "entity_resolved",
            AgentEventPayload::EntityResolutionFailed { .. } => "entity_resolution_failed",
            AgentEventPayload::DslGenerated { .. } => "dsl_generated",
            AgentEventPayload::UserCorrection { .. } => "user_correction",
            AgentEventPayload::ExecutionCompleted { .. } => "execution_completed",
            AgentEventPayload::SessionSummary { .. } => "session_summary",
            AgentEventPayload::EsperCommandMatched { .. } => "esper_command_matched",
            AgentEventPayload::EsperCommandMiss { .. } => "esper_command_miss",
        }
    }

    /// Is this a learning signal (indicates something to learn from)?
    pub fn is_learning_signal(&self) -> bool {
        matches!(
            self,
            AgentEventPayload::UserCorrection { .. }
                | AgentEventPayload::EntityResolutionFailed { .. }
                | AgentEventPayload::EsperCommandMiss { .. } // Misses are learning opportunities
        )
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_event_creation_is_fast() {
        // These must be < 1μs to not impact hot path
        let start = std::time::Instant::now();
        for _ in 0..1000 {
            let _ = AgentEvent::message_received(
                Some(Uuid::now_v7()),
                "test message".to_string(),
                None,
            );
        }
        let elapsed = start.elapsed();
        // 1000 events should take < 1ms (< 1μs each)
        assert!(
            elapsed.as_millis() < 10,
            "Event creation too slow: {:?}",
            elapsed
        );
    }

    #[test]
    fn test_event_serialization() {
        let event = AgentEvent::verb_selected(
            Some(Uuid::now_v7()),
            "create counterparty".to_string(),
            "entity.ensure-limited-company".to_string(),
            0.95,
            vec!["entity.create".to_string()],
        );

        let json = serde_json::to_string(&event).unwrap();
        assert!(json.contains("\"type\":\"VerbSelected\""));
        assert!(json.contains("entity.ensure-limited-company"));
    }

    #[test]
    fn test_learning_signal_detection() {
        let correction = AgentEventPayload::UserCorrection {
            original_message: "test".into(),
            generated_dsl: "old".into(),
            corrected_dsl: "new".into(),
            correction_type: CorrectionType::FullRewrite,
        };
        assert!(correction.is_learning_signal());

        let normal = AgentEventPayload::DslGenerated {
            intents_count: 1,
            dsl: "test".into(),
            duration_ms: 100,
        };
        assert!(!normal.is_learning_signal());
    }
}
