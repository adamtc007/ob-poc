//! Transaction rollback integration tests
//!
//! These tests verify that `execute_plan_atomic` correctly rolls back
//! all changes when any verb in the plan fails.
//!
//! This is critical for:
//! - Batch operations that must succeed or fail together
//! - Template expansion where partial execution is dangerous
//! - Any DSL program that creates interdependent entities

#[cfg(feature = "database")]
mod rollback_tests {
    use anyhow::Result;
    use sqlx::PgPool;
    use uuid::Uuid;

    use ob_poc::dsl_v2::{compile, parse_program, DslExecutor, ExecutionContext};

    // Uuid is still needed for TestDb prefix generation

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
            let prefix = format!("txtest_{}", &Uuid::now_v7().to_string()[..8]);
            Ok(Self { pool, prefix })
        }

        fn name(&self, base: &str) -> String {
            format!("{}_{}", self.prefix, base)
        }

        /// Check if an entity with the given name exists
        async fn entity_exists(&self, name: &str) -> Result<bool> {
            let row: (i64,) =
                sqlx::query_as(r#"SELECT COUNT(*) FROM "ob-poc".entities WHERE name = $1"#)
                    .bind(name)
                    .fetch_one(&self.pool)
                    .await?;
            Ok(row.0 > 0)
        }

        /// Check if a CBU with the given name exists
        async fn cbu_exists(&self, name: &str) -> Result<bool> {
            let row: (i64,) =
                sqlx::query_as(r#"SELECT COUNT(*) FROM "ob-poc".cbus WHERE name = $1"#)
                    .bind(name)
                    .fetch_one(&self.pool)
                    .await?;
            Ok(row.0 > 0)
        }

        async fn cleanup(&self) -> Result<()> {
            let pattern = format!("{}%", self.prefix);

            // Delete cbu_entity_roles first
            sqlx::query(
                r#"DELETE FROM "ob-poc".cbu_entity_roles WHERE cbu_id IN
                   (SELECT cbu_id FROM "ob-poc".cbus WHERE name LIKE $1)"#,
            )
            .bind(&pattern)
            .execute(&self.pool)
            .await
            .ok();

            // Delete CBUs
            sqlx::query(r#"DELETE FROM "ob-poc".cbus WHERE name LIKE $1"#)
                .bind(&pattern)
                .execute(&self.pool)
                .await
                .ok();

            // Delete entities (proper_persons extension table)
            sqlx::query(
                r#"DELETE FROM "ob-poc".entity_proper_persons WHERE entity_id IN
                   (SELECT entity_id FROM "ob-poc".entities WHERE name LIKE $1)"#,
            )
            .bind(&pattern)
            .execute(&self.pool)
            .await
            .ok();

            // Delete entities (limited_companies extension table)
            sqlx::query(
                r#"DELETE FROM "ob-poc".entity_limited_companies WHERE entity_id IN
                   (SELECT entity_id FROM "ob-poc".entities WHERE name LIKE $1)"#,
            )
            .bind(&pattern)
            .execute(&self.pool)
            .await
            .ok();

            // Delete base entities
            sqlx::query(r#"DELETE FROM "ob-poc".entities WHERE name LIKE $1"#)
                .bind(&pattern)
                .execute(&self.pool)
                .await
                .ok();

            Ok(())
        }
    }

    // =========================================================================
    // ROLLBACK TESTS
    // =========================================================================

    /// Test that a failing verb rolls back all preceding successful verbs
    ///
    /// Scenario:
    /// 1. Create a CBU (should succeed)
    /// 2. Create a person entity (should succeed)
    /// 3. Assign a role with an INVALID role code (should fail)
    ///
    /// Expected: Neither the CBU nor the entity should exist after rollback
    #[tokio::test]
    #[ignore] // Requires database
    async fn test_rollback_on_invalid_role() -> Result<()> {
        let db = TestDb::new().await?;
        let executor = DslExecutor::new(db.pool.clone());

        let cbu_name = db.name("fund");
        let entity_name = db.name("person");

        // DSL with an invalid role that will fail
        let dsl = format!(
            r#"
            (cbu.ensure :name "{}" :jurisdiction "LU" :as @fund)
            (entity.create-proper-person :first-name "{}" :last-name "Smith" :as @person)
            (cbu.assign-role :cbu-id @fund :entity-id @person :role "INVALID_ROLE_THAT_DOES_NOT_EXIST")
            "#,
            cbu_name, entity_name
        );

        let program = parse_program(&dsl).map_err(|e| anyhow::anyhow!("{}", e))?;
        let plan = compile(&program)?;

        let mut ctx = ExecutionContext::new();

        // Execute atomically - should fail on the invalid role
        let result = executor.execute_plan_atomic(&plan, &mut ctx).await;

        // Verify the execution failed
        assert!(
            result.is_err(),
            "Expected execution to fail due to invalid role"
        );

        // CRITICAL: Verify rollback happened - neither CBU nor entity should exist
        let cbu_found = db.cbu_exists(&cbu_name).await?;
        let entity_found = db.entity_exists(&format!("{} Smith", entity_name)).await?;

        assert!(
            !cbu_found,
            "CBU '{}' should NOT exist after rollback",
            cbu_name
        );
        assert!(!entity_found, "Entity should NOT exist after rollback");

        db.cleanup().await?;
        Ok(())
    }

    /// Test that successful atomic execution commits all changes
    ///
    /// Scenario:
    /// 1. Create a CBU
    /// 2. Create a person entity
    /// 3. Assign a valid role
    ///
    /// Expected: All three operations should be visible after commit
    #[tokio::test]
    #[ignore] // Requires database
    async fn test_atomic_commit_on_success() -> Result<()> {
        let db = TestDb::new().await?;
        let executor = DslExecutor::new(db.pool.clone());

        let cbu_name = db.name("success_fund");
        let entity_name = db.name("success");

        // DSL with valid operations
        let dsl = format!(
            r#"
            (cbu.ensure :name "{}" :jurisdiction "LU" :as @fund)
            (entity.create-proper-person :first-name "{}" :last-name "Jones" :as @person)
            (cbu.assign-role :cbu-id @fund :entity-id @person :role "DIRECTOR")
            "#,
            cbu_name, entity_name
        );

        let program = parse_program(&dsl).map_err(|e| anyhow::anyhow!("{}", e))?;
        let plan = compile(&program)?;

        let mut ctx = ExecutionContext::new();

        // Execute atomically - should succeed
        let result = executor.execute_plan_atomic(&plan, &mut ctx).await;
        assert!(
            result.is_ok(),
            "Expected execution to succeed: {:?}",
            result.err()
        );

        // Verify commit happened - both CBU and entity should exist
        let cbu_found = db.cbu_exists(&cbu_name).await?;
        let entity_found = db.entity_exists(&format!("{} Jones", entity_name)).await?;

        assert!(cbu_found, "CBU '{}' should exist after commit", cbu_name);
        assert!(entity_found, "Entity should exist after commit");

        db.cleanup().await?;
        Ok(())
    }

    /// Test that non-atomic execution does NOT rollback on failure
    ///
    /// This is a contrast test to show the difference between
    /// execute_plan (no rollback) vs execute_plan_atomic (with rollback)
    ///
    /// Scenario:
    /// 1. Create a CBU (succeeds)
    /// 2. Create a person (succeeds)
    /// 3. Invalid role assignment (fails)
    ///
    /// Expected with execute_plan: CBU and entity SHOULD exist (no rollback)
    #[tokio::test]
    #[ignore] // Requires database
    async fn test_non_atomic_no_rollback() -> Result<()> {
        let db = TestDb::new().await?;
        let executor = DslExecutor::new(db.pool.clone());

        let cbu_name = db.name("norollback_fund");
        let entity_name = db.name("norollback");

        // DSL with an invalid role that will fail
        let dsl = format!(
            r#"
            (cbu.ensure :name "{}" :jurisdiction "LU" :as @fund)
            (entity.create-proper-person :first-name "{}" :last-name "Brown" :as @person)
            (cbu.assign-role :cbu-id @fund :entity-id @person :role "INVALID_ROLE_XYZ")
            "#,
            cbu_name, entity_name
        );

        let program = parse_program(&dsl).map_err(|e| anyhow::anyhow!("{}", e))?;
        let plan = compile(&program)?;

        let mut ctx = ExecutionContext::new();

        // Execute non-atomically (regular execute_plan) - should fail on invalid role
        let result = executor.execute_plan(&plan, &mut ctx).await;

        // Verify the execution failed
        assert!(
            result.is_err(),
            "Expected execution to fail due to invalid role"
        );

        // CONTRAST: With non-atomic execution, CBU and entity SHOULD exist
        // because each verb auto-commits independently
        let cbu_found = db.cbu_exists(&cbu_name).await?;
        let entity_found = db.entity_exists(&format!("{} Brown", entity_name)).await?;

        assert!(
            cbu_found,
            "CBU '{}' SHOULD exist with non-atomic execution (no rollback)",
            cbu_name
        );
        assert!(
            entity_found,
            "Entity SHOULD exist with non-atomic execution (no rollback)"
        );

        db.cleanup().await?;
        Ok(())
    }

    /// Test rollback with multiple entities created before failure
    ///
    /// Scenario: Create 3 entities, then fail on the 4th operation
    /// Expected: All 3 entities should be rolled back
    #[tokio::test]
    #[ignore] // Requires database
    async fn test_rollback_multiple_entities() -> Result<()> {
        let db = TestDb::new().await?;
        let executor = DslExecutor::new(db.pool.clone());

        let cbu_name = db.name("multi_fund");
        let person1 = db.name("multi_p1");
        let person2 = db.name("multi_p2");
        let person3 = db.name("multi_p3");

        // DSL that creates multiple entities before failing
        let dsl = format!(
            r#"
            (cbu.ensure :name "{}" :jurisdiction "LU" :as @fund)
            (entity.create-proper-person :first-name "{}" :last-name "One" :as @p1)
            (entity.create-proper-person :first-name "{}" :last-name "Two" :as @p2)
            (entity.create-proper-person :first-name "{}" :last-name "Three" :as @p3)
            (cbu.assign-role :cbu-id @fund :entity-id @p1 :role "DOES_NOT_EXIST")
            "#,
            cbu_name, person1, person2, person3
        );

        let program = parse_program(&dsl).map_err(|e| anyhow::anyhow!("{}", e))?;
        let plan = compile(&program)?;

        let mut ctx = ExecutionContext::new();

        // Execute atomically - should fail
        let result = executor.execute_plan_atomic(&plan, &mut ctx).await;
        assert!(result.is_err(), "Expected execution to fail");

        // Verify ALL entities were rolled back
        assert!(
            !db.cbu_exists(&cbu_name).await?,
            "CBU should be rolled back"
        );
        assert!(
            !db.entity_exists(&format!("{} One", person1)).await?,
            "Entity 1 should be rolled back"
        );
        assert!(
            !db.entity_exists(&format!("{} Two", person2)).await?,
            "Entity 2 should be rolled back"
        );
        assert!(
            !db.entity_exists(&format!("{} Three", person3)).await?,
            "Entity 3 should be rolled back"
        );

        db.cleanup().await?;
        Ok(())
    }
}
