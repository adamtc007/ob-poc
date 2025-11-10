//! Global DSL State Management
//!
//! This module provides centralized state management for the egui DSL visualizer,
//! ensuring all windows and viewports stay synchronized when DSL operations occur.

use chrono::{DateTime, Utc};
use serde::{Deserialize, Serialize};
use std::sync::{Arc, Mutex};
use tracing::{debug, error, info};
use uuid::Uuid;

/// Thread-safe global state manager
pub type GlobalDslStateManager = Arc<Mutex<GlobalDslState>>;

/// Global DSL state shared across all egui windows and viewports
#[derive(Debug, Clone)]
pub struct GlobalDslState {
    /// Current active DSL context (the "selected" DSL)
    pub current_context: Option<DslContext>,

    /// All available DSL instances (for browser/picker)
    pub available_dsls: Vec<DslEntry>,

    /// State change notifications
    pub change_counter: u64, // Incremented on every state change
    pub last_updated: DateTime<Utc>,

    /// Operation status
    pub operation_status: OperationStatus,

    /// ROBUST REFRESH STATE MANAGEMENT
    /// Each viewport tracks its own refresh state independently
    pub refresh_states: RefreshStateTracker,

    /// Available CBUs for creation
    pub available_cbus: Vec<CbuData>,
}

/// Current DSL context - represents the actively selected/viewed DSL
#[derive(Debug, Clone)]
pub struct DslContext {
    pub instance_id: Uuid,
    pub version_id: Uuid,
    pub domain_name: String,
    pub business_reference: String,
    pub dsl_content: Option<String>,   // Cached content
    pub ast_data: Option<AstNodeData>, // Cached AST
    pub is_newly_created: bool,
    pub created_at: DateTime<Utc>,
    pub version_number: i32,
}

/// DSL entry for browser/picker display
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct DslEntry {
    pub id: String,
    pub name: String,
    pub domain: String,
    pub version: i32,
    pub description: String,
    pub created_at: DateTime<Utc>,
    pub status: String,
}

/// CBU data for creation picker
#[derive(Debug, Clone)]
pub struct CbuData {
    pub cbu_id: Uuid,
    pub name: String,
    pub description: Option<String>,
    pub nature_purpose: Option<String>,
    pub customer_type: Option<String>,
    pub jurisdiction: Option<String>,
    pub status: String,
}

/// AST node data for visualization
#[derive(Debug, Clone)]
pub struct AstNodeData {
    pub id: String,
    pub node_type: String,
    pub label: String,
    pub properties: std::collections::HashMap<String, String>,
    pub children: Vec<AstNodeData>,
}

/// Current operation status across the application
#[derive(Debug, Clone)]
pub enum OperationStatus {
    Idle,
    CreatingDsl {
        cbu_id: Uuid,
        name: String,
        progress: f32, // 0.0 to 1.0
    },
    LoadingDsl {
        instance_id: Uuid,
        progress: f32,
    },
    LoadingDslList {
        progress: f32,
    },
    LoadingCbuList,
    Error {
        message: String,
        recoverable: bool,
    },
    Success {
        message: String,
        auto_clear_after: Option<DateTime<Utc>>,
    },
}

/// Robust refresh state tracker to avoid state clashes
#[derive(Debug, Clone)]
pub struct RefreshStateTracker {
    /// Individual viewport refresh requests (never cleared automatically)
    pub dsl_browser_requests: Vec<RefreshRequest>,
    pub dsl_content_requests: Vec<RefreshRequest>,
    pub ast_viewer_requests: Vec<RefreshRequest>,
    pub creation_form_requests: Vec<RefreshRequest>,
    pub status_panel_requests: Vec<RefreshRequest>,
    pub cbu_picker_requests: Vec<RefreshRequest>,

    /// Last processed request ID per viewport (to avoid duplicate processing)
    pub last_processed: std::collections::HashMap<ViewportId, u64>,
}

/// Individual refresh request with unique ID and reason
#[derive(Debug, Clone)]
pub struct RefreshRequest {
    pub id: u64,
    pub reason: String,
    pub requested_at: DateTime<Utc>,
    pub required_data: Option<RefreshData>,
}

/// Specific data needed for refresh
#[derive(Debug, Clone)]
pub enum RefreshData {
    NewDslContext(DslContext),
    UpdatedDslList(Vec<DslEntry>),
    UpdatedCbuList(Vec<CbuData>),
    Error(String),
    Success(String),
}

