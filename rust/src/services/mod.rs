//! Services module for core business logic implementations
//!
//! This module contains the service implementations that provide
//! business logic and external interfaces for the DSL v2 engine.
//!
//! ## Architecture
//! DSL operations flow through dsl_v2::DslExecutor. Services here provide
//! specialized operations for entity search, document extraction, etc.

// Entity search is now handled by EntityGateway gRPC service.
// See rust/crates/entity-gateway/ for the central lookup service.

// Attribute services
pub mod attribute_executor;
pub(crate) mod attribute_identity_service;
pub(crate) mod attribute_registry_enrichment;

// Dictionary and document services
pub mod dictionary_service_impl;
pub mod document_attribute_crud_service;
pub mod document_catalog_source;
pub mod document_extraction_service;
pub mod extraction_service;

// Executor services
pub mod sink_executor;
pub mod source_executor;

// DSL enrichment (source → segments for UI display)
pub mod dsl_enrichment;

// Viewport resolution (lazy loading for viewport state)
pub mod viewport_resolution_service;

// Board control rules engine (computes who controls the board)
pub mod board_control_rules;

// Phase 5a — ob-poc-side impl of the `dsl_runtime::service_traits::SemanticStateService`
// trait, registered with the platform `ServiceRegistry` at host startup.
pub mod semantic_state_service_impl;

// Phase 5a — ob-poc-side impl of the
// `dsl_runtime::service_traits::StewardshipDispatch` trait; bridges to
// `crate::sem_reg::stewardship::dispatch_phase{0,1}_tool`.
pub mod stewardship_dispatch_impl;

// Phase 5a composite-blocker #1 — ob-poc-side impl of
// `dsl_runtime::service_traits::McpToolRegistry`; bridges to
// `crate::sem_reg::agent::mcp_tools::all_tool_specs()`.
pub mod mcp_tool_registry_impl;

// Phase 5a composite-blocker #5 — ob-poc-side impl of
// `dsl_runtime::service_traits::LifecycleCatalog`; bridges to the
// taxonomy-loaded `OntologyService` singleton.
pub mod lifecycle_catalog_impl;

// Phase 5a composite-blocker #8 — ob-poc-side impl of
// `dsl_runtime::service_traits::AttributeIdentityService`; bridges to
// the in-crate `attribute_identity_service::AttributeIdentityService`
// (multi-namespace UNION query over dictionary, registry, SemOS defs).
pub mod attribute_identity_dispatch_impl;

// Phase 5a composite-blocker #9 — ob-poc-side impl of
// `dsl_runtime::service_traits::ConstellationRuntime`; bridges to
// `crate::sem_os_runtime::constellation_runtime::handle_constellation_{hydrate,summary}`.
pub mod constellation_runtime_impl;

// Phase 5a composite-blocker #11 — ob-poc-side impl of
// `dsl_runtime::service_traits::TradingProfileDocument`; bridges to
// `crate::trading_profile::ast_db::{load_document, save_document}`.
pub mod trading_profile_document_impl;

// Phase 5a composite-blocker #24 — ob-poc-side impl of
// `dsl_runtime::service_traits::SemOsContextResolver`; bridges to
// `crate::sem_reg::agent::mcp_tools::build_sem_os_service(pool).resolve_context(...)`.
pub mod sem_os_context_resolver_impl;

// Phase 5a composite-blocker #25 — ob-poc-side impl of
// `dsl_runtime::service_traits::SchemaIntrospectionAccess`; bridges to
// `crate::ontology::ontology()`, `crate::dsl_v2::verb_registry::registry()`,
// and `crate::sem_reg::store::SnapshotStore` for the 5 structure-
// semantics verbs in `sem_os_schema_ops`.
pub mod schema_introspection_impl;

// Phase 5a composite-blocker #26 — ob-poc-side impl of
// `dsl_runtime::service_traits::ViewService`; single-method dispatch
// for all 15 `view.*` verbs. Bridges to `crate::session::ViewState`
// + `crate::taxonomy::*` (both multi-consumer mega-modules that stay
// in ob-poc).
pub mod view_service_impl;

// Phase 5a composite-blocker #27 — ob-poc-side impl of
// `dsl_runtime::service_traits::SessionService`; single-method dispatch
// for all 19 `session.*` verbs. Bridges to `crate::session::UnifiedSession`
// (the 10934 LOC multi-consumer session mega-module that stays in ob-poc).
pub mod session_service_impl;

// Phase 5a composite-blocker #28 — ob-poc-side impl of
// `dsl_runtime::service_traits::ServicePipelineService`; multi-domain
// single-method dispatch for the 16 verbs across the
// intent → discovery → attribute → provisioning → readiness pipeline.
// Bridges to `crate::service_resources::*` (engines, orchestrators,
// SRDEF registry loader) which stay in ob-poc.
pub mod service_pipeline_service_impl;

// Phase 5a composite-blocker #29 — ob-poc-side impl of
// `dsl_runtime::service_traits::PhraseService`; single-method dispatch
// for the 9 governed-phrase-authoring verbs. Bridges to
// `crate::sem_reg::store::SnapshotStore` + `crate::sem_reg::types::*`
// + `crate::sem_reg::ids::object_id_for` and the embedding-similarity
// SQL on `verb_pattern_embeddings` / `phrase_bank` / `session_traces`.
pub mod phrase_service_impl;

// Phase 5a composite-blocker #30 — ob-poc-side impl of
// `dsl_runtime::service_traits::AttributeService`; multi-domain
// single-method dispatch for the 16 verbs across `attribute.*`,
// `document.*`, and `derivation.*`. Bridges to
// `crate::sem_reg::derivation_spec`, `crate::sem_reg::store::SnapshotStore`,
// `crate::sem_reg::types::*`, and `crate::services::attribute_identity_service`
// (deferred from slice #8). Returns
// `AttributeDispatchOutcome { outcome, bindings }` — the wrapper applies
// `bindings` via `ctx.bind` for the 3 `define*` verbs that bind `@attribute`.
pub mod attribute_service_impl;

// Re-exports
pub use attribute_identity_dispatch_impl::ObPocAttributeIdentityService;
pub use constellation_runtime_impl::ObPocConstellationRuntime;
pub use schema_introspection_impl::ObPocSchemaIntrospectionAccess;
pub use sem_os_context_resolver_impl::ObPocSemOsContextResolver;
pub use trading_profile_document_impl::ObPocTradingProfileDocument;
pub use attribute_service_impl::ObPocAttributeService;
pub use phrase_service_impl::ObPocPhraseService;
pub use service_pipeline_service_impl::ObPocServicePipelineService;
pub use session_service_impl::ObPocSessionService;
pub use view_service_impl::ObPocViewService;
pub use board_control_rules::{BoardControlResult, BoardControlRulesEngine, RulesEngineConfig};
pub use lifecycle_catalog_impl::ObPocLifecycleCatalog;
pub use mcp_tool_registry_impl::ObPocMcpToolRegistry;
pub use semantic_state_service_impl::ObPocSemanticStateService;
pub use stewardship_dispatch_impl::ObPocStewardshipDispatch;
pub use dictionary_service_impl::DictionaryServiceImpl;
pub use document_attribute_crud_service::DocumentAttributeCrudService;
pub use document_extraction_service::DocumentExtractionService;
pub use dsl_enrichment::{bindings_from_session_context, enrich_dsl, BindingInfo};
pub use sink_executor::CompositeSinkExecutor;
pub use source_executor::CompositeSourceExecutor;
pub use viewport_resolution_service::ViewportResolutionService;
