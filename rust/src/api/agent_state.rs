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
use crate::dsl_v2::DslExecutor;
use crate::entity_linking::{
    EntityLinkingService, EntityLinkingServiceImpl, StubEntityLinkingService,
};

use axum::Router;
use sqlx::PgPool;
use std::sync::Arc;

use crate::policy::PolicyGate;

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
}

impl AgentState {
    /// Create with semantic verb search (blocks on embedder init ~3-5s)
    ///
    /// This is the primary constructor. Initializes Candle embedder synchronously
    /// so semantic search is available immediately when server starts accepting requests.
    pub async fn with_semantic(
        pool: PgPool,
        sessions: SessionStore,
        sem_os_client: Option<Arc<dyn sem_os_client::SemOsClient>>,
    ) -> Self {
        use crate::agent::learning::embedder::CandleEmbedder;

        let dsl_v2_executor = Arc::new(DslExecutor::new(pool.clone()));
        let generation_log = Arc::new(GenerationLogRepository::new(pool.clone()));
        let session_repo = Arc::new(crate::database::SessionRepository::new(pool.clone()));
        let dsl_repo = Arc::new(crate::database::DslRepository::new(pool.clone()));
        let session_manager = crate::api::session_manager::SessionManager::new(sessions.clone());

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
            allow_raw_execute = policy_gate.allow_raw_execute,
            "PolicyGate loaded from environment"
        );

        // Build agent service with embedder, learned data, lexicon, and entity linker
        let mut agent_service = crate::api::agent_service::AgentService::new(
            pool.clone(),
            embedder,
            learned_data,
            lexicon,
        )
        .with_entity_linker(entity_linker.clone());

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
