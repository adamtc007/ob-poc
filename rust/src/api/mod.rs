//! REST API module for DSL v2 operations
//!
//! This module provides HTTP endpoints for the DSL v2 system,
//! allowing external clients to interact with the system via REST API.

#[cfg(feature = "server")]
pub mod attribute_routes;

#[cfg(feature = "server")]
pub mod agent_types;

// agent_dsl_routes: deleted — DSL generation through unified REPL pipeline
// agent_learning_routes: deleted — verb selection through unified REPL pipeline

#[cfg(feature = "server")]
pub mod agent_routes;

#[cfg(feature = "server")]
pub mod agent_state;

#[cfg(feature = "server")]
pub mod policy_headers;

#[cfg(feature = "server")]
pub mod session;

#[cfg(feature = "server")]
pub mod session_manager;

#[cfg(feature = "server")]
pub mod dsl_session_file;

#[cfg(feature = "server")]
pub mod agent_service;

#[cfg(feature = "server")]
pub mod entity_routes;

#[cfg(feature = "server")]
pub mod dsl_viewer_routes;

#[cfg(feature = "server")]
pub mod graph_routes;

#[cfg(feature = "server")]
pub mod trading_matrix_routes;

#[cfg(feature = "server")]
pub mod capital_routes;

#[cfg(feature = "server")]
pub mod constellation_routes;

#[cfg(feature = "server")]
pub mod workflow_routes;

// Phase 3 slice 2z (2026-05-13): display-noun translation table relocated to
// ob-poc-boundary. Zero internal-crate deps; only serde_json/HashMap/LazyLock.
// The pub-use `translate_json` / `translate_string` / `DisplayNounTranslator`
// re-export below keeps the same external API surface.
#[cfg(feature = "server")]
pub use ob_poc_authoring::display_nouns;

// ob-poc-domain split v1 Slice A2 (2026-05-14): deal_types now lives in
// `ob-poc-deal`. Compat re-export keeps `crate::api::deal_types::*` paths
// in `deal_routes`, `deal_repository`, and `deal_graph_builder` working.
#[cfg(feature = "server")]
pub use ob_poc_deal as deal_types;

#[cfg(feature = "server")]
pub mod deal_routes;

#[cfg(feature = "server")]
pub mod graph_scene_projection;
pub mod observatory_routes;

#[cfg(feature = "server")]
pub mod catalogue_routes;

pub mod acp_dsl_dag_coverage;
pub mod agent_enrichment;
pub mod repl_routes_v2;
pub mod response_adapter;

#[cfg(feature = "server")]
pub use attribute_routes::create_attribute_router;

#[cfg(feature = "server")]
pub use entity_routes::{create_entity_router};
pub(crate) use entity_routes::{create_scoped_entity_router};

#[cfg(feature = "server")]
pub use agent_state::{create_agent_router_with_semantic, AgentState};

#[cfg(feature = "server")]
pub use agent_state::create_agent_router_with_semantic_and_repl;

#[cfg(feature = "server")]
pub use dsl_viewer_routes::create_dsl_viewer_router;

#[cfg(feature = "server")]
pub use graph_routes::{create_graph_router, create_session_graph_router};

#[cfg(feature = "server")]
pub use session::{create_session_store};
pub(crate) use session::{SessionStore};

#[cfg(feature = "server")]
pub(crate) use session_manager::{SessionManager, SessionSnapshot, SessionWatcher};

#[cfg(feature = "server")]
pub use agent_service::{AgentCommand, AgentService, ChatRequest};
pub(crate) use agent_service::{AgentChatResponse, ClientScope};

#[cfg(feature = "server")]
pub use trading_matrix_routes::create_trading_matrix_router;

#[cfg(feature = "server")]
pub(crate) use capital_routes::create_capital_router;

#[cfg(feature = "server")]
pub use constellation_routes::create_constellation_router;

#[cfg(feature = "server")]
pub(crate) use workflow_routes::{create_workflow_router, WorkflowState};

#[cfg(feature = "server")]
pub use display_nouns::{translate_json, translate_string, DisplayNounTranslator};

#[cfg(feature = "server")]
pub use deal_types::{
    DealContractSummary, DealFilters, DealGraphResponse, DealListResponse, DealParticipantSummary,
    DealProductSummary, DealSummary, DealViewMode, LoadDealRequest, LoadDealResponse,
    OnboardingRequestSummary, RateCardDetail, RateCardLineSummary, RateCardSummary,
    SessionDealContext,
};

#[cfg(feature = "server")]
pub use deal_routes::{create_deal_router};
pub(crate) use deal_routes::{create_deal_router_simple, DealState};

pub use repl_routes_v2::{router as create_repl_v2_router};
pub(crate) use repl_routes_v2::{navigation_router as create_repl_navigation_router, ReplV2RouteState};
