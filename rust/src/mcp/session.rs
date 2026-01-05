//! MCP Session Management
//!
//! This module provides session helpers for MCP tool handlers.
//!
//! IMPORTANT: The UI SessionStore (api/session.rs) is the SINGLE SOURCE OF TRUTH.
//! MCP tools must access that store via ToolHandlers.sessions, NOT maintain
//! separate state here. This is critical for egui compliance (EGUI-RULES.md).
//!
//! The legacy Session/SESSIONS static below is for standalone MCP mode only
//! (e.g., Claude Desktop without web UI). For integrated mode, use the
//! SessionStore passed to ToolHandlers::with_sessions().

use std::collections::HashMap;
use std::sync::{LazyLock, RwLock};

use uuid::Uuid;

use super::types::{BindingInfo, SessionAction, SessionState, StageInfo};
use crate::ontology::SemanticStageRegistry;

/// Session data stored in memory (legacy - for standalone MCP mode only)
#[derive(Debug, Clone)]
pub struct Session {
    /// Bindings: name → (uuid, entity_type)
    bindings: HashMap<String, (Uuid, String)>,
    /// History of binding snapshots for undo
    history: Vec<HashMap<String, (Uuid, String)>>,
    /// Currently focused semantic stage (e.g., "GLEIF_RESEARCH")
    stage_focus: Option<String>,
}

impl Session {
    pub fn new() -> Self {
        Self {
            bindings: HashMap::new(),
            history: Vec::new(),
            stage_focus: None,
        }
    }

    /// Set stage focus (filters available verbs)
    pub fn set_stage_focus(&mut self, stage_code: Option<String>) {
        self.stage_focus = stage_code;
    }

    /// Get current stage focus
    pub fn stage_focus(&self) -> Option<&str> {
        self.stage_focus.as_deref()
    }

    /// Record a new binding
    pub fn record_binding(&mut self, name: String, uuid: Uuid, entity_type: String) {
        self.bindings.insert(name, (uuid, entity_type));
    }

    /// Get all bindings as UUID map (for execution context)
    pub fn all_bindings(&self) -> HashMap<String, Uuid> {
        self.bindings
            .iter()
            .map(|(k, (v, _))| (k.clone(), *v))
            .collect()
    }

    /// Get binding info for display
    pub fn binding_info(&self) -> HashMap<String, BindingInfo> {
        self.bindings
            .iter()
            .map(|(name, (uuid, entity_type))| {
                (
                    name.clone(),
                    BindingInfo {
                        name: name.clone(),
                        uuid: uuid.to_string(),
                        entity_type: entity_type.clone(),
                    },
                )
            })
            .collect()
    }

    /// Save current state to history (call before execution)
    pub fn checkpoint(&mut self) {
        self.history.push(self.bindings.clone());
    }

    /// Undo to previous checkpoint
    pub fn undo(&mut self) -> bool {
        if let Some(prev) = self.history.pop() {
            self.bindings = prev;
            true
        } else {
            false
        }
    }

    /// Clear all bindings and history
    pub fn clear(&mut self) {
        self.bindings.clear();
        self.history.clear();
    }

    pub fn history_len(&self) -> usize {
        self.history.len()
    }

    pub fn can_undo(&self) -> bool {
        !self.history.is_empty()
    }
}

impl Default for Session {
    fn default() -> Self {
        Self::new()
    }
}

/// In-memory session store (legacy - for standalone MCP mode only)
static SESSIONS: LazyLock<RwLock<HashMap<String, Session>>> =
    LazyLock::new(|| RwLock::new(HashMap::new()));

/// Get a session by ID (cloned)
pub fn get_session(id: &str) -> Option<Session> {
    SESSIONS.read().ok()?.get(id).cloned()
}

/// Get session bindings as UUID map for execution context
pub fn get_session_bindings(id: &str) -> Option<HashMap<String, Uuid>> {
    SESSIONS.read().ok()?.get(id).map(|s| s.all_bindings())
}