/// Viewport identifiers
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
pub enum ViewportId {
    DslBrowser,
    DslContent,
    AstViewer,
    CreationForm,
    StatusPanel,
    CbuPicker,
}

/// Legacy flags for backwards compatibility (deprecated)
#[derive(Debug, Clone, Default)]
pub struct ViewportRefreshFlags {
    pub dsl_browser: bool,   // List of DSLs needs refresh
    pub dsl_content: bool,   // DSL text content needs refresh
    pub ast_viewer: bool,    // AST tree needs refresh
    pub creation_form: bool, // Creation form needs reset
    pub status_panel: bool,  // Status/error panel needs refresh
    pub cbu_picker: bool,    // CBU picker needs refresh
    pub all: bool,           // Force refresh everything
}

impl GlobalDslState {
    /// Create new global state
    pub fn new() -> Self {
        Self {
            current_context: None,
            available_dsls: Vec::new(),
            change_counter: 0,
            last_updated: Utc::now(),
            operation_status: OperationStatus::Idle,
            refresh_states: RefreshStateTracker::new(),
            available_cbus: Vec::new(),
        }
    }

    /// ROBUST: Request viewport refresh (never overwrites existing requests)
    pub fn request_viewport_refresh(
        &mut self,
        viewport: ViewportId,
        reason: String,
        data: Option<RefreshData>,
    ) {
        self.change_counter += 1;
        self.last_updated = Utc::now();

        let request = RefreshRequest {
            id: self.change_counter, // Use change counter as unique ID
            reason: reason.clone(),
            requested_at: Utc::now(),
            required_data: data,
        };

        let request_id = request.id; // Capture ID before moving

        // Add to appropriate viewport queue (NEVER clears existing requests)
        match viewport {
            ViewportId::DslBrowser => self.refresh_states.dsl_browser_requests.push(request),
            ViewportId::DslContent => self
                .refresh_states
                .dsl_content_requests
                .push(request.clone()),
            ViewportId::AstViewer => self
                .refresh_states
                .ast_viewer_requests
                .push(request.clone()),
            ViewportId::CreationForm => self
                .refresh_states
                .creation_form_requests
                .push(request.clone()),
            ViewportId::StatusPanel => self
                .refresh_states
                .status_panel_requests
                .push(request.clone()),
            ViewportId::CbuPicker => self
                .refresh_states
                .cbu_picker_requests
                .push(request.clone()),
        }

        debug!(
            "Requested viewport refresh: {:?} - {} (request ID: {})",
            viewport, reason, request_id
        );
    }

    /// LEGACY: Mark state as changed with flags (deprecated - use request_viewport_refresh)
    pub fn mark_changed(&mut self, refresh_flags: ViewportRefreshFlags) {
        // Convert legacy flags to new request system
        if refresh_flags.dsl_browser || refresh_flags.all {
            self.request_viewport_refresh(
                ViewportId::DslBrowser,
                "Legacy flag update".to_string(),
                None,
            );
        }
        if refresh_flags.dsl_content || refresh_flags.all {
            self.request_viewport_refresh(
                ViewportId::DslContent,
                "Legacy flag update".to_string(),
                None,
            );
        }
        if refresh_flags.ast_viewer || refresh_flags.all {
            self.request_viewport_refresh(
                ViewportId::AstViewer,
                "Legacy flag update".to_string(),
                None,
            );
        }
        if refresh_flags.creation_form || refresh_flags.all {
            self.request_viewport_refresh(
                ViewportId::CreationForm,
                "Legacy flag update".to_string(),
                None,
            );
        }
        if refresh_flags.status_panel || refresh_flags.all {
            self.request_viewport_refresh(
                ViewportId::StatusPanel,
                "Legacy flag update".to_string(),
                None,
            );
        }
        if refresh_flags.cbu_picker || refresh_flags.all {
            self.request_viewport_refresh(
                ViewportId::CbuPicker,
                "Legacy flag update".to_string(),
                None,
            );
        }
    }

