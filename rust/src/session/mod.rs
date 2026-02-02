//! Unified Session Context
//!
//! Single service handling REPL execution, graph navigation, and viewport.
//! Supports multiple scope sizes with windowing for large datasets.
//!
//! # Architecture
//!
//! ```text
//! UnifiedSessionContext
//! ├── session_id, user_id, created_at
//! ├── execution: ExecutionContext (DSL REPL state)
//! ├── graph: Option<EntityGraph> (navigation state)
//! ├── viewport: ViewportContext (zoom/pan/visibility)
//! ├── scope: SessionScope (definition + stats + load status)
//! ├── command_history: Vec<ExecutedCommand>
//! └── bookmarks: HashMap<String, Bookmark>
//! ```

pub mod agent_context;
pub mod agent_mode;
pub mod canonical_hash;
pub mod constraint_cascade;
pub mod dsl_sheet;
pub mod enhanced_context;
pub mod research_context;
pub mod scope;
pub mod scope_path;
pub mod struct_mass;
pub mod unified;
pub mod verb_contract;
pub mod verb_discovery;
pub mod verb_hash_lookup;
pub mod verb_sync;
pub mod verb_tiering_linter;
pub mod view_state;

use chrono::{DateTime, Utc};
use serde::{Deserialize, Serialize};
use std::collections::HashMap;
use uuid::Uuid;

use crate::dsl_v2::ExecutionContext;
use crate::graph::{EntityGraph, GraphFilters, ViewportContext};
use crate::navigation::{NavCommand, NavExecutor, NavResult};

