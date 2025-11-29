# DSL Database Integration Tests (Part 2 of 2)

**Goal**: Comprehensive tests that write to DB, read back, and verify correctness.

**Related**: See `DSL_DATABASE_EXECUTOR.md` for the executor implementation.

**Critical**: Database integration has historically been a major source of bugs. These tests prioritize:
- Round-trip verification (write → read → compare)
- Foreign key constraint validation
- Type mapping correctness
- Edge cases and boundary conditions

---

## Test Infrastructure

Create file: `rust/tests/db_integration.rs`

```rust
//! Database integration tests for DSL executor

mod db_tests {
    use anyhow::Result;
    use sqlx::PgPool;
    use uuid::Uuid;
    use ob_poc::dsl_v2::{
        parse_program, compile, create_pool, DbConfig, DslExecutor, ExecutionContext,
    };

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
                .unwrap_or_else(|_| "postgresql://localhost/ob-poc-test".into());
            let config = DbConfig::from_url(&url);
            let pool = create_pool(&config).await?;
            let prefix = format!("test_{}", &Uuid::new_v4().to_string()[..8]);
            Ok(Self { pool, prefix })
        }

        fn name(&self, base: &str) -> String {
            format!("{}_{}", self.prefix, base)
        }

        async fn cleanup(&self) -> Result<()> {
            let pattern = format!("{}%", self.prefix);
            
            // Delete in reverse dependency order
            sqlx::query!(r#"DELETE FROM "ob-poc".investigations WHERE cbu_id IN 
                          (SELECT cbu_id FROM "ob-poc".cbus WHERE name LIKE $1)"#, pattern)
                .execute(&self.pool).await?;
            sqlx::query!(r#"DELETE FROM "ob-poc".screenings WHERE entity_id IN 
                          (SELECT entity_id FROM "ob-poc".entities WHERE name LIKE $1)"#, pattern)
                .execute(&self.pool).await?;
            sqlx::query!(r#"DELETE FROM "ob-poc".documents WHERE cbu_id IN 
                          (SELECT cbu_id FROM "ob-poc".cbus WHERE name LIKE $1)"#, pattern)
                .execute(&self.pool).await?;
            sqlx::query!(r#"DELETE FROM "ob-poc".entity_roles WHERE cbu_id IN 
                          (SELECT cbu_id FROM "ob-poc".cbus WHERE name LIKE $1)"#, pattern)
                .execute(&self.pool).await?;
            sqlx::query!(r#"DELETE FROM "ob-poc".entities WHERE name LIKE $1"#, pattern)
                .execute(&self.pool).await?;
            sqlx::query!(r#"DELETE FROM "ob-poc".cbus WHERE name LIKE $1"#, pattern)
                .execute(&self.pool).await?;
            Ok(())
        }

        async fn execute_dsl(&self, dsl: &str) -> Result<ExecutionContext> {
            let ast = parse_program(dsl).map_err(|e| anyhow::anyhow!("{:?}", e))?;
            let plan = compile(&ast).map_err(|e| anyhow::anyhow!("{:?}", e))?;
            let executor = DslExecutor::new(self.pool.clone());
            let mut ctx = ExecutionContext::new();
            executor.execute(&plan, &mut ctx).await?;
            Ok(ctx)
        }
    }

    // Helper: Read CBU back from DB
    async fn read_cbu(pool: &PgPool, id: Uuid) -> Result<CbuRow> {
        let row = sqlx::query_as!(CbuRow,
            r#"SELECT cbu_id, name, jurisdiction, client_type, status 
               FROM "ob-poc".cbus WHERE cbu_id = $1"#, id)
            .fetch_one(pool).await?;
        Ok(row)
    }

    // Helper: Read entity back from DB
    async fn read_entity(pool: &PgPool, id: Uuid) -> Result<EntityRow> {
        let row = sqlx::query_as!(EntityRow,
            r#"SELECT e.entity_id, e.cbu_id, e.name, e.status, et.type_code as entity_type
               FROM "ob-poc".entities e
               JOIN "ob-poc".entity_types et ON e.entity_type_id = et.entity_type_id
               WHERE e.entity_id = $1"#, id)
            .fetch_one(pool).await?;
        Ok(row)
    }

    // Helper: Count roles for a CBU
    async fn count_roles(pool: &PgPool, cbu_id: Uuid) -> Result<i64> {
        let count: i64 = sqlx::query_scalar!(
            r#"SELECT COUNT(*) FROM "ob-poc".entity_roles WHERE cbu_id = $1"#, cbu_id)
            .fetch_one(pool).await?.unwrap_or(0);
        Ok(count)
    }

    // Helper: Count documents for a CBU
    async fn count_documents(pool: &PgPool, cbu_id: Uuid) -> Result<i64> {
        let count: i64 = sqlx::query_scalar!(
            r#"SELECT COUNT(*) FROM "ob-poc".documents WHERE cbu_id = $1"#, cbu_id)
            .fetch_one(pool).await?.unwrap_or(0);
        Ok(count)
    }

    // Helper: Count screenings for an entity
    async fn count_screenings(pool: &PgPool, entity_id: Uuid) -> Result<i64> {
        let count: i64 = sqlx::query_scalar!(
            r#"SELECT COUNT(*) FROM "ob-poc".screenings WHERE entity_id = $1"#, entity_id)
            .fetch_one(pool).await?.unwrap_or(0);
        Ok(count)
    }

    #[derive(Debug)]
    struct CbuRow {
        cbu_id: Uuid,
        name: String,
        jurisdiction: Option<String>,
        client_type: Option<String>,
        status: Option<String>,
    }

    #[derive(Debug)]
    struct EntityRow {
        entity_id: Uuid,
        cbu_id: Uuid,
        name: String,
        entity_type: String,
        status: Option<String>,
    }

    // =========================================================================
    // ROUND-TRIP TESTS
    // =========================================================================

    #[tokio::test]
    async fn test_cbu_round_trip() -> Result<()> {
        let db = TestDb::new().await?;
        let name = db.name("RoundTripCBU");

        let dsl = format!(r#"
            (cbu.create :name "{}" :client-type "corporate" :jurisdiction "GB" :as @cbu)
        "#, name);

        let ctx = db.execute_dsl(&dsl).await?;

        // Read back and verify
        let cbu_id = ctx.resolve("cbu").expect("cbu should be bound");
        let record = read_cbu(&db.pool, cbu_id).await?;

        assert_eq!(record.name, name);
        assert_eq!(record.jurisdiction.as_deref(), Some("GB"));
        assert_eq!(record.client_type.as_deref(), Some("corporate"));
        assert_eq!(record.status.as_deref(), Some("active"));

        db.cleanup().await?;
        Ok(())
    }

    #[tokio::test]
    async fn test_entity_type_mapping() -> Result<()> {
        let db = TestDb::new().await?;

        let dsl = format!(r#"
            (cbu.create :name "{}" :as @cbu)
            (entity.create-limited-company :cbu-id @cbu :name "{}" :as @company)
            (entity.create-proper-person :cbu-id @cbu :name "{}" :as @person)
            (entity.create-trust :cbu-id @cbu :name "{}" :as @trust)
        "#, db.name("TypeMapCBU"), db.name("Company"), db.name("Person"), db.name("Trust"));

        let ctx = db.execute_dsl(&dsl).await?;

        // Verify entity types mapped correctly
        let company = read_entity(&db.pool, ctx.resolve("company").unwrap()).await?;
        let person = read_entity(&db.pool, ctx.resolve("person").unwrap()).await?;
        let trust = read_entity(&db.pool, ctx.resolve("trust").unwrap()).await?;

        assert_eq!(company.entity_type, "LIMITED_COMPANY");
        assert_eq!(person.entity_type, "PROPER_PERSON");
        assert_eq!(trust.entity_type, "TRUST");

        db.cleanup().await?;
        Ok(())
    }

    #[tokio::test]
    async fn test_role_assignment_fk() -> Result<()> {
        let db = TestDb::new().await?;

        let dsl = format!(r#"
            (cbu.create :name "{}" :as @cbu)
            (entity.create-limited-company :cbu-id @cbu :name "{}" :as @company)
            (entity.create-proper-person :cbu-id @cbu :name "{}" :as @ubo)
            (cbu.assign-role :cbu-id @cbu :entity-id @ubo :target-entity-id @company 
                             :role "BENEFICIAL_OWNER" :ownership-percentage 75.0)
        "#, db.name("FKTestCBU"), db.name("FKCompany"), db.name("FKUBO"));

        let ctx = db.execute_dsl(&dsl).await?;

        // Verify role was created with correct FKs and data
        let cbu_id = ctx.resolve("cbu").unwrap();
        let company_id = ctx.resolve("company").unwrap();
        let ubo_id = ctx.resolve("ubo").unwrap();

        let role = sqlx::query!(
            r#"SELECT er.entity_id, er.target_entity_id, er.ownership_percentage, r.role_code
               FROM "ob-poc".entity_roles er
               JOIN "ob-poc".roles r ON er.role_id = r.role_id
               WHERE er.cbu_id = $1"#, cbu_id)
            .fetch_one(&db.pool).await?;

        assert_eq!(role.entity_id, ubo_id);
        assert_eq!(role.target_entity_id, company_id);
        assert_eq!(role.role_code, "BENEFICIAL_OWNER");
        assert_eq!(role.ownership_percentage, Some(75.0));

        db.cleanup().await?;
        Ok(())
    }

    #[tokio::test]
    async fn test_document_type_resolution() -> Result<()> {
        let db = TestDb::new().await?;

        let dsl = format!(r#"
            (cbu.create :name "{}" :as @cbu)
            (entity.create-proper-person :cbu-id @cbu :name "{}" :as @person)
            (document.catalog :cbu-id @cbu :entity-id @person :document-type "PASSPORT" :as @passport)
        "#, db.name("DocTestCBU"), db.name("DocPerson"));

        let ctx = db.execute_dsl(&dsl).await?;

        let doc_id = ctx.resolve("passport").unwrap();
        let doc = sqlx::query!(
            r#"SELECT d.document_id, dt.type_code, d.status
               FROM "ob-poc".documents d
               JOIN "ob-poc".document_types dt ON d.document_type_id = dt.type_id
               WHERE d.document_id = $1"#, doc_id)
            .fetch_one(&db.pool).await?;

        assert_eq!(doc.type_code, "PASSPORT");
        assert_eq!(doc.status.as_deref(), Some("pending"));

        db.cleanup().await?;
        Ok(())
    }

    #[tokio::test]
    async fn test_screening_all_types() -> Result<()> {
        let db = TestDb::new().await?;

        let dsl = format!(r#"
            (cbu.create :name "{}" :as @cbu)
            (entity.create-proper-person :cbu-id @cbu :name "{}" :as @person)
            (screening.pep :entity-id @person)
            (screening.sanctions :entity-id @person)
            (screening.adverse-media :entity-id @person :lookback-months 24)
        "#, db.name("ScreenCBU"), db.name("ScreenPerson"));

        let ctx = db.execute_dsl(&dsl).await?;
        let person_id = ctx.resolve("person").unwrap();

        let screenings = sqlx::query!(
            r#"SELECT screening_type, metadata FROM "ob-poc".screenings WHERE entity_id = $1"#, person_id)
            .fetch_all(&db.pool).await?;

        assert_eq!(screenings.len(), 3);
        
        let types: Vec<_> = screenings.iter().filter_map(|s| s.screening_type.as_deref()).collect();
        assert!(types.contains(&"PEP"));
        assert!(types.contains(&"SANCTIONS"));
        assert!(types.contains(&"ADVERSE_MEDIA"));

        // Verify adverse media has lookback metadata
        let adverse = screenings.iter().find(|s| s.screening_type.as_deref() == Some("ADVERSE_MEDIA")).unwrap();
        let metadata: serde_json::Value = adverse.metadata.clone().unwrap_or_default();
        assert_eq!(metadata["lookback_months"], 24);

        db.cleanup().await?;
        Ok(())
    }

    #[tokio::test]
    async fn test_ubo_calculation_threshold() -> Result<()> {
        let db = TestDb::new().await?;

        let dsl = format!(r#"
            (cbu.create :name "{}" :as @cbu)
            (entity.create-limited-company :cbu-id @cbu :name "{}" :as @company)
            (entity.create-proper-person :cbu-id @cbu :name "{}_Alice" :as @alice)
            (entity.create-proper-person :cbu-id @cbu :name "{}_Bob" :as @bob)
            (entity.create-proper-person :cbu-id @cbu :name "{}_Charlie" :as @charlie)
            (cbu.assign-role :cbu-id @cbu :entity-id @alice :target-entity-id @company :role "BENEFICIAL_OWNER" :ownership-percentage 45.0)
            (cbu.assign-role :cbu-id @cbu :entity-id @bob :target-entity-id @company :role "BENEFICIAL_OWNER" :ownership-percentage 35.0)
            (cbu.assign-role :cbu-id @cbu :entity-id @charlie :target-entity-id @company :role "BENEFICIAL_OWNER" :ownership-percentage 20.0)
            (ubo.calculate :cbu-id @cbu :entity-id @company :threshold 25.0)
        "#, db.name("UBOCalcCBU"), db.name("UBOCompany"), 
            db.prefix, db.prefix, db.prefix);

        let ctx = db.execute_dsl(&dsl).await?;

        // Get UBO calculation result
        let results = ctx.results();
        let ubo_result = results.last().unwrap();
        let data = ubo_result.data.as_ref().unwrap();
        let ubos = data["ubos"].as_array().unwrap();

        // Should have 2 UBOs (Alice 45%, Bob 35%), not Charlie (20%)
        assert_eq!(ubos.len(), 2);
        assert_eq!(data["ubo_count"], 2);

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
            (entity.create-proper-person :cbu-id @nonexistent :name "Test")
        "#;

        let result = db.execute_dsl(dsl).await;
        
        assert!(result.is_err());
        assert!(result.unwrap_err().to_string().contains("Unresolved symbol"));

        Ok(())
    }

    #[tokio::test]
    async fn test_fk_invalid_role() -> Result<()> {
        let db = TestDb::new().await?;

        let dsl = format!(r#"
            (cbu.create :name "{}" :as @cbu)
            (entity.create-limited-company :cbu-id @cbu :name "{}" :as @company)
            (entity.create-proper-person :cbu-id @cbu :name "{}" :as @person)
            (cbu.assign-role :cbu-id @cbu :entity-id @person :target-entity-id @company :role "INVALID_ROLE_XYZ")
        "#, db.name("InvalidRoleCBU"), db.name("InvalidRoleCompany"), db.name("InvalidRolePerson"));

        let result = db.execute_dsl(&dsl).await;

        assert!(result.is_err());
        assert!(result.unwrap_err().to_string().contains("Unknown role"));

        db.cleanup().await?;
        Ok(())
    }

    #[tokio::test]
    async fn test_fk_invalid_document_type() -> Result<()> {
        let db = TestDb::new().await?;

        let dsl = format!(r#"
            (cbu.create :name "{}" :as @cbu)
            (entity.create-proper-person :cbu-id @cbu :name "{}" :as @person)
            (document.catalog :cbu-id @cbu :entity-id @person :document-type "INVALID_DOC_XYZ")
        "#, db.name("InvalidDocCBU"), db.name("InvalidDocPerson"));

        let result = db.execute_dsl(&dsl).await;

        assert!(result.is_err());
        assert!(result.unwrap_err().to_string().contains("Unknown document type"));

        db.cleanup().await?;
        Ok(())
    }

    #[tokio::test]
    async fn test_fk_invalid_entity_type() -> Result<()> {
        let db = TestDb::new().await?;

        let dsl = format!(r#"
            (cbu.create :name "{}" :as @cbu)
            (entity.create-invalid-type-xyz :cbu-id @cbu :name "{}")
        "#, db.name("InvalidTypeCBU"), db.name("InvalidTypeEntity"));

        let result = db.execute_dsl(&dsl).await;

        assert!(result.is_err());
        assert!(result.unwrap_err().to_string().contains("Unknown entity type"));

        db.cleanup().await?;
        Ok(())
    }

    // =========================================================================
    // EDGE CASE TESTS
    // =========================================================================

    #[tokio::test]
    async fn test_unicode_names() -> Result<()> {
        let db = TestDb::new().await?;

        let dsl = format!(r#"
            (cbu.create :name "{}株式会社" :jurisdiction "JP" :as @cbu)
            (entity.create-limited-company :cbu-id @cbu :name "{}東京支店" :as @company)
        "#, db.name("Unicode"), db.name("Unicode"));

        let ctx = db.execute_dsl(&dsl).await?;

        let cbu = read_cbu(&db.pool, ctx.resolve("cbu").unwrap()).await?;
        assert!(cbu.name.contains("株式会社"));

        db.cleanup().await?;
        Ok(())
    }

    #[tokio::test]
    async fn test_special_characters() -> Result<()> {
        let db = TestDb::new().await?;

        // Test SQL injection prevention and special char handling
        let dsl = format!(r#"
            (cbu.create :name "{} O'Brien & Co." :as @cbu)
        "#, db.name("SpecialChar"));

        let ctx = db.execute_dsl(&dsl).await?;

        let cbu = read_cbu(&db.pool, ctx.resolve("cbu").unwrap()).await?;
        assert!(cbu.name.contains("O'Brien"));

        db.cleanup().await?;
        Ok(())
    }

    #[tokio::test]
    async fn test_null_vs_empty_optional_fields() -> Result<()> {
        let db = TestDb::new().await?;

        // Omit optional jurisdiction - should be NULL not empty string
        let dsl = format!(r#"
            (cbu.create :name "{}" :client-type "individual" :as @cbu)
        "#, db.name("NullTest"));

        let ctx = db.execute_dsl(&dsl).await?;
        let cbu = read_cbu(&db.pool, ctx.resolve("cbu").unwrap()).await?;

        assert!(cbu.jurisdiction.is_none());

        db.cleanup().await?;
        Ok(())
    }

    #[tokio::test]
    async fn test_duplicate_bindings_overwrites() -> Result<()> {
        let db = TestDb::new().await?;

        let dsl = format!(r#"
            (cbu.create :name "{}_First" :as @item)
            (cbu.create :name "{}_Second" :as @item)
        "#, db.name("DupBind"), db.name("DupBind"));

        let ctx = db.execute_dsl(&dsl).await?;

        // @item should point to the second CBU
        let item_id = ctx.resolve("item").unwrap();
        let cbu = read_cbu(&db.pool, item_id).await?;
        assert!(cbu.name.contains("Second"));

        db.cleanup().await?;
        Ok(())
    }

    #[tokio::test]
    async fn test_float_precision() -> Result<()> {
        let db = TestDb::new().await?;

        let dsl = format!(r#"
            (cbu.create :name "{}" :as @cbu)
            (entity.create-limited-company :cbu-id @cbu :name "{}" :as @company)
            (entity.create-proper-person :cbu-id @cbu :name "{}" :as @person)
            (cbu.assign-role :cbu-id @cbu :entity-id @person :target-entity-id @company 
                             :role "BENEFICIAL_OWNER" :ownership-percentage 33.333333)
        "#, db.name("FloatCBU"), db.name("FloatCompany"), db.name("FloatPerson"));

        let ctx = db.execute_dsl(&dsl).await?;
        let cbu_id = ctx.resolve("cbu").unwrap();

        let role = sqlx::query!(
            r#"SELECT ownership_percentage FROM "ob-poc".entity_roles WHERE cbu_id = $1"#, cbu_id)
            .fetch_one(&db.pool).await?;

        // Should preserve reasonable precision
        let pct = role.ownership_percentage.unwrap();
        assert!((pct - 33.333333).abs() < 0.0001);

        db.cleanup().await?;
        Ok(())
    }

    // =========================================================================
    // FULL SCENARIO TESTS
    // =========================================================================

    #[tokio::test]
    async fn test_full_corporate_onboarding() -> Result<()> {
        let db = TestDb::new().await?;

        let dsl = format!(r#"
            (cbu.create :name "{}" :client-type "corporate" :jurisdiction "GB" :as @cbu)
            (entity.create-limited-company :cbu-id @cbu :name "{}" :as @company)
            (entity.create-proper-person :cbu-id @cbu :name "{}_UBO1" :as @ubo1)
            (entity.create-proper-person :cbu-id @cbu :name "{}_UBO2" :as @ubo2)
            (cbu.assign-role :cbu-id @cbu :entity-id @ubo1 :target-entity-id @company :role "BENEFICIAL_OWNER" :ownership-percentage 60.0)
            (cbu.assign-role :cbu-id @cbu :entity-id @ubo2 :target-entity-id @company :role "BENEFICIAL_OWNER" :ownership-percentage 40.0)
            (document.catalog :cbu-id @cbu :entity-id @company :document-type "CERTIFICATE_OF_INCORPORATION")
            (document.catalog :cbu-id @cbu :entity-id @ubo1 :document-type "PASSPORT")
            (document.catalog :cbu-id @cbu :entity-id @ubo2 :document-type "PASSPORT")
            (screening.pep :entity-id @ubo1)
            (screening.pep :entity-id @ubo2)
            (screening.sanctions :entity-id @ubo1)
            (screening.sanctions :entity-id @ubo2)
            (ubo.calculate :cbu-id @cbu :entity-id @company :threshold 25.0)
        "#, db.name("FullCBU"), db.name("FullCompany"), db.prefix, db.prefix);

        let ctx = db.execute_dsl(&dsl).await?;

        let cbu_id = ctx.resolve("cbu").unwrap();
        let ubo1_id = ctx.resolve("ubo1").unwrap();
        let ubo2_id = ctx.resolve("ubo2").unwrap();

        // Verify counts
        assert_eq!(count_roles(&db.pool, cbu_id).await?, 2);
        assert_eq!(count_documents(&db.pool, cbu_id).await?, 3);
        assert_eq!(count_screenings(&db.pool, ubo1_id).await?, 2);
        assert_eq!(count_screenings(&db.pool, ubo2_id).await?, 2);

        // Verify UBO calculation found both
        let ubo_result = ctx.results().last().unwrap();
        let data = ubo_result.data.as_ref().unwrap();
        assert_eq!(data["ubo_count"], 2);

        db.cleanup().await?;
        Ok(())
    }

    #[tokio::test]
    async fn test_kyc_workflow() -> Result<()> {
        let db = TestDb::new().await?;

        let dsl = format!(r#"
            (cbu.create :name "{}" :as @cbu)
            (entity.create-proper-person :cbu-id @cbu :name "{}" :as @person)
            (document.catalog :cbu-id @cbu :entity-id @person :document-type "PASSPORT" :as @passport)
            (document.extract :document-id @passport)
            (screening.pep :entity-id @person)
            (screening.sanctions :entity-id @person)
            (kyc.initiate :cbu-id @cbu :investigation-type "NEW_CLIENT" :as @investigation)
        "#, db.name("KYCCBU"), db.name("KYCPerson"));

        let ctx = db.execute_dsl(&dsl).await?;

        let investigation_id = ctx.resolve("investigation").unwrap();

        let inv = sqlx::query!(
            r#"SELECT investigation_id, status, investigation_type 
               FROM "ob-poc".investigations WHERE investigation_id = $1"#, investigation_id)
            .fetch_one(&db.pool).await?;

        assert_eq!(inv.status.as_deref(), Some("open"));
        assert_eq!(inv.investigation_type.as_deref(), Some("NEW_CLIENT"));

        db.cleanup().await?;
        Ok(())
    }
}
```

