//! Agent state and initialization for the agent REST API.
//!
//! Contains `AgentState` (shared state for all agent route handlers)
//! and the `create_agent_router_with_semantic()` entry point.
//!
//! Chat endpoints flow through the single intent pipeline (orchestrator →
//! verb search → DSL generation → runbook execution). Non-chat endpoints
//! (template authoring, research tools) may use LLM helpers for content
//! generation, but execution remains runbook-gated.

use crate::agent::learning::warmup::LearningWarmup;
use crate::api::session::SessionStore;
use crate::database::generation_log_repository::GenerationLogRepository;
use crate::dsl_v2::execution::DslExecutor;
use crate::entity_linking::{
    EntityLinkingService, EntityLinkingServiceImpl, StubEntityLinkingService,
};

use axum::Router;
use sqlx::PgPool;
use std::sync::Arc;

use crate::policy::PolicyGate;
use crate::sage::{DeterministicSage, LlmSage, SageEngine};

// ============================================================================
// State
// ============================================================================

#[derive(Clone)]
pub struct AgentState {
    pub pool: PgPool,
    pub dsl_v2_executor: Arc<DslExecutor>,
    pub sessions: SessionStore,
    pub session_manager: crate::api::session_manager::SessionManager,
    pub generation_log: Arc<GenerationLogRepository>,
    pub session_repo: Arc<crate::database::SessionRepository>,
    pub dsl_repo: Arc<crate::database::DslRepository>,
    pub agent_service: Arc<crate::api::agent_service::AgentService>,
    pub expansion_audit: Arc<crate::database::ExpansionAuditRepository>,
    /// Entity linking service for in-memory entity resolution
    pub entity_linker: Arc<dyn EntityLinkingService>,
    /// Server-side policy enforcement for single-pipeline invariants
    pub policy_gate: Arc<PolicyGate>,
    /// Semantic OS client for SemReg verb filtering (governance enforcement)
    pub sem_os_client: Option<Arc<dyn sem_os_client::SemOsClient>>,
    /// REPL V2 orchestrator used by unified `/api/session/:id/input` adapter.
    pub repl_v2_orchestrator: Option<Arc<crate::sequencer::ReplOrchestratorV2>>,
}

impl AgentState {
    fn build_sage_engine() -> Arc<dyn SageEngine> {
        if std::env::var("SAGE_LLM").ok().as_deref() == Some("1") {
            match ob_agentic::client_factory::create_llm_client() {
                Ok(client) => {
                    tracing::info!(
                        provider = client.provider_name(),
                        model = client.model_name(),
                        "SAGE_LLM=1 enabled; using LlmSage"
                    );
                    return Arc::new(LlmSage::new(client));
                }
                Err(error) => {
                    tracing::warn!(
                        error = %error,
                        "SAGE_LLM=1 requested but LLM client unavailable; falling back to DeterministicSage"
                    );
                }
            }
        }

        Arc::new(DeterministicSage)
    }

    /// Create with semantic verb search (blocks on embedder init ~3-5s)
    ///
    /// This is the primary constructor. Initializes Candle embedder synchronously
    /// so semantic search is available immediately when server starts accepting requests.
    pub async fn with_semantic(
        pool: PgPool,
        sessions: SessionStore,
        sem_os_client: Option<Arc<dyn sem_os_client::SemOsClient>>,
    ) -> Self {
        Self::with_semantic_and_plugin_registry(pool, sessions, sem_os_client, None, None).await
    }

