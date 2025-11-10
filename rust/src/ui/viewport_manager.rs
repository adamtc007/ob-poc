//! Viewport Manager for Safe Refresh Handling
//!
//! This module provides safe refresh handling for egui viewports, ensuring that
//! refresh requests are processed correctly without state clashes or premature resets.

use crate::ui::global_dsl_state::{GlobalDslStateManager, RefreshData, RefreshRequest, ViewportId};
use std::collections::HashMap;
use tracing::{debug, error, info, warn};

/// Safe viewport refresh manager
pub struct ViewportManager {
    /// Reference to global state
    global_state: GlobalDslStateManager,

    /// Per-viewport refresh handlers
    refresh_handlers: HashMap<ViewportId, Box<dyn ViewportRefreshHandler>>,

    /// Last successful refresh timestamps per viewport
    last_refresh_success: HashMap<ViewportId, chrono::DateTime<chrono::Utc>>,

    /// Refresh attempt counters (for error handling)
    refresh_attempt_counts: HashMap<ViewportId, u32>,
}

/// Trait for viewport-specific refresh logic
pub trait ViewportRefreshHandler: Send + Sync {
    /// Execute the refresh for this viewport
    fn refresh(&mut self, requests: Vec<RefreshRequest>) -> Result<(), ViewportRefreshError>;

    /// Get viewport display name
    fn viewport_name(&self) -> &'static str;

    /// Check if this viewport can handle the given refresh data
    fn can_handle_data(&self, data: &RefreshData) -> bool;
}

/// Refresh execution errors
#[derive(Debug, thiserror::Error)]
pub enum ViewportRefreshError {
    #[error("Refresh failed: {message}")]
    RefreshFailed { message: String },

    #[error("Data unavailable: {data_type}")]
    DataUnavailable { data_type: String },

    #[error("Viewport busy: {viewport:?}")]
    ViewportBusy { viewport: ViewportId },

    #[error("Multiple refresh failures: {count}")]
    MultipleFailures { count: u32 },
}

impl ViewportManager {
    /// Create new viewport manager
    pub fn new(global_state: GlobalDslStateManager) -> Self {
        Self {
            global_state,
            refresh_handlers: HashMap::new(),
            last_refresh_success: HashMap::new(),
            refresh_attempt_counts: HashMap::new(),
        }
    }

    /// Register a refresh handler for a viewport
    pub fn register_handler(
        &mut self,
        viewport: ViewportId,
        handler: Box<dyn ViewportRefreshHandler>,
    ) {
        info!("Registering refresh handler for viewport: {:?}", viewport);
        self.refresh_handlers.insert(viewport, handler);
        self.refresh_attempt_counts.insert(viewport, 0);
    }

    /// SAFE: Process all pending refresh requests
    /// This is the main entry point called from the UI update loop
    pub fn process_pending_refreshes(&mut self) -> Result<RefreshSummary, ViewportRefreshError> {
        let mut summary = RefreshSummary::new();

        // Get all viewport types
        let viewports = [
            ViewportId::DslBrowser,
            ViewportId::DslContent,
            ViewportId::AstViewer,
            ViewportId::CreationForm,
            ViewportId::StatusPanel,
            ViewportId::CbuPicker,
        ];

        for viewport in viewports {
            match self.process_viewport_refreshes(viewport) {
                Ok(viewport_summary) => {
                    summary.merge(viewport_summary);
                }
                Err(e) => {
                    error!("Failed to refresh viewport {:?}: {}", viewport, e);
                    summary.errors.push((viewport, e));
                }
            }
        }

        debug!("Refresh processing complete: {}", summary.summary());
        Ok(summary)
    }

    /// Process refresh requests for a specific viewport
    fn process_viewport_refreshes(
        &mut self,
        viewport: ViewportId,
    ) -> Result<ViewportRefreshSummary, ViewportRefreshError> {
        // SAFE: Get pending requests without modifying state
        let pending_requests = {
            let mut state =
                self.global_state
                    .lock()
                    .map_err(|_| ViewportRefreshError::RefreshFailed {
                        message: "Failed to acquire state lock".to_string(),
                    })?;
            state.get_pending_refreshes(viewport)
        };

        if pending_requests.is_empty() {
            return Ok(ViewportRefreshSummary::empty(viewport));
        }

        debug!(
            "Processing {} refresh requests for viewport {:?}",
            pending_requests.len(),
            viewport
        );

        // Check if we have a handler for this viewport
        if !self.refresh_handlers.contains_key(&viewport) {
            return Err(ViewportRefreshError::RefreshFailed {
                message: format!("No handler registered for viewport {:?}", viewport),
            });
        }

        // SAFE: Execute refresh with error handling
        let refresh_result = self.execute_refresh_internal(viewport, pending_requests.clone());

        match refresh_result {
            Ok(_) => {
                // SAFE: Only mark as processed AFTER successful refresh
                self.mark_refresh_success(viewport, &pending_requests)?;

                Ok(ViewportRefreshSummary {
                    viewport,
                    requests_processed: pending_requests.len(),
                    success: true,
                    error: None,
                })
            }
            Err(e) => {
                // SAFE: Don't mark as processed on failure
                self.handle_refresh_failure(viewport, &e);

                Ok(ViewportRefreshSummary {
                    viewport,
                    requests_processed: 0,
                    success: false,
                    error: Some(e),
                })
            }
        }
    }

