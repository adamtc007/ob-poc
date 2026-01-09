//! Viewport state and context management
//!
//! This module provides the `ViewportContext` which wraps the core `ViewportState`
//! with additional runtime context needed for rendering and navigation.

use std::collections::HashMap;
use uuid::Uuid;

use ob_poc_types::viewport::{
    CameraState, CbuRef, CbuViewMemory, CbuViewType, FocusManager, ViewportFilters,
    ViewportFocusState, ViewportState,
};

/// Extended viewport context with runtime state
///
/// This wraps `ViewportState` with additional context needed during
/// rendering and DSL execution.
#[derive(Debug, Clone)]
pub struct ViewportContext {
    /// Core viewport state (focus, filters, camera)
    pub state: ViewportState,

    /// Per-CBU view memory for persistence across navigation
    pub view_memory: HashMap<Uuid, CbuViewMemory>,

    /// Session ID for state persistence
    pub session_id: Option<Uuid>,

    /// Whether viewport state has unsaved changes
    pub dirty: bool,

    /// Last DSL command that modified the viewport
    pub last_command: Option<String>,
}

impl Default for ViewportContext {
    fn default() -> Self {
        Self::new()
    }
}

impl ViewportContext {
    /// Create a new viewport context with default state
    pub fn new() -> Self {
        Self {
            state: ViewportState::default(),
            view_memory: HashMap::new(),
            session_id: None,
            dirty: false,
            last_command: None,
        }
    }

    /// Create a viewport context with a session ID
    pub fn with_session(session_id: Uuid) -> Self {
        Self {
            session_id: Some(session_id),
            ..Self::new()
        }
    }

    /// Get the current focus state
    pub fn focus(&self) -> &ViewportFocusState {
        &self.state.focus.state
    }

    /// Get the focus manager
    pub fn focus_manager(&self) -> &FocusManager {
        &self.state.focus
    }

    /// Get mutable access to the focus manager
    pub fn focus_manager_mut(&mut self) -> &mut FocusManager {
        self.dirty = true;
        &mut self.state.focus
    }

    /// Get the camera state
    pub fn camera(&self) -> &CameraState {
        &self.state.camera
    }

    /// Get mutable access to the camera
    pub fn camera_mut(&mut self) -> &mut CameraState {
        self.dirty = true;
        &mut self.state.camera
    }

    /// Get the viewport filters
    pub fn filters(&self) -> &ViewportFilters {
        &self.state.filters
    }

    /// Get mutable access to the filters
    pub fn filters_mut(&mut self) -> &mut ViewportFilters {
        self.dirty = true;
        &mut self.state.filters
    }

    /// Save current view state for a CBU
    pub fn save_cbu_view(&mut self, cbu_id: Uuid) {
        let memory = CbuViewMemory {
            last_view: self.current_view_type(),
            last_enhance: self.current_enhance_level(),
            last_focus_path: vec![self.state.focus.state.clone()],
            camera: self.state.camera.clone(),
        };
        self.view_memory.insert(cbu_id, memory);
    }

    /// Restore saved view state for a CBU
    pub fn restore_cbu_view(&mut self, cbu_id: Uuid) -> bool {
        if let Some(memory) = self.view_memory.get(&cbu_id).cloned() {
            self.state.camera = memory.camera;
            self.state.view_type = memory.last_view;
            self.dirty = true;
            true
        } else {
            false
        }
    }

    /// Get the current view type based on focus state
    pub fn current_view_type(&self) -> CbuViewType {
        match &self.state.focus.state {
            ViewportFocusState::None => CbuViewType::Structure,
            ViewportFocusState::CbuContainer { .. } => CbuViewType::Structure,
            ViewportFocusState::CbuEntity { .. } => CbuViewType::Structure,
            ViewportFocusState::CbuProductService { .. } => CbuViewType::Accounts,
            ViewportFocusState::InstrumentMatrix { .. } => CbuViewType::Instruments,
            ViewportFocusState::InstrumentType { .. } => CbuViewType::Instruments,
            ViewportFocusState::ConfigNode { .. } => CbuViewType::Instruments,
        }
    }

    /// Get the current enhance level of the focused element
    pub fn current_enhance_level(&self) -> u8 {
        self.state.focus.state.primary_enhance_level()
    }

    /// Get the currently focused CBU, if any
    pub fn current_cbu(&self) -> Option<&CbuRef> {
        self.state.focus.state.cbu()
    }

    /// Get the currently focused CBU ID, if any
    pub fn current_cbu_id(&self) -> Option<Uuid> {
        self.current_cbu().map(|cbu| cbu.0)
    }

    /// Check if we're at the top level (no focus or CBU container)
    pub fn is_at_root(&self) -> bool {
        matches!(
            &self.state.focus.state,
            ViewportFocusState::None | ViewportFocusState::CbuContainer { .. }
        )
    }

    /// Check if we can ascend (have parent context)
    pub fn can_ascend(&self) -> bool {
        self.state.focus.can_ascend()
    }

    /// Get the focus stack depth
    pub fn focus_depth(&self) -> usize {
        self.state.focus.stack_depth()
    }

    /// Record a DSL command that modified the viewport
    pub fn record_command(&mut self, command: impl Into<String>) {
        self.last_command = Some(command.into());
        self.dirty = true;
    }

    /// Mark the viewport as clean (saved)
    pub fn mark_clean(&mut self) {
        self.dirty = false;
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_new_context_is_at_root() {
        let ctx = ViewportContext::new();
        assert!(ctx.is_at_root());
        assert!(!ctx.can_ascend());
        assert_eq!(ctx.focus_depth(), 0);
    }

    #[test]
    fn test_current_enhance_level_none() {
        let ctx = ViewportContext::new();
        assert_eq!(ctx.current_enhance_level(), 0);
    }

    #[test]
    fn test_dirty_flag() {
        let mut ctx = ViewportContext::new();
        assert!(!ctx.dirty);

        ctx.camera_mut().zoom = 2.0;
        assert!(ctx.dirty);

        ctx.mark_clean();
        assert!(!ctx.dirty);
    }
}
