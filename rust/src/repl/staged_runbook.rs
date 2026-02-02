//! Staged Runbook Types
//!
//! Core data structures for the staged runbook REPL system.
//! These types mirror the database schema from migration 054.

use serde::{Deserialize, Serialize};
use uuid::Uuid;

/// Status of a staged runbook
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize, Default)]
#[serde(rename_all = "snake_case")]
pub enum RunbookStatus {
    /// Accepting new commands, can be edited
    #[default]
    Building,
    /// All commands resolved, DAG computed, awaiting execute
    Ready,
    /// Currently running
    Executing,
    /// Finished successfully
    Completed,
    /// User cancelled
    Aborted,
}

impl RunbookStatus {
    /// Parse from database string
    pub fn from_db(s: &str) -> Self {
        match s {
            "building" => Self::Building,
            "ready" => Self::Ready,
            "executing" => Self::Executing,
            "completed" => Self::Completed,
            "aborted" => Self::Aborted,
            _ => Self::Building,
        }
    }

    /// Convert to database string
    pub fn to_db(&self) -> &'static str {
        match self {
            Self::Building => "building",
            Self::Ready => "ready",
            Self::Executing => "executing",
            Self::Completed => "completed",
            Self::Aborted => "aborted",
        }
    }

    /// Check if runbook can accept new commands
    pub fn can_stage(&self) -> bool {
        matches!(self, Self::Building)
    }

    /// Check if runbook can be executed
    pub fn can_execute(&self) -> bool {
        matches!(self, Self::Ready)
    }
}

/// Resolution status for a staged command
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize, Default)]
#[serde(rename_all = "snake_case")]
pub enum ResolutionStatus {
    /// Not yet resolved
    #[default]
    Pending,
    /// All refs resolved to UUIDs
    Resolved,
    /// Needs picker - multiple low-confidence matches
    Ambiguous,
    /// Resolution error (no matches found)
    Failed,
    /// DSL syntax error
    ParseFailed,
}

impl ResolutionStatus {
    /// Parse from database string
    pub fn from_db(s: &str) -> Self {
        match s {
            "pending" => Self::Pending,
            "resolved" => Self::Resolved,
            "ambiguous" => Self::Ambiguous,
            "failed" => Self::Failed,
            "parse_failed" => Self::ParseFailed,
            _ => Self::Pending,
        }
    }

    /// Convert to database string
    pub fn to_db(&self) -> &'static str {
        match self {
            Self::Pending => "pending",
            Self::Resolved => "resolved",
            Self::Ambiguous => "ambiguous",
            Self::Failed => "failed",
            Self::ParseFailed => "parse_failed",
        }
    }

    /// Check if this status blocks execution
    pub fn blocks_execution(&self) -> bool {
        !matches!(self, Self::Resolved)
    }
}

/// How an entity reference was resolved
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum ResolutionSource {
    /// Exact tag match
    TagExact,
    /// Trigram fuzzy match
    TagFuzzy,
    /// Candle semantic embedding match
    TagSemantic,
    /// User provided UUID directly
    DirectUuid,
    /// User selected from picker
    Picker,
    /// From previous command's output ($N.result)
    OutputRef,
}

impl ResolutionSource {
    /// Parse from database string
    pub fn from_db(s: &str) -> Self {
        match s {
            "tag_exact" => Self::TagExact,
            "tag_fuzzy" => Self::TagFuzzy,
            "tag_semantic" => Self::TagSemantic,
            "direct_uuid" => Self::DirectUuid,
            "picker" => Self::Picker,
            "output_ref" => Self::OutputRef,
            _ => Self::TagExact,
        }
    }

    /// Convert to database string
    pub fn to_db(&self) -> &'static str {
        match self {
            Self::TagExact => "tag_exact",
            Self::TagFuzzy => "tag_fuzzy",
            Self::TagSemantic => "tag_semantic",
            Self::DirectUuid => "direct_uuid",
            Self::Picker => "picker",
            Self::OutputRef => "output_ref",
        }
    }
}

/// A resolved entity in a staged command
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ResolvedEntity {
    /// The resolved entity UUID
    pub entity_id: Uuid,
    /// Entity name (for display)
    pub entity_name: String,
    /// Which DSL argument this resolved (e.g., "entity-id", "entity-ids")
    pub arg_name: String,
    /// How the resolution happened
    pub resolution_source: ResolutionSource,
    /// Original reference text (e.g., "Irish funds")
    pub original_ref: String,
    /// Confidence score (for fuzzy/semantic matches)
    /// Using f64 to match PostgreSQL FLOAT
    pub confidence: Option<f64>,
}