/// Handle session action
pub fn session_context(action: SessionAction) -> Result<SessionState, String> {
    match action {
        SessionAction::Create => {
            let session = Session::new();
            let id = Uuid::new_v4().to_string();

            SESSIONS
                .write()
                .map_err(|e| e.to_string())?
                .insert(id.clone(), session);

            Ok(SessionState {
                session_id: id,
                bindings: HashMap::new(),
                history_count: 0,
                can_undo: false,
                stage_focus: None,
                relevant_verbs: None,
                available_stages: None,
            })
        }

        SessionAction::Get { session_id } => {
            let sessions = SESSIONS.read().map_err(|e| e.to_string())?;
            let session = sessions
                .get(&session_id)
                .ok_or_else(|| format!("Session not found: {}", session_id))?;

            Ok(to_session_state(&session_id, session))
        }

        SessionAction::Update {
            session_id,
            bindings,
        } => {
            let mut sessions = SESSIONS.write().map_err(|e| e.to_string())?;
            let session = sessions
                .get_mut(&session_id)
                .ok_or_else(|| format!("Session not found: {}", session_id))?;

            // Checkpoint before update
            session.checkpoint();

            for (name, uuid_str) in bindings {
                if let Ok(uuid) = Uuid::parse_str(&uuid_str) {
                    // Default entity_type to "unknown" - will be updated by execute
                    session.record_binding(name, uuid, "unknown".to_string());
                }
            }

            Ok(to_session_state(&session_id, session))
        }

        SessionAction::Undo { session_id } => {
            let mut sessions = SESSIONS.write().map_err(|e| e.to_string())?;
            let session = sessions
                .get_mut(&session_id)
                .ok_or_else(|| format!("Session not found: {}", session_id))?;

            session.undo();

            Ok(to_session_state(&session_id, session))
        }

        SessionAction::Clear { session_id } => {
            let mut sessions = SESSIONS.write().map_err(|e| e.to_string())?;
            let session = sessions
                .get_mut(&session_id)
                .ok_or_else(|| format!("Session not found: {}", session_id))?;

            session.clear();

            Ok(to_session_state(&session_id, session))
        }

        SessionAction::SetStageFocus {
            session_id,
            stage_code,
        } => {
            let mut sessions = SESSIONS.write().map_err(|e| e.to_string())?;
            let session = sessions
                .get_mut(&session_id)
                .ok_or_else(|| format!("Session not found: {}", session_id))?;

            // Validate stage exists if setting (not clearing)
            if let Some(ref code) = stage_code {
                let registry = SemanticStageRegistry::load_default()
                    .map_err(|e| format!("Failed to load stage registry: {}", e))?;
                if registry.get_stage(code).is_none() {
                    return Err(format!("Unknown stage: {}", code));
                }
            }

            session.set_stage_focus(stage_code);

            Ok(to_session_state(&session_id, session))
        }

        SessionAction::ListStages => {
            let registry = SemanticStageRegistry::load_default()
                .map_err(|e| format!("Failed to load stage registry: {}", e))?;

            let stages: Vec<StageInfo> = registry
                .stages_in_order()
                .map(|s| StageInfo {
                    code: s.code.clone(),
                    name: s.name.clone(),
                    description: s.description.clone(),
                    relevant_verbs: s.relevant_verbs.clone().unwrap_or_default(),
                })
                .collect();

            // Return a "virtual" session state with stages info
            // The caller should extract stages from the response
            Ok(SessionState {
                session_id: String::new(),
                bindings: HashMap::new(),
                history_count: 0,
                can_undo: false,
                stage_focus: None,
                relevant_verbs: None,
                available_stages: Some(stages),
            })
        }
    }
}

/// Update session with execution results
pub fn update_session_from_execution(
    session_id: &str,
    new_bindings: &HashMap<String, Uuid>,
    entity_types: &HashMap<String, String>,
) -> Result<(), String> {
    let mut sessions = SESSIONS.write().map_err(|e| e.to_string())?;
    let session = sessions
        .get_mut(session_id)
        .ok_or_else(|| format!("Session not found: {}", session_id))?;

    // Checkpoint before adding new bindings
    session.checkpoint();

    for (name, uuid) in new_bindings {
        let entity_type = entity_types.get(name).cloned().unwrap_or_default();
        session.record_binding(name.clone(), *uuid, entity_type);
    }

    Ok(())
}

fn to_session_state(id: &str, session: &Session) -> SessionState {
    // Get relevant verbs if stage is focused
    let relevant_verbs: Option<Vec<String>> = session.stage_focus().and_then(|code| {
        SemanticStageRegistry::load_default()
            .ok()
            .and_then(|reg| reg.get_stage(code).and_then(|s| s.relevant_verbs.clone()))
    });

    SessionState {
        session_id: id.to_string(),
        bindings: session.binding_info(),
        history_count: session.history_len(),
        can_undo: session.can_undo(),
        stage_focus: session.stage_focus().map(|s| s.to_string()),
        relevant_verbs,
        available_stages: None,
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_session_create_and_update() {
        let state = session_context(SessionAction::Create).unwrap();
        assert!(state.bindings.is_empty());

        let session_id = state.session_id.clone();

        // Update with bindings
        let mut bindings = HashMap::new();
        bindings.insert(
            "fund".to_string(),
            "550e8400-e29b-41d4-a716-446655440000".to_string(),
        );

        let state = session_context(SessionAction::Update {
            session_id: session_id.clone(),
            bindings,
        })
        .unwrap();

        assert_eq!(state.bindings.len(), 1);
        assert!(state.bindings.contains_key("fund"));
        assert!(state.can_undo);

        // Undo
        let state = session_context(SessionAction::Undo {
            session_id: session_id.clone(),
        })
        .unwrap();

        assert!(state.bindings.is_empty());
        assert!(!state.can_undo);
    }
}
