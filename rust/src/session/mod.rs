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
pub mod enhanced_context;
pub mod research_context;
pub mod scope;
pub mod verb_discovery;
pub mod verb_rag_metadata;
pub mod verb_sync;

use chrono::{DateTime, Utc};
use serde::{Deserialize, Serialize};
use std::collections::HashMap;
use uuid::Uuid;

use crate::dsl_v2::ExecutionContext;
use crate::graph::{EntityGraph, GraphFilters, ViewportContext};
use crate::navigation::{NavCommand, NavExecutor, NavResult};

pub use crate::research::ApprovedResearch;
pub use agent_context::AgentGraphContext;
pub use enhanced_context::{
    get_verb_suggestions, EnhancedAgentContext, EnhancedContextBuilder, SerializableAgentContext,
    SerializableBinding,
};
pub use research_context::{ResearchContext, ResearchState};
pub use scope::{ExpandableNode, LoadStatus, ScopeSummary, SessionScope};
pub use verb_discovery::{
    AgentVerbContext, CategoryInfo, DiscoveryQuery, SuggestionReason, VerbDiscoveryError,
    VerbDiscoveryService, VerbSuggestion, WorkflowPhaseInfo,
};
pub use verb_sync::{SyncResult, VerbSyncError, VerbSyncService};

/// Unified session context - handles REPL + Visualization + Navigation
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
            session_id: Uuid::new_v4(),
            user_id: None,
            created_at: Utc::now(),
            execution: ExecutionContext::new(),
            graph: None,
            viewport: ViewportContext::new(1200.0, 800.0),
            scope: SessionScope::empty(),
            command_history: Vec::new(),
            bookmarks: HashMap::new(),
            research: ResearchContext::new(),
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
        let user_id = Uuid::new_v4();
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
