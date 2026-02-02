//! Runbook Events and Audit Types
//!
//! Every tool boundary produces an auditable event.
//! This is the "bank-grade" story: deterministic trace, not magic.
//!
//! # Audit Points
//!
//! | Boundary | Event | Logged Fields |
//! |----------|-------|---------------|
//! | Scope Resolution | `ScopeResolved` | client_group_id, persona, method, confidence |
//! | Intent Classification | `IntentClassified` | intent_type, verb_candidates, confidence |
//! | Runbook Change | `CommandStaged/Removed/Edited` | command_id, diff_hash |
//! | Entity Resolution | `ResolutionAmbiguous/Failed` | unresolved, candidate_count |
//! | Ready Gate | `RunbookReady/NotReady` | can_run, blockers |
//! | Execution | `CommandExecuted` | runbook_id, exec_id, per-line results |

use serde::{Deserialize, Serialize};
use uuid::Uuid;

use super::staged_runbook::{ResolutionStatus, RunbookStatus};

// ============================================================================
// MCP Events (server → client)
// ============================================================================

/// Events emitted by the runbook system
#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(tag = "type", rename_all = "snake_case")]
pub enum RunbookEvent {
    // ========================================================================
    // Scope Resolution Events
    // ========================================================================
    /// Scope resolved for session
    ScopeResolved {
        session_id: String,
        client_group_id: Option<Uuid>,
        client_group_name: Option<String>,
        persona: Option<String>,
        method: ScopeResolutionMethod,
        /// Confidence score (f64 to match PostgreSQL FLOAT)
        confidence: f64,
    },

    // ========================================================================
    // Staging Events
    // ========================================================================
    /// Command staged successfully
    CommandStaged {
        runbook_id: Uuid,
        command: StagedCommandSummary,
        runbook_summary: RunbookSummary,
        /// SHA-256 of canonical DSL for audit
        dsl_hash: String,
    },

    /// Staging failed (parse error or invalid verb)
    StageFailed {
        runbook_id: Uuid,
        /// "parse_failed" | "invalid_verb" | "no_client_group"
        error_kind: String,
        error: String,
        dsl_raw: String,
    },

    /// Command removed
    CommandRemoved {
        runbook_id: Uuid,
        command_id: Uuid,
        source_order: i32,
        /// Dependents also removed (cascade)
        cascade_removed: Vec<Uuid>,
        runbook_summary: RunbookSummary,
    },

    /// Command edited
    CommandEdited {
        runbook_id: Uuid,
        command_id: Uuid,
        old_dsl_hash: String,
        new_dsl_hash: String,
        resolution_status: ResolutionStatus,
    },

    // ========================================================================
    // Resolution Events (where hallucination dies)
    // ========================================================================
    /// Resolution needs user input (picker required)
    /// CRITICAL: Agent MUST call runbook_pick with entity_ids from candidates
    ResolutionAmbiguous {
        runbook_id: Uuid,
        command_id: Uuid,
        arg_name: String,
        original_ref: String,
        /// Candidates from DB search - ONLY these can be picked
        candidates: Vec<PickerCandidate>,
        /// Audit: how many candidates found
        candidate_count: usize,
    },

    /// Resolution failed (no matches found)
    ResolutionFailed {
        runbook_id: Uuid,
        command_id: Uuid,
        arg_name: String,
        original_ref: String,
        error: String,
        /// Audit: search method used
        search_method: SearchMethod,
    },

    /// Entity resolved successfully
    EntityResolved {
        runbook_id: Uuid,
        command_id: Uuid,
        arg_name: String,
        original_ref: String,
        resolved_entity_id: Uuid,
        resolved_entity_name: String,
        resolution_source: String,
        /// Confidence score (f64 to match PostgreSQL FLOAT)
        confidence: f64,
    },

    /// Picker selection applied
    PickerApplied {
        runbook_id: Uuid,
        command_id: Uuid,
        arg_name: String,
        selected_entity_ids: Vec<Uuid>,
        /// Audit: selection was from candidate set
        from_candidate_set: bool,
    },

    // ========================================================================
    // Ready Gate Events
    // ========================================================================
    /// Runbook ready for execution
    RunbookReady {
        runbook_id: Uuid,
        summary: RunbookSummary,
        entity_footprint: Vec<EntityFootprintEntry>,
        /// If DAG reordered commands, show the diff
        reorder_diff: Option<ReorderDiff>,
    },

    /// Run rejected - runbook not ready
    RunbookNotReady {
        runbook_id: Uuid,
        blocking_commands: Vec<BlockingCommand>,
    },