    /// Variant of [`Self::with_semantic`] that accepts the canonical SemOS
    /// plugin op registry and platform service registry. Required for
    /// production — without the registry, plugin verbs in the agent legacy
    /// and runbook-gate paths hard-fail post-Phase-5c-migrate slice #80.
    ///
    /// The shorter [`Self::with_semantic`] preserves the legacy signature
    /// for tests that don't exercise plugin dispatch.
    pub async fn with_semantic_and_plugin_registry(
        pool: PgPool,
        sessions: SessionStore,
        sem_os_client: Option<Arc<dyn sem_os_client::SemOsClient>>,
        sem_os_ops: Option<Arc<sem_os_postgres::ops::SemOsVerbOpRegistry>>,
        service_registry: Option<Arc<dsl_runtime::ServiceRegistry>>,
    ) -> Self {
        use crate::agent::learning::embedder::CandleEmbedder;

        let dsl_v2_executor = Arc::new(DslExecutor::new(pool.clone()));
        let generation_log = Arc::new(GenerationLogRepository::new(pool.clone()));
        let session_repo = Arc::new(crate::database::SessionRepository::new(pool.clone()));
        let dsl_repo = Arc::new(crate::database::DslRepository::new(pool.clone()));
        let session_manager = crate::api::session_manager::SessionManager::new(sessions.clone());
        let sage_engine = Self::build_sage_engine();

        // Initialize embedder synchronously (blocks ~3-5s, but only at startup)
        // This is REQUIRED - server cannot start without semantic search
        tracing::info!("Initializing Candle embedder...");
        let start = std::time::Instant::now();

        let embedder: Arc<CandleEmbedder> = match tokio::task::spawn_blocking(CandleEmbedder::new)
            .await
        {
            Ok(Ok(e)) => {
                tracing::info!("Candle embedder ready in {}ms", start.elapsed().as_millis());
                Arc::new(e)
            }
            Ok(Err(e)) => {
                panic!("FATAL: Failed to initialize Candle embedder: {}. Server cannot start without semantic search.", e);
            }
            Err(e) => {
                panic!("FATAL: Candle embedder task panicked: {}. Server cannot start without semantic search.", e);
            }
        };

        // Load learned data (invocation_phrases, entity_aliases) for exact match lookup
        // This enables step 2 (global learned exact match) in verb search
        tracing::info!("Loading learned data for verb search...");
        let warmup_start = std::time::Instant::now();
        let warmup = LearningWarmup::new(pool.clone());
        let learned_data = match warmup.warmup().await {
            Ok((data, stats)) => {
                tracing::info!(
                    "Learned data loaded in {}ms: {} invocation phrases, {} entity aliases",
                    warmup_start.elapsed().as_millis(),
                    stats.invocation_phrases_loaded,
                    stats.entity_aliases_loaded
                );
                Some(data)
            }
            Err(e) => {
                tracing::warn!(
                    "Failed to load learned data: {}. Verb search will use semantic-only mode.",
                    e
                );
                None
            }
        };

        // Load lexicon snapshot for fast in-memory lexical verb search (Phase A of 072)
        // This runs BEFORE semantic search for exact label/token matches
        let lexicon: Option<crate::mcp::verb_search::SharedLexicon> = {
            use crate::lexicon::{LexiconServiceImpl, LexiconSnapshot};
            use std::path::Path;

            // Look for snapshot in standard locations
            let snapshot_paths = [
                Path::new("rust/assets/lexicon.snapshot.bin"),
                Path::new("assets/lexicon.snapshot.bin"),
                Path::new("../rust/assets/lexicon.snapshot.bin"),
            ];

            let mut loaded = None;
            for path in &snapshot_paths {
                if path.exists() {
                    match LexiconSnapshot::load_binary(path) {
                        Ok(snapshot) => {
                            tracing::info!(
                                hash = %snapshot.hash,
                                verbs = snapshot.verb_meta.len(),
                                entity_types = snapshot.entity_types.len(),
                                "Loaded lexicon snapshot from {}",
                                path.display()
                            );
                            loaded = Some(Arc::new(LexiconServiceImpl::new(Arc::new(snapshot)))
                                as Arc<dyn crate::lexicon::LexiconService>);
                            break;
                        }
                        Err(e) => {
                            tracing::warn!(
                                "Failed to load lexicon snapshot from {}: {}",
                                path.display(),
                                e
                            );
                        }
                    }
                }
            }

            if loaded.is_none() {
                tracing::warn!(
                    "Lexicon snapshot not found. Lexical search disabled. \
                     Run `cargo xtask lexicon compile` to generate."
                );
            }

            loaded
        };

        // Load entity linking snapshot for in-memory entity resolution (Phase 073)
        // This enables fast entity mention extraction without DB queries
        let entity_linker: Arc<dyn EntityLinkingService> = {
            let snapshot_path = std::env::var("ENTITY_SNAPSHOT_PATH")
                .unwrap_or_else(|_| "rust/assets/entity.snapshot.bin".to_string());
            let path = std::path::Path::new(&snapshot_path);

            // Also check alternate paths if primary doesn't exist
            let paths_to_try = [
                path.to_path_buf(),
                std::path::PathBuf::from("assets/entity.snapshot.bin"),
                std::path::PathBuf::from("../rust/assets/entity.snapshot.bin"),
            ];

            let mut loaded: Option<Arc<dyn EntityLinkingService>> = None;
            for try_path in &paths_to_try {
                if try_path.exists() {
                    match EntityLinkingServiceImpl::load_from(try_path) {
                        Ok(svc) => {
                            tracing::info!(
                                entities = svc.entity_count(),
                                version = svc.snapshot_version(),
                                hash = %&svc.snapshot_hash()[..12.min(svc.snapshot_hash().len())],
                                path = %try_path.display(),
                                "Loaded entity linking snapshot"
                            );
                            loaded = Some(Arc::new(svc));
                            break;
                        }
                        Err(e) => {
                            tracing::warn!(
                                path = %try_path.display(),
                                error = %e,
                                "Failed to load entity snapshot"
                            );
                        }
                    }
                }
            }

            loaded.unwrap_or_else(|| {
                tracing::info!(
                    "Entity snapshot not found. Entity linking disabled. \
                     Run `cargo xtask entity compile` to generate."
                );
                Arc::new(StubEntityLinkingService::new())
            })
        };

        // Load server-side policy from environment (single-pipeline enforcement)
        let policy_gate = Arc::new(PolicyGate::from_env());
        tracing::info!(
            strict = policy_gate.strict_single_pipeline,
            strict_semreg = policy_gate.strict_semreg,
            "PolicyGate loaded from environment"
        );

        // Load MacroRegistry once at startup (cached in AgentService)
        let macro_dir =
            std::path::PathBuf::from(env!("CARGO_MANIFEST_DIR")).join("config/verb_schemas/macros");
        let macro_registry = Arc::new(
            crate::dsl_v2::macros::load_macro_registry_from_dir(&macro_dir).unwrap_or_else(|e| {
                tracing::warn!(
                    "Failed to load operator macros: {}, using empty registry",
                    e
                );
                crate::dsl_v2::macros::MacroRegistry::new()
            }),
        );

        // Build MacroIndex for deterministic Tier -2B macro search
        let macro_index: Option<Arc<crate::mcp::macro_index::MacroIndex>> = {
            let mi = crate::mcp::macro_index::MacroIndex::from_registry(&macro_registry, None);
            tracing::info!(
                entries = mi.len(),
                "MacroIndex built for Chat API (Tier -2B)"
            );
            Some(Arc::new(mi))
        };

        // Load ScenarioIndex for journey-level Tier -2A resolution
        let scenario_index: Option<Arc<crate::mcp::scenario_index::ScenarioIndex>> = {
            let search_paths = [
                "rust/config/scenario_index.yaml",
                "config/scenario_index.yaml",
                "../rust/config/scenario_index.yaml",
            ];
            let mut loaded = None;
            for rel in &search_paths {
                let path = std::path::Path::new(rel);
                if path.exists() {
                    match crate::mcp::scenario_index::ScenarioIndex::from_yaml_file(path) {
                        Ok(si) => {
                            tracing::info!(
                                scenarios = si.len(),
                                path = %path.display(),
                                "ScenarioIndex loaded for Chat API (Tier -2A)"
                            );
                            loaded = Some(Arc::new(si));
                            break;
                        }
                        Err(e) => {
                            tracing::warn!(
                                "Failed to load scenario_index.yaml from {}: {}",
                                path.display(),
                                e
                            );
                        }
                    }
                }
            }
            if loaded.is_none() {
                tracing::info!(
                    "ScenarioIndex not found (Tier -2A disabled for Chat API). \
                     Looked in: rust/config/, config/, ../rust/config/"
                );
            }
            loaded
        };

        // Build agent service with embedder, learned data, lexicon, entity linker, and search indices
        let mut agent_service = crate::api::agent_service::AgentService::new(
            pool.clone(),
            embedder,
            learned_data,
            lexicon,
        )
        .with_entity_linker(entity_linker.clone())
        .with_macro_registry(macro_registry)
        .with_sage_engine(sage_engine);

        // Wire search indices for MacroIndex and ScenarioIndex
        if let Some(mi) = macro_index {
            agent_service = agent_service.with_macro_index(mi);
        }
        if let Some(si) = scenario_index {
            agent_service = agent_service.with_scenario_index(si);
        }

        // F1 fix (Slice 2.1b): thread the canonical SemOS plugin op registry
        // and platform service registry so agent-side `DslExecutor` /
        // `RealDslExecutor` construction inside `AgentService` reaches plugin
        // verbs without the "no SemOsVerbOp registered" hard-fail.
        if let Some(ref ops) = sem_os_ops {
            agent_service = agent_service.with_sem_os_ops(ops.clone());
            tracing::info!(
                registered_ops = ops.len(),
                "AgentService wired with SemOsVerbOpRegistry"
            );
        }
        if let Some(ref services) = service_registry {
            agent_service = agent_service.with_service_registry(services.clone());
            tracing::info!("AgentService wired with platform ServiceRegistry");
        }

        // Wire SemOsClient if provided (env-driven in main.rs)
        if let Some(ref client) = sem_os_client {
            agent_service = agent_service.with_sem_os_client(client.clone());
            tracing::info!("AgentService wired with SemOsClient");
        }

        let expansion_audit =
            Arc::new(crate::database::ExpansionAuditRepository::new(pool.clone()));

        Self {
            pool,
            dsl_v2_executor,
            sessions,
            session_manager,
            generation_log,
            session_repo,
            dsl_repo,
            agent_service: Arc::new(agent_service),
            expansion_audit,
            entity_linker,
            policy_gate,
            sem_os_client,
            repl_v2_orchestrator: None,
        }
    }
}