pub use crate::research::ApprovedResearch;
pub use agent_context::AgentGraphContext;
pub use agent_mode::{
    ActionRef, AgentState, AgentStatus, AgentTask, Candidate, Checkpoint, CheckpointContext,
    CheckpointType, DecisionRef, SessionMode,
};
pub use canonical_hash::{canonical_json_hash, hash_to_hex, hex_to_hash, sha256};
pub use constraint_cascade::{
    derive_extended_scope, derive_search_scope, set_case, set_client, set_structure,
    set_structure_type, update_dag_from_cascade, validate_set_case, validate_set_client,
    validate_set_structure, validate_set_structure_type, verb_available_for_persona, CascadeError,
    CascadeUpdateResult, ExtendedSearchScope,
};
// =============================================================================
// DEPRECATED: dsl_sheet types - use unified.rs types instead
// =============================================================================
// Migration guide:
// - DslSheet → RunSheet (unified.rs)
// - SessionDslStatement → RunSheetEntry (unified.rs)
// - StatementStatus → EntryStatus (unified.rs)
// - ErrorCode → UnifiedErrorCode (unified.rs)
// - ExecutionPhase → UnifiedExecutionPhase (unified.rs)
// - SheetExecutionResult → UnifiedSheetExecutionResult (unified.rs)
// - SheetStatus → UnifiedSheetStatus (unified.rs)
// - StatementError → EntryError (unified.rs)
// - StatementResult → EntryResult (unified.rs)
//
// These re-exports are kept for backward compatibility with legacy code paths
// (SheetExecutor.execute_phased, SheetExecutor.persist_audit).
// New code should use the unified types from the `unified` module.
#[allow(deprecated)]
#[deprecated(
    since = "0.1.0",
    note = "Use unified types from session::unified instead (RunSheet, EntryStatus, etc.)"
)]
pub use dsl_sheet::{
    CyclicDependency, DslSheet, EntityCandidate, ErrorCode, ExecutionPhase, SessionDslStatement,
    SheetExecutionResult, SheetStatus, SourceSpan, StatementError, StatementResult,
    StatementStatus, UnresolvedReference, ValidationError, ValidationResult, ValidationWarning,
};
pub use enhanced_context::{
    get_verb_suggestions, EnhancedAgentContext, EnhancedContextBuilder, SerializableAgentContext,
    SerializableBinding,
};
pub use research_context::{ResearchContext, ResearchState};
pub use scope::{ExpandableNode, LoadStatus, ScopeSummary, SessionScope};
pub use scope_path::{ScopePath, ScopeSegment};
pub use struct_mass::{
    MassBreakdown, MassContributions, MassThresholds, MassViewMode, MassWeights, StructMass,
};
pub use unified::{
    BoundEntity,
    CaseRef,
    // CBU undo/redo (migrated from CbuSession)
    CbuSnapshot,
    ChatMessage,
    ClientRef,
    // Correction sub-session
    CorrectionSubSession,
    DagState,
    DiscriminatorField,
    EntityMatch,
    // Entity match info for resolution UI
    EntityMatchInfo,
    EntityScope,
    // Sheet execution types (migrated from dsl_sheet)
    EntryError,
    EntryResult,
    EntryStatus,
    EnumValue,
    ErrorCode as UnifiedErrorCode,
    ExecutionPhase as UnifiedExecutionPhase,
    FieldType,
    MessageRole,
    Persona,
    PrereqCondition,
    // REPL state machine (migrated from CbuSession)
    ReplState,
    // Research sub-session
    ResearchSubSession,
    ResolutionState,
    // Resolution sub-session (full metadata for disambiguation)
    ResolutionSubSession,
    ResolvedRef,
    // Review status enum
    ReviewStatus,
    // Review sub-session
    ReviewSubSession,
    RunSheet,
    RunSheetEntry,
    SearchKeyField,
    SearchScope,
    // Session state machine (migrated from AgentSession)
    SessionEvent,
    // Session list item for API responses
    SessionListItem,
    // Session lifecycle states
    SessionState,
    SheetExecutionResult as UnifiedSheetExecutionResult,
    SheetStatus as UnifiedSheetStatus,
    StateSnapshot,
    StateStack,
    StructureRef,
    StructureType,
    // Sub-session types
    SubSessionType,
    TargetUniverse,
    UnifiedSession,
    UniverseDefinition,
    UnresolvedRef,
    // Unresolved ref info (full metadata for resolution UI)
    UnresolvedRefInfo,
    ValidationError as UnifiedValidationError,
    ViewState as UnifiedViewState,
    ZoomLevel,
};
pub use verb_contract::{codes as diagnostic_codes, VerbDiagnostic, VerbDiagnostics};
pub use verb_discovery::{
    AgentVerbContext, CategoryInfo, DiscoveryQuery, SuggestionReason, VerbDiscoveryError,
    VerbDiscoveryService, VerbSuggestion, WorkflowPhaseInfo,
};
pub use verb_sync::{SyncResult, VerbSyncError, VerbSyncService};
pub use verb_tiering_linter::{
    lint_all_verbs, lint_all_verbs_with_config, lint_verb_tiering, lint_verb_with_config,
    LintConfig, LintReport, LintTier, VerbLintResult,
};
pub use view_state::{
    BatchOperation, ContextMode, CrossSectionAxis, DetailLevel, DrillDirection, GapType,
    HighlightMode, IlluminateAspect, LayoutBounds, LayoutResult, NavStackEntry, NodePosition,
    OperationPreview, PendingOperation, RedFlagCategory, Refinement, RiskThreshold, ScaleLevel,
    TemporalMode, TraceMode, ViewState,
};

/// Unified session context - handles Visualization + Navigation
///
/// # Deprecation Notice
///
/// **This struct is deprecated for new code.** Use `UnifiedSession` from `session::unified`
/// for DSL execution, run sheet management, and constraint cascade context.
///
/// `UnifiedSessionContext` is retained for:
/// - Graph/viewport/navigation state (visualization layer)
/// - Agent mode orchestration (`agent`, `mode` fields)
/// - Research workflows via `AgentController`
///
/// **Why kept separate:**
/// - `UnifiedSession` is fully serializable; this struct has `#[serde(skip)]` fields
/// - Agent/visualization state is complex and well-isolated
/// - Incremental consolidation preferred over big-bang refactor
///
/// **Consumers:**
/// - `EnhancedContextBuilder::from_session_context()` - verb suggestions
/// - `AgentGraphContext::from_session()` - agent prompts
/// - `AgentController` - research loop orchestration
///
/// For new DSL/REPL features, prefer `UnifiedSession`.
#[derive(Debug, Serialize, Deserialize)]
pub struct UnifiedSessionContext {
    /// Session identity
    pub session_id: Uuid,
    pub user_id: Option<Uuid>,
    pub created_at: DateTime<Utc>,

