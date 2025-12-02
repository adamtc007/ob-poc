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
}