    /// Execute refresh with safety checks and error handling - internal method
    fn execute_refresh_internal(
        &mut self,
        viewport: ViewportId,
        requests: Vec<RefreshRequest>,
    ) -> Result<(), ViewportRefreshError> {
        let start_time = std::time::Instant::now();

        // Increment attempt counter
        let attempts = self.refresh_attempt_counts.entry(viewport).or_insert(0);
        *attempts += 1;

        // Safety check: Too many failures?
        if *attempts > 5 {
            warn!(
                "Viewport {:?} has failed {} times, throttling refresh",
                viewport, attempts
            );
            return Err(ViewportRefreshError::MultipleFailures { count: *attempts });
        }

        // Execute the actual refresh
        let result = {
            let handler = self.refresh_handlers.get_mut(&viewport).unwrap();
            handler.refresh(requests)
        };

        let duration = start_time.elapsed();

        match result {
            Ok(_) => {
                // Reset failure counter on success
                self.refresh_attempt_counts.insert(viewport, 0);

                debug!(
                    "Viewport {:?} refresh successful in {:?}",
                    viewport, duration
                );
                Ok(())
            }
            Err(e) => {
                error!(
                    "Viewport {:?} refresh failed in {:?}: {}",
                    viewport, duration, e
                );
                Err(e)
            }
        }
    }

    /// SAFE: Mark refresh requests as successfully processed
    fn mark_refresh_success(
        &mut self,
        viewport: ViewportId,
        processed_requests: &[RefreshRequest],
    ) -> Result<(), ViewportRefreshError> {
        // Find the highest request ID that was processed
        let max_request_id = processed_requests
            .iter()
            .map(|req| req.id)
            .max()
            .unwrap_or(0);

        // SAFE: Only update state AFTER confirming successful refresh
        {
            let mut state =
                self.global_state
                    .lock()
                    .map_err(|_| ViewportRefreshError::RefreshFailed {
                        message: "Failed to acquire state lock for success marking".to_string(),
                    })?;

            state.mark_refreshes_processed(viewport, max_request_id);
        }

        // Update local success tracking
        self.last_refresh_success
            .insert(viewport, chrono::Utc::now());

        info!(
            "Marked {} refresh requests as processed for viewport {:?}",
            processed_requests.len(),
            viewport
        );

        Ok(())
    }

    /// Handle refresh failure (don't mark requests as processed)
    fn handle_refresh_failure(&mut self, viewport: ViewportId, error: &ViewportRefreshError) {
        warn!(
            "Refresh failed for viewport {:?}: {} (requests remain pending)",
            viewport, error
        );

        // Requests remain in the pending queue for retry
        // This is SAFE - we don't lose refresh requests on failure
    }

    /// Get refresh status for debugging
    pub fn get_status(&self) -> ViewportManagerStatus {
        let mut status = ViewportManagerStatus {
            registered_handlers: self.refresh_handlers.len(),
            viewport_statuses: HashMap::new(),
        };

        for viewport in [
            ViewportId::DslBrowser,
            ViewportId::DslContent,
            ViewportId::AstViewer,
            ViewportId::CreationForm,
            ViewportId::StatusPanel,
            ViewportId::CbuPicker,
        ] {
            let pending_count = {
                if let Ok(state) = self.global_state.lock() {
                    let mut state_guard = state;
                    state_guard.get_pending_refreshes(viewport).len()
                } else {
                    0
                }
            };

            let viewport_status = ViewportStatus {
                has_handler: self.refresh_handlers.contains_key(&viewport),
                pending_refresh_count: pending_count,
                last_refresh: self.last_refresh_success.get(&viewport).copied(),
                failure_count: self
                    .refresh_attempt_counts
                    .get(&viewport)
                    .copied()
                    .unwrap_or(0),
            };

            status.viewport_statuses.insert(viewport, viewport_status);
        }

        status
    }
}

/// Summary of refresh processing results
#[derive(Debug)]
pub struct RefreshSummary {
    pub total_requests_processed: usize,
    pub successful_viewports: Vec<ViewportId>,
    pub failed_viewports: Vec<ViewportId>,
    pub errors: Vec<(ViewportId, ViewportRefreshError)>,
}

