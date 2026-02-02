//! Agent Mode State
//!
//! Implements the agent loop state machine for research workflows.
//! Part of the "Bounded Non-Determinism" pattern from 020 doc.
//!
//! # State Machine
//!
//! ```text
//! SessionMode::Manual (default)
//!     ↓ agent.start
//! SessionMode::Agent
//!     ↓ agent.pause / agent.stop
//! SessionMode::Manual
//! ```
//!
//! # Agent Loop
//!
//! When in Agent mode, the AgentController runs a loop:
//! 1. Identify gaps (DSL query)
//! 2. Load orchestration prompt
//! 3. LLM reasons about strategy
//! 4. Execute strategy (search, evaluate, import or checkpoint)
//! 5. Repeat until complete or stopped

use chrono::{DateTime, Utc};
use serde::{Deserialize, Serialize};
use uuid::Uuid;

/// Session operating mode
#[derive(Debug, Clone, Copy, Default, Serialize, Deserialize, PartialEq, Eq)]
#[serde(rename_all = "snake_case")]
pub enum SessionMode {
    /// User types DSL commands directly (default)
    #[default]
    Manual,

    /// Agent controller running, LLM generates DSL
    Agent,

    /// Both user and agent can issue commands
    Hybrid,
}

impl std::fmt::Display for SessionMode {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            SessionMode::Manual => write!(f, "manual"),
            SessionMode::Agent => write!(f, "agent"),
            SessionMode::Hybrid => write!(f, "hybrid"),
        }
    }
}

/// Agent task type
#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq)]
#[serde(rename_all = "snake_case")]
pub enum AgentTask {
    /// Resolve ownership gaps for an entity/group
    ResolveGaps,
    /// Build complete ownership chain to UBO
    ChainResearch,
    /// Enrich a single entity with external data
    EnrichEntity,
    /// Enrich all entities in a group
    EnrichGroup,
    /// Screen entities for sanctions/PEP
    ScreenEntities,
}

impl std::fmt::Display for AgentTask {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            AgentTask::ResolveGaps => write!(f, "resolve_gaps"),
            AgentTask::ChainResearch => write!(f, "chain_research"),
            AgentTask::EnrichEntity => write!(f, "enrich_entity"),
            AgentTask::EnrichGroup => write!(f, "enrich_group"),
            AgentTask::ScreenEntities => write!(f, "screen_entities"),
        }
    }
}

/// Agent execution status
#[derive(Debug, Clone, Copy, Default, Serialize, Deserialize, PartialEq, Eq)]
#[serde(rename_all = "snake_case")]
pub enum AgentStatus {
    /// Agent loop is running
    #[default]
    Running,

    /// Agent is paused (can resume)
    Paused,

    /// Awaiting user input at checkpoint
    Checkpoint,

    /// Task completed successfully
    Complete,

    /// Task failed with error
    Failed,

    /// User cancelled the task
    Cancelled,
}

impl std::fmt::Display for AgentStatus {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            AgentStatus::Running => write!(f, "running"),
            AgentStatus::Paused => write!(f, "paused"),
            AgentStatus::Checkpoint => write!(f, "checkpoint"),
            AgentStatus::Complete => write!(f, "complete"),
            AgentStatus::Failed => write!(f, "failed"),
            AgentStatus::Cancelled => write!(f, "cancelled"),
        }
    }
}

/// Checkpoint type requiring user input
#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq)]
#[serde(rename_all = "snake_case")]
pub enum CheckpointType {
    /// Multiple candidates found, need user selection
    AmbiguousMatch,

    /// High confidence match but context requires confirmation
    HighStakes,

    /// Sanctions or PEP screening hit found
    ScreeningHit,

    /// Post-import validation failed
    ValidationFailure,

    /// Preferred source unavailable, confirm fallback
    SourceUnavailable,
}

/// A candidate entity from search results
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct Candidate {
    /// Source key (LEI, company number, etc.)
    pub key: String,

    /// Key type identifier
    pub key_type: String,

    /// Display name
    pub name: String,

    /// Jurisdiction code
    pub jurisdiction: Option<String>,

    /// Match confidence score (0.0 - 1.0)
    pub score: f64,

    /// Additional details for display
    pub details: Option<serde_json::Value>,
}

/// Context for a checkpoint
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct CheckpointContext {
    /// What was searched for
    pub search_query: String,

    /// Source that was searched
    pub source: String,

    /// Target entity being researched
    pub target_entity_id: Option<Uuid>,

    /// Additional context
    pub context: Option<serde_json::Value>,
}

/// A checkpoint awaiting user response
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct Checkpoint {
    /// Unique checkpoint ID
    pub checkpoint_id: Uuid,

    /// Type of checkpoint
    pub checkpoint_type: CheckpointType,

    /// Context for the checkpoint
    pub context: CheckpointContext,

    /// Candidate options (for AmbiguousMatch)
    pub candidates: Vec<Candidate>,

    /// When checkpoint was created
    pub created_at: DateTime<Utc>,
}

