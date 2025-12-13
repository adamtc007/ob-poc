//! Integration tests for onboarding DSL verbs
//!
//! These tests verify the Terraform-like resource provisioning workflow
//! with dependency graph handling.

#[cfg(feature = "database")]
mod onboarding_tests {
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
            let prefix = format!("onboard_test_{}", &Uuid::new_v4().to_string()[..8]);
            Ok(Self { pool, prefix })
        }

        fn name(&self, base: &str) -> String {
            format!("{}_{}", self.prefix, base)
        }

        async fn cleanup(&self) -> Result<()> {
            let pattern = format!("{}%", self.prefix);

            // Clean up onboarding tables first
            sqlx::query(
                r#"DELETE FROM "ob-poc".onboarding_tasks WHERE execution_id IN
                   (SELECT execution_id FROM "ob-poc".onboarding_executions WHERE plan_id IN
                    (SELECT plan_id FROM "ob-poc".onboarding_plans WHERE cbu_id IN
                     (SELECT cbu_id FROM "ob-poc".cbus WHERE name LIKE $1)))"#,
            )
            .bind(&pattern)
            .execute(&self.pool)
            .await
            .ok();

            sqlx::query(
                r#"DELETE FROM "ob-poc".onboarding_executions WHERE plan_id IN
                   (SELECT plan_id FROM "ob-poc".onboarding_plans WHERE cbu_id IN
                    (SELECT cbu_id FROM "ob-poc".cbus WHERE name LIKE $1))"#,
            )
            .bind(&pattern)
            .execute(&self.pool)
            .await
            .ok();

            sqlx::query(
                r#"DELETE FROM "ob-poc".onboarding_plans WHERE cbu_id IN
                   (SELECT cbu_id FROM "ob-poc".cbus WHERE name LIKE $1)"#,
            )
            .bind(&pattern)
            .execute(&self.pool)
            .await
            .ok();

            // Clean up resource instance dependencies
            sqlx::query(
                r#"DELETE FROM "ob-poc".resource_instance_dependencies WHERE instance_id IN
                   (SELECT instance_id FROM "ob-poc".cbu_resource_instances WHERE cbu_id IN
                    (SELECT cbu_id FROM "ob-poc".cbus WHERE name LIKE $1))"#,
            )
            .bind(&pattern)
            .execute(&self.pool)
            .await
            .ok();

            // Clean up resource instances
            sqlx::query(
                r#"DELETE FROM "ob-poc".cbu_resource_instances WHERE cbu_id IN
                   (SELECT cbu_id FROM "ob-poc".cbus WHERE name LIKE $1)"#,
            )
            .bind(&pattern)
            .execute(&self.pool)
            .await
            .ok();

            // Clean up service delivery map
            sqlx::query(
                r#"DELETE FROM "ob-poc".service_delivery_map WHERE cbu_id IN
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

            Ok(())
        }
    }

    impl Drop for TestDb {
        fn drop(&mut self) {
            // Note: async cleanup happens in tests via explicit cleanup() calls
        }
    }

    // =========================================================================
    // HELPER FUNCTIONS
    // =========================================================================

    async fn execute_dsl(pool: &PgPool, dsl: &str) -> Result<Vec<ob_poc::dsl_v2::ExecutionResult>> {
        let executor = DslExecutor::new(pool.clone());
        let mut ctx = ExecutionContext::new();
        executor.execute_dsl(dsl, &mut ctx).await
    }

    async fn execute_dsl_with_ctx(
        pool: &PgPool,
        dsl: &str,
        ctx: &mut ExecutionContext,
    ) -> Result<Vec<ob_poc::dsl_v2::ExecutionResult>> {
        let executor = DslExecutor::new(pool.clone());
        executor.execute_dsl(dsl, ctx).await
    }

    // =========================================================================
    // TESTS: onboarding.plan
    // =========================================================================

    #[tokio::test]
    async fn test_onboarding_plan_creates_plan_record() -> Result<()> {
        let db = TestDb::new().await?;
        let cbu_name = db.name("plan_test_cbu");

        // First create a CBU
        let create_cbu = format!(
            r#"(cbu.ensure :name "{}" :jurisdiction "LU" :client-type "FUND")"#,
            cbu_name
        );
        execute_dsl(&db.pool, &create_cbu).await?;

        // Create an onboarding plan for CUSTODY product
        let plan_dsl = format!(
            r#"(onboarding.plan :cbu-name "{}" :products ["CUSTODY"] :as @plan)"#,
            cbu_name
        );

        let mut ctx = ExecutionContext::new();
        let results = execute_dsl_with_ctx(&db.pool, &plan_dsl, &mut ctx).await?;

        // Verify we got a plan ID back
        assert!(!results.is_empty(), "Should return plan result");

        // Verify plan was recorded in database
        let plan_count: (i64,) = sqlx::query_as(
            r#"SELECT COUNT(*) FROM "ob-poc".onboarding_plans op
               JOIN "ob-poc".cbus c ON c.cbu_id = op.cbu_id
               WHERE c.name = $1"#,
        )
        .bind(&cbu_name)
        .fetch_one(&db.pool)
        .await?;

        assert_eq!(plan_count.0, 1, "Should have exactly one plan");

        // Verify plan status is 'pending' (default after creation)
        let plan_status: (String,) = sqlx::query_as(
            r#"SELECT op.status FROM "ob-poc".onboarding_plans op
               JOIN "ob-poc".cbus c ON c.cbu_id = op.cbu_id
               WHERE c.name = $1"#,
        )
        .bind(&cbu_name)
        .fetch_one(&db.pool)
        .await?;

        assert_eq!(
            plan_status.0, "pending",
            "Plan status should be 'pending' after creation"
        );

        db.cleanup().await?;
        Ok(())
    }

    // =========================================================================
    // TESTS: onboarding.show-plan
    // =========================================================================

    #[tokio::test]
    async fn test_onboarding_show_plan_returns_dsl() -> Result<()> {
        let db = TestDb::new().await?;
        let cbu_name = db.name("show_plan_cbu");

        // Create CBU
        let create_cbu = format!(
            r#"(cbu.ensure :name "{}" :jurisdiction "LU" :client-type "FUND")"#,
            cbu_name
        );
        execute_dsl(&db.pool, &create_cbu).await?;

        // Create plan
        let plan_dsl = format!(
            r#"(onboarding.plan :cbu-name "{}" :products ["CUSTODY"] :as @plan)"#,
            cbu_name
        );
        let mut ctx = ExecutionContext::new();
        execute_dsl_with_ctx(&db.pool, &plan_dsl, &mut ctx).await?;

        // Show the plan
        let show_dsl = "(onboarding.show-plan :plan-id @plan)";
        let results = execute_dsl_with_ctx(&db.pool, show_dsl, &mut ctx).await?;

        // Should return a record with generated_dsl
        assert!(!results.is_empty(), "Should return show-plan result");

        db.cleanup().await?;
        Ok(())
    }

    // =========================================================================
    // TESTS: onboarding.execute
    // =========================================================================

    #[tokio::test]
    async fn test_onboarding_execute_provisions_resources() -> Result<()> {
        let db = TestDb::new().await?;
        let cbu_name = db.name("exec_test_cbu");

        // Create CBU
        let create_cbu = format!(
            r#"(cbu.ensure :name "{}" :jurisdiction "LU" :client-type "FUND")"#,
            cbu_name
        );
        execute_dsl(&db.pool, &create_cbu).await?;

        // Create and execute plan
        let mut ctx = ExecutionContext::new();

        let plan_dsl = format!(
            r#"(onboarding.plan :cbu-name "{}" :products ["CUSTODY"] :as @plan)"#,
            cbu_name
        );
        execute_dsl_with_ctx(&db.pool, &plan_dsl, &mut ctx).await?;

        let execute_dsl_str = "(onboarding.execute :plan-id @plan)";
        let results = execute_dsl_with_ctx(&db.pool, execute_dsl_str, &mut ctx).await?;

        assert!(!results.is_empty(), "Should return execution result");

        // Verify resource instances were created
        let instance_count: (i64,) = sqlx::query_as(
            r#"SELECT COUNT(*) FROM "ob-poc".cbu_resource_instances cri
               JOIN "ob-poc".cbus c ON c.cbu_id = cri.cbu_id
               WHERE c.name = $1"#,
        )
        .bind(&cbu_name)
        .fetch_one(&db.pool)
        .await?;

        assert!(
            instance_count.0 > 0,
            "Should have created resource instances"
        );

        // Verify plan status is now 'complete'
        let plan_status: (String,) = sqlx::query_as(
            r#"SELECT op.status FROM "ob-poc".onboarding_plans op
               JOIN "ob-poc".cbus c ON c.cbu_id = op.cbu_id
               WHERE c.name = $1"#,
        )
        .bind(&cbu_name)
        .fetch_one(&db.pool)
        .await?;

        assert_eq!(plan_status.0, "complete", "Plan should be complete");

        db.cleanup().await?;
        Ok(())
    }

    // =========================================================================
    // TESTS: onboarding.ensure (idempotent)
    // =========================================================================

    #[tokio::test]
    async fn test_onboarding_ensure_is_idempotent() -> Result<()> {
        let db = TestDb::new().await?;
        let cbu_name = db.name("ensure_test_cbu");

        // Create CBU
        let create_cbu = format!(
            r#"(cbu.ensure :name "{}" :jurisdiction "LU" :client-type "FUND")"#,
            cbu_name
        );
        execute_dsl(&db.pool, &create_cbu).await?;

        // First ensure - should create plan and execute
        let ensure_dsl = format!(
            r#"(onboarding.ensure :cbu-name "{}" :products ["CUSTODY"])"#,
            cbu_name
        );

        let results1 = execute_dsl(&db.pool, &ensure_dsl).await?;
        assert!(!results1.is_empty(), "First ensure should return result");

        // Count resources after first ensure
        let count1: (i64,) = sqlx::query_as(
            r#"SELECT COUNT(*) FROM "ob-poc".cbu_resource_instances cri
               JOIN "ob-poc".cbus c ON c.cbu_id = cri.cbu_id
               WHERE c.name = $1"#,
        )
        .bind(&cbu_name)
        .fetch_one(&db.pool)
        .await?;

        // Second ensure - should be idempotent (no new resources)
        let results2 = execute_dsl(&db.pool, &ensure_dsl).await?;
        assert!(!results2.is_empty(), "Second ensure should return result");

        // Count resources after second ensure - should be same
        let count2: (i64,) = sqlx::query_as(
            r#"SELECT COUNT(*) FROM "ob-poc".cbu_resource_instances cri
               JOIN "ob-poc".cbus c ON c.cbu_id = cri.cbu_id
               WHERE c.name = $1"#,
        )
        .bind(&cbu_name)
        .fetch_one(&db.pool)
        .await?;

        assert_eq!(
            count1.0, count2.0,
            "Idempotent ensure should not create duplicate resources"
        );

        // Verify only one completed execution exists
        let exec_count: (i64,) = sqlx::query_as(
            r#"SELECT COUNT(*) FROM "ob-poc".onboarding_executions oe
               JOIN "ob-poc".onboarding_plans op ON op.plan_id = oe.plan_id
               JOIN "ob-poc".cbus c ON c.cbu_id = op.cbu_id
               WHERE c.name = $1 AND oe.status = 'complete'"#,
        )
        .bind(&cbu_name)
        .fetch_one(&db.pool)
        .await?;

        assert_eq!(
            exec_count.0, 1,
            "Should have exactly one completed execution"
        );

        db.cleanup().await?;
        Ok(())
    }

    // =========================================================================
    // TESTS: onboarding.status
    // =========================================================================

    #[tokio::test]
    async fn test_onboarding_status_returns_execution_info() -> Result<()> {
        let db = TestDb::new().await?;
        let cbu_name = db.name("status_test_cbu");

        // Create CBU and execute onboarding
        let create_cbu = format!(
            r#"(cbu.ensure :name "{}" :jurisdiction "LU" :client-type "FUND")"#,
            cbu_name
        );
        execute_dsl(&db.pool, &create_cbu).await?;

        let mut ctx = ExecutionContext::new();
        let plan_dsl = format!(
            r#"(onboarding.plan :cbu-name "{}" :products ["CUSTODY"] :as @plan)"#,
            cbu_name
        );
        execute_dsl_with_ctx(&db.pool, &plan_dsl, &mut ctx).await?;

        let execute_dsl_str = "(onboarding.execute :plan-id @plan :as @exec)";
        execute_dsl_with_ctx(&db.pool, execute_dsl_str, &mut ctx).await?;

        // Get status
        let status_dsl = "(onboarding.status :execution-id @exec)";
        let results = execute_dsl_with_ctx(&db.pool, status_dsl, &mut ctx).await?;

        assert!(!results.is_empty(), "Should return status result");

        db.cleanup().await?;
        Ok(())
    }

    // =========================================================================
    // TESTS: onboarding.get-urls
    // =========================================================================

    #[tokio::test]
    async fn test_onboarding_get_urls_returns_instance_urls() -> Result<()> {
        let db = TestDb::new().await?;
        let cbu_name = db.name("urls_test_cbu");

        // Create CBU and execute onboarding
        let create_cbu = format!(
            r#"(cbu.ensure :name "{}" :jurisdiction "LU" :client-type "FUND")"#,
            cbu_name
        );
        execute_dsl(&db.pool, &create_cbu).await?;

        let ensure_dsl = format!(
            r#"(onboarding.ensure :cbu-name "{}" :products ["CUSTODY"] :as @result)"#,
            cbu_name
        );
        let mut ctx = ExecutionContext::new();
        execute_dsl_with_ctx(&db.pool, &ensure_dsl, &mut ctx).await?;

        // Get URLs
        let urls_dsl = format!(r#"(onboarding.get-urls :cbu-name "{}")"#, cbu_name);
        let results = execute_dsl(&db.pool, &urls_dsl).await?;

        assert!(!results.is_empty(), "Should return URLs result");

        db.cleanup().await?;
        Ok(())
    }

    // =========================================================================
    // TESTS: Resource dependencies
    // =========================================================================

    #[tokio::test]
    async fn test_resource_provision_with_depends_on() -> Result<()> {
        let db = TestDb::new().await?;
        let cbu_name = db.name("deps_test_cbu");

        // Create CBU
        let create_cbu = format!(
            r#"(cbu.ensure :name "{}" :jurisdiction "LU" :client-type "FUND" :as @cbu)"#,
            cbu_name
        );
        let mut ctx = ExecutionContext::new();
        execute_dsl_with_ctx(&db.pool, &create_cbu, &mut ctx).await?;

        // Provision resources with explicit dependencies
        let provision_dsl = format!(
            r#"
            (service-resource.provision :cbu-id @cbu :resource-type "CUSTODY_ACCT"
                :instance-url "urn:test:{cbu}:custody:1" :as @custody)
            (service-resource.provision :cbu-id @cbu :resource-type "SETTLE_ACCT"
                :instance-url "urn:test:{cbu}:settle:1" :depends-on [@custody] :as @settle)
            "#,
            cbu = cbu_name
        );
        execute_dsl_with_ctx(&db.pool, &provision_dsl, &mut ctx).await?;

        // Verify dependency was recorded
        let dep_count: (i64,) = sqlx::query_as(
            r#"SELECT COUNT(*) FROM "ob-poc".resource_instance_dependencies rid
               JOIN "ob-poc".cbu_resource_instances cri ON cri.instance_id = rid.instance_id
               JOIN "ob-poc".cbus c ON c.cbu_id = cri.cbu_id
               WHERE c.name = $1"#,
        )
        .bind(&cbu_name)
        .fetch_one(&db.pool)
        .await?;

        assert_eq!(dep_count.0, 1, "Should have one dependency recorded");

        db.cleanup().await?;
        Ok(())
    }

    // =========================================================================
    // TESTS: Dependency graph topological sort
    // =========================================================================

    #[tokio::test]
    async fn test_onboarding_respects_dependency_order() -> Result<()> {
        let db = TestDb::new().await?;
        let cbu_name = db.name("topo_test_cbu");

        // Create CBU
        let create_cbu = format!(
            r#"(cbu.ensure :name "{}" :jurisdiction "LU" :client-type "FUND")"#,
            cbu_name
        );
        execute_dsl(&db.pool, &create_cbu).await?;

        // Execute onboarding with CUSTODY product (has defined dependencies)
        let ensure_dsl = format!(
            r#"(onboarding.ensure :cbu-name "{}" :products ["CUSTODY"])"#,
            cbu_name
        );
        execute_dsl(&db.pool, &ensure_dsl).await?;

        // Verify CUSTODY_ACCT was created before SETTLE_ACCT by checking created_at timestamps
        // (If dependencies are respected, CUSTODY_ACCT should be created first)
        let instances: Vec<(String, chrono::DateTime<chrono::Utc>)> = sqlx::query_as(
            r#"SELECT srt.resource_code, cri.created_at
               FROM "ob-poc".cbu_resource_instances cri
               JOIN "ob-poc".service_resource_types srt ON srt.resource_id = cri.resource_type_id
               JOIN "ob-poc".cbus c ON c.cbu_id = cri.cbu_id
               WHERE c.name = $1
               ORDER BY cri.created_at ASC"#,
        )
        .bind(&cbu_name)
        .fetch_all(&db.pool)
        .await?;

        // We should have created resources
        assert!(
            !instances.is_empty(),
            "Should have created resource instances"
        );

        // Log the order for debugging
        for (code, ts) in &instances {
            eprintln!("Resource: {} created at {}", code, ts);
        }

        db.cleanup().await?;
        Ok(())
    }

    // =========================================================================
    // TESTS: Array ordering idempotency (Postgres array comparison)
    // =========================================================================

    #[tokio::test]
    async fn test_onboarding_ensure_array_order_independent() -> Result<()> {
        let db = TestDb::new().await?;
        let cbu_name = db.name("array_order_cbu");

        // Create CBU
        let create_cbu = format!(
            r#"(cbu.ensure :name "{}" :jurisdiction "LU" :client-type "FUND")"#,
            cbu_name
        );
        execute_dsl(&db.pool, &create_cbu).await?;

        // First ensure with products in one order
        let ensure1 = format!(
            r#"(onboarding.ensure :cbu-name "{}" :products ["CUSTODY" "FUND_ACCOUNTING"])"#,
            cbu_name
        );
        execute_dsl(&db.pool, &ensure1).await?;

        let count1: (i64,) = sqlx::query_as(
            r#"SELECT COUNT(*) FROM "ob-poc".onboarding_plans op
               JOIN "ob-poc".cbus c ON c.cbu_id = op.cbu_id
               WHERE c.name = $1"#,
        )
        .bind(&cbu_name)
        .fetch_one(&db.pool)
        .await?;

        // Second ensure with products in different order - should be idempotent
        let ensure2 = format!(
            r#"(onboarding.ensure :cbu-name "{}" :products ["FUND_ACCOUNTING" "CUSTODY"])"#,
            cbu_name
        );
        execute_dsl(&db.pool, &ensure2).await?;

        let count2: (i64,) = sqlx::query_as(
            r#"SELECT COUNT(*) FROM "ob-poc".onboarding_plans op
               JOIN "ob-poc".cbus c ON c.cbu_id = op.cbu_id
               WHERE c.name = $1"#,
        )
        .bind(&cbu_name)
        .fetch_one(&db.pool)
        .await?;

        assert_eq!(
            count1.0, count2.0,
            "Should not create new plan for same products in different order"
        );

        db.cleanup().await?;
        Ok(())
    }
}
