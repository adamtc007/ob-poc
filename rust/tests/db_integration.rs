//! Database integration tests for DSL executor
//!
//! These tests verify that DSL operations correctly persist to the database
//! and that data can be read back with correct values.

#[cfg(feature = "database")]
mod db_tests {
    use anyhow::Result;
    use sqlx::PgPool;
    use uuid::Uuid;

    use ob_poc::dsl_v2::{compile, parse_program, DslExecutor, ExecutionContext};

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
            let prefix = format!("test_{}", &Uuid::new_v4().to_string()[..8]);
            Ok(Self { pool, prefix })
        }

        fn name(&self, base: &str) -> String {
            format!("{}_{}", self.prefix, base)
        }

        async fn cleanup(&self) -> Result<()> {
            let pattern = format!("{}%", self.prefix);

            // Delete kyc schema tables first (new case model)
            sqlx::query(
                r#"DELETE FROM kyc.screenings WHERE workstream_id IN
                   (SELECT w.workstream_id FROM kyc.entity_workstreams w
                    JOIN kyc.cases c ON c.case_id = w.case_id
                    JOIN "ob-poc".cbus cbu ON cbu.cbu_id = c.cbu_id
                    WHERE cbu.name LIKE $1)"#,
            )
            .bind(&pattern)
            .execute(&self.pool)
            .await
            .ok();

            sqlx::query(
                r#"DELETE FROM kyc.entity_workstreams WHERE case_id IN
                   (SELECT c.case_id FROM kyc.cases c
                    JOIN "ob-poc".cbus cbu ON cbu.cbu_id = c.cbu_id
                    WHERE cbu.name LIKE $1)"#,
            )
            .bind(&pattern)
            .execute(&self.pool)
            .await
            .ok();

            sqlx::query(
                r#"DELETE FROM kyc.cases WHERE cbu_id IN
                   (SELECT cbu_id FROM "ob-poc".cbus WHERE name LIKE $1)"#,
            )
            .bind(&pattern)
            .execute(&self.pool)
            .await
            .ok();

            // Delete ob-poc tables in reverse dependency order
            sqlx::query(
                r#"DELETE FROM "ob-poc".document_catalog WHERE cbu_id IN
                   (SELECT cbu_id FROM "ob-poc".cbus WHERE name LIKE $1)"#,
            )
            .bind(&pattern)
            .execute(&self.pool)
            .await
            .ok();

            sqlx::query(
                r#"DELETE FROM "ob-poc".cbu_entity_roles WHERE cbu_id IN
                   (SELECT cbu_id FROM "ob-poc".cbus WHERE name LIKE $1)"#,
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

            sqlx::query(r#"DELETE FROM "ob-poc".cbus WHERE name LIKE $1"#)
                .bind(&pattern)
                .execute(&self.pool)
                .await
                .ok();

            Ok(())
        }

        async fn execute_dsl(&self, dsl: &str) -> Result<ExecutionContext> {
            let ast = parse_program(dsl).map_err(|e| anyhow::anyhow!("{:?}", e))?;
            let plan = compile(&ast).map_err(|e| anyhow::anyhow!("{:?}", e))?;
            let executor = DslExecutor::new(self.pool.clone());
            let mut ctx = ExecutionContext::new();
            executor.execute_plan(&plan, &mut ctx).await?;
            Ok(ctx)
        }
    }

    // Helper: Read CBU back from DB
    async fn read_cbu(pool: &PgPool, id: Uuid) -> Result<CbuRow> {
        let row = sqlx::query_as!(
            CbuRow,
            r#"SELECT cbu_id, name, jurisdiction, client_type
               FROM "ob-poc".cbus WHERE cbu_id = $1"#,
            id
        )
        .fetch_one(pool)
        .await?;
        Ok(row)
    }

    // Helper: Read entity back from DB
    async fn read_entity(pool: &PgPool, id: Uuid) -> Result<EntityRow> {
        let row = sqlx::query_as!(
            EntityRow,
            r#"SELECT e.entity_id, e.name, et.type_code as entity_type
               FROM "ob-poc".entities e
               JOIN "ob-poc".entity_types et ON e.entity_type_id = et.entity_type_id
               WHERE e.entity_id = $1"#,
            id
        )
        .fetch_one(pool)
        .await?;
        Ok(row)
    }

    // Helper: Count roles for a CBU
    async fn count_roles(pool: &PgPool, cbu_id: Uuid) -> Result<i64> {
        let count: Option<i64> = sqlx::query_scalar(
            r#"SELECT COUNT(*) FROM "ob-poc".cbu_entity_roles WHERE cbu_id = $1"#,
        )
        .bind(cbu_id)
        .fetch_one(pool)
        .await?;
        Ok(count.unwrap_or(0))
    }

    // Helper: Count documents for a CBU
    async fn count_documents(pool: &PgPool, cbu_id: Uuid) -> Result<i64> {
        let count: Option<i64> = sqlx::query_scalar(
            r#"SELECT COUNT(*) FROM "ob-poc".document_catalog WHERE cbu_id = $1"#,
        )
        .bind(cbu_id)
        .fetch_one(pool)
        .await?;
        Ok(count.unwrap_or(0))
    }

    // Helper: Count screenings for an entity (via kyc.screenings joined to workstreams)
    async fn count_screenings(pool: &PgPool, entity_id: Uuid) -> Result<i64> {
        let count: Option<i64> = sqlx::query_scalar(
            r#"SELECT COUNT(*) FROM kyc.screenings s
               JOIN kyc.entity_workstreams w ON w.workstream_id = s.workstream_id
               WHERE w.entity_id = $1"#,
        )
        .bind(entity_id)
        .fetch_one(pool)
        .await?;
        Ok(count.unwrap_or(0))
    }

    #[derive(Debug)]
    #[allow(dead_code)]
    struct CbuRow {
        cbu_id: Uuid,
        name: String,
        jurisdiction: Option<String>,
        client_type: Option<String>,
    }

    #[derive(Debug)]
    #[allow(dead_code)]
    struct EntityRow {
        entity_id: Uuid,
        name: String,
        entity_type: Option<String>,
    }

    // =========================================================================
    // ROUND-TRIP TESTS
    // =========================================================================

    #[tokio::test]
    async fn test_cbu_round_trip() -> Result<()> {
        let db = TestDb::new().await?;
        let name = db.name("RoundTripCBU");

        let dsl = format!(
            r#"
            (cbu.create :name "{}" :client-type "corporate" :jurisdiction "GB" :as @cbu)
        "#,
            name
        );

        let ctx = db.execute_dsl(&dsl).await?;

        // Read back and verify
        let cbu_id = ctx.resolve("cbu").expect("cbu should be bound");
        let record = read_cbu(&db.pool, cbu_id).await?;

        assert_eq!(record.name, name);
        assert_eq!(record.jurisdiction.as_deref(), Some("GB"));
        assert_eq!(record.client_type.as_deref(), Some("corporate"));

        db.cleanup().await?;
        Ok(())
    }

    #[tokio::test]
    async fn test_entity_type_mapping() -> Result<()> {
        let db = TestDb::new().await?;

        let dsl = format!(
            r#"
            (cbu.create :name "{}" :as @cbu)
            (entity.create-limited-company :cbu-id @cbu :name "{}" :as @company)
            (entity.create-proper-person :cbu-id @cbu :first-name "John" :last-name "Doe" :as @person)
        "#,
            db.name("TypeMapCBU"),
            db.name("Company")
        );

        let ctx = db.execute_dsl(&dsl).await?;

        // Verify entity types mapped correctly
        let company = read_entity(&db.pool, ctx.resolve("company").unwrap()).await?;
        let person = read_entity(&db.pool, ctx.resolve("person").unwrap()).await?;

        // Check entity types contain expected substrings (actual codes may vary)
        assert!(
            company
                .entity_type
                .as_ref()
                .map(|t| t.contains("LIMITED_COMPANY"))
                .unwrap_or(false),
            "Company type should contain LIMITED_COMPANY, got {:?}",
            company.entity_type
        );
        assert!(
            person
                .entity_type
                .as_ref()
                .map(|t| t.contains("PROPER_PERSON"))
                .unwrap_or(false),
            "Person type should contain PROPER_PERSON, got {:?}",
            person.entity_type
        );

        db.cleanup().await?;
        Ok(())
    }

    #[tokio::test]
    async fn test_role_assignment_fk() -> Result<()> {
        let db = TestDb::new().await?;

        let dsl = format!(
            r#"
            (cbu.create :name "{}" :as @cbu)
            (entity.create-limited-company :cbu-id @cbu :name "{}" :as @company)
            (entity.create-proper-person :cbu-id @cbu :first-name "Jane" :last-name "Doe" :as @ubo)
            (cbu.assign-role :cbu-id @cbu :entity-id @ubo :role "BENEFICIAL_OWNER")
        "#,
            db.name("FKTestCBU"),
            db.name("FKCompany")
        );

        let ctx = db.execute_dsl(&dsl).await?;

        // Verify role was created with correct FKs
        let cbu_id = ctx.resolve("cbu").unwrap();

        let role_count = count_roles(&db.pool, cbu_id).await?;
        assert_eq!(role_count, 1, "Should have exactly 1 role assigned");

        db.cleanup().await?;
        Ok(())
    }

    // =========================================================================
    // FOREIGN KEY CONSTRAINT TESTS
    // =========================================================================

    #[tokio::test]
    async fn test_fk_undefined_symbol() -> Result<()> {
        let db = TestDb::new().await?;

        let dsl = r#"
            (entity.create-proper-person :cbu-id @nonexistent :first-name "Test" :last-name "User")
        "#;

        let result = db.execute_dsl(dsl).await;

        assert!(result.is_err());
        let err = result.unwrap_err().to_string();
        assert!(
            err.contains("Unresolved") || err.contains("nonexistent"),
            "Error should mention unresolved symbol: {}",
            err
        );

        Ok(())
    }

    #[tokio::test]
    async fn test_fk_invalid_role() -> Result<()> {
        let db = TestDb::new().await?;

        let dsl = format!(
            r#"
            (cbu.create :name "{}" :as @cbu)
            (entity.create-limited-company :cbu-id @cbu :name "{}" :as @company)
            (entity.create-proper-person :cbu-id @cbu :first-name "Test" :last-name "Person" :as @person)
            (cbu.assign-role :cbu-id @cbu :entity-id @person :role "INVALID_ROLE_XYZ")
        "#,
            db.name("InvalidRoleCBU"),
            db.name("InvalidRoleCompany")
        );

        let result = db.execute_dsl(&dsl).await;

        assert!(result.is_err());
        let err = result.unwrap_err().to_string();
        assert!(
            err.contains("role") || err.contains("Unknown") || err.contains("no rows"),
            "Error should mention invalid role or lookup failure: {}",
            err
        );

        db.cleanup().await?;
        Ok(())
    }

    // =========================================================================
    // EDGE CASE TESTS
    // =========================================================================

    #[tokio::test]
    async fn test_unicode_names() -> Result<()> {
        let db = TestDb::new().await?;

        let dsl = format!(
            r#"
            (cbu.create :name "{}株式会社" :jurisdiction "JP" :as @cbu)
            (entity.create-limited-company :cbu-id @cbu :name "{}東京支店" :as @company)
        "#,
            db.name("Unicode"),
            db.name("Unicode")
        );

        let ctx = db.execute_dsl(&dsl).await?;

        let cbu = read_cbu(&db.pool, ctx.resolve("cbu").unwrap()).await?;
        assert!(
            cbu.name.contains("株式会社"),
            "Should preserve Japanese chars"
        );

        db.cleanup().await?;
        Ok(())
    }

    #[tokio::test]
    async fn test_special_characters() -> Result<()> {
        let db = TestDb::new().await?;

        // Test SQL injection prevention and special char handling
        let dsl = format!(
            r#"
            (cbu.create :name "{} O'Brien & Co." :as @cbu)
        "#,
            db.name("SpecialChar")
        );

        let ctx = db.execute_dsl(&dsl).await?;

        let cbu = read_cbu(&db.pool, ctx.resolve("cbu").unwrap()).await?;
        assert!(cbu.name.contains("O'Brien"), "Should preserve apostrophe");

        db.cleanup().await?;
        Ok(())
    }

    #[tokio::test]
    async fn test_null_vs_empty_optional_fields() -> Result<()> {
        let db = TestDb::new().await?;

        // Omit optional jurisdiction - should be NULL not empty string
        let dsl = format!(
            r#"
            (cbu.create :name "{}" :client-type "individual" :as @cbu)
        "#,
            db.name("NullTest")
        );

        let ctx = db.execute_dsl(&dsl).await?;
        let cbu = read_cbu(&db.pool, ctx.resolve("cbu").unwrap()).await?;

        assert!(cbu.jurisdiction.is_none(), "Jurisdiction should be NULL");

        db.cleanup().await?;
        Ok(())
    }

    #[tokio::test]
    async fn test_duplicate_bindings_overwrites() -> Result<()> {
        let db = TestDb::new().await?;

        let dsl = format!(
            r#"
            (cbu.create :name "{}_First" :as @item)
            (cbu.create :name "{}_Second" :as @item)
        "#,
            db.name("DupBind"),
            db.name("DupBind")
        );

        let ctx = db.execute_dsl(&dsl).await?;

        // @item should point to the second CBU
        let item_id = ctx.resolve("item").unwrap();
        let cbu = read_cbu(&db.pool, item_id).await?;
        assert!(cbu.name.contains("Second"), "Should bind to second CBU");

        db.cleanup().await?;
        Ok(())
    }

    // =========================================================================
    // FULL SCENARIO TESTS
    // =========================================================================

    // =========================================================================
    // CBU PRODUCT ASSIGNMENT TESTS (Critical Lifecycle)
    // =========================================================================

    /// Helper: Count service delivery entries for a CBU
    async fn count_service_deliveries(pool: &PgPool, cbu_id: Uuid) -> Result<i64> {
        let count: Option<i64> = sqlx::query_scalar(
            r#"SELECT COUNT(*) FROM "ob-poc".service_delivery_map WHERE cbu_id = $1"#,
        )
        .bind(cbu_id)
        .fetch_one(pool)
        .await?;
        Ok(count.unwrap_or(0))
    }

    /// Helper: Get CBU product_id
    async fn get_cbu_product_id(pool: &PgPool, cbu_id: Uuid) -> Result<Option<Uuid>> {
        let product_id: Option<Uuid> =
            sqlx::query_scalar(r#"SELECT product_id FROM "ob-poc".cbus WHERE cbu_id = $1"#)
                .bind(cbu_id)
                .fetch_one(pool)
                .await?;
        Ok(product_id)
    }

    /// Helper: Get service count for a product
    async fn get_product_service_count(pool: &PgPool, product_name: &str) -> Result<i64> {
        let count: Option<i64> = sqlx::query_scalar(
            r#"SELECT COUNT(*) FROM "ob-poc".product_services ps
               JOIN "ob-poc".products p ON ps.product_id = p.product_id
               WHERE p.name = $1"#,
        )
        .bind(product_name)
        .fetch_one(pool)
        .await?;
        Ok(count.unwrap_or(0))
    }

    #[tokio::test]
    async fn test_cbu_add_product_creates_delivery_entries() -> Result<()> {
        let db = TestDb::new().await?;

        // Create a CBU
        let dsl = format!(
            r#"(cbu.create :name "{}" :client-type "corporate" :jurisdiction "GB" :as @cbu)"#,
            db.name("ProductTestCBU")
        );
        let ctx = db.execute_dsl(&dsl).await?;
        let cbu_id = ctx.resolve("cbu").unwrap();

        // Verify CBU has no product and no deliveries initially
        assert!(
            get_cbu_product_id(&db.pool, cbu_id).await?.is_none(),
            "CBU should have no product initially"
        );
        assert_eq!(
            count_service_deliveries(&db.pool, cbu_id).await?,
            0,
            "CBU should have no service deliveries initially"
        );

        // Add product (Custody has 11 services)
        let add_product_dsl = format!(
            r#"(cbu.add-product :cbu-id "{}" :product "CUSTODY")"#,
            cbu_id
        );
        db.execute_dsl(&add_product_dsl).await?;

        // Note: cbus.product_id is NOT set by cbu.add-product
        // service_delivery_map is the source of truth for CBU->Product relationships
        // A CBU can have multiple products, tracked via service_delivery_map

        // Verify service delivery entries created (should match product's service count)
        let expected_services = get_product_service_count(&db.pool, "Custody").await?;
        let actual_deliveries = count_service_deliveries(&db.pool, cbu_id).await?;
        assert_eq!(
            actual_deliveries, expected_services,
            "Should create one delivery entry per service"
        );
        assert!(
            actual_deliveries >= 10,
            "CUSTODY product should have at least 10 services"
        );

        // Cleanup
        sqlx::query(r#"DELETE FROM "ob-poc".service_delivery_map WHERE cbu_id = $1"#)
            .bind(cbu_id)
            .execute(&db.pool)
            .await?;
        db.cleanup().await?;
        Ok(())
    }

    #[tokio::test]
    async fn test_cbu_add_product_idempotent() -> Result<()> {
        let db = TestDb::new().await?;

        // Create CBU and add product
        let dsl = format!(
            r#"(cbu.create :name "{}" :client-type "fund" :jurisdiction "LU" :as @cbu)"#,
            db.name("IdempotentCBU")
        );
        let ctx = db.execute_dsl(&dsl).await?;
        let cbu_id = ctx.resolve("cbu").unwrap();

        // Add product first time
        let add_product_dsl = format!(
            r#"(cbu.add-product :cbu-id "{}" :product "CUSTODY")"#,
            cbu_id
        );
        db.execute_dsl(&add_product_dsl).await?;

        let first_count = count_service_deliveries(&db.pool, cbu_id).await?;

        // Add same product again (should be idempotent)
        db.execute_dsl(&add_product_dsl).await?;

        let second_count = count_service_deliveries(&db.pool, cbu_id).await?;

        assert_eq!(
            first_count, second_count,
            "Re-running add-product should not duplicate entries"
        );

        // Cleanup
        sqlx::query(r#"DELETE FROM "ob-poc".service_delivery_map WHERE cbu_id = $1"#)
            .bind(cbu_id)
            .execute(&db.pool)
            .await?;
        db.cleanup().await?;
        Ok(())
    }

    #[tokio::test]
    async fn test_cbu_add_product_invalid_product() -> Result<()> {
        let db = TestDb::new().await?;

        let dsl = format!(
            r#"(cbu.create :name "{}" :as @cbu)"#,
            db.name("InvalidProductCBU")
        );
        let ctx = db.execute_dsl(&dsl).await?;
        let cbu_id = ctx.resolve("cbu").unwrap();

        // Try to add non-existent product
        let bad_dsl = format!(
            r#"(cbu.add-product :cbu-id "{}" :product "NonExistentProduct")"#,
            cbu_id
        );
        let result = db.execute_dsl(&bad_dsl).await;

        assert!(result.is_err(), "Should fail for unknown product");
        let err = result.unwrap_err().to_string();
        assert!(
            err.contains("not found") || err.contains("Unknown"),
            "Error should mention product not found: {}",
            err
        );

        db.cleanup().await?;
        Ok(())
    }

    #[tokio::test]
    async fn test_cbu_add_product_invalid_cbu() -> Result<()> {
        let db = TestDb::new().await?;

        // Try to add product to non-existent CBU
        let fake_cbu_id = Uuid::new_v4();
        let dsl = format!(
            r#"(cbu.add-product :cbu-id "{}" :product "CUSTODY")"#,
            fake_cbu_id
        );
        let result = db.execute_dsl(&dsl).await;

        assert!(result.is_err(), "Should fail for unknown CBU");
        let err = result.unwrap_err().to_string();
        assert!(
            err.contains("not found") || err.contains("CBU"),
            "Error should mention CBU not found: {}",
            err
        );

        Ok(())
    }

    #[tokio::test]
    async fn test_cbu_add_product_all_deliveries_pending() -> Result<()> {
        let db = TestDb::new().await?;

        let dsl = format!(
            r#"(cbu.create :name "{}" :as @cbu)"#,
            db.name("PendingStatusCBU")
        );
        let ctx = db.execute_dsl(&dsl).await?;
        let cbu_id = ctx.resolve("cbu").unwrap();

        let add_dsl = format!(
            r#"(cbu.add-product :cbu-id "{}" :product "CUSTODY")"#,
            cbu_id
        );
        db.execute_dsl(&add_dsl).await?;

        // Verify all entries have PENDING status
        let non_pending: Option<i64> = sqlx::query_scalar(
            r#"SELECT COUNT(*) FROM "ob-poc".service_delivery_map
               WHERE cbu_id = $1 AND delivery_status != 'PENDING'"#,
        )
        .bind(cbu_id)
        .fetch_one(&db.pool)
        .await?;

        assert_eq!(
            non_pending.unwrap_or(0),
            0,
            "All delivery entries should have PENDING status"
        );

        // Cleanup
        sqlx::query(r#"DELETE FROM "ob-poc".service_delivery_map WHERE cbu_id = $1"#)
            .bind(cbu_id)
            .execute(&db.pool)
            .await?;
        db.cleanup().await?;
        Ok(())
    }

    // =========================================================================
    // FULL SCENARIO TESTS
    // =========================================================================

    #[tokio::test]
    async fn test_full_corporate_onboarding() -> Result<()> {
        let db = TestDb::new().await?;

        // Full corporate onboarding with KYC case model:
        // 1. Create CBU and entities
        // 2. Assign roles
        // 3. Catalog documents
        // 4. Create KYC case
        // 5. Create entity workstreams for UBOs
        // 6. Run screenings via case-screening.run
        let dsl = format!(
            r#"
            (cbu.create :name "{}" :client-type "corporate" :jurisdiction "GB" :as @cbu)
            (entity.create-limited-company :cbu-id @cbu :name "{}" :as @company)
            (entity.create-proper-person :cbu-id @cbu :first-name "Alice" :last-name "UBO1" :as @ubo1)
            (entity.create-proper-person :cbu-id @cbu :first-name "Bob" :last-name "UBO2" :as @ubo2)
            (cbu.assign-role :cbu-id @cbu :entity-id @ubo1 :role "BENEFICIAL_OWNER")
            (cbu.assign-role :cbu-id @cbu :entity-id @ubo2 :role "BENEFICIAL_OWNER")
            (document.catalog :cbu-id @cbu :doc-type "CERTIFICATE_OF_INCORPORATION" :title "Company Certificate")
            (document.catalog :cbu-id @cbu :doc-type "PASSPORT" :title "Alice Passport")
            (document.catalog :cbu-id @cbu :doc-type "PASSPORT" :title "Bob Passport")
            (kyc-case.create :cbu-id @cbu :case-type "NEW_CLIENT" :as @case)
            (entity-workstream.create :case-id @case :entity-id @ubo1 :as @ws1)
            (entity-workstream.create :case-id @case :entity-id @ubo2 :as @ws2)
            (case-screening.run :workstream-id @ws1 :screening-type "PEP")
            (case-screening.run :workstream-id @ws1 :screening-type "SANCTIONS")
            (case-screening.run :workstream-id @ws2 :screening-type "PEP")
            (case-screening.run :workstream-id @ws2 :screening-type "SANCTIONS")
        "#,
            db.name("FullCBU"),
            db.name("FullCompany")
        );

        let ctx = db.execute_dsl(&dsl).await?;

        let cbu_id = ctx.resolve("cbu").unwrap();
        let ubo1_id = ctx.resolve("ubo1").unwrap();
        let ubo2_id = ctx.resolve("ubo2").unwrap();

        // Verify counts
        assert_eq!(
            count_roles(&db.pool, cbu_id).await?,
            2,
            "Should have 2 roles"
        );
        assert_eq!(
            count_documents(&db.pool, cbu_id).await?,
            3,
            "Should have 3 documents"
        );
        assert_eq!(
            count_screenings(&db.pool, ubo1_id).await?,
            2,
            "UBO1 should have 2 screenings"
        );
        assert_eq!(
            count_screenings(&db.pool, ubo2_id).await?,
            2,
            "UBO2 should have 2 screenings"
        );

        db.cleanup().await?;
        Ok(())
    }

    // =========================================================================
    // CBU DECISION & SNAPSHOT TESTS
    // =========================================================================

    /// Snapshot status point structure returned from database
    #[derive(Debug)]
    #[allow(dead_code)]
    struct SnapshotRow {
        snapshot_id: Uuid,
        case_id: Uuid,
        decision_made: Option<String>,
        decision_made_by: Option<String>,
        decision_notes: Option<String>,
        recommended_action: Option<String>,
    }

    /// CBU status row
    #[derive(Debug)]
    #[allow(dead_code)]
    struct CbuStatusRow {
        cbu_id: Uuid,
        name: String,
        status: Option<String>,
    }

    /// Case status row
    #[derive(Debug)]
    #[allow(dead_code)]
    struct CaseStatusRow {
        case_id: Uuid,
        status: String,
        closed_at: Option<chrono::DateTime<chrono::Utc>>,
    }

    /// Change log row
    #[derive(Debug)]
    #[allow(dead_code)]
    struct ChangeLogRow {
        log_id: Uuid,
        cbu_id: Uuid,
        change_type: String,
        field_name: Option<String>,
        old_value: Option<serde_json::Value>,
        new_value: Option<serde_json::Value>,
        reason: Option<String>,
    }

    /// Helper: Get CBU with status
    async fn read_cbu_status(pool: &PgPool, cbu_id: Uuid) -> Result<CbuStatusRow> {
        let row = sqlx::query_as!(
            CbuStatusRow,
            r#"SELECT cbu_id, name, status FROM "ob-poc".cbus WHERE cbu_id = $1"#,
            cbu_id
        )
        .fetch_one(pool)
        .await?;
        Ok(row)
    }

    /// Helper: Get case status
    async fn read_case_status(pool: &PgPool, case_id: Uuid) -> Result<CaseStatusRow> {
        let row = sqlx::query_as!(
            CaseStatusRow,
            r#"SELECT case_id, status, closed_at FROM kyc.cases WHERE case_id = $1"#,
            case_id
        )
        .fetch_one(pool)
        .await?;
        Ok(row)
    }

    /// Helper: Get latest snapshot for a case
    async fn get_latest_snapshot(pool: &PgPool, case_id: Uuid) -> Result<Option<SnapshotRow>> {
        let row = sqlx::query_as!(
            SnapshotRow,
            r#"SELECT snapshot_id, case_id, decision_made, decision_made_by,
                      decision_notes, recommended_action
               FROM "ob-poc".case_evaluation_snapshots
               WHERE case_id = $1
               ORDER BY evaluated_at DESC
               LIMIT 1"#,
            case_id
        )
        .fetch_optional(pool)
        .await?;
        Ok(row)
    }

    /// Helper: Count snapshots for a case
    async fn count_snapshots(pool: &PgPool, case_id: Uuid) -> Result<i64> {
        let count: Option<i64> = sqlx::query_scalar(
            r#"SELECT COUNT(*) FROM "ob-poc".case_evaluation_snapshots WHERE case_id = $1"#,
        )
        .bind(case_id)
        .fetch_one(pool)
        .await?;
        Ok(count.unwrap_or(0))
    }

    /// Helper: Get change log entries for a CBU
    async fn get_change_log(pool: &PgPool, cbu_id: Uuid) -> Result<Vec<ChangeLogRow>> {
        let rows = sqlx::query_as!(
            ChangeLogRow,
            r#"SELECT log_id, cbu_id, change_type, field_name,
                      old_value, new_value, reason
               FROM "ob-poc".cbu_change_log
               WHERE cbu_id = $1
               ORDER BY changed_at DESC"#,
            cbu_id
        )
        .fetch_all(pool)
        .await?;
        Ok(rows)
    }

    /// Helper: Create a CBU with KYC case ready for decision
    async fn setup_cbu_for_decision(db: &TestDb) -> Result<(Uuid, Uuid)> {
        let dsl = format!(
            r#"
            (cbu.create :name "{}" :client-type "corporate" :jurisdiction "GB" :as @cbu)
            (entity.create-proper-person :cbu-id @cbu :first-name "Test" :last-name "UBO" :as @ubo)
            (cbu.assign-role :cbu-id @cbu :entity-id @ubo :role "BENEFICIAL_OWNER")
            (kyc-case.create :cbu-id @cbu :case-type "NEW_CLIENT" :as @case)
            (entity-workstream.create :case-id @case :entity-id @ubo :as @ws)
            (case-screening.run :workstream-id @ws :screening-type "SANCTIONS")
            (case-screening.complete :screening-id @ws :status "CLEAR" :result-summary "No matches found")
            (entity-workstream.complete :workstream-id @ws)
            "#,
            db.name("DecisionCBU")
        );

        let ctx = db.execute_dsl(&dsl).await?;
        let cbu_id = ctx.resolve("cbu").expect("cbu should be bound");
        let case_id = ctx.resolve("case").expect("case should be bound");

        // Update case to REVIEW status (ready for decision)
        sqlx::query(r#"UPDATE kyc.cases SET status = 'REVIEW' WHERE case_id = $1"#)
            .bind(case_id)
            .execute(&db.pool)
            .await?;

        Ok((cbu_id, case_id))
    }

    /// Helper: Cleanup decision test data
    async fn cleanup_decision_data(db: &TestDb, cbu_id: Uuid, case_id: Uuid) -> Result<()> {
        // Clean up in reverse dependency order
        sqlx::query(r#"DELETE FROM "ob-poc".case_evaluation_snapshots WHERE case_id = $1"#)
            .bind(case_id)
            .execute(&db.pool)
            .await
            .ok();
        sqlx::query(r#"DELETE FROM "ob-poc".cbu_change_log WHERE cbu_id = $1"#)
            .bind(cbu_id)
            .execute(&db.pool)
            .await
            .ok();
        db.cleanup().await?;
        Ok(())
    }

    #[tokio::test]
    async fn test_snapshot_created_on_case_evaluation() -> Result<()> {
        let db = TestDb::new().await?;
        let (cbu_id, case_id) = setup_cbu_for_decision(&db).await?;

        // Verify initial state - no snapshots yet
        let initial_count = count_snapshots(&db.pool, case_id).await?;
        assert_eq!(initial_count, 0, "Should have no snapshots initially");

        // Create an evaluation snapshot directly (simulating case evaluation)
        sqlx::query(
            r#"INSERT INTO "ob-poc".case_evaluation_snapshots
               (case_id, soft_count, escalate_count, hard_stop_count,
                total_score, recommended_action, evaluated_by)
               VALUES ($1, 0, 0, 0, 0, 'APPROVE', 'test_system')"#,
        )
        .bind(case_id)
        .execute(&db.pool)
        .await?;

        // Verify snapshot was created
        let after_count = count_snapshots(&db.pool, case_id).await?;
        assert_eq!(after_count, 1, "Should have 1 snapshot after evaluation");

        let snapshot = get_latest_snapshot(&db.pool, case_id).await?;
        assert!(snapshot.is_some(), "Should find the snapshot");
        let snapshot = snapshot.unwrap();
        assert_eq!(snapshot.recommended_action.as_deref(), Some("APPROVE"));
        assert!(snapshot.decision_made.is_none(), "Decision not yet made");

        cleanup_decision_data(&db, cbu_id, case_id).await?;
        Ok(())
    }

    #[tokio::test]
    async fn test_snapshot_decision_recorded() -> Result<()> {
        let db = TestDb::new().await?;
        let (cbu_id, case_id) = setup_cbu_for_decision(&db).await?;

        // Create evaluation snapshot
        let snapshot_id = Uuid::new_v4();
        sqlx::query(
            r#"INSERT INTO "ob-poc".case_evaluation_snapshots
               (snapshot_id, case_id, soft_count, escalate_count, hard_stop_count,
                total_score, recommended_action, evaluated_by)
               VALUES ($1, $2, 0, 0, 0, 0, 'APPROVE', 'test_system')"#,
        )
        .bind(snapshot_id)
        .bind(case_id)
        .execute(&db.pool)
        .await?;

        // Record a decision on the snapshot
        sqlx::query(
            r#"UPDATE "ob-poc".case_evaluation_snapshots
               SET decision_made = 'APPROVE',
                   decision_made_at = NOW(),
                   decision_made_by = 'compliance_officer@test.com',
                   decision_notes = 'All checks passed. Approved for onboarding.'
               WHERE snapshot_id = $1"#,
        )
        .bind(snapshot_id)
        .execute(&db.pool)
        .await?;

        // Verify decision recorded
        let snapshot = get_latest_snapshot(&db.pool, case_id).await?.unwrap();
        assert_eq!(snapshot.decision_made.as_deref(), Some("APPROVE"));
        assert_eq!(
            snapshot.decision_made_by.as_deref(),
            Some("compliance_officer@test.com")
        );
        assert!(snapshot.decision_notes.is_some());
        assert!(snapshot.decision_notes.unwrap().contains("Approved"));

        cleanup_decision_data(&db, cbu_id, case_id).await?;
        Ok(())
    }

    #[tokio::test]
    async fn test_snapshot_multiple_evaluations() -> Result<()> {
        let db = TestDb::new().await?;
        let (cbu_id, case_id) = setup_cbu_for_decision(&db).await?;

        // Create first evaluation - ESCALATE recommendation
        sqlx::query(
            r#"INSERT INTO "ob-poc".case_evaluation_snapshots
               (case_id, soft_count, escalate_count, hard_stop_count,
                total_score, recommended_action, evaluated_by, notes)
               VALUES ($1, 2, 1, 0, 15, 'ESCALATE', 'analyst@test.com', 'Needs senior review')"#,
        )
        .bind(case_id)
        .execute(&db.pool)
        .await?;

        // Small delay to ensure different timestamps
        tokio::time::sleep(tokio::time::Duration::from_millis(10)).await;

        // Create second evaluation - APPROVE after escalation review
        sqlx::query(
            r#"INSERT INTO "ob-poc".case_evaluation_snapshots
               (case_id, soft_count, escalate_count, hard_stop_count,
                total_score, recommended_action, evaluated_by, notes,
                decision_made, decision_made_by, decision_made_at, decision_notes)
               VALUES ($1, 2, 0, 0, 10, 'APPROVE_WITH_CONDITIONS', 'senior_compliance@test.com',
                       'Flags mitigated',
                       'APPROVE_WITH_CONDITIONS', 'senior_compliance@test.com', NOW(),
                       'Approved with enhanced monitoring')"#,
        )
        .bind(case_id)
        .execute(&db.pool)
        .await?;

        // Verify we have 2 snapshots
        let count = count_snapshots(&db.pool, case_id).await?;
        assert_eq!(count, 2, "Should have 2 evaluation snapshots");

        // Verify latest snapshot is the approval
        let latest = get_latest_snapshot(&db.pool, case_id).await?.unwrap();
        assert_eq!(
            latest.recommended_action.as_deref(),
            Some("APPROVE_WITH_CONDITIONS")
        );
        assert_eq!(
            latest.decision_made.as_deref(),
            Some("APPROVE_WITH_CONDITIONS")
        );

        cleanup_decision_data(&db, cbu_id, case_id).await?;
        Ok(())
    }

    #[tokio::test]
    async fn test_snapshot_with_hard_stop() -> Result<()> {
        let db = TestDb::new().await?;
        let (cbu_id, case_id) = setup_cbu_for_decision(&db).await?;

        // Create evaluation with hard stop (sanctions hit)
        sqlx::query(
            r#"INSERT INTO "ob-poc".case_evaluation_snapshots
               (case_id, soft_count, escalate_count, hard_stop_count,
                has_hard_stop, total_score, recommended_action, evaluated_by, notes)
               VALUES ($1, 0, 0, 1, true, 100, 'DO_NOT_ONBOARD', 'system',
                       'Sanctions match confirmed')"#,
        )
        .bind(case_id)
        .execute(&db.pool)
        .await?;

        // Verify hard stop recorded
        let snapshot = get_latest_snapshot(&db.pool, case_id).await?.unwrap();
        assert_eq!(
            snapshot.recommended_action.as_deref(),
            Some("DO_NOT_ONBOARD")
        );

        // Record rejection decision
        sqlx::query(
            r#"UPDATE "ob-poc".case_evaluation_snapshots
               SET decision_made = 'DO_NOT_ONBOARD',
                   decision_made_at = NOW(),
                   decision_made_by = 'mlro@test.com',
                   decision_notes = 'OFAC sanctions match. Unable to proceed.'
               WHERE case_id = $1"#,
        )
        .bind(case_id)
        .execute(&db.pool)
        .await?;

        let updated = get_latest_snapshot(&db.pool, case_id).await?.unwrap();
        assert_eq!(updated.decision_made.as_deref(), Some("DO_NOT_ONBOARD"));

        cleanup_decision_data(&db, cbu_id, case_id).await?;
        Ok(())
    }

    #[tokio::test]
    async fn test_change_log_records_status_change() -> Result<()> {
        let db = TestDb::new().await?;
        let (cbu_id, case_id) = setup_cbu_for_decision(&db).await?;

        // Get initial change log count
        let initial_logs = get_change_log(&db.pool, cbu_id).await?;
        let initial_count = initial_logs.len();

        // Update CBU status (this should trigger change log via trigger)
        sqlx::query(r#"UPDATE "ob-poc".cbus SET status = 'VALIDATED' WHERE cbu_id = $1"#)
            .bind(cbu_id)
            .execute(&db.pool)
            .await?;

        // Check change log has new entry
        let logs = get_change_log(&db.pool, cbu_id).await?;
        assert!(
            logs.len() > initial_count,
            "Change log should have new entry after status change"
        );

        // Verify the latest log entry
        if let Some(latest) = logs.first() {
            assert_eq!(latest.change_type, "STATUS_CHANGE");
            assert_eq!(latest.field_name.as_deref(), Some("status"));
        }

        cleanup_decision_data(&db, cbu_id, case_id).await?;
        Ok(())
    }

    #[tokio::test]
    async fn test_case_status_transitions() -> Result<()> {
        let db = TestDb::new().await?;
        let (cbu_id, case_id) = setup_cbu_for_decision(&db).await?;

        // Verify initial status
        let case = read_case_status(&db.pool, case_id).await?;
        assert_eq!(case.status, "REVIEW");
        assert!(case.closed_at.is_none());

        // Transition to APPROVED
        sqlx::query(
            r#"UPDATE kyc.cases
               SET status = 'APPROVED', closed_at = NOW()
               WHERE case_id = $1"#,
        )
        .bind(case_id)
        .execute(&db.pool)
        .await?;

        let approved = read_case_status(&db.pool, case_id).await?;
        assert_eq!(approved.status, "APPROVED");
        assert!(
            approved.closed_at.is_some(),
            "Should have closed_at timestamp"
        );

        cleanup_decision_data(&db, cbu_id, case_id).await?;
        Ok(())
    }

    #[tokio::test]
    async fn test_cbu_status_reflects_decision() -> Result<()> {
        let db = TestDb::new().await?;
        let (cbu_id, case_id) = setup_cbu_for_decision(&db).await?;

        // Initial CBU status - may be NULL initially
        let _initial = read_cbu_status(&db.pool, cbu_id).await?;

        // Simulate approved decision - update CBU status
        sqlx::query(r#"UPDATE "ob-poc".cbus SET status = 'VALIDATED' WHERE cbu_id = $1"#)
            .bind(cbu_id)
            .execute(&db.pool)
            .await?;

        let approved = read_cbu_status(&db.pool, cbu_id).await?;
        assert_eq!(approved.status.as_deref(), Some("VALIDATED"));

        // Test rejection flow - VALIDATION_FAILED is the rejection status
        sqlx::query(r#"UPDATE "ob-poc".cbus SET status = 'VALIDATION_FAILED' WHERE cbu_id = $1"#)
            .bind(cbu_id)
            .execute(&db.pool)
            .await?;

        let rejected = read_cbu_status(&db.pool, cbu_id).await?;
        assert_eq!(rejected.status.as_deref(), Some("VALIDATION_FAILED"));

        cleanup_decision_data(&db, cbu_id, case_id).await?;
        Ok(())
    }

    #[tokio::test]
    async fn test_snapshot_audit_trail_integrity() -> Result<()> {
        let db = TestDb::new().await?;
        let (cbu_id, case_id) = setup_cbu_for_decision(&db).await?;

        // Create a complete audit trail: evaluation → decision → status change

        // 1. Evaluation snapshot
        let snapshot_id = Uuid::new_v4();
        sqlx::query(
            r#"INSERT INTO "ob-poc".case_evaluation_snapshots
               (snapshot_id, case_id, soft_count, escalate_count, hard_stop_count,
                total_score, recommended_action, evaluated_by)
               VALUES ($1, $2, 1, 0, 0, 5, 'APPROVE', 'analyst@test.com')"#,
        )
        .bind(snapshot_id)
        .bind(case_id)
        .execute(&db.pool)
        .await?;

        // 2. Record decision
        sqlx::query(
            r#"UPDATE "ob-poc".case_evaluation_snapshots
               SET decision_made = 'APPROVE',
                   decision_made_at = NOW(),
                   decision_made_by = 'reviewer@test.com',
                   decision_notes = 'Approved after review'
               WHERE snapshot_id = $1"#,
        )
        .bind(snapshot_id)
        .execute(&db.pool)
        .await?;

        // 3. Update case status
        sqlx::query(
            r#"UPDATE kyc.cases SET status = 'APPROVED', closed_at = NOW() WHERE case_id = $1"#,
        )
        .bind(case_id)
        .execute(&db.pool)
        .await?;

        // 4. Update CBU status
        sqlx::query(r#"UPDATE "ob-poc".cbus SET status = 'VALIDATED' WHERE cbu_id = $1"#)
            .bind(cbu_id)
            .execute(&db.pool)
            .await?;

        // Verify complete audit trail
        let snapshot = get_latest_snapshot(&db.pool, case_id).await?.unwrap();
        let case_status = read_case_status(&db.pool, case_id).await?;
        let cbu_status = read_cbu_status(&db.pool, cbu_id).await?;
        let change_logs = get_change_log(&db.pool, cbu_id).await?;

        // Assertions
        assert_eq!(snapshot.decision_made.as_deref(), Some("APPROVE"));
        assert_eq!(case_status.status, "APPROVED");
        assert!(case_status.closed_at.is_some());
        assert_eq!(cbu_status.status.as_deref(), Some("VALIDATED"));
        assert!(!change_logs.is_empty(), "Should have change log entries");

        cleanup_decision_data(&db, cbu_id, case_id).await?;
        Ok(())
    }

    #[tokio::test]
    async fn test_snapshot_referral_flow() -> Result<()> {
        let db = TestDb::new().await?;
        let (cbu_id, case_id) = setup_cbu_for_decision(&db).await?;

        // Create evaluation recommending escalation
        sqlx::query(
            r#"INSERT INTO "ob-poc".case_evaluation_snapshots
               (case_id, soft_count, escalate_count, hard_stop_count,
                total_score, recommended_action, required_escalation_level,
                evaluated_by, notes)
               VALUES ($1, 0, 2, 0, 20, 'ESCALATE', 'SENIOR_COMPLIANCE',
                       'system', 'PEP match requires senior review')"#,
        )
        .bind(case_id)
        .execute(&db.pool)
        .await?;

        // Record referral decision (not final approval/rejection)
        sqlx::query(
            r#"UPDATE "ob-poc".case_evaluation_snapshots
               SET decision_made = 'ESCALATE',
                   decision_made_at = NOW(),
                   decision_made_by = 'analyst@test.com',
                   decision_notes = 'Referred to senior compliance for PEP review'
               WHERE case_id = $1"#,
        )
        .bind(case_id)
        .execute(&db.pool)
        .await?;

        // Escalation stays in REVIEW status but changes escalation_level
        // (ESCALATED is not a valid status - escalation is tracked via escalation_level)
        sqlx::query(
            r#"UPDATE kyc.cases SET escalation_level = 'SENIOR_COMPLIANCE' WHERE case_id = $1"#,
        )
        .bind(case_id)
        .execute(&db.pool)
        .await?;

        // Verify referral state
        let snapshot = get_latest_snapshot(&db.pool, case_id).await?.unwrap();
        let case_status = read_case_status(&db.pool, case_id).await?;

        assert_eq!(snapshot.decision_made.as_deref(), Some("ESCALATE"));
        assert_eq!(
            case_status.status, "REVIEW",
            "Escalated case stays in REVIEW"
        );
        assert!(
            case_status.closed_at.is_none(),
            "Referral should not close the case"
        );

        cleanup_decision_data(&db, cbu_id, case_id).await?;
        Ok(())
    }
}