    /// Set current DSL context (when user selects a DSL)
    pub fn set_current_context(&mut self, context: DslContext) {
        info!(
            "Setting current DSL context: {} ({})",
            context.business_reference, context.instance_id
        );

        let context_clone = context.clone();
        self.current_context = Some(context);

        // ROBUST: Request specific refreshes with context data
        self.request_viewport_refresh(
            ViewportId::DslContent,
            format!("New DSL context: {}", context_clone.business_reference),
            Some(RefreshData::NewDslContext(context_clone.clone())),
        );
        self.request_viewport_refresh(
            ViewportId::AstViewer,
            format!("New AST context: {}", context_clone.business_reference),
            Some(RefreshData::NewDslContext(context_clone.clone())),
        );
        self.request_viewport_refresh(ViewportId::StatusPanel, "Context changed".to_string(), None);
    }

    /// Clear current DSL context
    pub fn clear_current_context(&mut self) {
        self.current_context = None;

        // ROBUST: Request specific refreshes for context clearing
        self.request_viewport_refresh(ViewportId::DslContent, "Context cleared".to_string(), None);
        self.request_viewport_refresh(ViewportId::AstViewer, "Context cleared".to_string(), None);
        self.request_viewport_refresh(ViewportId::StatusPanel, "Context cleared".to_string(), None);
    }

    /// Update operation status
    pub fn set_operation_status(&mut self, status: OperationStatus) {
        debug!("Operation status changed: {:?}", status);
        let status_msg = status.get_message();
        self.operation_status = status;

        // ROBUST: Request status panel refresh with specific status
        self.request_viewport_refresh(
            ViewportId::StatusPanel,
            format!("Status update: {}", status_msg),
            None,
        );
    }

    /// Update available DSL list
    pub fn set_available_dsls(&mut self, dsls: Vec<DslEntry>) {
        info!("Updated available DSLs: {} entries", dsls.len());
        let dsl_count = dsls.len();
        self.available_dsls = dsls.clone();

        // ROBUST: Request browser refresh with updated list
        self.request_viewport_refresh(
            ViewportId::DslBrowser,
            format!("DSL list updated: {} entries", dsl_count),
            Some(RefreshData::UpdatedDslList(dsls)),
        );
    }

    /// Update available CBU list
    pub fn set_available_cbus(&mut self, cbus: Vec<CbuData>) {
        info!("Updated available CBUs: {} entries", cbus.len());
        let cbu_count = cbus.len();
        self.available_cbus = cbus.clone();

        // ROBUST: Request CBU picker refresh with updated list
        self.request_viewport_refresh(
            ViewportId::CbuPicker,
            format!("CBU list updated: {} entries", cbu_count),
            Some(RefreshData::UpdatedCbuList(cbus)),
        );
    }

    /// Add newly created DSL to the list
    pub fn add_newly_created_dsl(&mut self, dsl_entry: DslEntry, context: DslContext) {
        info!(
            "Adding newly created DSL: {} ({})",
            dsl_entry.name, dsl_entry.id
        );

        // Add to available DSLs list
        self.available_dsls.push(dsl_entry.clone());

        // Set as current context
        let context_clone = context.clone();
        self.current_context = Some(context);

        // ROBUST: Request all viewport refreshes with specific reasons
        self.request_viewport_refresh(
            ViewportId::DslBrowser,
            format!("New DSL created: {}", dsl_entry.name),
            Some(RefreshData::UpdatedDslList(self.available_dsls.clone())),
        );
        self.request_viewport_refresh(
            ViewportId::DslContent,
            format!("Load new DSL: {}", dsl_entry.name),
            Some(RefreshData::NewDslContext(context_clone.clone())),
        );
        self.request_viewport_refresh(
            ViewportId::AstViewer,
            format!("Load new AST: {}", dsl_entry.name),
            Some(RefreshData::NewDslContext(context_clone.clone())),
        );
        self.request_viewport_refresh(
            ViewportId::CreationForm,
            "DSL creation successful - reset form".to_string(),
            Some(RefreshData::Success("DSL created successfully".to_string())),
        );
        self.request_viewport_refresh(
            ViewportId::StatusPanel,
            format!("DSL creation successful: {}", dsl_entry.name),
            Some(RefreshData::Success(format!(
                "Successfully created: {}",
                dsl_entry.name
            ))),
        );
    }

