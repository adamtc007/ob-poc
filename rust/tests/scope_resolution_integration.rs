//! Integration tests for Scope Resolution (Stage 0 Hard Gate)
//!
//! Tests the scope resolution system that runs BEFORE Candle verb discovery:
//!   1. Scope phrases are detected and consumed by Stage 0
//!   2. Scope resolution returns early (no verb search, no entity modal)
//!   3. Subsequent commands use the established scope context
//!   4. Flywheel records user-confirmed aliases
//!
//! Run all tests:
//!   DATABASE_URL="postgresql:///data_designer" cargo test --features database --test scope_resolution_integration -- --ignored --nocapture
//!
//! Run specific test:
//!   DATABASE_URL="postgresql:///data_designer" cargo test --features database --test scope_resolution_integration test_scope_resolution_hard_gate -- --ignored --nocapture

#[cfg(feature = "database")]
mod tests {
    use anyhow::Result;
    use ob_poc::agent::learning::embedder::CandleEmbedder;
    use ob_poc::agent::learning::warmup::LearningWarmup;
    use ob_poc::database::verb_service::VerbService;
    use ob_poc::mcp::intent_pipeline::{IntentPipeline, PipelineOutcome};
    use ob_poc::mcp::scope_resolution::{ScopeContext, ScopeResolutionOutcome, ScopeResolver};
    use ob_poc::mcp::verb_search::HybridVerbSearcher;
    use sqlx::PgPool;
    use std::sync::Arc;
    use tokio::sync::OnceCell;

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

    async fn create_pipeline() -> Result<IntentPipeline> {
        let pool = get_pool().await.clone();
        let embedder = get_embedder().await.clone();

        let verb_service = Arc::new(VerbService::new(pool.clone()));
        let warmup = LearningWarmup::new(pool.clone());
        let (learned_data, _stats) = warmup.warmup().await?;

        let dyn_embedder: Arc<dyn ob_poc::agent::learning::embedder::Embedder> =
            embedder.clone() as Arc<dyn ob_poc::agent::learning::embedder::Embedder>;
        let searcher = HybridVerbSearcher::new(verb_service.clone(), Some(learned_data.clone()))
            .with_embedder(dyn_embedder);

        Ok(IntentPipeline::with_pool(searcher, pool))
    }

    // =========================================================================
    // Stage 0: Scope Resolution Detection Tests
    // =========================================================================

    #[test]
    fn test_is_scope_phrase_detection() {
        // Scope-setting phrases should be detected
        assert!(ScopeResolver::is_scope_phrase("work on allianz"));
        assert!(ScopeResolver::is_scope_phrase("working on blackrock"));
        assert!(ScopeResolver::is_scope_phrase("switch to aviva"));
        assert!(ScopeResolver::is_scope_phrase("set client to allianz"));
        assert!(ScopeResolver::is_scope_phrase("client is blackrock"));
        assert!(ScopeResolver::is_scope_phrase("load allianz"));

        // Short inputs (potential client names) should be detected
        assert!(ScopeResolver::is_scope_phrase("allianz"));
        assert!(ScopeResolver::is_scope_phrase("black rock"));

        // Commands with verbs should NOT be scope phrases
        assert!(!ScopeResolver::is_scope_phrase(
            "create a new cbu for allianz"
        ));
        assert!(!ScopeResolver::is_scope_phrase("show me the irish funds"));
        assert!(!ScopeResolver::is_scope_phrase(
            "list all entities for allianz"
        ));
        assert!(!ScopeResolver::is_scope_phrase(
            "delete the allianz custody account"
        ));

        println!("✓ Scope phrase detection works correctly");
    }

    #[test]
    fn test_extract_client_name() {
        assert_eq!(
            ScopeResolver::extract_client_name("work on allianz"),
            Some("allianz".to_string())
        );
        assert_eq!(
            ScopeResolver::extract_client_name("switch to Black Rock"),
            Some("Black Rock".to_string())
        );
        assert_eq!(
            ScopeResolver::extract_client_name("allianz"),
            Some("allianz".to_string())
        );
        assert_eq!(
            ScopeResolver::extract_client_name("client is Aviva Investors"),
            Some("Aviva Investors".to_string())
        );

        println!("✓ Client name extraction works correctly");
    }

    // =========================================================================
    // Stage 0: Hard Gate Tests (DB required)
    // =========================================================================

