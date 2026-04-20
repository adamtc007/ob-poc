//! Platform service traits consumed by plugin ops via the [`crate::services::ServiceRegistry`].
//!
//! Each submodule defines one trait. Plugin ops in `dsl-runtime` call
//! [`crate::VerbExecutionContext::service::<dyn X>()`] to obtain the host's
//! impl; the host (ob-poc) registers impls at startup via
//! [`crate::services::ServiceRegistryBuilder::register`].
//!
//! Traits here are object-safe and `Send + Sync + 'static` — required for
//! trait-object storage in the registry.

pub mod lifecycle_catalog;
pub mod mcp_tool_registry;
pub mod semantic_state;
pub mod stewardship;

pub use lifecycle_catalog::LifecycleCatalog;
pub use mcp_tool_registry::{McpToolRegistry, McpToolSpec};
pub use semantic_state::SemanticStateService;
pub use stewardship::{StewardshipDispatch, StewardshipOutcome};