---

## CLI Test Script

Create file: `rust/tests/db_cli_test.sh`

```bash
#!/bin/bash
# CLI database integration tests

set -e

CLI="cargo run --features cli,database --bin dsl_cli --"
DB_URL="${TEST_DATABASE_URL:-postgresql://localhost/ob-poc-test}"

echo "═══════════════════════════════════════════════════════════"
echo "         DSL CLI Database Integration Tests                 "
echo "═══════════════════════════════════════════════════════════"
echo "Database: $DB_URL"
echo ""

PASS=0
FAIL=0
PREFIX="clitest_$(date +%s)"

cleanup() {
    echo "Cleaning up test data..."
    psql "$DB_URL" -c "DELETE FROM \"ob-poc\".investigations WHERE cbu_id IN (SELECT cbu_id FROM \"ob-poc\".cbus WHERE name LIKE '${PREFIX}%');" 2>/dev/null || true
    psql "$DB_URL" -c "DELETE FROM \"ob-poc\".screenings WHERE entity_id IN (SELECT entity_id FROM \"ob-poc\".entities WHERE name LIKE '${PREFIX}%');" 2>/dev/null || true
    psql "$DB_URL" -c "DELETE FROM \"ob-poc\".documents WHERE cbu_id IN (SELECT cbu_id FROM \"ob-poc\".cbus WHERE name LIKE '${PREFIX}%');" 2>/dev/null || true
    psql "$DB_URL" -c "DELETE FROM \"ob-poc\".entity_roles WHERE cbu_id IN (SELECT cbu_id FROM \"ob-poc\".cbus WHERE name LIKE '${PREFIX}%');" 2>/dev/null || true
    psql "$DB_URL" -c "DELETE FROM \"ob-poc\".entities WHERE name LIKE '${PREFIX}%';" 2>/dev/null || true
    psql "$DB_URL" -c "DELETE FROM \"ob-poc\".cbus WHERE name LIKE '${PREFIX}%';" 2>/dev/null || true
}

trap cleanup EXIT

test_execute() {
    local name="$1"
    local dsl="$2"
    
    echo -n "  $name... "
    
    if echo "$dsl" | $CLI execute --db-url "$DB_URL" --format json > /tmp/result.json 2>&1; then
        if jq -e '.success == true' /tmp/result.json > /dev/null; then
            echo "PASS"
            PASS=$((PASS + 1))
            return 0
        fi
    fi
    
    echo "FAIL"
    cat /tmp/result.json
    FAIL=$((FAIL + 1))
    return 1
}

test_should_fail() {
    local name="$1"
    local dsl="$2"
    local expected="$3"
    
    echo -n "  $name (should fail)... "
    
    if echo "$dsl" | $CLI execute --db-url "$DB_URL" --format json > /tmp/result.json 2>&1; then
        echo "FAIL (expected failure but succeeded)"
        FAIL=$((FAIL + 1))
        return 1
    fi
    
    if grep -q "$expected" /tmp/result.json 2>/dev/null; then
        echo "PASS"
        PASS=$((PASS + 1))
        return 0
    fi
    
    echo "FAIL (wrong error)"
    cat /tmp/result.json
    FAIL=$((FAIL + 1))
    return 1
}

echo "--- Dry Run Test ---"
echo -n "  Dry run... "
if echo "(cbu.create :name \"dryrun\" :as @cbu)" | $CLI execute --db-url "$DB_URL" --dry-run 2>&1 | grep -q "steps would execute"; then
    echo "PASS"
    PASS=$((PASS + 1))
else
    echo "FAIL"
    FAIL=$((FAIL + 1))
fi

echo ""
echo "--- Execute Tests ---"

test_execute "CBU create" "(cbu.create :name \"${PREFIX}_CBU1\" :client-type \"corporate\" :jurisdiction \"GB\" :as @cbu)"

test_execute "Full scenario" "
(cbu.create :name \"${PREFIX}_FullCBU\" :client-type \"corporate\" :as @cbu)
(entity.create-limited-company :cbu-id @cbu :name \"${PREFIX}_Company\" :as @company)
(entity.create-proper-person :cbu-id @cbu :name \"${PREFIX}_UBO\" :as @ubo)
(cbu.assign-role :cbu-id @cbu :entity-id @ubo :target-entity-id @company :role \"BENEFICIAL_OWNER\" :ownership-percentage 100.0)
(document.catalog :cbu-id @cbu :entity-id @company :document-type \"CERTIFICATE_OF_INCORPORATION\")
(document.catalog :cbu-id @cbu :entity-id @ubo :document-type \"PASSPORT\")
(screening.pep :entity-id @ubo)
(screening.sanctions :entity-id @ubo)
(ubo.calculate :cbu-id @cbu :entity-id @company :threshold 25.0)
"

echo ""
echo "--- Error Tests ---"

test_should_fail "Invalid role" "
(cbu.create :name \"${PREFIX}_ErrRole\" :as @cbu)
(entity.create-limited-company :cbu-id @cbu :name \"${PREFIX}_ErrCompany\" :as @company)
(entity.create-proper-person :cbu-id @cbu :name \"${PREFIX}_ErrPerson\" :as @person)
(cbu.assign-role :cbu-id @cbu :entity-id @person :target-entity-id @company :role \"INVALID_ROLE\")
" "Unknown role"

test_should_fail "Invalid document type" "
(cbu.create :name \"${PREFIX}_ErrDoc\" :as @cbu)
(entity.create-proper-person :cbu-id @cbu :name \"${PREFIX}_ErrDocPerson\" :as @person)
(document.catalog :cbu-id @cbu :entity-id @person :document-type \"INVALID_DOC\")
" "Unknown document type"

test_should_fail "Undefined symbol" "
(entity.create-proper-person :cbu-id @nonexistent :name \"Test\")
" "Unresolved symbol"

echo ""
echo "═══════════════════════════════════════════════════════════"
echo "                       RESULTS                              "
echo "═══════════════════════════════════════════════════════════"
echo "  Passed: $PASS"
echo "  Failed: $FAIL"

if [ $FAIL -gt 0 ]; then
    exit 1
fi

echo ""
echo "All tests passed!"
```

