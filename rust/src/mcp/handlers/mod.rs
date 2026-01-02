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
//!
//! ## Future Refactoring
//!
//! Handler implementations can be incrementally extracted to sub-modules:
//! - `dsl.rs` - DSL validation, execution, planning handlers
//! - `cbu.rs` - CBU get/list handlers
//! - `entity.rs` - Entity get/search handlers
//! - `workflow.rs` - Workflow orchestration handlers
//! - `template.rs` - Template list/get/expand handlers
//! - `batch.rs` - Batch execution handlers
//! - `research.rs` - Research macro handlers

mod core;

// Re-export the main ToolHandlers struct
pub use core::ToolHandlers;