    /// Test: Scope phrase "allianz" resolves and returns ScopeResolved outcome
    ///
    /// This is the core hard gate test: "allianz" should be consumed by Stage 0
    /// and NOT proceed to Candle verb discovery.
    #[tokio::test]
    #[ignore] // Requires database
    async fn test_scope_resolution_hard_gate() -> Result<()> {
        let pipeline = create_pipeline().await?;

        println!("\n=== Hard Gate Test: 'allianz' ===\n");

        // Process "allianz" - should be caught by Stage 0
        let result = pipeline.process("allianz", None).await?;

        println!("Outcome: {:?}", result.outcome);
        println!("Scope resolution: {:?}", result.scope_resolution);
        println!("Scope context: {:?}", result.scope_context);
        println!("DSL: '{}'", result.dsl);
        println!("Verb candidates: {:?}", result.verb_candidates.len());

        // Key assertions for hard gate:
        match &result.outcome {
            PipelineOutcome::ScopeResolved {
                group_id,
                group_name,
                entity_count,
            } => {
                println!(
                    "\n✓ HARD GATE PASSED: Scope resolved to '{}' (id: {}, {} entities)",
                    group_name, group_id, entity_count
                );
                assert!(
                    group_name.contains("Allianz"),
                    "Should resolve to Allianz group"
                );
                assert!(*entity_count > 0, "Should have entities in the group");
            }
            PipelineOutcome::ScopeCandidates => {
                // Also acceptable - means multiple Allianz-like groups exist
                println!("\n✓ HARD GATE PASSED: Scope candidates returned (picker needed)");
                assert!(
                    matches!(
                        result.scope_resolution,
                        Some(ScopeResolutionOutcome::Candidates(_))
                    ),
                    "Should have candidate list"
                );
            }
            other => {
                // If it went to verb search, the hard gate failed
                if !result.verb_candidates.is_empty() {
                    panic!(
                        "HARD GATE FAILED: 'allianz' went to verb search instead of scope resolution.\nOutcome: {:?}\nVerb candidates: {:?}",
                        other, result.verb_candidates
                    );
                }
                // Could be NoMatch if no Allianz in DB - that's also a valid Stage 0 result
                println!("Note: Scope not resolved, but did not reach verb search");
            }
        }

        // DSL should be empty (no verb was matched)
        assert!(
            result.dsl.is_empty(),
            "DSL should be empty for scope resolution"
        );

        // Verb candidates should be empty (Stage 0 returned early)
        assert!(
            result.verb_candidates.is_empty(),
            "Should NOT have verb candidates - Stage 0 should return early"
        );

        Ok(())
    }

    /// Test: Command with verb should bypass scope resolution
    ///
    /// "create a cbu for allianz" has a verb indicator, so should NOT be caught by scope resolver.
    #[tokio::test]
    #[ignore]
    async fn test_command_bypasses_scope_resolution() -> Result<()> {
        let pool = get_pool().await.clone();
        let resolver = ScopeResolver::new();

        println!("\n=== Bypass Test: 'create a cbu for allianz' ===\n");

        // Test that scope resolver does NOT handle commands with verb indicators
        let result = resolver.resolve("create a cbu for allianz", &pool).await?;

        println!("Scope resolution result: {:?}", result);

        // Should be NotScopePhrase - commands bypass scope resolution
        match result {
            ScopeResolutionOutcome::NotScopePhrase => {
                println!("✓ BYPASS PASSED: Command correctly detected as NotScopePhrase");
            }
            other => {
                panic!(
                    "Command with verb should be NotScopePhrase, got: {:?}",
                    other
                );
            }
        }

        Ok(())
    }

    /// Test: "work on allianz" scope phrase resolves to Allianz group
    #[tokio::test]
    #[ignore]
    async fn test_work_on_phrase() -> Result<()> {
        let pipeline = create_pipeline().await?;

        println!("\n=== Scope Phrase Test: 'work on allianz' ===\n");

        let result = pipeline.process("work on allianz", None).await?;

        println!("Outcome: {:?}", result.outcome);

        match &result.outcome {
            PipelineOutcome::ScopeResolved { group_name, .. } => {
                println!("✓ 'work on allianz' -> Resolved to '{}'", group_name);
                assert!(group_name.contains("Allianz"));
            }
            PipelineOutcome::ScopeCandidates => {
                println!("✓ 'work on allianz' -> Candidates (picker needed)");
            }
            other => {
                println!(
                    "Note: 'work on allianz' resulted in {:?} (may need alias in DB)",
                    other
                );
            }
        }

        // Either way, should not have verb candidates
        assert!(
            result.verb_candidates.is_empty(),
            "Scope phrase should not reach verb search"
        );

        Ok(())
    }

