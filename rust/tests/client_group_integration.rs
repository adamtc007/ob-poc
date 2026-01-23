//! Integration tests for client group resolution
//!
//! Tests the two-stage client group resolver:
//!   Stage 1: Alias → ClientGroupId (exact + semantic)
//!   Stage 2: ClientGroupId → AnchorEntityId (role-based)
//!
//! Run all tests:
//!   DATABASE_URL="postgresql:///data_designer" cargo test --features database --test client_group_integration -- --ignored --nocapture
//!
//! Run specific test:
//!   DATABASE_URL="postgresql:///data_designer" cargo test --features database --test client_group_integration test_exact_match -- --ignored --nocapture

#[cfg(feature = "database")]
mod tests {
    use anyhow::Result;
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

    pub async fn get_pool() -> &'static PgPool {
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

    pub async fn get_embedder() -> &'static Arc<CandleEmbedder> {
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
    pub struct EmbedderAdapter(pub Arc<CandleEmbedder>);

    #[async_trait::async_trait]
    impl ob_semantic_matcher::client_group_resolver::Embedder for EmbedderAdapter {
        async fn embed_query(&self, text: &str) -> Result<Vec<f32>, String> {
            // Use blocking method via spawn_blocking for async safety
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

    pub async fn get_resolver() -> PgClientGroupResolver<EmbedderAdapter> {
        let pool = get_pool().await.clone();
        let embedder = get_embedder().await.clone();
        let adapter = EmbedderAdapter(embedder);
        PgClientGroupResolver::new(
            pool,
            Arc::new(adapter),
            "BAAI/bge-small-en-v1.5".to_string(),
        )
    }

    // =========================================================================
    // Stage 1: Alias Resolution Tests
    // =========================================================================

    #[tokio::test]
    #[ignore] // Requires database
    async fn test_exact_match_allianz() -> Result<()> {
        let resolver = get_resolver().await;
        let config = ResolutionConfig::default();

        // Exact match should work (case-insensitive via alias_norm)
        let result = resolver.resolve_alias("allianz", &config).await?;
        assert_eq!(result.canonical_name, "Allianz Global Investors");
        assert_eq!(result.similarity_score, 1.0); // Exact match
        println!("✓ Exact match 'allianz' -> {}", result.canonical_name);

        Ok(())
    }

    #[tokio::test]
    #[ignore]
    async fn test_exact_match_case_insensitive() -> Result<()> {
        let resolver = get_resolver().await;
        let config = ResolutionConfig::default();

        // Should match regardless of case
        for variant in ["Allianz", "ALLIANZ", "allianz", "AlLiAnZ"] {
            let result = resolver.resolve_alias(variant, &config).await?;
            assert_eq!(result.canonical_name, "Allianz Global Investors");
            println!("✓ Matched '{}' -> {}", variant, result.canonical_name);
        }

        Ok(())
    }

    #[tokio::test]
    #[ignore]
    async fn test_semantic_match_allianzgi() -> Result<()> {
        let resolver = get_resolver().await;
        let config = ResolutionConfig::default();

        // "AllianzGI" is a stored alias, should match exactly
        let result = resolver.resolve_alias("AllianzGI", &config).await?;
        assert_eq!(result.canonical_name, "Allianz Global Investors");
        println!(
            "✓ Semantic match 'AllianzGI' -> {} (score: {:.3})",
            result.canonical_name, result.similarity_score
        );

        Ok(())
    }

    #[tokio::test]
    #[ignore]
    async fn test_semantic_match_fuzzy() -> Result<()> {
        let resolver = get_resolver().await;
        let config = ResolutionConfig::lenient(); // More permissive for fuzzy

        // Try a fuzzy variation - may be ambiguous if multiple aliases score similarly
        let result = resolver.resolve_alias("Allianz Investments", &config).await;
        match result {
            Ok(m) => {
                assert_eq!(m.canonical_name, "Allianz Global Investors");
                println!(
                    "✓ Fuzzy match 'Allianz Investments' -> {} (score: {:.3})",
                    m.canonical_name, m.similarity_score
                );
            }
            Err(ClientGroupResolveError::Ambiguous { candidates, .. }) => {
                // Ambiguous is acceptable - just verify Allianz is in the candidates
                assert!(candidates
                    .iter()
                    .any(|c| c.canonical_name == "Allianz Global Investors"));
                println!(
                    "✓ Fuzzy 'Allianz Investments' is ambiguous with {} candidates (top: {})",
                    candidates.len(),
                    candidates[0].canonical_name
                );
            }
            Err(e) => return Err(e.into()),
        }

        Ok(())
    }

    #[tokio::test]
    #[ignore]
    async fn test_ambiguous_agi() -> Result<()> {
        let resolver = get_resolver().await;
        let config = ResolutionConfig::default();

        // "AGI" matches both Allianz Global Investors and Aberdeen Global Infrastructure
        let result = resolver.resolve_alias("AGI", &config).await;

        // Should return ambiguous OR match one - depends on confidence scores
        match result {
            Ok(m) => {
                // If it resolved, it found a clear winner
                println!(
                    "✓ 'AGI' resolved to {} (score: {:.3})",
                    m.canonical_name, m.similarity_score
                );
            }
            Err(ClientGroupResolveError::Ambiguous { candidates, .. }) => {
                // Ambiguous is expected for "AGI"
                assert!(candidates.len() >= 2);
                println!("✓ 'AGI' is ambiguous with {} candidates:", candidates.len());
                for c in &candidates {
                    println!(
                        "    - {} (score: {:.3})",
                        c.canonical_name, c.similarity_score
                    );
                }
            }
            Err(e) => {
                panic!("Unexpected error: {}", e);
            }
        }

        Ok(())
    }

    #[tokio::test]
    #[ignore]
    async fn test_no_match_garbage() -> Result<()> {
        let resolver = get_resolver().await;
        let config = ResolutionConfig::default();

        let result = resolver
            .resolve_alias("xyzzy_not_a_client_12345", &config)
            .await;
        assert!(
            matches!(result, Err(ClientGroupResolveError::NoMatch(_))),
            "Expected NoMatch error for garbage input"
        );
        println!("✓ Garbage input correctly rejected");

        Ok(())
    }

    // =========================================================================
    // Stage 2: Anchor Resolution Tests
    // =========================================================================

    #[tokio::test]
    #[ignore]
    async fn test_anchor_governance_controller() -> Result<()> {
        let resolver = get_resolver().await;

        // Allianz group ID (from seed data)
        let allianz_group_id: Uuid = "11111111-1111-1111-1111-111111111111".parse()?;

        let anchor = resolver
            .resolve_anchor(allianz_group_id, AnchorRole::GovernanceController, None)
            .await?;

        // Should resolve to Allianz Global Investors Holdings GmbH
        println!(
            "✓ Allianz governance_controller -> {} (confidence: {:.2})",
            anchor.anchor_entity_id, anchor.confidence
        );
        assert_eq!(anchor.anchor_role, AnchorRole::GovernanceController);

        Ok(())
    }

    #[tokio::test]
    #[ignore]
    async fn test_anchor_ultimate_parent() -> Result<()> {
        let resolver = get_resolver().await;

        let allianz_group_id: Uuid = "11111111-1111-1111-1111-111111111111".parse()?;

        let anchor = resolver
            .resolve_anchor(allianz_group_id, AnchorRole::UltimateParent, None)
            .await?;

        // Should resolve to Allianz SE
        println!(
            "✓ Allianz ultimate_parent -> {} (confidence: {:.2})",
            anchor.anchor_entity_id, anchor.confidence
        );
        assert_eq!(anchor.anchor_role, AnchorRole::UltimateParent);

        // Verify it's the expected entity (Allianz SE = 7b6942b5-10e9-425f-b8c9-5a674a7d0701)
        let expected_allianz_se: Uuid = "7b6942b5-10e9-425f-b8c9-5a674a7d0701".parse()?;
        assert_eq!(anchor.anchor_entity_id, expected_allianz_se);

        Ok(())
    }

    #[tokio::test]
    #[ignore]
    async fn test_anchor_jurisdiction_specific() -> Result<()> {
        let resolver = get_resolver().await;

        // Aviva group ID (from seed data)
        let aviva_group_id: Uuid = "22222222-2222-2222-2222-222222222222".parse()?;

        // Without jurisdiction - should get global fallback
        let global = resolver
            .resolve_anchor(aviva_group_id, AnchorRole::GovernanceController, None)
            .await?;
        println!(
            "✓ Aviva governance_controller (global) -> {}",
            global.anchor_entity_id
        );

        // With LU jurisdiction - should get Aviva Investors Luxembourg
        let lu = resolver
            .resolve_anchor(aviva_group_id, AnchorRole::GovernanceController, Some("LU"))
            .await?;
        println!(
            "✓ Aviva governance_controller (LU) -> {}",
            lu.anchor_entity_id
        );

        // They should be different (LU-specific vs global)
        assert_ne!(
            global.anchor_entity_id, lu.anchor_entity_id,
            "Jurisdiction-specific anchor should differ from global"
        );

        Ok(())
    }

    // =========================================================================
    // Full Pipeline Tests (both stages)
    // =========================================================================

    #[tokio::test]
    #[ignore]
    async fn test_full_resolution_allianz() -> Result<()> {
        let resolver = get_resolver().await;
        let config = ResolutionConfig::default();

        // Full resolution: "Allianz" -> group -> governance_controller anchor
        let result = resolver
            .resolve_full("Allianz", AnchorRole::GovernanceController, None, &config)
            .await?;

        println!(
            "✓ Full resolution: 'Allianz' -> {} -> entity {}",
            result.stage1.canonical_name, result.stage2.anchor_entity_id
        );

        assert_eq!(result.stage1.canonical_name, "Allianz Global Investors");
        assert_eq!(result.stage2.anchor_role, AnchorRole::GovernanceController);

        Ok(())
    }

    #[tokio::test]
    #[ignore]
    async fn test_full_resolution_aviva_ubo() -> Result<()> {
        let resolver = get_resolver().await;
        let config = ResolutionConfig::default();

        // Full resolution with UBO role
        let result = resolver
            .resolve_full("Aviva", AnchorRole::UltimateParent, None, &config)
            .await?;

        println!(
            "✓ UBO resolution: 'Aviva' -> {} -> entity {}",
            result.stage1.canonical_name, result.stage2.anchor_entity_id
        );

        assert_eq!(result.stage1.canonical_name, "Aviva Investors");
        assert_eq!(result.stage2.anchor_role, AnchorRole::UltimateParent);

        Ok(())
    }

    #[tokio::test]
    #[ignore]
    async fn test_list_anchors() -> Result<()> {
        let resolver = get_resolver().await;

        let allianz_group_id: Uuid = "11111111-1111-1111-1111-111111111111".parse()?;
        let anchors = resolver.list_anchors(allianz_group_id).await?;

        println!("✓ Allianz has {} anchor mappings:", anchors.len());
        for a in &anchors {
            println!(
                "    {:?} -> {} (jurisdiction: {:?})",
                a.anchor_role,
                a.anchor_entity_id,
                a.jurisdiction.as_deref().unwrap_or("global")
            );
        }

        // Should have at least ultimate_parent and governance_controller
        assert!(anchors.len() >= 2);

        Ok(())
    }
}

// Unit tests (no DB required)
#[test]
fn test_anchor_role_roundtrip() {
    use ob_semantic_matcher::AnchorRole;
    use std::str::FromStr;

    for role in [
        AnchorRole::UltimateParent,
        AnchorRole::GovernanceController,
        AnchorRole::BookController,
        AnchorRole::OperatingController,
        AnchorRole::RegulatoryAnchor,
    ] {
        let s = role.as_str();
        let parsed = AnchorRole::from_str(s).expect("should parse");
        assert_eq!(role, parsed);
    }
}

#[test]
fn test_default_role_for_domain() {
    use ob_semantic_matcher::AnchorRole;

    assert_eq!(
        AnchorRole::default_for_domain("ubo"),
        AnchorRole::UltimateParent
    );
    assert_eq!(
        AnchorRole::default_for_domain("session"),
        AnchorRole::GovernanceController
    );
    assert_eq!(
        AnchorRole::default_for_domain("kyc"),
        AnchorRole::RegulatoryAnchor
    );
}

// =============================================================================
// End-to-End Tests: Verb Search → Client Resolution → Session Scope
// =============================================================================

#[cfg(feature = "database")]
mod e2e_tests {
    use super::tests::*;
    use anyhow::Result;
    use ob_poc::agent::learning::warmup::LearningWarmup;
    use ob_poc::database::verb_service::VerbService;
    use ob_poc::mcp::verb_search::HybridVerbSearcher;
    use ob_semantic_matcher::{ClientGroupAliasResolver, ClientGroupAnchorResolver};
    use std::sync::Arc;