/// A staged runbook - accumulates commands for review before execution
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct StagedRunbook {
    /// Unique runbook ID
    pub id: Uuid,
    /// MCP session ID (stable conversation key)
    pub session_id: String,
    /// Client group context (for entity resolution)
    pub client_group_id: Option<Uuid>,
    /// Persona (affects tag filtering)
    pub persona: Option<String>,
    /// Current status
    pub status: RunbookStatus,
    /// Staged commands
    pub commands: Vec<StagedCommand>,
    /// Creation timestamp
    pub created_at: chrono::DateTime<chrono::Utc>,
    /// Last update timestamp
    pub updated_at: chrono::DateTime<chrono::Utc>,
}

impl StagedRunbook {
    /// Create a new runbook for a session
    pub fn new(session_id: String, client_group_id: Option<Uuid>, persona: Option<String>) -> Self {
        let now = chrono::Utc::now();
        Self {
            id: Uuid::now_v7(),
            session_id,
            client_group_id,
            persona,
            status: RunbookStatus::Building,
            commands: Vec::new(),
            created_at: now,
            updated_at: now,
        }
    }

    /// Check if all commands are resolved
    pub fn is_ready(&self) -> bool {
        !self.commands.is_empty()
            && self
                .commands
                .iter()
                .all(|c| c.resolution_status == ResolutionStatus::Resolved)
    }

    /// Get count of commands by status
    pub fn status_counts(&self) -> StatusCounts {
        let mut counts = StatusCounts::default();
        for cmd in &self.commands {
            counts.total += 1;
            match cmd.resolution_status {
                ResolutionStatus::Pending => counts.pending += 1,
                ResolutionStatus::Resolved => counts.resolved += 1,
                ResolutionStatus::Ambiguous => counts.ambiguous += 1,
                ResolutionStatus::Failed | ResolutionStatus::ParseFailed => counts.failed += 1,
            }
        }
        counts
    }

    /// Get all unique entity IDs in the entity footprint
    pub fn entity_footprint(&self) -> Vec<Uuid> {
        let mut ids: std::collections::HashSet<Uuid> = std::collections::HashSet::new();
        for cmd in &self.commands {
            for entity in &cmd.entity_footprint {
                ids.insert(entity.entity_id);
            }
        }
        ids.into_iter().collect()
    }

    /// Get blocking commands (non-resolved)
    pub fn blocking_commands(&self) -> Vec<&StagedCommand> {
        self.commands
            .iter()
            .filter(|c| c.resolution_status.blocks_execution())
            .collect()
    }

    /// Get next source order for a new command
    pub fn next_source_order(&self) -> i32 {
        self.commands
            .iter()
            .map(|c| c.source_order)
            .max()
            .unwrap_or(0)
            + 1
    }
}

/// Status counts for runbook summary
#[derive(Debug, Clone, Default, Serialize, Deserialize)]
pub struct StatusCounts {
    pub total: usize,
    pub resolved: usize,
    pub pending: usize,
    pub ambiguous: usize,
    pub failed: usize,
}

/// A staged command in the runbook
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct StagedCommand {
    /// Unique command ID
    pub id: Uuid,
    /// User's original insertion order
    pub source_order: i32,
    /// Computed execution order (None until ready)
    pub dag_order: Option<i32>,
    /// Raw DSL as provided (may have shorthand)
    pub dsl_raw: String,
    /// DSL with UUIDs substituted (None until resolved)
    pub dsl_resolved: Option<String>,
    /// Parsed verb (e.g., "entity.list")
    pub verb: String,
    /// Human-readable description
    pub description: Option<String>,
    /// Original user utterance
    pub source_prompt: Option<String>,
    /// Resolution status
    pub resolution_status: ResolutionStatus,
    /// Error message if failed/parse_failed
    pub resolution_error: Option<String>,
    /// Command IDs this depends on (from $N refs)
    pub depends_on: Vec<Uuid>,
    /// Resolved entities (entity footprint)
    pub entity_footprint: Vec<ResolvedEntity>,
    /// Picker candidates (for ambiguous resolution)
    pub candidates: Vec<PickerCandidate>,
}