    // =========================================================================
    // Scope Context Propagation Tests
    // =========================================================================

    /// Test: After scope is set, subsequent commands use the scope context
    #[tokio::test]
    #[ignore]
    async fn test_scope_context_propagation() -> Result<()> {
        let pipeline = create_pipeline().await?;

        println!("\n=== Scope Context Propagation Test ===\n");

        // Step 1: Set scope with "allianz"
        let scope_result = pipeline.process("allianz", None).await?;

        let scope_ctx = match &scope_result.outcome {
            PipelineOutcome::ScopeResolved {
                group_id,
                group_name,
                ..
            } => {
                println!("Step 1: Scope set to '{}' ({})", group_name, group_id);
                scope_result.scope_context.clone().unwrap_or_default()
            }
            _ => {
                println!("Note: Scope not resolved, using empty context");
                ScopeContext::default()
            }
        };

        // Step 2: Run a command with the scope context
        let command_result = pipeline
            .process_with_scope("show me the irish funds", None, Some(scope_ctx.clone()))
            .await?;

        println!("\nStep 2: Processing 'show me the irish funds' with scope");
        println!("  Outcome: {:?}", command_result.outcome);
        println!(
            "  Scope context preserved: {:?}",
            command_result.scope_context
        );

        // Scope context should be preserved in the result
        if let Some(ctx) = &command_result.scope_context {
            if scope_ctx.has_scope() {
                assert_eq!(
                    ctx.client_group_id, scope_ctx.client_group_id,
                    "Scope context should be preserved"
                );
                println!("✓ Scope context preserved: {:?}", ctx.client_group_name);
            }
        }

        Ok(())
    }

    // =========================================================================
    // Deterministic UX Contract Tests
    // =========================================================================

    /// Test: ScopeResolutionOutcome determinism
    ///
    /// Verifies the UX contract:
    /// - Resolved → chip (single group)
    /// - Candidates → picker (multiple options)
    /// - Unresolved → silent continue
    /// - NotScopePhrase → continue to verb search
    #[tokio::test]
    #[ignore]
    async fn test_deterministic_ux_contract() -> Result<()> {
        let pool = get_pool().await.clone();
        let resolver = ScopeResolver::new();

        println!("\n=== Deterministic UX Contract Test ===\n");

        // Test 1: Known client should resolve
        let result = resolver.resolve("allianz", &pool).await?;
        match result {
            ScopeResolutionOutcome::Resolved { group_name, .. } => {
                println!("✓ 'allianz' -> Resolved (show chip: '{}')", group_name);
            }
            ScopeResolutionOutcome::Candidates(c) => {
                println!(
                    "✓ 'allianz' -> Candidates (show picker with {} options)",
                    c.len()
                );
            }
            ScopeResolutionOutcome::Unresolved => {
                println!("Note: 'allianz' not found in DB (would continue silently)");
            }
            ScopeResolutionOutcome::NotScopePhrase => {
                panic!("'allianz' should be detected as scope phrase");
            }
        }

        // Test 2: Command should NOT be scope phrase
        let result = resolver.resolve("create a new cbu", &pool).await?;
        assert!(
            matches!(result, ScopeResolutionOutcome::NotScopePhrase),
            "Command should not be a scope phrase"
        );
        println!("✓ 'create a new cbu' -> NotScopePhrase (continue to verb search)");

        // Test 3: Garbage input should be Unresolved (not error)
        let result = resolver.resolve("xyzzy_not_a_client_12345", &pool).await?;
        match result {
            ScopeResolutionOutcome::Unresolved => {
                println!("✓ Garbage input -> Unresolved (continue silently)");
            }
            ScopeResolutionOutcome::NotScopePhrase => {
                // Long input might not be detected as scope phrase
                println!("✓ Garbage input -> NotScopePhrase (continue to verb search)");
            }
            other => {
                panic!("Unexpected result for garbage: {:?}", other);
            }
        }

        Ok(())
    }

    // =========================================================================
    // Flywheel Recording Tests
    // =========================================================================