    /// End-to-end test: "allianz" → session.load-cluster verb
    ///
    /// Verifies the verb search pipeline correctly routes client name input
    /// to the session.load-cluster verb.
    #[tokio::test]
    #[ignore]
    async fn test_e2e_allianz_verb_search() -> Result<()> {
        let pool = get_pool().await.clone();
        let embedder = get_embedder().await.clone();

        // Set up verb searcher with learned data
        let verb_service = Arc::new(VerbService::new(pool.clone()));
        let warmup = LearningWarmup::new(pool.clone());
        let (learned_data, _stats) = warmup.warmup().await?;

        let dyn_embedder: Arc<dyn ob_poc::agent::learning::embedder::Embedder> =
            embedder.clone() as Arc<dyn ob_poc::agent::learning::embedder::Embedder>;
        let searcher = HybridVerbSearcher::new(verb_service.clone(), Some(learned_data.clone()))
            .with_embedder(dyn_embedder);

        // Test: "allianz" should match session.load-cluster
        let results = searcher.search("allianz", None, None, 5).await?;

        assert!(!results.is_empty(), "Should find at least one verb match");
        let top = &results[0];

        println!(
            "✓ 'allianz' -> {} (score: {:.3}, source: {:?})",
            top.verb, top.score, top.source
        );

        assert_eq!(
            top.verb, "session.load-cluster",
            "Expected session.load-cluster for bare client name"
        );
        assert!(
            top.score >= 0.78,
            "Score should be above semantic threshold (got {:.3})",
            top.score
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

        // Test: "load the allianz book" should match session.load-cluster
        let results = searcher
            .search("load the allianz book", None, None, 5)
            .await?;

        assert!(!results.is_empty(), "Should find at least one verb match");
        let top = &results[0];

        println!(
            "✓ 'load the allianz book' -> {} (score: {:.3}, source: {:?})",
            top.verb, top.score, top.source
        );

        assert_eq!(
            top.verb, "session.load-cluster",
            "Expected session.load-cluster"
        );

        Ok(())
    }

    /// End-to-end test: Client group resolution for DSL execution
    ///
    /// Verifies that a client group ID resolves to an anchor entity ID
    /// that can be used by session.load-cluster.
    #[tokio::test]
    #[ignore]
    async fn test_e2e_client_to_anchor_for_session() -> Result<()> {
        let pool = get_pool().await.clone();

        // Allianz group ID (from seed data)
        let allianz_group_id: uuid::Uuid = "11111111-1111-1111-1111-111111111111".parse()?;

        // This is the same query used by SessionLoadClusterOp
        let anchor: Option<uuid::Uuid> = sqlx::query_scalar!(
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

        // Verify the anchor entity exists and has a name
        let name: Option<String> = sqlx::query_scalar!(
            r#"SELECT name as "name!" FROM "ob-poc".entities WHERE entity_id = $1"#,
            anchor_id
        )
        .fetch_optional(&pool)
        .await?;

        println!(
            "✓ Client group {} -> anchor {} ({})",
            allianz_group_id,
            anchor_id,
            name.as_deref().unwrap_or("unknown")
        );

        assert!(name.is_some(), "Anchor entity should exist with a name");

        Ok(())
    }

    /// End-to-end test: Full pipeline "allianz" → verb + client resolution
    ///
    /// Verifies the complete flow:
    /// 1. "allianz" -> verb_search -> session.load-cluster
    /// 2. :client arg lookup -> client_group UUID
    /// 3. anchor resolution -> entity UUID for session scope
    #[tokio::test]
    #[ignore]
    async fn test_e2e_full_pipeline_allianz() -> Result<()> {
        let pool = get_pool().await.clone();
        let embedder = get_embedder().await.clone();

        println!("\n=== End-to-End: 'allianz' → Session Scope ===\n");

        // Step 1: Verb Search
        println!("Step 1: Verb Search");
        let verb_service = Arc::new(VerbService::new(pool.clone()));
        let warmup = LearningWarmup::new(pool.clone());
        let (learned_data, _stats) = warmup.warmup().await?;

        let dyn_embedder: Arc<dyn ob_poc::agent::learning::embedder::Embedder> =
            embedder.clone() as Arc<dyn ob_poc::agent::learning::embedder::Embedder>;
        let searcher = HybridVerbSearcher::new(verb_service.clone(), Some(learned_data.clone()))
            .with_embedder(dyn_embedder);

        let results = searcher.search("allianz", None, None, 3).await?;
        assert!(!results.is_empty());
        let verb = &results[0].verb;
        println!("  Input: 'allianz'");
        println!("  Matched verb: {} (score: {:.3})", verb, results[0].score);
        assert_eq!(verb, "session.load-cluster");

        // Step 2: Client Group Alias Resolution
        println!("\nStep 2: Client Group Resolution");
        let resolver = get_resolver().await;
        let config = ob_semantic_matcher::ResolutionConfig::default();

        let alias_result = resolver.resolve_alias("allianz", &config).await?;
        let client_group_id = alias_result.group_id;
        println!(
            "  Alias 'allianz' -> group '{}' (id: {})",
            alias_result.canonical_name, client_group_id
        );

        // Step 3: Anchor Resolution
        println!("\nStep 3: Anchor Resolution");
        let anchor = resolver
            .resolve_anchor(
                client_group_id,
                ob_semantic_matcher::AnchorRole::GovernanceController,
                None,
            )
            .await?;
        println!(
            "  GovernanceController anchor -> {} (confidence: {:.2})",
            anchor.anchor_entity_id, anchor.confidence
        );

        // Step 4: Verify anchor entity exists (would feed into CBU query)
        println!("\nStep 4: Verify Anchor Entity");
        let anchor_name: Option<String> = sqlx::query_scalar!(
            r#"SELECT name as "name!" FROM "ob-poc".entities WHERE entity_id = $1"#,
            anchor.anchor_entity_id
        )
        .fetch_optional(&pool)
        .await?;

        println!(
            "  Anchor entity: {} ({})",
            anchor.anchor_entity_id,
            anchor_name.as_deref().unwrap_or("unknown")
        );

        // Final assertion: we have a valid pipeline from user input to entity
        assert!(anchor_name.is_some(), "Anchor entity should have a name");

        println!("\n✓ Full pipeline verified: 'allianz' → session.load-cluster → client group → anchor entity");
        println!("  This anchor would be used by session.load-cluster to load CBUs under the client's hierarchy.");

        Ok(())
    }
}
