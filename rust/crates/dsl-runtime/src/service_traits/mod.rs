//! Platform service traits consumed by plugin ops via the [`crate::services::ServiceRegistry`].
//!
//! Each submodule defines one trait. Plugin ops in `dsl-runtime` call
//! [`crate::VerbExecutionContext::service::<dyn X>()`] to obtain the host's
//! impl; the host (ob-poc) registers impls at startup via
//! [`crate::services::ServiceRegistryBuilder::register`].
//!
//! Traits here are object-safe and `Send + Sync + 'static` — required for
//! trait-object storage in the registry.

mod attribute_identity;
mod attribute_service;
mod constellation_runtime;
mod lifecycle_catalog;
mod mcp_tool_registry;
mod phrase_service;
mod process_registry;
mod schema_introspection;
mod sem_os_child_dispatcher;
mod sem_os_context_resolver;
mod semantic_state;
mod service_pipeline_service;
mod session_service;
mod stewardship;
mod trading_profile_document;
mod view_service;

pub use attribute_identity::AttributeIdentityService;
pub use attribute_service::{AttributeDispatchOutcome, AttributeService};
pub use constellation_runtime::ConstellationRuntime;
pub use lifecycle_catalog::LifecycleCatalog;
pub use mcp_tool_registry::{McpToolRegistry, McpToolSpec};
pub use phrase_service::PhraseService;
pub use process_registry::ProcessRegistryService;
pub use schema_introspection::SchemaIntrospectionAccess;
pub use sem_os_child_dispatcher::SemOsChildDispatcher;
pub use sem_os_context_resolver::SemOsContextResolver;
pub use semantic_state::SemanticStateService;
pub use service_pipeline_service::ServicePipelineService;
pub use session_service::SessionService;
pub use stewardship::{StewardshipDispatch, StewardshipOutcome};
pub use trading_profile_document::TradingProfileDocument;
pub use view_service::ViewService;
