//! End-to-End Tests: Verb Search → Client Resolution → Session Scope
//!
//! Run with:
//!   DATABASE_URL="postgresql:///data_designer" cargo test --features database --test client_group_e2e_test -- --ignored --nocapture

#[cfg(feature = "database")]
#[allow(unused_imports)]
mod e2e_tests {
    use anyhow::Result;
    use ob_poc::agent::learning::warmup::LearningWarmup;
    use ob_poc::database::verb_service::VerbService;
    use ob_poc::mcp::verb_search::HybridVerbSearcher;
    use ob_semantic_matcher::{
        client_group_resolver::ClientGroupResolveError, AnchorRole, ClientGroupAliasResolver,
        ClientGroupAnchorResolver, ClientGroupResolver, PgClientGroupResolver, ResolutionConfig,
    };
    use sqlx::PgPool;
    use std::sync::Arc;
    use tokio::sync::OnceCell;
    use uuid::Uuid;

    // Import the Candle embedder
    use ob_poc::agent::learning::embedder::CandleEmbedder;

    // Shared resources
    static SHARED_POOL: OnceCell<PgPool> = OnceCell::const_new();
    static SHARED_EMBEDDER: OnceCell<Arc<CandleEmbedder>> = OnceCell::const_new();

    pub(crate) async fn get_pool() -> &'static PgPool {
        SHARED_POOL
            .get_or_init(|| async {
                let url = std::env::var("DATABASE_URL")
                    .unwrap_or_else(|_| panic!("DATABASE_URL must be set for integration tests"));
                PgPool::connect(&url)
                    .await
                    .expect("Failed to connect to database")
            })
            .await
    }

    pub(crate) async fn get_embedder() -> &'static Arc<CandleEmbedder> {
        SHARED_EMBEDDER
            .get_or_init(|| async {
                let embedder = tokio::task::spawn_blocking(|| {
                    CandleEmbedder::new().expect("Failed to load embedder")
                })
                .await
                .expect("Embedder task panicked");
                Arc::new(embedder)
            })
            .await
    }

    /// Wrapper to make CandleEmbedder implement the ob_semantic_matcher::client_group_resolver::Embedder trait
    pub(crate) struct EmbedderAdapter(pub(crate) Arc<CandleEmbedder>);

    #[async_trait::async_trait]
    impl ob_semantic_matcher::client_group_resolver::Embedder for EmbedderAdapter {
        async fn embed_query(&self, text: &str) -> Result<Vec<f32>, String> {
            let text_owned = text.to_string();
            let embedder = self.0.clone();
            tokio::task::spawn_blocking(move || embedder.embed_query_blocking(&text_owned))
                .await
                .map_err(|e| e.to_string())?
                .map_err(|e| e.to_string())
        }

        async fn embed_target(&self, text: &str) -> Result<Vec<f32>, String> {
            let text_owned = text.to_string();
            let embedder = self.0.clone();
            tokio::task::spawn_blocking(move || embedder.embed_target_blocking(&text_owned))
                .await
                .map_err(|e| e.to_string())?
                .map_err(|e| e.to_string())
        }
    }

    pub(crate) async fn get_resolver() -> PgClientGroupResolver<EmbedderAdapter> {
        let pool = get_pool().await.clone();
        let embedder = get_embedder().await.clone();
        let adapter = EmbedderAdapter(embedder);
        PgClientGroupResolver::new(
            pool,
            Arc::new(adapter),
            "BAAI/bge-small-en-v1.5".to_string(),
        )
    }

    /// End-to-end test: "allianz" → session.load-cluster verb
    #[tokio::test]
    #[ignore]
    async fn test_e2e_allianz_verb_search() -> Result<()> {
        let pool = get_pool().await.clone();
        let embedder = get_embedder().await.clone();

        let verb_service = Arc::new(VerbService::new(pool.clone()));
        let warmup = LearningWarmup::new(pool.clone());
        let (learned_data, _stats) = warmup.warmup().await?;

        let dyn_embedder: Arc<dyn ob_poc::agent::learning::embedder::Embedder> =
            embedder.clone() as Arc<dyn ob_poc::agent::learning::embedder::Embedder>;
        let searcher = HybridVerbSearcher::new(verb_service.clone(), Some(learned_data.clone()))
            .with_embedder(dyn_embedder);

        let results = searcher
            .search("allianz", None, None, None, 5, None, None, None)
            .await?;

        assert!(!results.is_empty(), "Should find at least one verb match");
        let top = &results[0];

        assert_eq!(
            top.verb, "session.load-cluster",
            "Expected session.load-cluster for bare client name"
        );
        Ok(())
    }

    /// End-to-end test: "load the allianz book" → session.load-cluster
    #[tokio::test]
    #[ignore]
    async fn test_e2e_load_allianz_book() -> Result<()> {
        let pool = get_pool().await.clone();
        let embedder = get_embedder().await.clone();

        let verb_service = Arc::new(VerbService::new(pool.clone()));
        let warmup = LearningWarmup::new(pool.clone());
        let (learned_data, _stats) = warmup.warmup().await?;

        let dyn_embedder: Arc<dyn ob_poc::agent::learning::embedder::Embedder> =
            embedder.clone() as Arc<dyn ob_poc::agent::learning::embedder::Embedder>;
        let searcher = HybridVerbSearcher::new(verb_service.clone(), Some(learned_data.clone()))
            .with_embedder(dyn_embedder);

        let results = searcher
            .search("load the allianz book", None, None, None, 5, None, None, None)
            .await?;

        assert!(!results.is_empty(), "Should find at least one verb match");
        let top = &results[0];

        assert_eq!(
            top.verb, "session.load-cluster",
            "Expected session.load-cluster"
        );
        Ok(())
    }

    /// End-to-end test: Client group resolution for DSL execution
    #[tokio::test]
    #[ignore]
    async fn test_e2e_client_to_anchor_for_session() -> Result<()> {
        let pool = get_pool().await.clone();
        let allianz_group_id: Uuid = "11111111-1111-1111-1111-111111111111".parse()?;

        let anchor: Option<Uuid> = sqlx::query_scalar!(
            r#"
            SELECT anchor_entity_id as "anchor_entity_id!"
            FROM "ob-poc".resolve_client_group_anchor($1, 'governance_controller', '')
            "#,
            allianz_group_id
        )
        .fetch_optional(&pool)
        .await?;

        assert!(anchor.is_some(), "Should resolve to anchor entity");
        let anchor_id = anchor.unwrap();

        let name: Option<String> = sqlx::query_scalar!(
            r#"SELECT name as "name!" FROM "ob-poc".entities WHERE entity_id = $1"#,
            anchor_id
        )
        .fetch_optional(&pool)
        .await?;

        assert!(name.is_some(), "Anchor entity should exist with a name");
        Ok(())
    }

    /// End-to-end test: Full pipeline "allianz" → verb + client resolution
    #[tokio::test]
    #[ignore]
    async fn test_e2e_full_pipeline_allianz() -> Result<()> {
        let pool = get_pool().await.clone();
        let embedder = get_embedder().await.clone();

        let verb_service = Arc::new(VerbService::new(pool.clone()));
        let warmup = LearningWarmup::new(pool.clone());
        let (learned_data, _stats) = warmup.warmup().await?;

        let dyn_embedder: Arc<dyn ob_poc::agent::learning::embedder::Embedder> =
            embedder.clone() as Arc<dyn ob_poc::agent::learning::embedder::Embedder>;
        let searcher = HybridVerbSearcher::new(verb_service.clone(), Some(learned_data.clone()))
            .with_embedder(dyn_embedder);

        let results = searcher
            .search("allianz", None, None, None, 3, None, None, None)
            .await?;
        assert!(!results.is_empty());
        let verb = &results[0].verb;
        assert_eq!(verb, "session.load-cluster");

        let resolver = get_resolver().await;
        let config = ResolutionConfig::default();

        let alias_result = resolver.resolve_alias("allianz", &config).await?;
        let client_group_id = alias_result.group_id;

        let anchor = resolver
            .resolve_anchor(client_group_id, AnchorRole::GovernanceController, None)
            .await?;

        let anchor_name: Option<String> = sqlx::query_scalar!(
            r#"SELECT name as "name!" FROM "ob-poc".entities WHERE entity_id = $1"#,
            anchor.anchor_entity_id
        )
        .fetch_optional(&pool)
        .await?;

        assert!(anchor_name.is_some(), "Anchor entity should have a name");
        Ok(())
    }
}