impl StagedCommand {
    /// Create a new command with parse failed status
    pub fn parse_failed(source_order: i32, dsl_raw: String, error: String) -> Self {
        Self {
            id: Uuid::now_v7(),
            source_order,
            dag_order: None,
            dsl_raw,
            dsl_resolved: None,
            verb: String::new(),
            description: None,
            source_prompt: None,
            resolution_status: ResolutionStatus::ParseFailed,
            resolution_error: Some(error),
            depends_on: Vec::new(),
            entity_footprint: Vec::new(),
            candidates: Vec::new(),
        }
    }

    /// Create a new command in pending state
    pub fn new(
        source_order: i32,
        dsl_raw: String,
        verb: String,
        description: Option<String>,
        source_prompt: Option<String>,
    ) -> Self {
        Self {
            id: Uuid::now_v7(),
            source_order,
            dag_order: None,
            dsl_raw,
            dsl_resolved: None,
            verb,
            description,
            source_prompt,
            resolution_status: ResolutionStatus::Pending,
            resolution_error: None,
            depends_on: Vec::new(),
            entity_footprint: Vec::new(),
            candidates: Vec::new(),
        }
    }

    /// Check if command has unresolved $N references
    pub fn has_output_refs(&self) -> bool {
        // Check for $N patterns in DSL
        self.dsl_raw.contains('$') && self.dsl_raw.chars().any(|c| c.is_ascii_digit())
    }
}

/// Picker candidate for ambiguous resolution
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct PickerCandidate {
    /// Entity ID
    pub entity_id: Uuid,
    /// Entity name (for display)
    pub entity_name: String,
    /// Which DSL argument this is for
    pub arg_name: String,
    /// Tag that matched
    pub matched_tag: Option<String>,
    /// Match confidence (f64 to match PostgreSQL FLOAT)
    pub confidence: Option<f64>,
    /// Match type
    pub match_type: String,
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_runbook_status_roundtrip() {
        for status in [
            RunbookStatus::Building,
            RunbookStatus::Ready,
            RunbookStatus::Executing,
            RunbookStatus::Completed,
            RunbookStatus::Aborted,
        ] {
            assert_eq!(RunbookStatus::from_db(status.to_db()), status);
        }
    }

    #[test]
    fn test_resolution_status_roundtrip() {
        for status in [
            ResolutionStatus::Pending,
            ResolutionStatus::Resolved,
            ResolutionStatus::Ambiguous,
            ResolutionStatus::Failed,
            ResolutionStatus::ParseFailed,
        ] {
            assert_eq!(ResolutionStatus::from_db(status.to_db()), status);
        }
    }

    #[test]
    fn test_runbook_is_ready() {
        let mut runbook = StagedRunbook::new("test".to_string(), None, None);

        // Empty runbook is not ready
        assert!(!runbook.is_ready());

        // Add a pending command
        runbook.commands.push(StagedCommand::new(
            1,
            "(entity.list)".to_string(),
            "entity.list".to_string(),
            None,
            None,
        ));
        assert!(!runbook.is_ready());

        // Mark as resolved
        runbook.commands[0].resolution_status = ResolutionStatus::Resolved;
        assert!(runbook.is_ready());

        // Add an ambiguous command
        let mut cmd2 = StagedCommand::new(
            2,
            "(entity.get entity-id=\"main manco\")".to_string(),
            "entity.get".to_string(),
            None,
            None,
        );
        cmd2.resolution_status = ResolutionStatus::Ambiguous;
        runbook.commands.push(cmd2);
        assert!(!runbook.is_ready());
    }

    #[test]
    fn test_status_counts() {
        let mut runbook = StagedRunbook::new("test".to_string(), None, None);

        // Add various commands
        let mut cmd1 = StagedCommand::new(1, "".to_string(), "".to_string(), None, None);
        cmd1.resolution_status = ResolutionStatus::Resolved;

        let mut cmd2 = StagedCommand::new(2, "".to_string(), "".to_string(), None, None);
        cmd2.resolution_status = ResolutionStatus::Ambiguous;

        let mut cmd3 = StagedCommand::new(3, "".to_string(), "".to_string(), None, None);
        cmd3.resolution_status = ResolutionStatus::Failed;

        runbook.commands = vec![cmd1, cmd2, cmd3];

        let counts = runbook.status_counts();
        assert_eq!(counts.total, 3);
        assert_eq!(counts.resolved, 1);
        assert_eq!(counts.ambiguous, 1);
        assert_eq!(counts.failed, 1);
        assert_eq!(counts.pending, 0);
    }
}