impl RefreshSummary {
    pub fn new() -> Self {
        Self {
            total_requests_processed: 0,
            successful_viewports: Vec::new(),
            failed_viewports: Vec::new(),
            errors: Vec::new(),
        }
    }

    pub fn merge(&mut self, viewport_summary: ViewportRefreshSummary) {
        self.total_requests_processed += viewport_summary.requests_processed;

        if viewport_summary.success {
            self.successful_viewports.push(viewport_summary.viewport);
        } else {
            self.failed_viewports.push(viewport_summary.viewport);
            if let Some(error) = viewport_summary.error {
                self.errors.push((viewport_summary.viewport, error));
            }
        }
    }

    pub fn summary(&self) -> String {
        format!(
            "Processed {} requests: {} successful, {} failed",
            self.total_requests_processed,
            self.successful_viewports.len(),
            self.failed_viewports.len()
        )
    }
}

/// Summary for individual viewport refresh
#[derive(Debug)]
pub struct ViewportRefreshSummary {
    pub viewport: ViewportId,
    pub requests_processed: usize,
    pub success: bool,
    pub error: Option<ViewportRefreshError>,
}

impl ViewportRefreshSummary {
    pub fn empty(viewport: ViewportId) -> Self {
        Self {
            viewport,
            requests_processed: 0,
            success: true,
            error: None,
        }
    }
}

/// Overall viewport manager status
#[derive(Debug)]
pub struct ViewportManagerStatus {
    pub registered_handlers: usize,
    pub viewport_statuses: HashMap<ViewportId, ViewportStatus>,
}

/// Individual viewport status
#[derive(Debug)]
pub struct ViewportStatus {
    pub has_handler: bool,
    pub pending_refresh_count: usize,
    pub last_refresh: Option<chrono::DateTime<chrono::Utc>>,
    pub failure_count: u32,
}

impl ViewportManagerStatus {
    pub fn summary(&self) -> String {
        let total_pending: usize = self
            .viewport_statuses
            .values()
            .map(|status| status.pending_refresh_count)
            .sum();

        let total_failures: u32 = self
            .viewport_statuses
            .values()
            .map(|status| status.failure_count)
            .sum();

        format!(
            "ViewportManager: {} handlers, {} pending refreshes, {} total failures",
            self.registered_handlers, total_pending, total_failures
        )
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::ui::global_dsl_state::create_global_state_manager;
    use std::sync::{Arc, Mutex};

    struct MockRefreshHandler {
        name: &'static str,
        should_fail: Arc<Mutex<bool>>,
    }

    impl MockRefreshHandler {
        fn new(name: &'static str) -> Self {
            Self {
                name,
                should_fail: Arc::new(Mutex::new(false)),
            }
        }

        fn set_should_fail(&self, should_fail: bool) {
            *self.should_fail.lock().unwrap() = should_fail;
        }
    }

    impl ViewportRefreshHandler for MockRefreshHandler {
        fn refresh(&mut self, _requests: Vec<RefreshRequest>) -> Result<(), ViewportRefreshError> {
            if *self.should_fail.lock().unwrap() {
                Err(ViewportRefreshError::RefreshFailed {
                    message: "Mock failure".to_string(),
                })
            } else {
                Ok(())
            }
        }

        fn viewport_name(&self) -> &'static str {
            self.name
        }

        fn can_handle_data(&self, _data: &RefreshData) -> bool {
            true
        }
    }

    #[test]
    fn test_viewport_manager_creation() {
        let state = create_global_state_manager();
        let manager = ViewportManager::new(state);

        let status = manager.get_status();
        assert_eq!(status.registered_handlers, 0);
    }

    #[test]
    fn test_handler_registration() {
        let state = create_global_state_manager();
        let mut manager = ViewportManager::new(state);

        let handler = Box::new(MockRefreshHandler::new("test"));
        manager.register_handler(ViewportId::DslBrowser, handler);

        let status = manager.get_status();
        assert_eq!(status.registered_handlers, 1);
        assert!(status.viewport_statuses[&ViewportId::DslBrowser].has_handler);
    }

    #[test]
    fn test_safe_refresh_processing() {
        let state = create_global_state_manager();
        let mut manager = ViewportManager::new(state.clone());

        // Register handler
        let handler = Box::new(MockRefreshHandler::new("test"));
        manager.register_handler(ViewportId::DslBrowser, handler);

        // Add refresh request
        {
            let mut state_guard = state.lock().unwrap();
            state_guard.request_viewport_refresh(
                ViewportId::DslBrowser,
                "Test refresh".to_string(),
                None,
            );
        }

        // Process refreshes
        let summary = manager.process_pending_refreshes().unwrap();
        assert_eq!(summary.total_requests_processed, 1);
        assert_eq!(summary.successful_viewports.len(), 1);
        assert_eq!(summary.failed_viewports.len(), 0);
    }
}