// ============================================================================
// Router Entry Point
// ============================================================================

/// Create agent router with semantic verb search
///
/// This is the ONLY constructor. Initializes Candle embedder synchronously
/// so semantic search is available immediately when server starts accepting requests.
/// There is no non-semantic path - all chat goes through the IntentPipeline.
pub async fn create_agent_router_with_semantic(
    pool: PgPool,
    sessions: SessionStore,
    sem_os_client: Option<Arc<dyn sem_os_client::SemOsClient>>,
) -> Router {
    let state = AgentState::with_semantic(pool, sessions, sem_os_client).await;
    crate::api::agent_routes::create_agent_router_with_state(state)
}

/// Create agent router with semantic verb search and an optional REPL V2 adapter.
#[allow(clippy::too_many_arguments)]
pub async fn create_agent_router_with_semantic_and_repl(
    pool: PgPool,
    sessions: SessionStore,
    sem_os_client: Option<Arc<dyn sem_os_client::SemOsClient>>,
    repl_v2_orchestrator: Option<Arc<crate::sequencer::ReplOrchestratorV2>>,
    sem_os_ops: Option<Arc<sem_os_postgres::ops::SemOsVerbOpRegistry>>,
    service_registry: Option<Arc<dsl_runtime::ServiceRegistry>>,
) -> Router {
    let mut state = AgentState::with_semantic_and_plugin_registry(
        pool,
        sessions,
        sem_os_client,
        sem_os_ops,
        service_registry,
    )
    .await;
    state.repl_v2_orchestrator = repl_v2_orchestrator.clone();
    let router = crate::api::agent_routes::create_agent_router_with_state(state);
    if let Some(orchestrator) = repl_v2_orchestrator {
        let repl_state = crate::api::repl_routes_v2::ReplV2RouteState { orchestrator };
        router
            .nest(
                "/api/repl/v2",
                crate::api::repl_routes_v2::router().with_state(repl_state.clone()),
            )
            .merge(crate::api::repl_routes_v2::session_scoped_router().with_state(repl_state))
    } else {
        router
    }
}