    /// DSL Execution state (from dsl_v2)
    /// Note: Not serialized or cloned - each session has its own execution context
    #[serde(skip)]
    pub execution: ExecutionContext,

    /// Graph data (from EntityGraph implementation)
    pub graph: Option<EntityGraph>,

    /// Viewport state (zoom, pan, visibility)
    pub viewport: ViewportContext,

    /// Scope definition and stats
    pub scope: SessionScope,

    /// Command history for undo/replay
    pub command_history: Vec<ExecutedCommand>,

    /// Named bookmarks
    pub bookmarks: HashMap<String, Bookmark>,

    /// Research macro state (pending results, approvals)
    pub research: ResearchContext,

    /// View state - the unified "it" that session is looking at
    /// This IS what the user sees = what operations target = what agent knows about
    pub view: Option<ViewState>,

    /// Session operating mode (Manual, Agent, Hybrid)
    #[serde(default)]
    pub mode: SessionMode,

    /// Agent state when in Agent mode
    pub agent: Option<AgentState>,
}

impl Clone for UnifiedSessionContext {
    fn clone(&self) -> Self {
        Self {
            session_id: self.session_id,
            user_id: self.user_id,
            created_at: self.created_at,
            // Create fresh execution context - don't clone REPL state
            execution: ExecutionContext::new(),
            graph: self.graph.clone(),
            viewport: self.viewport.clone(),
            scope: self.scope.clone(),
            command_history: self.command_history.clone(),
            bookmarks: self.bookmarks.clone(),
            research: self.research.clone(),
            view: self.view.clone(),
            mode: self.mode,
            agent: self.agent.clone(),
        }
    }
}

/// Executed command with timestamp for history
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ExecutedCommand {
    pub command: NavCommand,
    pub executed_at: DateTime<Utc>,
    pub result_summary: String,
}

/// Named position bookmark
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct Bookmark {
    pub name: String,
    pub cursor: Option<Uuid>,
    pub filters: GraphFilters,
    pub zoom: f32,
    pub pan_offset: (f32, f32),
}

impl Default for UnifiedSessionContext {
    fn default() -> Self {
        Self::new()
    }
}

impl UnifiedSessionContext {
    /// Create a new session with default values
    pub fn new() -> Self {
        Self {
            session_id: Uuid::now_v7(),
            user_id: None,
            created_at: Utc::now(),
            execution: ExecutionContext::new(),
            graph: None,
            viewport: ViewportContext::new(1200.0, 800.0),
            scope: SessionScope::empty(),
            command_history: Vec::new(),
            bookmarks: HashMap::new(),
            research: ResearchContext::new(),
            view: None,
            mode: SessionMode::Manual,
            agent: None,
        }
    }

    /// Create a session with a specific user
    pub fn with_user(user_id: Uuid) -> Self {
        let mut session = Self::new();
        session.user_id = Some(user_id);
        session
    }

    /// Execute a navigation command
    pub fn execute_nav(&mut self, cmd: NavCommand) -> NavResult {
        // Record in history
        let result = if let Some(graph) = &mut self.graph {
            graph.execute_nav(cmd.clone())
        } else {
            NavResult::Error {
                message: "No graph loaded. Use load_cbu, load_book, or load_jurisdiction first."
                    .into(),
            }
        };

        // Update viewport visibility after navigation
        if let Some(graph) = &self.graph {
            self.viewport.compute_visibility(graph);
        }

        self.command_history.push(ExecutedCommand {
            command: cmd,
            executed_at: Utc::now(),
            result_summary: format!("{:?}", result),
        });

        result
    }

