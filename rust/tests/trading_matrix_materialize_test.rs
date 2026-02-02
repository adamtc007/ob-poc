//! Trading Matrix Materialize Integration Tests
//!
//! Tests the full lifecycle of trading profile materialization:
//! 1. Matrix creation via trading-profile verbs
//! 2. Materialize projection to operational tables
//! 3. Idempotency (re-materialize produces same state)
//! 4. Orphan cleanup (removed items deleted from ops tables)
//! 5. CSA duplicate prevention
//!
//! Run with: cargo test --features database --test trading_matrix_materialize_test

#[cfg(feature = "database")]
mod materialize_tests {
    use anyhow::Result;
    use ob_poc::dsl_v2::{compile, parse_program, DslExecutor, ExecutionContext};
    use serde_json::json;
    use sha2::{Digest, Sha256};
    use sqlx::PgPool;
    use uuid::Uuid;

    // =========================================================================
    // TEST INFRASTRUCTURE
    // =========================================================================

    struct TestDb {
        pool: PgPool,
        prefix: String,
        cbu_id: Option<Uuid>,
        profile_id: Option<Uuid>,
    }

    impl TestDb {
        async fn new() -> Result<Self> {
            let url = std::env::var("TEST_DATABASE_URL")
                .or_else(|_| std::env::var("DATABASE_URL"))
                .unwrap_or_else(|_| "postgresql:///data_designer".into());

            let pool = PgPool::connect(&url).await?;
            let prefix = format!("tmm_{}", &Uuid::now_v7().to_string()[..8]);
            Ok(Self {
                pool,
                prefix,
                cbu_id: None,
                profile_id: None,
            })
        }

        fn name(&self, base: &str) -> String {
            format!("{}_{}", self.prefix, base)
        }

        async fn execute_dsl(&self, dsl: &str) -> Result<ExecutionContext> {
            let ast = parse_program(dsl).map_err(|e| anyhow::anyhow!("{:?}", e))?;
            let plan = compile(&ast)?;
            let executor = DslExecutor::new(self.pool.clone());
            let mut ctx = ExecutionContext::new();
            executor.execute_plan(&plan, &mut ctx).await?;
            Ok(ctx)
        }

        async fn ensure_cbu(&mut self) -> Result<Uuid> {
            if let Some(id) = self.cbu_id {
                return Ok(id);
            }

            let name = self.name("test_cbu");
            let row = sqlx::query!(
                r#"INSERT INTO "ob-poc".cbus (name, jurisdiction, client_type)
                   VALUES ($1, 'LU', 'fund')
                   RETURNING cbu_id"#,
                name
            )
            .fetch_one(&self.pool)
            .await?;

            self.cbu_id = Some(row.cbu_id);
            Ok(row.cbu_id)
        }

        async fn create_profile_with_document(
            &mut self,
            document: serde_json::Value,
        ) -> Result<Uuid> {
            let cbu_id = self.ensure_cbu().await?;
            let profile_id = Uuid::now_v7();

            // Compute a simple hash for the document
            let doc_str = serde_json::to_string(&document)?;
            let doc_hash = format!("{:x}", Sha256::digest(doc_str.as_bytes()));

            sqlx::query(
                r#"INSERT INTO "ob-poc".cbu_trading_profiles
                   (profile_id, cbu_id, version, status, document, document_hash)
                   VALUES ($1, $2, 1, 'ACTIVE', $3, $4)"#,
            )
            .bind(profile_id)
            .bind(cbu_id)
            .bind(&document)
            .bind(&doc_hash)
            .execute(&self.pool)
            .await?;

            self.profile_id = Some(profile_id);
            Ok(profile_id)
        }

        async fn materialize(&self, force: bool) -> Result<()> {
            let profile_id = self.profile_id.unwrap();
            let dsl = format!(
                r#"(trading-profile.materialize :profile-id "{}" :force {} :sections ["ssis" "isda"])"#,
                profile_id, force
            );
            self.execute_dsl(&dsl).await?;
            Ok(())
        }

        async fn count_ssis(&self) -> Result<i64> {
            let cbu_id = self.cbu_id.unwrap();
            let count: (i64,) =
                sqlx::query_as("SELECT COUNT(*) FROM custody.cbu_ssi WHERE cbu_id = $1")
                    .bind(cbu_id)
                    .fetch_one(&self.pool)
                    .await?;
            Ok(count.0)
        }

        async fn count_isda(&self) -> Result<i64> {
            let cbu_id = self.cbu_id.unwrap();
            let count: (i64,) =
                sqlx::query_as("SELECT COUNT(*) FROM custody.isda_agreements WHERE cbu_id = $1")
                    .bind(cbu_id)
                    .fetch_one(&self.pool)
                    .await?;
            Ok(count.0)
        }

