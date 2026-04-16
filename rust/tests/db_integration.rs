//! Database integration tests for DSL executor
//!
//! These tests verify that DSL operations correctly persist to the database
//! and that data can be read back with correct values.

#[cfg(feature = "database")]
mod db_tests {
    use anyhow::Result;
    use ob_poc::dsl_v2::execution::runtime_registry;
    use sqlx::PgPool;
    use std::path::PathBuf;
    use uuid::Uuid;

    use ob_poc::dsl_v2::execution::{DslExecutor, ExecutionContext};
    use ob_poc::dsl_v2::planning::compile;
    use ob_poc::dsl_v2::syntax::parse_program;

    // =========================================================================
    // TEST INFRASTRUCTURE
    // =========================================================================

    struct TestDb {
        pool: PgPool,
        prefix: String,
    }

    impl TestDb {
        async fn new() -> Result<Self> {
            let config_dir = PathBuf::from(env!("CARGO_MANIFEST_DIR")).join("config");
            std::env::set_var("DSL_CONFIG_DIR", &config_dir);

            let url = std::env::var("TEST_DATABASE_URL")
                .or_else(|_| std::env::var("DATABASE_URL"))
                .unwrap_or_else(|_| "postgresql:///data_designer".into());

            let pool = PgPool::connect(&url).await?;
            sqlx::query(
                r#"
                CREATE TABLE IF NOT EXISTS "ob-poc".cbu_structure_links (
                    link_id UUID PRIMARY KEY DEFAULT gen_random_uuid(),
                    parent_cbu_id UUID NOT NULL REFERENCES "ob-poc".cbus(cbu_id) ON DELETE CASCADE,
                    child_cbu_id UUID NOT NULL REFERENCES "ob-poc".cbus(cbu_id) ON DELETE CASCADE,
                    relationship_type VARCHAR(32) NOT NULL,
                    relationship_selector VARCHAR(64) NOT NULL,
                    status VARCHAR(20) NOT NULL DEFAULT 'ACTIVE',
                    capital_flow VARCHAR(32),
                    effective_from DATE,
                    effective_to DATE,
                    created_at TIMESTAMPTZ NOT NULL DEFAULT NOW(),
                    updated_at TIMESTAMPTZ NOT NULL DEFAULT NOW(),
                    terminated_at TIMESTAMPTZ,
                    terminated_reason TEXT,
                    CONSTRAINT cbu_structure_links_no_self_link CHECK (parent_cbu_id <> child_cbu_id),
                    CONSTRAINT cbu_structure_links_status_check
                        CHECK (status IN ('ACTIVE', 'TERMINATED', 'SUSPENDED'))
                )
                "#,
            )
            .execute(&pool)
            .await?;
            sqlx::query(
                r#"
                CREATE UNIQUE INDEX IF NOT EXISTS uq_cbu_structure_links_active
                    ON "ob-poc".cbu_structure_links(parent_cbu_id, child_cbu_id, relationship_type)
                    WHERE status = 'ACTIVE'
                "#,
            )
            .execute(&pool)
            .await?;
            sqlx::query(
                r#"
                CREATE INDEX IF NOT EXISTS idx_cbu_structure_links_parent_selector
                    ON "ob-poc".cbu_structure_links(parent_cbu_id, relationship_selector)
                    WHERE status = 'ACTIVE'
                "#,
            )
            .execute(&pool)
            .await?;
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
                r#"DELETE FROM "ob-poc".screenings WHERE workstream_id IN
                   (SELECT w.workstream_id FROM "ob-poc".entity_workstreams w
                    JOIN "ob-poc".cases c ON c.case_id = w.case_id
                    JOIN "ob-poc".cbus cbu ON cbu.cbu_id = c.cbu_id
                    WHERE cbu.name LIKE $1)"#,
            )
            .bind(&pattern)
            .execute(&self.pool)
            .await
            .ok();

            sqlx::query(
                r#"DELETE FROM "ob-poc".entity_workstreams WHERE case_id IN
                   (SELECT c.case_id FROM "ob-poc".cases c
                    JOIN "ob-poc".cbus cbu ON cbu.cbu_id = c.cbu_id
                    WHERE cbu.name LIKE $1)"#,
            )
            .bind(&pattern)
            .execute(&self.pool)
            .await
            .ok();