    /// ROBUST: Check for pending refresh requests for a specific viewport
    pub fn get_pending_refreshes(&mut self, viewport: ViewportId) -> Vec<RefreshRequest> {
        let last_processed = self
            .refresh_states
            .last_processed
            .get(&viewport)
            .copied()
            .unwrap_or(0);

        let pending_requests: Vec<RefreshRequest> = match viewport {
            ViewportId::DslBrowser => self
                .refresh_states
                .dsl_browser_requests
                .iter()
                .filter(|req| req.id > last_processed)
                .cloned()
                .collect(),
            ViewportId::DslContent => self
                .refresh_states
                .dsl_content_requests
                .iter()
                .filter(|req| req.id > last_processed)
                .cloned()
                .collect(),
            ViewportId::AstViewer => self
                .refresh_states
                .ast_viewer_requests
                .iter()
                .filter(|req| req.id > last_processed)
                .cloned()
                .collect(),
            ViewportId::CreationForm => self
                .refresh_states
                .creation_form_requests
                .iter()
                .filter(|req| req.id > last_processed)
                .cloned()
                .collect(),
            ViewportId::StatusPanel => self
                .refresh_states
                .status_panel_requests
                .iter()
                .filter(|req| req.id > last_processed)
                .cloned()
                .collect(),
            ViewportId::CbuPicker => self
                .refresh_states
                .cbu_picker_requests
                .iter()
                .filter(|req| req.id > last_processed)
                .cloned()
                .collect(),
        };

        if !pending_requests.is_empty() {
            debug!(
                "Found {} pending refresh requests for {:?}",
                pending_requests.len(),
                viewport
            );
        }

        pending_requests
    }

    /// ROBUST: Mark refresh requests as processed (ONLY call AFTER successful refresh)
    pub fn mark_refreshes_processed(&mut self, viewport: ViewportId, up_to_request_id: u64) {
        self.refresh_states
            .last_processed
            .insert(viewport, up_to_request_id);
        debug!(
            "Marked refresh requests processed for {:?} up to ID {}",
            viewport, up_to_request_id
        );
    }

    /// LEGACY: Check if any viewports need refresh and consume the flags (deprecated)
    pub fn consume_refresh_flags(&mut self) -> ViewportRefreshFlags {
        // Legacy compatibility - check if ANY viewport has pending requests
        ViewportRefreshFlags {
            dsl_browser: !self
                .get_pending_refreshes(ViewportId::DslBrowser)
                .is_empty(),
            dsl_content: !self
                .get_pending_refreshes(ViewportId::DslContent)
                .is_empty(),
            ast_viewer: !self.get_pending_refreshes(ViewportId::AstViewer).is_empty(),
            creation_form: !self
                .get_pending_refreshes(ViewportId::CreationForm)
                .is_empty(),
            status_panel: !self
                .get_pending_refreshes(ViewportId::StatusPanel)
                .is_empty(),
            cbu_picker: !self.get_pending_refreshes(ViewportId::CbuPicker).is_empty(),
            all: false,
        }
    }

    /// Force refresh all viewports
    pub fn force_refresh_all(&mut self) {
        let reason = "Force refresh all".to_string();
        self.request_viewport_refresh(ViewportId::DslBrowser, reason.clone(), None);
        self.request_viewport_refresh(ViewportId::DslContent, reason.clone(), None);
        self.request_viewport_refresh(ViewportId::AstViewer, reason.clone(), None);
        self.request_viewport_refresh(ViewportId::CreationForm, reason.clone(), None);
        self.request_viewport_refresh(ViewportId::StatusPanel, reason.clone(), None);
        self.request_viewport_refresh(ViewportId::CbuPicker, reason, None);
    }

    /// Get summary for debugging
    pub fn summary(&self) -> String {
        format!(
            "GlobalDslState(counter={}, dsls={}, current={}, status={:?})",
            self.change_counter,
            self.available_dsls.len(),
            self.current_context
                .as_ref()
                .map(|c| c.business_reference.as_str())
                .unwrap_or("None"),
            self.operation_status
        )
    }
}

impl Default for GlobalDslState {
    fn default() -> Self {
        Self::new()
    }
}

impl OperationStatus {
    /// Check if operation is in progress
    pub fn is_in_progress(&self) -> bool {
        matches!(
            self,
            OperationStatus::CreatingDsl { .. }
                | OperationStatus::LoadingDsl { .. }
                | OperationStatus::LoadingDslList { .. }
                | OperationStatus::LoadingCbuList
        )
    }

    /// Get progress percentage if applicable
    pub fn get_progress(&self) -> Option<f32> {
        match self {
            OperationStatus::CreatingDsl { progress, .. } => Some(*progress),
            OperationStatus::LoadingDsl { progress, .. } => Some(*progress),
            OperationStatus::LoadingDslList { progress } => Some(*progress),
            _ => None,
        }
    }