    /// Set the graph data and update scope stats
    pub fn set_graph(&mut self, graph: EntityGraph) {
        // Update scope stats from graph
        self.scope = SessionScope::from_graph(&graph, self.scope.definition.clone());

        // Store graph
        self.graph = Some(graph);

        // Reset viewport for new scope
        self.viewport =
            ViewportContext::new(self.viewport.canvas_size.0, self.viewport.canvas_size.1);
        if let Some(g) = &self.graph {
            self.viewport.compute_visibility(g);
        }
    }

    /// Clear the current graph
    pub fn clear_graph(&mut self) {
        self.graph = None;
        self.scope = SessionScope::empty();
        self.viewport =
            ViewportContext::new(self.viewport.canvas_size.0, self.viewport.canvas_size.1);
    }

    /// Create a bookmark at the current position
    pub fn create_bookmark(&mut self, name: &str) {
        if let Some(graph) = &self.graph {
            let bookmark = Bookmark {
                name: name.to_string(),
                cursor: graph.cursor,
                filters: graph.filters.clone(),
                zoom: self.viewport.zoom,
                pan_offset: self.viewport.pan_offset,
            };
            self.bookmarks.insert(name.to_string(), bookmark);
        }
    }

    /// Restore a named bookmark
    pub fn restore_bookmark(&mut self, name: &str) -> bool {
        if let Some(bookmark) = self.bookmarks.get(name).cloned() {
            if let Some(graph) = &mut self.graph {
                graph.cursor = bookmark.cursor;
                graph.filters = bookmark.filters;
                self.viewport.zoom = bookmark.zoom;
                self.viewport.pan_offset = bookmark.pan_offset;
                self.viewport.update_zoom_name();
                return true;
            }
        }
        false
    }

    /// Get list of bookmark names
    pub fn list_bookmarks(&self) -> Vec<&str> {
        self.bookmarks.keys().map(|s| s.as_str()).collect()
    }

    /// Get command history (most recent first)
    pub fn recent_commands(&self, limit: usize) -> Vec<&ExecutedCommand> {
        self.command_history.iter().rev().take(limit).collect()
    }

    /// Build agent context from current state
    pub fn build_agent_context(&self) -> AgentGraphContext {
        AgentGraphContext::from_session(self)
    }

    /// Check if session has a graph loaded
    pub fn has_graph(&self) -> bool {
        self.graph.is_some()
    }

    /// Check if session has a cursor set
    pub fn has_cursor(&self) -> bool {
        self.graph
            .as_ref()
            .map(|g| g.cursor.is_some())
            .unwrap_or(false)
    }

    /// Get current cursor entity ID
    pub fn cursor_id(&self) -> Option<Uuid> {
        self.graph.as_ref().and_then(|g| g.cursor)
    }

    /// Get cursor entity name
    pub fn cursor_name(&self) -> Option<String> {
        self.graph.as_ref().and_then(|g| {
            g.cursor
                .and_then(|id| g.nodes.get(&id).map(|n| n.name.clone()))
        })
    }

    // =========================================================================
    // VIEW STATE METHODS - The unified "it" management
    // =========================================================================

    /// Check if session has a view loaded
    pub fn has_view(&self) -> bool {
        self.view.is_some()
    }

    /// Get current view for rendering (immutable)
    pub fn current_view(&self) -> Option<&ViewState> {
        self.view.as_ref()
    }

    /// Get current view for modification
    pub fn current_view_mut(&mut self) -> Option<&mut ViewState> {
        self.view.as_mut()
    }

    /// Set view from an existing ViewState
    pub fn set_view(&mut self, view: ViewState) {
        self.view = Some(view);
    }

    /// Clear the current view
    pub fn clear_view(&mut self) {
        self.view = None;
    }

    /// Apply refinement to current view
    pub fn refine_view(&mut self, refinement: Refinement) -> anyhow::Result<()> {
        if let Some(view) = &mut self.view {
            view.refine(refinement);
            Ok(())
        } else {
            Err(anyhow::anyhow!("No active view"))
        }
    }