impl Checkpoint {
    /// Create a new checkpoint for ambiguous matches
    pub fn ambiguous_match(
        search_query: String,
        source: String,
        candidates: Vec<Candidate>,
        target_entity_id: Option<Uuid>,
    ) -> Self {
        Self {
            checkpoint_id: Uuid::now_v7(),
            checkpoint_type: CheckpointType::AmbiguousMatch,
            context: CheckpointContext {
                search_query,
                source,
                target_entity_id,
                context: None,
            },
            candidates,
            created_at: Utc::now(),
        }
    }

    /// Create a checkpoint for screening hits
    pub fn screening_hit(
        entity_name: String,
        source: String,
        matches: Vec<Candidate>,
        target_entity_id: Uuid,
    ) -> Self {
        Self {
            checkpoint_id: Uuid::now_v7(),
            checkpoint_type: CheckpointType::ScreeningHit,
            context: CheckpointContext {
                search_query: entity_name,
                source,
                target_entity_id: Some(target_entity_id),
                context: None,
            },
            candidates: matches,
            created_at: Utc::now(),
        }
    }
}

/// Reference to a recorded decision
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct DecisionRef {
    pub decision_id: Uuid,
    pub decision_type: String,
    pub source_provider: String,
    pub created_at: DateTime<Utc>,
}

/// Reference to a recorded action
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ActionRef {
    pub action_id: Uuid,
    pub verb_fqn: String,
    pub success: bool,
    pub executed_at: DateTime<Utc>,
}

/// Agent state within a session
#[derive(Debug, Clone, Default, Serialize, Deserialize)]
pub struct AgentState {
    /// Unique ID for this agent session
    pub agent_session_id: Uuid,

    /// Current task being executed
    pub task: Option<AgentTask>,

    /// Current status
    pub status: AgentStatus,

    /// Target entity for research
    pub target_entity_id: Option<Uuid>,

    /// Target group for research
    pub target_group_id: Option<Uuid>,

    // Loop state
    /// Current iteration count
    pub loop_iteration: u32,

    /// Maximum iterations allowed
    pub max_iterations: u32,

    /// Current prompt being processed
    pub current_prompt: String,

    // Checkpoint state
    /// Pending checkpoint awaiting user input
    pub pending_checkpoint: Option<Checkpoint>,

    // History
    /// Decisions made in this session
    pub decisions: Vec<DecisionRef>,

    /// Actions executed in this session
    pub actions: Vec<ActionRef>,

    // Timing
    /// When agent was started
    pub started_at: Option<DateTime<Utc>>,

    /// Last activity timestamp
    pub last_activity: Option<DateTime<Utc>>,

    /// Error message if failed
    pub error_message: Option<String>,
}

impl AgentState {
    /// Create a new agent state for a task
    pub fn new(task: AgentTask) -> Self {
        Self {
            agent_session_id: Uuid::now_v7(),
            task: Some(task),
            status: AgentStatus::Running,
            target_entity_id: None,
            target_group_id: None,
            loop_iteration: 0,
            max_iterations: 50,
            current_prompt: String::new(),
            pending_checkpoint: None,
            decisions: Vec::new(),
            actions: Vec::new(),
            started_at: Some(Utc::now()),
            last_activity: Some(Utc::now()),
            error_message: None,
        }
    }

    /// Create agent state for resolve-gaps task
    pub fn resolve_gaps(entity_id: Uuid) -> Self {
        let mut state = Self::new(AgentTask::ResolveGaps);
        state.target_entity_id = Some(entity_id);
        state
    }

    /// Create agent state for chain-research task
    pub fn chain_research(entity_id: Uuid) -> Self {
        let mut state = Self::new(AgentTask::ChainResearch);
        state.target_entity_id = Some(entity_id);
        state
    }

    /// Create agent state for enrich-entity task
    pub fn enrich_entity(entity_id: Uuid) -> Self {
        let mut state = Self::new(AgentTask::EnrichEntity);
        state.target_entity_id = Some(entity_id);
        state
    }

    /// Create agent state for enrich-group task
    pub fn enrich_group(group_id: Uuid) -> Self {
        let mut state = Self::new(AgentTask::EnrichGroup);
        state.target_group_id = Some(group_id);
        state
    }

    /// Update last activity timestamp
    pub fn touch(&mut self) {
        self.last_activity = Some(Utc::now());
    }

    /// Increment loop iteration
    pub fn increment_iteration(&mut self) -> bool {
        self.loop_iteration += 1;
        self.touch();
        self.loop_iteration <= self.max_iterations
    }

    /// Set checkpoint and pause for user input
    pub fn set_checkpoint(&mut self, checkpoint: Checkpoint) {
        self.pending_checkpoint = Some(checkpoint);
        self.status = AgentStatus::Checkpoint;
        self.touch();
    }

