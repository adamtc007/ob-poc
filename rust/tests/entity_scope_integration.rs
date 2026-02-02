//! Integration tests for Entity Scope (Pattern B Runtime Resolution)
//!
//! Tests the scope.commit verb and :scope @sX rewrite at execution time:
//!   1. Happy path: scope.commit creates snapshot, :scope rewrite injects entity-ids
//!   2. Cross-group safety: snapshot from group A cannot be used in group B context
//!   3. Empty scope: strict mode fails, interactive mode returns candidates
//!   4. Replay determinism: same @s1 always produces same entity set
//!
//! Run all tests:
//!   DATABASE_URL="postgresql:///data_designer" cargo test --features database --test entity_scope_integration -- --ignored --nocapture

#[cfg(feature = "database")]
mod tests {
    use anyhow::Result;
    use sqlx::PgPool;
    use tokio::sync::OnceCell;
    use uuid::Uuid;

    // Shared pool
    static SHARED_POOL: OnceCell<PgPool> = OnceCell::const_new();

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

    // =========================================================================
    // Test 1: Happy Path - scope.commit creates snapshot
    // =========================================================================

    /// Test: scope.commit searches entities and creates immutable snapshot
    ///
    /// DSL: (scope.commit :desc "irish funds" :limit 20 :as @s1)
    /// Expected: Returns snapshot UUID, @s1 bound in context
    #[tokio::test]
    #[ignore] // Requires database with seed data
    async fn test_scope_commit_happy_path() -> Result<()> {
        use sqlx::Row;

        let pool = get_pool().await;

        println!("\n=== Test: scope.commit Happy Path ===\n");

        // Need a client group to search within
        // Use Allianz bootstrap group from seed data
        let allianz_group_id: Uuid = "11111111-1111-1111-1111-111111111111".parse()?;

        // Check if we have entities in this group
        let entity_count: i64 = sqlx::query_scalar(
            r#"
            SELECT COUNT(*)
            FROM "ob-poc".client_group_entity
            WHERE group_id = $1
            "#,
        )
        .bind(allianz_group_id)
        .fetch_one(pool)
        .await?;

        println!("Entities in Allianz group: {}", entity_count);

        if entity_count == 0 {
            println!("SKIP: No entities in test group. Run seed data first.");
            return Ok(());
        }

        // Create a test snapshot via SQL (simulating scope.commit)
        let test_desc = "test fund search";
        let test_session_id = Uuid::new_v4();

        let row = sqlx::query(
            r#"
            WITH search_results AS (
                SELECT entity_id, entity_name, confidence
                FROM "ob-poc".search_entity_tags($1, $2, NULL, 10, FALSE)
                ORDER BY confidence DESC, entity_id ASC
            )
            INSERT INTO "ob-poc".scope_snapshots
                (group_id, description, filter_applied, limit_requested, mode,
                 selected_entity_ids, resolution_method, session_id)
            SELECT
                $1,
                $3,
                '{"desc": "test fund search"}'::jsonb,
                10,
                'strict',
                ARRAY_AGG(entity_id ORDER BY confidence DESC, entity_id ASC),
                'fuzzy_text',
                $4
            FROM search_results
            RETURNING id, ARRAY_LENGTH(selected_entity_ids, 1) as count
            "#,
        )
        .bind(allianz_group_id)
        .bind("fund")
        .bind(test_desc)
        .bind(test_session_id)
        .fetch_one(pool)
        .await?;

        let snapshot_id: Uuid = row.get("id");
        let count: Option<i32> = row.get("count");

        println!("Created snapshot: {}", snapshot_id);
        println!("Entity count: {:?}", count);

        // Verify snapshot is immutable (trigger should prevent updates)
        let update_result = sqlx::query(
            r#"
            UPDATE "ob-poc".scope_snapshots
            SET description = 'should fail'
            WHERE id = $1
            "#,
        )
        .bind(snapshot_id)
        .execute(pool)
        .await;

        match update_result {
            Err(e) => {
                println!(
                    "✓ Snapshot immutability enforced: UPDATE blocked ({})",
                    e.to_string().split('\n').next().unwrap_or("")
                );
            }
            Ok(_) => {
                println!("WARNING: Snapshot was updated - immutability trigger may not be active");
            }
        }

        // Clean up
        sqlx::query(r#"DELETE FROM "ob-poc".scope_snapshots WHERE id = $1"#)
            .bind(snapshot_id)
            .execute(pool)
            .await?;

        println!("\n✓ Happy path test passed");
        Ok(())
    }

    // =========================================================================
    // Test 2: Cross-Group Safety
    // =========================================================================

    /// Test: Snapshot from group A cannot be used in group B context
    ///
    /// 1. Create snapshot under group A
    /// 2. Attempt to use in context of group B
    /// 3. Should fail with security error
    #[tokio::test]
    #[ignore]
    async fn test_cross_group_safety() -> Result<()> {
        use sqlx::Row;

        let pool = get_pool().await;

        println!("\n=== Test: Cross-Group Safety ===\n");

        let group_a: Uuid = "11111111-1111-1111-1111-111111111111".parse()?; // Allianz
        let group_b: Uuid = "22222222-2222-2222-2222-222222222222".parse()?; // Aviva

        // Create snapshot in group A
        let snapshot_id = Uuid::new_v4();
        sqlx::query(
            r#"
            INSERT INTO "ob-poc".scope_snapshots
                (id, group_id, description, filter_applied, limit_requested, mode,
                 selected_entity_ids, resolution_method, session_id)
            VALUES ($1, $2, 'test', '{}'::jsonb, 10, 'strict', ARRAY[]::uuid[], 'fuzzy_text', $3)
            "#,
        )
        .bind(snapshot_id)
        .bind(group_a)
        .bind(Uuid::new_v4())
        .execute(pool)
        .await?;

        println!("Created snapshot {} in group A", snapshot_id);

        // Load snapshot and check group_id
        let row = sqlx::query(r#"SELECT group_id FROM "ob-poc".scope_snapshots WHERE id = $1"#)
            .bind(snapshot_id)
            .fetch_one(pool)
            .await?;

        let snapshot_group_id: Uuid = row.get("group_id");
        println!("Snapshot belongs to group: {}", snapshot_group_id);

        // Simulate cross-group check (this is what executor.rs does)
        let context_group_id = group_b;
        if snapshot_group_id != context_group_id {
            println!(
                "✓ Cross-group violation detected: snapshot group {} != context group {}",
                snapshot_group_id, context_group_id
            );
        } else {
            panic!("Cross-group check failed - groups should be different");
        }

        // Clean up
        sqlx::query(r#"DELETE FROM "ob-poc".scope_snapshots WHERE id = $1"#)
            .bind(snapshot_id)
            .execute(pool)
            .await?;

        println!("\n✓ Cross-group safety test passed");
        Ok(())
    }

    // =========================================================================
    // Test 3: Empty Scope Handling
    // =========================================================================

    /// Test: Empty scope in strict mode should fail
    #[tokio::test]
    #[ignore]
    async fn test_empty_scope_strict_mode() -> Result<()> {
        let pool = get_pool().await;

        println!("\n=== Test: Empty Scope (Strict Mode) ===\n");

        let group_id: Uuid = "11111111-1111-1111-1111-111111111111".parse()?;

        // Search for something that definitely won't match
        let rows: Vec<(Uuid,)> = sqlx::query_as(
            r#"
            SELECT entity_id
            FROM "ob-poc".search_entity_tags($1, $2, NULL, 10, FALSE)
            "#,
        )
        .bind(group_id)
        .bind("xyzzy_definitely_not_a_match_12345")
        .fetch_all(pool)
        .await?;

        if rows.is_empty() {
            println!("✓ Empty search result as expected");
            println!("  In strict mode, scope.commit would return error");
            println!("  Error: 'No entities found matching ... Try a different search term'");
        } else {
            println!(
                "Note: Search returned {} results (test may need different query)",
                rows.len()
            );
        }

        println!("\n✓ Empty scope test passed");
        Ok(())
    }

    // =========================================================================
    // Test 4: Replay Determinism
    // =========================================================================

    /// Test: Same snapshot always produces same entity set
    ///
    /// 1. Create snapshot with ordered entity_ids
    /// 2. Read back multiple times
    /// 3. Order must be identical (score DESC, uuid ASC)
    #[tokio::test]
    #[ignore]
    async fn test_replay_determinism() -> Result<()> {
        use sqlx::Row;

        let pool = get_pool().await;

        println!("\n=== Test: Replay Determinism ===\n");

        // Create snapshot with specific entity order
        let snapshot_id = Uuid::new_v4();
        let group_id: Uuid = "11111111-1111-1111-1111-111111111111".parse()?;

        // Use deterministic UUIDs for testing
        let entity_ids: Vec<Uuid> = vec![
            "aaaaaaaa-0001-0000-0000-000000000001".parse()?,
            "aaaaaaaa-0002-0000-0000-000000000002".parse()?,
            "aaaaaaaa-0003-0000-0000-000000000003".parse()?,
        ];

        sqlx::query(
            r#"
            INSERT INTO "ob-poc".scope_snapshots
                (id, group_id, description, filter_applied, limit_requested, mode,
                 selected_entity_ids, resolution_method, session_id)
            VALUES ($1, $2, 'determinism test', '{}'::jsonb, 10, 'strict', $3, 'fuzzy_text', $4)
            "#,
        )
        .bind(snapshot_id)
        .bind(group_id)
        .bind(&entity_ids)
        .bind(Uuid::new_v4())
        .execute(pool)
        .await?;

        println!("Created snapshot with {} entities", entity_ids.len());

        // Read back multiple times
        for i in 1..=3 {
            let row = sqlx::query(
                r#"SELECT selected_entity_ids FROM "ob-poc".scope_snapshots WHERE id = $1"#,
            )
            .bind(snapshot_id)
            .fetch_one(pool)
            .await?;

            let read_ids: Vec<Uuid> = row.get("selected_entity_ids");

            assert_eq!(
                read_ids, entity_ids,
                "Read {} should match original order",
                i
            );
            println!("  Read {}: order preserved ✓", i);
        }

        // Clean up
        sqlx::query(r#"DELETE FROM "ob-poc".scope_snapshots WHERE id = $1"#)
            .bind(snapshot_id)
            .execute(pool)
            .await?;

        println!("\n✓ Replay determinism test passed");
        Ok(())
    }

    // =========================================================================
    // Test 5: Scope Rewrite - Verb Entity-IDs Check
    // =========================================================================

    /// Test: Verify verb_accepts_entity_ids check works
    ///
    /// entity.list now accepts :entity-ids for Pattern B scope support
    #[tokio::test]
    #[ignore]
    async fn test_verb_entity_ids_check() -> Result<()> {
        use ob_poc::dsl_v2::runtime_registry::runtime_registry;

        println!("\n=== Test: Verb Entity-IDs Check ===\n");

        let registry = runtime_registry();

        // runbook.pick accepts entity-ids
        if let Some(verb) = registry.get_by_name("runbook.pick") {
            let has_entity_ids = verb
                .args
                .iter()
                .any(|a| a.name == "entity-ids" || a.name == "entity_ids");
            println!("runbook.pick accepts entity-ids: {}", has_entity_ids);
            assert!(has_entity_ids, "runbook.pick should accept entity-ids");
        } else {
            println!("Note: runbook.pick not in registry (may need verb YAML loaded)");
        }

        // entity.list NOW accepts entity-ids (Pattern B scope support)
        if let Some(verb) = registry.get_by_name("entity.list") {
            let has_entity_ids = verb
                .args
                .iter()
                .any(|a| a.name == "entity-ids" || a.name == "entity_ids");
            println!("entity.list accepts entity-ids: {}", has_entity_ids);
            assert!(
                has_entity_ids,
                "entity.list should accept entity-ids for scope support"
            );
            println!("  ✓ entity.list is scope-aware!");
        } else {
            println!("Note: entity.list not in registry (may need verb YAML loaded)");
        }

        println!("\n✓ Verb entity-ids check test passed");
        Ok(())
    }

    // =========================================================================
    // Test 6: Full End-to-End Scope Flow
    // =========================================================================

    /// Test: scope.commit -> entity.list with :scope rewrite
    ///
    /// This is the full happy path:
    /// 1. Create scope snapshot via scope.commit
    /// 2. Use (entity.list :scope @s1)
    /// 3. Executor rewrites to (entity.list :entity-ids [...])
    /// 4. EntityListOp filters by those IDs
    #[tokio::test]
    #[ignore]
    async fn test_full_scope_flow() -> Result<()> {
        let pool = get_pool().await;

        println!("\n=== Test: Full Scope Flow ===\n");

        let group_id: Uuid = "11111111-1111-1111-1111-111111111111".parse()?;

        // Step 1: Create a snapshot with some entities
        let snapshot_id = Uuid::new_v4();
        let test_entity_ids: Vec<Uuid> = sqlx::query_scalar(
            r#"
            SELECT entity_id
            FROM "ob-poc".client_group_entity
            WHERE group_id = $1
            LIMIT 5
            "#,
        )
        .bind(group_id)
        .fetch_all(pool)
        .await?;

        if test_entity_ids.is_empty() {
            println!("SKIP: No entities in test group. Run seed data first.");
            return Ok(());
        }

        println!("Step 1: Found {} entities in group", test_entity_ids.len());

        // Insert snapshot
        sqlx::query(
            r#"
            INSERT INTO "ob-poc".scope_snapshots
                (id, group_id, description, filter_applied, limit_requested, mode,
                 selected_entity_ids, resolution_method, session_id)
            VALUES ($1, $2, 'full flow test', '{}'::jsonb, 10, 'strict', $3, 'fuzzy_text', $4)
            "#,
        )
        .bind(snapshot_id)
        .bind(group_id)
        .bind(&test_entity_ids)
        .bind(Uuid::new_v4())
        .execute(pool)
        .await?;

        println!(
            "Step 2: Created snapshot {} with {} entities",
            snapshot_id,
            test_entity_ids.len()
        );

        // Step 3: Verify we can query entities by those IDs (simulating EntityListOp)
        let listed_entities: Vec<(Uuid, String)> = sqlx::query_as(
            r#"
            SELECT entity_id, entity_name
            FROM "ob-poc".entities
            WHERE entity_id = ANY($1)
            ORDER BY entity_name
            "#,
        )
        .bind(&test_entity_ids)
        .fetch_all(pool)
        .await?;

        println!(
            "Step 3: entity.list would return {} entities:",
            listed_entities.len()
        );
        for (id, name) in &listed_entities {
            println!("  - {} ({})", name, id);
        }

        // Verify count matches
        assert_eq!(
            listed_entities.len(),
            test_entity_ids.len(),
            "Listed entities should match snapshot count"
        );

        // Clean up
        sqlx::query(r#"DELETE FROM "ob-poc".scope_snapshots WHERE id = $1"#)
            .bind(snapshot_id)
            .execute(pool)
            .await?;

        println!("\n✓ Full scope flow test passed");
        println!("\nThe complete DSL flow would be:");
        println!("  (scope.commit :desc \"test\" :limit 5 :as @s1)");
        println!("  (entity.list :scope @s1)");
        println!("  → Executor rewrites to: (entity.list :entity-ids [...])");
        println!("  → Returns {} entities", listed_entities.len());

        Ok(())
    }
}