    /// Runbook aborted (all commands cleared)
    RunbookAborted { runbook_id: Uuid },

    // ========================================================================
    // Execution Events
    // ========================================================================
    /// Execution started
    ExecutionStarted {
        runbook_id: Uuid,
        execution_id: Uuid,
        total_commands: usize,
        /// Audit: execution order
        dag_order: Vec<Uuid>,
    },

    /// Single command executed
    CommandExecuted {
        runbook_id: Uuid,
        execution_id: Uuid,
        command_id: Uuid,
        dag_order: i32,
        result: CommandResult,
    },

    /// All commands executed
    ExecutionCompleted {
        runbook_id: Uuid,
        execution_id: Uuid,
        results: Vec<CommandResult>,
        /// Tags learned from successful resolutions
        learned_tags: Vec<LearnedTag>,
        /// Audit: total duration
        duration_ms: u64,
    },

    /// Execution failed
    ExecutionFailed {
        runbook_id: Uuid,
        execution_id: Uuid,
        failed_command_id: Uuid,
        error: String,
        /// Commands that were skipped due to failure
        skipped_commands: Vec<Uuid>,
    },
}

// ============================================================================
// Audit Types
// ============================================================================

/// How scope was resolved
#[derive(Debug, Clone, Copy, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum ScopeResolutionMethod {
    /// Explicit client group ID provided
    Explicit,
    /// Resolved from session context
    SessionContext,
    /// Resolved from entity hint
    EntityHint,
    /// No scope (global)
    NoScope,
}

/// Search method used for entity resolution
#[derive(Debug, Clone, Copy, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum SearchMethod {
    /// Exact tag match
    ExactTag,
    /// Trigram fuzzy match
    FuzzyTag,
    /// Candle semantic embedding
    SemanticEmbedding,
    /// Combined (tried multiple methods)
    Combined,
}

/// Command blocking execution
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct BlockingCommand {
    pub command_id: Uuid,
    pub source_order: i32,
    pub status: ResolutionStatus,
    pub error: Option<String>,
}

/// Summary of a staged command (for events)
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct StagedCommandSummary {
    pub id: Uuid,
    pub source_order: i32,
    pub verb: String,
    pub description: Option<String>,
    pub resolution_status: ResolutionStatus,
    pub entity_count: usize,
}

/// Summary of runbook state (for events)
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct RunbookSummary {
    pub id: Uuid,
    pub status: RunbookStatus,
    pub command_count: usize,
    pub resolved_count: usize,
    pub pending_count: usize,
    pub ambiguous_count: usize,
    pub failed_count: usize,
}

/// Picker candidate - ONLY these can be selected
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct PickerCandidate {
    pub entity_id: Uuid,
    pub entity_name: String,
    pub matched_tag: Option<String>,
    /// Confidence score (f64 to match PostgreSQL FLOAT)
    pub confidence: f64,
    pub match_type: String,
}

/// Entity in the footprint (what will be touched)
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct EntityFootprintEntry {
    pub entity_id: Uuid,
    pub entity_name: String,
    /// Which commands touch this entity
    pub commands: Vec<Uuid>,
    /// Verbs applied to this entity
    pub operations: Vec<String>,
}

/// DAG reorder diff (transparency)
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ReorderDiff {
    pub original_order: Vec<Uuid>,
    pub reordered: Vec<Uuid>,
    pub moves: Vec<ReorderMove>,
}

/// Single reorder move
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ReorderMove {
    pub command_id: Uuid,
    pub from_position: usize,
    pub to_position: usize,
    pub reason: String,
}

/// Result of executing a single command
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct CommandResult {
    pub command_id: Uuid,
    pub success: bool,
    pub error: Option<String>,
    /// Output value (if any)
    pub output: Option<serde_json::Value>,
    /// Duration in milliseconds
    pub duration_ms: u64,
}

/// Tag learned from successful resolution
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct LearnedTag {
    pub entity_id: Uuid,
    pub tag: String,
    pub source: String,
}

// ============================================================================
// Audit Log Entry (for DB persistence)
// ============================================================================

/// Structured audit log entry
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct AuditLogEntry {
    /// Unique audit entry ID
    pub id: Uuid,
    /// Timestamp
    pub timestamp: chrono::DateTime<chrono::Utc>,
    /// Session ID
    pub session_id: String,
    /// Runbook ID (if applicable)
    pub runbook_id: Option<Uuid>,
    /// Audit category
    pub category: AuditCategory,
    /// Event type (from RunbookEvent tag)
    pub event_type: String,
    /// Full event payload (JSON)
    pub payload: serde_json::Value,
}