        async fn count_csa(&self) -> Result<i64> {
            let cbu_id = self.cbu_id.unwrap();
            let count: (i64,) = sqlx::query_as(
                r#"SELECT COUNT(*) FROM custody.csa_agreements ca
                   JOIN custody.isda_agreements ia ON ca.isda_id = ia.isda_id
                   WHERE ia.cbu_id = $1"#,
            )
            .bind(cbu_id)
            .fetch_one(&self.pool)
            .await?;
            Ok(count.0)
        }

        async fn ssi_exists(&self, ssi_name: &str) -> Result<bool> {
            let cbu_id = self.cbu_id.unwrap();
            let exists: (bool,) = sqlx::query_as(
                "SELECT EXISTS(SELECT 1 FROM custody.cbu_ssi WHERE cbu_id = $1 AND ssi_name = $2)",
            )
            .bind(cbu_id)
            .bind(ssi_name)
            .fetch_one(&self.pool)
            .await?;
            Ok(exists.0)
        }

        async fn cleanup(&self) -> Result<()> {
            let pattern = format!("{}%", self.prefix);

            if let Some(cbu_id) = self.cbu_id {
                // CSA agreements (via ISDA FK)
                sqlx::query(
                    r#"DELETE FROM custody.csa_agreements WHERE isda_id IN
                       (SELECT isda_id FROM custody.isda_agreements WHERE cbu_id = $1)"#,
                )
                .bind(cbu_id)
                .execute(&self.pool)
                .await
                .ok();

                // ISDA product coverage
                sqlx::query(
                    r#"DELETE FROM custody.isda_product_coverage WHERE isda_id IN
                       (SELECT isda_id FROM custody.isda_agreements WHERE cbu_id = $1)"#,
                )
                .bind(cbu_id)
                .execute(&self.pool)
                .await
                .ok();

                // ISDA agreements
                sqlx::query("DELETE FROM custody.isda_agreements WHERE cbu_id = $1")
                    .bind(cbu_id)
                    .execute(&self.pool)
                    .await
                    .ok();

                // Booking rules
                sqlx::query("DELETE FROM custody.ssi_booking_rules WHERE cbu_id = $1")
                    .bind(cbu_id)
                    .execute(&self.pool)
                    .await
                    .ok();

                // Universe
                sqlx::query("DELETE FROM custody.cbu_instrument_universe WHERE cbu_id = $1")
                    .bind(cbu_id)
                    .execute(&self.pool)
                    .await
                    .ok();

                // SSIs
                sqlx::query("DELETE FROM custody.cbu_ssi WHERE cbu_id = $1")
                    .bind(cbu_id)
                    .execute(&self.pool)
                    .await
                    .ok();

                // Trading profiles
                sqlx::query(r#"DELETE FROM "ob-poc".cbu_trading_profiles WHERE cbu_id = $1"#)
                    .bind(cbu_id)
                    .execute(&self.pool)
                    .await
                    .ok();

                // Materialization audit
                if let Some(profile_id) = self.profile_id {
                    sqlx::query(
                        r#"DELETE FROM "ob-poc".trading_profile_materializations WHERE profile_id = $1"#,
                    )
                    .bind(profile_id)
                    .execute(&self.pool)
                    .await
                    .ok();
                }
            }

            // Entities created for counterparties
            sqlx::query(r#"DELETE FROM "ob-poc".entities WHERE name LIKE $1"#)
                .bind(&pattern)
                .execute(&self.pool)
                .await
                .ok();

            // CBU
            sqlx::query(r#"DELETE FROM "ob-poc".cbus WHERE name LIKE $1"#)
                .bind(&pattern)
                .execute(&self.pool)
                .await
                .ok();

            Ok(())
        }
    }

    // Helper to build a basic trading profile document
    fn build_test_document(ssi_names: &[&str]) -> serde_json::Value {
        let ssis: Vec<serde_json::Value> = ssi_names
            .iter()
            .map(|name| {
                json!({
                    "name": name,
                    "custody_account": format!("ACC_{}", name),
                    "custody_bic": "IRVTUS3N",
                    "currency": "USD"
                })
            })
            .collect();

        json!({
            "universe": {
                "base_currency": "USD",
                "allowed_currencies": ["USD"],
                "allowed_markets": [],
                "instrument_classes": []
            },
            "investment_managers": [],
            "isda_agreements": [],
            "settlement_config": null,
            "booking_rules": [],
            "standing_instructions": {
                "CUSTODY": ssis
            },
            "pricing_matrix": [],
            "valuation_config": null,
            "constraints": null,
            "corporate_actions": null,
            "metadata": null
        })
    }

    // =========================================================================
    // TEST: SSI Idempotency - Re-materialize produces same state
    // =========================================================================

    #[tokio::test]
    async fn test_ssi_materialize_idempotency() -> Result<()> {
        let mut db = TestDb::new().await?;

        // Create profile with 2 SSIs
        let doc = build_test_document(&["SSI_A", "SSI_B"]);
        let _profile_id = db.create_profile_with_document(doc).await?;
        let cbu_id = db.cbu_id.unwrap();

        // First materialize
        db.materialize(false).await?;
        let count_1 = db.count_ssis().await?;
        assert_eq!(count_1, 2, "Should have 2 SSIs after first materialize");

        // Second materialize (idempotency test)
        db.materialize(false).await?;
        let count_2 = db.count_ssis().await?;
        assert_eq!(count_2, 2, "Should still have 2 SSIs after re-materialize");

        // Third materialize with force=true
        db.materialize(true).await?;
        let count_3 = db.count_ssis().await?;
        assert_eq!(
            count_3, 2,
            "Should still have 2 SSIs after force materialize"
        );

        // Verify no duplicates
        let duplicates: (i64,) = sqlx::query_as(
            r#"SELECT COUNT(*) FROM (
                SELECT ssi_name, COUNT(*) as cnt
                FROM custody.cbu_ssi
                WHERE cbu_id = $1
                GROUP BY ssi_name
                HAVING COUNT(*) > 1
            ) dups"#,
        )
        .bind(cbu_id)
        .fetch_one(&db.pool)
        .await?;

        assert_eq!(duplicates.0, 0, "Should have no duplicate SSIs");

        db.cleanup().await?;
        Ok(())
    }

    // =========================================================================
    // TEST: SSI Orphan Cleanup - Removed SSIs deleted from operational tables
    // =========================================================================

    #[tokio::test]
    async fn test_ssi_orphan_cleanup() -> Result<()> {
        let mut db = TestDb::new().await?;

        // Create profile with 3 SSIs
        let doc = build_test_document(&["SSI_A", "SSI_B", "SSI_C"]);
        let profile_id = db.create_profile_with_document(doc).await?;

        // First materialize
        db.materialize(false).await?;
        assert_eq!(db.count_ssis().await?, 3, "Should have 3 SSIs");
        assert!(db.ssi_exists("SSI_B").await?, "SSI_B should exist");

        // Update document to remove SSI_B
        let doc_v2 = build_test_document(&["SSI_A", "SSI_C"]);
        sqlx::query(
            r#"UPDATE "ob-poc".cbu_trading_profiles SET document = $1 WHERE profile_id = $2"#,
        )
        .bind(&doc_v2)
        .bind(profile_id)
        .execute(&db.pool)
        .await?;

        // Re-materialize
        db.materialize(true).await?;

        // Verify SSI_B is gone
        assert_eq!(
            db.count_ssis().await?,
            2,
            "Should have 2 SSIs after removal"
        );
        assert!(
            !db.ssi_exists("SSI_B").await?,
            "SSI_B should be deleted after removal from matrix"
        );
        assert!(db.ssi_exists("SSI_A").await?, "SSI_A should still exist");
        assert!(db.ssi_exists("SSI_C").await?, "SSI_C should still exist");

        db.cleanup().await?;
        Ok(())
    }

    // =========================================================================
    // TEST: CSA Duplicate Prevention - Multiple materializes don't create dupes
    // =========================================================================

    #[tokio::test]
    async fn test_csa_no_duplicates() -> Result<()> {
        let mut db = TestDb::new().await?;

        // First, ensure we have a counterparty entity
        // Use "Limited Company" entity type
        let legal_type_id: Uuid = sqlx::query_scalar(
            r#"SELECT entity_type_id FROM "ob-poc".entity_types WHERE name = 'Limited Company' LIMIT 1"#,
        )
        .fetch_one(&db.pool)
        .await?;

        let counterparty_name = db.name("counterparty");
        let counterparty_id: Uuid = sqlx::query_scalar(
            r#"INSERT INTO "ob-poc".entities (name, entity_type_id)
               VALUES ($1, $2)
               RETURNING entity_id"#,
        )
        .bind(&counterparty_name)
        .bind(legal_type_id)
        .fetch_one(&db.pool)
        .await?;

        // Create profile with ISDA + CSA
        let doc = json!({
            "universe": {
                "base_currency": "USD",
                "allowed_currencies": ["USD"],
                "allowed_markets": [],
                "instrument_classes": []
            },
            "investment_managers": [],
            "isda_agreements": [{
                "counterparty": {
                    "type": "UUID",
                    "value": counterparty_id.to_string()
                },
                "agreement_date": "2024-01-01",
                "governing_law": "ENGLISH",
                "effective_date": null,
                "product_coverage": [],
                "csa": {
                    "csa_type": "VM",
                    "threshold_amount": 10000000,
                    "threshold_currency": "USD",
                    "minimum_transfer_amount": null,
                    "rounding_amount": null,
                    "eligible_collateral": [],
                    "initial_margin": null,
                    "collateral_ssi_ref": null,
                    "collateral_ssi": null,
                    "valuation_time": null,
                    "valuation_timezone": null,
                    "notification_time": null,
                    "settlement_days": null,
                    "dispute_resolution": null
                }
            }],
            "settlement_config": null,
            "booking_rules": [],
            "standing_instructions": {},
            "pricing_matrix": [],
            "valuation_config": null,
            "constraints": null,
            "corporate_actions": null,
            "metadata": null
        });

        let _profile_id = db.create_profile_with_document(doc).await?;

        // Materialize 3 times
        for i in 0..3 {
            db.materialize(true).await?;
            let csa_count = db.count_csa().await?;
            assert_eq!(
                csa_count,
                1,
                "Should have exactly 1 CSA after materialize #{}, got {}",
                i + 1,
                csa_count
            );
        }

        db.cleanup().await?;
        Ok(())
    }

    // =========================================================================
    // TEST: ISDA Orphan Cleanup - Removed ISDAs deleted with CSAs
    // =========================================================================

    #[tokio::test]
    async fn test_isda_orphan_cleanup() -> Result<()> {
        let mut db = TestDb::new().await?;

        // Get entity type for legal entities
        let legal_type_id: Uuid = sqlx::query_scalar(
            r#"SELECT entity_type_id FROM "ob-poc".entity_types WHERE name = 'Limited Company' LIMIT 1"#,
        )
        .fetch_one(&db.pool)
        .await?;

        // Create 2 counterparty entities
        let cp1_name = db.name("goldman");
        let cp2_name = db.name("jpmorgan");

        let cp1_id: Uuid = sqlx::query_scalar(
            r#"INSERT INTO "ob-poc".entities (name, entity_type_id)
               VALUES ($1, $2) RETURNING entity_id"#,
        )
        .bind(&cp1_name)
        .bind(legal_type_id)
        .fetch_one(&db.pool)
        .await?;

        let cp2_id: Uuid = sqlx::query_scalar(
            r#"INSERT INTO "ob-poc".entities (name, entity_type_id)
               VALUES ($1, $2) RETURNING entity_id"#,
        )
        .bind(&cp2_name)
        .bind(legal_type_id)
        .fetch_one(&db.pool)
        .await?;

        // Create profile with 2 ISDAs
        let doc = json!({
            "universe": {
                "base_currency": "USD",
                "allowed_currencies": ["USD"],
                "allowed_markets": [],
                "instrument_classes": []
            },
            "investment_managers": [],
            "isda_agreements": [
                {
                    "counterparty": { "type": "UUID", "value": cp1_id.to_string() },
                    "agreement_date": "2024-01-01",
                    "governing_law": "ENGLISH",
                    "product_coverage": [],
                    "csa": null
                },
                {
                    "counterparty": { "type": "UUID", "value": cp2_id.to_string() },
                    "agreement_date": "2024-01-01",
                    "governing_law": "ENGLISH",
                    "product_coverage": [],
                    "csa": null
                }
            ],
            "settlement_config": null,
            "booking_rules": [],
            "standing_instructions": {},
            "pricing_matrix": [],
            "valuation_config": null,
            "constraints": null,
            "corporate_actions": null,
            "metadata": null
        });

        let profile_id = db.create_profile_with_document(doc).await?;

        // First materialize
        db.materialize(true).await?;
        assert_eq!(db.count_isda().await?, 2, "Should have 2 ISDAs");

        // Update document to remove first ISDA (goldman)
        let doc_v2 = json!({
            "universe": {
                "base_currency": "USD",
                "allowed_currencies": ["USD"],
                "allowed_markets": [],
                "instrument_classes": []
            },
            "investment_managers": [],
            "isda_agreements": [
                {
                    "counterparty": { "type": "UUID", "value": cp2_id.to_string() },
                    "agreement_date": "2024-01-01",
                    "governing_law": "ENGLISH",
                    "product_coverage": [],
                    "csa": null
                }
            ],
            "settlement_config": null,
            "booking_rules": [],
            "standing_instructions": {},
            "pricing_matrix": [],
            "valuation_config": null,
            "constraints": null,
            "corporate_actions": null,
            "metadata": null
        });

        sqlx::query(
            r#"UPDATE "ob-poc".cbu_trading_profiles SET document = $1 WHERE profile_id = $2"#,
        )
        .bind(&doc_v2)
        .bind(profile_id)
        .execute(&db.pool)
        .await?;

        // Re-materialize
        db.materialize(true).await?;

        // Verify goldman ISDA is gone
        assert_eq!(
            db.count_isda().await?,
            1,
            "Should have 1 ISDA after removal"
        );

        db.cleanup().await?;
        Ok(())
    }
}
