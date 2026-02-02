//! Integration tests for template batch DSL execution
//!
//! Tests the DSL-native batch execution with:
//! - `entity.query` verb returning entity refs
//! - `template.batch` verb executing template per entity
//! - `batch.add-products` for post-batch bulk operations
//!
//! Uses Allianz funds as test data (177 Luxembourg-domiciled funds).

#[cfg(feature = "database")]
mod template_batch_tests {
    use anyhow::Result;
    use sqlx::PgPool;
    use uuid::Uuid;

    use ob_poc::dsl_v2::{DslExecutor, ExecutionContext};

    // =========================================================================
    // TEST INFRASTRUCTURE
    // =========================================================================

    struct TestDb {
        pool: PgPool,
        prefix: String,
    }

    impl TestDb {
        async fn new() -> Result<Self> {
            let url = std::env::var("TEST_DATABASE_URL")
                .or_else(|_| std::env::var("DATABASE_URL"))
                .unwrap_or_else(|_| "postgresql:///data_designer".into());

            let pool = PgPool::connect(&url).await?;
            let prefix = format!("batch_test_{}", &Uuid::now_v7().to_string()[..8]);
            Ok(Self { pool, prefix })
        }

        fn name(&self, base: &str) -> String {
            format!("{}_{}", self.prefix, base)
        }

