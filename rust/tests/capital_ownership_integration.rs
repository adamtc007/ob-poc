//! Integration tests for Capital Structure & Ownership Model
//!
//! These tests verify the database schema for:
//! - Share class creation and issuance
//! - Ownership computation with voting/economic rights
//! - Control position calculation
//! - Special rights handling
//! - Dilution instruments

#[cfg(feature = "database")]
mod capital_tests {
    use anyhow::Result;
    use sqlx::PgPool;
    use uuid::Uuid;

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
            let prefix = format!("captest_{}", &Uuid::now_v7().to_string()[..8]);
            Ok(Self { pool, prefix })
        }

        fn name(&self, base: &str) -> String {
            format!("{}_{}", self.prefix, base)
        }

        async fn cleanup(&self) -> Result<()> {
            let pattern = format!("{}%", self.prefix);

            // Clean up capital structure tables
            sqlx::query(
                r#"DELETE FROM kyc.special_rights WHERE issuer_entity_id IN
                   (SELECT entity_id FROM "ob-poc".entities WHERE name LIKE $1)"#,
            )
            .bind(&pattern)
            .execute(&self.pool)
            .await
            .ok();

            sqlx::query(
                r#"DELETE FROM kyc.dilution_instruments WHERE issuer_entity_id IN
                   (SELECT entity_id FROM "ob-poc".entities WHERE name LIKE $1)"#,
            )
            .bind(&pattern)
            .execute(&self.pool)
            .await
            .ok();

            sqlx::query(
                r#"DELETE FROM kyc.holdings WHERE share_class_id IN
                   (SELECT id FROM kyc.share_classes WHERE name LIKE $1)"#,
            )
            .bind(&pattern)
            .execute(&self.pool)
            .await
            .ok();

            sqlx::query(
                r#"DELETE FROM kyc.issuance_events WHERE share_class_id IN
                   (SELECT id FROM kyc.share_classes WHERE name LIKE $1)"#,
            )
            .bind(&pattern)
            .execute(&self.pool)
            .await
            .ok();

            sqlx::query(
                r#"DELETE FROM kyc.share_class_supply WHERE share_class_id IN
                   (SELECT id FROM kyc.share_classes WHERE name LIKE $1)"#,
            )
            .bind(&pattern)
            .execute(&self.pool)
            .await
            .ok();

            sqlx::query(
                r#"DELETE FROM kyc.share_class_identifiers WHERE share_class_id IN
                   (SELECT id FROM kyc.share_classes WHERE name LIKE $1)"#,
            )
            .bind(&pattern)
            .execute(&self.pool)
            .await
            .ok();

            sqlx::query(r#"DELETE FROM kyc.share_classes WHERE name LIKE $1"#)
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

        async fn get_or_create_cbu(&self) -> Result<Uuid> {
            // Get any existing CBU or create a test one
            let cbu = sqlx::query_scalar!(r#"SELECT cbu_id FROM "ob-poc".cbus LIMIT 1"#)
                .fetch_optional(&self.pool)
                .await?;

            match cbu {
                Some(id) => Ok(id),
                None => {
                    let id = sqlx::query_scalar!(
                        r#"INSERT INTO "ob-poc".cbus (name, jurisdiction) VALUES ($1, 'US') RETURNING cbu_id"#,
                        self.name("test_cbu")
                    )
                    .fetch_one(&self.pool)
                    .await?;
                    Ok(id)
                }
            }
        }

        async fn create_entity(&self, name: &str, type_code: &str) -> Result<Uuid> {
            let entity_id = sqlx::query_scalar!(
                r#"
                INSERT INTO "ob-poc".entities (entity_type_id, name)
                SELECT entity_type_id, $1 FROM "ob-poc".entity_types WHERE type_code = $2 LIMIT 1
                RETURNING entity_id
                "#,
                name,
                type_code
            )
            .fetch_one(&self.pool)
            .await?;
            Ok(entity_id)
        }

        async fn create_company(&self, name: &str) -> Result<Uuid> {
            self.create_entity(name, "LIMITED_COMPANY_PRIVATE").await
        }
    }

    // =========================================================================
    // SHARE CLASS TESTS
    // =========================================================================

    #[tokio::test]
    async fn test_share_class_creation() -> Result<()> {
        let db = TestDb::new().await?;
        let issuer_name = db.name("issuer_corp");
        let class_name = db.name("ordinary_a");

        let cbu_id = db.get_or_create_cbu().await?;
        let issuer_id = db.create_company(&issuer_name).await?;

        // Create share class
        let share_class_id = sqlx::query_scalar!(
            r#"
            INSERT INTO kyc.share_classes (
                cbu_id, issuer_entity_id, name, instrument_kind,
                votes_per_unit, economic_per_unit, status
            ) VALUES ($1, $2, $3, 'ORDINARY_EQUITY', 1.0, 1.0, 'active')
            RETURNING id
            "#,
            cbu_id,
            issuer_id,
            class_name
        )
        .fetch_one(&db.pool)
        .await?;

        // Verify share class was created
        let row = sqlx::query!(
            r#"SELECT name, instrument_kind, votes_per_unit FROM kyc.share_classes WHERE id = $1"#,
            share_class_id
        )
        .fetch_one(&db.pool)
        .await?;

        assert_eq!(row.name, class_name);
        assert_eq!(row.instrument_kind.as_deref(), Some("ORDINARY_EQUITY"));

        db.cleanup().await?;
        Ok(())
    }

    #[tokio::test]
    async fn test_share_class_with_super_voting() -> Result<()> {
        let db = TestDb::new().await?;
        let issuer_name = db.name("founder_corp");
        let class_a = db.name("class_a");
        let class_b = db.name("class_b");

        let cbu_id = db.get_or_create_cbu().await?;
        let issuer_id = db
            .create_entity(&issuer_name, "LIMITED_COMPANY_PRIVATE")
            .await?;

        // Create Class A (1 vote per share)
        sqlx::query!(
            r#"
            INSERT INTO kyc.share_classes (
                cbu_id, issuer_entity_id, name, instrument_kind,
                votes_per_unit, economic_per_unit, status
            ) VALUES ($1, $2, $3, 'ORDINARY_EQUITY', 1.0, 1.0, 'active')
            "#,
            cbu_id,
            issuer_id,
            class_a
        )
        .execute(&db.pool)
        .await?;

        // Create Class B (10 votes per share - super voting)
        let class_b_id = sqlx::query_scalar!(
            r#"
            INSERT INTO kyc.share_classes (
                cbu_id, issuer_entity_id, name, instrument_kind,
                votes_per_unit, economic_per_unit, status
            ) VALUES ($1, $2, $3, 'ORDINARY_EQUITY', 10.0, 1.0, 'active')
            RETURNING id
            "#,
            cbu_id,
            issuer_id,
            class_b
        )
        .fetch_one(&db.pool)
        .await?;

        // Verify super-voting class
        let row = sqlx::query!(
            r#"SELECT votes_per_unit FROM kyc.share_classes WHERE id = $1"#,
            class_b_id
        )
        .fetch_one(&db.pool)
        .await?;

        let votes: f64 = row
            .votes_per_unit
            .map(|d| d.to_string().parse().unwrap_or(0.0))
            .unwrap_or(0.0);
        assert!((votes - 10.0).abs() < 0.01, "Should have 10 votes per unit");

        db.cleanup().await?;
        Ok(())
    }

    // =========================================================================
    // SUPPLY & ISSUANCE TESTS
    // =========================================================================

    #[tokio::test]
    async fn test_share_class_supply_tracking() -> Result<()> {
        let db = TestDb::new().await?;
        let issuer_name = db.name("supply_corp");
        let class_name = db.name("tracked_shares");

        let cbu_id = db.get_or_create_cbu().await?;
        let issuer_id = db
            .create_entity(&issuer_name, "LIMITED_COMPANY_PRIVATE")
            .await?;

        // Create share class
        let share_class_id = sqlx::query_scalar!(
            r#"
            INSERT INTO kyc.share_classes (
                cbu_id, issuer_entity_id, name, instrument_kind,
                votes_per_unit, status
            ) VALUES ($1, $2, $3, 'ORDINARY_EQUITY', 1.0, 'active')
            RETURNING id
            "#,
            cbu_id,
            issuer_id,
            class_name
        )
        .fetch_one(&db.pool)
        .await?;

        // Insert supply state
        sqlx::query!(
            r#"
            INSERT INTO kyc.share_class_supply (
                share_class_id, as_of_date,
                authorized_units, issued_units, outstanding_units,
                treasury_units, reserved_units
            ) VALUES ($1, CURRENT_DATE, 10000000, 5000000, 4500000, 500000, 1000000)
            "#,
            share_class_id
        )
        .execute(&db.pool)
        .await?;

        // Query supply
        let supply = sqlx::query!(
            r#"
            SELECT
                authorized_units,
                issued_units,
                outstanding_units,
                treasury_units
            FROM kyc.share_class_supply
            WHERE share_class_id = $1
            "#,
            share_class_id
        )
        .fetch_one(&db.pool)
        .await?;

        // PostgreSQL NUMERIC returns without trailing zeros
        assert_eq!(supply.authorized_units.unwrap().to_string(), "10000000");
        assert_eq!(supply.issued_units.to_string(), "5000000");
        assert_eq!(supply.outstanding_units.to_string(), "4500000");
        assert_eq!(supply.treasury_units.unwrap().to_string(), "500000");

        db.cleanup().await?;
        Ok(())
    }

    // =========================================================================
    // HOLDINGS & CONTROL TESTS
    // =========================================================================

    #[tokio::test]
    async fn test_holdings_and_ownership() -> Result<()> {
        let db = TestDb::new().await?;
        let issuer_name = db.name("ctrl_issuer");
        let holder_name = db.name("majority_holder");
        let class_name = db.name("voting_shares");

        let cbu_id = db.get_or_create_cbu().await?;
        let issuer_id = db
            .create_entity(&issuer_name, "LIMITED_COMPANY_PRIVATE")
            .await?;
        let holder_id = db
            .create_entity(&holder_name, "LIMITED_COMPANY_PRIVATE")
            .await?;

        // Create share class
        let share_class_id = sqlx::query_scalar!(
            r#"
            INSERT INTO kyc.share_classes (
                cbu_id, issuer_entity_id, name, instrument_kind, votes_per_unit, status
            ) VALUES ($1, $2, $3, 'ORDINARY_EQUITY', 1.0, 'active')
            RETURNING id
            "#,
            cbu_id,
            issuer_id,
            class_name
        )
        .fetch_one(&db.pool)
        .await?;

        // Set supply - 1000 shares issued
        sqlx::query!(
            r#"
            INSERT INTO kyc.share_class_supply (share_class_id, as_of_date, issued_units, outstanding_units)
            VALUES ($1, CURRENT_DATE, 1000, 1000)
            "#,
            share_class_id
        )
        .execute(&db.pool)
        .await?;

        // Create holding - holder owns 600 shares (60%)
        sqlx::query!(
            r#"
            INSERT INTO kyc.holdings (share_class_id, investor_entity_id, units, status)
            VALUES ($1, $2, 600, 'active')
            "#,
            share_class_id,
            holder_id
        )
        .execute(&db.pool)
        .await?;

        // Verify holding
        let holding = sqlx::query!(
            r#"
            SELECT units, status
            FROM kyc.holdings
            WHERE share_class_id = $1 AND investor_entity_id = $2
            "#,
            share_class_id,
            holder_id
        )
        .fetch_one(&db.pool)
        .await?;

        assert_eq!(holding.units.to_string(), "600");
        assert_eq!(holding.status, "active");

        db.cleanup().await?;
        Ok(())
    }

    // =========================================================================
    // SPECIAL RIGHTS TESTS
    // =========================================================================

    #[tokio::test]
    async fn test_special_rights_creation() -> Result<()> {
        let db = TestDb::new().await?;
        let issuer_name = db.name("rights_issuer");
        let holder_name = db.name("rights_holder");
        let class_name = db.name("pref_shares");

        let cbu_id = db.get_or_create_cbu().await?;
        let issuer_id = db
            .create_entity(&issuer_name, "LIMITED_COMPANY_PRIVATE")
            .await?;
        let _holder_id = db
            .create_entity(&holder_name, "LIMITED_COMPANY_PRIVATE")
            .await?;

        // Create share class
        let share_class_id = sqlx::query_scalar!(
            r#"
            INSERT INTO kyc.share_classes (
                cbu_id, issuer_entity_id, name, instrument_kind, status
            ) VALUES ($1, $2, $3, 'PREFERENCE_EQUITY', 'active')
            RETURNING id
            "#,
            cbu_id,
            issuer_id,
            class_name
        )
        .fetch_one(&db.pool)
        .await?;

        // Create special rights (attached to share class)
        sqlx::query!(
            r#"
            INSERT INTO kyc.special_rights (
                issuer_entity_id, share_class_id, right_type, notes
            ) VALUES
            ($1, $2, 'BOARD_APPOINTMENT', 'Right to appoint one board member'),
            ($1, $2, 'VETO_MA', 'Veto right over M&A transactions')
            "#,
            issuer_id,
            share_class_id
        )
        .execute(&db.pool)
        .await?;

        // Query rights
        let rights = sqlx::query!(
            r#"
            SELECT right_type, notes
            FROM kyc.special_rights
            WHERE share_class_id = $1
            ORDER BY right_type
            "#,
            share_class_id
        )
        .fetch_all(&db.pool)
        .await?;

        assert_eq!(rights.len(), 2);
        assert_eq!(rights[0].right_type, "BOARD_APPOINTMENT");
        assert_eq!(rights[1].right_type, "VETO_MA");

        db.cleanup().await?;
        Ok(())
    }

    // =========================================================================
    // DILUTION INSTRUMENTS TESTS
    // =========================================================================

    #[tokio::test]
    async fn test_dilution_instrument_creation() -> Result<()> {
        let db = TestDb::new().await?;
        let issuer_name = db.name("option_issuer");
        let holder_name = db.name("option_holder");
        let class_name = db.name("common_shares");

        let cbu_id = db.get_or_create_cbu().await?;
        let issuer_id = db
            .create_entity(&issuer_name, "LIMITED_COMPANY_PRIVATE")
            .await?;
        let holder_id = db
            .create_entity(&holder_name, "PROPER_PERSON_NATURAL")
            .await?;

        // Create share class for conversion target
        let share_class_id = sqlx::query_scalar!(
            r#"
            INSERT INTO kyc.share_classes (
                cbu_id, issuer_entity_id, name, instrument_kind, status
            ) VALUES ($1, $2, $3, 'ORDINARY_EQUITY', 'active')
            RETURNING id
            "#,
            cbu_id,
            issuer_id,
            class_name
        )
        .fetch_one(&db.pool)
        .await?;

        // Create stock option grant
        let option_id = sqlx::query_scalar!(
            r#"
            INSERT INTO kyc.dilution_instruments (
                issuer_entity_id, converts_to_share_class_id, instrument_type,
                holder_entity_id, units_granted, exercise_price, exercise_currency,
                vesting_start_date, vesting_end_date, vesting_cliff_months,
                expiration_date, plan_name, status
            ) VALUES (
                $1, $2, 'STOCK_OPTION',
                $3, 10000, 1.50, 'USD',
                CURRENT_DATE, CURRENT_DATE + INTERVAL '4 years', 12,
                CURRENT_DATE + INTERVAL '10 years', 'Employee Option Plan 2024', 'ACTIVE'
            )
            RETURNING instrument_id
            "#,
            issuer_id,
            share_class_id,
            holder_id
        )
        .fetch_one(&db.pool)
        .await?;

        // Verify option was created
        let option = sqlx::query!(
            r#"
            SELECT instrument_type, units_granted, status
            FROM kyc.dilution_instruments
            WHERE instrument_id = $1
            "#,
            option_id
        )
        .fetch_one(&db.pool)
        .await?;

        assert_eq!(option.instrument_type, "STOCK_OPTION");
        assert_eq!(option.units_granted.to_string(), "10000");
        assert_eq!(option.status.as_deref(), Some("ACTIVE"));

        db.cleanup().await?;
        Ok(())
    }

    // =========================================================================
    // IDENTIFIER SCHEME TESTS
    // =========================================================================

    #[tokio::test]
    async fn test_share_class_identifiers() -> Result<()> {
        let db = TestDb::new().await?;
        let issuer_name = db.name("id_issuer");
        let class_name = db.name("identified_class");

        let cbu_id = db.get_or_create_cbu().await?;
        let issuer_id = db
            .create_entity(&issuer_name, "LIMITED_COMPANY_PRIVATE")
            .await?;

        // Create share class
        let share_class_id = sqlx::query_scalar!(
            r#"
            INSERT INTO kyc.share_classes (
                cbu_id, issuer_entity_id, name, instrument_kind, status
            ) VALUES ($1, $2, $3, 'ORDINARY_EQUITY', 'active')
            RETURNING id
            "#,
            cbu_id,
            issuer_id,
            class_name
        )
        .fetch_one(&db.pool)
        .await?;

        // Add identifiers
        sqlx::query!(
            r#"
            INSERT INTO kyc.share_class_identifiers (share_class_id, scheme_code, identifier_value, is_primary)
            VALUES
            ($1, 'ISIN', 'US1234567890', true),
            ($1, 'SEDOL', 'B123456', false),
            ($1, 'CUSIP', '123456789', false)
            "#,
            share_class_id
        )
        .execute(&db.pool)
        .await?;

        // Query identifiers
        let identifiers = sqlx::query!(
            r#"
            SELECT scheme_code, identifier_value, is_primary
            FROM kyc.share_class_identifiers
            WHERE share_class_id = $1
            ORDER BY is_primary DESC, scheme_code
            "#,
            share_class_id
        )
        .fetch_all(&db.pool)
        .await?;

        assert_eq!(identifiers.len(), 3);
        assert_eq!(identifiers[0].scheme_code, "ISIN");
        assert!(identifiers[0].is_primary.unwrap_or(false));
        assert_eq!(identifiers[0].identifier_value, "US1234567890");

        db.cleanup().await?;
        Ok(())
    }

    // =========================================================================
    // ISSUANCE EVENTS TESTS
    // =========================================================================

    #[tokio::test]
    async fn test_issuance_events() -> Result<()> {
        let db = TestDb::new().await?;
        let issuer_name = db.name("issuance_corp");
        let class_name = db.name("issued_shares");

        let cbu_id = db.get_or_create_cbu().await?;
        let issuer_id = db
            .create_entity(&issuer_name, "LIMITED_COMPANY_PRIVATE")
            .await?;

        // Create share class
        let share_class_id = sqlx::query_scalar!(
            r#"
            INSERT INTO kyc.share_classes (
                cbu_id, issuer_entity_id, name, instrument_kind, status
            ) VALUES ($1, $2, $3, 'ORDINARY_EQUITY', 'active')
            RETURNING id
            "#,
            cbu_id,
            issuer_id,
            class_name
        )
        .fetch_one(&db.pool)
        .await?;

        // Record initial issuance
        sqlx::query!(
            r#"
            INSERT INTO kyc.issuance_events (
                share_class_id, issuer_entity_id, event_type, effective_date,
                units_delta, price_per_unit, price_currency,
                notes
            ) VALUES ($1, $2, 'INITIAL_ISSUE', CURRENT_DATE, 1000000, 1.00, 'USD', 'Seed round')
            "#,
            share_class_id,
            issuer_id
        )
        .execute(&db.pool)
        .await?;

        // Record follow-on issuance
        sqlx::query!(
            r#"
            INSERT INTO kyc.issuance_events (
                share_class_id, issuer_entity_id, event_type, effective_date,
                units_delta, price_per_unit, price_currency,
                notes
            ) VALUES ($1, $2, 'NEW_ISSUE', CURRENT_DATE, 500000, 2.50, 'USD', 'Series A')
            "#,
            share_class_id,
            issuer_id
        )
        .execute(&db.pool)
        .await?;

        // Query events
        let events = sqlx::query!(
            r#"
            SELECT event_type, units_delta, notes
            FROM kyc.issuance_events
            WHERE share_class_id = $1
            ORDER BY effective_date, created_at
            "#,
            share_class_id
        )
        .fetch_all(&db.pool)
        .await?;

        assert_eq!(events.len(), 2);
        assert_eq!(events[0].event_type, "INITIAL_ISSUE");
        assert_eq!(events[0].units_delta.to_string(), "1000000");
        assert_eq!(events[1].event_type, "NEW_ISSUE");

        db.cleanup().await?;
        Ok(())
    }
}
