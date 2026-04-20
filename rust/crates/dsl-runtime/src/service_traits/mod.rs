//! Platform service traits consumed by plugin ops via the [`crate::services::ServiceRegistry`].
//!
//! Each submodule defines one trait. Plugin ops in `dsl-runtime` call
//! [`crate::VerbExecutionContext::service::<dyn X>()`] to obtain the host's
//! impl; the host (ob-poc) registers impls at startup via
//! [`crate::services::ServiceRegistryBuilder::register`].
//!
//! Traits here are object-safe and `Send + Sync + 'static` — required for
//! trait-object storage in the registry.

pub mod attribute_identity;
pub mod constellation_runtime;
pub mod lifecycle_catalog;
pub mod mcp_tool_registry;
pub mod schema_introspection;
pub mod sem_os_context_resolver;
pub mod semantic_state;
pub mod session_service;
pub mod stewardship;
pub mod trading_profile_document;
pub mod view_service;

pub use attribute_identity::AttributeIdentityService;
pub use constellation_runtime::ConstellationRuntime;
pub use lifecycle_catalog::LifecycleCatalog;
pub use mcp_tool_registry::{McpToolRegistry, McpToolSpec};
pub use schema_introspection::SchemaIntrospectionAccess;
pub use sem_os_context_resolver::SemOsContextResolver;
pub use semantic_state::SemanticStateService;
pub use session_service::SessionService;
pub use stewardship::{StewardshipDispatch, StewardshipOutcome};
pub use trading_profile_document::TradingProfileDocument;
pub use view_service::ViewService;
