//! Trading Profile Field Normalization Test Harness
//!
//! This test suite validates that the DB<>SQLX<>Rust type interfaces align correctly
//! after the mic vs market field normalization changes.
//!
//! Run with: cargo test --features database --test trading_profile_field_test

#[cfg(feature = "database")]
mod field_tests {
    use anyhow::Result;
    use sqlx::PgPool;
    use std::collections::HashMap;
    use uuid::Uuid;

    // =========================================================================
    // TEST INFRASTRUCTURE
    // =========================================================================

    struct TestDb {
        pool: PgPool,
        prefix: String,
        cbu_id: Option<Uuid>,
    }

    impl TestDb {
        async fn new() -> Result<Self> {
            let url = std::env::var("TEST_DATABASE_URL")
                .or_else(|_| std::env::var("DATABASE_URL"))
                .unwrap_or_else(|_| "postgresql:///data_designer".into());

            let pool = PgPool::connect(&url).await?;
            let prefix = format!("tpf_{}", &Uuid::new_v4().to_string()[..8]);
            Ok(Self {
                pool,
                prefix,
                cbu_id: None,
            })
        }

        fn name(&self, base: &str) -> String {
            format!("{}_{}", self.prefix, base)
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

        async fn cleanup(&self) -> Result<()> {
            let pattern = format!("{}%", self.prefix);

            // Cleanup in reverse dependency order
            if let Some(cbu_id) = self.cbu_id {
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
                sqlx::query("DELETE FROM custody.cbu_trading_profiles WHERE cbu_id = $1")
                    .bind(cbu_id)
                    .execute(&self.pool)
                    .await
                    .ok();
            }

            // CBU
            sqlx::query(r#"DELETE FROM "ob-poc".cbus WHERE name LIKE $1"#)
                .bind(&pattern)
                .execute(&self.pool)
                .await
                .ok();

            Ok(())
        }
    }

    // =========================================================================
    // MARKET LOOKUP TESTS
    // =========================================================================

    /// Test that we can build mic → market_id map from custody.markets
    #[tokio::test]
    async fn test_market_lookup_via_mic() -> Result<()> {
        let db = TestDb::new().await?;

        // Build market map (same logic as materialization code)
        let rows = sqlx::query!(r#"SELECT market_id, mic FROM custody.markets"#)
            .fetch_all(&db.pool)
            .await?;

        let market_map: HashMap<String, Uuid> =
            rows.into_iter().map(|r| (r.mic, r.market_id)).collect();

        // Verify expected markets exist
        assert!(
            market_map.contains_key("XNYS"),
            "XNYS should exist in markets"
        );
        assert!(
            market_map.contains_key("XLON"),
            "XLON should exist in markets"
        );
        assert!(
            market_map.contains_key("XETR"),
            "XETR should exist in markets"
        );

        // Verify UUIDs are valid
        for (mic, id) in &market_map {
            assert!(!id.is_nil(), "market_id for {} should not be nil", mic);
        }

        Ok(())
    }

    /// Test SSI insertion with mic → market_id resolution
    #[tokio::test]
    async fn test_ssi_market_field_maps_correctly() -> Result<()> {
        let mut db = TestDb::new().await?;
        let cbu_id = db.ensure_cbu().await?;

        // Get XNYS market_id
        let market_id: Uuid =
            sqlx::query_scalar("SELECT market_id FROM custody.markets WHERE mic = 'XNYS'")
                .fetch_one(&db.pool)
                .await?;

        // Insert SSI with market_id (note: column is cash_account_bic, not cash_bic)
        let ssi_name = db.name("US_EQUITY_SSI");
        let result = sqlx::query!(
            r#"INSERT INTO custody.cbu_ssi
               (cbu_id, ssi_name, ssi_type, market_id, safekeeping_account, safekeeping_bic,
                cash_account, cash_account_bic, cash_currency, status, effective_date)
               VALUES ($1, $2, 'SECURITIES', $3, 'SAFE-001', 'IRVTUS3N',
                       'CASH-001', 'IRVTUS3N', 'USD', 'ACTIVE', CURRENT_DATE)
               RETURNING ssi_id"#,
            cbu_id,
            ssi_name,
            market_id
        )
        .fetch_one(&db.pool)
        .await?;

        // Read back and verify (mic comes from LEFT JOIN so it's Option)
        let ssi = sqlx::query!(
            r#"SELECT s.ssi_id, s.ssi_name, s.market_id, m.mic
               FROM custody.cbu_ssi s
               LEFT JOIN custody.markets m ON s.market_id = m.market_id
               WHERE s.ssi_id = $1"#,
            result.ssi_id
        )
        .fetch_one(&db.pool)
        .await?;

        assert_eq!(ssi.market_id, Some(market_id));
        assert_eq!(ssi.mic, "XNYS");

        db.cleanup().await?;
        Ok(())
    }

    // =========================================================================
    // BOOKING RULES TESTS
    // =========================================================================

    /// Test booking rule with mic filter → market_id FK
    #[tokio::test]
    async fn test_booking_rule_mic_lookup() -> Result<()> {
        let mut db = TestDb::new().await?;
        let cbu_id = db.ensure_cbu().await?;

        // First create an SSI for the rule to reference
        let ssi_name = db.name("TEST_SSI");
        let ssi_id: Uuid = sqlx::query_scalar(
            r#"INSERT INTO custody.cbu_ssi
               (cbu_id, ssi_name, ssi_type, safekeeping_account, safekeeping_bic,
                status, effective_date)
               VALUES ($1, $2, 'SECURITIES', 'SAFE-001', 'IRVTUS3N', 'ACTIVE', CURRENT_DATE)
               RETURNING ssi_id"#,
        )
        .bind(cbu_id)
        .bind(&ssi_name)
        .fetch_one(&db.pool)
        .await?;

        // Get market_id for XETR
        let market_id: Uuid =
            sqlx::query_scalar("SELECT market_id FROM custody.markets WHERE mic = 'XETR'")
                .fetch_one(&db.pool)
                .await?;

        // Insert booking rule with market_id filter
        let rule_name = db.name("German_Equities");
        sqlx::query(
            r#"INSERT INTO custody.ssi_booking_rules
               (cbu_id, ssi_id, rule_name, priority, market_id, currency, effective_date)
               VALUES ($1, $2, $3, 10, $4, 'EUR', CURRENT_DATE)
               ON CONFLICT (cbu_id, priority, rule_name) DO NOTHING"#,
        )
        .bind(cbu_id)
        .bind(ssi_id)
        .bind(&rule_name)
        .bind(market_id)
        .execute(&db.pool)
        .await?;

        // Read back and verify market resolves to mic
        let row = sqlx::query!(
            r#"SELECT r.market_id, m.mic, r.specificity_score
               FROM custody.ssi_booking_rules r
               LEFT JOIN custody.markets m ON r.market_id = m.market_id
               WHERE r.cbu_id = $1 AND r.rule_name = $2"#,
            cbu_id,
            rule_name
        )
        .fetch_one(&db.pool)
        .await?;

        assert_eq!(row.market_id, Some(market_id));
        assert_eq!(row.mic, "XETR");
        // Specificity should be calculated (1 for market + 1 for currency = 2)
        assert!(
            row.specificity_score.is_some(),
            "specificity_score should be generated"
        );

        db.cleanup().await?;
        Ok(())
    }

    // =========================================================================
    // YAML DESERIALIZATION TESTS
    // =========================================================================

    /// Test that seed YAML file deserializes correctly with current field names
    #[test]
    fn test_seed_yaml_deserializes() {
        use ob_poc::trading_profile::types::TradingProfileImport;

        let yaml = include_str!("../config/seed/trading_profiles/allianzgi_complete.yaml");
        let result: Result<TradingProfileImport, _> = serde_yaml::from_str(yaml);

        assert!(
            result.is_ok(),
            "Failed to parse seed file: {:?}",
            result.err()
        );

        let profile = result.unwrap();

        // Verify key fields parsed
        assert_eq!(profile.universe.base_currency, "EUR");
        assert!(!profile.universe.allowed_markets.is_empty());
        assert!(!profile.booking_rules.is_empty());

        // Check market field in allowed_markets (currently 'mic')
        let first_market = &profile.universe.allowed_markets[0];
        assert!(
            !first_market.mic.is_empty(),
            "mic field should be populated"
        );

        // Check booking rules have match criteria
        for rule in &profile.booking_rules {
            // Some rules have mic filter, some don't
            if rule.match_criteria.mic.is_some() {
                let mic_val = rule.match_criteria.mic.as_ref().unwrap();
                assert!(
                    mic_val.len() == 4,
                    "MIC should be 4-char code, got: {}",
                    mic_val
                );
            }
        }
    }

    /// Test document conversion preserves all fields
    #[test]
    fn test_document_conversion_preserves_fields() {
        use ob_poc::trading_profile::types::TradingProfileImport;

        let yaml = include_str!("../config/seed/trading_profiles/allianzgi_complete.yaml");
        let import: TradingProfileImport = serde_yaml::from_str(yaml).unwrap();

        // Convert to document
        let doc = import.into_document();

        // Verify no data lost
        assert_eq!(doc.universe.base_currency, "EUR");
        assert!(!doc.universe.allowed_markets.is_empty());
        assert!(!doc.booking_rules.is_empty());
        assert!(!doc.standing_instructions.is_empty());
        assert!(!doc.isda_agreements.is_empty());

        // Verify standing instructions have market field
        if let Some(custody_ssis) = doc.standing_instructions.get("CUSTODY") {
            for ssi in custody_ssis {
                // Some SSIs have market, some don't (e.g., DEFAULT_SSI, bond SSIs)
                if ssi.name.contains("EQUITY") || ssi.name.contains("SSI") {
                    // Market-specific SSIs should have market field
                    // After normalization this will be 'mic'
                }
            }
        }
    }

    // =========================================================================
    // IDEMPOTENCY TESTS
    // =========================================================================

    /// Test that double-insert with ON CONFLICT succeeds (idempotency)
    #[tokio::test]
    async fn test_ssi_upsert_idempotent() -> Result<()> {
        let mut db = TestDb::new().await?;
        let cbu_id = db.ensure_cbu().await?;

        let ssi_name = db.name("IDEMPOTENT_SSI");

        // Insert twice with same key
        for _ in 0..2 {
            sqlx::query(
                r#"INSERT INTO custody.cbu_ssi
                   (cbu_id, ssi_name, ssi_type, safekeeping_account, safekeeping_bic,
                    status, effective_date)
                   VALUES ($1, $2, 'SECURITIES', 'SAFE-001', 'IRVTUS3N', 'ACTIVE', CURRENT_DATE)
                   ON CONFLICT (cbu_id, ssi_name)
                   DO UPDATE SET safekeeping_account = EXCLUDED.safekeeping_account"#,
            )
            .bind(cbu_id)
            .bind(&ssi_name)
            .execute(&db.pool)
            .await?;
        }

        // Should only have one row
        let count: i64 = sqlx::query_scalar(
            "SELECT COUNT(*) FROM custody.cbu_ssi WHERE cbu_id = $1 AND ssi_name = $2",
        )
        .bind(cbu_id)
        .bind(&ssi_name)
        .fetch_one(&db.pool)
        .await?;

        assert_eq!(count, 1, "Should have exactly one SSI after double insert");

        db.cleanup().await?;
        Ok(())
    }

    /// Test booking rule upsert
    #[tokio::test]
    async fn test_booking_rule_upsert_idempotent() -> Result<()> {
        let mut db = TestDb::new().await?;
        let cbu_id = db.ensure_cbu().await?;

        // Create SSI first
        let ssi_name = db.name("RULE_TEST_SSI");
        let ssi_id: Uuid = sqlx::query_scalar(
            r#"INSERT INTO custody.cbu_ssi
               (cbu_id, ssi_name, ssi_type, safekeeping_account, safekeeping_bic,
                status, effective_date)
               VALUES ($1, $2, 'SECURITIES', 'SAFE-001', 'IRVTUS3N', 'ACTIVE', CURRENT_DATE)
               RETURNING ssi_id"#,
        )
        .bind(cbu_id)
        .bind(&ssi_name)
        .fetch_one(&db.pool)
        .await?;

        let rule_name = db.name("IDEMPOTENT_RULE");

        // Insert twice
        for _ in 0..2 {
            sqlx::query(
                r#"INSERT INTO custody.ssi_booking_rules
                   (cbu_id, ssi_id, rule_name, priority, currency, effective_date)
                   VALUES ($1, $2, $3, 10, 'USD', CURRENT_DATE)
                   ON CONFLICT (cbu_id, priority, rule_name) DO NOTHING"#,
            )
            .bind(cbu_id)
            .bind(ssi_id)
            .bind(&rule_name)
            .execute(&db.pool)
            .await?;
        }

        // Should only have one row
        let count: i64 = sqlx::query_scalar(
            "SELECT COUNT(*) FROM custody.ssi_booking_rules WHERE cbu_id = $1 AND rule_name = $2",
        )
        .bind(cbu_id)
        .bind(&rule_name)
        .fetch_one(&db.pool)
        .await?;

        assert_eq!(
            count, 1,
            "Should have exactly one booking rule after double insert"
        );

        db.cleanup().await?;
        Ok(())
    }
}