---

## Execution Checklist

### Phase 1: Set Up Test Database
```bash
# Create test database (if not exists)
createdb ob-poc-test

# Run migrations on test database
psql ob-poc-test -f sql/migrations/...

# Set environment variable
export TEST_DATABASE_URL="postgresql://localhost/ob-poc-test"
```

### Phase 2: Run Rust Integration Tests
```bash
# Run all DB integration tests
cargo test --features database db_tests -- --test-threads=1

# Run specific test
cargo test --features database test_cbu_round_trip -- --nocapture
```

### Phase 3: Run CLI Tests
```bash
# Make script executable
chmod +x rust/tests/db_cli_test.sh

# Run CLI integration tests
./rust/tests/db_cli_test.sh
```

### Phase 4: Verify Data
```sql
-- Check test data was created and cleaned up
SELECT COUNT(*) FROM "ob-poc".cbus WHERE name LIKE 'test_%';
SELECT COUNT(*) FROM "ob-poc".cbus WHERE name LIKE 'clitest_%';
```

---

## Test Coverage Summary

| Category | Tests | Validates |
|----------|-------|-----------|
| Round-trip | 6 | Write → read → compare |
| FK constraints | 4 | Invalid role/doc/entity/symbol |
| Edge cases | 5 | Unicode, special chars, null, float |
| Full scenarios | 2 | Corporate onboarding, KYC workflow |
| CLI integration | 5 | Dry-run, execute, errors |

**Total: 22 tests**

---

## Common Failure Modes to Watch

1. **UUID serialization** - Ensure UUIDs round-trip correctly (not as strings when they should be UUIDs)
2. **NULL vs empty string** - Optional fields should be NULL, not ""
3. **Float precision** - ownership_percentage should preserve reasonable precision
4. **FK lookup failures** - Clear error messages when role/document_type/entity_type not found
5. **Transaction isolation** - Tests should not interfere with each other
6. **Cleanup failures** - Ensure test data is removed even if test fails