    /// Clear all refinements from current view
    pub fn clear_view_refinements(&mut self) -> anyhow::Result<()> {
        if let Some(view) = &mut self.view {
            view.clear_refinements();
            Ok(())
        } else {
            Err(anyhow::anyhow!("No active view"))
        }
    }

    /// Stage batch operation on current selection
    pub fn stage_operation(&mut self, operation: BatchOperation) -> anyhow::Result<()> {
        if let Some(view) = &mut self.view {
            view.stage_operation(operation)
        } else {
            Err(anyhow::anyhow!("No active view"))
        }
    }

    /// Clear pending operation from current view
    pub fn clear_pending_operation(&mut self) -> anyhow::Result<()> {
        if let Some(view) = &mut self.view {
            view.clear_pending();
            Ok(())
        } else {
            Err(anyhow::anyhow!("No active view"))
        }
    }

    /// Get current selection count
    pub fn selection_count(&self) -> usize {
        self.view.as_ref().map(|v| v.selection_count()).unwrap_or(0)
    }

    /// Check if there's a pending operation
    pub fn has_pending_operation(&self) -> bool {
        self.view.as_ref().is_some_and(|v| v.has_pending())
    }

    /// Get the pending operation's generated DSL (for preview/editing)
    pub fn pending_dsl(&self) -> Option<&str> {
        self.view
            .as_ref()
            .and_then(|v| v.pending.as_ref())
            .map(|p| p.verbs.as_str())
    }

    /// Get selection IDs for external use
    pub fn selection_ids(&self) -> Vec<Uuid> {
        self.view
            .as_ref()
            .map(|v| v.selection.clone())
            .unwrap_or_default()
    }

    // =========================================================================
    // FRACTAL NAVIGATION - Zoom in/out through taxonomy stack
    // =========================================================================

    /// Zoom into a node, expanding it into its child taxonomy.
    ///
    /// Delegates to ViewState::zoom_in. Returns Ok(true) if zoom succeeded.
    pub async fn zoom_in(&mut self, node_id: Uuid) -> anyhow::Result<bool> {
        if let Some(view) = &mut self.view {
            view.zoom_in(node_id).await
        } else {
            Err(anyhow::anyhow!("No active view"))
        }
    }

    /// Zoom out to the parent taxonomy.
    ///
    /// Delegates to ViewState::zoom_out. Returns Ok(true) if zoom out succeeded.
    pub fn zoom_out(&mut self) -> anyhow::Result<bool> {
        if let Some(view) = &mut self.view {
            view.zoom_out()
        } else {
            Err(anyhow::anyhow!("No active view"))
        }
    }

    /// Jump back to a specific breadcrumb level.
    ///
    /// `depth` is 0-indexed: 0 = root, 1 = first zoom, etc.
    pub fn back_to(&mut self, depth: usize) -> anyhow::Result<bool> {
        if let Some(view) = &mut self.view {
            view.back_to(depth)
        } else {
            Err(anyhow::anyhow!("No active view"))
        }
    }

    /// Get breadcrumbs for navigation display.
    pub fn breadcrumbs(&self) -> Vec<String> {
        self.view
            .as_ref()
            .map(|v| v.breadcrumbs())
            .unwrap_or_default()
    }

    /// Get breadcrumbs with frame IDs.
    pub fn breadcrumbs_with_ids(&self) -> Vec<(String, Uuid)> {
        self.view
            .as_ref()
            .map(|v| v.breadcrumbs_with_ids())
            .unwrap_or_default()
    }

    /// Get current zoom depth (0 = root level).
    pub fn zoom_depth(&self) -> usize {
        self.view.as_ref().map(|v| v.zoom_depth()).unwrap_or(0)
    }

    /// Check if we can zoom out (not at root).
    pub fn can_zoom_out(&self) -> bool {
        self.view.as_ref().is_some_and(|v| v.can_zoom_out())
    }

    /// Check if a node can be zoomed into.
    pub fn can_zoom_in(&self, node_id: Uuid) -> bool {
        self.view.as_ref().is_some_and(|v| v.can_zoom_in(node_id))
    }