    /// Clear checkpoint and resume
    pub fn clear_checkpoint(&mut self) {
        self.pending_checkpoint = None;
        self.status = AgentStatus::Running;
        self.touch();
    }

    /// Pause the agent
    pub fn pause(&mut self) {
        if self.status == AgentStatus::Running {
            self.status = AgentStatus::Paused;
            self.touch();
        }
    }

    /// Resume the agent
    pub fn resume(&mut self) {
        if self.status == AgentStatus::Paused {
            self.status = AgentStatus::Running;
            self.touch();
        }
    }

    /// Mark as complete
    pub fn complete(&mut self) {
        self.status = AgentStatus::Complete;
        self.touch();
    }

    /// Mark as failed
    pub fn fail(&mut self, error: &str) {
        self.status = AgentStatus::Failed;
        self.error_message = Some(error.to_string());
        self.touch();
    }

    /// Mark as cancelled
    pub fn cancel(&mut self) {
        self.status = AgentStatus::Cancelled;
        self.touch();
    }

    /// Record a decision reference
    pub fn record_decision(&mut self, decision_id: Uuid, decision_type: &str, source: &str) {
        self.decisions.push(DecisionRef {
            decision_id,
            decision_type: decision_type.to_string(),
            source_provider: source.to_string(),
            created_at: Utc::now(),
        });
        self.touch();
    }

    /// Record an action reference
    pub fn record_action(&mut self, action_id: Uuid, verb_fqn: &str, success: bool) {
        self.actions.push(ActionRef {
            action_id,
            verb_fqn: verb_fqn.to_string(),
            success,
            executed_at: Utc::now(),
        });
        self.touch();
    }

    /// Check if agent is in a terminal state
    pub fn is_terminal(&self) -> bool {
        matches!(
            self.status,
            AgentStatus::Complete | AgentStatus::Failed | AgentStatus::Cancelled
        )
    }

    /// Check if agent is waiting for user input
    pub fn is_waiting(&self) -> bool {
        matches!(self.status, AgentStatus::Checkpoint | AgentStatus::Paused)
    }

    /// Get summary for display
    pub fn summary(&self) -> String {
        let task_name = self
            .task
            .as_ref()
            .map(|t| t.to_string())
            .unwrap_or_else(|| "unknown".to_string());

        format!(
            "[{}] {} - iteration {}/{}, decisions: {}, actions: {}",
            self.status,
            task_name,
            self.loop_iteration,
            self.max_iterations,
            self.decisions.len(),
            self.actions.len()
        )
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_session_mode_default() {
        let mode = SessionMode::default();
        assert_eq!(mode, SessionMode::Manual);
    }

    #[test]
    fn test_agent_state_new() {
        let state = AgentState::new(AgentTask::ResolveGaps);
        assert_eq!(state.status, AgentStatus::Running);
        assert_eq!(state.loop_iteration, 0);
        assert!(state.started_at.is_some());
    }

    #[test]
    fn test_agent_state_lifecycle() {
        let mut state = AgentState::resolve_gaps(Uuid::now_v7());

        // Running
        assert_eq!(state.status, AgentStatus::Running);
        assert!(!state.is_terminal());
        assert!(!state.is_waiting());

        // Pause
        state.pause();
        assert_eq!(state.status, AgentStatus::Paused);
        assert!(state.is_waiting());

        // Resume
        state.resume();
        assert_eq!(state.status, AgentStatus::Running);

        // Checkpoint
        let checkpoint =
            Checkpoint::ambiguous_match("Test Corp".to_string(), "gleif".to_string(), vec![], None);
        state.set_checkpoint(checkpoint);
        assert_eq!(state.status, AgentStatus::Checkpoint);
        assert!(state.is_waiting());
        assert!(state.pending_checkpoint.is_some());

        // Clear checkpoint
        state.clear_checkpoint();
        assert_eq!(state.status, AgentStatus::Running);
        assert!(state.pending_checkpoint.is_none());

        // Complete
        state.complete();
        assert_eq!(state.status, AgentStatus::Complete);
        assert!(state.is_terminal());
    }

    #[test]
    fn test_iteration_limit() {
        let mut state = AgentState::new(AgentTask::ResolveGaps);
        state.max_iterations = 3;

        assert!(state.increment_iteration()); // 1
        assert!(state.increment_iteration()); // 2
        assert!(state.increment_iteration()); // 3
        assert!(!state.increment_iteration()); // 4 > max
    }

    #[test]
    fn test_record_decision_action() {
        let mut state = AgentState::new(AgentTask::EnrichEntity);

        state.record_decision(Uuid::now_v7(), "AUTO_SELECTED", "gleif");
        assert_eq!(state.decisions.len(), 1);

        state.record_action(Uuid::now_v7(), "gleif:enrich", true);
        assert_eq!(state.actions.len(), 1);
    }
}
