//! Platform service traits consumed by plugin ops via the [`crate::services::ServiceRegistry`].
//!
//! Each submodule defines one trait. Plugin ops in `dsl-runtime` call
//! [`crate::VerbExecutionContext::service::<dyn X>()`] to obtain the host's
//! impl; the host (ob-poc) registers impls at startup via
//! [`crate::services::ServiceRegistryBuilder::register`].
//!
//! Traits here are object-safe and `Send + Sync + 'static` — required for
//! trait-object storage in the registry.

pub mod semantic_state;

pub use semantic_state::SemanticStateService;
