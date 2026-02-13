//! MCP Tool Handlers - Modular Structure
//!
//! This module provides a modular organization for MCP tool handlers.
//! The core module contains the main ToolHandlers struct and all handler implementations.
//!
//! ## Architecture
//!
//! ```text
//! ToolHandlers (core.rs)
//!     ├── dispatch() - routes tool calls to handlers
//!     ├── DSL handlers (dsl_validate, dsl_execute, etc.)
//!     ├── CBU handlers (cbu_get, cbu_list)
//!     ├── Entity handlers (entity_get, entity_search)
//!     ├── Workflow handlers (workflow_status, etc.)
//!     ├── Template handlers (template_list, etc.)
//!     ├── Batch handlers (batch_start, etc.)
//!     └── Research handlers (research_list, etc.)
//! ```

mod batch_tools;
mod core;
mod learning_tools;
mod navigation_tools;
mod session_tools;

// Re-export the main ToolHandlers struct
pub use core::ToolHandlers;
