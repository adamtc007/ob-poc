//! UI Module for egui DSL Visualizer
//!
//! This module provides comprehensive UI components and state management
//! for the egui-based DSL visualizer with robust refresh handling.

pub mod global_dsl_state;
pub mod viewport_manager;

// Re-export key types for convenience
pub use global_dsl_state::{
    create_global_state_manager, AstNodeData, CbuData, DslContext, DslEntry, GlobalDslState,
    GlobalDslStateManager, OperationStatus, RefreshData, RefreshRequest, ViewportId,
};

pub use viewport_manager::{
    RefreshSummary, ViewportManager, ViewportManagerStatus, ViewportRefreshError,
    ViewportRefreshHandler, ViewportStatus,
};