    // =========================================================================
    // AGENT MODE METHODS
    // =========================================================================

    /// Start agent mode with a task
    pub fn start_agent(&mut self, task: AgentTask) -> Uuid {
        let state = AgentState::new(task);
        let agent_session_id = state.agent_session_id;
        self.agent = Some(state);
        self.mode = SessionMode::Agent;
        agent_session_id
    }

    /// Start agent mode for resolving gaps
    pub fn start_resolve_gaps(&mut self, entity_id: Uuid) -> Uuid {
        let state = AgentState::resolve_gaps(entity_id);
        let agent_session_id = state.agent_session_id;
        self.agent = Some(state);
        self.mode = SessionMode::Agent;
        agent_session_id
    }

    /// Start agent mode for chain research
    pub fn start_chain_research(&mut self, entity_id: Uuid) -> Uuid {
        let state = AgentState::chain_research(entity_id);
        let agent_session_id = state.agent_session_id;
        self.agent = Some(state);
        self.mode = SessionMode::Agent;
        agent_session_id
    }

    /// Stop agent mode and return to manual
    pub fn stop_agent(&mut self) {
        if let Some(agent) = &mut self.agent {
            agent.cancel();
        }
        self.mode = SessionMode::Manual;
    }

    /// Pause agent execution
    pub fn pause_agent(&mut self) {
        if let Some(agent) = &mut self.agent {
            agent.pause();
        }
    }

    /// Resume agent execution
    pub fn resume_agent(&mut self) {
        if let Some(agent) = &mut self.agent {
            agent.resume();
        }
    }

    /// Check if session is in agent mode
    pub fn is_agent_mode(&self) -> bool {
        matches!(self.mode, SessionMode::Agent | SessionMode::Hybrid)
    }

    /// Check if agent is waiting for user input
    pub fn agent_waiting(&self) -> bool {
        self.agent.as_ref().is_some_and(|a| a.is_waiting())
    }

    /// Get pending checkpoint if any
    pub fn pending_checkpoint(&self) -> Option<&Checkpoint> {
        self.agent
            .as_ref()
            .and_then(|a| a.pending_checkpoint.as_ref())
    }

    /// Get agent status
    pub fn agent_status(&self) -> Option<AgentStatus> {
        self.agent.as_ref().map(|a| a.status)
    }

    /// Get agent summary for display
    pub fn agent_summary(&self) -> Option<String> {
        self.agent.as_ref().map(|a| a.summary())
    }

    /// Get mutable reference to agent state
    pub fn agent_mut(&mut self) -> Option<&mut AgentState> {
        self.agent.as_mut()
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_new_session() {
        let session = UnifiedSessionContext::new();
        assert!(!session.has_graph());
        assert!(!session.has_cursor());
        assert!(session.bookmarks.is_empty());
        assert!(session.command_history.is_empty());
    }

    #[test]
    fn test_session_with_user() {
        let user_id = Uuid::now_v7();
        let session = UnifiedSessionContext::with_user(user_id);
        assert_eq!(session.user_id, Some(user_id));
    }

    #[test]
    fn test_execute_nav_without_graph() {
        let mut session = UnifiedSessionContext::new();
        let result = session.execute_nav(NavCommand::GoUp);

        match result {
            NavResult::Error { message } => {
                assert!(message.contains("No graph loaded"));
            }
            _ => panic!("Expected error result"),
        }

        // Command should still be recorded in history
        assert_eq!(session.command_history.len(), 1);
    }

    #[test]
    fn test_recent_commands() {
        let mut session = UnifiedSessionContext::new();

        // Execute several commands
        session.execute_nav(NavCommand::GoUp);
        session.execute_nav(NavCommand::GoDown {
            index: None,
            name: None,
        });
        session.execute_nav(NavCommand::ZoomIn);

        let recent = session.recent_commands(2);
        assert_eq!(recent.len(), 2);

        // Most recent first
        assert!(matches!(recent[0].command, NavCommand::ZoomIn));
        assert!(matches!(recent[1].command, NavCommand::GoDown { .. }));
    }
}