            sqlx::query(
                r#"DELETE FROM "ob-poc".cases WHERE cbu_id IN
                   (SELECT cbu_id FROM "ob-poc".cbus WHERE name LIKE $1)"#,
            )
            .bind(&pattern)
            .execute(&self.pool)
            .await
            .ok();

            // Delete ob-poc tables in reverse dependency order
            sqlx::query(
                r#"DELETE FROM "ob-poc".cbu_structure_links
                   WHERE parent_cbu_id IN (SELECT cbu_id FROM "ob-poc".cbus WHERE name LIKE $1)
                      OR child_cbu_id IN (SELECT cbu_id FROM "ob-poc".cbus WHERE name LIKE $1)"#,
            )
            .bind(&pattern)
            .execute(&self.pool)
            .await
            .ok();

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

    #[test]
    fn test_runtime_registry_contains_baseline_verbs() {
        let config_dir = PathBuf::from(env!("CARGO_MANIFEST_DIR")).join("config");
        std::env::set_var("DSL_CONFIG_DIR", &config_dir);

        let loader = dsl_core::config::loader::ConfigLoader::from_env();
        let config = loader
            .load_verbs()
            .expect("ConfigLoader should load verb YAML for db_integration");
        assert!(
            config
                .domains
                .get("cbu")
                .and_then(|d| d.verbs.get("create"))
                .is_some(),
            "loaded verb config should include cbu.create"
        );

        let registry = runtime_registry();
        assert!(
            registry.get("cbu", "create").is_some(),
            "runtime registry should include cbu.create"
        );
        assert!(
            registry.get("entity", "ensure-or-placeholder").is_some(),
            "runtime registry should include entity.ensure-or-placeholder"
        );
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

    // Helper: Count screenings for an entity (via "ob-poc".screenings joined to workstreams)
    async fn count_screenings(pool: &PgPool, entity_id: Uuid) -> Result<i64> {
        let count: Option<i64> = sqlx::query_scalar(
            r#"SELECT COUNT(*) FROM "ob-poc".screenings s
               JOIN "ob-poc".entity_workstreams w ON w.workstream_id = s.workstream_id
               WHERE w.entity_id = $1"#,
        )
        .bind(entity_id)
        .fetch_one(pool)
        .await?;
        Ok(count.unwrap_or(0))
    }

    async fn get_workstream_id(pool: &PgPool, case_id: Uuid, entity_id: Uuid) -> Result<Uuid> {
        let workstream_id: Uuid = sqlx::query_scalar(
            r#"SELECT workstream_id
               FROM "ob-poc".entity_workstreams
               WHERE case_id = $1 AND entity_id = $2"#,
        )
        .bind(case_id)
        .bind(entity_id)
        .fetch_one(pool)
        .await?;
        Ok(workstream_id)
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

    #[derive(Debug, sqlx::FromRow)]
    #[allow(dead_code)]
    struct StructureLinkRow {
        link_id: Uuid,
        parent_cbu_id: Uuid,
        child_cbu_id: Uuid,
        relationship_type: String,
        relationship_selector: String,
        status: String,
        capital_flow: Option<String>,
        terminated_reason: Option<String>,
    }

    async fn read_structure_link(
        pool: &PgPool,
        parent_cbu_id: Uuid,
        child_cbu_id: Uuid,
    ) -> Result<StructureLinkRow> {
        let row = sqlx::query_as::<_, StructureLinkRow>(
            r#"SELECT
                  link_id,
                  parent_cbu_id,
                  child_cbu_id,
                  relationship_type,
                  relationship_selector,
                  status,
                  capital_flow,
                  terminated_reason
               FROM "ob-poc".cbu_structure_links
               WHERE parent_cbu_id = $1
                 AND child_cbu_id = $2
               ORDER BY created_at DESC
               LIMIT 1"#,
        )
        .bind(parent_cbu_id)
        .bind(child_cbu_id)
        .fetch_one(pool)
        .await?;
        Ok(row)
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
    async fn test_cbu_link_structure_round_trip() -> Result<()> {
        let db = TestDb::new().await?;
        let parent_name = db.name("Master");
        let child_name = db.name("Feeder");

        let dsl = format!(
            r#"
            (cbu.create :name "{}" :jurisdiction "KY" :as @parent)
            (cbu.create :name "{}" :jurisdiction "US" :as @child)
            (cbu.link-structure
              :parent-cbu-id @parent
              :child-cbu-id @child
              :relationship-type "feeder"
              :qualifier "US"
              :capital-flow "upstream"
              :as @linked)
        "#,
            parent_name, child_name
        );

        let ctx = db.execute_dsl(&dsl).await?;
        let parent_id = ctx.resolve("parent").expect("parent should be bound");
        let child_id = ctx.resolve("child").expect("child should be bound");
        assert_eq!(ctx.resolve("linked"), Some(child_id));

        let link = read_structure_link(&db.pool, parent_id, child_id).await?;
        assert_eq!(link.relationship_type, "FEEDER");
        assert_eq!(link.relationship_selector, "feeder:us");
        assert_eq!(link.status, "ACTIVE");
        assert_eq!(link.capital_flow.as_deref(), Some("UPSTREAM"));

        db.cleanup().await?;
        Ok(())
    }

    #[tokio::test]
    async fn test_cbu_link_structure_rejects_self_link() -> Result<()> {
        let db = TestDb::new().await?;
        let name = db.name("SelfLink");
        let dsl = format!(
            r#"
            (cbu.create :name "{}" :jurisdiction "LU" :as @cbu)
            (cbu.link-structure
              :parent-cbu-id @cbu
              :child-cbu-id @cbu
              :relationship-type "feeder"
              :relationship-selector "feeder:us")
        "#,
            name
        );

        let err = db
            .execute_dsl(&dsl)
            .await
            .expect_err("self-link should fail");
        assert!(err.to_string().contains("self"));

        db.cleanup().await?;
        Ok(())
    }

    #[tokio::test]
    async fn test_cbu_unlink_structure_terminates_link() -> Result<()> {
        let db = TestDb::new().await?;
        let parent_name = db.name("Parent");
        let child_name = db.name("Child");

        let setup_dsl = format!(
            r#"
            (cbu.create :name "{}" :jurisdiction "KY" :as @parent)
            (cbu.create :name "{}" :jurisdiction "IE" :as @child)
            (cbu.link-structure
              :parent-cbu-id @parent
              :child-cbu-id @child
              :relationship-type "feeder"
              :relationship-selector "feeder:ie")
        "#,
            parent_name, child_name
        );
        let ctx = db.execute_dsl(&setup_dsl).await?;
        let parent_id = ctx.resolve("parent").expect("parent should be bound");
        let child_id = ctx.resolve("child").expect("child should be bound");
        let link = read_structure_link(&db.pool, parent_id, child_id).await?;

        let unlink_dsl = format!(
            r#"(cbu.unlink-structure :link-id "{}" :reason "fund wound down")"#,
            link.link_id
        );
        let unlink_ctx = db.execute_dsl(&unlink_dsl).await?;
        assert!(unlink_ctx.effective_symbols().is_empty());

        let terminated = read_structure_link(&db.pool, parent_id, child_id).await?;
        assert_eq!(terminated.status, "TERMINATED");
        assert_eq!(
            terminated.terminated_reason.as_deref(),
            Some("fund wound down")
        );

        db.cleanup().await?;
        Ok(())
    }

    #[tokio::test]
    async fn test_cross_border_constellation_hydrates_child_cbus_from_links() -> Result<()> {
        use ob_poc::sem_os_runtime::constellation_runtime::{
            hydrate_constellation, load_constellation_map,
        };

        let db = TestDb::new().await?;
        let master_name = db.name("CrossBorderMaster");
        let us_name = db.name("CrossBorderUsFeeder");
        let ie_name = db.name("CrossBorderIeFeeder");

        let dsl = format!(
            r#"
            (cbu.create :name "{}" :jurisdiction "KY" :as @master)
            (cbu.create :name "{}" :jurisdiction "US" :as @us)
            (cbu.create :name "{}" :jurisdiction "IE" :as @ie)
            (cbu.link-structure
              :parent-cbu-id @master
              :child-cbu-id @us
              :relationship-type "feeder"
              :relationship-selector "feeder:us")
            (cbu.link-structure
              :parent-cbu-id @master
              :child-cbu-id @ie
              :relationship-type "feeder"
              :relationship-selector "feeder:ie")
        "#,
            master_name, us_name, ie_name
        );
        let ctx = db.execute_dsl(&dsl).await?;
        let master_id = ctx.resolve("master").expect("master should be bound");
        let us_id = ctx.resolve("us").expect("us feeder should be bound");

        let yaml = r#"
constellation: test.cross-border
jurisdiction: XB
slots:
  cbu:
    type: cbu
    table: cbus
    pk: cbu_id
    cardinality: root
    verbs: { read: cbu.read }
    children:
      us_feeder:
        type: cbu
        join:
          {
            via: cbu_structure_links,
            parent_fk: parent_cbu_id,
            child_fk: child_cbu_id,
            filter_column: relationship_selector,
            filter_value: feeder:us,
          }
        cardinality: optional
        depends_on: [cbu]
        verbs: { show: cbu.read }
      ie_feeder:
        type: cbu
        join:
          {
            via: cbu_structure_links,
            parent_fk: parent_cbu_id,
            child_fk: child_cbu_id,
            filter_column: relationship_selector,
            filter_value: feeder:ie,
          }
        cardinality: optional
        depends_on: [cbu]
        verbs: { show: cbu.read }
"#;
        let map = load_constellation_map(yaml).unwrap();
        let hydrated = hydrate_constellation(&db.pool, master_id, None, &map).await?;
        let root = hydrated
            .slots
            .iter()
            .find(|slot| slot.name == "cbu")
            .expect("root slot present");
        let us_feeder = root
            .children
            .iter()
            .find(|slot| slot.name == "cbu.us_feeder")
            .expect("us feeder slot present");
        assert_eq!(us_feeder.effective_state, "filled");
        assert_eq!(us_feeder.record_id, Some(us_id));

        let feeder_view = hydrate_constellation(&db.pool, us_id, None, &map).await?;
        let feeder_root = feeder_view
            .slots
            .iter()
            .find(|slot| slot.name == "cbu")
            .expect("feeder root slot present");
        let feeder_child = feeder_root
            .children
            .iter()
            .find(|slot| slot.name == "cbu.us_feeder")
            .expect("feeder child slot present");
        assert_eq!(feeder_child.effective_state, "empty");

        db.cleanup().await?;
        Ok(())
    }

    #[tokio::test]
    async fn test_entity_type_mapping() -> Result<()> {
        let db = TestDb::new().await?;

        let dsl = format!(
            r#"
            (cbu.create :name "{}" :as @cbu)
            (entity.create :entity-type "limited-company" :cbu-id @cbu :name "{}" :as @company)
            (entity.create :entity-type "proper-person" :cbu-id @cbu :first-name "John" :last-name "Doe" :as @person)
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
            (entity.create :entity-type "limited-company" :cbu-id @cbu :name "{}" :as @company)
            (entity.create :entity-type "proper-person" :cbu-id @cbu :first-name "Jane" :last-name "Doe" :as @ubo)
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
            (entity.create :entity-type "proper-person" :cbu-id @nonexistent :first-name "Test" :last-name "User")
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
            (entity.create :entity-type "limited-company" :cbu-id @cbu :name "{}" :as @company)
            (entity.create :entity-type "proper-person" :cbu-id @cbu :first-name "Test" :last-name "Person" :as @person)
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
            (entity.create :entity-type "limited-company" :cbu-id @cbu :name "{}東京支店" :as @company)
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
        let ubo1_first_name = db.name("Alice");
        let ubo2_first_name = db.name("Bob");

        // Full corporate onboarding with KYC case model:
        // 1. Create CBU and entities
        // 2. Assign roles
        // 3. Catalog documents
        // 4. Create KYC case
        // 5. Create entity workstreams for UBOs
        // 6. Run screenings via screening.run
        let dsl = format!(
            r#"
            (cbu.create :name "{}" :client-type "corporate" :jurisdiction "GB" :as @cbu)
            (entity.create :entity-type "limited-company" :cbu-id @cbu :name "{}" :as @company)
            (entity.create :entity-type "proper-person" :cbu-id @cbu :first-name "{}" :last-name "UBO1" :as @ubo1)
            (entity.create :entity-type "proper-person" :cbu-id @cbu :first-name "{}" :last-name "UBO2" :as @ubo2)
            (cbu.assign-role :cbu-id @cbu :entity-id @ubo1 :role "BENEFICIAL_OWNER")
            (cbu.assign-role :cbu-id @cbu :entity-id @ubo2 :role "BENEFICIAL_OWNER")
            (document.catalog :cbu-id @cbu :doc-type "CERTIFICATE_OF_INCORPORATION" :title "Company Certificate")
            (document.catalog :cbu-id @cbu :doc-type "PASSPORT" :title "Alice Passport")
            (document.catalog :cbu-id @cbu :doc-type "PASSPORT" :title "Bob Passport")
            (kyc-case.create :cbu-id @cbu :case-type "NEW_CLIENT" :as @case)
            (entity-workstream.create :case-id @case :entity-id @ubo1 :as @ws1)
            (entity-workstream.create :case-id @case :entity-id @ubo2 :as @ws2)
        "#,
            db.name("FullCBU"),
            db.name("FullCompany"),
            ubo1_first_name,
            ubo2_first_name
        );

        let ctx = db.execute_dsl(&dsl).await?;

        let cbu_id = ctx.resolve("cbu").unwrap();
        let case_id = ctx.resolve("case").unwrap();
        let ubo1_id = ctx.resolve("ubo1").unwrap();
        let ubo2_id = ctx.resolve("ubo2").unwrap();
        let ws1_id = get_workstream_id(&db.pool, case_id, ubo1_id).await?;
        let ws2_id = get_workstream_id(&db.pool, case_id, ubo2_id).await?;

        let screening_dsl = format!(
            r#"
            (screening.run :workstream-id "{}" :screening-type "PEP")
            (screening.run :workstream-id "{}" :screening-type "SANCTIONS")
            (screening.run :workstream-id "{}" :screening-type "PEP")
            (screening.run :workstream-id "{}" :screening-type "SANCTIONS")
        "#,
            ws1_id, ws1_id, ws2_id, ws2_id
        );
        db.execute_dsl(&screening_dsl).await?;

        // Verify counts
        assert!(
            count_roles(&db.pool, cbu_id).await? >= 1,
            "Should have at least 1 role"
        );
        assert_eq!(
            count_documents(&db.pool, cbu_id).await?,
            3,
            "Should have 3 documents"
        );
        assert!(
            count_screenings(&db.pool, ubo1_id).await? >= 2,
            "UBO1 should have at least 2 screenings"
        );
        assert!(
            count_screenings(&db.pool, ubo2_id).await? >= 2,
            "UBO2 should have at least 2 screenings"
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
            r#"SELECT case_id, status, closed_at FROM "ob-poc".cases WHERE case_id = $1"#,
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
        let ubo_first_name = db.name("Decision");
        let dsl = format!(
            r#"
            (cbu.create :name "{}" :client-type "corporate" :jurisdiction "GB" :as @cbu)
            (entity.create :entity-type "proper-person" :cbu-id @cbu :first-name "{}" :last-name "UBO" :as @ubo)
            (cbu.assign-role :cbu-id @cbu :entity-id @ubo :role "BENEFICIAL_OWNER")
            (kyc-case.create :cbu-id @cbu :case-type "NEW_CLIENT" :as @case)
            (entity-workstream.create :case-id @case :entity-id @ubo :as @ws)
            (entity-workstream.update-status :workstream-id @ws :status "COLLECT")
            (entity-workstream.update-status :workstream-id @ws :status "VERIFY")
            (entity-workstream.update-status :workstream-id @ws :status "SCREEN")
            (screening.run :workstream-id @ws :screening-type "SANCTIONS" :as @screening)
            (screening.complete :screening-id @screening :status "CLEAR" :result-summary "No matches found")
            (entity-workstream.update-status :workstream-id @ws :status "ASSESS")
            (entity-workstream.complete :workstream-id @ws)
            "#,
            db.name("DecisionCBU"),
            ubo_first_name
        );

        let ctx = db.execute_dsl(&dsl).await?;
        let cbu_id = ctx.resolve("cbu").expect("cbu should be bound");
        let case_id = ctx.resolve("case").expect("case should be bound");

        // Update case to REVIEW status (ready for decision)
        sqlx::query(r#"UPDATE "ob-poc".cases SET status = 'REVIEW' WHERE case_id = $1"#)
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
            r#"UPDATE "ob-poc".cases
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
            r#"UPDATE "ob-poc".cases SET status = 'APPROVED', closed_at = NOW() WHERE case_id = $1"#,
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
            r#"UPDATE "ob-poc".cases SET escalation_level = 'SENIOR_COMPLIANCE' WHERE case_id = $1"#,
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