    /// Get user-friendly status message
    pub fn get_message(&self) -> String {
        match self {
            OperationStatus::Idle => "Ready".to_string(),
            OperationStatus::CreatingDsl { name, .. } => format!("Creating DSL: {}", name),
            OperationStatus::LoadingDsl { instance_id, .. } => {
                format!("Loading DSL: {}", instance_id)
            }
            OperationStatus::LoadingDslList { .. } => "Loading DSL list...".to_string(),
            OperationStatus::LoadingCbuList => "Loading CBU list...".to_string(),
            OperationStatus::Error { message, .. } => format!("Error: {}", message),
            OperationStatus::Success { message, .. } => message.clone(),
        }
    }
}

impl ViewportRefreshFlags {
    /// Create flags for refreshing everything
    pub fn all() -> Self {
        Self {
            dsl_browser: true,
            dsl_content: true,
            ast_viewer: true,
            creation_form: true,
            status_panel: true,
            cbu_picker: true,
            all: true,
        }
    }

    /// Check if any refresh is needed
    pub fn needs_refresh(&self) -> bool {
        self.dsl_browser
            || self.dsl_content
            || self.ast_viewer
            || self.creation_form
            || self.status_panel
            || self.cbu_picker
            || self.all
    }
}

/// Helper to create a new global state manager
pub fn create_global_state_manager() -> GlobalDslStateManager {
    Arc::new(Mutex::new(GlobalDslState::new()))
}

impl RefreshStateTracker {
    pub fn new() -> Self {
        Self {
            dsl_browser_requests: Vec::new(),
            dsl_content_requests: Vec::new(),
            ast_viewer_requests: Vec::new(),
            creation_form_requests: Vec::new(),
            status_panel_requests: Vec::new(),
            cbu_picker_requests: Vec::new(),
            last_processed: std::collections::HashMap::new(),
        }
    }
}

impl Default for RefreshStateTracker {
    fn default() -> Self {
        Self::new()
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_global_state_creation() {
        let state = GlobalDslState::new();
        assert_eq!(state.change_counter, 0);
        assert!(state.current_context.is_none());
        assert!(state.available_dsls.is_empty());
    }

    #[test]
    fn test_robust_refresh_requests() {
        let mut state = GlobalDslState::new();
        assert_eq!(state.change_counter, 0);

        // Request refresh for browser
        state.request_viewport_refresh(ViewportId::DslBrowser, "Test reason".to_string(), None);
        assert_eq!(state.change_counter, 1);

        // Request another refresh for content - should not overwrite browser request
        state.request_viewport_refresh(ViewportId::DslContent, "Another reason".to_string(), None);
        assert_eq!(state.change_counter, 2);

        // Check both requests exist
        let browser_requests = state.get_pending_refreshes(ViewportId::DslBrowser);
        let content_requests = state.get_pending_refreshes(ViewportId::DslContent);

        assert_eq!(browser_requests.len(), 1);
        assert_eq!(content_requests.len(), 1);
        assert_eq!(browser_requests[0].reason, "Test reason");
        assert_eq!(content_requests[0].reason, "Another reason");
    }

    #[test]
    fn test_refresh_processing() {
        let mut state = GlobalDslState::new();

        // Add multiple requests
        state.request_viewport_refresh(ViewportId::DslBrowser, "Request 1".to_string(), None);
        state.request_viewport_refresh(ViewportId::DslBrowser, "Request 2".to_string(), None);

        // Should have 2 pending requests
        let pending = state.get_pending_refreshes(ViewportId::DslBrowser);
        assert_eq!(pending.len(), 2);

        // Mark first request as processed
        state.mark_refreshes_processed(ViewportId::DslBrowser, 1);

        // Should have 1 pending request
        let pending = state.get_pending_refreshes(ViewportId::DslBrowser);
        assert_eq!(pending.len(), 1);
        assert_eq!(pending[0].reason, "Request 2");
    }

    #[test]
    fn test_operation_status_progress() {
        let status = OperationStatus::CreatingDsl {
            cbu_id: Uuid::new_v4(),
            name: "Test".to_string(),
            progress: 0.5,
        };

        assert!(status.is_in_progress());
        assert_eq!(status.get_progress(), Some(0.5));
    }
}