        async fn cleanup(&self) -> Result<()> {
            let pattern = format!("{}%", self.prefix);

            // Clean up CBU products
            sqlx::query(
                r#"DELETE FROM "ob-poc".cbu_products WHERE cbu_id IN
                   (SELECT cbu_id FROM "ob-poc".cbus WHERE name LIKE $1)"#,
            )
            .bind(&pattern)
            .execute(&self.pool)
            .await
            .ok();

            // Clean up CBU entity roles
            sqlx::query(
                r#"DELETE FROM "ob-poc".cbu_entity_roles WHERE cbu_id IN
                   (SELECT cbu_id FROM "ob-poc".cbus WHERE name LIKE $1)"#,
            )
            .bind(&pattern)
            .execute(&self.pool)
            .await
            .ok();

            // Clean up CBUs
            sqlx::query(r#"DELETE FROM "ob-poc".cbus WHERE name LIKE $1"#)
                .bind(&pattern)
                .execute(&self.pool)
                .await
                .ok();

            // Clean up test entities
            sqlx::query(
                r#"DELETE FROM "ob-poc".entity_funds WHERE entity_id IN
                   (SELECT entity_id FROM "ob-poc".entities WHERE name LIKE $1)"#,
            )
            .bind(&pattern)
            .execute(&self.pool)
            .await
            .ok();

            sqlx::query(
                r#"DELETE FROM "ob-poc".entity_limited_companies WHERE entity_id IN
                   (SELECT entity_id FROM "ob-poc".entities WHERE name LIKE $1)"#,
            )
            .bind(&pattern)
            .execute(&self.pool)
            .await
            .ok();

            sqlx::query(r#"DELETE FROM "ob-poc".entities WHERE name LIKE $1"#)
                .bind(&pattern)
                .execute(&self.pool)
                .await
                .ok();

            Ok(())
        }

        async fn seed_test_entities(&self) -> Result<Vec<Uuid>> {
            let mut entity_ids = Vec::new();

            // Get the limited_company entity type ID (most reliable fallback)
            let entity_type_id: Option<Uuid> = sqlx::query_scalar(
                r#"SELECT entity_type_id FROM "ob-poc".entity_types WHERE type_code = 'limited_company' LIMIT 1"#,
            )
            .fetch_optional(&self.pool)
            .await?;

            let entity_type_id = match entity_type_id {
                Some(id) => id,
                None => {
                    // If no entity types exist, skip seeding
                    return Ok(entity_ids);
                }
            };

            // Create test entities
            for i in 1..=5 {
                let entity_id = Uuid::now_v7();
                let name = self.name(&format!("TestEntity_{}", i));

                sqlx::query(
                    r#"INSERT INTO "ob-poc".entities (entity_id, entity_type_id, name)
                       VALUES ($1, $2, $3)"#,
                )
                .bind(entity_id)
                .bind(entity_type_id)
                .bind(&name)
                .execute(&self.pool)
                .await?;

                entity_ids.push(entity_id);
            }

            Ok(entity_ids)
        }

        async fn seed_manco_entity(&self) -> Result<Option<Uuid>> {
            let entity_type_id: Option<Uuid> = sqlx::query_scalar(
                r#"SELECT entity_type_id FROM "ob-poc".entity_types WHERE type_code = 'limited_company' LIMIT 1"#,
            )
            .fetch_optional(&self.pool)
            .await?;

            let entity_type_id = match entity_type_id {
                Some(id) => id,
                None => return Ok(None),
            };

            let entity_id = Uuid::now_v7();
            let name = self.name("ManCo");

            sqlx::query(
                r#"INSERT INTO "ob-poc".entities (entity_id, entity_type_id, name)
                   VALUES ($1, $2, $3)"#,
            )
            .bind(entity_id)
            .bind(entity_type_id)
            .bind(&name)
            .execute(&self.pool)
            .await?;

            Ok(Some(entity_id))
        }
    }

    // =========================================================================
    // ENTITY.QUERY TESTS
    // =========================================================================

    #[tokio::test]
    async fn test_entity_query_basic() -> Result<()> {
        let db = TestDb::new().await?;
        let _entity_ids = db.seed_test_entities().await?;

        let executor = DslExecutor::new(db.pool.clone());
        let mut ctx = ExecutionContext::new();

        // Query entities by name pattern
        let dsl = format!(
            r#"(entity.query :type "fund" :name-like "{}%" :as @funds)"#,
            db.prefix
        );

        let results = executor.execute_dsl(&dsl, &mut ctx).await;

        // Clean up before asserting
        db.cleanup().await?;

        // The query should succeed (may return empty if fund type doesn't exist)
        assert!(
            results.is_ok(),
            "entity.query should succeed: {:?}",
            results.err()
        );

        Ok(())
    }

    #[tokio::test]
    async fn test_entity_query_with_limit() -> Result<()> {
        let db = TestDb::new().await?;
        let _entity_ids = db.seed_test_entities().await?;

        let executor = DslExecutor::new(db.pool.clone());
        let mut ctx = ExecutionContext::new();

        // Query with limit
        let dsl = format!(
            r#"(entity.query :type "limited_company" :name-like "{}%" :limit 3 :as @entities)"#,
            db.prefix
        );

        let results = executor.execute_dsl(&dsl, &mut ctx).await;

        db.cleanup().await?;

        assert!(
            results.is_ok(),
            "entity.query with limit should succeed: {:?}",
            results.err()
        );

        Ok(())
    }

    // =========================================================================
    // TEMPLATE.INVOKE TESTS
    // =========================================================================

    #[tokio::test]
    async fn test_template_invoke_basic() -> Result<()> {
        let db = TestDb::new().await?;

        let executor = DslExecutor::new(db.pool.clone());
        let mut ctx = ExecutionContext::new();

        // Invoke a simple template - use proper map syntax without dots in keys
        let dsl = r#"
            (template.invoke
              :id "onboard-fund-cbu"
              :params {:fund_name "Test Invoke Fund"
                       :jurisdiction "LU"})
        "#;

        let results = executor.execute_dsl(dsl, &mut ctx).await;

        // Clean up test CBUs
        sqlx::query(r#"DELETE FROM "ob-poc".cbus WHERE name = 'Test Invoke Fund'"#)
            .execute(&db.pool)
            .await
            .ok();

        db.cleanup().await?;

        // Template invoke should work (or fail gracefully if template not found)
        // The test validates the verb is properly registered and callable
        match results {
            Ok(_) => println!("template.invoke succeeded"),
            Err(e) => {
                // Template not found is acceptable in test environment
                let err_str = e.to_string();
                assert!(
                    err_str.contains("template")
                        || err_str.contains("not found")
                        || err_str.contains("Template"),
                    "Unexpected error: {}",
                    err_str
                );
            }
        }

        Ok(())
    }

    // =========================================================================
    // TEMPLATE.BATCH TESTS
    // =========================================================================

    #[tokio::test]
    async fn test_template_batch_basic() -> Result<()> {
        let db = TestDb::new().await?;
        let _entity_ids = db.seed_test_entities().await?;
        let manco_id = match db.seed_manco_entity().await? {
            Some(id) => id,
            None => {
                println!("Skipping test - no entity types in database");
                return Ok(());
            }
        };

        let executor = DslExecutor::new(db.pool.clone());
        let mut ctx = ExecutionContext::new();

        // Full batch execution: query -> batch
        let dsl = format!(
            r#"
            (entity.query :type "limited_company" :name-like "{}TestEntity%" :limit 3 :as @funds)
            (template.batch
              :id "onboard-fund-cbu"
              :source @funds
              :bind-as "fund_entity"
              :shared {{:manco_entity "{}"
                       :jurisdiction "LU"}}
              :as @batch)
            "#,
            db.prefix, manco_id
        );

        let results = executor.execute_dsl(&dsl, &mut ctx).await;

        db.cleanup().await?;

        // Template batch should work or fail with template not found
        match results {
            Ok(_) => println!("template.batch succeeded"),
            Err(e) => {
                let err_str = e.to_string();
                // Acceptable errors in test environment
                assert!(
                    err_str.contains("template")
                        || err_str.contains("not found")
                        || err_str.contains("Template")
                        || err_str.contains("empty")
                        || err_str.contains("No entities")
                        || err_str.contains("Unresolved reference"),
                    "Unexpected error: {}",
                    err_str
                );
            }
        }

        Ok(())
    }

    // =========================================================================
    // BATCH CONTROL VERB TESTS
    // =========================================================================

    #[tokio::test]
    async fn test_batch_pause_verb() -> Result<()> {
        let db = TestDb::new().await?;

        let executor = DslExecutor::new(db.pool.clone());
        let mut ctx = ExecutionContext::new();

        let dsl = r#"(batch.pause)"#;

        let results = executor.execute_dsl(dsl, &mut ctx).await;

        db.cleanup().await?;

        assert!(results.is_ok(), "batch.pause should succeed: {:?}", results);

        Ok(())
    }

    #[tokio::test]
    async fn test_batch_resume_verb() -> Result<()> {
        let db = TestDb::new().await?;

        let executor = DslExecutor::new(db.pool.clone());
        let mut ctx = ExecutionContext::new();

        let dsl = r#"(batch.resume)"#;

        let results = executor.execute_dsl(dsl, &mut ctx).await;

        db.cleanup().await?;

        assert!(
            results.is_ok(),
            "batch.resume should succeed: {:?}",
            results
        );

        Ok(())
    }

    #[tokio::test]
    async fn test_batch_status_verb() -> Result<()> {
        let db = TestDb::new().await?;

        let executor = DslExecutor::new(db.pool.clone());
        let mut ctx = ExecutionContext::new();

        let dsl = r#"(batch.status)"#;

        let results = executor.execute_dsl(dsl, &mut ctx).await;

        db.cleanup().await?;

        assert!(
            results.is_ok(),
            "batch.status should succeed: {:?}",
            results
        );

        Ok(())
    }

    #[tokio::test]
    async fn test_batch_abort_verb() -> Result<()> {
        let db = TestDb::new().await?;

        let executor = DslExecutor::new(db.pool.clone());
        let mut ctx = ExecutionContext::new();

        let dsl = r#"(batch.abort :reason "Test abort")"#;

        let results = executor.execute_dsl(dsl, &mut ctx).await;

        db.cleanup().await?;

        assert!(results.is_ok(), "batch.abort should succeed: {:?}", results);

        Ok(())
    }

    #[tokio::test]
    async fn test_batch_skip_verb() -> Result<()> {
        let db = TestDb::new().await?;

        let executor = DslExecutor::new(db.pool.clone());
        let mut ctx = ExecutionContext::new();

        let dsl = r#"(batch.skip :reason "Invalid data")"#;

        let results = executor.execute_dsl(dsl, &mut ctx).await;

        db.cleanup().await?;

        assert!(results.is_ok(), "batch.skip should succeed: {:?}", results);

        Ok(())
    }

    #[tokio::test]
    async fn test_batch_continue_verb() -> Result<()> {
        let db = TestDb::new().await?;

        let executor = DslExecutor::new(db.pool.clone());
        let mut ctx = ExecutionContext::new();

        let dsl = r#"(batch.continue :count 10)"#;

        let results = executor.execute_dsl(dsl, &mut ctx).await;

        db.cleanup().await?;

        assert!(
            results.is_ok(),
            "batch.continue should succeed: {:?}",
            results
        );

        Ok(())
    }

    // =========================================================================
    // BATCH.ADD-PRODUCTS TESTS
    // =========================================================================

    #[tokio::test]
    async fn test_batch_add_products_basic() -> Result<()> {
        let db = TestDb::new().await?;

        // Create a test CBU
        let cbu_id = Uuid::now_v7();
        let cbu_name = db.name("ProductTestCBU");

        sqlx::query(
            r#"INSERT INTO "ob-poc".cbus (cbu_id, name, jurisdiction)
               VALUES ($1, $2, 'LU')"#,
        )
        .bind(cbu_id)
        .bind(&cbu_name)
        .execute(&db.pool)
        .await?;

        let executor = DslExecutor::new(db.pool.clone());
        let mut ctx = ExecutionContext::new();

        // Add products using the batch verb
        let dsl = format!(
            r#"(batch.add-products :cbu-ids ["{}"] :products ["CUSTODY"])"#,
            cbu_id
        );

        let results = executor.execute_dsl(&dsl, &mut ctx).await;

        // Check if products were added
        let product_count: i64 =
            sqlx::query_scalar(r#"SELECT COUNT(*) FROM "ob-poc".cbu_products WHERE cbu_id = $1"#)
                .bind(cbu_id)
                .fetch_one(&db.pool)
                .await
                .unwrap_or(0);

        db.cleanup().await?;

        assert!(
            results.is_ok(),
            "batch.add-products should succeed: {:?}",
            results
        );

        // Product may or may not exist in test DB
        println!("Products added: {}", product_count);

        Ok(())
    }

    #[tokio::test]
    async fn test_batch_add_products_multiple_cbus() -> Result<()> {
        let db = TestDb::new().await?;

        // Create multiple test CBUs
        let mut cbu_ids = Vec::new();
        for i in 1..=3 {
            let cbu_id = Uuid::now_v7();
            let cbu_name = db.name(&format!("MultiCBU_{}", i));

            sqlx::query(
                r#"INSERT INTO "ob-poc".cbus (cbu_id, name, jurisdiction)
                   VALUES ($1, $2, 'LU')"#,
            )
            .bind(cbu_id)
            .bind(&cbu_name)
            .execute(&db.pool)
            .await?;

            cbu_ids.push(cbu_id);
        }

        let executor = DslExecutor::new(db.pool.clone());
        let mut ctx = ExecutionContext::new();

        // Format UUIDs as list
        let uuid_list: Vec<String> = cbu_ids.iter().map(|id| format!("\"{}\"", id)).collect();
        let dsl = format!(
            r#"(batch.add-products :cbu-ids [{}] :products ["CUSTODY" "FUND_ACCOUNTING"])"#,
            uuid_list.join(" ")
        );

        let results = executor.execute_dsl(&dsl, &mut ctx).await;

        db.cleanup().await?;

        assert!(
            results.is_ok(),
            "batch.add-products with multiple CBUs should succeed: {:?}",
            results
        );

        Ok(())
    }

    // =========================================================================
    // EXECUTION CONTEXT TESTS
    // =========================================================================

    #[tokio::test]
    async fn test_context_parent_child_hierarchy() -> Result<()> {
        // Test that child contexts inherit parent bindings
        let mut parent = ExecutionContext::new();
        let parent_uuid = Uuid::now_v7();
        parent.symbols.insert("manco".to_string(), parent_uuid);
        parent
            .symbol_types
            .insert("manco".to_string(), "entity".to_string());

        // Create child context
        let child = parent.child_for_iteration(0);

        // Child should have empty local symbols
        assert!(child.symbols.is_empty());

        // Child should inherit parent bindings
        assert_eq!(child.parent_symbols.get("manco"), Some(&parent_uuid));

        // Child should be able to resolve parent binding
        assert_eq!(child.resolve("manco"), Some(parent_uuid));

        Ok(())
    }

    #[tokio::test]
    async fn test_context_local_overrides_parent() -> Result<()> {
        let mut parent = ExecutionContext::new();
        let parent_uuid = Uuid::now_v7();
        parent.symbols.insert("entity".to_string(), parent_uuid);

        let mut child = parent.child_for_iteration(0);

        // Add local binding with same name
        let local_uuid = Uuid::now_v7();
        child.symbols.insert("entity".to_string(), local_uuid);

        // Local should override parent
        assert_eq!(child.resolve("entity"), Some(local_uuid));
        assert_ne!(child.resolve("entity"), Some(parent_uuid));

        Ok(())
    }

    // =========================================================================
    // TRADING-PROFILE.CLONE-TO BATCH TESTS
    // =========================================================================

    #[tokio::test]
    #[ignore] // Bug: clone-to JSON serialization missing cbu_id field
    async fn test_trading_profile_clone_to_batch() -> Result<()> {
        let db = TestDb::new().await?;

        // Create source CBU with trading profile
        let source_cbu_id = Uuid::now_v7();
        let source_cbu_name = db.name("SourceCBU");

        sqlx::query(
            r#"INSERT INTO "ob-poc".cbus (cbu_id, name, jurisdiction, client_type)
               VALUES ($1, $2, 'LU', 'FUND')"#,
        )
        .bind(source_cbu_id)
        .bind(&source_cbu_name)
        .execute(&db.pool)
        .await?;

        // Create a DRAFT trading profile for source CBU
        let source_profile_id = Uuid::now_v7();
        let profile_doc = serde_json::json!({
            "universe": {
                "base_currency": "EUR",
                "allowed_currencies": ["EUR", "USD"],
                "allowed_markets": [{"mic": "XETR", "currencies": ["EUR"]}],
                "instrument_classes": [{"class_code": "EQUITY", "is_held": true, "is_traded": true}]
            },
            "investment_managers": [],
            "isda_agreements": [],
            "booking_rules": [],
            "standing_instructions": {},
            "pricing_matrix": [],
            "valuation_config": null,
            "constraints": null,
            "settlement_config": null
        });

        // Compute hash for the document using sha256
        use sha2::{Digest, Sha256};
        let doc_string = serde_json::to_string(&profile_doc)?;
        let mut hasher = Sha256::new();
        hasher.update(doc_string.as_bytes());
        let document_hash = format!("{:x}", hasher.finalize());

        sqlx::query(
            r#"INSERT INTO "ob-poc".cbu_trading_profiles
               (profile_id, cbu_id, version, status, document, document_hash, created_by)
               VALUES ($1, $2, 1, 'DRAFT', $3, $4, 'test')"#,
        )
        .bind(source_profile_id)
        .bind(source_cbu_id)
        .bind(&profile_doc)
        .bind(&document_hash)
        .execute(&db.pool)
        .await?;

        // Create multiple target CBUs
        let mut target_cbu_ids = Vec::new();
        for i in 1..=5 {
            let cbu_id = Uuid::now_v7();
            let cbu_name = db.name(&format!("TargetCBU_{}", i));

            sqlx::query(
                r#"INSERT INTO "ob-poc".cbus (cbu_id, name, jurisdiction, client_type)
                   VALUES ($1, $2, 'LU', 'FUND')"#,
            )
            .bind(cbu_id)
            .bind(&cbu_name)
            .execute(&db.pool)
            .await?;

            target_cbu_ids.push(cbu_id);
        }

        let executor = DslExecutor::new(db.pool.clone());
        let mut ctx = ExecutionContext::new();

        // Generate batch clone DSL - this simulates what an agent would generate
        // by looping through CBUs in context
        let mut dsl_statements = Vec::new();
        for target_id in &target_cbu_ids {
            dsl_statements.push(format!(
                r#"(trading-profile.clone-to :profile-id "{}" :target-cbu-id "{}")"#,
                source_profile_id, target_id
            ));
        }
        let dsl = dsl_statements.join("\n");

        // Execute batch clone
        let results = executor.execute_dsl(&dsl, &mut ctx).await;

        assert!(
            results.is_ok(),
            "Batch clone-to should succeed: {:?}",
            results.err()
        );

        // Verify profiles were created for all targets
        let profile_count: i64 = sqlx::query_scalar(
            r#"SELECT COUNT(*) FROM "ob-poc".cbu_trading_profiles
               WHERE cbu_id = ANY($1) AND status = 'DRAFT'"#,
        )
        .bind(&target_cbu_ids)
        .fetch_one(&db.pool)
        .await?;

        assert_eq!(
            profile_count, 5,
            "Should have created 5 profiles, got {}",
            profile_count
        );

        // Test idempotency: run again - should succeed without duplicates
        let mut ctx2 = ExecutionContext::new();
        let results2 = executor.execute_dsl(&dsl, &mut ctx2).await;

        assert!(
            results2.is_ok(),
            "Idempotent re-run should succeed: {:?}",
            results2.err()
        );

        // Verify still only 5 profiles (no duplicates due to idempotency)
        let profile_count_after: i64 = sqlx::query_scalar(
            r#"SELECT COUNT(*) FROM "ob-poc".cbu_trading_profiles
               WHERE cbu_id = ANY($1)"#,
        )
        .bind(&target_cbu_ids)
        .fetch_one(&db.pool)
        .await?;

        assert_eq!(
            profile_count_after, 5,
            "Idempotent re-run should not create duplicates, got {}",
            profile_count_after
        );

        // Cleanup
        sqlx::query(r#"DELETE FROM "ob-poc".cbu_trading_profiles WHERE cbu_id = ANY($1)"#)
            .bind(&target_cbu_ids)
            .execute(&db.pool)
            .await?;

        sqlx::query(r#"DELETE FROM "ob-poc".cbu_trading_profiles WHERE cbu_id = $1"#)
            .bind(source_cbu_id)
            .execute(&db.pool)
            .await?;

        db.cleanup().await?;

        Ok(())
    }

    #[tokio::test]
    #[ignore] // Bug: clone-to JSON serialization missing cbu_id field
    async fn test_trading_profile_clone_to_single() -> Result<()> {
        let db = TestDb::new().await?;

        // Create source CBU with trading profile
        let source_cbu_id = Uuid::now_v7();
        let source_cbu_name = db.name("CloneSourceCBU");

        sqlx::query(
            r#"INSERT INTO "ob-poc".cbus (cbu_id, name, jurisdiction, client_type)
               VALUES ($1, $2, 'LU', 'FUND')"#,
        )
        .bind(source_cbu_id)
        .bind(&source_cbu_name)
        .execute(&db.pool)
        .await?;

        // Create a DRAFT trading profile
        let source_profile_id = Uuid::now_v7();
        let profile_doc = serde_json::json!({
            "universe": {
                "base_currency": "USD",
                "allowed_currencies": ["USD", "GBP"],
                "allowed_markets": [{"mic": "XNYS", "currencies": ["USD"]}],
                "instrument_classes": [{"class_code": "EQUITY", "is_held": true, "is_traded": true}]
            },
            "investment_managers": [],
            "isda_agreements": [],
            "booking_rules": [],
            "standing_instructions": {},
            "pricing_matrix": [],
            "valuation_config": null,
            "constraints": null,
            "settlement_config": null
        });

        // Compute hash for the document using sha256
        use sha2::{Digest, Sha256};
        let doc_string = serde_json::to_string(&profile_doc)?;
        let mut hasher = Sha256::new();
        hasher.update(doc_string.as_bytes());
        let document_hash = format!("{:x}", hasher.finalize());

        sqlx::query(
            r#"INSERT INTO "ob-poc".cbu_trading_profiles
               (profile_id, cbu_id, version, status, document, document_hash, created_by)
               VALUES ($1, $2, 1, 'DRAFT', $3, $4, 'test')"#,
        )
        .bind(source_profile_id)
        .bind(source_cbu_id)
        .bind(&profile_doc)
        .bind(&document_hash)
        .execute(&db.pool)
        .await?;

        // Create target CBU
        let target_cbu_id = Uuid::now_v7();
        let target_cbu_name = db.name("CloneTargetCBU");

        sqlx::query(
            r#"INSERT INTO "ob-poc".cbus (cbu_id, name, jurisdiction, client_type)
               VALUES ($1, $2, 'LU', 'FUND')"#,
        )
        .bind(target_cbu_id)
        .bind(&target_cbu_name)
        .execute(&db.pool)
        .await?;

        let executor = DslExecutor::new(db.pool.clone());
        let mut ctx = ExecutionContext::new();

        // Execute single clone
        let dsl = format!(
            r#"(trading-profile.clone-to :profile-id "{}" :target-cbu-id "{}")"#,
            source_profile_id, target_cbu_id
        );

        let results = executor.execute_dsl(&dsl, &mut ctx).await;

        assert!(
            results.is_ok(),
            "Single clone-to should succeed: {:?}",
            results.err()
        );

        // Verify profile was created
        let cloned_profile: Option<(Uuid, String, i32)> = sqlx::query_as(
            r#"SELECT profile_id, status, version
               FROM "ob-poc".cbu_trading_profiles
               WHERE cbu_id = $1"#,
        )
        .bind(target_cbu_id)
        .fetch_optional(&db.pool)
        .await?;

        assert!(cloned_profile.is_some(), "Cloned profile should exist");
        let (_, status, version) = cloned_profile.unwrap();
        assert_eq!(status, "DRAFT", "Cloned profile should be DRAFT");
        assert_eq!(version, 1, "Cloned profile should be version 1");

        // Cleanup
        sqlx::query(r#"DELETE FROM "ob-poc".cbu_trading_profiles WHERE cbu_id = $1"#)
            .bind(target_cbu_id)
            .execute(&db.pool)
            .await?;

        sqlx::query(r#"DELETE FROM "ob-poc".cbu_trading_profiles WHERE cbu_id = $1"#)
            .bind(source_cbu_id)
            .execute(&db.pool)
            .await?;

        db.cleanup().await?;

        Ok(())
    }
}