/// Audit categories for filtering
#[derive(Debug, Clone, Copy, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum AuditCategory {
    /// Scope resolution
    ScopeResolution,
    /// Intent classification
    IntentClassification,
    /// Runbook changes (stage/edit/remove)
    RunbookChange,
    /// Entity resolution (resolve/ambiguous/failed)
    EntityResolution,
    /// Ready gate (ready/not ready)
    ReadyGate,
    /// Execution (start/per-command/complete/fail)
    Execution,
}

impl RunbookEvent {
    /// Get the audit category for this event
    pub fn category(&self) -> AuditCategory {
        match self {
            Self::ScopeResolved { .. } => AuditCategory::ScopeResolution,
            Self::CommandStaged { .. }
            | Self::StageFailed { .. }
            | Self::CommandRemoved { .. }
            | Self::CommandEdited { .. }
            | Self::RunbookAborted { .. } => AuditCategory::RunbookChange,
            Self::ResolutionAmbiguous { .. }
            | Self::ResolutionFailed { .. }
            | Self::EntityResolved { .. }
            | Self::PickerApplied { .. } => AuditCategory::EntityResolution,
            Self::RunbookReady { .. } | Self::RunbookNotReady { .. } => AuditCategory::ReadyGate,
            Self::ExecutionStarted { .. }
            | Self::CommandExecuted { .. }
            | Self::ExecutionCompleted { .. }
            | Self::ExecutionFailed { .. } => AuditCategory::Execution,
        }
    }

    /// Get event type name for logging
    pub fn event_type(&self) -> &'static str {
        match self {
            Self::ScopeResolved { .. } => "scope_resolved",
            Self::CommandStaged { .. } => "command_staged",
            Self::StageFailed { .. } => "stage_failed",
            Self::CommandRemoved { .. } => "command_removed",
            Self::CommandEdited { .. } => "command_edited",
            Self::ResolutionAmbiguous { .. } => "resolution_ambiguous",
            Self::ResolutionFailed { .. } => "resolution_failed",
            Self::EntityResolved { .. } => "entity_resolved",
            Self::PickerApplied { .. } => "picker_applied",
            Self::RunbookReady { .. } => "runbook_ready",
            Self::RunbookNotReady { .. } => "runbook_not_ready",
            Self::RunbookAborted { .. } => "runbook_aborted",
            Self::ExecutionStarted { .. } => "execution_started",
            Self::CommandExecuted { .. } => "command_executed",
            Self::ExecutionCompleted { .. } => "execution_completed",
            Self::ExecutionFailed { .. } => "execution_failed",
        }
    }

    /// Create an audit log entry from this event
    pub fn to_audit_entry(&self, session_id: &str, runbook_id: Option<Uuid>) -> AuditLogEntry {
        AuditLogEntry {
            id: Uuid::now_v7(),
            timestamp: chrono::Utc::now(),
            session_id: session_id.to_string(),
            runbook_id,
            category: self.category(),
            event_type: self.event_type().to_string(),
            payload: serde_json::to_value(self).unwrap_or_default(),
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_event_serialization() {
        let event = RunbookEvent::CommandStaged {
            runbook_id: Uuid::now_v7(),
            command: StagedCommandSummary {
                id: Uuid::now_v7(),
                source_order: 1,
                verb: "entity.list".to_string(),
                description: Some("List Irish funds".to_string()),
                resolution_status: ResolutionStatus::Resolved,
                entity_count: 3,
            },
            runbook_summary: RunbookSummary {
                id: Uuid::now_v7(),
                status: RunbookStatus::Building,
                command_count: 1,
                resolved_count: 1,
                pending_count: 0,
                ambiguous_count: 0,
                failed_count: 0,
            },
            dsl_hash: "abc123".to_string(),
        };

        let json = serde_json::to_string(&event).unwrap();
        assert!(json.contains("command_staged"));
        assert!(json.contains("entity.list"));
    }

    #[test]
    fn test_audit_category() {
        let event = RunbookEvent::ResolutionAmbiguous {
            runbook_id: Uuid::now_v7(),
            command_id: Uuid::now_v7(),
            arg_name: "entity-ids".to_string(),
            original_ref: "main manco".to_string(),
            candidates: vec![],
            candidate_count: 2,
        };

        assert!(matches!(event.category(), AuditCategory::EntityResolution));
        assert_eq!(event.event_type(), "resolution_ambiguous");
    }
}