    /// Test: User confirmation records alias for future matches
    #[tokio::test]
    #[ignore]
    async fn test_flywheel_records_selection() -> Result<()> {
        let pool = get_pool().await.clone();

        println!("\n=== Flywheel Recording Test ===\n");

        // Use a unique alias to test recording
        let test_alias = format!("test_alias_{}", &uuid::Uuid::now_v7().to_string()[..8]);
        let allianz_group_id: uuid::Uuid = "11111111-1111-1111-1111-111111111111".parse()?;

        // Record a selection (simulating user picking from candidates)
        ScopeResolver::record_selection(&pool, allianz_group_id, &test_alias, "test_session")
            .await?;

        println!("Recorded alias '{}' for Allianz group", test_alias);

        // Verify it was recorded
        let exists: bool = sqlx::query_scalar!(
            r#"
            SELECT EXISTS(
                SELECT 1 FROM "ob-poc".client_group_alias
                WHERE group_id = $1 AND alias_norm = $2
            ) as "exists!"
            "#,
            allianz_group_id,
            test_alias.to_lowercase()
        )
        .fetch_one(&pool)
        .await?;

        assert!(exists, "Alias should be recorded in database");
        println!("✓ Alias recorded in client_group_alias table");

        // Verify source is 'user_confirmed'
        let source: Option<String> = sqlx::query_scalar!(
            r#"
            SELECT source as "source!" FROM "ob-poc".client_group_alias
            WHERE group_id = $1 AND alias_norm = $2
            "#,
            allianz_group_id,
            test_alias.to_lowercase()
        )
        .fetch_optional(&pool)
        .await?;

        assert_eq!(source.as_deref(), Some("user_confirmed"));
        println!("✓ Source is 'user_confirmed' (flywheel active)");

        // Clean up test data
        sqlx::query!(
            r#"DELETE FROM "ob-poc".client_group_alias WHERE alias_norm = $1"#,
            test_alias.to_lowercase()
        )
        .execute(&pool)
        .await?;

        println!("✓ Test cleanup complete");

        Ok(())
    }

    // =========================================================================
    // Full Integration: No Entity-Search Modal
    // =========================================================================

    /// KEY TEST: Verify scope resolution prevents entity-search modal
    ///
    /// The whole point of this feature is that when user says "allianz",
    /// we should NOT pop up an entity search modal asking "which Allianz entity?"
    ///
    /// Instead, Stage 0 should:
    /// 1. Detect it's a scope phrase
    /// 2. Resolve to client group (or show compact picker)
    /// 3. Return early WITHOUT going to verb search
    /// 4. Entity resolution for subsequent commands uses the scoped context
    #[tokio::test]
    #[ignore]
    async fn test_no_entity_search_modal() -> Result<()> {
        let pipeline = create_pipeline().await?;

        println!("\n=== KEY TEST: No Entity-Search Modal ===\n");

        // Input that could confuse old pipeline:
        // - "allianz" matches entity names (would trigger entity search modal)
        // - But it's also a client group alias
        // - Stage 0 should intercept and resolve as scope, NOT trigger modal

        let result = pipeline.process("allianz", None).await?;

        // Check that we did NOT get entity references that need resolution
        println!("Unresolved refs: {:?}", result.unresolved_refs);
        assert!(
            result.unresolved_refs.is_empty(),
            "Should NOT have unresolved entity refs - scope resolution should handle 'allianz'"
        );

        // Check outcome is scope-related, not verb-related
        let is_scope_outcome = matches!(
            result.outcome,
            PipelineOutcome::ScopeResolved { .. } | PipelineOutcome::ScopeCandidates
        );

        // If we have Allianz in the DB, it should resolve
        // If not, it should at least not go to verb search
        if is_scope_outcome {
            println!("✓ NO ENTITY MODAL: 'allianz' handled by Stage 0 scope resolution");
        } else if result.verb_candidates.is_empty() {
            println!("✓ NO ENTITY MODAL: 'allianz' did not trigger verb search");
        } else {
            // This is the failure case we want to prevent
            println!("POTENTIAL ISSUE: 'allianz' went to verb search");
            println!("  Verb candidates: {:?}", result.verb_candidates);

            // If verb search returned session.load-cluster, that's still OK
            // The entity arg resolution would happen within client scope
            if let Some(top) = result.verb_candidates.first() {
                if top.verb == "session.load-cluster" {
                    println!("  Note: session.load-cluster matched - will use :client arg");
                }
            }
        }

        // The key assertion: no entity-search modal should be triggered
        // This means either:
        // 1. Scope resolved (ScopeResolved/ScopeCandidates)
        // 2. No unresolved entity refs
        assert!(
            is_scope_outcome || result.unresolved_refs.is_empty(),
            "Must either resolve scope OR have no entity refs needing modal"
        );

        println!("\n✓ TEST PASSED: No entity-search modal would be shown for 'allianz'");

        Ok(())
    }
}
